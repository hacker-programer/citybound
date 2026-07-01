// Módulo de Simulación v0.7.0
//
// SUBSISTEMAS:
// - time: Avance del tiempo
// - traffic: Flow Fields [TA#7] + Bitboards [TI#6] + Lanes [#361] + RoadWear [M#4]
// - economy: Economía de hogares y recursos
// - land_use: Zonificación y desarrollo
// - supply_chain: Cadenas de suministro [M#1]
// - land_value: Valor del suelo y gentrificación [M#2]
// - utilities: Agua y electricidad [M#3]
// - road_wear: Desgaste de calles [M#4]
// - labor_market: Mercado laboral [M#5]

use crate::ecs::{GameWorld, Position, Velocity, TrafficCar, ZoneComponent, ZoneType,
                  ResourceStorage, ConstructionState, Lifetime, BuildingType, Renderable};
use crate::flow_field::{FlowFieldManager, FlowCell};
use crate::bitboard::BitGrid;
use crate::traffic_lanes::{LaneManager, LaneDirection, IdmParams};
use crate::rng_pool;
use crate::supply_chain;
use crate::land_value;
use crate::utilities;
use crate::road_wear;
use crate::labor_market;

pub fn init_simulation(game_world: &mut GameWorld) {
    game_world.sim_tick = 0;
    game_world.time_of_day = 7 * 60;

    init_bitboard_obstacles(game_world);
    init_car_idm_params(game_world);

    // [M#1]: Inicializar cadenas de suministro
    supply_chain::init_supply_chain(game_world);

    // [M#5]: Inicializar mercado laboral
    labor_market::init_labor_market(game_world);

    // [M#3]: Configurar fuentes de servicios (centro del mapa)
    game_world.utility_grid.add_water_source(64.0, 64.0);
    game_world.utility_grid.add_power_source(64.0, 64.0);
}

fn init_bitboard_obstacles(gw: &mut GameWorld) {
    let obstacle_positions: Vec<(f32, f32)> = gw.world
        .query::<(&Position, &ConstructionState)>()
        .iter()
        .flat_map(|(_entity, (pos, _))| {
            let mut positions = Vec::with_capacity(9);
            for dx in -1i32..=1 {
                for dy in -1i32..=1 {
                    positions.push((pos.x + dx as f32, pos.y + dy as f32));
                }
            }
            positions
        })
        .collect();
    for (x, y) in obstacle_positions { gw.bitgrid.set(0, x, y); }
}

fn init_car_idm_params(gw: &mut GameWorld) {
    let assignments: Vec<(u32, IdmParams)> = gw.world.query::<&TrafficCar>()
        .iter()
        .map(|(entity, car)| (entity.to_bits() as u32, IdmParams { desired_speed: car.max_speed, ..IdmParams::default() }))
        .collect();
    for (entity_bits, params) in assignments { gw.lane_manager.set_vehicle_params(entity_bits, params); }
}

// ---------------------------------------------------------------------------
// TICK PRINCIPAL
// ---------------------------------------------------------------------------

pub fn tick(game_world: &mut GameWorld, dt: f32) {
    // 1. Tiempo
    tick_time(game_world);

    // 2. Intersecciones
    tick_intersections(game_world, dt);

    // 3. Tráfico con desgaste de calles [M#4] integrado
    tick_traffic_flow(game_world, dt);

    // 4. Congestión
    tick_lane_congestion(game_world);

    // 5. Desgaste de carreteras [M#4]
    road_wear::tick_road_wear(&mut game_world.road_wear, game_world);

    // 6. Servicios públicos [M#3]
    utilities::tick_utilities(&mut game_world.utility_grid, game_world);

    // 7. Cadenas de suministro [M#1]
    supply_chain::tick_supply_chain(game_world, dt);

    // 8. Valor del suelo [M#2]
    land_value::tick_land_value(game_world);

    // 9. Mercado laboral [M#5]
    labor_market::tick_labor_market(game_world);

    // 10. Economía base
    tick_economy(game_world, dt);

    // 11. Desarrollo de zonas
    tick_land_use(game_world);

    // 12. Limpiar expirados
    tick_lifetimes(game_world);
}

// ---------------------------------------------------------------------------
// TIEMPO
// ---------------------------------------------------------------------------

const TICKS_PER_SIM_SECOND: u32 = 3;
const MINUTES_PER_DAY: u16 = 24 * 60;
const BEGINNING_TIME_OF_DAY: u16 = 7 * 60;

fn tick_time(game_world: &mut GameWorld) {
    game_world.sim_tick = game_world.sim_tick.wrapping_add(1);
    if game_world.sim_tick % TICKS_PER_SIM_SECOND as u64 == 0 {
        let sim_seconds = game_world.sim_tick / TICKS_PER_SIM_SECOND as u64;
        game_world.time_of_day = ((BEGINNING_TIME_OF_DAY as u64 + (sim_seconds / 60)) % MINUTES_PER_DAY as u64) as u16;
    }
}

pub fn formatted_time(time_of_day: u16) -> String {
    format!("{:02}:{:02}", time_of_day / 60, time_of_day % 60)
}

fn tick_intersections(gw: &mut GameWorld, dt: f32) {
    for intersection in gw.lane_manager.intersections.iter_mut() { intersection.tick(dt); }
}

fn tick_lane_congestion(gw: &mut GameWorld) {
    for lane in gw.lane_manager.lanes.iter_mut() { lane.vehicle_count = 0; }
    for (_entity, (_pos, car)) in gw.world.query::<(&Position, &TrafficCar)>().iter() {
        let lane_id = car.lane_id as usize;
        if lane_id < gw.lane_manager.lanes.len() { gw.lane_manager.lanes[lane_id].vehicle_count += 1; }
    }
    for lane in gw.lane_manager.lanes.iter_mut() {
        let capacity = (lane.length / 2.0).max(1.0);
        lane.congestion = (lane.vehicle_count as f32 / capacity).min(1.0);
    }
}

// ---------------------------------------------------------------------------
// TRÁFICO CON DESGASTE DE CALLES [M#4]
// ---------------------------------------------------------------------------

const MAX_ACCELERATION: f32 = 3.0;
const MAX_DECELERATION: f32 = 6.0;
const HIGHWAY_SPEED: f32 = 20.0;
const STREET_SPEED: f32 = 8.0;

fn tick_traffic_flow(gw: &mut GameWorld, dt: f32) {
    gw.bitgrid.clear_layer(5);

    let car_positions: Vec<(f32, f32)> = gw.world.query::<&Position>()
        .iter().map(|(_e, pos)| (pos.x, pos.y)).collect();
    for (x, y) in car_positions { gw.bitgrid.set(5, x, y); }

    let num_lanes = gw.lane_manager.lanes.len();
    let has_intersections = !gw.lane_manager.intersections.is_empty();

    for (_entity, (pos, vel, car)) in gw.world
        .query::<(&mut Position, &mut Velocity, &mut TrafficCar)>()
        .iter()
    {
        let lane_speed = if (car.lane_id as usize) < num_lanes {
            let lane = &gw.lane_manager.lanes[car.lane_id as usize];
            let can_proceed = if let Some(intersection_id) = lane.to_intersection {
                if has_intersections && (intersection_id as usize) < gw.lane_manager.intersections.len() {
                    gw.lane_manager.intersections[intersection_id as usize].can_proceed(car.lane_id)
                } else { true }
            } else { true };
            if can_proceed { lane.speed_limit } else { lane.speed_limit * 0.3 }
        } else { STREET_SPEED };

        let flow: FlowCell = gw.flow_fields.sample_combined(pos.x, pos.y, false);
        let on_highway = flow.magnitude > 0.5 && flow.angle.abs() < 0.3;
        let base_max_speed = if on_highway { HIGHWAY_SPEED } else { lane_speed };

        // [M#4]: Aplicar factor de desgaste de la carretera
        let gx = pos.x as usize % 128;
        let gy = pos.y as usize % 128;
        let wear_factor = gw.road_wear.speed_factor(gx, gy);
        let max_speed = base_max_speed * wear_factor;

        car.max_speed = max_speed;
        let target_speed = max_speed * flow.magnitude.max(0.3);

        let look_ahead_x = pos.x + flow.angle.cos() * 3.0;
        let look_ahead_y = pos.y + flow.angle.sin() * 3.0;
        let obstacle_ahead = gw.bitgrid.is_obstacle(look_ahead_x, look_ahead_y)
            || gw.bitgrid.test(5, look_ahead_x, look_ahead_y);

        let desired_accel: f32 = if obstacle_ahead { -MAX_DECELERATION }
            else if car.speed < target_speed { MAX_ACCELERATION * (1.0 - car.speed / target_speed.max(0.1)) }
            else if car.speed > target_speed * 1.1 { -MAX_ACCELERATION * 0.3 }
            else { 0.0 };

        car.acceleration = desired_accel.clamp(-MAX_DECELERATION, MAX_ACCELERATION);
        car.speed = (car.speed + car.acceleration * dt).clamp(0.0, max_speed);

        let (flow_dx, flow_dy) = FlowField::cell_to_velocity(&flow, car.speed);
        pos.x += flow_dx * dt;
        pos.y += flow_dy * dt;

        let gs = gw.grid_size as f32;
        if pos.x < 0.0 { pos.x += gs; }
        if pos.x >= gs { pos.x -= gs; }
        if pos.y < 0.0 { pos.y += gs; }
        if pos.y >= gs { pos.y -= gs; }

        vel.dx = flow_dx;
        vel.dy = flow_dy;

        if (car.lane_id as usize) < num_lanes {
            let lane = &gw.lane_manager.lanes[car.lane_id as usize];
            let (t, _, _) = lane.project(pos.x, pos.y);
            car.lane_position = t;
        }
    }
}

// ---------------------------------------------------------------------------
// ECONOMÍA, USO DE SUELO, LIFETIMES
// ---------------------------------------------------------------------------

fn tick_economy(gw: &mut GameWorld, dt: f32) {
    for (_entity, (storage,)) in gw.world.query::<(&mut ResourceStorage,)>().iter() {
        storage.food -= 0.001 * dt;
        storage.money += 0.01 * dt;
        storage.food = storage.food.max(0.0);
        storage.money = storage.money.max(0.0);
    }
}

fn tick_land_use(game_world: &mut GameWorld) {
    let mut to_spawn: Vec<(f32, f32, ZoneType)> = Vec::with_capacity(16);
    for (_entity, (pos, zone)) in game_world.world.query::<(&Position, &ZoneComponent)>().iter() {
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
        if !game_world.bitgrid.is_obstacle(x, y) {
            game_world.world.spawn((
                Position::new(x, y), Renderable::rect(color, 2.0, 3),
                ConstructionState { progress: 0.0, building_type: btype },
                ResourceStorage { money: 100.0, food: 10.0, goods: 5.0 },
            ));
            game_world.bitgrid.set(0, x, y);
        }
    }
}

fn tick_lifetimes(game_world: &mut GameWorld) {
    let mut to_remove: Vec<hecs::Entity> = Vec::with_capacity(64);
    for (entity, (lifetime,)) in game_world.world.query::<(&mut Lifetime,)>().iter() {
        if lifetime.remaining_ticks > 0 { lifetime.remaining_ticks -= 1; }
        else { to_remove.push(entity); }
    }
    for entity in to_remove { let _ = game_world.world.despawn(entity); }
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
        for _ in 0..180 { tick_time(&mut gw); }
        assert_eq!(gw.time_of_day, 7 * 60 + 1);
    }

    #[test]
    fn test_formatted_time_output() {
        assert_eq!(formatted_time(7 * 60), "07:00");
        assert_eq!(formatted_time(12 * 60 + 30), "12:30");
    }

    #[test]
    fn test_tick_traffic_flow_moves_cars() {
        crate::luts::init_trig_luts();
        crate::rng_pool::init_rng_pool(42);
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        let before = gw.world.query::<&TrafficCar>().iter().count();
        for _ in 0..10 { tick_traffic_flow(&mut gw, 0.1); }
        assert_eq!(gw.world.query::<&TrafficCar>().iter().count(), before);
    }

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
    fn test_new_systems_in_tick() {
        crate::luts::init_trig_luts();
        crate::rng_pool::init_rng_pool(42);
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        // Ejecutar varios ticks para probar todos los sistemas nuevos
        for _ in 0..50 {
            tick(&mut gw, 0.1);
        }
        // No debe crashear
    }
}
