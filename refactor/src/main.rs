// Citybound Native v0.7.0 - Punto de entrada principal
//
// ARQUITECTURA COMPLETA:
// - ECS puro con hecs
// - Framebuffer software rendering con SIMD
// - Flow Fields + Bitboards + Lanes (tráfico A/B Street)
// - Design Tool interactivo
// - 5 sistemas de realismo económico:
//   [M#1] Cadenas de suministro con camiones físicos
//   [M#2] Valor del suelo, contaminación, gentrificación
//   [M#3] Propagación de agua y electricidad
//   [M#4] Desgaste de infraestructura y baches
//   [M#5] Mercado laboral con commutes reales

#![allow(unsafe_code)]
#![cfg_attr(not(test), windows_subsystem = "windows")]

use citybound_native::*;
use minifb::{Key, Window, WindowOptions, Scale, ScaleMode};
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

    println!("Citybound Native v0.7.0 - City Builder Realista");
    println!("Sistemas: Supply Chain | Land Value | Utilities | Road Wear | Labor Market");

    // ===== FASE DE CARGA =====
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);
    bump_alloc::init_frame_allocator();
    let _audio = audio::AudioPlayer::init();

    let mut pool = object_pool::EntityPool::new(1000);
    let mut world = ecs::create_world(&mut pool);
    sim::init_simulation(&mut world);

    // ===== CACHE WARMING =====
    println!("Calentando caché...");
    rng_pool::warm_rng_cache();

    for y in (0..terrain::TERRAIN_SIZE).step_by(8) {
        for x in (0..terrain::TERRAIN_SIZE).step_by(8) {
            let _ = world.terrain.height(x, y);
        }
    }

    for y in (0..flow_field::FLOW_GRID_SIZE).step_by(8) {
        for x in (0..flow_field::FLOW_GRID_SIZE).step_by(8) {
            let _ = world.flow_fields.sample_combined(x as f32, y as f32, false);
        }
    }

    // Warm land value y pollution
    for _ in 0..10 {
        let _ = world.land_value_map.get(64, 64);
        let _ = world.pollution_map.get(64, 64);
    }

    println!("Caché caliente. {} carriles, {} intersecciones.",
        world.lane_manager.lanes.len(), world.lane_manager.intersections.len());
    println!("[Tab] Diseño | [WASD] Mover | [ESC] Salir");

    // ===== VENTANA =====
    let mut window = Window::new(
        "Citybound Native v0.7.0 - City Builder (ESC para salir)",
        WINDOW_WIDTH, WINDOW_HEIGHT,
        WindowOptions {
            scale: Scale::X2,
            scale_mode: ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    ).expect("No se pudo crear la ventana.");

    window.set_target_fps(TARGET_FPS as usize);

    let mut framebuffer: Vec<u32> = vec![0xFF_1A_1A_2E; FB_SIZE];
    let mut backbuffer: Vec<u32> = vec![0xFF_1A_1A_2E; FB_SIZE];
    simd_render::warm_cache(&mut backbuffer, FB_SIZE);

    let mut last_tick = Instant::now();
    let mut accumulator = Duration::from_micros(0);
    let tick_dur = Duration::from_micros(MICROS_PER_TICK);

    let mut input_state = input::InputState::default();
    let mut frame_count: u64 = 0;
    let mut fps_timer = Instant::now();
    let mut current_fps: u32 = 0;

    // ===== BUCLE PRINCIPAL =====
    while window.is_open() && !window.is_key_down(Key::Escape) {
        bump_alloc::reset_frame();
        input_state.update(&window);

        interactive::process_design_input(
            &mut world.design_tool, &mut world, &input_state,
            WINDOW_WIDTH, WINDOW_HEIGHT,
        );

        if !world.design_tool.active
            || input_state.is_key_down(input::GameKey::W)
            || input_state.is_key_down(input::GameKey::A)
            || input_state.is_key_down(input::GameKey::S)
            || input_state.is_key_down(input::GameKey::D)
        {
            ecs::process_input(&mut world, &input_state);
        }

        let now = Instant::now();
        let elapsed = now - last_tick;
        last_tick = now;
        accumulator += elapsed;

        while accumulator >= tick_dur {
            sim::tick(&mut world, 1.0 / SIM_TICKS_PER_SECOND as f32);
            accumulator -= tick_dur;
        }

        backbuffer.fill(0xFF_1A_1A_2E);
        render::render_world(&world, &mut backbuffer, WINDOW_WIDTH, WINDOW_HEIGHT);

        interactive::render_design_overlay(
            &world.design_tool, &mut backbuffer,
            WINDOW_WIDTH, WINDOW_HEIGHT, &world,
        );

        framebuffer.copy_from_slice(&backbuffer);
        window.update_with_buffer(&framebuffer, WINDOW_WIDTH, WINDOW_HEIGHT)
            .expect("Error al actualizar ventana");

        frame_count += 1;
        if fps_timer.elapsed() >= Duration::from_secs(1) {
            current_fps = frame_count as u32;
            frame_count = 0;
            fps_timer = Instant::now();

            let trucks = world.world.query::<&supply_chain::CargoTruck>().iter().count();
            window.set_title(&format!(
                "Citybound v0.7 - {} FPS | {} coches | {} camiones | {} carriles",
                current_fps,
                world.world.query::<&ecs::TrafficCar>().iter().count(),
                trucks,
                world.lane_manager.lanes.len()
            ));
        }
    }

    println!("Simulación terminada. FPS: {}, Ticks: {}", current_fps, world.sim_tick);
}
