// ============================================================================
// LOAD & STRESS TESTS — Límites del Sistema con Carga Extrema
// ============================================================================
//
// Estos tests empujan el sistema a sus límites para encontrar:
// - Memory leaks
// - Degradación de rendimiento
// - Cuellos de botella
// - Condiciones de carrera
//
// NO deben ejecutarse en CI normal (son pesados). Usar:
//   cargo test --test load_stress_tests -- --ignored

use citybound_native::ecs;
use citybound_native::sim;
use citybound_native::luts;
use citybound_native::rng_pool;
use citybound_native::object_pool::EntityPool;
use citybound_native::render;

fn init_all() {
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);
}

// ============================================================================
// STRESS 1: 10,000 ticks sin degradación
// ============================================================================

#[test]
fn stress_10k_ticks_no_degradation() {
    init_all();
    let mut pool = EntityPool::new(10000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    let start = std::time::Instant::now();

    for i in 0..10_000 {
        sim::tick(&mut gw, 0.1);

        // Verificar cada 1000 ticks
        if i % 1000 == 0 {
            assert!(gw.world.len() > 0, "Mundo vacío en tick {}", i);
            assert!(gw.world.len() < 100_000, "Explosión en tick {}", i);
        }
    }

    let elapsed = start.elapsed();
    let ticks_per_sec = 10_000.0 / elapsed.as_secs_f64();
    println!("10k ticks: {:.1} ticks/s, {:.2}s", ticks_per_sec, elapsed.as_secs_f64());

    // Criterio: Al menos 500 ticks/segundo (aceptable para simulación)
    assert!(ticks_per_sec > 500.0,
        "Rendimiento insuficiente: {:.1} ticks/s", ticks_per_sec);
}

// ============================================================================
// STRESS 2: Pool con 100,000 entidades
// ============================================================================

#[test]
fn stress_massive_pool_100k() {
    let mut pool = EntityPool::new(100_000);
    assert_eq!(pool.capacity(), 100_000);

    // Adquirir todas
    let mut handles = Vec::with_capacity(100_000);
    for _ in 0..100_000 {
        handles.push(pool.acquire());
    }
    assert_eq!(pool.alive_count(), 100_000);
    assert_eq!(pool.free_count(), 0);

    // Liberar en orden inverso
    for h in handles.iter().rev() {
        pool.release(*h);
    }
    assert_eq!(pool.alive_count(), 0);
    assert_eq!(pool.free_count(), 100_000);
}

// ============================================================================
// STRESS 3: Renderizado a resoluciones extremas
// ============================================================================

#[test]
fn stress_extreme_resolutions() {
    init_all();
    let mut pool = EntityPool::new(2000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    for _ in 0..20 {
        sim::tick(&mut gw, 0.1);
    }

    // Resoluciones extremas
    let resolutions = [
        (3840, 2160), // 4K
        (7680, 4320), // 8K
        (320, 240),
        (1600, 1200),
    ];

    for &(w, h) in &resolutions {
        let start = std::time::Instant::now();
        let mut fb = vec![0xFF_1A_1A_2Eu32; w * h];
        render::render_world(&gw, &mut fb, w, h);
        let elapsed = start.elapsed();

        // 8K puede ser lento, pero no debe exceder 5 segundos
        assert!(elapsed.as_secs_f64() < 5.0,
            "Render {}x{} demasiado lento: {:.2}s", w, h, elapsed.as_secs_f64());
    }
}

// ============================================================================
// STRESS 4: Crear y destruir mundos repetidamente
// ============================================================================

#[test]
fn stress_create_destroy_worlds() {
    init_all();

    for i in 0..50 {
        let mut pool = EntityPool::new(2000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);

        for _ in 0..10 {
            sim::tick(&mut gw, 0.1);
        }

        // Verificar integridad
        assert!(gw.world.len() > 0, "Mundo {} vacío", i);
        // World se droppea al final de la iteración
    }
}

// ============================================================================
// STRESS 5: SpatialGrid con muchos queries
// ============================================================================

#[test]
fn stress_spatial_grid_many_queries() {
    init_all();
    let mut pool = EntityPool::new(5000);
    let gw = ecs::create_world(&mut pool);

    let start = std::time::Instant::now();
    let mut total_results = 0usize;

    for y in (0..128).step_by(4) {
        for x in (0..128).step_by(4) {
            let nearby: Vec<u64> = gw.spatial_grid.query_near(x as f32, y as f32, 10.0).collect();
            total_results += nearby.len();
        }
    }

    let elapsed = start.elapsed();
    assert!(elapsed.as_secs_f64() < 2.0,
        "SpatialGrid queries muy lentos: {:.2}s para {} resultados",
        elapsed.as_secs_f64(), total_results);
}

// ============================================================================
// STRESS 6: Save/Load con ciudades grandes
// ============================================================================

#[test]
fn stress_save_load_large_city() {
    init_all();
    let mut pool = EntityPool::new(20000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    // Hacer crecer la ciudad
    for _ in 0..500 {
        sim::tick(&mut gw, 0.1);
    }

    let entity_count = gw.world.len();

    let start = std::time::Instant::now();
    let save = citybound_native::persistence::SaveData::from_world(&gw);
    let encoded = bincode::serialize(&save).expect("Serializar");
    let save_time = start.elapsed();

    let start = std::time::Instant::now();
    let decoded: citybound_native::persistence::SaveData =
        bincode::deserialize(&encoded).expect("Deserializar");
    let load_time = start.elapsed();

    println!("City con {} entidades: save={:.2}ms, load={:.2}ms, size={}KB",
        entity_count,
        save_time.as_secs_f64() * 1000.0,
        load_time.as_secs_f64() * 1000.0,
        encoded.len() / 1024);

    // Save/load debe ser rápido
    assert!(save_time.as_secs_f64() < 1.0, "Save muy lento");
    assert!(load_time.as_secs_f64() < 0.5, "Load muy lento");
}

// ============================================================================
// STRESS 7: FlowField sampling masivo
// ============================================================================

#[test]
fn stress_flow_field_massive_sampling() {
    init_all();
    let mut pool = EntityPool::new(100);
    let gw = ecs::create_world(&mut pool);

    let start = std::time::Instant::now();
    let mut sum_magnitude: f32 = 0.0;

    for _ in 0..100_000 {
        let x = (rand::random::<f32>() * 128.0).abs();
        let y = (rand::random::<f32>() * 128.0).abs();
        let cell = gw.flow_fields.primary.sample(x, y);
        sum_magnitude += cell.magnitude;
    }

    let elapsed = start.elapsed();
    assert!(elapsed.as_secs_f64() < 0.5,
        "FlowField sampling muy lento: {:.2}s", elapsed.as_secs_f64());
    assert!(sum_magnitude > 0.0, "Magnitud total > 0");
}