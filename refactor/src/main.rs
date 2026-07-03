// Citybound Native v0.15.0 — Punto de entrada principal
//
// Game loop cross-platform con minifb (desktop) + platform.rs (Android)
// Simulación a 10 ticks/s, renderizado a 30 FPS objetivo
//
// PLATAFORMAS: Windows, Android, macOS, Linux

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
            ..minifb::WindowOptions::default()
        },
    ) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Error creando ventana: {:?}", e);
            std::process::exit(1);
        }
    };

    window.set_fps_target(TARGET_FPS);
    )));

    // ===== FRAMEBUFFER =====
    let mut framebuffer: Vec<u32> = vec![0xFF_1A_1A_2Eu32; FB_SIZE];

    // ===== MUNDO DEL JUEGO =====
    let mut pool = object_pool::EntityPool::new(20000);
    let mut world = Box::new(ecs::create_world(&mut pool));

    // ===== ESTADO =====
    let mut input_state = input::InputState::default();
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
        // Resetear flancos
        input_state.keys_pressed = 0;
        input_state.keys_released = 0;

        // Capturar teclas presionadas
        for key in window.get_keys() {
            if let Some(gk) = map_minifb_key(key) {
                let bit = 1u128 << (gk as u8);
                if input_state.keys_down & bit == 0 {
                    input_state.keys_pressed |= bit;
                }
                input_state.keys_down |= bit;
            }
        }

        // Mouse
        if let Some((mx, my)) = window.get_mouse_pos(minifb::MouseMode::Clamp) {
            input_state.mouse_x = mx;
            input_state.mouse_y = my;
            input_state.mouse_inside = true;
        }
        input_state.mouse_left = window.get_mouse_down(minifb::MouseButton::Left);
        input_state.mouse_right = window.get_mouse_down(minifb::MouseButton::Right);

        // Scroll
        if let Some((_scroll_x, scroll_y)) = window.get_scroll_wheel() {
            input_state.scroll_delta = scroll_y;
        }

        // ---- PROCESAR INPUT DEL JUEGO ----
        ecs::process_input(&mut world, &input_state);

        // ---- SIMULACIÓN (PASO FIJO) ----
        sim_accumulator += dt.as_micros() as u64;
        while sim_accumulator >= MICROS_PER_TICK {
            sim_accumulator -= MICROS_PER_TICK;
            sim::tick(&mut world, MICROS_PER_TICK as f32 / 1_000_000.0);
        }

        // Renderizar mundo
        render::render_world_cached(
            &world,
            &mut framebuffer,
            WINDOW_WIDTH,
            WINDOW_HEIGHT,
        );

        // ---- ACTUALIZAR VENTANA ----
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
        minifb::Key::Delete => GameKey::Delete,
        minifb::Key::Left => GameKey::Left,
        minifb::Key::Right => GameKey::Right,
        minifb::Key::Up => GameKey::Up,
        minifb::Key::Down => GameKey::Down,
        minifb::Key::W => GameKey::W,
        minifb::Key::A => GameKey::A,
        minifb::Key::S => GameKey::S,
        minifb::Key::D => GameKey::D,
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
        minifb::Key::F1 => GameKey::F1,
        minifb::Key::F2 => GameKey::F2,
        minifb::Key::F3 => GameKey::F3,
        minifb::Key::F4 => GameKey::F4,
        minifb::Key::F5 => GameKey::F5,
        minifb::Key::F6 => GameKey::F6,
        minifb::Key::F7 => GameKey::F7,
        minifb::Key::F8 => GameKey::F8,
        minifb::Key::F9 => GameKey::F9,
        minifb::Key::F10 => GameKey::F10,
        minifb::Key::F11 => GameKey::F11,
        minifb::Key::F12 => GameKey::F12,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_keys() {
        assert_eq!(map_minifb_key(minifb::Key::W), Some(input::GameKey::W));
        assert_eq!(
            map_minifb_key(minifb::Key::Escape),
            Some(input::GameKey::Escape)
        );
    }

    #[test]
    fn test_constants_sane() {
        assert!(WINDOW_WIDTH > 0);
        assert!(WINDOW_HEIGHT > 0);
        assert!(FB_SIZE == WINDOW_WIDTH * WINDOW_HEIGHT);
        assert!(SIM_TICKS_PER_SECOND > 0);
        assert!(TARGET_FPS > 0);
    }
}
