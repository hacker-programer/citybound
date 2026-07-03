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

    // ===== DETECCIÓN DE HARDWARE =====
    let hw_tier = gpu_backend::HardwareTier::current();
    let gpu_api = gpu_backend::GpuApi::detect();

    println!("══════════════════════════════════════════════════");
    println!("  Citybound Native v0.15.0 — City Builder Realista");
    println!("  Plataforma: {} | {}", platform::platform_name(), platform::arch_name());
    println!("  GPU API: {} | Tier: {:?} (nivel {})",
        gpu_api.name(), hw_tier, hw_tier as u8);
    println!("  Compute Shaders: {} | Max Textures: {}",
        if hw_tier.supports_compute_shaders() { "SI" } else { "NO" },
        hw_tier.max_texture_units());
    println!("══════════════════════════════════════════════════");

    // ===== VENTANA NATIVA =====
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
    let mut framebuffer: Vec<u32> = vec![0xFF_00_00_00u32; FB_SIZE];

    // ===== LUNA GRAFICA =====
    luts::init_trig_luts();
    rng_pool::init_rng_pool(42);

    // ===== MUNDO (Box — en heap, NO en stack) =====
    let mut pool = object_pool::EntityPool::new(10000);
    let mut world = ecs::create_world(&mut pool);
    // world es Box<GameWorld>, todos los arrays masivos están en heap

    // ===== RENDER BACKEND (CPU SIMD + GPU opcional) =====
    let _render_backend = gpu_backend::init_render_backend(WINDOW_WIDTH as u32, WINDOW_HEIGHT as u32);

    // ===== INPUT =====
    let mut input_state = input::InputState::new();

    // ===== VARIABLES DE CONTROL =====
    let mut sim_accumulator: u64 = 0;
    let mut last_time = Instant::now();
    let mut frame_count: u64 = 0;
    let mut fps_timer = Instant::now();

    // Warm render cache
    world.render_cache.rebuild_from_world(&world.world);

    println!("[OK] Mundo creado: {} entidades", ecs::entity_count(&world));
    println!("[OK] GPU Backend: CPU SIMD (Tier {})", hw_tier as u8);
    println!("[OK] Simulación iniciada — {}/s ticks, {} FPS objetivo",
        SIM_TICKS_PER_SECOND, TARGET_FPS);

    // ===== GAME LOOP =====
    while window.is_open()
        && !input_state.is_key_down(input::GameKey::Escape)
    {
        let now = Instant::now();
        let dt = now.duration_since(last_time);
        last_time = now;

        // ---- INPUT ----
        input_state.keys_pressed = 0;
        input_state.keys_released = 0;

        for key in window.get_keys() {
            if let Some(gk) = map_minifb_key(key) {
                let bit = 1u128 << (gk as u8);
                if input_state.keys_down & bit == 0 {
                    input_state.keys_pressed |= bit;
                }
                input_state.keys_down |= bit;
            }
        }

        if let Some((mx, my)) = window.get_mouse_pos(minifb::MouseMode::Clamp) {
            input_state.mouse_x = mx;
            input_state.mouse_y = my;
            input_state.mouse_inside = true;
        }
        input_state.mouse_left = window.get_mouse_down(minifb::MouseButton::Left);
        input_state.mouse_right = window.get_mouse_down(minifb::MouseButton::Right);

        if let Some((_scroll_x, scroll_y)) = window.get_scroll_wheel() {
            input_state.scroll_delta = scroll_y;
        }

        // ---- PROCESAR INPUT DEL JUEGO ----
        ecs::process_input(&mut world, &input_state);

        // ---- SIMULACIÓN (PASO FIJO) ----
        sim_accumulator += dt.as_micros() as u64;
        while sim_accumulator >= MICROS_PER_TICK {
            sim::tick(&mut world, 1.0 / SIM_TICKS_PER_SECOND as f32);
            sim_accumulator -= MICROS_PER_TICK;
        }

        // ---- RENDERIZAR ----
        render::render_world_cached(
            &world,
            &mut framebuffer,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
        );

        window
            .update_with_buffer(&framebuffer, WINDOW_WIDTH, WINDOW_HEIGHT)
            .unwrap();

        // ---- FPS ----
        frame_count += 1;
        if fps_timer.elapsed() >= Duration::from_secs(1) {
            let fps = frame_count;
            frame_count = 0;
            fps_timer = Instant::now();

            let title = format!(
                "Citybound v0.15 | {} FPS | Tick {} | Ents: {} | Tesoro: ${:.0} | {}",
                fps,
                world.sim_tick,
                ecs::entity_count(&world),
                world.finance.treasury,
                gpu_api.name(),
            );
            window.set_title(&title);
        }
    }

    // ===== GUARDAR PARTIDA =====
    let save_data = persistence::SaveData::from_world(&world);
    match persistence::save_game(&save_data, "save.dat") {
        Ok(()) => println!("[OK] Partida guardada en save.dat"),
        Err(e) => eprintln!("[ERR] Error guardando: {}", e),
    }

    println!("══════════════════════════════════════════════════");
    println!("  Citybound Native — Sesión finalizada");
    println!("  Ticks simulados: {}", world.sim_tick);
    println!("  Entidades finales: {}", ecs::entity_count(&world));
    println!("  Tesorería: ${:.2}", world.finance.treasury);
    println!("══════════════════════════════════════════════════");
}

// ===== HELPERS =====

/// Convierte tecla minifb a GameKey
fn map_minifb_key(key: minifb::Key) -> Option<input::GameKey> {
    use input::GameKey;
    Some(match key {
        minifb::Key::Escape => GameKey::Escape,
        minifb::Key::Space => GameKey::Space,
        minifb::Key::Enter => GameKey::Enter,
        minifb::Key::Tab => GameKey::Tab,
        minifb::Key::Backspace => GameKey::Backspace,
        minifb::Key::W => GameKey::W,
        minifb::Key::A => GameKey::A,
        minifb::Key::S => GameKey::S,
        minifb::Key::D => GameKey::D,
        minifb::Key::Q => GameKey::Q,
        minifb::Key::E => GameKey::E,
        minifb::Key::R => GameKey::R,
        minifb::Key::F => GameKey::F,
        minifb::Key::G => GameKey::G,
        minifb::Key::Z => GameKey::Z,
        minifb::Key::X => GameKey::X,
        minifb::Key::C => GameKey::C,
        minifb::Key::V => GameKey::V,
        minifb::Key::B => GameKey::B,
        minifb::Key::T => GameKey::T,
        minifb::Key::Y => GameKey::Y,
        minifb::Key::U => GameKey::U,
        minifb::Key::I => GameKey::I,
        minifb::Key::O => GameKey::O,
        minifb::Key::P => GameKey::P,
        minifb::Key::Key1 => GameKey::Key1,
        minifb::Key::Key2 => GameKey::Key2,
        minifb::Key::Key3 => GameKey::Key3,
        minifb::Key::Key4 => GameKey::Key4,
        minifb::Key::Key5 => GameKey::Key5,
        minifb::Key::Key6 => GameKey::Key6,
        minifb::Key::Key7 => GameKey::Key7,
        minifb::Key::Key8 => GameKey::Key8,
        minifb::Key::Key9 => GameKey::Key9,
        minifb::Key::Key0 => GameKey::Key0,
        minifb::Key::Left => GameKey::Left,
        minifb::Key::Right => GameKey::Right,
        minifb::Key::Up => GameKey::Up,
        minifb::Key::Down => GameKey::Down,
        minifb::Key::PageUp => GameKey::PageUp,
        minifb::Key::PageDown => GameKey::PageDown,
        minifb::Key::Plus => GameKey::Plus,
        minifb::Key::Minus => GameKey::Minus,
        _ => return None,
    })
}