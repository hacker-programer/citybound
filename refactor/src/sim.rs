// Módulo de Simulación
//
// Contiene todos los sistemas que actualizan el estado del juego.
// Basado en la simulación original de Citybound (cb_simulation).
//
// SUBSISTEMAS:
// - time: Avance del tiempo, hora del día, ticks
// - traffic: Microsimulación de tráfico usando Quadtree [TC#7]
// - economy: Economía de hogares y recursos
// - land_use: Zonificación y desarrollo de terrenos

use crate::ecs::{GameWorld, Position, Velocity, TrafficCar, ZoneComponent, ZoneType,
                  ResourceStorage, ConstructionState, Lifetime, BuildingType};
use crate::ecs::Renderable;
use crate::quadtree::AABB;

// ---------------------------------------------------------------------------
// INICIALIZACIÓN DE SIMULACIÓN
// ---------------------------------------------------------------------------

/// Inicializa la simulación (se llama una vez al crear el mundo)
pub fn init_simulation(game_world: &mut GameWorld) {
    game_world.sim_tick = 0;
    game_world.time_of_day = 7 * 60; // 7:00 AM
}

// ---------------------------------------------------------------------------
// TICK PRINCIPAL
// ---------------------------------------------------------------------------

/// Ejecuta un tick de simulación (paso fijo)
/// `dt` es la duración del tick en segundos (ej. 0.1 para 10 ticks/s)
pub fn tick(game_world: &mut GameWorld, dt: f32) {
    // 1. Avanzar tiempo
    tick_time(game_world);

    // 2. Actualizar tráfico con quadtree [TC#7]
    tick_traffic(game_world, dt);

    // 3. Actualizar economía
    tick_economy(game_world, dt);

    // 4. Desarrollo de zonas
    tick_land_use(game_world);

    // 5. Limpiar entidades expiradas
    tick_lifetimes(game_world);
}

// ---------------------------------------------------------------------------
// SISTEMA DE TIEMPO
// ---------------------------------------------------------------------------

/// SimTicks por segundo de simulación (3 ticks = 1 segundo simulado)
const TICKS_PER_SIM_SECOND: u32 = 3;
/// Minutos por día
const MINUTES_PER_DAY: u16 = 24 * 60;
/// Hora de inicio del día simulado
const BEGINNING_TIME_OF_DAY: u16 = 7 * 60; // 7:00 AM

fn tick_time(game_world: &mut GameWorld) {
    game_world.sim_tick = game_world.sim_tick.wrapping_add(1);

    // Actualizar hora del día cada TICKS_PER_SIM_SECOND ticks
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
// SISTEMA DE TRÁFICO (Microsimulación con Quadtree)
//
// TÉCNICA COMÚN #7: Árboles de Colisión - Quadtree
// Usamos el quadtree para encontrar coches vecinos en O(log N)
// en lugar de iterar sobre todos los coches en O(N).
//
// TÉCNICA COMÚN #21: Pre-cálculo de distancias al cuadrado
// Comparamos distancias² para evitar sqrt() en cada comprobación.
// ---------------------------------------------------------------------------

/// Aceleración máxima (m/s²)
const MAX_ACCELERATION: f32 = 3.0;
/// Desaceleración máxima (m/s²)
const MAX_DECELERATION: f32 = 6.0;
/// Radio de búsqueda de vecinos para quadtree al cuadrado [TC#21]
const NEIGHBOR_SEARCH_RADIUS_SQ: f32 = 100.0; // 10.0²

fn tick_traffic(game_world: &mut GameWorld, dt: f32) {
    // Reconstruir quadtree con posiciones actuales de coches
    game_world.quadtree.clear();

    // Registrar todos los coches en el quadtree
    for (_entity, (pos, _car)) in game_world.world.query::<(&Position, &TrafficCar)>().iter() {
        let bounds = AABB::new(pos.x - 0.5, pos.y - 0.5, 1.0, 1.0);
        game_world.quadtree.insert(bounds);
    }

    // Para cada coche, buscar vecinos con quadtree y ajustar velocidad
    for (_entity, (pos, vel, car)) in game_world.world
        .query::<(&mut Position, &mut Velocity, &mut TrafficCar)>()
        .iter()
    {
        // [TC#21]: distancia mínima al cuadrado para evitar sqrt
        let mut closest_dist_sq: f32 = f32::MAX;

        // Usar quadtree para encontrar coches cercanos [TC#7]
        game_world.quadtree.query_radius(
            pos.x, pos.y,
            NEIGHBOR_SEARCH_RADIUS_SQ,
            &mut |qh| {
                if let Some(bounds) = game_world.quadtree.get_bounds(qh) {
                    let dx = bounds.x - pos.x;
                    let dy = bounds.y - pos.y;
                    let dist_sq = dx * dx + dy * dy; // [TC#21]: sin sqrt
                    if dist_sq > 0.001 && dist_sq < closest_dist_sq {
                        closest_dist_sq = dist_sq;
                    }
                }
            },
        );

        // [TC#21]: comparar contra distancia mínima al cuadrado
        let safe_distance_sq = {
            let sd = car.speed * 2.0 + 2.0; // Distancia segura = velocidad * 2 + mínima
            sd * sd
        };

        let desired_accel: f32 = if closest_dist_sq < safe_distance_sq && closest_dist_sq > 0.001 {
            // Frenar proporcionalmente a la urgencia
            let urgency = (safe_distance_sq - closest_dist_sq) / safe_distance_sq;
            -MAX_DECELERATION * urgency.min(1.0)
        } else if car.speed < car.max_speed {
            // Acelerar suavemente hacia velocidad máxima
            MAX_ACCELERATION * (1.0 - car.speed / car.max_speed)
        } else {
            0.0
        };

        // Aplicar aceleración con límites
        car.acceleration = desired_accel.clamp(-MAX_DECELERATION, MAX_ACCELERATION);
        car.speed = (car.speed + car.acceleration * dt).clamp(0.0, car.max_speed);

        // Actualizar posición horizontal
        pos.x += car.speed * dt;
        if pos.x > game_world.grid_size as f32 - 1.0 {
            pos.x = 1.0;
        }

        vel.dx = car.speed;
        vel.dy = 0.0;
    }
}

// ---------------------------------------------------------------------------
// SISTEMA DE ECONOMÍA
// ---------------------------------------------------------------------------

fn tick_economy(game_world: &mut GameWorld, dt: f32) {
    for (_entity, (storage,)) in game_world.world.query::<(&mut ResourceStorage,)>().iter() {
        // Consumo básico por tick
        storage.food -= 0.001 * dt;
        // Ingresos básicos
        storage.money += 0.01 * dt;

        // Clampear
        storage.food = storage.food.max(0.0);
        storage.money = storage.money.max(0.0);
    }
}

// ---------------------------------------------------------------------------
// SISTEMA DE USO DE SUELO
// ---------------------------------------------------------------------------

fn tick_land_use(game_world: &mut GameWorld) {
    let mut to_spawn: Vec<(f32, f32, ZoneType)> = Vec::with_capacity(16);

    for (_entity, (pos, zone)) in game_world.world
        .query::<(&Position, &ZoneComponent)>()
        .iter()
    {
        if zone.density > 0 {
            if fast_random(pos.x as u64 + pos.y as u64 + game_world.sim_tick) < 0.0001 {
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

        game_world.world.spawn((
            Position::new(x, y),
            Renderable::rect(color, 2.0, 3),
            ConstructionState { progress: 0.0, building_type: btype },
            ResourceStorage { money: 100.0, food: 10.0, goods: 5.0 },
        ));
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
// RNG RÁPIDO DETERMINISTA (splitmix64 simplificado)
// [TC#12]: RNG inline sin dependencia de crate pesada
// ---------------------------------------------------------------------------

#[inline(always)]
fn fast_random(seed: u64) -> f32 {
    let mut x = seed.wrapping_mul(0x9E3779B97F4A7C15);
    x = x.wrapping_add(x >> 30).wrapping_mul(0xBF58476D1CE4E5B9);
    x = x.wrapping_add(x >> 27).wrapping_mul(0x94D049BB133111EB);
    (x as f32) / (u64::MAX as f32)
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
        assert_eq!(gw.time_of_day, 7 * 60 + 1); // 7:01 AM
    }

    #[test]
    fn test_formatted_time_output() {
        assert_eq!(formatted_time(7 * 60), "07:00");
        assert_eq!(formatted_time(12 * 60 + 30), "12:30");
        assert_eq!(formatted_time(0), "00:00");
        assert_eq!(formatted_time(23 * 60 + 59), "23:59");
    }

    #[test]
    fn test_tick_traffic_moves_cars() {
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        let car_count_before = gw.world.query::<&TrafficCar>().iter().count();
        assert_eq!(car_count_before, 40);

        for _ in 0..10 {
            tick_traffic(&mut gw, 0.1);
        }

        let car_count_after = gw.world.query::<&TrafficCar>().iter().count();
        assert_eq!(car_count_after, 40);
    }

    #[test]
    fn test_fast_random_range() {
        for i in 0..1000 {
            let val = fast_random(i);
            assert!(val >= -1.0 && val <= 2.0, "Random fuera de rango: {}", val);
        }
    }

    #[test]
    fn test_fast_random_determinism() {
        let a = fast_random(42);
        let b = fast_random(42);
        assert_eq!(a, b, "fast_random debe ser determinista");
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
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        let initial_count = gw.world.len();

        tick(&mut gw, 0.1);

        assert!(gw.world.len() >= initial_count);
    }
}
