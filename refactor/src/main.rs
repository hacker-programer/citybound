// Citybound Native v0.16.0 — Punto de entrada principal
//
// Game loop cross-platform con minifb (desktop) + platform.rs (Android)
// Simulación a 10 ticks/s, renderizado a 30 FPS objetivo
// FASE 8: Integración de TextureAtlas con spritesheets
//
// PLATAFORMAS: Windows, Android, macOS, Linux
//
// [FIX STACK OVERFLOW]: GameWorld se almacena como Box para evitar
// que los arrays masivos de sub-sistemas (TerrainMap 144KB, FlowField 128KB×8,
// UtilityGrids, etc.) desborden el stack de 1MB de Windows.

#![allow(unsafe_code)]

use citybound_native::*;
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
    println!("  Citybound Native v0.16.0 — City Builder Realista");
    println!("  Plataforma: {} | {}", platform::platform_name(), platform::arch_name());
    println!("  GPU API: {} | Tier: {:?} (nivel {})",
        gpu_api.name(), hw_tier, hw_tier as u8);
    println!("  Compute Shaders: {} | Max Textures: {}",
        if hw_tier.supports_compute_shaders() { "SI" } else { "NO" },
        hw_tier.max_texture_units());
    println!("══════════════════════════════════════════════════");

    // ===== VENTANA NATIVA =====
    let mut window = match minifb::Window::new(
        "Citybound Native v0.16.0",
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

    // ===== TEXTURE ATLAS (spritesheets) =====
    let mut atlas = texture_atlas::TextureAtlas::new();

    // Cargar Roguelike Modern City (edificios, carreteras)
    let city_path = std::path::Path::new("assets/textures/kenney/roguelike_modern_city/Spritesheet/roguelikeCity_transparent.png");
    match atlas.load_spritesheet(city_path, "roguelike_modern_city", 16, 16, 1) {
        Ok((start, count)) => println!("[OK] Roguelike Modern City: {} sprites (idx {}-{})", count, start, start + count - 1),
        Err(e) => println!("[WARN] No se pudo cargar Roguelike Modern City: {}", e),
    }

    // Cargar Tiny Town (edificios adicionales)
    let tiny_path = std::path::Path::new("assets/textures/kenney/tiny_town/tilemap_packed.png");
    match atlas.load_spritesheet(tiny_path, "tiny_town", 16, 16, 1) {
        Ok((start, count)) => println!("[OK] Tiny Town: {} sprites (idx {}-{})", count, start, start + count - 1),
        Err(e) => println!("[WARN] No se pudo cargar Tiny Town: {}", e),
    }

    // Cargar Pico-8 City (decoraciones)
    let pico_path = std::path::Path::new("assets/textures/kenney/pico8_city/tilemap_packed.png");
    match atlas.load_spritesheet(pico_path, "pico8_city", 8, 8, 1) {
        Ok((start, count)) => println!("[OK] Pico-8 City: {} sprites (idx {}-{})", count, start, start + count - 1),
        Err(e) => println!("[WARN] No se pudo cargar Pico-8 City: {}", e),
    }

    // Cargar LPC Terrain
    let lpc_path = std::path::Path::new("assets/textures/terrain/lpc/terrain.png");
    match atlas.load_spritesheet(lpc_path, "lpc_terrain", 32, 32, 0) {
        Ok((start, count)) => println!("[OK] LPC Terrain: {} sprites (idx {}-{})", count, start, start + count - 1),
        Err(e) => println!("[WARN] No se pudo cargar LPC Terrain: {}", e),
    }

    // Si no se cargaron assets, generar tiles procedurales como fallback
    if atlas.len() <= 1 {
        println!("[INFO] Sin assets externos. Generando texturas procedurales...");
        let start = atlas.len();
        for i in 0..8 {
            atlas.tiles.push(texture_atlas::generate_grass_tile(i));
        }
        atlas.tiles.push(texture_atlas::generate_road_tile());
        let building_colors = [
            0xFF_C4_7B_4A, 0xFF_B0_BEC5, 0xFF_26_C6_DA,
            0xFF_78_90_9C, 0xFF_8D_6E_63, 0xFF_8B_C3_4A,
            0xFF_F4_81_81, 0xFF_FF_D5_4F, 0xFF_42_45_E8,
        ];
        for &color in &building_colors {
            atlas.tiles.push(texture_atlas::generate_building_tile(color, 10));
        }
        println!("[OK] Generados {} tiles procedurales", atlas.len() - start);
    }

    println!("[OK] Atlas: {} tiles en {} banks", atlas.len(), atlas.banks.len());

    // ===== MUNDO (Box — en heap, NO en stack) =====
    let mut pool = object_pool::EntityPool::new(10000);
    let mut world = ecs::create_world(&mut pool);

    // Warm render cache
    world.render_cache.rebuild_from_world(&world.world);

    println!("[OK] Mundo creado: {} entidades", ecs::entity_count(&world));
    println!("[OK] GPU Backend: CPU SIMD (Tier {})", hw_tier as u8);
    println!("[OK] Iniciando game loop...");

    // ===== GAME LOOP =====
    let mut input_state = input::InputState::new();
    let mut last_time = Instant::now();
    let mut sim_accumulator: u64 = 0;
    let mut fps_timer = Instant::now();
    let mut frame_count: u64 = 0;

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

        // Reconstruir cache de render
        world.render_cache.rebuild_from_world(&world.world);

        // Render con texturas
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
    }

    println!("[OK] Saliendo limpiamente.");
}
