// Input handling optimizado para el juego
//
// TÉCNICA COMÚN #1 (aplicaciones): Debounce y Throttle Radical en Inputs
// Los eventos de mouse/teclado se capturan una vez por frame,
// no en cada evento del sistema operativo.
//
// TÉCNICA COMÚN #19 (aplicaciones): Event Delegation
// Usamos bitfields para detectar múltiples teclas simultáneamente
//
// ARQUITECTURA DUAL:
// - Backend minifb (desktop Windows/macOS/Linux)
// - Backend platform (Android/alternativo via PlatformEvent)

#![allow(dead_code)]

use minifb::{Key, MouseButton as MinifbMouseButton, MouseMode, Window};

// Re-export para uso en otros módulos
pub use minifb::MouseButton;

// ---------------------------------------------------------------------------
// INPUT STATE (bitfield para caché-friendly)
// ---------------------------------------------------------------------------

/// Estado de input para un frame
#[derive(Clone, Debug, Default)]
pub struct InputState {
    /// Teclas presionadas en este frame (bitfield para hasta 128 teclas)
    pub keys_down: u128,
    /// Teclas que acaban de ser presionadas (flanco positivo)
    pub keys_pressed: u128,
    /// Teclas que acaban de ser soltadas (flanco negativo)
    pub keys_released: u128,
    /// Posición del mouse en coordenadas de ventana
    pub mouse_x: f32,
    pub mouse_y: f32,
    /// Botones del mouse presionados actualmente
    pub mouse_left: bool,
    pub mouse_right: bool,
    pub mouse_middle: bool,
    /// Botones del mouse en frame anterior (para detección de flancos)
    prev_mouse_left: bool,
    prev_mouse_right: bool,
    prev_mouse_middle: bool,
    /// Scroll vertical
    pub scroll_delta: f32,
    /// Si el mouse está dentro de la ventana
    pub mouse_inside: bool,
}

// ---------------------------------------------------------------------------
// GAME KEY ENUM (índices de bitfield, máximo 128)
// ---------------------------------------------------------------------------

/// Mapeo de teclas comunes a índices de bitfield
#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GameKey {
    Escape = 0,
    Space = 1,
    Enter = 2,
    Tab = 3,
    Backspace = 4,
    Delete = 5,
    Left = 6,
    Right = 7,
    Up = 8,
    Down = 9,
    Shift = 10,
    Control = 11,
    Alt = 12,
    Key1 = 13,
    Key2 = 14,
    Key3 = 15,
    Key4 = 16,
    Key5 = 17,
    Key6 = 18,
    Key7 = 19,
    Key8 = 20,
    Key9 = 21,
    Key0 = 22,
    A = 23, B = 24, C = 25, D = 26, E = 27,
    F = 28, G = 29, H = 30, I = 31, J = 32,
    K = 33, L = 34, M = 35, N = 36, O = 37,
    P = 38, Q = 39, R = 40, S = 41, T = 42,
    U = 43, V = 44, W = 45, X = 46, Y = 47, Z = 48,
    Minus = 49,
    Equals = 50,
    PageUp = 51,
    PageDown = 52,
    Home = 53,
    End = 54,
    F1 = 55, F2 = 56, F3 = 57, F4 = 58, F5 = 59,
    F6 = 62, F7 = 63, F8 = 64, F9 = 65, F10 = 66, F11 = 67, F12 = 68,
    BracketLeft = 60,
    BracketRight = 61,
}

// ---------------------------------------------------------------------------
// IMPL INPUT STATE
// ---------------------------------------------------------------------------

impl InputState {
    /// Crea un nuevo estado de input con todos los campos inicializados a cero
    pub fn new() -> Self {
        Self {
            keys_down: 0,
            keys_pressed: 0,
            keys_released: 0,
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_left: false,
            mouse_right: false,
            mouse_middle: false,
            prev_mouse_left: false,
            prev_mouse_right: false,
            prev_mouse_middle: false,
            scroll_delta: 0.0,
            mouse_inside: false,
        }
    }

    /// Procesa un evento de plataforma unificado (PlatformEvent → InputState)
    /// Procesa un evento de plataforma unificado (PlatformEvent → InputState)
    /// Permite que el sistema de input funcione con cualquier backend de plataforma
    pub fn process_platform_event(&mut self, event: &crate::platform::PlatformEvent) {
        use crate::platform::{PlatformEvent, MouseButton as PMb};

        match *event {
            PlatformEvent::KeyPressed(key) => {
                if let Some(gk) = map_platform_key_internal(key) {
                    self.keys_pressed |= 1u128 << (gk as u8);
                    self.keys_down |= 1u128 << (gk as u8);
                }
            }
            PlatformEvent::KeyReleased(key) => {
            }
            PlatformEvent::MouseMoved { x, y } => {
                self.mouse_x = x;
                self.mouse_y = y;
                self.mouse_inside = true;
            }
            PlatformEvent::MouseDown(button) => {
                self.prev_mouse_left = self.mouse_left;
                self.prev_mouse_right = self.mouse_right;
                self.prev_mouse_middle = self.mouse_middle;
                match button {
                    PMb::Left => self.mouse_left = true,
                    PMb::Right => self.mouse_right = true,
                    PMb::Middle => self.mouse_middle = true,
                }
            }
            PlatformEvent::MouseUp(button) => {
                self.prev_mouse_left = self.mouse_left;
                self.prev_mouse_right = self.mouse_right;
                self.prev_mouse_middle = self.mouse_middle;
                match button {
                    PMb::Left => self.mouse_left = false,
                    PMb::Right => self.mouse_right = false,
                    PMb::Middle => self.mouse_middle = false,
                }
            }
            PlatformEvent::MouseWheel(delta) => {
                self.scroll_delta = delta;
            }
            _ => {} // Touch events, resize, focus — ignorados por ahora
        }
    }

    /// Actualiza el estado de input desde la ventana minifb (una vez por frame)
    pub fn update(&mut self, window: &Window) {
        let prev_keys = self.keys_down;

        // Guardar estado previo del mouse
        self.prev_mouse_left = self.mouse_left;
        self.prev_mouse_right = self.mouse_right;
        self.prev_mouse_middle = self.mouse_middle;

        // Mouse
        if let Some((mx, my)) = window.get_mouse_pos(MouseMode::Clamp) {
            self.mouse_x = mx;
            self.mouse_y = my;
            self.mouse_inside = true;
        } else {
            self.mouse_inside = false;
        }

        self.mouse_left = window.get_mouse_down(MinifbMouseButton::Left);
        self.mouse_right = window.get_mouse_down(MinifbMouseButton::Right);
        self.mouse_middle = window.get_mouse_down(MinifbMouseButton::Middle);

        if let Some(scroll) = window.get_scroll_wheel() {
            self.scroll_delta = scroll.1;
        } else {
            self.scroll_delta = 0.0;
        }

        // Construir bitfield de teclas
        self.keys_down = 0;
        for (key, game_key) in KEY_MAP.iter() {
            if window.is_key_down(*key) {
                self.keys_down |= 1u128 << (*game_key as u8);
            }
        }

        // Detectar flancos
        self.keys_pressed = self.keys_down & !prev_keys;
        // Detectar flancos
        self.keys_pressed = self.keys_down & !prev_keys;
        self.keys_released = !self.keys_down & prev_keys;
    }

    /// Tecla presionada (mantenida)
    /// Tecla presionada (mantenida)
    #[inline(always)]
    pub fn is_key_down(&self, key: GameKey) -> bool {
        (self.keys_down & (1u128 << (key as u8))) != 0
    }

    /// Tecla recién presionada en este frame (flanco)
    #[inline(always)]
    pub fn is_key_pressed(&self, key: GameKey) -> bool {
        (self.keys_pressed & (1u128 << (key as u8))) != 0
    }
            MouseButton::Left => self.mouse_left,
            MouseButton::Right => self.mouse_right,
            MouseButton::Middle => self.mouse_middle,
        }
    }

    /// Botón del mouse recién presionado en este frame (flanco positivo)
    #[inline(always)]
    pub fn is_mouse_pressed(&self, button: MouseButton) -> bool {
        match button {
            MouseButton::Left => self.mouse_left && !self.prev_mouse_left,
            MouseButton::Right => self.mouse_right && !self.prev_mouse_right,
            MouseButton::Middle => self.mouse_middle && !self.prev_mouse_middle,
        }
    }

    /// Botón del mouse recién soltado en este frame (flanco negativo)
    #[inline(always)]
    pub fn is_mouse_released(&self, button: MouseButton) -> bool {
        match button {
            MouseButton::Left => !self.mouse_left && self.prev_mouse_left,
            MouseButton::Right => !self.mouse_right && self.prev_mouse_right,
            MouseButton::Middle => !self.mouse_middle && self.prev_mouse_middle,
        }
    }
}

// ---------------------------------------------------------------------------
// MAPEO DE TECLAS PLATFORM → GAMEKEY
// ---------------------------------------------------------------------------

/// Convierte PlatformKey a GameKey (para uso interno y cross-platform)
#[allow(dead_code)]
fn map_platform_key_internal(key: crate::platform::PlatformKey) -> Option<GameKey> {
    use crate::platform::PlatformKey as Pk;
    Some(match key {
        Pk::Escape => GameKey::Escape,
        Pk::Space => GameKey::Space,
        Pk::Enter => GameKey::Enter,
        Pk::Tab => GameKey::Tab,
        Pk::Backspace => GameKey::Backspace,
        Pk::Delete => GameKey::Delete,
        Pk::Left => GameKey::Left,
        Pk::Right => GameKey::Right,
        Pk::Up => GameKey::Up,
        Pk::Down => GameKey::Down,
        Pk::LShift | Pk::RShift => GameKey::Shift,
        Pk::LCtrl | Pk::RCtrl => GameKey::Control,
        Pk::LAlt | Pk::RAlt => GameKey::Alt,
        Pk::Key1 => GameKey::Key1,
        Pk::Key2 => GameKey::Key2,
        Pk::Key3 => GameKey::Key3,
        Pk::Key4 => GameKey::Key4,
        Pk::Key5 => GameKey::Key5,
        Pk::Key6 => GameKey::Key6,
        Pk::Key7 => GameKey::Key7,
        Pk::Key8 => GameKey::Key8,
        Pk::Key9 => GameKey::Key9,
        Pk::Key0 => GameKey::Key0,
        Pk::A => GameKey::A, Pk::B => GameKey::B, Pk::C => GameKey::C,
        Pk::D => GameKey::D, Pk::E => GameKey::E, Pk::F => GameKey::F,
        Pk::G => GameKey::G, Pk::H => GameKey::H, Pk::I => GameKey::I,
        Pk::J => GameKey::J, Pk::K => GameKey::K, Pk::L => GameKey::L,
        Pk::M => GameKey::M, Pk::N => GameKey::N, Pk::O => GameKey::O,
        Pk::P => GameKey::P, Pk::Q => GameKey::Q, Pk::R => GameKey::R,
        Pk::S => GameKey::S, Pk::T => GameKey::T, Pk::U => GameKey::U,
        Pk::V => GameKey::V, Pk::W => GameKey::W, Pk::X => GameKey::X,
        Pk::Y => GameKey::Y, Pk::Z => GameKey::Z,
        Pk::Minus => GameKey::Minus,
        Pk::Equals => GameKey::Equals,
        Pk::PageUp => GameKey::PageUp,
        Pk::PageDown => GameKey::PageDown,
        Pk::Home => GameKey::Home,
        Pk::End => GameKey::End,
        Pk::F1 => GameKey::F1, Pk::F2 => GameKey::F2, Pk::F3 => GameKey::F3,
        Pk::F4 => GameKey::F4, Pk::F5 => GameKey::F5, Pk::F6 => GameKey::F6,
        Pk::F7 => GameKey::F7, Pk::F8 => GameKey::F8, Pk::F9 => GameKey::F9,
        Pk::F10 => GameKey::F10, Pk::F11 => GameKey::F11, Pk::F12 => GameKey::F12,
        Pk::BracketLeft => GameKey::BracketLeft,
        Pk::BracketRight => GameKey::BracketRight,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// MAPA ESTÁTICO DE TECLAS MINIFB → GAMEKEY
// ---------------------------------------------------------------------------

static KEY_MAP: &[(Key, GameKey)] = &[
    (Key::Escape, GameKey::Escape),
    (Key::Space, GameKey::Space),
    (Key::Enter, GameKey::Enter),
    (Key::Tab, GameKey::Tab),
    (Key::Backspace, GameKey::Backspace),
    (Key::Delete, GameKey::Delete),
    (Key::Left, GameKey::Left),
    (Key::Right, GameKey::Right),
    (Key::Up, GameKey::Up),
    (Key::Down, GameKey::Down),
    (Key::LeftShift, GameKey::Shift),
    (Key::RightShift, GameKey::Shift),
    (Key::LeftCtrl, GameKey::Control),
    (Key::RightCtrl, GameKey::Control),
    (Key::LeftAlt, GameKey::Alt),
    (Key::RightAlt, GameKey::Alt),
    (Key::Key1, GameKey::Key1),
    (Key::Key2, GameKey::Key2),
    (Key::Key3, GameKey::Key3),
    (Key::Key4, GameKey::Key4),
    (Key::Key5, GameKey::Key5),
    (Key::Key6, GameKey::Key6),
    (Key::Key7, GameKey::Key7),
    (Key::Key8, GameKey::Key8),
    (Key::Key9, GameKey::Key9),
    (Key::Key0, GameKey::Key0),
    (Key::A, GameKey::A), (Key::B, GameKey::B), (Key::C, GameKey::C),
    (Key::D, GameKey::D), (Key::E, GameKey::E), (Key::F, GameKey::F),
    (Key::G, GameKey::G), (Key::H, GameKey::H), (Key::I, GameKey::I),
    (Key::J, GameKey::J), (Key::K, GameKey::K), (Key::L, GameKey::L),
    (Key::M, GameKey::M), (Key::N, GameKey::N), (Key::O, GameKey::O),
    (Key::P, GameKey::P), (Key::Q, GameKey::Q), (Key::R, GameKey::R),
    (Key::S, GameKey::S), (Key::T, GameKey::T), (Key::U, GameKey::U),
    (Key::V, GameKey::V), (Key::W, GameKey::W), (Key::X, GameKey::X),
    (Key::Y, GameKey::Y), (Key::Z, GameKey::Z),
    (Key::Minus, GameKey::Minus),
    (Key::Equal, GameKey::Equals),
    (Key::PageUp, GameKey::PageUp),
    (Key::PageDown, GameKey::PageDown),
    (Key::Home, GameKey::Home),
    (Key::End, GameKey::End),
    (Key::F1, GameKey::F1), (Key::F2, GameKey::F2), (Key::F3, GameKey::F3),
    (Key::F4, GameKey::F4), (Key::F5, GameKey::F5), (Key::F6, GameKey::F6),
    (Key::F7, GameKey::F7), (Key::F8, GameKey::F8), (Key::F9, GameKey::F9),
    (Key::F10, GameKey::F10), (Key::F11, GameKey::F11), (Key::F12, GameKey::F12),
    (Key::LeftBracket, GameKey::BracketLeft),
    (Key::RightBracket, GameKey::BracketRight),
];

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_state_default() {
        let state = InputState::default();
        assert_eq!(state.keys_down, 0);
        assert_eq!(state.keys_pressed, 0);
        assert_eq!(state.keys_released, 0);
        assert!(!state.mouse_left);
        assert!(!state.mouse_right);
    }

    #[test]
    fn test_key_down() {
        let mut state = InputState::default();
        state.keys_down = 1u128 << (GameKey::W as u8);
        assert!(state.is_key_down(GameKey::W));
        assert!(!state.is_key_down(GameKey::S));
    }

    #[test]
    fn test_key_pressed() {
        let mut state = InputState::default();
        state.keys_pressed = 1u128 << (GameKey::Space as u8);
        assert!(state.is_key_pressed(GameKey::Space));
        assert!(!state.is_key_pressed(GameKey::Enter));
    }

    #[test]
    fn test_key_released() {
        let mut state = InputState::default();
        state.keys_released = 1u128 << (GameKey::Escape as u8);
        assert!(state.is_key_released(GameKey::Escape));
        assert!(!state.is_key_released(GameKey::Tab));
    }

    #[test]
    fn test_mouse_methods() {
        let mut state = InputState::default();

        // Simular click
        state.prev_mouse_left = false;
        state.mouse_left = true;
        assert!(state.is_mouse_pressed(MouseButton::Left));
        assert!(state.is_mouse_down(MouseButton::Left));
        assert!(!state.is_mouse_released(MouseButton::Left));

        // Simular release
        state.prev_mouse_left = true;
        state.mouse_left = false;
        assert!(!state.is_mouse_pressed(MouseButton::Left));
        assert!(!state.is_mouse_down(MouseButton::Left));
        assert!(state.is_mouse_released(MouseButton::Left));
    }

    #[test]
    fn test_game_key_values() {
        assert_eq!(GameKey::Escape as u8, 0);
        assert_eq!(GameKey::W as u8, 45);
        assert_eq!(GameKey::BracketLeft as u8, 60);
        assert_eq!(GameKey::BracketRight as u8, 61);
    }

    #[test]
    fn test_fkeys() {
        assert_eq!(GameKey::F5 as u8, 59);
        assert_eq!(GameKey::F9 as u8, 65);
        assert_eq!(GameKey::F12 as u8, 68);
    }
}
