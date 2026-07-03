// Módulo ECS - Entity Component System v0.10.0 [FASE 7]
//
// FASE 7: Nuevos tipos de edificios (Hospital, Escuela, Policía)
// - RenderCache integrado con rebuild_from_world
// - Spatial Hashing + Query Fusion
//
// GameWorld con todos los sistemas integrados:
// [#361] LaneManager - Tráfico con carriles
// [#392] DesignTool - Diseño urbano interactivo
// [M#1..M#10] 10 sistemas de realismo

use crate::object_pool::EntityPool;
use crate::input::InputState;
use crate::terrain::TerrainMap;
use crate::quadtree::Quadtree;
use crate::flow_field::FlowFieldManager;
use crate::bitboard::BitGrid;
use crate::traffic_lanes::LaneManager;
use crate::interactive::DesignTool;
use crate::utilities::UtilityGrid;
use crate::road_wear::RoadWearGrid;
use crate::land_value::{LandValueHeatmap, PollutionHeatmap};
use crate::tax_system::MunicipalFinance;
use crate::parking::ParkingManager;
use crate::waste_mgmt::WasteManager;
use crate::customization::CustomizationManager;
use crate::politics::PoliticalSystem;
use crate::render_cache::RenderCache;
use rand::rngs::SmallRng;
use rand::SeedableRng;

// ---------------------------------------------------------------------------
// COMPONENTES (alineados a 64B para caché L1)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Position { pub x: f32, pub y: f32 }
impl Position {
    #[inline(always)] pub fn new(x: f32, y: f32) -> Self { Position { x, y } }
}

#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Velocity { pub dx: f32, pub dy: f32 }
impl Velocity {
    #[inline(always)] pub fn new(dx: f32, dy: f32) -> Self { Velocity { dx, dy } }
}

#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Renderable { pub shape_type: u8, pub color: u32, pub size: f32, pub layer: u8 }
impl Renderable {
    #[inline(always)] pub fn circle(color: u32, radius: f32, layer: u8) -> Self { Renderable { shape_type: 0, color, size: radius, layer } }
    #[inline(always)] pub fn rect(color: u32, width: f32, layer: u8) -> Self { Renderable { shape_type: 1, color, size: width, layer } }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(align(64))]
pub enum ZoneType { Residential, Commercial, Industrial, Agricultural, Road, Park }

#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct ZoneComponent { pub zone_type: ZoneType, pub density: u8 }

#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct TrafficCar { pub speed: f32, pub max_speed: f32, pub acceleration: f32, pub lane_position: f32, pub lane_id: u32 }

#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct ResourceStorage { pub money: f32, pub food: f32, pub goods: f32 }

#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct ConstructionState { pub progress: f32, pub building_type: BuildingType }

/// [FASE 7]: Nuevos tipos de edificios públicos
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BuildingType { 
    House, Apartment, Shop, Office, Factory, Farm,
    Hospital, School, Police,  // [FASE 7]
}

#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Camera { pub offset_x: f32, pub offset_y: f32, pub zoom: f32 }

#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Lifetime { pub remaining_ticks: u32 }

#[derive(Copy, Clone, Debug)]
pub struct QuadIndex(pub u32);

// ---------------------------------------------------------------------------
// SPATIAL GRID — Búsqueda O(1) de entidades por posición [FASE 6]
// ---------------------------------------------------------------------------

pub const SPATIAL_CELL_SIZE: f32 = 8.0;
pub const SPATIAL_GRID_DIM: usize = 16;
pub const SPATIAL_GRID_SIZE: usize = SPATIAL_GRID_DIM * SPATIAL_GRID_DIM;

#[derive(Clone)]
pub struct SpatialGrid {
    pub cells: [[Vec<u64>; SPATIAL_GRID_DIM]; SPATIAL_GRID_DIM],
    pub dirty: bool,
}

impl SpatialGrid {
    pub fn new() -> Self {
        let mut cells: [[Vec<u64>; SPATIAL_GRID_DIM]; SPATIAL_GRID_DIM] =
            unsafe { std::mem::zeroed() };
        for row in cells.iter_mut() {
            for cell in row.iter_mut() {
                unsafe { std::ptr::write(cell, Vec::with_capacity(64)); }
            }
        }
        SpatialGrid { cells, dirty: true }
    }

    #[inline(always)]
    fn cell_index(x: f32, y: f32) -> (usize, usize) {
        let cx = (x / SPATIAL_CELL_SIZE) as usize % SPATIAL_GRID_DIM;
        let cy = (y / SPATIAL_CELL_SIZE) as usize % SPATIAL_GRID_DIM;
        (cx, cy)
    }

    pub fn clear(&mut self) {
        for row in self.cells.iter_mut() {
            for cell in row.iter_mut() { cell.clear(); }
        }
        self.dirty = false;
    }

    #[inline(always)]
    pub fn insert(&mut self, x: f32, y: f32, entity_bits: u64) {
        let (cx, cy) = Self::cell_index(x, y);
        unsafe { self.cells.get_unchecked_mut(cy).get_unchecked_mut(cx).push(entity_bits); }
    }

    pub fn rebuild(&mut self, world: &hecs::World) {
        self.clear();
        for (entity, pos) in world.query::<&Position>().iter() {
            let bits = entity.to_bits().get();
            self.insert(pos.x, pos.y, bits);
        }
        self.dirty = false;
    }

    #[inline]
    #[inline]
    pub fn query_near(&self, x: f32, y: f32, radius: f32) -> SpatialQueryIter<'_> {

        let (cx, cy) = Self::cell_index(x, y);
        let cell_radius = ((radius / SPATIAL_CELL_SIZE).ceil() as usize).min(SPATIAL_GRID_DIM / 2);
        let min_x = if cx >= cell_radius { cx - cell_radius } else { 0 };
        let max_x = (cx + cell_radius).min(SPATIAL_GRID_DIM - 1);
        let min_y = if cy >= cell_radius { cy - cell_radius } else { 0 };
        let max_y = (cy + cell_radius).min(SPATIAL_GRID_DIM - 1);
        SpatialQueryIter { grid: self, min_x, max_x, min_y, max_y, current_cx: min_x, current_cy: min_y, current_idx: 0, done: false }
    }
pub struct SpatialQueryIter<'a> {
    grid: &'a SpatialGrid,
    min_x: usize, max_x: usize,
    #[allow(dead_code)]
    min_y: usize, max_y: usize,
    current_cx: usize, current_cy: usize,

    current_idx: usize,
    done: bool,
}

impl<'a> Iterator for SpatialQueryIter<'a> {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        if self.done { return None; }
        loop {
            if self.current_cy > self.max_y { self.done = true; return None; }
            let cell = &self.grid.cells[self.current_cy][self.current_cx];
            if self.current_idx < cell.len() {
                let bits = cell[self.current_idx];
                self.current_idx += 1;
                return Some(bits);
            }
            self.current_idx = 0;
            self.current_cx += 1;
            if self.current_cx > self.max_x {
                self.current_cx = self.min_x;
                self.current_cy += 1;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// GAMEWORLD
// ---------------------------------------------------------------------------

pub struct GameWorld {
    pub world: hecs::World,
    pub spatial_grid: SpatialGrid,
    pub render_cache: RenderCache,
    pub pool: EntityPool,
    pub sim_tick: u64,
    pub time_of_day: u16,
    pub rng: SmallRng,
    pub terrain: TerrainMap,
    pub quadtree: Quadtree,
    pub flow_fields: FlowFieldManager,
    pub bitgrid: BitGrid,
    pub lane_manager: LaneManager,
    pub design_tool: DesignTool,
    pub water_grid: UtilityGrid,
    pub power_grid: UtilityGrid,
    pub road_wear: RoadWearGrid,
    pub land_value_map: LandValueHeatmap,
    pub pollution_map: PollutionHeatmap,
    pub finance: MunicipalFinance,
    pub parking_mgr: ParkingManager,
    pub waste_mgr: WasteManager,
    pub customization: CustomizationManager,
    pub politics: PoliticalSystem,
    pub grid_size: i32,
}

pub fn create_world(_pool: &mut EntityPool) -> GameWorld {
    let mut world = hecs::World::new();
    let grid_size: i32 = 128;

    let terrain = TerrainMap::generate(42);
    let quadtree = Quadtree::new(grid_size as f32, grid_size as f32);
    let flow_fields = FlowFieldManager::generate_all();
    let bitgrid = BitGrid::new();

    let mut lane_manager = LaneManager::new();
    lane_manager.generate_default_network();

    let design_tool = DesignTool::new();

    let mut water_grid = UtilityGrid::new(crate::utilities::UtilitySourceType::WaterTower);
    water_grid.add_source(64.0, 64.0);
    let mut power_grid = UtilityGrid::new(crate::utilities::UtilitySourceType::PowerPlant);
    power_grid.add_source(64.0, 64.0);

    let road_wear = RoadWearGrid::new();
    let land_value_map = LandValueHeatmap::new();
    let pollution_map = PollutionHeatmap::new();
    let finance = MunicipalFinance::new();

    let mut parking_mgr = ParkingManager::new();
    for i in 0..6 {
        let ave_x = 20.0 + i as f32 * 20.0;
        for y in (10..120).step_by(4) {
            parking_mgr.add_street_parking(ave_x, y as f32, 4, false, 0.0);
        }
    }

    let waste_mgr = WasteManager::new();
    let customization = CustomizationManager::new();
    let politics = PoliticalSystem::new();
    let _render_cache = RenderCache::new();


    // Cámara
    world.spawn((
        Camera { offset_x: grid_size as f32 / 2.0, offset_y: grid_size as f32 / 2.0, zoom: 1.0 },
        Position::new(0.0, 0.0),
    ));

    // Pool de coches
    for i in 0..40 {
        let lane_id = if i < lane_manager.lanes.len() as i32 {
            i as u32
        } else {
            (i as u32) % lane_manager.lanes.len().max(1) as u32
        };
        let (sx, sy) = if (lane_id as usize) < lane_manager.lanes.len() {
            lane_manager.lanes[lane_id as usize].position_at(0.1 + (i as f32 * 0.02))
        } else {
            (i as f32 * 3.0 + 5.0, 60.0)
        };

        world.spawn((
            Position::new(sx, sy),
            Velocity::new(0.0, 0.0),
            TrafficCar { speed: (i as f32 % 5.0 + 1.0) * 2.0, max_speed: 13.8, acceleration: 0.0, lane_position: i as f32 / 40.0, lane_id },
            Renderable::circle(0xFF_FF_AA_00, 1.2, 5),
        ));
    }

    // [FASE 7]: Edificios iniciales con nuevos tipos
    let buildings: [(f32, f32, BuildingType); 14] = [
        (30.0, 30.0, BuildingType::House),
        (35.0, 30.0, BuildingType::Shop),
        (40.0, 30.0, BuildingType::Factory),
        (30.0, 36.0, BuildingType::Apartment),
        (35.0, 36.0, BuildingType::Office),
        (40.0, 36.0, BuildingType::Farm),
        (60.0, 45.0, BuildingType::House),
        (64.0, 45.0, BuildingType::Shop),
        // [FASE 7]: Edificios públicos
        (50.0, 60.0, BuildingType::Hospital),
        (55.0, 60.0, BuildingType::School),
        (60.0, 60.0, BuildingType::Police),
        (80.0, 25.0, BuildingType::Hospital),
        (85.0, 25.0, BuildingType::School),
        (90.0, 25.0, BuildingType::Police),
    ];

    for &(bx, by, btype) in &buildings {
        let color = crate::render_cache::building_color(btype);
        world.spawn((
            Position::new(bx, by),
            Renderable::rect(color, 3.0, 3),
            ConstructionState { progress: 1.0, building_type: btype },
            ResourceStorage { money: 1000.0, food: 100.0, goods: 50.0 },
        ));
    }

    // Zonas
    let zones: [(f32, f32, f32, f32, ZoneType); 4] = [
        (15.0, 15.0, 30.0, 18.0, ZoneType::Residential),
        (55.0, 15.0, 25.0, 18.0, ZoneType::Commercial),
        (15.0, 50.0, 25.0, 18.0, ZoneType::Industrial),
        (55.0, 50.0, 25.0, 18.0, ZoneType::Agricultural),
    ];

    for &(zx, zy, zw, zh, ztype) in &zones {
        let color = crate::render_cache::zone_color(ztype);
        for dx in 0..zw as i32 {
            for dy in 0..zh as i32 {
                world.spawn((
                    Position::new(zx + dx as f32, zy + dy as f32),
                    Renderable::rect(color, 1.0, 1),
                    ZoneComponent { zone_type: ztype, density: 2 },
                ));
            }
        }
    }

    let mut spatial_grid = SpatialGrid::new();
    spatial_grid.rebuild(&world);

    // [FASE 7]: Inicializar RenderCache
    let mut render_cache = RenderCache::new();
    render_cache.rebuild_from_world(&world);

    GameWorld {
        world,
        spatial_grid,
        render_cache,
        pool: EntityPool::new(1000),
        sim_tick: 0, time_of_day: 7 * 60,
        rng: SmallRng::seed_from_u64(42),
        terrain, quadtree, flow_fields, bitgrid,
        lane_manager, design_tool,
        water_grid, power_grid, road_wear,
        land_value_map, pollution_map,
        finance, parking_mgr, waste_mgr,
        customization, politics,
        grid_size,
    }
}

pub fn rebuild_spatial_grid(game_world: &mut GameWorld) {
    game_world.spatial_grid.rebuild(&game_world.world);
}

#[inline(always)]
pub fn entity_count(game_world: &GameWorld) -> usize {
    game_world.world.len() as usize
}

pub fn process_input(game_world: &mut GameWorld, input: &InputState) {
    let move_speed: f32 = 0.5;
    for (_entity, (camera,)) in game_world.world.query::<(&mut Camera,)>().iter() {
        if input.is_key_down(crate::input::GameKey::W) || input.is_key_down(crate::input::GameKey::Up) { camera.offset_y -= move_speed; }
        if input.is_key_down(crate::input::GameKey::S) || input.is_key_down(crate::input::GameKey::Down) { camera.offset_y += move_speed; }
        if input.is_key_down(crate::input::GameKey::A) || input.is_key_down(crate::input::GameKey::Left) { camera.offset_x -= move_speed; }
        if input.is_key_down(crate::input::GameKey::D) || input.is_key_down(crate::input::GameKey::Right) { camera.offset_x += move_speed; }
        if input.is_key_down(crate::input::GameKey::PageUp) { camera.zoom = (camera.zoom * 1.05_f32).min(4.0); }
        if input.is_key_down(crate::input::GameKey::PageDown) { camera.zoom = (camera.zoom / 1.05_f32).max(0.25); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spatial_grid_insert() {
        let mut grid = SpatialGrid::new();
        grid.insert(10.0, 10.0, 42);
        grid.insert(10.0, 10.0, 43);
        let (cx, cy) = SpatialGrid::cell_index(10.0, 10.0);
        assert_eq!(grid.cells[cy][cx].len(), 2);
    }

    #[test]
    fn test_spatial_query_near() {
        let mut grid = SpatialGrid::new();
        grid.insert(10.0, 10.0, 1);
        grid.insert(12.0, 12.0, 2);
        grid.insert(64.0, 64.0, 3);
        let nearby: Vec<u64> = grid.query_near(10.0, 10.0, 5.0).collect();
        assert!(nearby.contains(&1));
        assert!(nearby.contains(&2));
        assert!(!nearby.contains(&3));
    }

    #[test]
    fn test_spatial_rebuild() {
        let mut pool = EntityPool::new(100);
        let gw = create_world(&mut pool);
        assert!(!gw.spatial_grid.dirty);
        let total_in_grid: usize = gw.spatial_grid.cells.iter()
            .flat_map(|row| row.iter()).map(|cell| cell.len()).sum();
        assert!(total_in_grid > 0);
    }

    #[test]
    fn test_create_world() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        assert!(gw.world.len() > 0);
        assert_eq!(gw.grid_size, 128);
    }

    #[test]
    fn test_new_building_types_exist() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        let has_hospital = gw.world.query::<&ConstructionState>().iter()
            .any(|(_, cs)| cs.building_type == BuildingType::Hospital);
        let has_school = gw.world.query::<&ConstructionState>().iter()
            .any(|(_, cs)| cs.building_type == BuildingType::School);
        let has_police = gw.world.query::<&ConstructionState>().iter()
            .any(|(_, cs)| cs.building_type == BuildingType::Police);
        assert!(has_hospital, "Debe haber hospital");
        assert!(has_school, "Debe haber escuela");
        assert!(has_police, "Debe haber policía");
    }

    #[test]
    fn test_finance_exists() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        assert!(gw.finance.treasury > 0.0);
    }

    #[test]
    fn test_parking_exists() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        assert!(!gw.parking_mgr.street_segments.is_empty());
    }

    #[test]
    fn test_waste_mgr_exists() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        assert_eq!(gw.waste_mgr.landfills.len(), 0);
    }

    #[test]
    fn test_politics_exists() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        assert_eq!(gw.politics.districts.len(), 9);
    }

    #[test]
    fn test_entity_count() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        assert!(gw.world.len() > 100);
    }

    #[test]
    fn test_render_cache_filled() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        assert!(gw.render_cache.total_entries() > 0, "RenderCache debe llenarse en create_world");
    }
}
