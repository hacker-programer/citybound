// Rycimmu v0.18.0 — Punto de entrada principal
//
// Game loop cross-platform con minifb (desktop) + platform.rs (Android)
// Simulación a 10 ticks/s, renderizado a 30 FPS objetivo
// FASE 9: Terreno con tiles, edificios con sprites reales, vehículos texturizados
//
//
// Inspirado por Citybound de Anselm Eickhoff — implementación 100% independiente.

// PLATAFORMAS: Windows, Android, macOS, Linux

#![allow(unsafe_code)]

use rycimmu::*;
use std::time::{Duration, Instant};

pub const WINDOW_WIDTH: usize = 800;
pub const WINDOW_HEIGHT: usize = 600;
pub const FB_SIZE: usize = WINDOW_WIDTH * WINDOW_HEIGHT;
pub const SIM_TICKS_PER_SECOND: u32 = 10;
pub const MICROS_PER_TICK: u64 = 1_000_000 / SIM_TICKS_PER_SECOND as u64;
pub const TARGET_FPS: u32 = 30;

fn main() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("FATAL: {}", info);
        std::process::abort();
    }));

    // ===== DETECCIÓN DE HARDWARE =====
    let hw_tier = gpu_backend::HardwareTier::current();
    let gpu_api = gpu_backend::GpuApi::detect();

    println!("══════════════════════════════════════════════════");
    println!("  Rycimmu v0.18.0 — City Builder Realista");
    println!("  FASE 9: Texturas reales, terreno tileado, sprites");
    println!("  Plataforma: {} | {}", platform::platform_name(), platform::arch_name());
    println!("  GPU API: {} | Tier: {:?} (nivel {})",
        gpu_api.name(), hw_tier, hw_tier as u8);
    println!("══════════════════════════════════════════════════");

    // ===== VENTANA NATIVA =====
    let mut window = match minifb::Window::new(
        "Rycimmu v0.18.0 — Fase 9: Texturas",
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        minifb::WindowOptions {
            resize: false,
            scale: minifb::Scale::X1,
            ..Default::default()
        },
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[ERR] No se pudo abrir ventana: {}", e);
            return;
        }
    };

    window.set_target_fps(TARGET_FPS as usize);

    // ===== FRAMEBUFFER (heap) =====
    let mut framebuffer: Vec<u32> = vec![0xFF_00_00_00u32; FB_SIZE];

    // ===== RENDER BACKEND (CPU SIMD + GPU opcional) =====
    let _render_backend = gpu_backend::init_render_backend(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);

    // ===== LUTS Y RNG =====
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);
    // ===== TEXTURE ATLAS (spritesheets con categorización) =====
    let mut atlas = texture_atlas::TextureAtlas::new();

    // Directorio base de assets (absoluto, resuelto en compilación)
    let asset_base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    // Cargar Roguelike Modern City (edificios, terreno, carreteras, vehículos)
    let city_path = asset_base.join("assets/textures/kenney/roguelike_modern_city/Spritesheet/roguelikeCity_transparent.png");
    match atlas.load_spritesheet(&city_path, "roguelike_modern_city", 16, 16, 1) {
        Ok((start, count)) => println!("[OK] Roguelike Modern City: {} sprites (idx {}-{})",
            count, start, start + count - 1),
        Err(e) => println!("[WARN] No se pudo cargar Roguelike Modern City: {}", e),
    }

    // Cargar Tiny Town (más edificios)
    let tiny_path = asset_base.join("assets/textures/kenney/tiny_town/Tilemap/tilemap_packed.png");
    match atlas.load_spritesheet(&tiny_path, "tiny_town", 16, 16, 1) {
        Ok((start, count)) => println!("[OK] Tiny Town: {} sprites (idx {}-{})",
            count, start, start + count - 1),
        Err(e) => println!("[WARN] No se pudo cargar Tiny Town: {}", e),
    }

    // Cargar Pico-8 City (decoraciones, mini-edificios)
    let pico_path = asset_base.join("assets/textures/kenney/pico8_city/Tilemap/tilemap_packed.png");
    match atlas.load_spritesheet(&pico_path, "pico8_city", 8, 8, 1) {
        Ok((start, count)) => println!("[OK] Pico-8 City: {} sprites (idx {}-{})",
            count, start, start + count - 1),
        Err(e) => println!("[WARN] No se pudo cargar Pico-8 City: {}", e),
    }

    // Cargar LPC Terrain
    let lpc_path = asset_base.join("assets/textures/terrain/lpc/terrain.png");
    match atlas.load_spritesheet(&lpc_path, "lpc_terrain", 32, 32, 0) {
        Ok((start, count)) => println!("[OK] LPC Terrain: {} sprites (idx {}-{})",
            count, start, start + count - 1),
        Err(e) => println!("[WARN] No se pudo cargar LPC Terrain: {}", e),
    }

    // Si no se cargaron assets, generar tiles procedurales como fallback
    // Si no se cargaron assets, generar tiles procedurales como fallback
    if atlas.len() <= 1 {
        println!("[INFO] Sin assets externos. Generando texturas procedurales...");

        // Terreno: hierba (8 variantes)
        for i in 0..8 {
            atlas.tiles.push(texture_atlas::generate_grass_tile(i));
            atlas.categories.grass.push(atlas.tiles.len() - 1);
        }
        // Tierra (4 variantes)
        for i in 0..4 {
            atlas.tiles.push(texture_atlas::generate_dirt_tile(i));
            atlas.categories.dirt.push(atlas.tiles.len() - 1);
        }
        // Arena (2 variantes)
        for i in 0..2 {
            atlas.tiles.push(texture_atlas::generate_sand_tile(i));
            atlas.categories.sand.push(atlas.tiles.len() - 1);
        }
        // Carretera
        atlas.tiles.push(texture_atlas::generate_road_tile());
        atlas.categories.road.push(atlas.tiles.len() - 1);
        // Agua (3 frames de animación)
        for i in 0..3 {
            atlas.tiles.push(texture_atlas::generate_water_tile(i));
            atlas.categories.water.push(atlas.tiles.len() - 1);
        }

        // Edificios: un tile por cada estilo
        let building_styles = [
            (texture_atlas::BuildingTileStyle::House,     0xFF_C4_8E_6A), // terracota
            (texture_atlas::BuildingTileStyle::Apartment, 0xFF_A8_A8_B0), // gris
            (texture_atlas::BuildingTileStyle::Shop,      0xFF_5C_A0_B8), // azul comercio
            (texture_atlas::BuildingTileStyle::Office,    0xFF_8A_9B_A8), // gris azulado
            (texture_atlas::BuildingTileStyle::Factory,   0xFF_8A_7A_6E), // marrón industrial
            (texture_atlas::BuildingTileStyle::Farm,      0xFF_8C_A8_6A), // verde rural
            (texture_atlas::BuildingTileStyle::Hospital,  0xFF_E8_E8_F0), // blanco
            (texture_atlas::BuildingTileStyle::School,    0xFF_E8_D8_8C), // amarillo
            (texture_atlas::BuildingTileStyle::Police,    0xFF_5C_70_C4), // azul policial
        ];
        for (style, color) in &building_styles {
            atlas.tiles.push(texture_atlas::generate_building_tile(*color, 10));
            atlas.categories.buildings
                .entry(*style)
                .or_insert_with(Vec::new)
                .push(atlas.tiles.len() - 1);
        }

        // Vehículos (4 colores)
        let vehicle_colors = [0xFF_E0_30_30, 0xFF_30_80_E0, 0xFF_F0_C0_30, 0xFF_30_C0_30];
        for &color in &vehicle_colors {
            atlas.tiles.push(texture_atlas::generate_vehicle_tile(color));
            atlas.categories.vehicles.push(atlas.tiles.len() - 1);
        }

        println!("[OK] Generados {} tiles procedurales (terreno + edificios + vehículos)", atlas.len() - 1);
    }
    // ===== MUNDO (Box — en heap, NO en stack) =====
    let mut pool = object_pool::EntityPool::new(10000);
    let mut world = ecs::create_world(&mut pool);

    // Warm render cache con atlas
    world.render_cache.rebuild_from_world_with_atlas(&world.world, &atlas);

    println!("[OK] Mundo creado: {} entidades", ecs::entity_count(&world));
    println!("[OK] GPU Backend: CPU SIMD (Tier {})", hw_tier as u8);
    println!("[OK] Iniciando game loop...");

    // ===== GAME LOOP =====
    let mut input_state = input::InputState::new();
    let mut last_time = Instant::now();
    let mut sim_accumulator: u64 = 0;
    let mut fps_timer = Instant::now();
    let mut frame_count: u64 = 0;
    let mut frame_idx: u64 = 0;

    while window.is_open() && !window.is_key_down(minifb::Key::Escape) {
        let now = Instant::now();
        let dt = now.duration_since(last_time).as_micros() as u64;
        last_time = now;
        sim_accumulator += dt;

        // Capturar input
        input_state.update(&window);

        // Procesar input para diseño y cámara
        ecs::process_input(&mut world, &input_state);

        // Simulación a paso fijo (10 ticks/s)
        while sim_accumulator >= MICROS_PER_TICK {
            sim_accumulator -= MICROS_PER_TICK;
            sim::tick(&mut world, 1.0 / SIM_TICKS_PER_SECOND as f32);
        }

        // Reconstruir cache de render cada 3 frames (ahorra CPU)
        if frame_idx % 3 == 0 {
            world.render_cache.rebuild_from_world_with_atlas(&world.world, &atlas);
        }

        // Render con texturas reales
        render::render_world_cached(&world, &atlas, &mut framebuffer, WINDOW_WIDTH, WINDOW_HEIGHT);

        // Stats panel
        let current_fps = if fps_timer.elapsed() >= Duration::from_secs(1) {
            let fps = frame_count;
            frame_count = 0;
            fps_timer = Instant::now();
            fps
        } else {
            TARGET_FPS as u64
        };
        render::render_stats_panel(&world, &mut framebuffer, WINDOW_WIDTH, WINDOW_HEIGHT, current_fps as u32);

        // Update window
        window.update_with_buffer(&framebuffer, WINDOW_WIDTH, WINDOW_HEIGHT).ok();

        frame_count += 1;
        frame_idx += 1;
    }

    println!("[OK] Saliendo limpiamente.");
}
