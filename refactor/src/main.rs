// Citybound Native v0.15.0 — Punto de entrada principal
//
// Game loop cross-platform con minifb (desktop) + platform.rs (Android)
// Simulación a 10 ticks/s, renderizado a 30 FPS objetivo
//
// PLATAFORMAS: Windows, Android, macOS, Linux
//
// [FIX STACK OVERFLOW]: GameWorld se almacena como Box para evitar
// que los arrays masivos de sub-sistemas (TerrainMap 144KB, FlowField 128KB×8,
// UtilityGrids, etc.) desborden el stack de 1MB de Windows.
//
// [DIAGNÓSTICO STATUS_STACK_BUFFER_OVERRUN]:
// Temporalmente desactivado windows_subsystem para ver output de consola.
// Stack size: 32MB

#![allow(unsafe_code)]
// #![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // DIAGNÓSTICO: desactivado

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
        let payload = info.payload();
        let msg = if let Some(s) = payload.downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };
        let location = info.location().map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown location".to_string());
        eprintln!("FATAL PANIC at {}: {}", location, msg);
        std::process::abort();
    }));

    eprintln!("[DIAG] Iniciando...");

    // ===== DETECCIÓN DE HARDWARE =====
    eprintln!("[DIAG] Detectando hardware...");
    let hw_tier = gpu_backend::HardwareTier::current();
    let gpu_api = gpu_backend::GpuApi::detect();

    eprintln!("══════════════════════════════════════════════════");
    eprintln!("  Citybound Native v0.15.0 — City Builder Realista");
    eprintln!("  Plataforma: {} | {}", platform::platform_name(), platform::arch_name());
    eprintln!("  GPU API: {} | Tier: {:?} (nivel {})",
        gpu_api.name(), hw_tier, hw_tier as u8);
    eprintln!("  Compute Shaders: {} | Max Textures: {}",
        if hw_tier.supports_compute_shaders() { "SI" } else { "NO" },
        hw_tier.max_texture_units());
    eprintln!("══════════════════════════════════════════════════");

    // ===== VENTANA NATIVA =====
    eprintln!("[DIAG] Creando ventana...");
    let mut window = match minifb::Window::new(
        "Citybound Native v0.15.0",
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
    eprintln!("[DIAG] Asignando framebuffer {} píxeles...", FB_SIZE);
    let mut framebuffer: Vec<u32> = vec![0xFF_00_00_00u32; FB_SIZE];

    // ===== LUTS Y RNG =====
    eprintln!("[DIAG] Inicializando LUTs trigonométricas...");
    luts::init_trig_luts();
    eprintln!("[DIAG] Inicializando RNG pool...");
    rng_pool::init_rng_pool(42);

    // ===== MUNDO (Box — en heap, NO en stack) =====
    eprintln!("[DIAG] Creando entity pool...");
    let mut pool = object_pool::EntityPool::new(10000);
    eprintln!("[DIAG] Creando mundo (esto puede tomar un momento)...");
    let mut world = ecs::create_world(&mut pool);
    eprintln!("[DIAG] Mundo creado: {} entidades", ecs::entity_count(&world));

    // ===== RENDER BACKEND (CPU SIMD + GPU opcional) =====
    eprintln!("[DIAG] Inicializando render backend...");
    let _render_backend = gpu_backend::init_render_backend(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);
    eprintln!("[DIAG] Render backend listo.");

    // ===== INPUT =====
    let mut input_state = input::InputState::default();

    // ===== VARIABLES DE CONTROL =====
    let mut sim_accumulator: u64 = 0;
    let mut last_time = Instant::now();
    let mut frame_count: u64 = 0;
    let mut fps_timer = Instant::now();

    // Warm render cache
    eprintln!("[DIAG] Reconstruyendo render cache...");
    world.render_cache.rebuild_from_world(&world.world);
    eprintln!("[DIAG] Render cache: {} entradas", world.render_cache.total_entries());

    eprintln!("[OK] Inicialización completa. Entrando al game loop...");

    // ===== GAME LOOP =====
    while window.is_open() && !window.is_key_down(minifb::Key::Escape) {
        let now = Instant::now();
        let dt = now.duration_since(last_time).as_micros() as u64;
        last_time = now;

        sim_accumulator += dt;

        // Capturar input
        input_state.update(&window);

        // Simulación a paso fijo (10 ticks/s)
        while sim_accumulator >= MICROS_PER_TICK {
            sim_accumulator -= MICROS_PER_TICK;
            sim::tick(&mut world);
        }

        // Reconstruir cache de render si dirty
        world.render_cache.rebuild_from_world(&world.world);

        // Render
        render::render_world_cached(&world, &mut framebuffer, WINDOW_WIDTH, WINDOW_HEIGHT);

        // Stats panel
        render::render_stats_panel(&world, &mut framebuffer, WINDOW_WIDTH, WINDOW_HEIGHT, TARGET_FPS);

        // Update window
        window.update_with_buffer(&framebuffer, WINDOW_WIDTH, WINDOW_HEIGHT).ok();

        // FPS counter
        frame_count += 1;
        if fps_timer.elapsed() >= Duration::from_secs(1) {
            let _fps = frame_count;
            frame_count = 0;
            fps_timer = Instant::now();
        }
    }

    eprintln!("[OK] Saliendo limpiamente.");
}
