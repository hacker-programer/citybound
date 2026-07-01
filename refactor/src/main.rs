// Citybound Native Refactor - Punto de entrada principal
//
// ARQUITECTURA:
// - ECS puro con hecs (Entity Component System)
// - Framebuffer software rendering con SIMD (minifb)
// - Flow Fields para tráfico O(1) [TA#7]
// - Bitboards para colisiones O(1) [TI#6]
// - RNG Pool pre-generado [TC#22]
// - Bump Allocator por frame [TA#20]
// - Audio procedural [TC#6]
// - Lane-based traffic A/B Street [#361]
// - Interactive urban design tool [#392]
//
// OPTIMIZACIONES APLICADAS (referencias a las 90 técnicas):
// [TC#1]  Object Pooling Masivo
// [TC#2]  Pre-reserva de capacidad
// [TC#5]  LUTs trigonométricas
// [TC#7]  Quadtree espacial
// [TC#14] Ruido Perlin pre-generado
// [TC#22] RNG Pool pre-generado
// [TA#2]  LTO + PGO en release
// [TA#7]  Flow Fields para pathfinding
// [TA#8]  Cache Warming
// [TA#9]  Structs alineados a 64B
// [TA#17] Acceso unchecked en bucles
// [TA#20] Bump allocators por frame
// [TI#6]  Bitboards para colisiones
// [TC#6]  Audio procedural pre-generado

#![allow(unsafe_code)]
#![cfg_attr(not(test), windows_subsystem = "windows")]

use citybound_native::*;
use minifb::{Key, Window, WindowOptions, Scale, ScaleMode};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// CONSTANTES DE CONFIGURACIÓN
// ---------------------------------------------------------------------------

pub const WINDOW_WIDTH: usize = 800;
pub const WINDOW_HEIGHT: usize = 600;
pub const FB_SIZE: usize = WINDOW_WIDTH * WINDOW_HEIGHT;
pub const SIM_TICKS_PER_SECOND: u32 = 10;
pub const MICROS_PER_TICK: u64 = 1_000_000 / SIM_TICKS_PER_SECOND as u64;
pub const TARGET_FPS: u32 = 30;
pub const MICROS_PER_FRAME: u64 = 1_000_000 / TARGET_FPS as u64;

// ---------------------------------------------------------------------------
// PUNTO DE ENTRADA
// ---------------------------------------------------------------------------
fn main() {
    std::panic::set_hook(Box::new(|info| {
        eprintln!("FATAL: {}", info);
        std::process::abort();
    }));

    println!("Citybound Native v0.6.0 - ECS City Simulator");
    println!("Features: Flow Fields | Bitboards | Lane Traffic | Design Tool");
    println!("Optimizado para Pentium 4GB RAM / 2 cores");

    // =======================================================================
    // FASE DE CARGA: Todas las precomputaciones aquí
    // =======================================================================

    // [TC#5]: LUTs trigonométricas
    luts::init_trig_luts();

    // [TC#22]: RNG Pool pre-generado (4096 valores, 16KB L1)
    rng_pool::init_rng_pool(42);

    // [TA#20]: Bump allocator del frame
    bump_alloc::init_frame_allocator();

    // Inicializar audio [TC#6]
    let _audio = audio::AudioPlayer::init();

    // [TC#1]: Object Pool
    let mut pool = object_pool::EntityPool::new(1000);

    // ECS World (incluye Terrain [TC#14], Quadtree [TC#7],
    // Flow Fields [TA#7], BitGrid [TI#6], LaneManager [#361], DesignTool [#392])
    let mut world = ecs::create_world(&mut pool);

    // Inicializar simulación (registra obstáculos en bitgrid)
    sim::init_simulation(&mut world);

    // =======================================================================
    // CACHE WARMING [TA#8]
    // =======================================================================
    println!("Calentando caché...");

    // Warm RNG pool
    rng_pool::warm_rng_cache();

    // Warm terrain
    for y in (0..terrain::TERRAIN_SIZE).step_by(8) {
        for x in (0..terrain::TERRAIN_SIZE).step_by(8) {
            let _h = world.terrain.height(x, y);
            let _c = world.terrain.baked_color(x, y);
        }
    }

    // Warm flow fields
    for y in (0..flow_field::FLOW_GRID_SIZE).step_by(8) {
        for x in (0..flow_field::FLOW_GRID_SIZE).step_by(8) {
            let _ = world.flow_fields.sample_combined(x as f32, y as f32, false);
        }
    }

    // Warm lane spatial grid
    for _ in 0..10 {
        let _ = world.lane_manager.lanes_near(64.0, 64.0, 10.0);
    }

    println!("Caché caliente. {} carriles, {} intersecciones.",
        world.lane_manager.lanes.len(),
        world.lane_manager.intersections.len());
    println!("[Tab] Herramienta de diseño | [WASD] Mover | [ESC] Salir");

    // =======================================================================
    // VENTANA Y FRAMEBUFFER
    // =======================================================================
    let mut window = Window::new(
        "Citybound Native v0.6.0 - ECS Simulator (ESC para salir)",
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

    // [TC#2]: Framebuffers pre-reservados
    let mut framebuffer: Vec<u32> = vec![0xFF_1A_1A_2E; FB_SIZE];
    let mut backbuffer: Vec<u32> = vec![0xFF_1A_1A_2E; FB_SIZE];

    // [TA#8]: Calentar framebuffer
    simd_render::warm_cache(&mut backbuffer, FB_SIZE);

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

    // =======================================================================
    // BUCLE PRINCIPAL
    // =======================================================================
    while window.is_open() && !window.is_key_down(Key::Escape) {
        // [TA#20]: Resetear bump allocator al inicio de cada frame
        bump_alloc::reset_frame();

        // Input
        input_state.update(&window);

        // Procesar input de herramienta de diseño [#392]
        interactive::process_design_input(
            &mut world.design_tool,
            &mut world,
            &input_state,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
        );

        // Input de cámara (solo si herramienta de diseño no está activa,
        // o con teclas WASD siempre)
        if !world.design_tool.active
            || input_state.is_key_down(input::GameKey::W)
            || input_state.is_key_down(input::GameKey::S)
            || input_state.is_key_down(input::GameKey::A)
            || input_state.is_key_down(input::GameKey::D)
        {
            ecs::process_input(&mut world, &input_state);
        }

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

        // Renderizar overlay de herramienta de diseño [#392]
        interactive::render_design_overlay(
            &world.design_tool,
            &mut backbuffer,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
            &world,
        );

        framebuffer.copy_from_slice(&backbuffer);
        window
            .update_with_buffer(&framebuffer, WINDOW_WIDTH, WINDOW_HEIGHT)
            .expect("Error al actualizar la ventana");

        // Estadísticas
        frame_count += 1;
        if fps_timer.elapsed() >= Duration::from_secs(1) {
            current_fps = frame_count as u32;
            frame_count = 0;
            fps_timer = Instant::now();

            let cars = world.world.query::<&ecs::TrafficCar>().iter().count();
            let buildings = world.world.query::<&ecs::ConstructionState>().iter().count();
            let mode_str = if world.design_tool.active {
                match world.design_tool.mode {
                    interactive::DesignMode::PaintZone => "PINTAR",
                    interactive::DesignMode::PlaceBuilding => "CONSTRUIR",
                    interactive::DesignMode::Inspect => "INSPECCIONAR",
                    _ => "DISEÑO",
                }
            } else {
                "SIM"
            };

            window.set_title(&format!(
                "Citybound v0.6 [{}] - {} FPS | {} coches | {} edificios | {} carriles",
                mode_str, current_fps, cars, buildings,
                world.lane_manager.lanes.len()
            ));
        }
    }

    println!("Simulación terminada. Estadísticas finales:");
    println!("  FPS: {}", current_fps);
    println!("  Ticks simulados: {}", world.sim_tick);
    println!("  Entidades: {}", ecs::entity_count(&world));
    println!("  Carriles: {}", world.lane_manager.lanes.len());
    println!("  Intersecciones: {}", world.lane_manager.intersections.len());
    println!("  Acciones deshechas: {}", world.design_tool.undo_stack.len());
}
