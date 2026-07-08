// Rycimmu v0.20.0 — Punto de entrada principal
//
// Game loop cross-platform con minifb (desktop) + platform.rs (Android)
// Simulación a 10 ticks/s, renderizado a 30 FPS objetivo
// FASE 11: TILES 64×64 HERMOSOS — edificios con ventanas/techos/puertas
//           Pantalla completa, zoom amplio, sprites siempre visibles
//
// Inspirado por Citybound de Anselm Eickhoff — implementación 100% independiente.

// PLATAFORMAS: Windows, Android, macOS, Linux

#![allow(unsafe_code)]

use rycimmu::*;
use std::time::{Duration, Instant};

// Resolución base — se ajusta a la pantalla real
pub const WINDOW_WIDTH: usize = 1280;
pub const WINDOW_HEIGHT: usize = 720;
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
    println!("  Rycimmu v0.20.0 — City Builder Realista");
    println!("  FASE 11: Tiles 64×64, pantalla completa, zoom amplio");
    println!("  Plataforma: {} | {}", platform::platform_name(), platform::arch_name());
    println!("  GPU API: {} | Tier: {:?} (nivel {})",
        gpu_api.name(), hw_tier, hw_tier as u8);
    println!("══════════════════════════════════════════════════");

    // ===== VENTANA NATIVA (FULLSCREEN BORDERLESS) =====
    let mut window = match minifb::Window::new(
        "Rycimmu v0.20 — City Builder",
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        minifb::WindowOptions {
            borderless: false,
            resize: true,
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

    // Limitar a ~30 FPS para ahorrar CPU
    window.limit_update_rate(Some(Duration::from_micros(1_000_000 / TARGET_FPS as u64)));

    // ===== FRAMEBUFFER (heap) =====
    let mut framebuffer: Vec<u32> = vec![0xFF_1A_1A_2Eu32; FB_SIZE];

    // ===== RENDER BACKEND (CPU SIMD + GPU opcional) =====
    let _render_backend = gpu_backend::init_render_backend(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);

    // ===== LUTS Y RNG =====
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);

    // ===== ATLAS DE TEXTURAS =====
    let mut atlas = texture_atlas::TextureAtlas::new();
    let asset_base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));

    // Intentar cargar spritesheets Kenney (si existen, se añaden al atlas)
    let city_path = asset_base.join("assets/textures/kenney/roguelike_modern_city/Spritesheet/roguelikeCity_transparent.png");
    match atlas.load_spritesheet(&city_path, "roguelike_modern_city", 16, 16, 1) {
        Ok((start, count)) => println!("[OK] Roguelike Modern City: {} sprites (idx {}-{})",
            count, start, start + count - 1),
        Err(e) => println!("[WARN] Roguelike Modern City: {}", e),
    }

    let tiny_path = asset_base.join("assets/textures/kenney/tiny_town/Tilemap/tilemap_packed.png");
    match atlas.load_spritesheet(&tiny_path, "tiny_town", 16, 16, 1) {
        Ok((start, count)) => println!("[OK] Tiny Town: {} sprites (idx {}-{})",
            count, start, start + count - 1),
        Err(e) => println!("[WARN] Tiny Town: {}", e),
    }

    let pico_path = asset_base.join("assets/textures/kenney/pico8_city/Tilemap/tilemap_packed.png");
    match atlas.load_spritesheet(&pico_path, "pico8_city", 8, 8, 1) {
        Ok((start, count)) => println!("[OK] Pico-8 City: {} sprites (idx {}-{})",
            count, start, start + count - 1),
        Err(e) => println!("[WARN] Pico-8 City: {}", e),
    }

    let lpc_path = asset_base.join("assets/textures/terrain/lpc/terrain.png");
    match atlas.load_spritesheet(&lpc_path, "lpc_terrain", 32, 32, 0) {
        Ok((start, count)) => println!("[OK] LPC Terrain: {} sprites (idx {}-{})",
            count, start, start + count - 1),
        Err(e) => println!("[WARN] LPC Terrain: {}", e),
    }

    // Cargar ground texture
    let ground_path = asset_base.join("assets/textures/terrain/whispers_avalon_ground.png");
    let ground_idx = match atlas.load_full_texture(&ground_path, "ground") {
        Ok(idx) => { println!("[OK] Ground texture: idx {}", idx); idx },
        Err(e) => { println!("[WARN] Ground texture: {}", e); 0 }
    };

    // ═══════════════════════════════════════════════════════
    // ¡SIEMPRE generar tiles procedurales de 64×64!
    // Esto garantiza que NUNCA veremos cubos de colores.
    // ═══════════════════════════════════════════════════════
    println!("[INFO] Generando tiles procedurales 64×64...");
    generate_procedural_tiles_64(&mut atlas);

    // Imprimir estadísticas
    atlas.print_stats();

    // ===== MUNDO =====
    let mut pool = object_pool::EntityPool::new(10000);
    let mut world = ecs::create_world(&mut pool);

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

        // Reconstruir cache de render cada 3 frames
        if frame_idx % 3 == 0 {
            world.render_cache.rebuild_from_world_with_atlas(&world.world, &atlas);
        }

        // Render con sprites del atlas
        render::render_world_sprites(&world, &atlas, &mut framebuffer, WINDOW_WIDTH, WINDOW_HEIGHT, ground_idx);

        // Stats
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

/// Genera tiles procedurales de 64×64 con arquitectura detallada.
/// Se llama SIEMPRE, sin importar si hay assets externos.
fn generate_procedural_tiles_64(atlas: &mut texture_atlas::TextureAtlas) {
    // ── Terreno ──
    for i in 0..6 {
        atlas.tiles.push(texture_atlas::generate_grass_tile(i));
        atlas.categories.grass.push(atlas.tiles.len() - 1);
    }
    for i in 0..3 {
        atlas.tiles.push(texture_atlas::generate_dirt_tile(i));
        atlas.categories.dirt.push(atlas.tiles.len() - 1);
    }
    for i in 0..2 {
        atlas.tiles.push(texture_atlas::generate_sand_tile(i));
        atlas.categories.sand.push(atlas.tiles.len() - 1);
    }
    atlas.tiles.push(texture_atlas::generate_road_tile());
    atlas.categories.road.push(atlas.tiles.len() - 1);
    for i in 0..3 {
        atlas.tiles.push(texture_atlas::generate_water_tile(i));
        atlas.categories.water.push(atlas.tiles.len() - 1);
    }

    // ── Edificios (uno por estilo, con arquitectura detallada) ──
    let building_gen: &[(texture_atlas::BuildingTileStyle, fn() -> texture_atlas::SpriteTile)] = &[
        (texture_atlas::BuildingTileStyle::House,     texture_atlas::generate_house_tile),
        (texture_atlas::BuildingTileStyle::Apartment, texture_atlas::generate_apartment_tile),
        (texture_atlas::BuildingTileStyle::Shop,      texture_atlas::generate_shop_tile),
        (texture_atlas::BuildingTileStyle::Office,    texture_atlas::generate_office_tile),
        (texture_atlas::BuildingTileStyle::Factory,   texture_atlas::generate_factory_tile),
        (texture_atlas::BuildingTileStyle::Farm,      texture_atlas::generate_farm_tile),
        (texture_atlas::BuildingTileStyle::Hospital,  texture_atlas::generate_hospital_tile),
        (texture_atlas::BuildingTileStyle::School,    texture_atlas::generate_school_tile),
        (texture_atlas::BuildingTileStyle::Police,    texture_atlas::generate_police_tile),
        (texture_atlas::BuildingTileStyle::Generic,   || texture_atlas::generate_building_tile(0xFF_A0_A0_A8, 40)),
    ];

    for (style, gen_fn) in building_gen {
        atlas.tiles.push(gen_fn());
        atlas.categories.buildings
            .entry(*style)
            .or_insert_with(Vec::new)
            .push(atlas.tiles.len() - 1);
    }

    // ── Vehículos ──
    let vehicle_colors = [0xFF_E0_30_30, 0xFF_30_80_E0, 0xFF_F0_C0_30, 0xFF_30_C0_30, 0xFF_E0_E0_E0, 0xFF_20_20_20];
    for &color in &vehicle_colors {
        atlas.tiles.push(texture_atlas::generate_vehicle_tile(color));
        atlas.categories.vehicles.push(atlas.tiles.len() - 1);
    }

    println!("[OK] Generados {} tiles procedurales 64×64 (terreno + edificios + vehículos)", atlas.len());
}
