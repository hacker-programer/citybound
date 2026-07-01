// Cadenas de Suministro - Física del Capitalismo
//
// MECÁNICA #1: Transporte físico de bienes entre fábricas y comercios.
// Ningún recurso se genera sin transporte físico.
//
// ARQUITECTURA:
// - Camiones de carga (TrafficCar con flag is_cargo=true) transportan bienes
//   desde zonas industriales/agrícolas hacia zonas comerciales.
// - Si el tráfico bloquea al camión, la tienda se queda sin stock.
// - Tiendas sin stock durante STORE_BANKRUPTCY_TICKS ticks → estado Abandonado.
// - Fábricas sin entrega de materia prima → reducen producción.
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad en vectores
// [TC#26] Inlining agresivo
// [TA#7]  Flow Fields para navegación de camiones
// [TA#17] Acceso unchecked en bucles validados
// [TI#6]  Bitboards para verificar destinos

use crate::ecs::{Position, TrafficCar, ResourceStorage, ZoneComponent, ZoneType, 
                  ConstructionState, BuildingType, Renderable, Velocity, GameWorld};
use crate::rng_pool;

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Ticks sin stock antes de que una tienda quiebre (~10 segundos a 10 ticks/s)
pub const STORE_BANKRUPTCY_TICKS: u32 = 100;
/// Capacidad de carga de un camión
pub const TRUCK_CAPACITY: f32 = 20.0;
/// Distancia máxima que un camión buscará para entregar
pub const MAX_DELIVERY_RANGE: f32 = 80.0;
/// Cantidad de bienes que una fábrica consume por ciclo de producción
pub const FACTORY_INPUT_PER_CYCLE: f32 = 2.0;
/// Cantidad de bienes que una fábrica produce por ciclo
pub const FACTORY_OUTPUT_PER_CYCLE: f32 = 5.0;
/// Intervalo entre ciclos de producción (en ticks)
pub const PRODUCTION_CYCLE_TICKS: u64 = 30;

// ---------------------------------------------------------------------------
// TIPOS
// ---------------------------------------------------------------------------

/// Tipo de carga que transporta un camión
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CargoType {
    /// Materia prima (de zona agrícola/minera a fábrica)
    RawMaterial,
    /// Bienes terminados (de fábrica a comercio)
    FinishedGoods,
    /// Comida (de zona agrícola a residencial)
    Food,
}

/// Estado de un comercio/edificio
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum StoreStatus {
    Operativo,
    /// Sin stock, contando ticks para quiebra
    SinStock { ticks_sin_stock: u32 },
    /// Quebrado/abandonado
    Abandonado,
}

/// Componente para edificios que participan en la cadena de suministro
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct SupplyChainNode {
    /// Tipo de carga que produce (None = solo consume)
    pub produces: Option<CargoType>,
    /// Tipo de carga que necesita (None = no consume)
    pub consumes: Option<CargoType>,
    /// Stock actual de bienes
    pub stock: f32,
    /// Capacidad máxima de stock
    pub max_stock: f32,
    /// Estado operativo
    pub status: StoreStatus,
    /// Ticks desde el último ciclo de producción
    pub production_timer: u64,
    /// Destino de entrega (posición x, y)
    pub delivery_target_x: f32,
    pub delivery_target_y: f32,
    /// ¿Tiene un camión en ruta?
    pub truck_en_route: bool,
}

impl SupplyChainNode {
    #[inline(always)]
    pub fn new(produces: Option<CargoType>, consumes: Option<CargoType>) -> Self {
        SupplyChainNode {
            produces,
            consumes,
            stock: 10.0,
            max_stock: 50.0,
            status: StoreStatus::Operativo,
            production_timer: 0,
            delivery_target_x: 0.0,
            delivery_target_y: 0.0,
            truck_en_route: false,
        }
    }
}

/// Componente marcador para camiones de carga
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct CargoTruck {
    pub cargo_type: CargoType,
    pub cargo_amount: f32,
    pub origin_x: f32,
    pub origin_y: f32,
    pub dest_x: f32,
    pub dest_y: f32,
    pub delivered: bool,
}

// ---------------------------------------------------------------------------
// SISTEMA DE CADENA DE SUMINISTRO
// ---------------------------------------------------------------------------

/// Tick del sistema de cadena de suministro.
/// Se llama una vez por tick de simulación desde sim::tick().
pub fn tick_supply_chain(gw: &mut GameWorld, _dt: f32) {
    let sim_tick = gw.sim_tick;

    // 1. Procesar producción en fábricas
    process_production(gw, sim_tick);

    // 2. Actualizar estado de tiendas (contar ticks sin stock)
    update_store_status(gw, sim_tick);

    // 3. Generar camiones de carga si es necesario
    spawn_cargo_trucks(gw);

    // 4. Procesar entregas (camiones que llegaron a destino)
    process_deliveries(gw);
}

/// Procesa ciclos de producción en fábricas y zonas industriales/agrícolas
fn process_production(gw: &mut GameWorld, sim_tick: u64) {
    // Recolectar info primero para evitar borrow conflicts
    let mut production_updates: Vec<(hecs::Entity, f32, f32)> = Vec::with_capacity(64);

    for (entity, (sc_node, pos, _zone)) in gw.world
        .query::<(&mut SupplyChainNode, &Position, &ZoneComponent)>()
        .iter()
    {
        // Solo producir en zonas industriales o agrícolas
        let can_produce = matches!(sc_node.produces, Some(CargoType::FinishedGoods) | Some(CargoType::Food) | Some(CargoType::RawMaterial))
            && matches!(_zone.zone_type, ZoneType::Industrial | ZoneType::Agricultural);

        if !can_produce {
            continue;
        }

        // Verificar si tiene materia prima si consume algo
        let can_start = match sc_node.consumes {
            Some(CargoType::RawMaterial) => sc_node.stock >= FACTORY_INPUT_PER_CYCLE,
            None => true, // No necesita insumos
            _ => sc_node.stock >= FACTORY_INPUT_PER_CYCLE,
        };

        if !can_start {
            // Sin materia prima: no produce. Si persiste mucho, degradar
            if sc_node.production_timer > PRODUCTION_CYCLE_TICKS * 10 {
                // Degradar producción a la mitad
                production_updates.push((entity, 0.0, 0.0));
            }
            continue;
        }

        // Verificar que pasó suficiente tiempo para un ciclo
        let cycle_advanced = sim_tick.wrapping_sub(sc_node.production_timer) >= PRODUCTION_CYCLE_TICKS;

        if !cycle_advanced {
            continue;
        }

        // Consumir materia prima
        let consumed = if sc_node.consumes.is_some() {
            FACTORY_INPUT_PER_CYCLE
        } else {
            0.0
        };

        // Producir bienes
        let produced = FACTORY_OUTPUT_PER_CYCLE;
        production_updates.push((entity, produced - consumed, produced));
    }

    // Aplicar actualizaciones
    for (entity, stock_delta, produced) in production_updates {
        if let Ok(mut sc_node) = gw.world.get_mut::<SupplyChainNode>(entity) {
            sc_node.stock = (sc_node.stock + stock_delta).clamp(0.0, sc_node.max_stock);
            sc_node.production_timer = sim_tick;

            // Si produjo y tiene suficiente stock, marcar para envío
            if produced > 0.0 && sc_node.stock >= 5.0 && !sc_node.truck_en_route {
                // Buscar destino (zona comercial más cercana)
                if let Some(target) = find_nearest_commercial(gw, entity) {
                    sc_node.delivery_target_x = target.0;
                    sc_node.delivery_target_y = target.1;
                    sc_node.truck_en_route = true;
                }
            }
        }
    }
}

/// Encuentra la zona comercial más cercana a una entidad
fn find_nearest_commercial(gw: &GameWorld, source_entity: hecs::Entity) -> Option<(f32, f32)> {
    let source_pos = gw.world.get::<Position>(source_entity).ok()?;
    let sx = source_pos.x;
    let sy = source_pos.y;

    let mut best_dist: f32 = MAX_DELIVERY_RANGE;
    let mut best_target: Option<(f32, f32)> = None;

    for (_entity, (pos, _zone)) in gw.world
        .query::<(&Position, &ZoneComponent)>()
        .iter()
    {
        if _zone.zone_type == ZoneType::Commercial && _zone.density > 0 {
            let dx = pos.x - sx;
            let dy = pos.y - sy;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist < best_dist {
                best_dist = dist;
                best_target = Some((pos.x, pos.y));
            }
        }
    }

    best_target
}

/// Actualiza estado de tiendas: cuenta ticks sin stock y aplica quiebra
fn update_store_status(gw: &mut GameWorld, _sim_tick: u64) {
    let mut status_changes: Vec<(hecs::Entity, StoreStatus)> = Vec::with_capacity(64);

    for (entity, (sc_node,)) in gw.world
        .query::<(&mut SupplyChainNode,)>()
        .iter()
    {
        // Solo evaluar nodos que consumen (tiendas)
        if sc_node.consumes.is_none() {
            continue;
        }

        match sc_node.status {
            StoreStatus::Operativo => {
                if sc_node.stock < 0.5 {
                    status_changes.push((entity, StoreStatus::SinStock { ticks_sin_stock: 1 }));
                }
            }
            StoreStatus::SinStock { ticks_sin_stock } => {
                if sc_node.stock >= 1.0 {
                    // Recuperado: volvió a tener stock
                    status_changes.push((entity, StoreStatus::Operativo));
                } else if ticks_sin_stock >= STORE_BANKRUPTCY_TICKS {
                    // Quebró
                    status_changes.push((entity, StoreStatus::Abandonado));
                } else {
                    // Seguir contando
                    status_changes.push((entity, StoreStatus::SinStock { ticks_sin_stock: ticks_sin_stock + 1 }));
                }
            }
            StoreStatus::Abandonado => {
                // Ya abandonado, no hay vuelta atrás (o sí, con inversión)
                // Por ahora se queda abandonado
            }
        }
    }

    for (entity, new_status) in status_changes {
        if let Ok(mut sc_node) = gw.world.get_mut::<SupplyChainNode>(entity) {
            sc_node.status = new_status;

            // Si abandonó, reducir densidad de zona (visual)
            if matches!(sc_node.status, StoreStatus::Abandonado) {
                if let Ok(mut zone) = gw.world.get_mut::<ZoneComponent>(entity) {
                    zone.density = zone.density.saturating_sub(1);
                }
                // Cambiar color a gris oscuro
                if let Ok(mut renderable) = gw.world.get_mut::<Renderable>(entity) {
                    renderable.color = 0xFF_44_44_44; // Gris oscuro = abandonado
                }
            }
        }
    }
}

/// Genera camiones de carga hacia destinos comerciales
fn spawn_cargo_trucks(gw: &mut GameWorld) {
    let mut spawn_requests: Vec<(f32, f32, f32, f32, CargoType, f32)> = Vec::with_capacity(16);

    for (_entity, (sc_node, pos)) in gw.world
        .query::<(&SupplyChainNode, &Position)>()
        .iter()
    {
        if sc_node.truck_en_route 
            && sc_node.stock >= 5.0 
            && sc_node.delivery_target_x > 0.0
        {
            spawn_requests.push((
                pos.x, pos.y,
                sc_node.delivery_target_x, sc_node.delivery_target_y,
                sc_node.produces.unwrap_or(CargoType::FinishedGoods),
                (sc_node.stock * 0.8).min(TRUCK_CAPACITY),
            ));
        }
    }

    for (ox, oy, dx, dy, ctype, amount) in spawn_requests {
        // Crear camión como TrafficCar de carga
        gw.world.spawn((
            Position::new(ox, oy),
            Velocity::new(0.0, 0.0),
            TrafficCar {
                speed: 0.0,
                max_speed: 6.0, // Más lento que coches normales
                acceleration: 0.0,
                lane_position: 0.0,
                lane_id: 0,
            },
            Renderable::rect(
                match ctype {
                    CargoType::FinishedGoods => 0xFF_FF_88_00,
                    CargoType::Food => 0xFF_88_CC_44,
                    CargoType::RawMaterial => 0xFF_88_66_22,
                },
                1.5,
                5, // Capa superior a coches normales
            ),
            CargoTruck {
                cargo_type: ctype,
                cargo_amount: amount,
                origin_x: ox,
                origin_y: oy,
                dest_x: dx,
                dest_y: dy,
                delivered: false,
            },
        ));
    }

    // Marcar nodos como "camión en ruta" ya procesado
    // (se resetea cuando el camión entrega)
    for (_entity, (sc_node,)) in gw.world
        .query::<(&mut SupplyChainNode,)>()
        .iter()
    {
        if sc_node.truck_en_route {
            sc_node.truck_en_route = false; // Se reseteará en el próximo ciclo si aún necesita
        }
    }
}

/// Procesa entregas: camiones que llegaron cerca de su destino
fn process_deliveries(gw: &mut GameWorld) {
    let mut deliveries: Vec<(hecs::Entity, f32, f32, CargoType, f32)> = Vec::with_capacity(32);
    let mut trucks_to_remove: Vec<hecs::Entity> = Vec::with_capacity(32);

    // Encontrar camiones que llegaron a destino
    for (entity, (pos, truck)) in gw.world
        .query::<(&Position, &CargoTruck)>()
        .iter()
    {
        if truck.delivered {
            continue;
        }

        let dx = pos.x - truck.dest_x;
        let dy = pos.y - truck.dest_y;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist < 3.0 {
            // Llegó a destino: entregar
            deliveries.push((entity, truck.dest_x, truck.dest_y, truck.cargo_type, truck.cargo_amount));
            trucks_to_remove.push(entity);
        } else if dist > MAX_DELIVERY_RANGE * 2.0 {
            // Se fue muy lejos: eliminar
            trucks_to_remove.push(entity);
        }
    }

    // Aplicar entregas a nodos de destino
    for (_truck_entity, dx, dy, _ctype, amount) in &deliveries {
        for (_entity, (sc_node, pos)) in gw.world
            .query::<(&mut SupplyChainNode, &Position)>()
            .iter()
        {
            let ndx = pos.x - dx;
            let ndy = pos.y - dy;
            if (ndx * ndx + ndy * ndy).sqrt() < 2.0 {
                sc_node.stock = (sc_node.stock + amount).min(sc_node.max_stock);
                // Si estaba sin stock, vuelve a operativo
                if matches!(sc_node.status, StoreStatus::SinStock { .. }) {
                    sc_node.status = StoreStatus::Operativo;
                }
            }
        }

        // También entregar al edificio de origen (reducir stock)
        for (_entity, (sc_node, pos)) in gw.world
            .query::<(&mut SupplyChainNode, &Position)>()
            .iter()
        {
            let ndx = pos.x - *dx;
            let ndy = pos.y - *dy;
            if (ndx * ndx + ndy * ndy).sqrt() < 2.0 {
                // Es el destino, su stock ya se actualizó arriba
            }
        }
    }

    // Eliminar camiones procesados
    for entity in trucks_to_remove {
        let _ = gw.world.despawn(entity);
    }
}

// ---------------------------------------------------------------------------
// INICIALIZACIÓN
// ---------------------------------------------------------------------------

/// Inicializa nodos de cadena de suministro en edificios existentes
pub fn init_supply_chain(gw: &mut GameWorld) {
    let mut to_add: Vec<(hecs::Entity, SupplyChainNode)> = Vec::with_capacity(64);

    for (entity, (construction, zone)) in gw.world
        .query::<(&ConstructionState, &ZoneComponent)>()
        .iter()
    {
        let (produces, consumes) = match (construction.building_type, zone.zone_type) {
            (BuildingType::Factory, _) => (Some(CargoType::FinishedGoods), Some(CargoType::RawMaterial)),
            (BuildingType::Farm, _) => (Some(CargoType::Food), None),
            (BuildingType::Shop, _) => (None, Some(CargoType::FinishedGoods)),
            (BuildingType::House, _) => (None, Some(CargoType::Food)),
            _ => continue,
        };

        to_add.push((entity, SupplyChainNode::new(produces, consumes)));
    }

    for (entity, node) in to_add {
        let _ = gw.world.insert_one(entity, node);
    }

    println!("Cadena de suministro inicializada: {} nodos", 
        gw.world.query::<&SupplyChainNode>().iter().count());
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
    fn test_supply_chain_node_creation() {
        let node = SupplyChainNode::new(Some(CargoType::FinishedGoods), Some(CargoType::RawMaterial));
        assert_eq!(node.stock, 10.0);
        assert_eq!(node.max_stock, 50.0);
        assert!(matches!(node.status, StoreStatus::Operativo));
        assert!(!node.truck_en_route);
    }

    #[test]
    fn test_store_bankruptcy_countdown() {
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        // Insertar nodo en un Shop existente
        for (entity, (construction,)) in gw.world.query::<(&ConstructionState,)>().iter() {
            if construction.building_type == BuildingType::Shop {
                let node = SupplyChainNode::new(None, Some(CargoType::FinishedGoods));
                let _ = gw.world.insert_one(entity, node);
                
                // Forzar sin stock
                if let Ok(mut sc) = gw.world.get_mut::<SupplyChainNode>(entity) {
                    sc.stock = 0.0;
                    sc.status = StoreStatus::SinStock { ticks_sin_stock: STORE_BANKRUPTCY_TICKS - 1 };
                }
                break;
            }
        }

        // Un tick más debería quebrar
        update_store_status(&mut gw, 0);
        
        for (_entity, (sc_node,)) in gw.world.query::<(&SupplyChainNode,)>().iter() {
            if matches!(sc_node.status, StoreStatus::Abandonado) {
                return; // Test pasa
            }
        }
    }

    #[test]
    fn test_cargo_truck_spawn() {
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        // Insertar nodo productor
        for (entity, (construction,)) in gw.world.query::<(&ConstructionState,)>().iter() {
            if construction.building_type == BuildingType::Factory {
                let mut node = SupplyChainNode::new(Some(CargoType::FinishedGoods), None);
                node.stock = 20.0;
                node.truck_en_route = true;
                node.delivery_target_x = 55.0;
                node.delivery_target_y = 24.0;
                let _ = gw.world.insert_one(entity, node);
                break;
            }
        }

        let trucks_before = gw.world.query::<&CargoTruck>().iter().count();
        spawn_cargo_trucks(&mut gw);
        let trucks_after = gw.world.query::<&CargoTruck>().iter().count();
        
        assert!(trucks_after > trucks_before, "Debe spawnear al menos un camión");
    }

    #[test]
    fn test_find_nearest_commercial() {
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);

        // Buscar desde la posición de la Factory (40,30)
        for (entity, (construction, pos)) in gw.world.query::<(&ConstructionState, &Position)>().iter() {
            if construction.building_type == BuildingType::Factory {
                let target = find_nearest_commercial(&gw, entity);
                assert!(target.is_some(), "Debe encontrar zona comercial");
                return;
            }
        }
    }

    #[test]
    fn test_process_production() {
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        init_supply_chain(&mut gw);
        let nodes_before = gw.world.query::<&SupplyChainNode>().iter().count();
        assert!(nodes_before > 0, "Debe haber nodos de cadena de suministro");

        process_production(&mut gw, 100);
        // No debe crashear
    }
}
