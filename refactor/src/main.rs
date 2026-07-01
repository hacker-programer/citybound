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

// Permitimos unsafe con documentación obligatoria (SAFETY comments)
#![warn(unsafe_code)]
#![cfg_attr(not(test), windows_subsystem = "windows")] // Sin ventana de consola en release

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

/// Tamaño del mundo en celdas de grilla
pub const WORLD_GRID_SIZE: usize = 256;
/// Tamaño de celda en píxeles
pub const CELL_SIZE: usize = 4;

/// Ticks de simulación por segundo (paso fijo)
pub const SIM_TICKS_PER_SECOND: u32 = 10;
/// Duración de un tick en microsegundos
pub const MICROS_PER_TICK: u64 = 1_000_000 / SIM_TICKS_PER_SECOND as u64;

/// Frame rate objetivo
pub const TARGET_FPS: u32 = 30;
/// Duración de un frame en microsegundos
pub const MICROS_PER_FRAME: u64 = 1_000_000 / TARGET_FPS as u64;

// ---------------------------------------------------------------------------
// TÉCNICA COMÚN #1: Object Pooling Masivo
// 10,000 entidades preasignadas en arrays estáticos
// ---------------------------------------------------------------------------
const MAX_ENTITIES: usize = 10_000;

// ---------------------------------------------------------------------------
// PUNTO DE ENTRADA
// ---------------------------------------------------------------------------
fn main() {
    // Configurar backtrace en caso de panic
    std::panic::set_hook(Box::new(|info| {
        eprintln!("FATAL: {}", info);
        std::process::abort();
    }));

    println!("Citybound Native v0.4.0 - ECS City Simulator");
    println!("Optimizado para Pentium 4GB RAM / 2 cores");
    println!("Inicializando...");

    // Inicializar LUTs trigonométricas [TC#5]
    luts::init_trig_luts();

    // Inicializar Object Pool [TC#1]
    let mut pool = object_pool::EntityPool::new(MAX_ENTITIES);

    // Inicializar ECS World
    let mut world = ecs::create_world(&mut pool);

    // Inicializar sistemas de simulación
    sim::init_simulation(&mut world);

    // Crear ventana con framebuffer
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
    .expect("No se pudo crear la ventana. Verifica que los drivers de video estén instalados.");

    // Limitar el framerate a TARGET_FPS
    window.set_target_fps(TARGET_FPS as usize);

    // Framebuffer principal [TC#2]: pre-reservado con capacidad exacta
    let mut framebuffer: Vec<u32> = Vec::with_capacity(FB_SIZE);
    framebuffer.resize(FB_SIZE, 0xFF_1A_1A_2E);

    // Buffer de respaldo para double-buffering implícito
    let mut backbuffer: Vec<u32> = vec![0xFF_1A_1A_2E; FB_SIZE];

    // Relojes para paso fijo de simulación
    let mut last_sim_tick = Instant::now();
    let mut sim_accumulator = Duration::from_micros(0);
    let sim_tick_duration = Duration::from_micros(MICROS_PER_TICK);

    // Estado de input
    let mut input_state = input::InputState::default();

    // Contador de frames para estadísticas
    let mut frame_count: u64 = 0;
    let mut fps_timer = Instant::now();
    let mut current_fps: u32 = 0;

    println!("Simulación iniciada. Presiona ESC para salir.");

    // =======================================================================
    // BUCLE PRINCIPAL DEL JUEGO
    // =======================================================================
    while window.is_open() && !window.is_key_down(Key::Escape) {
        let frame_start = Instant::now();

        // --- Input ---
        input_state.update(&window);
        ecs::process_input(&mut world, &input_state);

        // --- Simulación (paso fijo) ---
        let now = Instant::now();
        let elapsed = now - last_sim_tick;
        last_sim_tick = now;
        sim_accumulator += elapsed;

        while sim_accumulator >= sim_tick_duration {
            sim::tick(&mut world, 1.0 / SIM_TICKS_PER_SECOND as f32);
            sim_accumulator -= sim_tick_duration;
        }

        // --- Render ---
        // [TA#17]: slice::fill evita bounds checks y es SIMD-friendly
        backbuffer.fill(0xFF_1A_1A_2E);

        render::render_world(&world, &mut backbuffer, WINDOW_WIDTH, WINDOW_HEIGHT);

        framebuffer.copy_from_slice(&backbuffer);

        window
            .update_with_buffer(&framebuffer, WINDOW_WIDTH, WINDOW_HEIGHT)
            .expect("Error al actualizar la ventana");

        // --- Estadísticas FPS ---
        frame_count += 1;
        if fps_timer.elapsed() >= Duration::from_secs(1) {
            current_fps = frame_count as u32;
            frame_count = 0;
            fps_timer = Instant::now();
            window.set_title(&format!(
                "Citybound Native - {} FPS | Entidades: {}",
                current_fps,
                ecs::entity_count(&world)
            ));
        }

        // --- Frame rate limiting ---
        let frame_time = frame_start.elapsed();
        if frame_time < Duration::from_micros(MICROS_PER_FRAME) {
            std::thread::sleep(Duration::from_micros(MICROS_PER_FRAME) - frame_time);
        }
    }

    println!("Simulación terminada. FPS promedio: {}", current_fps);
}
