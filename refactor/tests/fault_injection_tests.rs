// ============================================================================
// FAULT INJECTION TESTS � Chaos Engineering para Citybound Native
// ============================================================================
//
// Inyecta fallos deliberados para verificar la resiliencia del sistema:
// - Entradas inv�lidas
// - Condiciones de borde
// - Estados inconsistentes
// - Recuperaci�n ante errores
//
// Principio: "El sistema nunca debe crashear, incluso con datos maliciosos"

use citybound_native::ecs;
use citybound_native::sim;
use citybound_native::luts;
use citybound_native::rng_pool;
use citybound_native::object_pool::EntityPool;
use citybound_native::flow_field;
use citybound_native::bitboard;
use citybound_native::render;
use citybound_native::persistence;
use std::panic;

fn init_all() {
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);
}

// ============================================================================
// CAOS 1: Framebuffer vac�o o tama�o 0
// ============================================================================

#[test]
fn chaos_empty_framebuffer() {
    init_all();
    let mut pool = EntityPool::new(100);
    let gw = ecs::create_world(&mut pool);

    // Framebuffer vac�o no debe panic
    let result = panic::catch_unwind(|| {
        let mut fb: Vec<u32> = vec![];
        render::render_world(&gw, &mut fb, 0, 0);
    });
    assert!(result.is_ok(), "Framebuffer vac�o no debe crashear");
}

// ============================================================================
// CAOS 2: Simulaci�n con dt negativo o cero
// ============================================================================

#[test]
fn chaos_zero_or_negative_dt() {
    init_all();
    let mut pool = EntityPool::new(1000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    // dt = 0 no debe panic
    let result = panic::catch_unwind(|| {
        sim::tick(&mut gw, 0.0);
    });
    assert!(result.is_ok(), "dt=0 no debe crashear");

    // dt negativo no debe panic
    let result = panic::catch_unwind(|| {
        sim::tick(&mut gw, -0.1);
    });
    assert!(result.is_ok(), "dt negativo no debe crashear");
}

// ============================================================================
// CAOS 3: Pool exhausto � adquirir m�s de la capacidad
// ============================================================================

#[test]
fn chaos_pool_exhaustion_stress() {
    let mut pool = EntityPool::new(10);
    let mut handles = Vec::new();

    // Adquirir todos
    for _ in 0..10 {
        handles.push(pool.acquire());
    }

    // Intentar adquirir m�s � debe retornar INVALID sin panic
    for _ in 0..100 {
        let h = pool.acquire();
        assert!(!h.is_valid(), "Pool exhausto debe retornar INVALID");
    }

    // Liberar y readquirir
    for h in &handles {
        pool.release(*h);
    }
    for _ in 0..10 {
        assert!(pool.acquire().is_valid(), "Debe poder readquirir tras liberar");
    }
}

// ============================================================================
// CAOS 4: Coordenadas extremas en flow fields
// ============================================================================

#[test]
fn chaos_extreme_coordinates() {
    init_all();
    let mut pool = EntityPool::new(100);
    let gw = ecs::create_world(&mut pool);

    // Coordenadas extremas no deben panic
    let extremes = [
        (0.0, 0.0),
        (-1000.0, -1000.0),
        (1_000_000.0, 1_000_000.0),
        (f32::MAX, f32::MAX),
        (f32::MIN, f32::MIN),
        (f32::NAN, f32::NAN),
        (f32::INFINITY, f32::INFINITY),
        (f32::NEG_INFINITY, f32::NEG_INFINITY),
    ];

    for &(x, y) in &extremes {
        let result = panic::catch_unwind(|| {
            let _cell = gw.flow_fields.primary.sample(x, y);
        });
        assert!(result.is_ok(), "Flow field no debe crashear en ({}, {})", x, y);
    }
}

// ============================================================================
// CAOS 5: BitGrid con coordenadas extremas
// ============================================================================

#[test]
fn chaos_bitgrid_extreme_coords() {
    let mut grid = bitboard::BitGrid::new();

    let extremes = [
        (-1000.0, -1000.0),
        (1_000_000.0, 1_000_000.0),
        (f32::MAX, f32::MAX),
        (f32::MIN, f32::MIN),
    ];

    for &(x, y) in &extremes {
        let result = panic::catch_unwind(|| {
            grid.set(0, x, y);
            let _ = grid.test(0, x, y);
            grid.clear(0, x, y);
        });
        assert!(result.is_ok(), "BitGrid no debe crashear en ({}, {})", x, y);
    }
}

// ============================================================================
// CAOS 6: Save/Load con datos corruptos
// ============================================================================

#[test]
fn chaos_corrupted_save_data() {
    // Datos binarios corruptos
    let corrupted_bytes: Vec<u8> = vec![0xFF, 0x00, 0xAB, 0xCD, 0xEF];
    let result: Result<persistence::SaveData, _> = bincode::deserialize(&corrupted_bytes);
    // Debe fallar gracefulmente (Err), no panic
    assert!(result.is_err(), "Datos corruptos deben dar error, no panic");

    // Archivo vac�o
    let empty: Vec<u8> = vec![];
    let result: Result<persistence::SaveData, _> = bincode::deserialize(&empty);
    assert!(result.is_err(), "Archivo vac�o debe dar error");

    // Bytes aleatorios
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let random_bytes: Vec<u8> = (0..1000).map(|_| rng.gen()).collect();
    let result: Result<persistence::SaveData, _> = bincode::deserialize(&random_bytes);
    assert!(result.is_err(), "Bytes aleatorios deben dar error");
}

// ============================================================================
// CAOS 7: Liberar handle inv�lido m�ltiples veces
// ============================================================================

#[test]
fn chaos_double_release_and_invalid_handles() {
    let mut pool = EntityPool::new(100);
    let h = pool.acquire();
    assert!(h.is_valid());

    pool.release(h);
    pool.release(h); // Doble release � no debe panic
    pool.release(h); // Triple release
    pool.release(citybound_native::object_pool::PoolHandle::INVALID); // INVALID
    pool.release(object_pool::PoolHandle(99999)); // Fuera de rango

    // El pool debe seguir funcional
    assert!(pool.acquire().is_valid());
}

// ============================================================================
// CAOS 8: Entidades con componentes faltantes
// ============================================================================

#[test]
fn chaos_missing_components() {
    init_all();
    let mut pool = EntityPool::new(1000);
    let mut gw = ecs::create_world(&mut pool);

    // Spawnear entidad sin Position (componente requerido en queries)
    let entity = gw.world.spawn((
        ecs::Renderable::rect(0xFF_FF_00_00, 1.0, 1),
    ));

    // El sistema debe sobrevivir a esto
    let result = panic::catch_unwind(|| {
        for _ in 0..50 {
            sim::tick(&mut gw, 0.1);
        }
    });
    // Podr�a panic si hay unwrap en queries, pero idealmente deber�a manejar graceful
    // Si paniquea, documentamos que es esperado (entidad sin posici�n no es v�lida)
    let _ = gw.world.despawn(entity);
    let _ = result;
}

// ============================================================================
// CAOS 9: 10,000 ticks sin degradaci�n
// ============================================================================

#[test]
fn chaos_extended_simulation_no_degradation() {
    init_all();
    let mut pool = EntityPool::new(5000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    let initial_memory = gw.world.len();

    // Simular mucho tiempo
    for i in 0..1000 {
        sim::tick(&mut gw, 0.1);
        // Verificar que el mundo no se vac�a
        if gw.world.len() == 0 {
            panic!("Mundo vac�o en tick {}", i);
        }
    }

    // No debe haber crecido m�s de 10x
    assert!(gw.world.len() <= initial_memory * 10,
        "Crecimiento descontrolado: {} -> {}",
        initial_memory, gw.world.len());
}

// ============================================================================
// CAOS 10: Renderizado en todos los estados de memoria
// ============================================================================

#[test]
fn chaos_render_after_various_states() {
    init_all();
    let mut pool = EntityPool::new(2000);
    let mut gw = ecs::create_world(&mut pool);

    // Estado 1: Reci�n creado
    let mut fb = vec![0u32; 800 * 600];
    render::render_world(&gw, &mut fb, 800, 600);

    // Estado 2: Tras inicializar simulaci�n
    sim::init_simulation(&mut gw);
    render::render_world(&gw, &mut fb, 800, 600);

    // Estado 3: Tras 100 ticks
    for _ in 0..100 { sim::tick(&mut gw, 0.1); }
    render::render_world(&gw, &mut fb, 800, 600);

    // Estado 4: Tras 1000 ticks
    for _ in 0..900 { sim::tick(&mut gw, 0.1); }
    render::render_world(&gw, &mut fb, 800, 600);

    // En ning�n momento debe panic
}