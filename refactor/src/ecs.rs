// Módulo ECS - Entity Component System v0.16.0 [FASE 8]
//
// FASE 8: create_world refactorizado en 6 fases para eliminar stack overflow.
// Cada fase es una función separada cuyo stack frame se libera antes de la
// siguiente fase, evitando que todos los subsistemas coexistan en el stack.
//
// GameWorld con todos los sistemas integrados:
// [#361] LaneManager - Tráfico con carriles
// [#392] DesignTool - Diseño urbano interactivo
// [M#1..M#10] 10 sistemas de realismo
//
// [FIX STACK OVERFLOW DEFINITIVO]:
// - create_world dividido en 6 fases (funciones helper)
// - Cada fase construye 2-4 subsistemas y los retorna en una tupla
// - El stack frame de cada fase se libera antes de la siguiente
// - Box::new solo recibe los valores finales (ya movidos del stack)
// - Stack size configurado a 16MB como respaldo en .cargo/config.toml

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
pub struct Camera { pub offset_x: f32, pub offset_y: f32, pub zoom: f32 }
#[derive(Copy, Clone, Debug)]
pub struct Renderable {
    pub color: u32,
    pub size_x: f32,
    pub size_y: f32,
    pub layer: i8,
    pub shape_type: u8, // 0=rect, 1=circle
}
impl Renderable {
    #[inline(always)] pub fn rect(color: u32, w: f32, layer: i8) -> Self { Renderable { color, size_x: w, size_y: w, layer, shape_type: 0 } }
    #[inline(always)] pub fn circle(color: u32, r: f32, layer: i8) -> Self { Renderable { color, size_x: r, size_y: r, layer, shape_type: 1 } }
}

#[derive(Copy, Clone, Debug)]
#[derive(Copy, Clone, Debug)]
pub struct TrafficCar {
    pub speed: f32,
    pub max_speed: f32,
    pub acceleration: f32,
    pub lane_position: f32,
    pub lane_id: u32,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum BuildingType {
    House, Shop, Factory, Apartment, Office, Farm,
    Hospital, School, Police,
}

#[derive(Copy, Clone, Debug)]
pub struct ConstructionState {
    pub progress: f32,
    pub building_type: BuildingType,
}

#[derive(Copy, Clone, Debug)]
pub struct ResourceStorage {
    pub money: f32,
    pub food: f32,
    pub goods: f32,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ZoneType {
    Residential, Commercial, Industrial, Agricultural,
    Road, Park,
}

#[derive(Copy, Clone, Debug)]
pub struct Lifetime {
    pub remaining_ticks: u64,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PedestrianState {
    Idle, Walking, Running, Crossing, Panicking,
}
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
    pub fn query_near(&self, x: f32, y: f32, radius: f32) -> SpatialQueryIter<'_> {
        let (cx, cy) = Self::cell_index(x, y);
        let cell_radius = ((radius / SPATIAL_CELL_SIZE).ceil() as usize).min(SPATIAL_GRID_DIM / 2);
        SpatialQueryIter {
            grid: self,
            center_x: cx, center_y: cy,
            radius: cell_radius,
            current_dx: 0, current_dy: 0,
            current_cell_idx: 0,
        }
    }
}

pub struct SpatialQueryIter<'a> {
    grid: &'a SpatialGrid,
    center_x: usize, center_y: usize,
    radius: usize,
    current_dx: usize, current_dy: usize,
    current_cell_idx: usize,
}

impl<'a> Iterator for SpatialQueryIter<'a> {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        loop {
            let dx = self.current_dx as isize - self.radius as isize;
            let dy = self.current_dy as isize - self.radius as isize;
            if self.current_dy > self.radius * 2 { return None; }

            let cx = ((self.center_x as isize + dx + SPATIAL_GRID_DIM as isize) as usize) % SPATIAL_GRID_DIM;
            let cy = ((self.center_y as isize + dy + SPATIAL_GRID_DIM as isize) as usize) % SPATIAL_GRID_DIM;
            let cell = &self.grid.cells[cy][cx];

            if self.current_cell_idx < cell.len() {
                let val = cell[self.current_cell_idx];
                self.current_cell_idx += 1;
                return Some(val);
            }

            self.current_cell_idx = 0;
            self.current_dx += 1;
            if self.current_dx > self.radius * 2 {
                self.current_dx = 0;
                self.current_dy += 1;
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
    pub sim_speed: u8,
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

// =========================================================================
// FASES DE CONSTRUCCIÓN (una función por fase para liberar stack)
// =========================================================================
// 
// Cada fase construye 2-4 subsistemas y los retorna.
// Al salir de la función, su stack frame se libera completamente.
// Esto evita que los 18+ subsistemas coexistan en el stack simultáneamente.

/// Fase 1: Terreno, flow fields, quadtree, bitgrid
fn build_phase1_geo(grid_size: i32) -> (TerrainMap, FlowFieldManager, Quadtree, BitGrid) {
    let terrain = TerrainMap::generate(42);
    let flow_fields = FlowFieldManager::generate_all();
    let quadtree = Quadtree::new(grid_size as f32, grid_size as f32);
    let bitgrid = BitGrid::new();
    (terrain, flow_fields, quadtree, bitgrid)
}

/// Fase 2: Lane manager + design tool
fn build_phase2_traffic() -> (LaneManager, DesignTool) {
    let mut lane_manager = LaneManager::new();
    lane_manager.generate_default_network();
    let design_tool = DesignTool::new();
    (lane_manager, design_tool)
}

/// Fase 3: Utilities (agua + electricidad)
fn build_phase3_utilities() -> (UtilityGrid, UtilityGrid) {
    let mut water_grid = UtilityGrid::new(crate::utilities::UtilitySourceType::WaterTower);
    water_grid.add_source(64.0, 64.0);
    let mut power_grid = UtilityGrid::new(crate::utilities::UtilitySourceType::PowerPlant);
    power_grid.add_source(64.0, 64.0);
    (water_grid, power_grid)
}

/// Fase 4: Road wear, land value, pollution, finance
fn build_phase4_economy() -> (RoadWearGrid, LandValueHeatmap, PollutionHeatmap, MunicipalFinance) {
    let road_wear = RoadWearGrid::new();
    let land_value_map = LandValueHeatmap::new();
    let pollution_map = PollutionHeatmap::new();
    let finance = MunicipalFinance::new();
    (road_wear, land_value_map, pollution_map, finance)
}

/// Fase 5: Parking, waste, customization, politics
fn build_phase5_civic() -> (ParkingManager, WasteManager, CustomizationManager, PoliticalSystem) {
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
    (parking_mgr, waste_mgr, customization, politics)
}

/// Fase 6: Entidades ECS (building, cars, zones, spatial grid, render cache)
fn build_phase6_entities(
    mut world: hecs::World,
    grid_size: i32,
    lane_manager: &LaneManager,
) -> (hecs::World, SpatialGrid, RenderCache) {
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

    // Edificios iniciales
    let buildings: [(f32, f32, BuildingType); 11] = [
        (30.0, 30.0, BuildingType::House),
        (35.0, 30.0, BuildingType::Shop),
        (40.0, 30.0, BuildingType::Factory),
        (30.0, 36.0, BuildingType::Apartment),
        (35.0, 36.0, BuildingType::Office),
        (40.0, 36.0, BuildingType::Farm),
        (60.0, 45.0, BuildingType::House),
        (64.0, 45.0, BuildingType::Shop),
        (50.0, 60.0, BuildingType::Hospital),
        (55.0, 60.0, BuildingType::School),
        (60.0, 60.0, BuildingType::Police),
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

    let mut render_cache = RenderCache::new();
    render_cache.rebuild_from_world(&world);

    (world, spatial_grid, render_cache)
}

/// Crea un mundo nuevo. Construye en 6 fases con funciones helper para
/// minimizar el stack usage. Cada fase libera su stack frame al retornar.
pub fn create_world(_pool: &mut EntityPool) -> Box<GameWorld> {
    let grid_size: i32 = 128;
    let world = hecs::World::new();

    // Fase 1: Geo — su stack frame se libera al terminar
    let (terrain, flow_fields, quadtree, bitgrid) = build_phase1_geo(grid_size);

    // Fase 2: Tráfico
    let (lane_manager, design_tool) = build_phase2_traffic();

    // Fase 3: Utilities
    let (water_grid, power_grid) = build_phase3_utilities();

    // Fase 4: Economía
    let (road_wear, land_value_map, pollution_map, finance) = build_phase4_economy();

    // Fase 5: Cívico
    let (parking_mgr, waste_mgr, customization, politics) = build_phase5_civic();

    // Fase 6: Entidades ECS (toma ownership de world, retorna nuevo)
    let (world, spatial_grid, render_cache) = build_phase6_entities(world, grid_size, &lane_manager);

    // Construir GameWorld en heap. Los valores ya están en registros/stack
    // pero cada uno es pequeño (Vec internos apuntan a heap).
    Box::new(GameWorld {
        world,
        spatial_grid,
        render_cache,
        pool: EntityPool::new(1000),
        sim_tick: 0,
        time_of_day: 7 * 60,
        sim_speed: 1,
        rng: SmallRng::seed_from_u64(42),
        terrain,
        quadtree,
        flow_fields,
        bitgrid,
        lane_manager,
        design_tool,
        water_grid,
        power_grid,
        road_wear,
        land_value_map,
        pollution_map,
        finance,
        parking_mgr,
        waste_mgr,
        customization,
        politics,
        grid_size,
    })
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