mod e2e_tests {
    use rycimmu::ecs;
    use rycimmu::object_pool::EntityPool;
    use rycimmu::sim;
    use rycimmu::luts;
    use rycimmu::rng_pool;
    use rycimmu::persistence::SaveData;
    use tempfile::NamedTempFile;

    fn init() { luts::init_trig_luts(); rng_pool::init_rng_pool(42); }

    #[test]
    fn e2e_full_city_lifecycle() {
        init();
        let mut pool = EntityPool::new(2000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);
        let count = gw.world.query::<&ecs::ConstructionState>().iter().count();
        assert!(count >= 8, "City should have at least 8 buildings, got {}", count);
        for _ in 0..10 { sim::tick(&mut gw, 0.1); }
        assert!(gw.world.len() > 0, "World should not be empty after simulation");
    }

    #[test]
    fn e2e_terrain_exists() {
        init();
        let mut pool = EntityPool::new(100);
        let gw = ecs::create_world(&mut pool);
        let h = gw.terrain.height_at(64.0, 64.0);
        assert!(h >= 0.0 && h <= 1.0);
    }

    #[test]
    fn e2e_finance_initialized() {
        init();
        let mut pool = EntityPool::new(100);
        let gw = ecs::create_world(&mut pool);
        assert!(gw.finance.treasury >= 0.0);
        assert!(gw.finance.credit_rating >= 0.0 && gw.finance.credit_rating <= 1.0);
    }

    #[test]
    fn e2e_save_load_roundtrip() {
        init();
        let mut pool = EntityPool::new(500);
        let gw = ecs::create_world(&mut pool);
        let data = SaveData::from_world(&gw);
        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        let result = rycimmu::persistence::save_game(&data, path);
        assert!(result.is_ok(), "Save should succeed");
    }

    #[test]
    fn e2e_multiple_ticks_stable() {
        init();
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);
        for i in 0..50 {
            sim::tick(&mut gw, 0.1);
            assert!(gw.world.len() > 0, "World died at tick {}", i);
        }
    }
}