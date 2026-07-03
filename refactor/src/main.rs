// Citybound Native v0.10.0 — Punto de entrada principal
//
// FASE 7: FEATURES COMPLETAS
// - RenderCache integrado en pipeline
// - Audio procedural con cpal (real)
// - Save/Load con bincode
// - Panel de estadísticas
// - Rayon para paralelismo
// - MOBIL lane changes reales
// - Ciclo día/noche con color grading
// - Más edificios: Hospital, Escuela, Policía, Parque
//
// ARQUITECTURA:
// - ECS puro con hecs + SpatialGrid
// - Framebuffer software rendering con SSE2
// - Flow Fields + Bitboards + Lanes (tráfico A/B Street)
// - Design Tool interactivo
// - 10 sistemas de realismo [M#1..M#10]

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

    println!("Citybound Native v0.10.0 — City Builder Realista [Fase 7: Features]");
    println!("RenderCache | Audio cpal | Save/Load | Stats | Rayon | MOBIL | Dia/Noche");

    // ===== FASE DE CARGA =====
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);
    bump_alloc::init_frame_allocator();
    let mut audio_player = audio::AudioPlayer::init();
    audio_player.play_ambient();

    let mut pool = object_pool::EntityPool::new(1000);
    let mut world = ecs::create_world(&mut pool);
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

    // Rebuild render cache inicial
    world.render_cache.rebuild_from_world(&world.world);

    println!("Caché caliente. {} carriles, {} intersecciones.",
        world.lane_manager.lanes.len(), world.lane_manager.intersections.len());
    println!("Tesoro inicial: ${:.0}", world.finance.treasury);
    println!("[Tab] Diseño | [WASD] Mover | [F5] Guardar | [F9] Cargar | [ESC] Salir");

    // ===== VENTANA =====
    let mut window = Window::new(
        "Citybound Native v0.10 — City Builder (ESC para salir)",
        WINDOW_WIDTH, WINDOW_HEIGHT,
        WindowOptions {
            scale: Scale::X2,
            scale_mode: ScaleMode::AspectRatioStretch,
            ..WindowOptions::default()
        },
    ).expect("No se pudo crear la ventana.");

    window.set_target_fps(TARGET_FPS as usize);

    // [FASE 6]: Doble buffer con punteros (sin memcpy)
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

    // Buffer stack para título (zero-allocation)
    let mut title_buf: [u8; 256] = [0; 256];

    // ===== BUCLE PRINCIPAL =====
    while window.is_open() && !window.is_key_down(Key::Escape) {
        bump_alloc::reset_frame();
        input_state.update(&window);

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

        interactive::process_design_input(
            &mut world.design_tool, &mut world, &input_state,
            WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32,
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
            let dt = 1.0 / SIM_TICKS_PER_SECOND as f32;
            sim::tick(&mut world, dt);
            accumulator -= tick_dur;

            // Sistemas periódicos
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

        // [FASE 6]: Rebuild spatial grid una vez por frame
        world.spatial_grid.rebuild(&world.world);

        // [FASE 7]: Actualizar RenderCache (incremental)
        if world.render_cache.dirty {
            world.render_cache.rebuild_from_world(&world.world);
        }

        // ===== RENDER al backbuffer =====
        let backbuffer: &mut [u32] = unsafe { &mut *back_ptr };
        backbuffer.fill(0xFF_1A_1A_2E);

        // [FASE 7]: Usar RenderCache
        render::render_world_cached(&world, backbuffer, WINDOW_WIDTH, WINDOW_HEIGHT);

        // [FASE 7]: Overlay de día/noche
        climate::apply_day_night_overlay(backbuffer, WINDOW_WIDTH, WINDOW_HEIGHT, world.time_of_day);

        interactive::render_design_overlay(
            &world.design_tool, backbuffer,
            WINDOW_WIDTH, WINDOW_HEIGHT, &world,
        );

        // [FASE 7]: Panel de estadísticas
        render::render_stats_panel(&world, backbuffer, WINDOW_WIDTH, WINDOW_HEIGHT, current_fps);

        // ===== SWAP: enviar backbuffer a pantalla sin memcpy =====
        {
            let front: &[u32] = unsafe { &*back_ptr };
            window.update_with_buffer(front, WINDOW_WIDTH, WINDOW_HEIGHT)
                .expect("Error al actualizar ventana");
        }

        // Swap punteros (sin copiar datos)
        std::mem::swap(&mut front_ptr, &mut back_ptr);

        // ===== ESTADÍSTICAS (zero-alloc) =====
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

            // Zero-allocation title
            let title = write_fps_title(&mut title_buf, current_fps, cars, trucks, treasury, circling, approval, pop);
            window.set_title(title);
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
    circling: usize,
    approval: f32,
    pop: usize,
) -> &'a str {
    let prefix = b"CB v0.10 ";
    let mut pos = prefix.len();
    buf[..pos].copy_from_slice(prefix);

    // FPS + población
    pos += write_u32(buf, pos, fps);
    buf[pos] = b'f'; pos += 1;
    buf[pos] = b' '; pos += 1;
    pos += write_usize(buf, pos, pop);
    buf[pos] = b'p'; pos += 1;
    buf[pos] = b' '; pos += 1;

    // Cars
    pos += write_usize(buf, pos, cars);
    buf[pos] = b'c'; pos += 1;
    buf[pos] = b' '; pos += 1;

    // Trucks
    pos += write_usize(buf, pos, trucks);
    buf[pos] = b't'; pos += 1;
    buf[pos] = b' '; pos += 1;

    // Treasury
    let tk = treasury / 1000.0;
    let tki = tk as u32;
    buf[pos] = b'$'; pos += 1;
    pos += write_u32(buf, pos, tki);
    buf[pos] = b'k'; pos += 1;
    buf[pos] = b' '; pos += 1;

    // Approval
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
