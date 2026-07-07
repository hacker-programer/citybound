// ============================================================================
// BENCHMARKS — Mediciones de Rendimiento con Criterion
// ============================================================================
//
// Uso: cargo bench
//
// Mide el rendimiento de operaciones críticas para detectar regresiones.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use citybound_native::ecs;
use citybound_native::sim;
use citybound_native::luts;
use citybound_native::rng_pool;
use citybound_native::object_pool::EntityPool;

fn init_all() {
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);
}

fn bench_full_tick(c: &mut Criterion) {
    init_all();
    let mut pool = EntityPool::new(5000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    c.bench_function("tick_100_entities", |b| {
        b.iter(|| {
            sim::tick(black_box(&mut gw), black_box(0.1));
        })
    });
}

fn bench_world_creation(c: &mut Criterion) {
    init_all();

    c.bench_function("create_world", |b| {
        b.iter(|| {
            let mut pool = EntityPool::new(2000);
            let _gw = ecs::create_world(black_box(&mut pool));
        })
    });
}

fn bench_render(c: &mut Criterion) {
    init_all();
    let mut pool = EntityPool::new(2000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    for _ in 0..20 {
        sim::tick(&mut gw, 0.1);
    }

    let mut fb = vec![0u32; 800 * 600];

    c.bench_function("render_800x600", |b| {
        b.iter(|| {
            citybound_native::render::render_world(
                black_box(&gw),
                black_box(&mut fb),
                black_box(800),
                black_box(600),
            );
        })
    });
}

fn bench_flow_field_sample(c: &mut Criterion) {
    init_all();
    let mut pool = EntityPool::new(100);
    let gw = ecs::create_world(&mut pool);

    c.bench_function("flow_field_sample_1000", |b| {
        b.iter(|| {
            for i in 0..1000 {
                let x = (i as f32 * 0.1) % 128.0;
                let y = (i as f32 * 0.13) % 128.0;
                black_box(gw.flow_fields.primary.sample(x, y));
            }
        })
    });
}

fn bench_object_pool(c: &mut Criterion) {
    c.bench_function("pool_acquire_release_1000", |b| {
        let mut pool = EntityPool::new(1000);
        b.iter(|| {
            for _ in 0..100 {
                let h = pool.acquire();
                if h.is_valid() {
                    pool.release(h);
                }
            }
        })
    });
}

criterion_group!(
    benches,
    bench_full_tick,
    bench_world_creation,
    bench_render,
    bench_flow_field_sample,
    bench_object_pool,
);
criterion_main!(benches);