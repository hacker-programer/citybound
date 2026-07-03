// Platform Abstraction Layer v0.12.0
//
// Arquitectura universal: este módulo abstrae toda la funcionalidad
// específica de plataforma (windowing, input, renderizado) detrás de
// una interfaz unificada.
//
// BACKENDS:
// - Desktop (Windows/macOS/Linux): minifb (actual), winit+softbuffer (próximo)
// - Android: winit + android-activity + ANativeWindow (próximo)
//
// Para añadir una nueva plataforma:
// 1. Implementar PlatformBackend trait
// 2. Añadir feature flag en Cargo.toml
// 3. Seleccionar backend en tiempo de compilación con #[cfg(...)]

// ---------------------------------------------------------------------------
// EVENTOS DE PLATAFORMA UNIFICADOS
// ---------------------------------------------------------------------------

/// Evento de input unificado para todas las plataformas
/// NOTA: No derivamos Eq porque contiene f32
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlatformEvent {
    KeyPressed(PlatformKey),
    KeyReleased(PlatformKey),
    MouseMoved { x: f32, y: f32 },
    MouseDown(MouseButton),
    MouseUp(MouseButton),
    MouseWheel(f32),
    TouchBegan { id: u64, x: f32, y: f32 },
    TouchMoved { id: u64, x: f32, y: f32 },
    TouchEnded { id: u64 },
    Resized { width: u32, height: u32 },
    Focused(bool),
    CloseRequested,
}

/// Teclas de plataforma unificadas (mapeo universal)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlatformKey {
    // Letras
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    // Números (estilo Key0..Key9 para coincidir con input.rs)
    Key0, Key1, Key2, Key3, Key4, Key5, Key6, Key7, Key8, Key9,
    // Función
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    // Navegación
    Up, Down, Left, Right,
    // Modificadores
    LShift, RShift, LCtrl, RCtrl, LAlt, RAlt,
    // Especiales
    Escape, Enter, Space, Backspace, Tab, Delete, Home, End, PageUp, PageDown,
    // Símbolos
    Minus, Equals, BracketLeft, BracketRight,
    // Touch virtual (Android)
    Back, Menu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left, Right, Middle,
}

// ---------------------------------------------------------------------------
// TRAIT DE BACKEND DE PLATAFORMA
// ---------------------------------------------------------------------------

/// Trait que debe implementar cada backend de plataforma.
/// Añadir un nuevo SO solo requiere implementar este trait.
pub trait PlatformBackend {
    fn new(title: &str, width: u32, height: u32) -> Self;
    fn is_open(&self) -> bool;
    fn poll_events(&mut self) -> Vec<PlatformEvent>;
    fn present_frame(&mut self, buffer: &[u32], width: u32, height: u32);
    fn set_title(&mut self, title: &str);
    fn inner_size(&self) -> (u32, u32);
    fn scale_factor(&self) -> f64;
    fn set_cursor_visible(&mut self, visible: bool);
}

// ---------------------------------------------------------------------------
// BACKEND DE ESCRITORIO (minifb - Windows/macOS/Linux)
// ---------------------------------------------------------------------------

#[cfg(not(target_os = "android"))]
mod desktop {
    use super::*;
    use minifb::{Key, MouseButton as MfMouseButton, MouseMode, Window, WindowOptions, Scale, ScaleMode};

    pub struct DesktopBackend {
        window: Window,
        width: u32,
        height: u32,
        event_queue: Vec<PlatformEvent>,
    }

    impl PlatformBackend for DesktopBackend {
        fn new(title: &str, width: u32, height: u32) -> Self {
            let mut window = Window::new(
                title,
                width as usize,
                height as usize,
                WindowOptions {
                    scale: Scale::X2,
                    scale_mode: ScaleMode::AspectRatioStretch,
                    ..WindowOptions::default()
                },
            ).expect("No se pudo crear la ventana.");

            window.set_target_fps(60);

            DesktopBackend {
                window,
                width,
                height,
                event_queue: Vec::with_capacity(32),
            }
        }

        fn is_open(&self) -> bool {
            self.window.is_open()
        }

        fn poll_events(&mut self) -> Vec<PlatformEvent> {
            self.event_queue.clear();

            // Teclas presionadas — get_keys() devuelve Vec<Key>, iteramos directamente
            let keys: Vec<Key> = self.window.get_keys();
            for key in &keys {
                self.event_queue.push(PlatformEvent::KeyPressed(map_key(*key)));
            }

            // Mouse
            if let Some((x, y)) = self.window.get_mouse_pos(minifb::MouseMode::Clamp) {
                self.event_queue.push(PlatformEvent::MouseMoved { x: x as f32, y: y as f32 });
            }

            if self.window.get_mouse_down(MfMouseButton::Left) {
                self.event_queue.push(PlatformEvent::MouseDown(MouseButton::Left));
            }
            if self.window.get_mouse_down(MfMouseButton::Right) {
                self.event_queue.push(PlatformEvent::MouseDown(MouseButton::Right));
            }

            // Scroll
            if let Some(scroll) = self.window.get_scroll_wheel() {
                if scroll.1 != 0.0 {
                    self.event_queue.push(PlatformEvent::MouseWheel(scroll.1));
                }
            }

            self.event_queue.clone()
        }

        fn present_frame(&mut self, buffer: &[u32], width: u32, height: u32) {
            self.window.update_with_buffer(buffer, width as usize, height as usize)
                .expect("Error al actualizar ventana");
        }

        fn set_title(&mut self, title: &str) {
            self.window.set_title(title);
        }

        fn inner_size(&self) -> (u32, u32) {
            (self.width, self.height)
        }

        fn scale_factor(&self) -> f64 {
            2.0 // Scale::X2
        }

        fn set_cursor_visible(&mut self, visible: bool) {
            self.window.set_cursor_visibility(visible);
        }
    }

    fn map_key(key: Key) -> PlatformKey {
        match key {
            Key::A => PlatformKey::A, Key::B => PlatformKey::B, Key::C => PlatformKey::C,
            Key::D => PlatformKey::D, Key::E => PlatformKey::E, Key::F => PlatformKey::F,
            Key::G => PlatformKey::G, Key::H => PlatformKey::H, Key::I => PlatformKey::I,
            Key::J => PlatformKey::J, Key::K => PlatformKey::K, Key::L => PlatformKey::L,
            Key::M => PlatformKey::M, Key::N => PlatformKey::N, Key::O => PlatformKey::O,
            Key::P => PlatformKey::P, Key::Q => PlatformKey::Q, Key::R => PlatformKey::R,
            Key::S => PlatformKey::S, Key::T => PlatformKey::T, Key::U => PlatformKey::U,
            Key::V => PlatformKey::V, Key::W => PlatformKey::W, Key::X => PlatformKey::X,
            Key::Y => PlatformKey::Y, Key::Z => PlatformKey::Z,
            Key::Key0 => PlatformKey::Key0, Key::Key1 => PlatformKey::Key1,
            Key::Key2 => PlatformKey::Key2, Key::Key3 => PlatformKey::Key3,
            Key::Key4 => PlatformKey::Key4, Key::Key5 => PlatformKey::Key5,
            Key::Key6 => PlatformKey::Key6, Key::Key7 => PlatformKey::Key7,
            Key::Key8 => PlatformKey::Key8, Key::Key9 => PlatformKey::Key9,
            Key::F1 => PlatformKey::F1, Key::F2 => PlatformKey::F2,
            Key::F3 => PlatformKey::F3, Key::F4 => PlatformKey::F4,
            Key::F5 => PlatformKey::F5, Key::F6 => PlatformKey::F6,
            Key::F7 => PlatformKey::F7, Key::F8 => PlatformKey::F8,
            Key::F9 => PlatformKey::F9, Key::F10 => PlatformKey::F10,
            Key::F11 => PlatformKey::F11, Key::F12 => PlatformKey::F12,
            Key::Up => PlatformKey::Up, Key::Down => PlatformKey::Down,
            Key::Left => PlatformKey::Left, Key::Right => PlatformKey::Right,
            Key::LeftShift => PlatformKey::LShift, Key::RightShift => PlatformKey::RShift,
            Key::LeftCtrl => PlatformKey::LCtrl, Key::RightCtrl => PlatformKey::RCtrl,
            Key::LeftAlt => PlatformKey::LAlt, Key::RightAlt => PlatformKey::RAlt,
            Key::Escape => PlatformKey::Escape, Key::Enter => PlatformKey::Enter,
            Key::Space => PlatformKey::Space, Key::Backspace => PlatformKey::Backspace,
            Key::Tab => PlatformKey::Tab, Key::Delete => PlatformKey::Delete,
            Key::Home => PlatformKey::Home, Key::End => PlatformKey::End,
            Key::PageUp => PlatformKey::PageUp, Key::PageDown => PlatformKey::PageDown,
            // Símbolos
            Key::Minus => PlatformKey::Minus,
            Key::Equal => PlatformKey::Equals,
            Key::LeftBracket => PlatformKey::BracketLeft,
            Key::RightBracket => PlatformKey::BracketRight,
            _ => PlatformKey::Escape, // fallback
        }
    }
}

// ---------------------------------------------------------------------------
// BACKEND DE ANDROID (stub - android-activity + ndk)
// ---------------------------------------------------------------------------

#[cfg(target_os = "android")]
mod android {
    use super::*;

    pub struct AndroidBackend {
        width: u32,
        height: u32,
        is_open: bool,
        event_queue: Vec<PlatformEvent>,
    }

    impl PlatformBackend for AndroidBackend {
        fn new(title: &str, width: u32, height: u32) -> Self {
            AndroidBackend {
                width,
                height,
                is_open: true,
                event_queue: Vec::with_capacity(32),
            }
        }

        fn is_open(&self) -> bool {
            self.is_open
        }

        fn poll_events(&mut self) -> Vec<PlatformEvent> {
            self.event_queue.clone()
        }

        fn present_frame(&mut self, buffer: &[u32], width: u32, height: u32) {
            // TODO: Blit al ANativeWindow mediante ndk
        }

        fn set_title(&mut self, _title: &str) {}
        fn inner_size(&self) -> (u32, u32) { (self.width, self.height) }
        fn scale_factor(&self) -> f64 { 1.0 }
        fn set_cursor_visible(&mut self, _visible: bool) {}
    }
}

// ---------------------------------------------------------------------------
// WINDOW SYSTEM (fachada unificada)
// ---------------------------------------------------------------------------

/// Sistema de ventana unificado que selecciona el backend según la plataforma
pub struct WindowSystem {
    #[cfg(not(target_os = "android"))]
    backend: desktop::DesktopBackend,
    #[cfg(target_os = "android")]
    backend: android::AndroidBackend,
}

impl WindowSystem {
    pub fn new(title: &str, width: u32, height: u32) -> Self {
        #[cfg(not(target_os = "android"))]
        { WindowSystem { backend: desktop::DesktopBackend::new(title, width, height) } }
        #[cfg(target_os = "android")]
        { WindowSystem { backend: android::AndroidBackend::new(title, width, height) } }
    }

    #[inline(always)]
    pub fn is_open(&self) -> bool {
        self.backend.is_open()
    }

    #[inline(always)]
    pub fn poll_events(&mut self) -> Vec<PlatformEvent> {
        self.backend.poll_events()
    }

    #[inline(always)]
    pub fn present_frame(&mut self, buffer: &[u32], width: u32, height: u32) {
        self.backend.present_frame(buffer, width, height);
    }

    #[inline(always)]
    pub fn set_title(&mut self, title: &str) {
        self.backend.set_title(title);
    }

    #[inline(always)]
    pub fn inner_size(&self) -> (u32, u32) {
        self.backend.inner_size()
    }
}

// ---------------------------------------------------------------------------
// INFORMACIÓN DE PLATAFORMA
// ---------------------------------------------------------------------------

pub fn platform_name() -> &'static str {
    if cfg!(target_os = "windows") { "Windows" }
    else if cfg!(target_os = "macos") { "macOS" }
    else if cfg!(target_os = "linux") { "Linux" }
    else if cfg!(target_os = "android") { "Android" }
    else { "Unknown" }
}

pub fn arch_name() -> &'static str {
    if cfg!(target_arch = "x86_64") { "x86_64" }
    else if cfg!(target_arch = "aarch64") { "ARM64" }
    else if cfg!(target_arch = "arm") { "ARM32" }
    else if cfg!(target_arch = "wasm32") { "WASM32" }
    else { "Unknown" }
}

// ---------------------------------------------------------------------------
// UTILIDADES DE OPTIMIZACIÓN POR PLATAFORMA
// ---------------------------------------------------------------------------

/// Retorna el tamaño de línea de caché L1 de la CPU objetivo
pub const fn cache_line_size() -> usize {
    if cfg!(target_arch = "x86_64") { 64 }
    else if cfg!(target_arch = "aarch64") { 128 }
    else { 64 }
}

/// Indica si SIMD está disponible en esta plataforma
pub const fn simd_available() -> bool {
    cfg!(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
    ))
}

/// Tamaño de stack recomendado para threads
pub const fn recommended_stack_size() -> usize {
    if cfg!(target_os = "android") {
        512 * 1024 // 512KB para Android (limitado)
    } else {
        8 * 1024 * 1024 // 8MB para desktop
    }
}
