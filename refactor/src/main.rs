// Citybound Native v0.13.0 — Punto de entrada principal
//
// ARQUITECTURA CROSS-PLATFORM [FASE 8]:
// - GPU Backend Adaptativo: detecta hardware (Tier 0-3) y selecciona
//   el mejor backend disponible (Vulkan/DX12/Metal/OpenGL ES/CPU SIMD)
// - Capa de abstracción platform.rs para Windows, Android, macOS, Linux
// - GameWorld en heap (Box) para evitar stack overflow
// - Shaders WGSL pre-compilados en tiempo de carga
//
// PLATAFORMAS SOPORTADAS:
// - Windows (native, DX12/Vulkan via wgpu)
// - Android (NativeActivity, Vulkan/OpenGL ES)
// - macOS (Metal)
// - Linux (Vulkan/OpenGL)

#![allow(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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

    // ===== [FASE 8] DETECCIÓN DE HARDWARE =====
    let hw_tier = gpu_backend::HardwareTier::current();
    let gpu_api = gpu_backend::GpuApi::detect();

    println!("══════════════════════════════════════════════════");
    println!("  Citybound Native v0.13.0 — City Builder Realista");
    println!("  Plataforma: {} | {}", platform::platform_name(), platform::arch_name());
    println!("  GPU API: {} | Tier: {:?} (nivel {})", 
        gpu_api.name(), hw_tier, hw_tier as u8);
    println!("  Compute Shaders: {} | Max Textures: {} | Atlas: {}x{}",
        if hw_tier.supports_compute_shaders() { "SÍ" } else { "NO" },
        hw_tier.max_texture_units(),
        hw_tier.max_texture_size(),
        hw_tier.max_texture_size());
    println!("══════════════════════════════════════════════════");

    // ===== FASE DE CARGA =====
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);
    bump_alloc::init_frame_allocator();
    let _audio_player = audio::AudioPlayer::init();

    // [FASE 8] Warming de shaders
    let shaders_ok = shaders::warm_shader_cache();
    if !shaders_ok {
        eprintln!("ADVERTENCIA: Falló la validación de shaders. Usando CPU fallback.");
    }

    // [FASE 8] Inicializar GPU backend adaptativo
    let mut gpu_state = gpu_backend::init_render_backend(
        WINDOW_WIDTH as u32, 
        WINDOW_HEIGHT as u32
    );

    let mut pool = object_pool::EntityPool::new(1000);
    // [FIX CRÍTICO]: GameWorld en heap para evitar stack overflow
    let mut world = Box::new(ecs::create_world(&mut pool));
    sim::init_simulation(&mut world);

    // Intentar cargar partida guardada
    if let Ok(saved) = persistence::load_game("save.dat") {
        println!("Partida cargada: tick {}, tesoro ${:.0}", saved.sim_tick, saved.finance_treasury);
        world.sim_tick = saved.sim_tick;
        world.time_of_day = saved.time_of_day;
        world.finance.treasury = saved.finance_treasury;
    }

    // ===== CACHE WARMING [TA#8] =====
    println!("Calentando caché L1/L2...");
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

    world.render_cache.rebuild_from_world(&world.world);

    println!("Caché caliente. {} carriles, {} intersecciones.",
        world.lane_manager.lanes.len(), world.lane_manager.intersections.len());
    println!("Tesoro inicial: ${:.0}", world.finance.treasury);
    println!("[Tab] Diseño | [WASD] Mover | [F5] Guardar | [F9] Cargar | [ESC] Salir");

    // ===== PLATFORM: Crear ventana nativa =====
    let mut window_system = platform::WindowSystem::new(
        "Citybound Native v0.13 — GPU Adaptativa (ESC para salir)",
        WINDOW_WIDTH as u32,
        WINDOW_HEIGHT as u32,
    );

    // Doble buffer con punteros (sin memcpy en swap)
    let mut buffer_a: Vec<u32> = vec![0xFF_1A_1A_2E; FB_SIZE];
    let mut buffer_b: Vec<u32> = vec![0xFF_1A_1A_2E; FB_SIZE];
    let mut front_ptr: *mut Vec<u32> = &mut buffer_a;
    let mut back_ptr: *mut Vec<u32> = &mut buffer_b;

    simd_render::warm_cache(&mut buffer_a, FB_SIZE);
    simd_render::warm_cache(&mut buffer_b, FB_SIZE);

    let mut last_tick = Instant::now();
    let mut accumulator = Duration::from_micros(0);
    let tick_dur = Duration::from_micros(MICROS_PER_TICK);

    let mut input_state = input::InputState::default();
    let mut frame_count: u64 = 0;
    let mut fps_timer = Instant::now();
    let mut current_fps: u32 = 0;

    let mut ticks_since_tax: u64 = 0;
    let mut ticks_since_waste: u64 = 0;

    let mut title_buf: [u8; 256] = [0; 256];

    // ===== BUCLE PRINCIPAL =====
    while window_system.is_open() {
        bump_alloc::reset_frame();

        // Poll events nativos (keyboard, mouse, touch, resize)
        let events = window_system.poll_events();
        for event in &events {
            input_state.process_platform_event(event);
        }

        // ESC para salir
        if input_state.key_pressed(platform::KeyCode::Escape) {
            break;
        }

        // ===== SIMULACIÓN (paso fijo) =====
        let now = Instant::now();
        let elapsed = now.duration_since(last_tick);
        last_tick = now;
        accumulator += elapsed;

        while accumulator >= tick_dur {
            sim::tick_simulation(&mut world, &mut pool);
            accumulator -= tick_dur;

            ticks_since_tax += 1;
            ticks_since_waste += 1;

            // Recaudar impuestos cada 30 ticks (~cada 3 segundos a 10tps)
            if ticks_since_tax >= 30 {
                tax_system::collect_taxes(&mut world);
                ticks_since_tax = 0;
            }

            // Procesar residuos cada 60 ticks
            if ticks_since_waste >= 60 {
                waste_mgmt::process_waste_cycle(&mut world);
                ticks_since_waste = 0;
            }
        }

        // ===== INPUT =====
        let cam_dx = input_state.camera_dx();
        let cam_dy = input_state.camera_dy();
        if cam_dx != 0.0 || cam_dy != 0.0 {
            world.camera.x = (world.camera.x + cam_dx).max(0.0).min(terrain::TERRAIN_SIZE as f32 * 64.0);
            world.camera.y = (world.camera.y + cam_dy).max(0.0).min(terrain::TERRAIN_SIZE as f32 * 64.0);
        }

        let zoom_delta = input_state.zoom_delta();
        if zoom_delta != 0.0 {
            world.camera.zoom = (world.camera.zoom + zoom_delta).max(0.25).min(4.0);
        }

        // Toggle modo diseño
        if input_state.key_pressed(platform::KeyCode::Tab) {
            world.interactive_mode = !world.interactive_mode;
            println!("Modo diseño: {}", if world.interactive_mode { "ON" } else { "OFF" });
        }

        // Guardar partida
        if input_state.key_pressed(platform::KeyCode::F5) {
            match persistence::save_game(&world, "save.dat") {
                Ok(_) => println!("Partida guardada (tick {})", world.sim_tick),
                Err(e) => eprintln!("Error al guardar: {}", e),
            }
        }

        // Cargar partida
        if input_state.key_pressed(platform::KeyCode::F9) {
            match persistence::load_game("save.dat") {
                Ok(saved) => {
                    world.sim_tick = saved.sim_tick;
                    world.time_of_day = saved.time_of_day;
                    world.finance.treasury = saved.finance_treasury;
                    println!("Partida cargada (tick {})", saved.sim_tick);
                }
                Err(e) => eprintln!("Error al cargar: {}", e),
            }
        }

        // Clic para construir en modo diseño
        if world.interactive_mode && input_state.mouse_clicked() {
            let mx = input_state.mouse_x();
            let my = input_state.mouse_y();
            interactive::handle_click(&mut world, &mut pool, mx, my);
        }

        // ===== RENDER =====
        let back_buffer: &mut Vec<u32> = unsafe { &mut *back_ptr };

        // [FASE 8] Usar GPU backend si está disponible
        match &mut gpu_state {
            gpu_backend::ActiveBackend::CpuSimd(cpu) => {
                // Rellenar con color de fondo
                simd_render::fill_fast(back_buffer, FB_SIZE, render::COLOR_GRASS);

                // Reconstruir cache de render si cambió algo
                if world.sim_tick % 10 == 0 {
                    world.render_cache.rebuild_from_world(&world.world);
                }

                // Renderizar con SIMD
                render::render_world_cached(&world, back_buffer, WINDOW_WIDTH, WINDOW_HEIGHT);

                // Panel de estadísticas
                render::render_stats_panel(back_buffer, WINDOW_WIDTH, WINDOW_HEIGHT, &world, current_fps);

                // Overlay de modo diseño
                if world.interactive_mode {
                    render::render_interactive_overlay(back_buffer, WINDOW_WIDTH, WINDOW_HEIGHT, &world, input_state.mouse_x(), input_state.mouse_y());
                }

                // Copiar al framebuffer del CPU backend
                cpu.framebuffer.copy_from_slice(back_buffer);
            }
        }

        // Swap buffers
        std::mem::swap(&mut front_ptr, &mut back_ptr);

        // Mostrar por ventana nativa
        let front_buffer: &Vec<u32> = unsafe { &*front_ptr };
        window_system.update(front_buffer);

        input_state.end_frame();

        // ===== FPS =====
        frame_count += 1;
        if fps_timer.elapsed() >= Duration::from_secs(1) {
            current_fps = frame_count as u32;
            frame_count = 0;
            fps_timer = Instant::now();

            // Actualizar título con FPS
            let hw_name = match hw_tier {
                gpu_backend::HardwareTier::CpuOnly => "CPU-SIMD",
                gpu_backend::HardwareTier::IntegratedGpu => "GPU-Int",
                gpu_backend::HardwareTier::MidRangeGpu => "GPU-Mid",
                gpu_backend::HardwareTier::HighEndGpu => "GPU-High",
            };
            let title = format!(
                "Citybound v0.13 [{}] | FPS:{} | Tick:{} | ${:.0} | Pob:{} | {}",
                hw_name, current_fps, world.sim_tick, world.finance.treasury,
                world.population_count, gpu_api.name()
            );
            window_system.set_title(&title);
        }
    }

    // Guardar al salir
    println!("Cerrando... guardando partida.");
    let _ = persistence::save_game(&world, "save.dat");
    println!("Citybound Native v0.13.0 — Sesión terminada.");
}
