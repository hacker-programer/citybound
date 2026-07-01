// Citybound Native Refactor - Punto de entrada principal
//
// ARQUITECTURA:
// - ECS puro con hecs (Entity Component System)
// - Framebuffer software rendering (minifb)
// - Sistemas: Tiempo, Tráfico, Economía, Zonas, Render
//
// OPTIMIZACIONES APLICADAS (referencias a las 90 técnicas):
// [TC#1]  Object Pooling Masivo: 10,000 entidades preasignadas
// [TC#2]  Pre-reserva de capacidad en Vec::with_capacity
// [TC#5]  LUTs trigonométricas precalculadas
// [TA#2]  LTO + PGO en release
// [TA#9]  Structs alineados a 64 bytes para caché L1
// [TA#15] Uso exclusivo de f32
// [TA#16] Inlining agresivo
// [TA#20] Bump allocators por frame
// [TI#1]  Autómatas finitos compilados para estados
// [TI#6]  Bitboards para colisiones en grilla

// Todo el código unsafe está documentado con comentarios // SAFETY:
#![allow(unsafe_code)]
#![cfg_attr(not(test), windows_subsystem = "windows")]

mod ecs;
mod sim;
mod render;
mod object_pool;
mod luts;
mod input;
mod bump_alloc;

use minifb::{Key, Window, WindowOptions, Scale, ScaleMode};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// CONSTANTES DE CONFIGURACIÓN (precalculadas en compilación)
// ---------------------------------------------------------------------------

/// Dimensiones de la ventana - 800x600 es óptimo para Pentium 4GB
pub const WINDOW_WIDTH: usize = 800;
pub const WINDOW_HEIGHT: usize = 600;
/// Framebuffer: ARGB empaquetado en u32 para escritura directa
pub const FB_SIZE: usize = WINDOW_WIDTH * WINDOW_HEIGHT;

/// Ticks de simulación por segundo (paso fijo)
pub const SIM_TICKS_PER_SECOND: u32 = 10;
/// Duración de un tick en microsegundos
pub const MICROS_PER_TICK: u64 = 1_000_000 / SIM_TICKS_PER_SECOND as u64;

/// Frame rate objetivo (30 FPS suficiente para simulación)
pub const TARGET_FPS: u32 = 30;
/// Duración de un frame en microsegundos
pub const MICROS_PER_FRAME: u64 = 1_000_000 / TARGET_FPS as u64;

// ---------------------------------------------------------------------------
// PUNTO DE ENTRADA
// ---------------------------------------------------------------------------
fn main() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("FATAL: {}", info);
        std::process::abort();
    }));

    println!("Citybound Native v0.4.0 - ECS City Simulator");
    println!("Optimizado para Pentium 4GB RAM / 2 cores");

    // [TA#20] Inicializar Bump Allocator global (16MB preasignados)
    bump_alloc::init_frame_allocator();
    println!("  Bump Allocator: {}MB listos", bump_alloc::frame_allocator().free() / (1024 * 1024));

    // [TC#5] Inicializar LUTs trigonométricas
    luts::init_trig_luts();
    println!("  LUTs trigonométricas: {} entradas ({}KB)",
        luts::TRIG_RESOLUTION, luts::TRIG_RESOLUTION * 2 * 4 / 1024);

    // [TC#1] Inicializar Object Pool
    let mut pool = object_pool::EntityPool::new(10_000);
    println!("  Object Pool: {} slots preasignados", pool.capacity());

    // Inicializar ECS World
    let mut world = ecs::create_world(&mut pool);

    // Inicializar sistemas de simulación
    sim::init_simulation(&mut world);

    // [TC#2] Crear ventana con framebuffer
    let mut window = Window::new(
        "Citybound Native - ECS Simulator (ESC para salir)",
        WINDOW_WIDTH,
        WINDOW_HEIGHT,
        WindowOptions {
            scale: Scale::X2,
            scale_mode: ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    )
    .expect("No se pudo crear la ventana. Drivers de video requeridos.");

    window.set_target_fps(TARGET_FPS as usize);

    // [TC#2]: Framebuffer pre-reservado con capacidad exacta
    let mut framebuffer: Vec<u32> = vec![0xFF_1A_1A_2E; FB_SIZE];
    let mut backbuffer: Vec<u32> = vec![0xFF_1A_1A_2E; FB_SIZE];

    // Relojes para paso fijo de simulación
    let mut last_sim_tick = Instant::now();
    let mut sim_accumulator = Duration::from_micros(0);
    let sim_tick_duration = Duration::from_micros(MICROS_PER_TICK);

    // Estado de input
    let mut input_state = input::InputState::default();

    // Estadísticas FPS
    let mut frame_count: u64 = 0;
    let mut fps_timer = Instant::now();
    let mut current_fps: u32 = 0;

    // Contador de frames para estadísticas del bump allocator
    let mut frames_since_bump_report: u64 = 0;

    println!("Simulación iniciada. Presiona ESC para salir.");

    // =======================================================================
    // BUCLE PRINCIPAL
    // =======================================================================
    while window.is_open() && !window.is_key_down(Key::Escape) {
        // [TA#20]: Resetear bump allocator al inicio de cada frame
        bump_alloc::reset_frame();

        let _frame_start = Instant::now();

        // Input
        input_state.update(&window);
        ecs::process_input(&mut world, &input_state);

        // Simulación (paso fijo)
        let now = Instant::now();
        let elapsed = now - last_sim_tick;
        last_sim_tick = now;
        sim_accumulator += elapsed;

        while sim_accumulator >= sim_tick_duration {
            sim::tick(&mut world, 1.0 / SIM_TICKS_PER_SECOND as f32);
            sim_accumulator -= sim_tick_duration;
        }

        // Render
        backbuffer.fill(0xFF_1A_1A_2E);
        render::render_world(&world, &mut backbuffer, WINDOW_WIDTH, WINDOW_HEIGHT);
        framebuffer.copy_from_slice(&backbuffer);
        window
            .update_with_buffer(&framebuffer, WINDOW_WIDTH, WINDOW_HEIGHT)
            .expect("Error al actualizar la ventana");

        // Estadísticas
        frame_count += 1;
        frames_since_bump_report += 1;

        if fps_timer.elapsed() >= Duration::from_secs(1) {
            current_fps = frame_count as u32;
            frame_count = 0;
            fps_timer = Instant::now();

            // Reportar uso del bump allocator cada 5 segundos
            if frames_since_bump_report >= 5 * TARGET_FPS as u64 {
                let used_kb = bump_alloc::frame_allocator().used() / 1024;
                let free_kb = bump_alloc::frame_allocator().free() / 1024;
                frames_since_bump_report = 0;
                window.set_title(&format!(
                    "Citybound Native - {} FPS | Entidades: {} | Bump: {}KB/{}KB libres",
                    current_fps,
                    ecs::entity_count(&world),
                    used_kb,
                    free_kb,
                ));
            } else {
                window.set_title(&format!(
                    "Citybound Native - {} FPS | Entidades: {}",
                    current_fps,
                    ecs::entity_count(&world),
                ));
            }
        }
    }

    println!("Simulación terminada. FPS final: {}", current_fps);
    println!("Pool: {}/{} slots usados", pool.alive_count(), pool.capacity());
    println!("Bump allocator: {}KB pico usado", bump_alloc::frame_allocator().used() / 1024);
}
