// Tests de integración para Citybound Native v0.5.0
//
// Verifica que los sistemas funcionan correctamente juntos.

#[cfg(test)]
mod integration_tests {
    use citybound_native::ecs;
    use citybound_native::object_pool::EntityPool;
    use citybound_native::sim;
    use citybound_native::luts;
    use citybound_native::rng_pool;

    fn init_all_systems() {
        luts::init_trig_luts();
        rng_pool::init_rng_pool(42);
    }

    #[test]
    fn test_world_creation_and_simulation() {
        init_all_systems();
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);

        assert!(gw.world.len() > 0);
        assert_eq!(gw.sim_tick, 0);
        assert_eq!(gw.time_of_day, 7 * 60);

        for _ in 0..100 {
            sim::tick(&mut gw, 0.1);
        }
        assert!(gw.sim_tick > 0);
    }

    #[test]
    fn test_traffic_entities_exist_and_survive() {
        init_all_systems();
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);

        let initial = gw.world.query::<&ecs::TrafficCar>().iter().count();
        assert_eq!(initial, 40);

        for _ in 0..50 {
            sim::tick(&mut gw, 0.1);
        }
        assert_eq!(gw.world.query::<&ecs::TrafficCar>().iter().count(), 40);
    }

    #[test]
    fn test_zone_entities_exist() {
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);
        let count = gw.world.query::<&ecs::ZoneComponent>().iter().count();
        assert!(count > 0);
    }

    #[test]
Ahora actualizo el test de integración que espera 8 edificios (ahora son 11):
    fn test_camera_exists() {
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);
        assert_eq!(gw.world.query::<&ecs::Camera>().iter().count(), 1);
    }

    #[test]
    fn test_extended_simulation_stability() {
        init_all_systems();
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);

        for i in 0..1000 {
            sim::tick(&mut gw, 0.1);
            assert!(gw.world.len() > 0, "Mundo vacío en tick {}", i);
        }
    }

    #[test]
    fn test_render_world_stability() {
        init_all_systems();
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);

        for _ in 0..10 {
            sim::tick(&mut gw, 0.1);
        }

        let mut fb = vec![0xFF_1A_1A_2Eu32; 800 * 600];
        citybound_native::render::render_world(&gw, &mut fb, 800, 600);

        let modified = fb.iter().any(|&p| p != 0xFF_1A_1A_2E);
        assert!(modified, "Framebuffer debe tener píxeles dibujados");
    }

    #[test]
    fn test_no_entity_leak() {
        init_all_systems();
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);

        let initial = gw.world.len();
        for _ in 0..100 {
            sim::tick(&mut gw, 0.1);
        }
        let final_count = gw.world.len();
        assert!(final_count <= initial + 500,
            "Crecimiento excesivo: {} -> {}", initial, final_count);
    }

    #[test]
    fn test_terrain_exists() {
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);
        let h = gw.terrain.height(64, 64);
        assert!(h >= 0.0 && h <= 1.0);
        let color = gw.terrain.baked_color(64, 64);
        assert_eq!((color >> 24) & 0xFF, 0xFF);
    }

    #[test]
    fn test_flow_fields_exist() {
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);
        let cell = gw.flow_fields.primary.sample(64.0, 64.0);
        assert!(cell.magnitude >= 0.0 && cell.magnitude <= 1.0);
        let hwy = gw.flow_fields.highway.sample(64.0, 64.0);
        assert!(hwy.magnitude > 0.3, "Autopista debe tener flujo");
    }

    #[test]
    fn test_bitgrid_after_init() {
        init_all_systems();
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);
        assert!(gw.bitgrid.count_layer(0) > 0, "Debe haber obstáculos");
    }

    #[test]
    fn test_full_render_pipeline_multiple_sizes() {
        init_all_systems();
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);

✅ **Compila con 4 warnings.** Ahora comienzo a arreglar los bugs uno por uno.

---

## 🔧 FASE 1: Arreglando bugs críticos

### B1 — `integration_tests.rs`: Método inexistente `height()`
    }

    #[test]
    fn test_traffic_cars_change_position() {
        init_all_systems();
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        sim::init_simulation(&mut gw);

        // Get initial positions of cars
        let initial: Vec<(f32, f32)> = gw.world
            .query::<(&ecs::Position, &ecs::TrafficCar)>()
            .iter()
            .map(|(_, (pos, _))| (pos.x, pos.y))
            .collect();

        // Run several ticks
        for _ in 0..30 {
            sim::tick(&mut gw, 0.1);
        }

        // Get final positions
        let final_pos: Vec<(f32, f32)> = gw.world
            .query::<(&ecs::Position, &ecs::TrafficCar)>()
            .iter()
            .map(|(_, (pos, _))| (pos.x, pos.y))
            .collect();

        // At least some cars should have moved
        let moved = initial.iter().zip(final_pos.iter())
            .any(|((ix, iy), (fx, fy))| {
                (fx - ix).abs() > 0.01 || (fy - iy).abs() > 0.01
            });

        // Cars should move with flow fields (at least most of them)
        assert!(moved || initial.len() == 0,
            "Cars should move with flow fields enabled");
    }
}
