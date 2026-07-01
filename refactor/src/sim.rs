// Módulo de Simulación v0.7.0
//
// Todos los sistemas que actualizan el estado del juego.
//
// SUBSISTEMAS:
// - time: Avance del tiempo
// - traffic: Flow Fields [TA#7] + Bitboards [TI#6] + Lanes [#361]
// - supply_chain: Camiones de carga física [M#1]
// - land_value: Valor del suelo y gentrificación [M#2]
// - utilities: Propagación de agua y electricidad [M#3]
// - road_wear: Desgaste de infraestructura [M#4]
// - labor_market: Mercado laboral [M#5]
// - economy: Recursos base
// - land_use: Desarrollo de zonas

use crate::ecs::{GameWorld, Position, Velocity, TrafficCar, ZoneComponent, ZoneType,
                  ResourceStorage, ConstructionState, Lifetime, BuildingType, Renderable};
use crate::flow_field::FlowCell;
use crate::traffic_lanes::IdmParams;
use crate::rng_pool;

pub fn init_simulation(game_world: &mut GameWorld) {
    game_world.sim_tick = 0;
    game_world.time_of_day = 7 * 60;

    init_bitboard_obstacles(game_world);
    init_car_idm_params(game_world);

    // [M#5]: Inicializar mercado laboral
    crate::labor_market::init_labor_market(game_world);

    // [M#3]: Propagar utilidades iniciales
    game_world.water_grid.propagate();
    game_world.power_grid.propagate();
}

fn init_bitboard_obstacles(gw: &mut GameWorld) {
    let positions: Vec<(f32, f32)> = gw.world
        .query::<(&Position, &ConstructionState)>()
        .iter()
        .flat_map(|(_, (pos, _))| {
            (-1i32..=1).flat_map(move |dx| {
                (-1i32..=1).map(move |dy| (pos.x + dx as f32, pos.y + dy as f32))
            }).collect::<Vec<_>>()
        })
        .collect();
    for (x, y) in positions { gw.bitgrid.set(0, x, y); }
}

fn init_car_idm_params(gw: &mut GameWorld) {
    let assignments: Vec<(u32, IdmParams)> = gw.world.query::<&TrafficCar>()
        .iter()
        .map(|(entity, car)| {
            // hecs 0.10: to_bits() devuelve NonZero<u64>, usamos .get()
            let raw_id = entity.to_bits().get();
            (raw_id as u32, IdmParams { desired_speed: car.max_speed, ..IdmParams::default() })
        })
        .collect();
    for (id, params) in assignments { gw.lane_manager.set_vehicle_params(id, params); }
}

// ---------------------------------------------------------------------------
// TICK PRINCIPAL - Paso fijo, unificado
// ---------------------------------------------------------------------------

pub fn tick(game_world: &mut GameWorld, dt: f32) {
    // 1. Tiempo
    tick_time(game_world);

    // 2. Semáforos
    tick_intersections(game_world, dt);

    // 3. Tráfico base (Flow Fields + Bitboards + Lanes)
    tick_traffic_flow(game_world, dt);

    // 4. [M#1] Cadenas de suministro
    crate::supply_chain::tick_supply_chain(game_world, dt);

    // 5. [M#4] Desgaste de carreteras (modifica Flow Fields)
    crate::road_wear::tick_road_wear(game_world);

    // 6. [M#2] Valor del suelo y gentrificación
    crate::land_value::tick_land_value(game_world);

    // 7. [M#3] Utilidades (agua/electricidad)
    crate::utilities::tick_utilities(game_world);

    // 8. [M#5] Mercado laboral
    crate::labor_market::tick_labor_market(game_world);

    // 9. Congestión de carriles
    tick_lane_congestion(game_world);

    // 10. Economía base
    tick_economy(game_world, dt);

    // 11. Desarrollo de zonas
    tick_land_use(game_world);

    // 12. Limpiar entidades expiradas
    tick_lifetimes(game_world);
}

// ---------------------------------------------------------------------------
// TIEMPO
// ---------------------------------------------------------------------------

const TICKS_PER_SIM_SECOND: u32 = 3;
const MINUTES_PER_DAY: u16 = 24 * 60;

fn tick_time(game_world: &mut GameWorld) {
    game_world.sim_tick = game_world.sim_tick.wrapping_add(1);
    if game_world.sim_tick % TICKS_PER_SIM_SECOND as u64 == 0 {
        let secs = game_world.sim_tick / TICKS_PER_SIM_SECOND as u64;
        game_world.time_of_day = (((7 * 60) as u64 + (secs / 60)) % MINUTES_PER_DAY as u64) as u16;
    }
}

pub fn formatted_time(time_of_day: u16) -> String {
    format!("{:02}:{:02}", time_of_day / 60, time_of_day % 60)
}

// ---------------------------------------------------------------------------
// INTERSECCIONES
// ---------------------------------------------------------------------------

fn tick_intersections(gw: &mut GameWorld, dt: f32) {
    for intersection in gw.lane_manager.intersections.iter_mut() {
        intersection.tick(dt);
    }
}

// ---------------------------------------------------------------------------
// CONGESTIÓN DE CARRILES
// ---------------------------------------------------------------------------

fn tick_lane_congestion(gw: &mut GameWorld) {
    for lane in gw.lane_manager.lanes.iter_mut() { lane.vehicle_count = 0; }
    for (_entity, (_pos, car)) in gw.world.query::<(&Position, &TrafficCar)>().iter() {
        let lid = car.lane_id as usize;
        if lid < gw.lane_manager.lanes.len() { gw.lane_manager.lanes[lid].vehicle_count += 1; }
    }
    for lane in gw.lane_manager.lanes.iter_mut() {
        let cap = (lane.length / 2.0).max(1.0);
        lane.congestion = (lane.vehicle_count as f32 / cap).min(1.0);
    }
}

// ---------------------------------------------------------------------------
// TRÁFICO
// ---------------------------------------------------------------------------

const MAX_ACCELERATION: f32 = 3.0;
const MAX_DECELERATION: f32 = 6.0;
const HIGHWAY_SPEED: f32 = 20.0;
const STREET_SPEED: f32 = 8.0;

fn tick_traffic_flow(gw: &mut GameWorld, dt: f32) {
    gw.bitgrid.clear_layer(5);

    let positions: Vec<(f32, f32)> = gw.world.query::<&Position>()
        .iter().map(|(_, p)| (p.x, p.y)).collect();
    for (x, y) in positions { gw.bitgrid.set(5, x, y); }

    let num_lanes = gw.lane_manager.lanes.len();
    let has_intersections = !gw.lane_manager.intersections.is_empty();

    for (_entity, (pos, vel, car)) in gw.world
        .query::<(&mut Position, &mut Velocity, &mut TrafficCar)>()
        .iter()
    {
        let lane_speed = if (car.lane_id as usize) < num_lanes {
            let lane = &gw.lane_manager.lanes[car.lane_id as usize];
            let can_proceed = if let Some(iid) = lane.to_intersection {
                if has_intersections && (iid as usize) < gw.lane_manager.intersections.len() {
                    gw.lane_manager.intersections[iid as usize].can_proceed(car.lane_id)
                } else { true }
            } else { true };
            if can_proceed { lane.speed_limit } else { lane.speed_limit * 0.3 }
        } else { STREET_SPEED };

        let flow: FlowCell = gw.flow_fields.sample_combined(pos.x, pos.y, false);
        let on_highway = flow.magnitude > 0.5 && flow.angle.abs() < 0.3;
        let max_speed = if on_highway { HIGHWAY_SPEED } else { lane_speed };
        car.max_speed = max_speed;
        let target_speed = max_speed * flow.magnitude.max(0.3);

        let lx = pos.x + flow.angle.cos() * 3.0;
        let ly = pos.y + flow.angle.sin() * 3.0;
        let obstacle = gw.bitgrid.is_obstacle(lx, ly) || gw.bitgrid.test(5, lx, ly);

        let desired: f32 = if obstacle { -MAX_DECELERATION }
        else if car.speed < target_speed { MAX_ACCELERATION * (1.0 - car.speed / target_speed.max(0.1)) }
        else if car.speed > target_speed * 1.1 { -MAX_ACCELERATION * 0.3 }
        else { 0.0 };

        car.acceleration = desired.clamp(-MAX_DECELERATION, MAX_ACCELERATION);
        car.speed = (car.speed + car.acceleration * dt).clamp(0.0, max_speed);

        let (dx, dy) = crate::flow_field::FlowField::cell_to_velocity(&flow, car.speed);
        pos.x += dx * dt; pos.y += dy * dt;

        let gs = gw.grid_size as f32;
        if pos.x < 0.0 { pos.x += gs; } if pos.x >= gs { pos.x -= gs; }
        if pos.y < 0.0 { pos.y += gs; } if pos.y >= gs { pos.y -= gs; }

        vel.dx = dx; vel.dy = dy;

        if (car.lane_id as usize) < num_lanes {
            let lane = &gw.lane_manager.lanes[car.lane_id as usize];
            let (t, _, _) = lane.project(pos.x, pos.y);
            car.lane_position = t;
        }
    }
}

// ---------------------------------------------------------------------------
// ECONOMÍA Y USO DE SUELO
// ---------------------------------------------------------------------------

fn tick_economy(gw: &mut GameWorld, dt: f32) {
    for (_entity, (storage,)) in gw.world.query::<(&mut ResourceStorage,)>().iter() {
        storage.food = (storage.food - 0.001 * dt).max(0.0);
        storage.money = (storage.money + 0.01 * dt).max(0.0);
    }
}

fn tick_land_use(gw: &mut GameWorld) {
    let mut to_spawn: Vec<(f32, f32, ZoneType)> = Vec::with_capacity(16);
    for (_entity, (pos, zone)) in gw.world.query::<(&Position, &ZoneComponent)>().iter() {
        if zone.density > 0 && rng_pool::rng_chance(0.0001) {
            to_spawn.push((pos.x, pos.y, zone.zone_type));
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
        if !gw.bitgrid.is_obstacle(x, y) {
            gw.world.spawn((
                Position::new(x, y),
                Renderable::rect(color, 2.0, 3),
                ConstructionState { progress: 0.0, building_type: btype },
                ResourceStorage { money: 100.0, food: 10.0, goods: 5.0 },
            ));
            gw.bitgrid.set(0, x, y);
        }
    }
}

fn tick_lifetimes(gw: &mut GameWorld) {
    let mut to_remove: Vec<hecs::Entity> = Vec::with_capacity(64);
    for (entity, (lifetime,)) in gw.world.query::<(&mut Lifetime,)>().iter() {
        if lifetime.remaining_ticks > 0 { lifetime.remaining_ticks -= 1; }
        else { to_remove.push(entity); }
    }
    for entity in to_remove { let _ = gw.world.despawn(entity); }
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
    fn test_full_tick_pipeline() {
        crate::luts::init_trig_luts();
        crate::rng_pool::init_rng_pool(42);
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        let initial = gw.world.len();
        tick(&mut gw, 0.1);
        assert!(gw.world.len() >= initial);
    }

    #[test]
    fn test_all_systems_tick_without_panic() {
        crate::luts::init_trig_luts();
        crate::rng_pool::init_rng_pool(42);
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        for _ in 0..50 { tick(&mut gw, 0.1); }
    }
}
