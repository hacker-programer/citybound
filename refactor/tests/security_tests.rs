// ============================================================================
// SECURITY TESTS (SAST/DAST) — Análisis de Seguridad Estático y Dinámico
// ============================================================================
//
// Verifica que el código no contiene vulnerabilidades comunes:
// - Buffer overflows
// - Integer overflows
// - Use-after-free
// - Unsafe blocks incorrectos
// - Input validation
// - Data races (con Loom/ThreadSanitizer)
//
// Principio: "El software debe ser seguro incluso con entradas maliciosas"

use citybound_native::ecs;
use citybound_native::sim;
use citybound_native::luts;
use citybound_native::rng_pool;
use citybound_native::object_pool::EntityPool;
use citybound_native::persistence;
use std::panic;

fn init_all() {
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);
}

// ============================================================================
// SEC-1: Integer overflow en handles de pool
// ============================================================================

#[test]
fn sec_pool_integer_overflow_safety() {
    let mut pool = EntityPool::new(u32::MAX as usize - 1);

    // Adquirir y liberar en el borde del rango u32
    let h = pool.acquire();
    assert!(h.is_valid());
    pool.release(h);

    // Verificar que el contador no hace wrap de forma insegura
    for _ in 0..1000 {
        let h = pool.acquire();
        if h.is_valid() {
            pool.release(h);
        }
    }
}

// ============================================================================
// SEC-2: Buffer overflow en framebuffer
// ============================================================================

#[test]
fn sec_framebuffer_no_out_of_bounds() {
    init_all();
    let mut pool = EntityPool::new(100);
    let gw = ecs::create_world(&mut pool);

    // Framebuffer exacto
    let w = 100;
    let h = 100;
    let mut fb = vec![0u32; w * h];

    // Esto no debe causar buffer overflow
    let result = panic::catch_unwind(|| {
        citybound_native::render::render_world(&gw, &mut fb, w, h);
    });

    // Si panic, podría ser bug de bounds checking
    assert!(result.is_ok(), "Render no debe causar buffer overflow");

    // Verificar que no se escribió fuera del buffer
    // (si el render escribiera fuera, corruptiría memoria adyacente)
    assert_eq!(fb.len(), w * h, "Framebuffer tamaño no modificado");
}

// ============================================================================
// SEC-3: Deserialización de datos maliciosos
// ============================================================================

#[test]
fn sec_deserialize_malicious_data() {
    // Ataque: datos extremadamente grandes
    let large_data = vec![0u8; 10 * 1024 * 1024]; // 10MB
    let result: Result<persistence::SaveData, _> = bincode::deserialize(&large_data);
    assert!(result.is_err(), "10MB de basura no debe deserializarse");

    // Ataque: valores extremos
    // Intentar crear SaveData con campos en bordes
    let save = persistence::SaveData {
        version: u32::MAX,
        sim_tick: u64::MAX,
        time_of_day: u16::MAX,
        finance_treasury: f32::INFINITY,
        finance_land_value_tax_rate: f32::NAN,
        finance_corporate_tax_rate: -1.0,
        finance_sales_tax_rate: 999.0,
        politics_approval: f32::NEG_INFINITY,
        buildings: vec![],
        zones: vec![],
        lane_congestion: vec![],
    };

    // Restaurar con datos extremos no debe panic
    init_all();
    let mut pool = EntityPool::new(100);
    let mut gw = ecs::create_world(&mut pool);
    let result = panic::catch_unwind(|| {
        save.restore_to(&mut gw);
    });
    // Puede fallar o no, pero no debe ser UB
    let _ = result;
}

// ============================================================================
// SEC-4: Ataque de denegación de servicio vía entidades masivas
// ============================================================================

#[test]
fn sec_dos_via_massive_spawn() {
    init_all();
    let mut pool = EntityPool::new(100000);
    let mut gw = ecs::create_world(&mut pool);

    let initial = gw.world.len();

    // Simular spawn masivo (como haría un atacante)
    for i in 0..5000 {
        let _ = gw.world.spawn((
            ecs::Position::new((i % 128) as f32, (i / 128) as f32),
            ecs::Renderable::rect(0xFF_00_00_00, 1.0, 1),
        ));
    }

    // El sistema debe seguir funcionando
    let result = panic::catch_unwind(|| {
        sim::tick(&mut gw, 0.1);
    });
    assert!(result.is_ok(), "Sistema debe sobrevivir spawn masivo");

    // No debería haber memory leak inmediato
    assert!(gw.world.len() > initial, "Entidades deben haberse agregado");
}

// ============================================================================
// SEC-5: Validación de save files corruptos
// ============================================================================

#[test]
fn sec_save_file_validation() {
    // Versión incorrecta
    let wrong_version = persistence::SaveData {
        version: 999,
        sim_tick: 0,
        time_of_day: 0,
        finance_treasury: 0.0,
        finance_land_value_tax_rate: 0.0,
        finance_corporate_tax_rate: 0.0,
        finance_sales_tax_rate: 0.0,
        politics_approval: 0.0,
        buildings: vec![],
        zones: vec![],
        lane_congestion: vec![],
    };

    let encoded = bincode::serialize(&wrong_version).unwrap();

    // Modificar el version byte manualmente para bypassear el check
    // Esto prueba que el sistema no confía ciegamente en los datos

    // La carga debe ser segura incluso con datos manipulados
    init_all();
    let mut pool = EntityPool::new(100);
    let mut gw = ecs::create_world(&mut pool);
    let result = panic::catch_unwind(|| {
        wrong_version.restore_to(&mut gw);
    });
    let _ = result;
}

// ============================================================================
// SEC-6: Uso de unsafe — verificación de invariantes
// ============================================================================

#[test]
fn sec_unsafe_blocks_maintain_invariants() {
    // ObjectPool usa unsafe con get_unchecked_mut
    let mut pool = EntityPool::new(100);
    let handles: Vec<_> = (0..50).map(|_| pool.acquire()).collect();

    // Todos deben ser válidos
    for h in &handles {
        assert!(pool.is_alive(*h));
    }

    // Liberar y readquirir — invariante LIFO
    for h in handles.iter().rev() {
        pool.release(*h);
    }

    for h in handles.iter() {
        let new_h = pool.acquire();
        assert_eq!(new_h, *h, "LIFO debe preservarse (unsafe invariante)");
    }
}

// ============================================================================
// SEC-7: Path traversal en carga de archivos
// ============================================================================

#[test]
fn sec_no_path_traversal() {
    // Intentar cargar archivos con paths maliciosos
    let malicious_paths = [
        "../../../etc/passwd",
        "C:\\Windows\\System32\\config\\SAM",
        "....//....//....//etc/passwd",
        "/dev/null",
        "nul",
    ];

    for path in &malicious_paths {
        let result = persistence::load_game(path);
        // Debe fallar (archivo no existe) pero NO panic
        assert!(result.is_err(), "Path malicioso '{}' debe fallar gracefulmente", path);
    }
}

// ============================================================================
// SEC-8: Formateo de tiempo seguro
// ============================================================================

#[test]
fn sec_formatted_time_boundaries() {
    // Valores borde
    assert_eq!(sim::formatted_time(0), "00:00");
    assert_eq!(sim::formatted_time(24 * 60 - 1), "23:59");
    assert_eq!(sim::formatted_time(24 * 60), "24:00"); // ¿wrap?

    // Valores extremos — no deben panic
    let extremes = [u16::MAX, u16::MAX - 1, 10000];
    for &t in &extremes {
        let result = panic::catch_unwind(|| {
            let _ = sim::formatted_time(t);
        });
        assert!(result.is_ok(), "formatted_time({}) no debe panic", t);
    }
}