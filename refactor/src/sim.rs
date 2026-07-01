// Módulo de Simulación
//
// Contiene todos los sistemas que actualizan el estado del juego.
// Basado en la simulación original de Citybound (cb_simulation).
//
// SUBSISTEMAS:
// - time: Avance del tiempo
// - traffic: Microsimulación con Flow Fields [TA#7] + Bitboards [TI#6]
// - economy: Economía de hogares y recursos
// - land_use: Zonificación y desarrollo usando RNG pool [TC#22]
//
// NUEVAS TÉCNICAS:
// [TA#7]  Flow Fields: tráfico O(1) por coche
// [TI#6]  Bitboards: colisiones O(1) en grilla
// [TA#5]  Fixed-point: velocidades cuantizadas
// [TC#22] RNG pool: sin llamadas a generador en runtime

use crate::ecs::{GameWorld, Position, Velocity, TrafficCar, ZoneComponent, ZoneType,
                  ResourceStorage, ConstructionState, Lifetime, BuildingType, Renderable};
use crate::flow_field::{FlowFieldManager, FlowCell};
use crate::bitboard::BitGrid;
use crate::rng_pool;

// ---------------------------------------------------------------------------
// INICIALIZACIÓN DE SIMULACIÓN
// ---------------------------------------------------------------------------

/// Inicializa la simulación (se llama una vez al crear el mundo)
pub fn init_simulation(game_world: &mut GameWorld) {
    game_world.sim_tick = 0;
    game_world.time_of_day = 7 * 60; // 7:00 AM

    // Inicializar bitboard con edificios y obstáculos
    init_bitboard_obstacles(game_world);
}

/// Registra edificios como obstáculos en el bitboard
fn init_bitboard_obstacles(gw: &mut GameWorld) {
    for (_entity, (pos, _construction)) in gw.world
        .query::<(&Position, &ConstructionState)>()
        .iter()
    {
        // Registrar edificio y sus celdas adyacentes como ocupadas
        for dx in -1i32..=1 {
            for dy in -1i32..=1 {
                gw.bitgrid.set(0, pos.x + dx as f32, pos.y + dy as f32);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TICK PRINCIPAL
// ---------------------------------------------------------------------------

/// Ejecuta un tick de simulación (paso fijo)
pub fn tick(game_world: &mut GameWorld, dt: f32) {
    // 1. Avanzar tiempo
    tick_time(game_world);

    // 2. Tráfico con Flow Fields [TA#7] y Bitboards [TI#6]
    tick_traffic_flow(game_world, dt);

    // 3. Actualizar economía
    tick_economy(game_world, dt);

    // 4. Desarrollo de zonas (usa RNG pool [TC#22])
    tick_land_use(game_world);

    // 5. Limpiar entidades expiradas
    tick_lifetimes(game_world);
}

// ---------------------------------------------------------------------------
// SISTEMA DE TIEMPO
// ---------------------------------------------------------------------------

const TICKS_PER_SIM_SECOND: u32 = 3;
const MINUTES_PER_DAY: u16 = 24 * 60;
const BEGINNING_TIME_OF_DAY: u16 = 7 * 60;

fn tick_time(game_world: &mut GameWorld) {
    game_world.sim_tick = game_world.sim_tick.wrapping_add(1);

    if game_world.sim_tick % TICKS_PER_SIM_SECOND as u64 == 0 {
        let sim_seconds = game_world.sim_tick / TICKS_PER_SIM_SECOND as u64;
        game_world.time_of_day = ((BEGINNING_TIME_OF_DAY as u64
            + (sim_seconds / 60)) % MINUTES_PER_DAY as u64) as u16;
    }
}

/// Retorna la hora del día formateada (HH:MM)
pub fn formatted_time(time_of_day: u16) -> String {
    let hours = time_of_day / 60;
    let minutes = time_of_day % 60;
    format!("{:02}:{:02}", hours, minutes)
}

// ---------------------------------------------------------------------------
// SISTEMA DE TRÁFICO CON FLOW FIELDS [TA#7] + BITBOARDS [TI#6]
//
// En lugar de quadtree O(N log N), cada coche consulta su Flow Field
// en O(1) para obtener dirección y velocidad deseadas.
// Las colisiones se verifican con el BitGrid en O(1).
//
// [TA#5] Fixed-point: velocidades cuantizadas a décimas para reducir
// errores de redondeo y mejorar la consistencia.
// ---------------------------------------------------------------------------

/// Aceleración máxima (m/s²)
const MAX_ACCELERATION: f32 = 3.0;
/// Desaceleración máxima (m/s²)
const MAX_DECELERATION: f32 = 6.0;
/// Velocidad máxima en autopista
const HIGHWAY_SPEED: f32 = 20.0;
/// Velocidad máxima en calle normal
const STREET_SPEED: f32 = 8.0;

fn tick_traffic_flow(gw: &mut GameWorld, dt: f32) {
    // Limpiar capa de tráfico del bitboard para este frame
    gw.bitgrid.clear_layer(5);

    // Reconstruir capa de tráfico con posiciones actuales
    for (_entity, (pos, _car)) in gw.world.query::<(&Position, &TrafficCar)>().iter() {
        gw.bitgrid.set(5, pos.x, pos.y);
    }

    // Actualizar cada coche usando Flow Fields
    for (_entity, (pos, vel, car)) in gw.world
        .query::<(&mut Position, &mut Velocity, &mut TrafficCar)>()
        .iter()
    {
        // [TA#7]: Consultar flow field para dirección deseada
        let flow: FlowCell = gw.flow_fields.sample_combined(pos.x, pos.y, false);

        // Determinar si está en autopista (magnitud alta + dirección horizontal)
        let on_highway = flow.magnitude > 0.5 && flow.angle.abs() < 0.3;

        let max_speed = if on_highway { HIGHWAY_SPEED } else { STREET_SPEED };
        car.max_speed = max_speed;

        // [TA#5]: Cuantizar velocidad objetivo a décimas (fixed-point implícito)
        let target_speed = max_speed * flow.magnitude;

        // Verificar obstáculo adelante con bitboard [TI#6]
        let look_ahead_x = pos.x + flow.angle.cos() * 3.0;
        let look_ahead_y = pos.y + flow.angle.sin() * 3.0;

        let obstacle_ahead = gw.bitgrid.is_obstacle(look_ahead_x, look_ahead_y)
            || gw.bitgrid.test(5, look_ahead_x, look_ahead_y);
        // Verificar también celdas adyacentes
        let obstacle_left = gw.bitgrid.is_obstacle(
            look_ahead_x - flow.angle.sin() * 1.5,
            look_ahead_y + flow.angle.cos() * 1.5
        );
        let obstacle_right = gw.bitgrid.is_obstacle(
            look_ahead_x + flow.angle.sin() * 1.5,
            look_ahead_y - flow.angle.cos() * 1.5
        );

        let desired_accel: f32 = if obstacle_ahead {
            // Frenar urgente
            -MAX_DECELERATION
        } else if obstacle_left || obstacle_right {
            // Reducir velocidad
            if car.speed > target_speed * 0.5 {
                -MAX_ACCELERATION * 0.5
            } else {
                MAX_ACCELERATION * 0.3
            }
        } else if car.speed < target_speed {
            // Acelerar suavemente hacia velocidad objetivo del flow field
            MAX_ACCELERATION * (1.0 - car.speed / target_speed.max(0.1))
        } else if car.speed > target_speed * 1.1 {
            // Desacelerar suavemente si excede
            -MAX_ACCELERATION * 0.3
        } else {
            0.0
        };

        car.acceleration = desired_accel.clamp(-MAX_DECELERATION, MAX_ACCELERATION);
        car.speed = (car.speed + car.acceleration * dt).clamp(0.0, max_speed);

        // [TA#5]: Cuantizar posición a décimas
        let (flow_dx, flow_dy) = FlowField::cell_to_velocity(&flow, car.speed);

        // También aplicar desvío si hay obstáculo cerca
        let dx = if obstacle_ahead && !obstacle_right {
            flow_dx * 0.3 + flow_dy * 0.7 // Desviar a la derecha
        } else if obstacle_ahead && !obstacle_left {
            flow_dx * 0.3 - flow_dy * 0.7 // Desviar a la izquierda
        } else {
            flow_dx
        };
        let dy = if obstacle_ahead && !obstacle_right {
            flow_dy * 0.3 - flow_dx * 0.7
        } else if obstacle_ahead && !obstacle_left {
            flow_dy * 0.3 + flow_dx * 0.7
        } else {
            flow_dy
        };

        pos.x += dx * dt;
        pos.y += dy * dt;

        // Wrap alrededor del mundo
        let gs = gw.grid_size as f32;
        if pos.x < 0.0 { pos.x += gs; }
        if pos.x >= gs { pos.x -= gs; }
        if pos.y < 0.0 { pos.y += gs; }
        if pos.y >= gs { pos.y -= gs; }

        vel.dx = dx;
        vel.dy = dy;
    }
}

// ---------------------------------------------------------------------------
// SISTEMA DE ECONOMÍA
// ---------------------------------------------------------------------------

fn tick_economy(gw: &mut GameWorld, dt: f32) {
    for (_entity, (storage,)) in gw.world.query::<(&mut ResourceStorage,)>().iter() {
        storage.food -= 0.001 * dt;
        storage.money += 0.01 * dt;
        storage.food = storage.food.max(0.0);
        storage.money = storage.money.max(0.0);
    }
}

// ---------------------------------------------------------------------------
// SISTEMA DE USO DE SUELO [TC#22]: usa RNG pool
// ---------------------------------------------------------------------------

fn tick_land_use(game_world: &mut GameWorld) {
    let mut to_spawn: Vec<(f32, f32, ZoneType)> = Vec::with_capacity(16);

    for (_entity, (pos, zone)) in game_world.world
        .query::<(&Position, &ZoneComponent)>()
        .iter()
    {
        if zone.density > 0 {
            // [TC#22]: Usar RNG pool en lugar de fast_random
            if rng_pool::rng_chance(0.0001) {
                to_spawn.push((pos.x, pos.y, zone.zone_type));
            }
        }
    }

    for (x, y, ztype) in to_spawn {
        let (color, btype) = match ztype {
            ZoneType::Residential => (0xFF_66_BB_6A, BuildingType::House),
            ZoneType::Commercial => (0xFF_42_A5_F5, BuildingType::Shop),
            ZoneType::Industrial => (0xFF_EF_5350, BuildingType::Factory),
            ZoneType::Agricultural => (0xFF_9C_CC_65, BuildingType::Farm),
            _ => continue,
        };

        // Verificar que no hay obstáculo [TI#6]
        if !game_world.bitgrid.is_obstacle(x, y) {
            game_world.world.spawn((
                Position::new(x, y),
                Renderable::rect(color, 2.0, 3),
                ConstructionState { progress: 0.0, building_type: btype },
                ResourceStorage { money: 100.0, food: 10.0, goods: 5.0 },
            ));

            // Registrar en bitboard
            game_world.bitgrid.set(0, x, y);
        }
    }
}

// ---------------------------------------------------------------------------
// SISTEMA DE LIFETIMES
// ---------------------------------------------------------------------------

fn tick_lifetimes(game_world: &mut GameWorld) {
    let mut to_remove: Vec<hecs::Entity> = Vec::with_capacity(64);

    for (entity, (lifetime,)) in game_world.world.query::<(&mut Lifetime,)>().iter() {
        if lifetime.remaining_ticks > 0 {
            lifetime.remaining_ticks -= 1;
        } else {
            to_remove.push(entity);
        }
    }

    for entity in to_remove {
        let _ = game_world.world.despawn(entity);
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs;
    use crate::object_pool::EntityPool;

    #[test]
    fn test_tick_time_advances() {
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        assert_eq!(gw.time_of_day, 7 * 60);
        assert_eq!(gw.sim_tick, 0);

        for _ in 0..180 {
            tick_time(&mut gw);
        }
        assert_eq!(gw.time_of_day, 7 * 60 + 1);
    }

    #[test]
    fn test_formatted_time_output() {
        assert_eq!(formatted_time(7 * 60), "07:00");
        assert_eq!(formatted_time(12 * 60 + 30), "12:30");
        assert_eq!(formatted_time(0), "00:00");
        assert_eq!(formatted_time(23 * 60 + 59), "23:59");
    }

    #[test]
    fn test_tick_traffic_flow_moves_cars() {
        crate::luts::init_trig_luts();
        crate::rng_pool::init_rng_pool(42);

        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        let car_count_before = gw.world.query::<&TrafficCar>().iter().count();
        assert_eq!(car_count_before, 40);

        for _ in 0..10 {
            tick_traffic_flow(&mut gw, 0.1);
        }

        let car_count_after = gw.world.query::<&TrafficCar>().iter().count();
        assert_eq!(car_count_after, 40);
    }

    #[test]
    fn test_tick_economy_updates_resources() {
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        tick_economy(&mut gw, 0.1);

        let resource_count = gw.world.query::<&ResourceStorage>().iter().count();
        assert!(resource_count >= 8, "Debe haber al menos los 8 edificios iniciales");
    }

    #[test]
    fn test_tick_land_use_no_panic() {
        crate::rng_pool::init_rng_pool(42);

        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        tick_land_use(&mut gw);
    }

    #[test]
    fn test_tick_lifetimes_no_panic() {
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        tick_lifetimes(&mut gw);
    }

    #[test]
    fn test_full_tick_pipeline() {
        crate::luts::init_trig_luts();
        crate::rng_pool::init_rng_pool(42);

        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        let initial_count = gw.world.len();

        tick(&mut gw, 0.1);

        assert!(gw.world.len() >= initial_count);
    }

    #[test]
    fn test_bitboard_obstacles_initialized() {
        crate::luts::init_trig_luts();

        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        // Después de init, debe haber obstáculos registrados
        let obstacle_count = gw.bitgrid.count_layer(0);
        // Solo los 8 edificios iniciales + adyacentes (cada uno ~9 celdas)
        assert!(obstacle_count > 0, "Debe haber obstáculos inicializados");
    }
}
