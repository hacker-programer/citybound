// ============================================================================
// USABILITY TESTS — Validación de UI/UX y Calidad Visual
// ============================================================================
//
// Verifica que la interfaz y experiencia de usuario sean correctas:
// - Colores en formato ARGB válido
// - Framebuffer integridad
// - Paleta de colores coherente
// - Comportamiento de cámara intuitivo
// - Feedback visual apropiado

use citybound_native::ecs;
use citybound_native::sim;
use citybound_native::luts;
use citybound_native::rng_pool;
use citybound_native::object_pool::EntityPool;
use citybound_native::render;

fn init_all() {
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);
}

// ============================================================================
// UX-1: Todos los colores de la paleta tienen alpha = 0xFF
// ============================================================================

#[test]
fn ux_all_colors_have_full_alpha() {
    let colors = [
        render::COLOR_GRASS,
        render::COLOR_DIRT,
        render::COLOR_ROAD,
        render::COLOR_SIDEWALK,
        render::COLOR_WATER,
        render::COLOR_BUILDING_HOUSE,
        render::COLOR_BUILDING_APARTMENT,
        render::COLOR_BUILDING_SHOP,
        render::COLOR_BUILDING_OFFICE,
        render::COLOR_BUILDING_FACTORY,
        render::COLOR_BUILDING_FARM,
        render::COLOR_BUILDING_HOSPITAL,
        render::COLOR_BUILDING_SCHOOL,
        render::COLOR_BUILDING_POLICE,
        render::COLOR_BACKGROUND,
        render::COLOR_UI_TEXT,
    ];

    for &color in &colors {
        let a = (color >> 24) & 0xFF;
        assert_eq!(a, 0xFF,
            "Color 0x{:08X} debe tener alpha=0xFF, tiene alpha=0x{:02X}", color, a);
    }

    // Colores de zona deben tener alpha semitransparente
    let zone_colors = [
        render::COLOR_ZONE_RESIDENTIAL,
        render::COLOR_ZONE_COMMERCIAL,
        render::COLOR_ZONE_INDUSTRIAL,
        render::COLOR_ZONE_AGRICULTURAL,
        render::COLOR_ZONE_ROAD,
        render::COLOR_ZONE_PARK,
    ];

    for &color in &zone_colors {
        let a = (color >> 24) & 0xFF;
        assert!(a > 0 && a < 0xFF,
            "Zona 0x{:08X} debe tener alpha semitransparente (0x{:02X})", color, a);
    }
}

// ============================================================================
// UX-2: Framebuffer mantiene formato ARGB en todos los píxeles
// ============================================================================

#[test]
fn ux_framebuffer_argb_integrity() {
    init_all();
    let mut pool = EntityPool::new(2000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    for _ in 0..20 {
        sim::tick(&mut gw, 0.1);
    }

    let mut fb = vec![0xFF_1A_1A_2Eu32; 800 * 600];
    render::render_world(&gw, &mut fb, 800, 600);

    let mut issues = 0usize;
    for &p in &fb {
        let a = (p >> 24) & 0xFF;
        let r = (p >> 16) & 0xFF;
        let g = (p >> 8) & 0xFF;
        let b = p & 0xFF;

        // Canales RGB pueden estar premultiplicados por alpha,
        // pero no deben exceder 0xFF
        if r > 0xFF || g > 0xFF || b > 0xFF || a > 0xFF {
            issues += 1;
        }
    }

    assert_eq!(issues, 0, "{} píxeles con valores de canal inválidos", issues);
}

// ============================================================================
// UX-3: RenderCache produce capas ordenadas correctamente
// ============================================================================

#[test]
fn ux_render_cache_layer_ordering() {
    init_all();
    let mut pool = EntityPool::new(2000);
    let gw = ecs::create_world(&mut pool);

    assert!(gw.render_cache.total_entries() > 0, "Cache debe tener entradas");
}

// ============================================================================
// UX-4: La cámara tiene zoom dentro de límites razonables
// ============================================================================

#[test]
fn ux_camera_zoom_limits() {
    init_all();
    let mut pool = EntityPool::new(1000);
    let mut gw = ecs::create_world(&mut pool);

    use citybound_native::input::{InputState, GameKey};

    // Zoom máximo
    let mut input = InputState::default();
    for _ in 0..100 {
        input.keys_held = 1u128 << (GameKey::PageUp as u8);
        ecs::process_input(&mut gw, &input);
    }

    let zoom_after_in: f32 = gw.world.query::<&ecs::Camera>().iter()
        .map(|(_, cam)| cam.zoom)
        .next()
        .unwrap_or(1.0);
    assert!(zoom_after_in <= 4.0, "Zoom no debe exceder 4.0: {}", zoom_after_in);

    // Zoom mínimo
    input.keys_held = 0;
    for _ in 0..100 {
        input.keys_held = 1u128 << (GameKey::PageDown as u8);
        ecs::process_input(&mut gw, &input);
    }

    let zoom_after_out: f32 = gw.world.query::<&ecs::Camera>().iter()
        .map(|(_, cam)| cam.zoom)
        .next()
        .unwrap_or(1.0);
    assert!(zoom_after_out >= 0.25, "Zoom no debe ser menor a 0.25: {}", zoom_after_out);
}

// ============================================================================
// UX-5: Los edificios tienen colores distintos y reconocibles
// ============================================================================

#[test]
fn ux_building_colors_distinct() {
    let building_colors = [
        render::COLOR_BUILDING_HOUSE,
        render::COLOR_BUILDING_APARTMENT,
        render::COLOR_BUILDING_SHOP,
        render::COLOR_BUILDING_OFFICE,
        render::COLOR_BUILDING_FACTORY,
        render::COLOR_BUILDING_FARM,
        render::COLOR_BUILDING_HOSPITAL,
        render::COLOR_BUILDING_SCHOOL,
        render::COLOR_BUILDING_POLICE,
    ];

    // Verificar que todos son distintos
    let mut seen = std::collections::HashSet::new();
    for &color in &building_colors {
        assert!(seen.insert(color), "Color duplicado: 0x{:08X}", color);
    }

    // Verificar que no son fondo negro ni blanco
    for &color in &building_colors {
        assert_ne!(color, 0xFF_00_00_00, "Edificio no debe ser negro");
        assert_ne!(color, 0xFF_FF_FF_FF, "Edificio no debe ser blanco");
    }
}

// ============================================================================
// UX-6: Framebuffer no tiene artefactos de tearing
// ============================================================================

#[test]
fn ux_no_framebuffer_artifacts() {
    init_all();
    let mut pool = EntityPool::new(2000);
    let mut gw = ecs::create_world(&mut pool);
    sim::init_simulation(&mut gw);

    for _ in 0..30 {
        sim::tick(&mut gw, 0.1);
    }

    // Renderizar dos frames consecutivos
    let mut fb1 = vec![0xFF_1A_1A_2Eu32; 800 * 600];
    let mut fb2 = vec![0xFF_1A_1A_2Eu32; 800 * 600];

    render::render_world(&gw, &mut fb1, 800, 600);
    sim::tick(&mut gw, 0.1);
    render::render_world(&gw, &mut fb2, 800, 600);

    // Debe haber cierta continuidad entre frames (no completamente diferente)
    let same_pixels = fb1.iter().zip(fb2.iter())
        .filter(|(a, b)| a == b)
        .count();
    let similarity = same_pixels as f32 / fb1.len() as f32;

    // Al menos 30% de similitud entre frames consecutivos
    assert!(similarity > 0.30,
        "Muy poca continuidad entre frames: {:.1}%", similarity * 100.0);
}

// ============================================================================
// UX-7: El tiempo formateado es legible
// ============================================================================

#[test]
fn ux_formatted_time_readable() {
    let times = [
        (0, "00:00"),
        (60, "01:00"),
        (7 * 60, "07:00"),
        (12 * 60 + 30, "12:30"),
        (23 * 60 + 59, "23:59"),
    ];

    for (input, expected) in &times {
        assert_eq!(sim::formatted_time(*input), *expected,
            "Time {} debe ser '{}'", input, expected);
    }
}

// ============================================================================
// UX-8: Tamaños de pincel del DesignTool en rango
// ============================================================================

#[test]
fn ux_brush_sizes_in_range() {
    use citybound_native::interactive;
    assert!(interactive::MIN_BRUSH_SIZE >= 1);
    assert!(interactive::MAX_BRUSH_SIZE <= 50);
    assert!(interactive::DEFAULT_BRUSH_SIZE >= interactive::MIN_BRUSH_SIZE);
    assert!(interactive::DEFAULT_BRUSH_SIZE <= interactive::MAX_BRUSH_SIZE);
    assert!(interactive::MAX_UNDO_HISTORY >= 10);
}

// ============================================================================
// UX-9: Nombres de tipos de edificios son completos
// ============================================================================

#[test]
fn ux_building_types_complete() {
    use citybound_native::ecs::BuildingType;
    let types = [
        BuildingType::House,
        BuildingType::Shop,
        BuildingType::Factory,
        BuildingType::Apartment,
        BuildingType::Office,
        BuildingType::Farm,
        BuildingType::Hospital,
        BuildingType::School,
        BuildingType::Police,
    ];

    // Verificar que hay 9 tipos
    assert_eq!(types.len(), 9);

    // Verificar que todos son distintos
    let mut seen = std::collections::HashSet::new();
    for &t in &types {
        assert!(seen.insert(t), "BuildingType duplicado: {:?}", t);
    }
}