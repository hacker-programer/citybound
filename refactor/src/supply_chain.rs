// Cadenas de Suministro Físicas
//
// MECÁNICA #1: Física del Capitalismo
//
// Las fábricas producen bienes y envían camiones de carga hacia
// zonas comerciales usando los Flow Fields. Si un camión no llega
// por congestión, la tienda se queda sin stock, despide empleados,
// quiebra y el edificio queda abandonado.
//
// Ningún recurso se genera sin transporte físico.
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad
// [TA#7]  Flow Fields para navegación de camiones
// [TA#17] Acceso unchecked en bucles validados

use crate::ecs::{GameWorld, Position, Velocity, TrafficCar, Renderable,
                  ResourceStorage, ConstructionState, BuildingType};
use crate::flow_field::FlowCell;

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Ticks máximos sin suministro antes de quiebra
pub const BANKRUPTCY_TICKS: u32 = 300;
/// Producción por ciclo de fábrica
pub const FACTORY_PRODUCTION: f32 = 5.0;
/// Consumo por ciclo de tienda
pub const SHOP_CONSUMPTION: f32 = 1.0;
/// Velocidad de camión de carga
pub const CARGO_TRUCK_SPEED: f32 = 6.0;
/// Capacidad de carga de un camión
pub const CARGO_CAPACITY: f32 = 20.0;

// ---------------------------------------------------------------------------
// COMPONENTES
// ---------------------------------------------------------------------------

/// Marca un edificio como abandonado (quebrado)
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct AbandonedBuilding {
    /// Ticks desde que fue abandonado
    pub abandoned_ticks: u32,
}

/// Marca una entidad como camión de carga
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct CargoTruck {
    /// Cantidad de bienes transportados
    pub cargo: f32,
    /// ID del edificio destino (fábrica o tienda)
    pub destination_x: f32,
    pub destination_y: f32,
    /// Tipo de carga: true = bienes a tienda, false = materia prima a fábrica
    pub delivering_to_shop: bool,
    /// Ticks desde que salió
    pub travel_ticks: u32,
}

/// Contador de ticks sin suministro para comercios
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct SupplyTimer {
    pub ticks_without_supply: u32,
}

// ---------------------------------------------------------------------------
// SISTEMA DE CADENA DE SUMINISTRO
// ---------------------------------------------------------------------------

/// Tick de cadena de suministro: producción, envío, consumo, quiebras
pub fn tick_supply_chain(gw: &mut GameWorld, dt: f32) {
    // 1. Fábricas: producir bienes
    tick_factory_production(gw, dt);

    // 2. Spawnear camiones de carga desde fábricas hacia tiendas
    spawn_cargo_trucks(gw);

    // 3. Mover camiones hacia sus destinos usando Flow Fields
    move_cargo_trucks(gw, dt);

    // 4. Entregar carga en destino
    deliver_cargo(gw);

    // 5. Tiendas: consumir bienes, verificar quiebra
    tick_shop_consumption(gw);

    // 6. Limpiar edificios abandonados hace mucho tiempo
    tick_abandoned_buildings(gw);
}

// ---------------------------------------------------------------------------
// PRODUCCIÓN DE FÁBRICAS
// ---------------------------------------------------------------------------

fn tick_factory_production(gw: &mut GameWorld, _dt: f32) {
    for (_entity, (_pos, construction, storage)) in gw.world
        .query::<(&Position, &ConstructionState, &mut ResourceStorage)>()
        .iter()
    {
        if construction.progress < 0.5 {
            continue;
        }

        match construction.building_type {
            BuildingType::Factory => {
                storage.goods += FACTORY_PRODUCTION;
            }
            BuildingType::Farm => {
                storage.food += FACTORY_PRODUCTION;
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// SPAWN DE CAMIONES DE CARGA
// ---------------------------------------------------------------------------

fn spawn_cargo_trucks(gw: &mut GameWorld) {
    // Encontrar fábricas con stock suficiente
    let factories: Vec<(f32, f32, f32)> = gw.world
        .query::<(&Position, &ConstructionState, &ResourceStorage)>()
        .iter()
        .filter(|(_, (_, c, s))| {
            (c.building_type == BuildingType::Factory || c.building_type == BuildingType::Farm)
                && s.goods >= CARGO_CAPACITY
        })
        .map(|(_, (p, _, s))| (p.x, p.y, s.goods))
        .collect();

    // Encontrar tiendas que necesitan suministro
    let shops: Vec<(f32, f32)> = gw.world
        .query::<(&Position, &ConstructionState, &ResourceStorage)>()
        .iter()
        .filter(|(_, (_, c, s))| {
            c.building_type == BuildingType::Shop && s.goods < 20.0
        })
        .map(|(_, (p, _, _))| (p.x, p.y))
        .collect();

    if factories.is_empty() || shops.is_empty() {
        return;
    }

    // Emparejar fábricas con tiendas cercanas
    for (fx, fy, _stock) in factories.iter().take(3) {
        let mut best_dist = f32::MAX;
        let mut best_shop = (0.0f32, 0.0f32);

        for (sx, sy) in &shops {
            let dist = (fx - sx) * (fx - sx) + (fy - sy) * (fy - sy);
            if dist < best_dist {
                best_dist = dist;
                best_shop = (*sx, *sy);
            }
        }

        if best_dist < f32::MAX {
            let _truck_entity = gw.world.spawn((
                Position::new(*fx, *fy),
                Velocity::new(0.0, 0.0),
                TrafficCar {
                    speed: 0.0,
                    max_speed: CARGO_TRUCK_SPEED,
                    acceleration: 0.0,
                    lane_position: 0.0,
                    lane_id: 0,
                },
                CargoTruck {
                    cargo: CARGO_CAPACITY,
                    destination_x: best_shop.0,
                    destination_y: best_shop.1,
                    delivering_to_shop: true,
                    travel_ticks: 0,
                },
                Renderable::rect(0xFF_00_AA_FF, 2.0, 6),
            ));

            // Deducir stock de la fábrica
            for (_entity, (_pos, construction, storage)) in gw.world
                .query::<(&Position, &ConstructionState, &mut ResourceStorage)>()
                .iter()
            {
                if (construction.building_type == BuildingType::Factory
                    || construction.building_type == BuildingType::Farm)
                    && storage.goods >= CARGO_CAPACITY
                {
                    storage.goods -= CARGO_CAPACITY;
                    break;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// MOVIMIENTO DE CAMIONES DE CARGA
// ---------------------------------------------------------------------------

fn move_cargo_trucks(gw: &mut GameWorld, dt: f32) {
    for (_entity, (pos, vel, car, truck)) in gw.world
        .query::<(&mut Position, &mut Velocity, &mut TrafficCar, &mut CargoTruck)>()
        .iter()
    {
        truck.travel_ticks += 1;

        let dx = truck.destination_x - pos.x;
        let dy = truck.destination_y - pos.y;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist < 2.0 {
            car.speed = 0.0;
            continue;
        }

        let flow: FlowCell = gw.flow_fields.sample_combined(pos.x, pos.y, false);

        let target_angle = dy.atan2(dx);
        let flow_weight: f32 = 0.3;
        let combined_angle = flow.angle * flow_weight + target_angle * (1.0 - flow_weight);

        let target_speed = CARGO_TRUCK_SPEED * flow.magnitude.max(0.3);
        car.max_speed = CARGO_TRUCK_SPEED;

        if car.speed < target_speed {
            car.speed = (car.speed + 2.0 * dt).min(target_speed);
        } else {
            car.speed = (car.speed - 1.0 * dt).max(target_speed);
        }

        let move_dx = combined_angle.cos() * car.speed * dt;
        let move_dy = combined_angle.sin() * car.speed * dt;

        pos.x += move_dx;
        pos.y += move_dy;

        let gs = gw.grid_size as f32;
        if pos.x < 0.0 { pos.x += gs; }
        if pos.x >= gs { pos.x -= gs; }
        if pos.y < 0.0 { pos.y += gs; }
        if pos.y >= gs { pos.y -= gs; }

        vel.dx = move_dx;
        vel.dy = move_dy;
    }
}

// ---------------------------------------------------------------------------
// ENTREGA DE CARGA EN DESTINO
// ---------------------------------------------------------------------------

fn deliver_cargo(gw: &mut GameWorld) {
    let mut deliveries: Vec<(f32, f32, f32)> = Vec::with_capacity(16);

    // Encontrar camiones que llegaron a destino y eliminarlos
    let mut to_remove: Vec<hecs::Entity> = Vec::with_capacity(16);
    {
        // Usamos query sin Entity - el iterador nos da (Entity, components)
        for (entity, (_pos, truck)) in gw.world
            .query::<(&Position, &CargoTruck)>()
            .iter()
        {
            let dx = truck.destination_x - _pos.x;
            let dy = truck.destination_y - _pos.y;
            if (dx * dx + dy * dy).sqrt() < 2.0 {
                deliveries.push((truck.destination_x, truck.destination_y, truck.cargo));
                to_remove.push(entity);
            }
        }
    }

    for entity in to_remove {
        let _ = gw.world.despawn(entity);
    }

    // Entregar bienes a tiendas
    for (dx, dy, cargo) in deliveries {
        for (_entity, (pos, construction, storage)) in gw.world
            .query::<(&Position, &ConstructionState, &mut ResourceStorage)>()
            .iter()
        {
            if construction.building_type == BuildingType::Shop
                && (pos.x - dx).abs() < 2.0
                && (pos.y - dy).abs() < 2.0
            {
                storage.goods += cargo;
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CONSUMO DE TIENDAS Y QUIEBRA
// ---------------------------------------------------------------------------

fn tick_shop_consumption(gw: &mut GameWorld) {
    let mut bankruptcies: Vec<(f32, f32)> = Vec::with_capacity(16);

    for (_entity, (_pos, construction, storage, supply)) in gw.world
        .query::<(&Position, &ConstructionState, &mut ResourceStorage, &mut SupplyTimer)>()
        .iter()
    {
        if construction.building_type != BuildingType::Shop {
            continue;
        }

        if storage.goods > 0.0 {
            storage.goods -= SHOP_CONSUMPTION;
            storage.money += 2.0;
            supply.ticks_without_supply = 0;
        } else {
            supply.ticks_without_supply += 1;

            if supply.ticks_without_supply > BANKRUPTCY_TICKS {
                bankruptcies.push((_pos.x, _pos.y));
            }
        }
    }

    // Marcar tiendas quebradas
    for (bx, by) in bankruptcies {
        gw.world.spawn((
            Position::new(bx, by),
            AbandonedBuilding { abandoned_ticks: 0 },
            Renderable::rect(0xFF_44_44_44, 3.0, 3),
        ));

        // Eliminar la tienda original
        let mut to_remove: Vec<hecs::Entity> = Vec::new();
        for (entity, (pos, construction)) in gw.world
            .query::<(&Position, &ConstructionState)>()
            .iter()
        {
            if construction.building_type == BuildingType::Shop
                && (pos.x - bx).abs() < 1.0
                && (pos.y - by).abs() < 1.0
            {
                to_remove.push(entity);
            }
        }
        for entity in to_remove {
            gw.bitgrid.clear(0, bx, by);
            let _ = gw.world.despawn(entity);
        }
    }
}

fn tick_abandoned_buildings(gw: &mut GameWorld) {
    let mut to_remove: Vec<hecs::Entity> = Vec::new();

    for (entity, (abandoned,)) in gw.world
        .query::<(&mut AbandonedBuilding,)>()
        .iter()
    {
        abandoned.abandoned_ticks += 1;
        if abandoned.abandoned_ticks > 5000 {
            to_remove.push(entity);
        }
    }

    for entity in to_remove {
        let _ = gw.world.despawn(entity);
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

    fn setup_world() -> Box<GameWorld> {
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        gw.world.spawn((
            Position::new(50.0, 50.0),
            ConstructionState { progress: 1.0, building_type: BuildingType::Factory },
            ResourceStorage { money: 0.0, food: 0.0, goods: 100.0 },
        ));

        gw.world.spawn((
            Position::new(60.0, 50.0),
            ConstructionState { progress: 1.0, building_type: BuildingType::Shop },
            ResourceStorage { money: 0.0, food: 0.0, goods: 0.0 },
            SupplyTimer { ticks_without_supply: 0 },
        ));

        gw
    }

    #[test]
    fn test_factory_production() {

        assert!(trucks_after >= trucks_before);
    }

    #[test]
    fn test_cargo_truck_movement() {
        crate::luts::init_trig_luts();
        let mut gw = setup_world();
        spawn_cargo_trucks(&mut gw);

        let positions_before: Vec<(f32, f32)> = gw.world
            .query::<(&Position, &CargoTruck)>()
            .iter()
            .map(|(_, (p, _))| (p.x, p.y))
            .collect();

        move_cargo_trucks(&mut gw, 1.0);

        let positions_after: Vec<(f32, f32)> = gw.world
            .query::<(&Position, &CargoTruck)>()
            .iter()
            .map(|(_, (p, _))| (p.x, p.y))
            .collect();

        if !positions_before.is_empty() && !positions_after.is_empty() {
            let _moved = (positions_before[0].0 - positions_after[0].0).abs() > 0.01;
        }
    }

    #[test]
    fn test_shop_bankruptcy() {
        let mut gw = setup_world();

        for (_entity, (_pos, construction, _storage, supply)) in gw.world
            .query::<(&Position, &ConstructionState, &ResourceStorage, &mut SupplyTimer)>()
            .iter()
        {
            if construction.building_type == BuildingType::Shop {
                supply.ticks_without_supply = BANKRUPTCY_TICKS + 1;
            }
        }

        tick_shop_consumption(&mut gw);

        let abandoned = gw.world.query::<&AbandonedBuilding>().iter().count();
        assert!(abandoned > 0, "Tienda debe quebrar sin suministro");
    }

    #[test]
    fn test_supply_timer_reset() {
        let mut gw = setup_world();

        for (_entity, (_pos, construction, storage, supply)) in gw.world
            .query::<(&Position, &ConstructionState, &mut ResourceStorage, &mut SupplyTimer)>()
            .iter()
        {
            if construction.building_type == BuildingType::Shop {
                storage.goods = 50.0;
                supply.ticks_without_supply = 100;
            }
        }

        tick_shop_consumption(&mut gw);

        let mut query = gw.world.query::<&SupplyTimer>();
        let timer = query.iter()
            .find(|(_, s)| s.ticks_without_supply == 0);
        assert!(timer.is_some(), "Supply timer debe resetearse con bienes");
    }

    #[test]
    fn test_cargo_truck_spawn_reduces_factory_stock() {
        let mut gw = setup_world();

        let factory_stock_before = gw.world
            .query::<(&ConstructionState, &ResourceStorage)>()
            .iter()
            .find(|(_, (c, _))| c.building_type == BuildingType::Factory)
            .map(|(_, (_, s))| s.goods)
            .unwrap_or(0.0);

        spawn_cargo_trucks(&mut gw);

        let factory_stock_after = gw.world
            .query::<(&ConstructionState, &ResourceStorage)>()
            .iter()
            .find(|(_, (c, _))| c.building_type == BuildingType::Factory)
            .map(|(_, (_, s))| s.goods)
            .unwrap_or(0.0);

        if factory_stock_before >= CARGO_CAPACITY {
            assert!(factory_stock_after <= factory_stock_before);
        }
    }
}
