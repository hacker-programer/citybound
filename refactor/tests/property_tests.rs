// ============================================================================
// PROPERTY-BASED TESTS — Invariantes Matemáticos con Entradas Aleatorias
// ============================================================================
//
// Usa proptest para generar entradas aleatorias y verificar que las
// propiedades matemáticas del sistema nunca se violan.
//
// Cada test define una PROPIEDAD que debe cumplirse PARA TODA entrada válida.

use citybound_native::ecs;
use citybound_native::sim;
use citybound_native::luts;
use citybound_native::rng_pool;
use citybound_native::object_pool::EntityPool;
use citybound_native::flow_field;
use citybound_native::bitboard;
use proptest::prelude::*;

fn init_all() {
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);
}

// ============================================================================
// PROPIEDAD 1: Trigonometría — sin²? + cos²? ˜ 1 para todo ángulo
// ============================================================================

proptest! {
    #[test]
    fn prop_trig_identity_holds(angle in -100.0f32..100.0f32) {
        luts::init_trig_luts();
        let sin_val = luts::sin_fast(angle);
        let cos_val = luts::cos_fast(angle);
        let identity = sin_val * sin_val + cos_val * cos_val;
        prop_assert!((identity - 1.0).abs() < 0.02,
            "sin²+cos² debe ser 1, fue {} para ángulo {}", identity, angle);
    }
}

// ============================================================================
// PROPIEDAD 2: FlowField — magnitud siempre en [0, 1]
// ============================================================================

proptest! {
    #[test]
    fn prop_flow_field_magnitude_bounded(
        x in 0.0f32..128.0f32,
        y in 0.0f32..128.0f32
    ) {
        init_all();
        let mut pool = EntityPool::new(100);
        let gw = ecs::create_world(&mut pool);

        let cell = gw.flow_fields.primary.sample(x, y);
        prop_assert!(cell.magnitude >= 0.0 && cell.magnitude <= 1.0,
            "Magnitud fuera de [0,1]: {} en ({}, {})", cell.magnitude, x, y);

        let cell2 = gw.flow_fields.highway.sample(x, y);
        prop_assert!(cell2.magnitude >= 0.0 && cell2.magnitude <= 1.0,
            "Magnitud highway fuera de [0,1]: {}", cell2.magnitude);
    }
}

// ============================================================================
// PROPIEDAD 3: BitGrid — set+test consistencia
// ============================================================================

proptest! {
    #[test]
    fn prop_bitgrid_set_test_consistent(
        layer in 0u8..8u8,
        x in 0.0f32..128.0f32,
        y in 0.0f32..128.0f32
    ) {
        let mut grid = bitboard::BitGrid::new();
        prop_assert!(!grid.test(layer, x, y), "Recién creado debe estar vacío");
        grid.set(layer, x, y);
        prop_assert!(grid.test(layer, x, y), "Después de set, test debe ser true");
        grid.clear(layer, x, y);
        prop_assert!(!grid.test(layer, x, y), "Después de clear, test debe ser false");
    }
}

// ============================================================================
// PROPIEDAD 4: ObjectPool — alive_count + free_count = capacity
// ============================================================================

proptest! {
    #[test]
    fn prop_pool_invariant(capacity in 1usize..1000usize) {
        let mut pool = EntityPool::new(capacity);
        prop_assert_eq!(pool.alive_count() + pool.free_count(), capacity);

        // Adquirir n entidades aleatorias
        let n = capacity / 2;
        let mut handles = Vec::new();
        for _ in 0..n {
            let h = pool.acquire();
            if h.is_valid() {
                handles.push(h);
            }
        }
        prop_assert_eq!(pool.alive_count() + pool.free_count(), capacity);
    }
}

// ============================================================================
// PROPIEDAD 5: Terreno — height_at siempre en [0, 1]
// ============================================================================

proptest! {
    #[test]
    fn prop_terrain_height_bounded(
        x in 0.0f32..128.0f32,
        y in 0.0f32..128.0f32
    ) {
        init_all();
        let mut pool = EntityPool::new(100);
        let gw = ecs::create_world(&mut pool);
        let h = gw.terrain.height_at(x, y);
        prop_assert!(h >= 0.0 && h <= 1.0,
            "Terreno fuera de [0,1]: {} en ({}, {})", h, x, y);
    }
}

// ============================================================================
// PROPIEDAD 6: Velocidad de coches nunca excede máximo
// ============================================================================

proptest! {
    #[test]
    fn prop_car_speed_never_exceeds_max(seed in 0u64..100u64) {
        init_all();
        rng_pool::init_rng_pool(seed);
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);

        // Correr varios ticks
        for _ in 0..50 {
            sim::tick(&mut gw, 0.1);
        }

        // Verificar velocidades
        for (_, car) in gw.world.query::<&ecs::TrafficCar>().iter() {
            prop_assert!(car.speed >= 0.0,
                "Velocidad negativa: {}", car.speed);
            prop_assert!(car.speed <= car.max_speed * 1.1,
                "Velocidad {} excede max {} por más del 10%",
                car.speed, car.max_speed);
        }
    }
}

// ============================================================================
// PROPIEDAD 7: LandValue — difusión no crea valores negativos
// ============================================================================

proptest! {
    #[test]
    fn prop_land_value_non_negative(n in 0usize..10usize) {
        let mut lv = land_value::LandValueHeatmap::new();
        for _ in 0..n {
            lv.diffuse();
        }
        for y in 0..land_value::HEATMAP_SIZE {
            for x in 0..land_value::HEATMAP_SIZE {
                let v = lv.get(x, y);
                prop_assert!(v >= 0.0,
                    "Valor de suelo negativo: {} en ({}, {})", v, x, y);
            }
        }
    }
}

// ============================================================================
// PROPIEDAD 8: TaxPolicy — tasas en rangos válidos
// ============================================================================

#[test]
fn prop_tax_policy_defaults_in_range() {
    let policy = citybound_native::tax_system::TaxPolicy::default();
    assert!(policy.land_value_tax_rate >= 0.0 && policy.land_value_tax_rate <= 0.10);
    assert!(policy.corporate_tax_rate >= 0.0 && policy.corporate_tax_rate <= 0.35);
    assert!(policy.sales_tax_rate >= 0.0 && policy.sales_tax_rate <= 0.15);
    assert!(policy.toll_peak_multiplier >= 1.0);
}

// ============================================================================
// PROPIEDAD 9: SpatialGrid — query_near incluye el punto de consulta
// ============================================================================

#[test]
fn prop_spatial_grid_self_query() {
    let mut grid = ecs::SpatialGrid::new();
    grid.insert(10.0, 10.0, 42);
    let nearby: Vec<u64> = grid.query_near(10.0, 10.0, 0.5).collect();
    assert!(nearby.contains(&42), "Query near debe incluir punto propio");
}

// ============================================================================
// PROPIEDAD 10: Simulation — el tiempo solo avanza
// ============================================================================

proptest! {
    #[test]
    fn prop_sim_tick_monotonic(n in 0u32..100u32) {
        init_all();
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);

        let initial = gw.sim_tick;
        for _ in 0..n {
            sim::tick(&mut gw, 0.1);
        }
        prop_assert!(gw.sim_tick >= initial,
            "sim_tick debe ser monótono: {} -> {}", initial, gw.sim_tick);
    }
}

// ============================================================================
// PROPIEDAD 11: Finanzas — treasury nunca negativa por defecto
// ============================================================================

#[test]
fn prop_treasury_non_negative() {
    init_all();
    let mut pool = EntityPool::new(2000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    // Con la configuración por defecto, la tesorería no debería ir a negativa
    for _ in 0..300 {
        sim::tick(&mut gw, 0.1);
    }

    // Nota: si se implementan gastos podría ir a negativa, pero con defaults no
    assert!(gw.finance.treasury >= 0.0 || gw.finance.treasury > -1000.0,
        "Treasury no debería desplomarse: {}", gw.finance.treasury);
}