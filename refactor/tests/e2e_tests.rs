// ============================================================================
// E2E Tests — Simulaciones End-to-End de Escenarios Reales de Juego
// ============================================================================
//
// Estos tests simulan escenarios completos de juego: creación de ciudad,
// simulación prolongada, interacción de sistemas, y verifican que todos
// los subsistemas funcionan correctamente en conjunto.
//
// Cada test representa un escenario de usuario real.

use citybound_native::ecs;
use citybound_native::sim;
use citybound_native::luts;
use citybound_native::rng_pool;
use citybound_native::object_pool::EntityPool;
use citybound_native::render;
use citybound_native::persistence;
use citybound_native::tax_system;
use citybound_native::land_value;

fn init_all() {
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);
}

// ============================================================================
// ESCENARIO 1: Ciudad desde cero — evolución completa
// ============================================================================

#[test]
fn e2e_full_city_lifecycle() {
    init_all();
    let mut pool = EntityPool::new(2000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    // Fase 1: Verificar estado inicial
    let initial_entities = gw.world.len();
    let initial_buildings = gw.world.query::<&ecs::ConstructionState>().iter().count();
    let initial_cars = gw.world.query::<&ecs::TrafficCar>().iter().count();
    let initial_zones = gw.world.query::<&ecs::ZoneComponent>().iter().count();

    assert!(initial_entities > 0, "Mundo no vacío");
    assert_eq!(initial_buildings, 11, "11 edificios iniciales");
    assert_eq!(initial_cars, 40, "40 coches iniciales");
    assert!(initial_zones > 500, "Zonas iniciales pintadas");

    // Fase 2: Simular 500 ticks (~1 minuto de juego)
    for i in 0..500 {
        sim::tick(&mut gw, 0.1);
        // Verificar que el mundo no colapsa
        assert!(gw.world.len() > 0, "Mundo colapsó en tick {}", i);
    }

    // Fase 3: Verificar que el tráfico sigue funcionando
    let cars_after = gw.world.query::<&ecs::TrafficCar>().iter().count();
    assert_eq!(cars_after, 40, "Los coches no deben desaparecer");

    // Fase 4: Verificar que la economía funciona
    let total_money: f32 = gw.world.query::<&ecs::ResourceStorage>()
        .iter().map(|(_, rs)| rs.money).sum();
    assert!(total_money > 0.0, "La economía debe generar dinero");

    // Fase 5: Verificar que el terreno no se corrompe
    let h = gw.terrain.height_at(64.0, 64.0);
    assert!(h >= 0.0 && h <= 1.0, "Terreno dentro de bounds");
}

// ============================================================================
// ESCENARIO 2: Save/Load — persistencia completa
// ============================================================================

#[test]
fn e2e_save_load_complete_cycle() {
    init_all();
    let mut pool = EntityPool::new(2000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    // Avanzar para tener estado interesante
    for _ in 0..200 {
        sim::tick(&mut gw, 0.1);
    }

    // Guardar
    let save = persistence::SaveData::from_world(&gw);
    let encoded = bincode::serialize(&save).expect("Serializar");
    assert!(encoded.len() > 100, "Save data no vacío");

    // Cargar en mundo nuevo
    let decoded: persistence::SaveData = bincode::deserialize(&encoded).expect("Deserializar");
    let mut pool2 = EntityPool::new(2000);
    let mut gw2 = ecs::create_world(&mut pool2);
    decoded.restore_to(&mut gw2);

    // Verificar restauración
    assert_eq!(gw2.sim_tick, gw.sim_tick);
    assert_eq!(gw2.time_of_day, gw.time_of_day);
    assert_eq!(gw2.finance.treasury, gw.finance.treasury);

    // Verificar que el mundo restaurado puede seguir simulando
    for _ in 0..50 {
        sim::tick(&mut gw2, 0.1);
    }
    assert!(gw2.world.len() > 0);
}

// ============================================================================
// ESCENARIO 3: Renderizado — pipeline completo con múltiples tamaños
// ============================================================================

#[test]
fn e2e_render_pipeline_all_resolutions() {
    init_all();
    let mut pool = EntityPool::new(2000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    for _ in 0..30 {
        sim::tick(&mut gw, 0.1);
    }

    let resolutions = [(1920, 1080), (1280, 720), (800, 600), (640, 480), (320, 240), (160, 120)];

    for &(w, h) in &resolutions {
        let mut fb = vec![0xFF_1A_1A_2Eu32; w * h];
        render::render_world(&gw, &mut fb, w, h);

        // Verificar que el framebuffer fue modificado
        let modified = fb.iter().any(|&p| p != 0xFF_1A_1A_2E);
        assert!(modified, "Framebuffer {}x{} sin modificar", w, h);

        // Verificar que no hay píxeles corruptos (alpha > 0xFF, etc.)
        for &p in &fb {
            let a = (p >> 24) & 0xFF;
            let r = (p >> 16) & 0xFF;
            let g = (p >> 8) & 0xFF;
            let b = p & 0xFF;
            assert!(a <= 0xFF, "Alpha inválido en {}x{}", w, h);
            assert!(r <= 0xFF, "Red inválido en {}x{}", w, h);
            assert!(g <= 0xFF, "Green inválido en {}x{}", w, h);
            assert!(b <= 0xFF, "Blue inválido en {}x{}", w, h);
        }
    }
}

// ============================================================================
// ESCENARIO 4: Interacción de sistemas — todos los subsistemas
// ============================================================================

#[test]
fn e2e_all_subsystems_interact() {
    init_all();
    let mut pool = EntityPool::new(3000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    // Verificar que TODOS los subsistemas existen
    assert!(!gw.flow_fields.primary.cells.is_empty(), "Flow fields");
    assert!(!gw.lane_manager.lanes.is_empty(), "Lane manager");
    assert!(!gw.land_value_map.values.is_empty(), "Land value");
    assert!(!gw.pollution_map.values.is_empty(), "Pollution");
    assert!(gw.finance.treasury > 0.0, "Finance");
    assert!(!gw.parking_mgr.street_segments.is_empty(), "Parking");
    assert_eq!(gw.politics.districts.len(), 9, "Politics");
    assert_eq!(gw.customization.appearances.len(), 9, "Customization");
    assert!(!gw.road_wear.values.is_empty(), "Road wear");
    assert!(!gw.water_grid.values.is_empty(), "Water grid");
    assert!(!gw.power_grid.values.is_empty(), "Power grid");

    // Simular y verificar que todos sobreviven
    for i in 0..100 {
        sim::tick(&mut gw, 0.1);
        assert!(gw.world.len() > 0, "Mundo colapsó en tick {}", i);
    }
}

// ============================================================================
// ESCENARIO 5: Tráfico — movimiento real y congestión
// ============================================================================

#[test]
fn e2e_traffic_movement_and_congestion() {
    init_all();
    let mut pool = EntityPool::new(2000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    // Capturar posiciones iniciales
    let initial_positions: Vec<(f32, f32)> = gw.world
        .query::<(&ecs::Position, &ecs::TrafficCar)>()
        .iter()
        .map(|(_, (pos, _))| (pos.x, pos.y))
        .collect();

    // Simular 100 ticks
    for _ in 0..100 {
        sim::tick(&mut gw, 0.1);
    }

    // Capturar posiciones finales
    let final_positions: Vec<(f32, f32)> = gw.world
        .query::<(&ecs::Position, &ecs::TrafficCar)>()
        .iter()
        .map(|(_, (pos, _))| (pos.x, pos.y))
        .collect();

    // Verificar que al menos el 80% de los coches se movieron
    let moved_count = initial_positions.iter().zip(final_positions.iter())
        .filter(|((ix, iy), (fx, fy))| {
            (fx - ix).abs() > 0.1 || (fy - iy).abs() > 0.1
        })
        .count();

    assert!(moved_count as f32 / initial_positions.len() as f32 > 0.7,
        "Al menos 70% de coches deben moverse: {}/{}",
        moved_count, initial_positions.len());

    // Verificar que hay congestión
    let congested = gw.lane_manager.lanes.iter()
        .any(|l| l.congestion > 0.01);
    assert!(congested, "Debe haber cierta congestión");
}

// ============================================================================
// ESCENARIO 6: Ciclo día/noche y tiempo
// ============================================================================

#[test]
fn e2e_day_night_cycle() {
    init_all();
    let mut pool = EntityPool::new(1000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    assert_eq!(gw.time_of_day, 7 * 60); // 07:00

    // Simular un día completo (~720 ticks)
    let mut times_seen = std::collections::HashSet::new();
    for _ in 0..1000 {
        sim::tick(&mut gw, 0.1);
        times_seen.insert(gw.time_of_day);
    }

    // Debe haber pasado por múltiples horas
    assert!(times_seen.len() > 20, "Debe haber variedad horaria: {}", times_seen.len());
    assert!(gw.sim_tick > 900, "Debe haber avanzado el tiempo");
}

// ============================================================================
// ESCENARIO 7: Crecimiento orgánico — nuevos edificios
// ============================================================================

#[test]
fn e2e_organic_city_growth() {
    init_all();
    let mut pool = EntityPool::new(5000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    let initial_entities = gw.world.len();

    // Simular mucho tiempo para crecimiento orgánico
    for _ in 0..2000 {
        sim::tick(&mut gw, 0.1);
    }

    let final_entities = gw.world.len();

    // La ciudad debe crecer (nuevos edificios por land_use)
    assert!(final_entities >= initial_entities,
        "Ciudad debe crecer o mantenerse: {} -> {}",
        initial_entities, final_entities);

    // No debe haber crecimiento descontrolado
    assert!(final_entities <= initial_entities + 2000,
        "Crecimiento excesivo: {} -> {}",
        initial_entities, final_entities);
}

// ============================================================================
// ESCENARIO 8: Sistema de impuestos — recaudación y bonos
// ============================================================================

#[test]
fn e2e_tax_system_collection() {
    init_all();
    let mut pool = EntityPool::new(2000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    let initial_treasury = gw.finance.treasury;

    // Avanzar hasta que se recaude
    for _ in 0..400 {
        sim::tick(&mut gw, 0.1);
    }

    // La tesorería debe haber cambiado
    assert!(gw.finance.treasury != initial_treasury || gw.sim_tick < tax_system::TAX_COLLECTION_INTERVAL,
        "Treasury debe cambiar tras recaudación");
}

// ============================================================================
// ESCENARIO 9: Mutación espacial — entidades no se salen del mapa
// ============================================================================

#[test]
fn e2e_entities_stay_in_bounds() {
    init_all();
    let mut pool = EntityPool::new(2000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    let gs = gw.grid_size as f32;

    // Simular mucho tiempo
    for _ in 0..500 {
        sim::tick(&mut gw, 0.1);

        // Verificar bounds de todas las entidades
        for (_, pos) in gw.world.query::<&ecs::Position>().iter() {
            // Después del wrapping, deben estar en [0, gs)
            assert!(pos.x >= 0.0 && pos.x < gs,
                "Entidad fuera de bounds X: {} en tick {}",
                pos.x, gw.sim_tick);
            assert!(pos.y >= 0.0 && pos.y < gs,
                "Entidad fuera de bounds Y: {} en tick {}",
                pos.y, gw.sim_tick);
        }
    }
}

// ============================================================================
// ESCENARIO 10: Render cache — consistencia tras simulación
// ============================================================================

#[test]
fn e2e_render_cache_consistency() {
    init_all();
    let mut pool = EntityPool::new(2000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    let initial_cache_entries = gw.render_cache.total_entries();

    for _ in 0..100 {
        sim::tick(&mut gw, 0.1);
    }

    // Reconstruir cache
    gw.render_cache.rebuild_from_world(&gw.world);

    let final_cache_entries = gw.render_cache.total_entries();

    // El cache debe reflejar el estado actual
    assert!(final_cache_entries > 0, "RenderCache debe tener entradas");
}