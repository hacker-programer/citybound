// Tests unitarios para Rycimmu
//
// Cubre todos los mÃ³dulos:
// - luts (trigonometrÃ­a)
// - object_pool
// - bump_alloc
// - ecs (componentes, world)
// - sim (tiempo, trÃ¡fico, economÃ­a)
// - render (dibujo, paleta)

#[cfg(test)]
mod tests {
    // Los tests estÃ¡n en cada mÃ³dulo respectivo con #[cfg(test)]
    // Este archivo ejecuta tests de integraciÃ³n adicionales

    #[test]
    fn test_full_pipeline() {
        // Verificar que todo el pipeline funciona junto
        // (Este es un smoke test de integraciÃ³n)
        let result = std::panic::catch_unwind(|| {
            // Inicializar LUTs
            rycimmu::luts::init_trig_luts();

            // Crear object pool
            let mut pool = rycimmu::object_pool::EntityPool::new(100);

            // Adquirir y liberar entidades
            let h = pool.acquire();
            assert!(h.is_valid());
            pool.release(h);

            // Verificar bump allocator
            let bump = rycimmu::bump_alloc::BumpAllocator::new();
            bump.reset();
        });

        assert!(result.is_ok(), "Full pipeline debe ejecutarse sin panic");
    }

    #[test]
    fn test_luts_integration() {
        rycimmu::luts::init_trig_luts();

        // Verificar identidad fundamental en varios Ã¡ngulos
        for i in 0..36 {
            let angle = (i as f32) * std::f32::consts::PI / 18.0;
            let sin_val = rycimmu::luts::sin_fast(angle);
            let cos_val = rycimmu::luts::cos_fast(angle);
            let identity = sin_val * sin_val + cos_val * cos_val;
            assert!((identity - 1.0).abs() < 0.01,
                "sinÂ²+cosÂ² debe ser 1, fue {} en Ã¡ngulo {}", identity, angle);
        }
    }

    #[test]
    fn test_object_pool_stress() {
        let mut pool = rycimmu::object_pool::EntityPool::new(10000);

        // Adquirir todas
        let mut handles = Vec::with_capacity(10000);
        for _ in 0..10000 {
            handles.push(pool.acquire());
        }

        // Debe estar lleno
        assert_eq!(pool.acquire(), rycimmu::object_pool::PoolHandle::INVALID);

        // Liberar la mitad
        for i in 0..5000 {
            pool.release(handles[i]);
        }

        // Readquirir 5000
        for _ in 0..5000 {
            assert!(pool.acquire().is_valid());
        }

        // Lleno otra vez
        assert_eq!(pool.acquire(), rycimmu::object_pool::PoolHandle::INVALID);
    }

    #[test]
    fn test_sim_time_consistency() {
        use rycimmu::sim;

        assert_eq!(sim::formatted_time(0), "00:00");
        assert_eq!(sim::formatted_time(7 * 60), "07:00");
        assert_eq!(sim::formatted_time(12 * 60 + 30), "12:30");
        assert_eq!(sim::formatted_time(23 * 60 + 59), "23:59");
    }

    #[test]
    fn test_render_colors() {
        use rycimmu::render;
        // Verificar que los colores son ARGB vÃ¡lidos
        let colors = [
            render::COLOR_GRASS,
            render::COLOR_DIRT,
            render::COLOR_CAR,
            render::COLOR_WATER,
        ];
        for color in &colors {
            let a = (color >> 24) & 0xFF;
            assert_eq!(a, 0xFF, "Alpha debe ser 0xFF para colores opacos");
        }
    }
}
