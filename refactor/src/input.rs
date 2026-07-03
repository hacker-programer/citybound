// Input handling optimizado para el juego
//
// TÉCNICA COMÚN #1 (aplicaciones): Debounce y Throttle Radical en Inputs
// Los eventos de mouse/teclado se capturan una vez por frame,
// no en cada evento del sistema operativo.
//
// TÉCNICA COMÚN #19 (aplicaciones): Event Delegation
// Usamos bitfields para detectar múltiples teclas simultáneamente

#![allow(dead_code)]

use minifb::{Key, MouseButton as MinifbMouseButton, MouseMode, Window};

// Re-export para uso en otros módulos
pub use minifb::MouseButton;

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

// Mapeo de teclas comunes a índices de bitfield (máximo 128 teclas)
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
    F1 = 55, F2 = 56, F3 = 57, F4 = 58, F5 = 59,
    F6 = 62, F7 = 63, F8 = 64, F9 = 65, F10 = 66, F11 = 67, F12 = 68,
    BracketLeft = 60,
    BracketRight = 61,
}