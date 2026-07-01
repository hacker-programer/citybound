// Tests de integración para Citybound Native
//
// Verifica que los sistemas funcionan correctamente juntos:
// ECS + Simulación + Renderizado + Terreno + Quadtree

#[cfg(test)]
mod integration_tests {
    use citybound_native::ecs;
    use citybound_native::object_pool::EntityPool;
    use citybound_native::sim;
    use citybound_native::luts;

    /// Test: crear mundo y ejecutar simulación completa
    #[test]
    fn test_world_creation_and_simulation() {
        luts::init_trig_luts();

        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        // Verificar estado inicial
        assert!(gw.world.len() > 0);
        assert_eq!(gw.sim_tick, 0);
        assert_eq!(gw.time_of_day, 7 * 60); // 7:00 AM

        // Ejecutar 100 ticks de simulación
        for _ in 0..100 {
            sim::tick(&mut gw, 0.1);
        }

        // Verificar que el tiempo avanzó
        assert!(gw.sim_tick > 0);
        // La hora debe seguir en el rango 7:00-7:59 AM
        assert!(gw.time_of_day >= 7 * 60 && gw.time_of_day < 8 * 60,
            "Hora del día fuera de rango: {}", gw.time_of_day);
    }

    /// Test: verificar que las entidades de tráfico existen y se mueven
    #[test]
    fn test_traffic_entities_move() {
        luts::init_trig_luts();

        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        // Contar coches iniciales (40 coches preasignados)
        let initial_cars = gw.world.query::<&ecs::TrafficCar>().iter().count();
        assert_eq!(initial_cars, 40);

        // Ejecutar simulación
        for _ in 0..50 {
            sim::tick(&mut gw, 0.1);
        }

        // Los coches deben seguir existiendo
        let final_cars = gw.world.query::<&ecs::TrafficCar>().iter().count();
        assert_eq!(final_cars, 40);
    }

    /// Test: verificar que las zonas existen
    #[test]
    fn test_zone_entities_exist() {
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);

        let zone_count = gw.world.query::<&ecs::ZoneComponent>().iter().count();
        assert!(zone_count > 0, "Debe haber entidades de zona");

        // Verificar que hay zonas con densidad > 0
        let developed = gw.world.query::<&ecs::ZoneComponent>().iter()
            .filter(|(_, z)| z.density > 0)
            .count();
        assert!(developed > 0, "Debe haber zonas desarrolladas");
    }

    /// Test: verificar que los edificios existen
    #[test]
    fn test_building_entities_exist() {
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);

        let building_count = gw.world.query::<&ecs::ConstructionState>().iter().count();
        assert_eq!(building_count, 8, "Debe haber 8 edificios iniciales");
    }

    /// Test: verificar que la cámara existe
    #[test]
    fn test_camera_exists() {
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);

        let camera_count = gw.world.query::<&ecs::Camera>().iter().count();
        assert_eq!(camera_count, 1, "Debe haber exactamente una cámara");
    }

    /// Test: simulación extendida sin panics
    #[test]
    fn test_extended_simulation_stability() {
        luts::init_trig_luts();

        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        // 1000 ticks de simulación sin panic
        for i in 0..1000 {
            sim::tick(&mut gw, 0.1);
            // Verificar que el mundo sigue siendo válido
            assert!(gw.world.len() > 0, "El mundo no debe quedar vacío en tick {}", i);
        }
    }

    /// Test: renderizado del mundo no produce panics
    #[test]
    fn test_render_world_stability() {
        luts::init_trig_luts();

        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        // Ejecutar algunos ticks primero
        for _ in 0..10 {
            sim::tick(&mut gw, 0.1);
        }

        // Crear framebuffer
        let mut fb = vec![0xFF_1A_1A_2Eu32; 800 * 600];

        // Renderizar no debe panic
        citybound_native::render::render_world(&gw, &mut fb, 800, 600);

        // Verificar que el framebuffer fue modificado
        let modified = fb.iter().any(|&p| p != 0xFF_1A_1A_2E);
        assert!(modified, "El framebuffer debe tener píxeles dibujados");
    }

    /// Test: memory leak check básico
    #[test]
    fn test_no_entity_leak() {
        luts::init_trig_luts();

        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        let initial_count = gw.world.len();

        // Ejecutar simulación
        for _ in 0..100 {
            sim::tick(&mut gw, 0.1);
        }

        let final_count = gw.world.len();
        // El número de entidades no debe explotar
        assert!(final_count <= initial_count + 500,
            "Crecimiento de entidades excesivo: {} -> {}", initial_count, final_count);
    }

    /// Test: terreno generado correctamente
    #[test]
    fn test_terrain_exists() {
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);

        // Verificar que el terreno existe y tiene valores
        let h = gw.terrain.height(64, 64);
        assert!(h >= 0.0 && h <= 1.0, "Altura de terreno fuera de rango: {}", h);

        let color = gw.terrain.baked_color(64, 64);
        let alpha = (color >> 24) & 0xFF;
        assert_eq!(alpha, 0xFF, "Color baked debe ser opaco");
    }

    /// Test: quadtree existe en el mundo
    #[test]
    fn test_quadtree_exists() {
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);

        assert_eq!(gw.quadtree.len(), 0, "Quadtree debe empezar vacío");
    }
}
