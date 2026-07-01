// Módulo ECS - Entity Component System v0.7.0
//
// GameWorld con todos los sistemas integrados:
// [#361] LaneManager - Tráfico con carriles
// [#392] DesignTool - Diseño urbano interactivo
// [M#1] SupplyChain - Cadenas de suministro física
// [M#2] LandValue - Valor del suelo y contaminación
// [M#3] Utilities - Agua y electricidad con propagación
// [M#4] RoadWear - Desgaste de asfalto
// [M#5] LaborMarket - Mercado laboral
// [M#6] TaxSystem - Impuestos milimétricos + bonos
// [M#7] Parking - Estacionamiento físico + HOA
// [M#8] WasteMgmt - Clasificación de basura
// [M#9] Customization - Personalización visual
// [M#10] Politics - NIMBY, sindicatos, elecciones

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
use rand::rngs::SmallRng;
use rand::SeedableRng;

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

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BuildingType { House, Apartment, Shop, Office, Factory, Farm }

#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Camera { pub offset_x: f32, pub offset_y: f32, pub zoom: f32 }

#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Lifetime { pub remaining_ticks: u32 }

#[derive(Copy, Clone, Debug)]
pub struct QuadIndex(pub u32);

pub struct GameWorld {
    pub world: hecs::World,
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
    /// Finanzas municipales e impuestos [M#6]
    pub finance: MunicipalFinance,
    /// Gestión de estacionamiento [M#7]
    pub parking_mgr: ParkingManager,
    /// Gestión de residuos [M#8]
    pub waste_mgr: WasteManager,
    /// Personalización visual [M#9]
    pub customization: CustomizationManager,
    /// Sistema político [M#10]
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

    // [M#6]: Finanzas municipales
    let finance = MunicipalFinance::new();

    // [M#7]: Parking manager
    let mut parking_mgr = ParkingManager::new();
    // Generar estacionamiento en calle a lo largo de avenidas principales
    for i in 0..6 {
        let ave_x = 20.0 + i as f32 * 20.0;
        for y in (10..120).step_by(4) {
            parking_mgr.add_street_parking(ave_x, y as f32, 4, false, 0.0);
        }
    }

    // [M#8]: Gestor de residuos
    let waste_mgr = WasteManager::new();

    // [M#9]: Personalización
    let customization = CustomizationManager::new();

    // [M#10]: Sistema político
    let politics = PoliticalSystem::new();

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

    // Edificios de ejemplo
    let buildings: [(f32, f32, BuildingType, u32); 8] = [
        (30.0, 30.0, BuildingType::House, 0xFF_C4_7B_4A),
        (35.0, 30.0, BuildingType::Shop, 0xFF_26_C6_DA),
        (40.0, 30.0, BuildingType::Factory, 0xFF_8D_6E_63),
        (30.0, 36.0, BuildingType::Apartment, 0xFF_B0_BEC5),
        (35.0, 36.0, BuildingType::Office, 0xFF_78_90_9C),
        (40.0, 36.0, BuildingType::Farm, 0xFF_8B_C3_4A),
        (60.0, 45.0, BuildingType::House, 0xFF_C4_7B_4A),
        (64.0, 45.0, BuildingType::Shop, 0xFF_26_C6_DA),
    ];

    for &(bx, by, btype, color) in &buildings {
        world.spawn((
            Position::new(bx, by),
            Renderable::rect(color, 3.0, 3),
            ConstructionState { progress: 1.0, building_type: btype },
            ResourceStorage { money: 1000.0, food: 100.0, goods: 50.0 },
        ));
    }

    // Zonas
    let zones: [(f32, f32, f32, f32, ZoneType, u32); 4] = [
        (15.0, 15.0, 30.0, 18.0, ZoneType::Residential, 0x44_66_BB_6A),
        (55.0, 15.0, 25.0, 18.0, ZoneType::Commercial, 0x44_42_A5_F5),
        (15.0, 50.0, 25.0, 18.0, ZoneType::Industrial, 0x44_EF_5350),
        (55.0, 50.0, 25.0, 18.0, ZoneType::Agricultural, 0x44_9C_CC_65),
    ];

    for &(zx, zy, zw, zh, ztype, color) in &zones {
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

    GameWorld {
        world,
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
    fn test_create_world() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        assert!(gw.world.len() > 0);
        assert_eq!(gw.grid_size, 128);
    }

    #[test]
    fn test_finance_exists() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        assert!(gw.finance.treasury > 0.0);
        assert_eq!(gw.finance.active_bonds.len(), 0);
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
    fn test_customization_exists() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        assert!(gw.customization.appearances.is_empty());
    }

    #[test]
    fn test_politics_exists() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        assert_eq!(gw.politics.districts.len(), 9);
        assert_eq!(gw.politics.unions.len(), 6);
    }

    #[test]
    fn test_entity_count() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        assert!(gw.world.len() > 100);
    }
}
