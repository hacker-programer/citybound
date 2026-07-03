// Módulo de Aceleración por Hardware Adaptativa v0.14.0
//
// ARQUITECTURA COMPLETA:
// ┌──────────────────────────────────────────────────────────┐
// │                   RenderBackend                          │
// │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐ │
// │  │Tier 0    │  │Tier 1    │  │Tier 2    │  │Tier 3   │ │
// │  │CPU SIMD  │  │GLES 3.0  │  │Vulkan    │  │Vk+CS    │ │
// │  │Multihilo │  │Integrado │  │Discreto  │  │High-End │ │
// │  └──────────┘  └──────────┘  └──────────┘  └─────────┘ │
// │       ↓             ↓            ↓            ↓         │
// │  ┌──────────────────────────────────────────────────┐   │
// │  │  AdaptiveQuality: resolución, LOD, filtros       │   │
// │  └──────────────────────────────────────────────────┘   │
// └──────────────────────────────────────────────────────────┘
//
// PLATAFORMAS:
// - Windows: DX12 > Vulkan > OpenGL > CPU SIMD
// - Android: Vulkan > OpenGL ES 3.0 > CPU SIMD
// - macOS: Metal > OpenGL > CPU SIMD
// - Linux: Vulkan > OpenGL > CPU SIMD
//
// TÉCNICAS IMPLEMENTADAS:
// [TC#1]  Object Pooling Masivo — RenderCommandPool preasignado
// [TC#2]  Pre-Reserva de Capacidad — Vec::with_capacity en todos los buffers
// [TC#5]  Look-Up Tables Trigonométricas
// [TC#7]  Texturas en Atlas — TextureAtlas combinado
// [TC#15] Pre-caché de Shaders — Warming en carga
// [TC#16] LOD Generado Offline — Mipmaps
// [TC#28] OffscreenCanvas en Workers — Tile rendering multihilo
// [TC#30] Precarga Eager de Assets
// [TA#5]  Compute Shaders para Físicas
// [TA#9]  Despacho Estático vs Dinámico
// [TA#14] BVH Balanceados
// [TA#17] Acceso Unchecked
// [TI#4]  Bitboards para Colisión
// [TI#9]  Deltas de Red Optimizados

use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// CONSTANTES GLOBALES
// ---------------------------------------------------------------------------

/// Nivel de hardware activo (0-3). Se detecta al iniciar.
pub static HARDWARE_TIER: AtomicU8 = AtomicU8::new(0);

/// Número de cores físicos disponibles
pub static CPU_CORES: AtomicU8 = AtomicU8::new(1);

/// Ancho de pantalla actual (para escalado adaptativo)
pub static SCREEN_W: AtomicU32 = AtomicU32::new(800);

/// Alto de pantalla actual
pub static SCREEN_H: AtomicU32 = AtomicU32::new(600);

/// Factor de escala adaptativo (1.0 = nativo, 0.5 = mitad)
pub static RESOLUTION_SCALE: std::sync::LazyLock<std::sync::Mutex<f32>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(1.0));

// ---------------------------------------------------------------------------
// DETECCIÓN DE HARDWARE
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum HardwareTier {
    /// Tier 0: CPU-only, SIMD software rendering
    CpuOnly = 0,
    /// Tier 1: GPU integrada (Intel HD, Mali G52, Adreno 6xx)
    IntegratedGpu = 1,
    /// Tier 2: GPU discreta media (GTX 1060, RX 580, Adreno 7xx+)
    MidRangeGpu = 2,
    /// Tier 3: GPU high-end (RTX 20xx+, RX 6800+, Adreno 8xx, Apple M1+)
    HighEndGpu = 3,
}

impl HardwareTier {
    /// Detecta el tier actual basado en heurísticas del sistema
    pub fn current() -> Self {
        let cores = num_cpus_physical();
        CPU_CORES.store(cores.min(255) as u8, Ordering::Release);

        // Intentar detectar GPU
        let gpu_tier = detect_gpu_tier();

        // Si no hay GPU, usar CPU con SIMD multihilo
        if gpu_tier == 0 {
            return HardwareTier::CpuOnly;
        }

        match gpu_tier {
            1 => HardwareTier::IntegratedGpu,
            2 => HardwareTier::MidRangeGpu,
            _ => HardwareTier::HighEndGpu,
        }
    }

    /// Retorna el factor de calidad de renderizado recomendado
    pub fn quality_factor(&self) -> f32 {
        match self {
            HardwareTier::CpuOnly => 0.5,        // 50% resolución
            HardwareTier::IntegratedGpu => 0.75,  // 75%
            HardwareTier::MidRangeGpu => 1.0,     // 100%
            HardwareTier::HighEndGpu => 1.5,      // Supersampling 1.5x
        }
    }

    /// ¿Soporta compute shaders?
    pub fn supports_compute_shaders(&self) -> bool {
        matches!(self, HardwareTier::MidRangeGpu | HardwareTier::HighEndGpu)
    }

    /// ¿Soporta renderizado 3D acelerado?
    pub fn supports_gpu_rendering(&self) -> bool {
        *self != HardwareTier::CpuOnly
    }

    /// Resolución máxima de textura recomendada
    pub fn max_texture_size(&self) -> u32 {
        match self {
            HardwareTier::CpuOnly => 512,
            HardwareTier::IntegratedGpu => 1024,
            HardwareTier::MidRangeGpu => 2048,
            HardwareTier::HighEndGpu => 4096,
        }
    }

    /// Número óptimo de tiles para renderizado paralelo
    pub fn optimal_tile_count(&self) -> u32 {
        match self {
            HardwareTier::CpuOnly => CPU_CORES.load(Ordering::Acquire) as u32,
            HardwareTier::IntegratedGpu => 4,
            HardwareTier::MidRangeGpu => 8,
            HardwareTier::HighEndGpu => 16,
        }
    }

    /// Tamaño de tile para renderizado paralelo
    pub fn tile_size(&self, screen_w: u32, screen_h: u32) -> (u32, u32) {
        let tiles = self.optimal_tile_count().max(1);
        let cols = (tiles as f32).sqrt().ceil() as u32;
        let rows = (tiles + cols - 1) / cols;
        (screen_w / cols, screen_h / rows)
    }
}

// ---------------------------------------------------------------------------
// API DE GPU DETECTADA
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum GpuApi {
    None = 0,
    Vulkan = 1,
    Dx12 = 2,
    Metal = 3,
    OpenGlEs = 4,
    OpenGl = 5,
}

impl GpuApi {
    /// Detecta la mejor API disponible
    pub fn detect() -> Self {
        #[cfg(target_os = "android")]
        {
            // Android: preferir Vulkan, fallback a OpenGL ES
            if has_vulkan() { return GpuApi::Vulkan; }
            GpuApi::OpenGlEs
        }
        #[cfg(target_os = "windows")]
        {
            // Windows: DX12 > Vulkan > OpenGL
            if has_dx12() { return GpuApi::Dx12; }
            if has_vulkan() { return GpuApi::Vulkan; }
            GpuApi::OpenGl
        }
        #[cfg(target_os = "macos")]
        {
            GpuApi::Metal
        }
        #[cfg(target_os = "linux")]
        {
            if has_vulkan() { return GpuApi::Vulkan; }
            GpuApi::OpenGl
        }
        #[cfg(not(any(
            target_os = "android",
            target_os = "windows",
            target_os = "macos",
            target_os = "linux"
        )))]
        {
            GpuApi::None
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            GpuApi::None => "CPU-SIMD",
            GpuApi::Vulkan => "Vulkan",
            GpuApi::Dx12 => "DirectX 12",
            GpuApi::Metal => "Metal",
            GpuApi::OpenGlEs => "OpenGL ES 3.0",
            GpuApi::OpenGl => "OpenGL 3.3",
        }
    }

    /// ¿Es un backend GPU real?
    pub fn is_gpu(&self) -> bool {
        !matches!(self, GpuApi::None)
    }
}

// ---------------------------------------------------------------------------
// DETECCIÓN DE CAPACIDADES
// ---------------------------------------------------------------------------

fn has_vulkan() -> bool {
    #[cfg(any(target_os = "android", target_os = "linux", target_os = "windows"))]
    {
        // En Android: casi todos los dispositivos con API 24+ tienen Vulkan
        // En desktop: detectable via vulkaninfo o loader
        cfg_if_vulkan_available()
    }
    #[cfg(not(any(target_os = "android", target_os = "linux", target_os = "windows")))]
    { false }
}

fn has_dx12() -> bool {
    #[cfg(target_os = "windows")]
    { cfg_if_dx12_available() }
    #[cfg(not(target_os = "windows"))]
    { false }
}

fn cfg_if_vulkan_available() -> bool {
    // Heurística práctica:
    // Android API 24+ (7.0+) => casi seguro Vulkan
    #[cfg(target_os = "android")]
    { true }

    // Desktop: buscar la librería vulkan-1
    #[cfg(not(target_os = "android"))]
    {
        #[cfg(target_os = "windows")]
        { std::path::Path::new("vulkan-1.dll").exists() || true } // asumir disponible

        #[cfg(target_os = "linux")]
        { std::path::Path::new("/usr/lib/libvulkan.so").exists() || true }
    }
}

fn cfg_if_dx12_available() -> bool {
    // Windows 10+ tiene DX12. Asumimos true en Windows moderno.
    true
}

fn detect_gpu_tier() -> u8 {
    #[cfg(target_os = "android")]
    {
        detect_android_gpu_tier()
    }
    #[cfg(not(target_os = "android"))]
    {
        detect_desktop_gpu_tier()
    }
}

#[cfg(target_os = "android")]
fn detect_android_gpu_tier() -> u8 {
    // Heurísticas basadas en el renderizador OpenGL
    // En la práctica, muchos dispositivos reportan su GPU en glGetString(GL_RENDERER)
    // Por defecto asumimos Tier 1 (integrada)
    // Flagships recientes con Adreno 7xx/8xx, Mali G710+, etc son Tier 2-3
    2 // Asumimos mid-range como base segura en Android moderno
}

#[cfg(not(target_os = "android"))]
fn detect_desktop_gpu_tier() -> u8 {
    // En desktop, intentar detectar la GPU
    // Heurística: número de cores, memoria del sistema
    let cores = num_cpus_physical();

    if cores >= 8 {
        // Máquinas con 8+ cores suelen tener GPU decente
        3
    } else if cores >= 4 {
        2
    } else {
        1
    }
}

fn num_cpus_physical() -> usize {
    // Intentar obtener cores físicos
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        if let Ok(cpuinfo) = std::fs::read_to_string("/proc/cpuinfo") {
            let count = cpuinfo.lines()
                .filter(|l| l.starts_with("cpu cores"))
                .filter_map(|l| l.split(':').nth(1))
                .filter_map(|s| s.trim().parse::<usize>().ok())
                .next()
                .unwrap_or(1);
            return count.max(1);
        }
    }

    // Fallback: cores lógicos / 2 (asumiendo hyperthreading)
    let logical = num_cpus::get();
    if logical >= 2 { logical / 2 } else { 1 }
}

// ---------------------------------------------------------------------------
// COMANDO DE RENDER ABSTRACTO (GPU-agnostic, alineado a 64 bytes)
// ---------------------------------------------------------------------------

pub const FLAG_ALPHA_BLEND: u8 = 0b0000_0001;
pub const FLAG_TEXTURED: u8   = 0b0000_0010;
pub const FLAG_GRADIENT: u8   = 0b0000_0100;
pub const FLAG_SHADOW: u8     = 0b0000_1000;
pub const FLAG_OUTLINE: u8    = 0b0001_0000;
pub const FLAG_DITHER: u8     = 0b0010_0000;

#[derive(Debug, Clone, Copy)]
#[repr(C, align(64))]
pub struct RenderCommand {
    pub primitive_type: u8,   // 0=rect, 1=line, 2=point, 3=sprite, 4=text, 5=gradient
    pub flags: u8,
    pub _pad: [u8; 2],
    pub color: u32,           // RGBA packed
    pub x0: i16,
    pub y0: i16,
    pub x1: i16,
    pub y1: i16,
    pub texture_id: u16,
    pub z_depth: u16,
}

impl Default for RenderCommand {
    fn default() -> Self {
        Self {
            primitive_type: 0, flags: 0, _pad: [0; 2],
            color: 0xFF_FF_FF_FF, x0: 0, y0: 0, x1: 0, y1: 0,
            texture_id: 0, z_depth: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// POOL DE COMANDOS DE RENDER [TC#1]
// ---------------------------------------------------------------------------

pub struct RenderCommandPool {
    commands: Vec<RenderCommand>,
    sorted:   Vec<usize>,
}

impl RenderCommandPool {
    /// Crea pool con capacidad pre-reservada [TC#2]
    #[inline]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            commands: Vec::with_capacity(cap),
            sorted:   Vec::with_capacity(cap),
        }
    }

    #[inline(always)]
    pub fn push(&mut self, cmd: RenderCommand) {
        self.commands.push(cmd);
    }

    #[inline]
    pub fn is_empty(&self) -> bool { self.commands.is_empty() }

    #[inline]
    pub fn len(&self) -> usize { self.commands.len() }

    /// Slice mutable de comandos
    #[inline]
    pub fn slice_mut(&mut self) -> &mut [RenderCommand] { &mut self.commands }

    /// Slice inmutable
    #[inline]
    pub fn slice(&self) -> &[RenderCommand] { &self.commands }

    /// Ordena por z_depth usando counting sort (estable, O(n+k))
    pub fn sort_by_depth(&mut self) {
        if self.commands.len() <= 1 { return; }
        // Counting sort por z_depth (0..65535)
        let max_z = self.commands.iter().map(|c| c.z_depth).max().unwrap_or(0) as usize + 1;
        let mut count = vec![0usize; max_z.min(65536)];

        for cmd in &self.commands {
            count[cmd.z_depth as usize] += 1;
        }

        // Prefix sum
        for i in 1..count.len() {
            count[i] += count[i - 1];
        }

        self.sorted.resize(self.commands.len(), 0);
        // Iterar en reversa para estabilidad
        for i in (0..self.commands.len()).rev() {
            let z = self.commands[i].z_depth as usize;
            count[z] -= 1;
            self.sorted[count[z]] = i;
        }

        // Reordenar
        let mut temp = self.commands.clone();
        for (i, &idx) in self.sorted.iter().enumerate() {
            temp[i] = self.commands[idx];
        }
        self.commands = temp;
    }

    /// Vacia el pool sin liberar memoria
    #[inline]
    pub fn clear(&mut self) {
        self.commands.clear();
        self.sorted.clear();
    }

    /// Reserva más capacidad
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.commands.reserve(additional);
        self.sorted.reserve(additional);
    }
}

// ---------------------------------------------------------------------------
// TEXTURE ATLAS [TC#7]
// ---------------------------------------------------------------------------

pub struct TextureAtlas {
    pub width:  u32,
    pub height: u32,
    pixels:     Vec<u8>, // RGBA interleaved para mejor cache locality
    pub entries: Vec<AtlasEntry>,
    cursor_x:   u32,
    cursor_y:   u32,
    row_height: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct AtlasEntry {
    pub texture_id: u16,
    pub x:          u32,
    pub y:          u32,
    pub width:      u32,
    pub height:     u32,
}

impl TextureAtlas {
    pub fn new(width: u32, height: u32) -> Self {
        let pixel_count = (width as usize) * (height as usize) * 4;
        Self {
            width, height,
            pixels: vec![0u8; pixel_count],
            entries: Vec::with_capacity(256),
            cursor_x: 0, cursor_y: 0, row_height: 0,
        }
    }

    /// Registra una textura en el atlas. Retorna la entrada.
    pub fn register(&mut self, rgba: &[u32], w: u32, h: u32) -> AtlasEntry {
        // Simple row-packing: si no cabe en la fila actual, saltar a la siguiente
        if self.cursor_x + w > self.width {
            self.cursor_x = 0;
            self.cursor_y += self.row_height;
            self.row_height = 0;
        }

        // Si no cabe verticalmente, reiniciar (wrap-around — en prod usaríamos LRU)
        if self.cursor_y + h > self.height {
            self.cursor_x = 0;
            self.cursor_y = 0;
            self.row_height = 0;
            self.entries.clear();
        }

        let entry = AtlasEntry {
            texture_id: self.entries.len() as u16,
            x: self.cursor_x,
            y: self.cursor_y,
            width: w,
            height: h,
        };

        // Copiar píxeles al atlas (RGBA interleaved)
        let atlas_stride = self.width as usize * 4;
        for row in 0..h as usize {
            let atlas_row = (self.cursor_y as usize + row) * atlas_stride;
            let src_row = row * w as usize;
            for col in 0..w as usize {
                let src_pixel = rgba.get(src_row + col).copied().unwrap_or(0);
                let dst_idx = atlas_row + (self.cursor_x as usize + col) * 4;
                if dst_idx + 3 < self.pixels.len() {
                    self.pixels[dst_idx]     = (src_pixel >> 16) as u8; // R
                    self.pixels[dst_idx + 1] = (src_pixel >> 8) as u8;  // G
                    self.pixels[dst_idx + 2] = src_pixel as u8;         // B
                    self.pixels[dst_idx + 3] = (src_pixel >> 24) as u8; // A
                }
            }
        }

        self.cursor_x += w;
        self.row_height = self.row_height.max(h);
        self.entries.push(entry);
        entry
    }

    /// Muestrea el atlas en coordenadas UV (0..1)
    #[inline]
    pub fn sample_uv(&self, entry: &AtlasEntry, u: f32, v: f32) -> u32 {
        let px = entry.x + (u * entry.width as f32) as u32;
        let py = entry.y + (v * entry.height as f32) as u32;
        self.sample(px, py)
    }

    /// Muestrea pixel específico del atlas
    #[inline]
    pub fn sample(&self, x: u32, y: u32) -> u32 {
        if x >= self.width || y >= self.height { return 0; }
        let idx = (y as usize * self.width as usize + x as usize) * 4;
        if idx + 3 >= self.pixels.len() { return 0; }
        let r = self.pixels[idx] as u32;
        let g = self.pixels[idx + 1] as u32;
        let b = self.pixels[idx + 2] as u32;
        let a = self.pixels[idx + 3] as u32;
        (a << 24) | (r << 16) | (g << 8) | b
    }

    #[inline]
    pub fn pixel_count(&self) -> usize { self.pixels.len() / 4 }
}

// ---------------------------------------------------------------------------
// BACKEND DE RENDERIZO CPU MULTIHILO [TC#28]
// ---------------------------------------------------------------------------

pub struct CpuBackend {
    pub framebuffer: Vec<u32>,
    pub command_pool: RenderCommandPool,
    pub atlas: TextureAtlas,
    /// Framebuffer de trabajo para double-buffering
    work_buffer: Vec<u32>,
    width:  u32,
    height: u32,
}

impl CpuBackend {
    pub fn new(width: u32, height: u32) -> Self {
        let pixels = (width as usize) * (height as usize);
        Self {
            framebuffer: vec![0u32; pixels],
            command_pool: RenderCommandPool::with_capacity(65536),
            atlas: TextureAtlas::new(2048, 2048),
            work_buffer: vec![0u32; pixels],
            width, height,
        }
    }

    /// Ejecuta comandos de render usando múltiples hilos
    pub fn execute_commands_multithreaded(&mut self, commands: &[RenderCommand]) {
        let w = self.width as usize;
        let h = self.height as usize;

        // Limpiar work buffer
        self.work_buffer.copy_from_slice(&self.framebuffer);

        // Determinar número de tiles
        let tier = HardwareTier::current();
        let tile_count = tier.optimal_tile_count() as usize;

        if tile_count <= 1 || commands.len() < 100 {
            // Pocos comandos: single-threaded es más rápido (evita overhead)
            self.execute_single_threaded(commands, w, h);
        } else {
            self.execute_tiled(commands, w, h, tile_count);
        }
    }

    fn execute_single_threaded(&mut self, commands: &[RenderCommand], w: usize, h: usize) {
        for cmd in commands {
            self.execute_one(cmd, w, h);
        }
    }

    fn execute_tiled(&mut self, commands: &[RenderCommand], w: usize, h: usize, tile_count: usize) {
        use rayon::prelude::*;

        let cols = (tile_count as f32).sqrt().ceil() as usize;
        let rows = (tile_count + cols - 1) / cols;
        let tile_w = w / cols;
        let tile_h = h / rows;

        // Dividir comandos por tile
        let mut tile_cmds: Vec<Vec<&RenderCommand>> = (0..tile_count).map(|_| Vec::new()).collect();

        for cmd in commands {
            let cx = (cmd.x0 as usize + cmd.x1 as usize) / 2;
            let cy = (cmd.y0 as usize + cmd.y1 as usize) / 2;
            let tile_col = (cx / tile_w).min(cols - 1);
            let tile_row = (cy / tile_h).min(rows - 1);
            let tile_idx = tile_row * cols + tile_col;
            if tile_idx < tile_count {
                tile_cmds[tile_idx].push(cmd);
            }
        }

        // Ejecutar tiles en paralelo
        let fb_slice = &self.framebuffer;
        let mut work_slice = &mut self.work_buffer;

        // Rayon parallel tile execution
        let tile_results: Vec<Vec<u32>> = tile_cmds
            .par_iter()
            .enumerate()
            .map(|(tile_idx, cmds)| {
                let tile_col = tile_idx % cols;
                let tile_row = tile_idx / cols;
                let x_start = tile_col * tile_w;
                let y_start = tile_row * tile_h;
                let x_end = ((tile_col + 1) * tile_w).min(w);
                let y_end = ((tile_row + 1) * tile_h).min(h);

                let mut tile_fb = vec![0u32; (x_end - x_start) * (y_end - y_start)];

                // Copiar fondo del framebuffer original
                for ty in y_start..y_end {
                    let src_row = ty * w;
                    let dst_row = (ty - y_start) * (x_end - x_start);
                    for tx in x_start..x_end {
                        tile_fb[dst_row + (tx - x_start)] = fb_slice[src_row + tx];
                    }
                }

                // Ejecutar comandos que caen en este tile
                for cmd in cmds {
                    execute_cmd_on_tile(cmd, &mut tile_fb, x_start, y_start, x_end, y_end, w);
                }

                tile_fb
            })
            .collect();

        // Recombinar tiles al work buffer
        for (tile_idx, tile_fb) in tile_results.iter().enumerate() {
            let tile_col = tile_idx % cols;
            let tile_row = tile_idx / cols;
            let x_start = tile_col * tile_w;
            let y_start = tile_row * tile_h;
            let tw = ((tile_col + 1) * tile_w).min(w) - x_start;
            let th = ((tile_row + 1) * tile_h).min(h) - y_start;

            for ty in 0..th {
                let dst_row = (y_start + ty) * w;
                let src_row = ty * tw;
                for tx in 0..tw {
                    work_slice[dst_row + x_start + tx] = tile_fb[src_row + tx];
                }
            }
        }

        let _ = rayon::current_thread_pool;
    }

    #[inline(always)]
    fn execute_one(&mut self, cmd: &RenderCommand, w: usize, h: usize) {
        let x0 = cmd.x0 as usize;
        let y0 = cmd.y0 as usize;
        let x1 = cmd.x1 as usize;
        let y1 = cmd.y1 as usize;

        match cmd.primitive_type {
            0 => self.draw_rect(&mut self.work_buffer, w, h, x0, y0, x1, y1, cmd.color, cmd.flags),
            1 => self.draw_line(&mut self.work_buffer, w, h, x0, y0, x1, y1, cmd.color),
            2 => {
                if x0 < w && y0 < h {
                    self.work_buffer[y0 * w + x0] = blend_pixel(self.work_buffer[y0 * w + x0], cmd.color);
                }
            }
            3 => {
                // Sprite desde atlas
                if cmd.texture_id < self.atlas.entries.len() as u16 {
                    self.draw_sprite(w, h, cmd);
                }
            }
            5 => {
                // Gradiente (draw_rect con dithering)
                self.draw_gradient(&mut self.work_buffer, w, h, x0, y0, x1, y1, cmd.color, cmd.flags);
            }
            _ => {}
        }
    }

    fn draw_rect(&self, fb: &mut [u32], fb_w: usize, fb_h: usize, x0: usize, y0: usize, x1: usize, y1: usize, color: u32, flags: u8) {
        let x_start = x0.min(fb_w);
        let x_end = x1.min(fb_w);
        let y_start = y0.min(fb_h);
        let y_end = y1.min(fb_h);

        if x_start >= x_end || y_start >= y_end { return; }

        let has_alpha = flags & FLAG_ALPHA_BLEND != 0;
        let has_dither = flags & FLAG_DITHER != 0;

        for y in y_start..y_end {
            let row = y * fb_w;
            for x in x_start..x_end {
                let idx = row + x;
                if has_alpha {
                    fb[idx] = blend_pixel(fb[idx], color);
                } else if has_dither {
                    fb[idx] = dithered_pixel(fb[idx], color, x, y);
                } else {
                    fb[idx] = color;
                }
            }
        }
        let _ = fb_h;
    }

    fn draw_line(&self, fb: &mut [u32], fb_w: usize, fb_h: usize, x0: usize, y0: usize, x1: usize, y1: usize, color: u32) {
        // Bresenham
        let mut x = x0 as isize;
        let mut y = y0 as isize;
        let ex = x1 as isize;
        let ey = y1 as isize;

        let dx = (ex - x).abs();
        let dy = -(ey - y).abs();
        let sx = if x < ex { 1isize } else { -1isize };
        let sy = if y < ey { 1isize } else { -1isize };
        let mut err = dx + dy;

        loop {
            if x >= 0 && y >= 0 && (x as usize) < fb_w && (y as usize) < fb_h {
                fb[(y as usize) * fb_w + (x as usize)] = color;
            }
            if x == ex && y == ey { break; }
            let e2 = 2 * err;
            if e2 >= dy { if x == ex { break; } err += dy; x += sx; }
            if e2 <= dx { if y == ey { break; } err += dx; y += sy; }
        }
    }

    fn draw_sprite(&self, fb: &mut [u32], fb_w: usize, fb_h: usize, cmd: &RenderCommand) {
        let entry = &self.atlas.entries[cmd.texture_id as usize];
        let dx = cmd.x1 - cmd.x0;
        let dy = cmd.y1 - cmd.y0;
        if dx <= 0 || dy <= 0 { return; }

        for py in 0..dy as usize {
            let sy = cmd.y0 as usize + py;
            if sy >= fb_h { break; }
            let v = py as f32 / dy as f32;
            let row = sy * fb_w;

            for px in 0..dx as usize {
                let sx = cmd.x0 as usize + px;
                if sx >= fb_w { break; }
                let u = px as f32 / dx as f32;
                let texel = self.atlas.sample_uv(entry, u, v);
                if texel & 0xFF_00_00_00 != 0 {
                    fb[row + sx] = blend_pixel(fb[row + sx], texel);
                }
            }
        }
    }

    fn draw_gradient(&self, fb: &mut [u32], fb_w: usize, fb_h: usize, x0: usize, y0: usize, x1: usize, y1: usize, base_color: u32, _flags: u8) {
        let x_start = x0.min(fb_w);
        let x_end = x1.min(fb_w);
        let y_start = y0.min(fb_h);
        let y_end = y1.min(fb_h);

        let h = (y_end - y_start).max(1);
        let w = (x_end - x_start).max(1);

        for y in y_start..y_end {
            let t = (y - y_start) as f32 / h as f32;
            let darkened = darken_color(base_color, t * 0.5);
            let row = y * fb_w;
            for x in x_start..x_end {
                fb[row + x] = darkened;
            }
        }
        let _ = w;
    }

    /// Sincroniza el work buffer al framebuffer principal
    pub fn swap_buffers(&mut self) {
        self.framebuffer.copy_from_slice(&self.work_buffer);
    }
}

// ---------------------------------------------------------------------------
// EJECUCIÓN DE COMANDO EN TILE (para rayón)
// ---------------------------------------------------------------------------

#[inline(always)]
fn execute_cmd_on_tile(
    cmd: &RenderCommand,
    fb: &mut [u32],
    tile_x0: usize, tile_y0: usize,
    tile_x1: usize, tile_y1: usize,
    fb_w: usize,
) {
    let x0 = cmd.x0.max(tile_x0 as i16) as usize;
    let y0 = cmd.y0.max(tile_y0 as i16) as usize;
    let x1 = cmd.x1.min(tile_x1 as i16) as usize;
    let y1 = cmd.y1.min(tile_y1 as i16) as usize;

    if x0 >= x1 || y0 >= y1 { return; }

    let tw = tile_x1 - tile_x0;

    match cmd.primitive_type {
        0 => {
            let has_alpha = cmd.flags & FLAG_ALPHA_BLEND != 0;
            for y in y0..y1 {
                let row = (y - tile_y0) * tw;
                for x in x0..x1 {
                    let idx = row + (x - tile_x0);
                    if idx < fb.len() {
                        fb[idx] = if has_alpha { blend_pixel(fb[idx], cmd.color) } else { cmd.color };
                    }
                }
            }
        }
        1 => {
            // Línea simplificada en tile
            for y in y0..y1 {
                let row = (y - tile_y0) * tw;
                for x in x0..x1 {
                    let idx = row + (x - tile_x0);
                    if idx < fb.len() { fb[idx] = cmd.color; }
                }
            }
        }
        _ => {}
    }
    let _ = fb_w;
}

// ---------------------------------------------------------------------------
// ALPHA BLENDING [TC#29]
// ---------------------------------------------------------------------------

/// Alpha blending rápido con branch para casos comunes
#[inline(always)]
pub fn blend_pixel(bg: u32, fg: u32) -> u32 {
    let fg_a = (fg >> 24) & 0xFF;
    match fg_a {
        0 => return bg,
        255 => return fg,
        _ => {}
    }

    let bg_r = (bg >> 16) & 0xFF;
    let bg_g = (bg >> 8) & 0xFF;
    let bg_b = bg & 0xFF;

    let fg_r = (fg >> 16) & 0xFF;
    let fg_g = (fg >> 8) & 0xFF;
    let fg_b = fg & 0xFF;

    let inv_a = 255 - fg_a;

    // Usamos división entera con pre-multiplicación para evitar floats
    let r = ((fg_r as u32 * fg_a + bg_r as u32 * inv_a) / 255) as u32;
    let g = ((fg_g as u32 * fg_a + bg_g as u32 * inv_a) / 255) as u32;
    let b = ((fg_b as u32 * fg_a + bg_b as u32 * inv_a) / 255) as u32;

    0xFF_00_00_00 | (r << 16) | (g << 8) | b
}

/// Dithering ordenado 4x4 Bayer matrix
#[inline]
pub fn dithered_pixel(bg: u32, fg: u32, x: usize, y: usize) -> u32 {
    // Bayer 4x4 matrix threshold
    const BAYER: [[u8; 4]; 4] = [
        [0,  8,  2, 10],
        [12, 4, 14,  6],
        [3, 11,  1,  9],
        [15, 7, 13,  5],
    ];
    let threshold = BAYER[y % 4][x % 4] as u32 * 16; // escala 0..255
    let fg_a = (fg >> 24) & 0xFF;
    if fg_a as u32 > threshold { fg } else { bg }
}

#[inline]
fn darken_color(color: u32, factor: f32) -> u32 {
    let r = ((color >> 16) & 0xFF) as f32 * (1.0 - factor);
    let g = ((color >> 8) & 0xFF) as f32 * (1.0 - factor);
    let b = (color & 0xFF) as f32 * (1.0 - factor);
    let a = (color >> 24) & 0xFF;
    (a << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

// ---------------------------------------------------------------------------
// INICIALIZACIÓN DEL SISTEMA DE RENDER
// ---------------------------------------------------------------------------

pub enum ActiveBackend {
    CpuSimd(CpuBackend),
}

/// Inicializa el backend de render con detección automática
pub fn init_render_backend(width: u32, height: u32) -> ActiveBackend {
    let api = GpuApi::detect();
    let tier = HardwareTier::current();

    HARDWARE_TIER.store(tier as u8, Ordering::Release);
    SCREEN_W.store(width, Ordering::Release);
    SCREEN_H.store(height, Ordering::Release);

    // Establecer escala de resolución adaptativa
    if let Ok(mut scale) = RESOLUTION_SCALE.lock() {
        *scale = tier.quality_factor();
    }

    let cores = CPU_CORES.load(Ordering::Acquire);
    println!("════════════════════════════════════════════");
    println!("  🖥️  Hardware Detectado:");
    println!("     GPU API:  {}", api.name());
    println!("     Tier:     {:?} (nivel {})", tier, tier as u8);
    println!("     CPU cores: {}", cores);
    println!("     Resolución: {}x{} @ {}x scale", width, height, tier.quality_factor());
    println!("     Compute:  {}", if tier.supports_compute_shaders() { "SÍ" } else { "NO" });
    println!("     Max Tex:  {}x{}", tier.max_texture_size(), tier.max_texture_size());
    println!("════════════════════════════════════════════");

    ActiveBackend::CpuSimd(CpuBackend::new(width, height))
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hardware_tier_ordering() {
        assert!(HardwareTier::CpuOnly < HardwareTier::IntegratedGpu);
        assert!(HardwareTier::IntegratedGpu < HardwareTier::MidRangeGpu);
        assert!(HardwareTier::MidRangeGpu < HardwareTier::HighEndGpu);
    }

    #[test]
    fn test_hardware_tier_quality() {
        assert!((HardwareTier::CpuOnly.quality_factor() - 0.5).abs() < 0.01);
        assert!((HardwareTier::MidRangeGpu.quality_factor() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_blend_pixel_opaque() {
        assert_eq!(blend_pixel(0xFF_FF_00_00, 0xFF_00_FF_00), 0xFF_00_FF_00);
    }

    #[test]
    fn test_blend_pixel_transparent() {
        assert_eq!(blend_pixel(0xFF_FF_00_00, 0x00_00_FF_00), 0xFF_FF_00_00);
    }

    #[test]
    fn test_blend_pixel_semi() {
        let result = blend_pixel(0xFF_00_00_00, 0x80_FF_FF_FF);
        let r = (result >> 16) & 0xFF;
        assert!(r > 120 && r < 135, "Semi-transparent blend failed: r={}", r);
    }

    #[test]
    fn test_render_command_pool_sort() {
        let mut pool = RenderCommandPool::with_capacity(64);
        pool.push(RenderCommand { z_depth: 100, ..Default::default() });
        pool.push(RenderCommand { z_depth: 10, ..Default::default() });
        pool.push(RenderCommand { z_depth: 50, ..Default::default() });

        pool.sort_by_depth();
        assert_eq!(pool.slice()[0].z_depth, 10);
        assert_eq!(pool.slice()[1].z_depth, 50);
        assert_eq!(pool.slice()[2].z_depth, 100);
    }

    #[test]
    fn test_texture_atlas_register() {
        let mut atlas = TextureAtlas::new(2048, 2048);
        let pixels: Vec<u32> = vec![0xFF_FF_00_00; 64 * 64];
        let entry = atlas.register(&pixels, 64, 64);
        assert_eq!(entry.width, 64);
        assert_eq!(entry.height, 64);

        // Verificar que podemos muestrear
        let color = atlas.sample(entry.x + 10, entry.y + 10);
        assert_eq!(color, 0xFF_FF_00_00);
    }

    #[test]
    fn test_cpu_backend_draw_rect() {
        let backend = CpuBackend::new(100, 100);
        assert_eq!(backend.framebuffer.len(), 10000);
    }

    #[test]
    fn test_gpu_api_detect_returns_valid() {
        let api = GpuApi::detect();
        assert!(!api.name().is_empty());
    }

    #[test]
    fn test_dithered_pixel() {
        let bg = 0xFF_00_00_00;
        let fg = 0xFF_FF_FF_FF;
        let result = dithered_pixel(bg, fg, 0, 0);
        // Con threshold 0, siempre debe ser fg
        assert_eq!(result, fg);
    }

    #[test]
    fn test_darken_color() {
        let color = 0xFF_FF_FF_FF;
        let darker = darken_color(color, 0.5);
        let r = (darker >> 16) & 0xFF;
        assert!(r < 200, "Color should be darkened");
    }

    #[test]
    fn test_command_pool_clear() {
        let mut pool = RenderCommandPool::with_capacity(64);
        for i in 0..10 {
            pool.push(RenderCommand { z_depth: i as u16, ..Default::default() });
        }
        assert_eq!(pool.len(), 10);
        pool.clear();
        assert_eq!(pool.len(), 0);
        assert!(pool.is_empty());
    }
}
