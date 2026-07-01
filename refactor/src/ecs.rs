// Módulo ECS (Entity Component System)
//
// ARQUITECTURA:
// Usamos hecs como motor ECS puro. Todos los componentes se almacenan
// en Struct-of-Arrays (SoA) para maximizar la localidad de caché.
// En hecs, cualquier tipo Send + Sync + 'static es automáticamente
// un Component, sin necesidad de derive macro.
//
// TÉCNICA AVANZADA #4: ECS puro - Struct of Arrays
// TÉCNICA AVANZADA #9: Structs alineados a 64 bytes (línea caché L1)
// TÉCNICA COMÚN #24 (juegos): Máquinas de estado aplanadas

// Allow dead_code en componentes que serán usados por sistemas futuros
#![allow(dead_code)]

use crate::object_pool::EntityPool;
use crate::input::InputState;
use rand::rngs::SmallRng;
use rand::SeedableRng;

// ---------------------------------------------------------------------------
// COMPONENTES
// [TA#9]: Cada struct alineado a 64 bytes = una línea de caché L1 completa
// Esto maximiza los hits de caché en CPUs legacy como Pentium
// ---------------------------------------------------------------------------

/// Posición en el mundo (coordenadas de grilla)
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Position {
    pub x: f32,
    pub y: f32,
}

impl Position {
    #[inline(always)]
    pub fn new(x: f32, y: f32) -> Self {
        Position { x, y }
    }
}

/// Velocidad para movimiento
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Velocity {
    pub dx: f32,
    pub dy: f32,
}

impl Velocity {
    #[inline(always)]
    pub fn new(dx: f32, dy: f32) -> Self {
        Velocity { dx, dy }
    }
}

/// Componente de renderizado
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Renderable {
    /// Tipo de forma: 0=círculo, 1=rectángulo, 2=triángulo
    pub shape_type: u8,
    /// Color en formato ARGB
    pub color: u32,
    /// Tamaño en píxeles (radio para círculo, ancho para rect)
    pub size: f32,
    /// Capa de renderizado (z-order): 0=terreno, 1=zonas, 2-3=edificios, 4+=tráfico
    pub layer: u8,
}

impl Renderable {
    #[inline(always)]
    pub fn circle(color: u32, radius: f32, layer: u8) -> Self {
        Renderable { shape_type: 0, color, size: radius, layer }
    }

    #[inline(always)]
    pub fn rect(color: u32, width: f32, layer: u8) -> Self {
        Renderable { shape_type: 1, color, size: width, layer }
    }
}

/// Tipo de zona (planificación urbana)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(align(64))]
pub enum ZoneType {
    Residential,
    Commercial,
    Industrial,
    Agricultural,
    Road,
    Park,
}

/// Componente de zona para celdas del mapa
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct ZoneComponent {
    pub zone_type: ZoneType,
    /// Densidad: 0=sin desarrollar, 1=baja, 2=media, 3=alta
    pub density: u8,
}

/// Estado de un coche en el sistema de tráfico
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct TrafficCar {
    /// Velocidad actual (m/s)
    pub speed: f32,
    /// Velocidad máxima (m/s)
    pub max_speed: f32,
    /// Aceleración actual (m/s²)
    pub acceleration: f32,
    /// Posición en el carril (0.0 = inicio, 1.0 = final)
    pub lane_position: f32,
    /// ID del carril actual
    pub lane_id: u32,
}

/// Almacenamiento de recursos para economía
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct ResourceStorage {
    /// Dinero
    pub money: f32,
    /// Comida
    pub food: f32,
    /// Bienes
    pub goods: f32,
}

/// Estado de construcción
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct ConstructionState {
    /// Progreso 0.0 a 1.0
    pub progress: f32,
    /// Tipo de construcción
    pub building_type: BuildingType,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BuildingType {
    House,
    Apartment,
    Shop,
    Office,
    Factory,
    Farm,
}

/// Cámara (viewport)
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Camera {
    pub offset_x: f32,
    pub offset_y: f32,
    pub zoom: f32,
}

/// Tiempo de vida para entidades temporales
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Lifetime {
    pub remaining_ticks: u32,
}

// ---------------------------------------------------------------------------
// ECS WORLD
// ---------------------------------------------------------------------------

/// Estado global del mundo ECS
pub struct GameWorld {
    pub world: hecs::World,
    pub pool: EntityPool,
    /// Tiempo de simulación actual (en ticks)
    pub sim_tick: u64,
    /// Hora del día simulada (minutos desde medianoche)
    pub time_of_day: u16,
    /// Generador de números aleatorios determinista [TC#12]
    pub rng: SmallRng,
}

/// Crea el mundo ECS inicial con todas las entidades del juego
pub fn create_world(_pool: &mut EntityPool) -> GameWorld {
    let mut world = hecs::World::new();

    // Cámara (1 entidad)
    world.spawn((
        Camera { offset_x: 0.0, offset_y: 0.0, zoom: 1.0 },
        Position::new(0.0, 0.0),
    ));

    // Mapa base: grilla de 128x128 celdas de terreno
    let grid_size: i32 = 128;
    for gx in 0..grid_size {
        for gy in 0..grid_size {
            world.spawn((
                Position::new(gx as f32, gy as f32),
                Renderable::rect(0xFF_2D_2D_44, 1.0, 0),
                ZoneComponent { zone_type: ZoneType::Residential, density: 0 },
            ));
        }
    }

    // Pool de coches preasignados (100 coches)
    for i in 0..100 {
        world.spawn((
            Position::new(i as f32 * 10.0, 50.0),
            Velocity::new(0.0, 0.0),
            TrafficCar {
                speed: (i as f32 % 5.0) * 3.0,
                max_speed: 13.8,
                acceleration: 0.0,
                lane_position: i as f32 / 100.0,
                lane_id: 0,
            },
            Renderable::circle(0xFF_FF_AA_00, 1.5, 5),
        ));
    }

    // Edificios de ejemplo
    let buildings: [(f32, f32, BuildingType, u32); 6] = [
        (30.0, 30.0, BuildingType::House, 0xFF_66_BB_6A),
        (35.0, 30.0, BuildingType::Shop, 0xFF_42_A5_F5),
        (40.0, 30.0, BuildingType::Factory, 0xFF_EF_5350),
        (30.0, 35.0, BuildingType::Apartment, 0xFF_AB_47_BC),
        (35.0, 35.0, BuildingType::Office, 0xFF_26_C6_DA),
        (40.0, 35.0, BuildingType::Farm, 0xFF_9C_CC_65),
    ];

    for &(bx, by, btype, color) in &buildings {
        world.spawn((
            Position::new(bx, by),
            Renderable::rect(color, 3.0, 3),
            ConstructionState { progress: 1.0, building_type: btype },
            ResourceStorage { money: 1000.0, food: 100.0, goods: 50.0 },
        ));
    }

    // Zonas planificadas
    let zones: [(f32, f32, f32, f32, ZoneType, u32); 4] = [
        (10.0, 10.0, 40.0, 20.0, ZoneType::Residential, 0x44_66_BB_6A),
        (55.0, 10.0, 30.0, 20.0, ZoneType::Commercial, 0x44_42_A5_F5),
        (10.0, 55.0, 30.0, 20.0, ZoneType::Industrial, 0x44_EF_5350),
        (55.0, 55.0, 30.0, 20.0, ZoneType::Agricultural, 0x44_9C_CC_65),
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
        sim_tick: 0,
        time_of_day: 7 * 60,
        rng: SmallRng::seed_from_u64(42),
    }
}

/// Retorna el número de entidades en el mundo
#[inline(always)]
pub fn entity_count(game_world: &GameWorld) -> usize {
    game_world.world.len() as usize
}

/// Procesa input del usuario para mover la cámara
pub fn process_input(game_world: &mut GameWorld, input: &InputState) {
    let move_speed: f32 = 0.5;

    for (_entity, (camera,)) in game_world.world.query::<(&mut Camera,)>().iter() {
        if input.is_key_down(crate::input::GameKey::W) || input.is_key_down(crate::input::GameKey::Up) {
            camera.offset_y -= move_speed;
        }
        if input.is_key_down(crate::input::GameKey::S) || input.is_key_down(crate::input::GameKey::Down) {
            camera.offset_y += move_speed;
        }
        if input.is_key_down(crate::input::GameKey::A) || input.is_key_down(crate::input::GameKey::Left) {
            camera.offset_x -= move_speed;
        }
        if input.is_key_down(crate::input::GameKey::D) || input.is_key_down(crate::input::GameKey::Right) {
            camera.offset_x += move_speed;
        }
        if input.is_key_down(crate::input::GameKey::PageUp) {
            camera.zoom = (camera.zoom * 1.05_f32).min(4.0);
        }
        if input.is_key_down(crate::input::GameKey::PageDown) {
            camera.zoom = (camera.zoom / 1.05_f32).max(0.25);
        }
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_world() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        assert!(gw.world.len() > 0, "El mundo debe tener entidades");
        assert_eq!(gw.time_of_day, 7 * 60);
        assert_eq!(gw.sim_tick, 0);
    }

    #[test]
    fn test_camera_query() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        let count = gw.world.query::<&Camera>().iter().count();
        assert_eq!(count, 1, "Debe haber exactamente una cámara");
    }

    #[test]
    fn test_traffic_cars_exist() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        let car_count = gw.world.query::<&TrafficCar>().iter().count();
        assert_eq!(car_count, 100, "Debe haber 100 coches preasignados");
    }

    #[test]
    fn test_position_component() {
        let pos = Position::new(10.0, 20.0);
        assert_eq!(pos.x, 10.0);
        assert_eq!(pos.y, 20.0);
    }

    #[test]
    fn test_zone_types_distinct() {
        assert_ne!(ZoneType::Residential, ZoneType::Commercial);
        assert_ne!(ZoneType::Industrial, ZoneType::Agricultural);
    }

    #[test]
    fn test_building_types_distinct() {
        assert_ne!(BuildingType::House, BuildingType::Factory);
        assert_ne!(BuildingType::Shop, BuildingType::Farm);
    }

    #[test]
    fn test_renderable_circle() {
        let c = Renderable::circle(0xFF_FF_00_00, 5.0, 3);
        assert_eq!(c.shape_type, 0);
        assert_eq!(c.size, 5.0);
        assert_eq!(c.layer, 3);
    }

    #[test]
    fn test_renderable_rect() {
        let r = Renderable::rect(0xFF_00_00_FF, 4.0, 2);
        assert_eq!(r.shape_type, 1);
        assert_eq!(r.size, 4.0);
        assert_eq!(r.layer, 2);
    }

    #[test]
    fn test_component_alignment() {
        assert_eq!(std::mem::align_of::<Position>(), 64);
        assert_eq!(std::mem::align_of::<Velocity>(), 64);
        assert_eq!(std::mem::align_of::<Renderable>(), 64);
        assert_eq!(std::mem::align_of::<ZoneComponent>(), 64);
        assert_eq!(std::mem::align_of::<TrafficCar>(), 64);
        assert_eq!(std::mem::align_of::<ResourceStorage>(), 64);
        assert_eq!(std::mem::align_of::<ConstructionState>(), 64);
        assert_eq!(std::mem::align_of::<Camera>(), 64);
    }

    #[test]
    fn test_process_input_no_panic() {
        let mut pool = EntityPool::new(1000);
        let mut gw = create_world(&mut pool);
        let input = InputState::default();
        process_input(&mut gw, &input);
    }

    #[test]
    fn test_entity_count() {
        let mut pool = EntityPool::new(1000);
        let gw = create_world(&mut pool);
        let count = entity_count(&gw);
        assert!(count > 15000, "Esperado > 15000 entidades, hay {}", count);
    }
}
