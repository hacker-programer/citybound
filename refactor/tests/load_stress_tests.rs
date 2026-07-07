#[cfg(test)]
mod load_stress {
    use rycimmu::ecs;
    use rycimmu::object_pool::EntityPool;
    use rycimmu::sim;
    use rycimmu::luts;
    use rycimmu::rng_pool;

    fn init() { luts::init_trig_luts(); rng_pool::init_rng_pool(42); }

    #[test]
    fn stress_1000_ticks_stable() {
        init();
        let mut pool = EntityPool::new(5000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);
        for i in 0..1000 {
            sim::tick(&mut gw, 0.1);
            if i % 100 == 0 {
                assert!(gw.world.len() > 0, "World empty at tick {}", i);
            }
        }
    }

    #[test]
    fn stress_many_entities_stable() {
        init();
        let mut pool = EntityPool::new(20000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);
        let count = gw.world.len();
        assert!(count > 0);
    }

    #[test]
    fn stress_rapid_ticks() {
        init();
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);
        let start = std::time::Instant::now();
        for _ in 0..500 {
            sim::tick(&mut gw, 0.1);
        }
        let elapsed = start.elapsed();
        assert!(elapsed.as_secs() < 30, "500 ticks took too long: {:?}", elapsed);
    }
}
