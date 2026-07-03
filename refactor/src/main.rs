// Citybound Native v0.12.0 — Punto de entrada principal
//
// ARQUITECTURA CROSS-PLATFORM:
// - Capa de abstracción platform.rs para Windows, Android, macOS, Linux
// - GameWorld en heap (Box) para evitar stack overflow
// - Renderizado software con SIMD + framebuffer nativo
//
// PLATAFORMAS SOPORTADAS:
// - Windows (native, DX12/Vulkan via wgpu)
// - Android (NativeActivity, Vulkan)
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

    println!("Citybound Native v0.12.0 — City Builder Realista [Cross-Platform]");
    println!("Plataforma: {} | Arquitectura: {}", 
        platform::platform_name(), 
        platform::arch_name());
    println!("RenderCache | Audio | Save/Load | Stats | Rayon | MOBIL | Dia/Noche");

    // ===== FASE DE CARGA =====
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);
    bump_alloc::init_frame_allocator();
    let _audio_player = audio::AudioPlayer::init();

    let mut pool = object_pool::EntityPool::new(1000);
    // [FIX CRÍTICO]: GameWorld en heap para evitar stack overflow
    // (el struct contiene múltiples grillas de 128x128 floats = ~65KB cada una)
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

    world.render_cache.rebuild_from_world(&world.world);

    println!("Caché caliente. {} carriles, {} intersecciones.",
        world.lane_manager.lanes.len(), world.lane_manager.intersections.len());
    println!("Tesoro inicial: ${:.0}", world.finance.treasury);
    println!("[Tab] Diseño | [WASD] Mover | [F5] Guardar | [F9] Cargar | [ESC] Salir");

    // ===== PLATFORM: Crear ventana nativa =====
    let mut window_system = platform::WindowSystem::new(
        "Citybound Native v0.12 — City Builder (ESC para salir)",
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

        if input_state.is_key_pressed(input::GameKey::Escape) {
            break;
        }

        // F5: Guardar partida
        if input_state.is_key_pressed(input::GameKey::F5) {
            let save_data = persistence::SaveData::from_world(&world);
            if persistence::save_game(&save_data, "save.dat").is_ok() {
                println!("Partida guardada.");
            }
        }
        // F9: Cargar partida
        if input_state.is_key_pressed(input::GameKey::F9) {
            if let Ok(saved) = persistence::load_game("save.dat") {
                world.sim_tick = saved.sim_tick;
                world.time_of_day = saved.time_of_day;
                world.finance.treasury = saved.finance_treasury;
                println!("Partida cargada: tick {}", saved.sim_tick);
            }
        }

        let mut dt = std::mem::replace(&mut world.design_tool, interactive::DesignTool::new());
        interactive::process_design_input(
            &mut dt, &mut world, &input_state,
            WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32,
        );
        world.design_tool = dt;

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
            let dt = 1.0 / SIM_TICKS_PER_SECOND as f32;
            sim::tick(&mut world, dt);
            accumulator -= tick_dur;

            ticks_since_tax += 1;
            if ticks_since_tax >= tax_system::TAX_COLLECTION_INTERVAL {
                ticks_since_tax = 0;
                tax_system::collect_taxes(&mut world, &[1000.0_f32; 128 * 128]);
            }

            world.parking_mgr.tick(dt);

            ticks_since_waste += 1;
            if ticks_since_waste >= 600 {
                ticks_since_waste = 0;
                world.waste_mgr.tick(dt);
            }
            world.waste_mgr.tick(dt);

            let _strike_effects = world.politics.tick(dt, &mut world.finance.treasury);

            if world.sim_tick % 10 == 0 {
                world.land_value_map.diffuse();
                world.pollution_map.diffuse_and_decay();
            }

            road_wear::tick_road_wear(&mut world);

            if world.sim_tick % 10 == 0 {
                let _ = &world.water_grid;
                let _ = &world.power_grid;
            }
        }

        // Rebuild spatial grid una vez por frame
        world.spatial_grid.rebuild(&world.world);

        // Actualizar RenderCache
        if world.render_cache.dirty {
            world.render_cache.rebuild_from_world(&world.world);
        }

        // ===== RENDER al backbuffer =====
        let backbuffer: &mut [u32] = unsafe { &mut *back_ptr };
        backbuffer.fill(0xFF_1A_1A_2E);

        render::render_world_cached(&world, backbuffer, WINDOW_WIDTH, WINDOW_HEIGHT);
        climate::apply_day_night_overlay(backbuffer, WINDOW_WIDTH, WINDOW_HEIGHT, world.time_of_day);

        interactive::render_design_overlay(
            &world.design_tool, backbuffer,
            WINDOW_WIDTH, WINDOW_HEIGHT, &world,
        );

        render::render_stats_panel(&world, backbuffer, WINDOW_WIDTH, WINDOW_HEIGHT, current_fps);

        // ===== PRESENT: enviar backbuffer a pantalla =====
        window_system.present_frame(unsafe { &*back_ptr }, WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);

        // Swap punteros (sin copiar datos)
        std::mem::swap(&mut front_ptr, &mut back_ptr);

        // ===== ESTADÍSTICAS =====
        frame_count += 1;
        if fps_timer.elapsed() >= Duration::from_secs(1) {
            current_fps = frame_count as u32;
            frame_count = 0;
            fps_timer = Instant::now();

            let cars = world.world.query::<&ecs::TrafficCar>().iter().count();
            let trucks = world.world.query::<&supply_chain::CargoTruck>().iter().count();
            let treasury = world.finance.treasury;
            let circling = world.parking_mgr.circling_cars as usize;
            let approval = world.politics.global_approval;
            let pop = world.world.query::<&ecs::ConstructionState>().iter().count();

            let title = write_fps_title(&mut title_buf, current_fps, cars, trucks, treasury, circling, approval, pop);
            window_system.set_title(title);
        }
    }

    // Auto-guardar al salir
    let save_data = persistence::SaveData::from_world(&world);
    let _ = persistence::save_game(&save_data, "save.dat");

    println!("Simulación terminada. FPS: {}, Ticks: {}, Tesoro: ${:.0}, Población: {}",
        current_fps, world.sim_tick, world.finance.treasury,
        world.world.query::<&ecs::ConstructionState>().iter().count());
}

/// Zero-allocation: escribe el título en buffer de stack
fn write_fps_title<'a>(
    buf: &'a mut [u8],
    fps: u32,
    cars: usize,
    trucks: usize,
    treasury: f32,
    _circling: usize,
    approval: f32,
    pop: usize,
) -> &'a str {
    let prefix = b"CB v0.12 ";
    let mut pos = prefix.len();
    buf[..pos].copy_from_slice(prefix);

    pos += write_u32(buf, pos, fps);
    buf[pos] = b'f'; pos += 1;
    buf[pos] = b' '; pos += 1;
    pos += write_usize(buf, pos, pop);
    buf[pos] = b'p'; pos += 1;
    buf[pos] = b' '; pos += 1;
    pos += write_usize(buf, pos, cars);
    buf[pos] = b'c'; pos += 1;
    buf[pos] = b' '; pos += 1;
    pos += write_usize(buf, pos, trucks);
    buf[pos] = b't'; pos += 1;
    buf[pos] = b' '; pos += 1;

    let tk = treasury / 1000.0;
    let tki = tk as u32;
    buf[pos] = b'$'; pos += 1;
    pos += write_u32(buf, pos, tki);
    buf[pos] = b'k'; pos += 1;
    buf[pos] = b' '; pos += 1;

    let appr = (approval * 100.0) as u32;
    pos += write_u32(buf, pos, appr);
    buf[pos] = b'%'; pos += 1;

    let valid_len = pos.min(buf.len());
    unsafe { std::str::from_utf8_unchecked(&buf[..valid_len]) }
}

#[inline(always)]
fn write_u32(buf: &mut [u8], pos: usize, mut val: u32) -> usize {
    if val == 0 {
        buf[pos] = b'0';
        return 1;
    }
    let mut digits: [u8; 10] = [0; 10];
    let mut d = 0;
    while val > 0 && d < 10 {
        digits[d] = b'0' + (val % 10) as u8;
        val /= 10;
        d += 1;
    }
    for i in (0..d).rev() {
        buf[pos + (d - 1 - i)] = digits[i];
    }
    d
}

#[inline(always)]
fn write_usize(buf: &mut [u8], pos: usize, val: usize) -> usize {
    write_u32(buf, pos, val as u32)
}
