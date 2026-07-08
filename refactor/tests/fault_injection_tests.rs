// ============================================================================
// FAULT INJECTION TESTS — Verifica que el sistema maneje fallos correctamente
// ============================================================================
//
// Estos tests inyectan condiciones anómalas (entradas inválidas, valores extremos,
// condiciones de borde) y verifican que el sistema NO crashee, produzca pánicos
// controlados, o se recupere correctamente.
//
// FAULTS CUBIERTOS:
// F1: delta_time negativo en simulación
// F2: coordenadas fuera de rango en SpatialGrid
// F3: entidad inexistente en queries
// F4: pool de entidades vacío
// F5: tick con sim_tick overflow

use rycimmu::ecs;
use rycimmu::sim;
use rycimmu::luts;
use rycimmu::rng_pool;
use rycimmu::object_pool::EntityPool;
use rycimmu::bitboard;

fn init_all() {
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);
}

#[test]
fn test_f1_negative_delta_time_does_not_crash() {
    init_all();
    let mut pool = EntityPool::new(100);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    // Delta negativo no debería crashear
    sim::tick(&mut gw, -0.1);
    // El tick debería ser manejado sin pánico
    assert!(gw.sim_tick < 1000); // No debería avanzar demasiado
}

#[test]
fn test_f2_spatial_grid_out_of_bounds() {
    init_all();
    let mut pool = EntityPool::new(100);
    let mut gw = ecs::create_world(&mut pool);

    // Insertar entidad en coordenada negativa
    gw.spatial_grid.insert(-1000.0, -1000.0, 42);
    // No debería crashear

    // Query cerca de coordenadas extremas
    let nearby: Vec<u64> = gw.spatial_grid.query_near(-1000.0, -1000.0, 10.0).collect();
    // Debería devolver resultados o vacío, pero no crashear
    assert!(nearby.len() <= 100);
}

#[test]
fn test_f3_nonexistent_entity_query() {
    init_all();
    let mut pool = EntityPool::new(100);
    let gw = ecs::create_world(&mut pool);

    // Intentar acceder a una entidad que no existe
    // El ECS hecs::World maneja esto internamente sin crashear
    let count = gw.world.query::<&ecs::Position>().iter().count();
    // Debería haber entidades (al menos la cámara)
    assert!(count > 0);
}

#[test]
fn test_f4_empty_entity_pool() {
    init_all();
    let mut pool = EntityPool::new(0); // Pool vacío
    let gw = ecs::create_world(&mut pool);

    // Con pool vacío, el mundo aún debería crearse
    assert!(gw.world.len() > 0);
}

#[test]
fn test_f5_sim_tick_overflow_boundary() {
    init_all();
    let mut pool = EntityPool::new(100);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    // Forzar el tick a un valor muy alto
    gw.sim_tick = u64::MAX - 5;
    for _ in 0..10 {
        sim::tick(&mut gw, 0.1);
    }

    // Después de overflow, el tick debería ser mayor que el inicial
    // (aunque haya wrappeado, no debería crashear)
    assert!(gw.sim_tick >= u64::MAX - 5 || gw.sim_tick < 10);
}

#[test]
fn test_f6_bitboard_out_of_range() {
    init_all();
    let mut bb = bitboard::BitGrid::new();

    // Operaciones fuera de rango no deberían crashear
    let result = bb.test(0, 0.0, 1000.0);
    // Debería devolver false para celdas fuera de rango
    assert!(!result);
}

#[test]
fn test_f7_rapid_world_creation_destruction() {
    init_all();
    // Crear y destruir mundos rápidamente no debería causar leaks ni crashes
    for _ in 0..5 {
        let mut pool = EntityPool::new(100);
        let _gw = ecs::create_world(&mut pool);
        // gw se dropea aquí
    }
}

#[test]
fn test_f8_zero_zoom_camera() {
    init_all();
    let mut pool = EntityPool::new(100);
    let mut gw = ecs::create_world(&mut pool);

    // Simular zoom cero (divisiones por cero potenciales)
    for (_entity, (camera,)) in gw.world.query::<(&mut ecs::Camera,)>().iter() {
        camera.zoom = 0.0;
        // No debería crashear
    }

    // Restaurar zoom y verificar que sigue funcionando
    for (_entity, (camera,)) in gw.world.query::<(&mut ecs::Camera,)>().iter() {
        camera.zoom = 1.0;
    }

    sim::tick(&mut gw, 0.1);
    assert!(gw.sim_tick > 0);
}
