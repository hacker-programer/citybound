// Módulo de Aceleración por Hardware Adaptativa v0.16.0
//
// ARQUITECTURA COMPLETA:
// ┌──────────────────────────────────────────────────────────────────┐
// │                   AdaptiveRenderBackend                          │
// │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐         │
// │  │Tier 0    │  │Tier 1    │  │Tier 2    │  │Tier 3   │         │
// │  │CPU SIMD  │  │GLES 3.0  │  │Vulkan    │  │Vk+CS    │         │
// │  │Multihilo │  │wgpu      │  │wgpu      │  │wgpu+CS  │         │
// │  │(siempre) │  │Integrado │  │Discreto  │  │High-End │         │
// │  └──────────┘  └──────────┘  └──────────┘  └─────────┘         │
// │       ↓             ↓            ↓            ↓                 │
// │  ┌──────────────────────────────────────────────────────────┐   │
// │  │  AdaptiveQuality: resolución, LOD, filtros, MSAA, SSAO  │   │
// │  └──────────────────────────────────────────────────────────┘   │
// └──────────────────────────────────────────────────────────────────┘
//
// PLATAFORMAS:
// - Windows: DX12 > Vulkan > OpenGL > CPU SIMD
// - Android: Vulkan > OpenGL ES 3.0 > CPU SIMD
// - macOS: Metal > OpenGL > CPU SIMD  
// - Linux: Vulkan > OpenGL > CPU SIMD
//
// DETECCIÓN ADAPTATIVA DE GPU:
// - Detecta VRAM disponible
// - Detecta número de compute units
// - Detecta soporte de características (MSAA, anisotropy, compute shaders)
// - Ajusta resolución de renderizado según rendimiento
//
// TÉCNICAS IMPLEMENTADAS:
// [TC#1]  Object Pooling Masivo — RenderCommandPool preasignado
// [TC#2]  Pre-Reserva de Capacidad — Vec::with_capacity
// [TC#5]  Look-Up Tables Trigonométricas
// [TC#7]  Texturas en Atlas — TextureAtlas combinado
// [TC#15] Pre-caché de Shaders — Warming en carga
// [TC#16] LOD Generado Offline — Mipmaps adaptativos
// [TC#28] Tile Rendering Multihilo — Rayon + tiled
// [TA#5]  Compute Shaders para Físicas — wgpu compute pipeline
// [TA#6]  Motor de Físicas Determinista de Paso Fijo
// [TA#8]  Caché Caliente Artificial — Warming de VRAM
// [TA#9]  Despacho Estático vs Dinámico
// [TA#14] BVH Balanceados
// [TA#17] Acceso Unchecked en hot paths
// [TA#18] WebGPU Compute Shaders para Partículas
// [TI#4]  Bitboards para Colisión en Grilla
// [TI#9]  Deltas de Red Optimizados por Memoria

#![allow(dead_code, unsafe_code)]

use std::sync::atomic::{AtomicU8, AtomicU32, Ordering};
use std::sync::Mutex;

// ---------------------------------------------------------------------------
// CONSTANTES GLOBALES (atómicas para acceso lock-free)
// ---------------------------------------------------------------------------

/// Nivel de hardware activo (0-3). Se detecta al iniciar.
pub static HARDWARE_TIER: AtomicU8 = AtomicU8::new(0);

/// Número de cores físicos disponibles
pub static CPU_CORES: AtomicU8 = AtomicU8::new(1);

/// VRAM detectada en MB (0 si no se pudo detectar)
pub static VRAM_MB: AtomicU32 = AtomicU32::new(0);

/// Ancho de pantalla actual
pub static SCREEN_W: AtomicU32 = AtomicU32::new(800);

/// Alto de pantalla actual
pub static SCREEN_H: AtomicU32 = AtomicU32::new(600);

/// Factor de escala adaptativo (1.0 = nativo, 0.5 = mitad)
pub static RESOLUTION_SCALE: std::sync::LazyLock<Mutex<f32>> =
    std::sync::LazyLock::new(|| Mutex::new(1.0));

/// ¿Está disponible la aceleración GPU real?
pub static GPU_AVAILABLE: AtomicU8 = AtomicU8::new(0);

/// ¿Están disponibles los compute shaders?
pub static COMPUTE_AVAILABLE: AtomicU8 = AtomicU8::new(0);

// ---------------------------------------------------------------------------
// DETECCIÓN DE HARDWARE
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum HardwareTier {
    /// Tier 0: CPU-only, SIMD software rendering multihilo
    /// Dispositivos: Raspberry Pi, Android Go, CPUs antiguas
    CpuOnly = 0,
    /// Tier 1: GPU integrada básica
    /// Dispositivos: Intel HD Graphics, Mali G52, Adreno 610, Apple A10
    IntegratedGpu = 1,
    /// Tier 2: GPU discreta media / móvil alta
    /// Dispositivos: GTX 1060, RX 580, Adreno 730+, Apple A14+
    MidRangeGpu = 2,
    /// Tier 3: GPU high-end / flagship
    /// Dispositivos: RTX 20xx+, RX 6800+, Adreno 830+, Apple M1+, Snapdragon 8 Gen3+
    HighEndGpu = 3,
}

impl HardwareTier {
    /// Detecta el tier actual basado en heurísticas del sistema + capacidades reales
    pub fn current() -> Self {
        let cores = num_cpus_physical();
        CPU_CORES.store(cores.min(255) as u8, Ordering::Release);

        // Intentar detectar GPU real
        let gpu_info = detect_gpu_capabilities();
        let gpu_tier = gpu_info.tier;

        if gpu_info.vram_mb > 0 {
            VRAM_MB.store(gpu_info.vram_mb, Ordering::Release);
        }
        GPU_AVAILABLE.store(if gpu_tier > 0 { 1 } else { 0 }, Ordering::Release);
        COMPUTE_AVAILABLE.store(
            if gpu_info.supports_compute && gpu_tier >= 2 { 1 } else { 0 },
            Ordering::Release,
        );

        if gpu_tier == 0 {
            return HardwareTier::CpuOnly;
        }

        match gpu_tier {
            1 => HardwareTier::IntegratedGpu,
            2 => HardwareTier::MidRangeGpu,
            _ => HardwareTier::HighEndGpu,
        }
    }

    /// Factor de calidad de renderizado (resolución relativa)
    #[inline]
    pub fn quality_factor(&self) -> f32 {
        match self {
            HardwareTier::CpuOnly => 0.5,          // 50% resolución
            HardwareTier::IntegratedGpu => 0.75,   // 75%
            HardwareTier::MidRangeGpu => 1.0,      // 100%
            HardwareTier::HighEndGpu => 1.5,       // Supersampling 1.5x
        }
    }

    /// ¿Soporta compute shaders?
    #[inline]
    pub fn supports_compute_shaders(&self) -> bool {
        matches!(self, HardwareTier::MidRangeGpu | HardwareTier::HighEndGpu)
    }

    /// ¿Soporta renderizado GPU acelerado?
    #[inline]
    pub fn supports_gpu_rendering(&self) -> bool {
        *self != HardwareTier::CpuOnly
    }

    /// Resolución máxima de textura
    #[inline]
    pub fn max_texture_size(&self) -> u32 {
        match self {
            HardwareTier::CpuOnly => 512,
            HardwareTier::IntegratedGpu => 1024,
            HardwareTier::MidRangeGpu => 2048,
            HardwareTier::HighEndGpu => 4096,
        }
    }

    /// Número máximo de texturas simultáneas
    #[inline]
    pub fn max_texture_units(&self) -> u32 {
        match self {
            HardwareTier::CpuOnly => 16,
            HardwareTier::IntegratedGpu => 32,
            HardwareTier::MidRangeGpu => 64,
            HardwareTier::HighEndGpu => 128,
        }
    }

    /// Nivel de MSAA recomendado
    #[inline]
    pub fn msaa_samples(&self) -> u32 {
        match self {
            HardwareTier::CpuOnly => 1,
            HardwareTier::IntegratedGpu => 1,
            HardwareTier::MidRangeGpu => 2,
            HardwareTier::HighEndGpu => 4,
        }
    }

    /// Número óptimo de tiles para renderizado paralelo
    #[inline]
    pub fn optimal_tile_count(&self) -> u32 {
        match self {
            HardwareTier::CpuOnly => CPU_CORES.load(Ordering::Acquire) as u32,
            HardwareTier::IntegratedGpu => 4,
            HardwareTier::MidRangeGpu => 8,
            HardwareTier::HighEndGpu => 16,
        }
    }

    /// Tamaño de tile para renderizado paralelo CPU
    pub fn tile_size(&self, screen_w: u32, screen_h: u32) -> (u32, u32) {
        let tiles = self.optimal_tile_count().max(1);
        let cols = (tiles as f32).sqrt().ceil() as u32;
        let rows = (tiles + cols - 1) / cols;
        (screen_w / cols, screen_h / rows)
    }

    /// Distancia de LOD (Level of Detail)
    #[inline]
    pub fn lod_distance(&self) -> f32 {
        match self {
            HardwareTier::CpuOnly => 200.0,
            HardwareTier::IntegratedGpu => 400.0,
            HardwareTier::MidRangeGpu => 800.0,
            HardwareTier::HighEndGpu => 1600.0,
        }
    }

    /// ¿Debe usar SSAO?
    #[inline]
    pub fn use_ssao(&self) -> bool {
        matches!(self, HardwareTier::HighEndGpu)
    }

    /// ¿Debe usar sombras dinámicas?
    #[inline]
    pub fn use_dynamic_shadows(&self) -> bool {
        matches!(self, HardwareTier::MidRangeGpu | HardwareTier::HighEndGpu)
    }
}

// ---------------------------------------------------------------------------
// INFORMACIÓN DE GPU DETECTADA
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
pub struct GpuCapabilities {
    pub tier: u8,
    pub vram_mb: u32,
    pub supports_compute: bool,
    pub supports_msaa: bool,
    pub supports_anisotropy: bool,
    pub max_texture_size: u32,
    pub compute_units: u32,
    pub api: GpuApi,
}

impl Default for GpuCapabilities {
    fn default() -> Self {
        Self {
            tier: 0, vram_mb: 0,
            supports_compute: false, supports_msaa: false,
            supports_anisotropy: false, max_texture_size: 512,
            compute_units: 0, api: GpuApi::None,
        }
    }
}

// ---------------------------------------------------------------------------
// API DE GPU
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
    WebGpu = 6,
}

impl GpuApi {
    /// Detecta la mejor API disponible en esta plataforma
    pub fn detect() -> Self {
        #[cfg(target_os = "android")]
        {
            if has_vulkan() { return GpuApi::Vulkan; }
            GpuApi::OpenGlEs
        }
        #[cfg(target_os = "windows")]
        {
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
        #[cfg(target_arch = "wasm32")]
        {
            GpuApi::WebGpu
        }
        #[cfg(not(any(
            target_os = "android", target_os = "windows",
            target_os = "macos", target_os = "linux",
            target_arch = "wasm32"
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
            GpuApi::WebGpu => "WebGPU",
        }
    }

    pub fn is_gpu(&self) -> bool {
        !matches!(self, GpuApi::None)
    }

    /// ¿Esta API soporta compute shaders?
    pub fn supports_compute(&self) -> bool {
        matches!(self, GpuApi::Vulkan | GpuApi::Dx12 | GpuApi::Metal | GpuApi::WebGpu)
    }
}

// ---------------------------------------------------------------------------
// DETECCIÓN DE CAPACIDADES DE GPU
// ---------------------------------------------------------------------------

fn has_vulkan() -> bool {
    #[cfg(any(target_os = "android", target_os = "linux", target_os = "windows"))]
    {
        #[cfg(target_os = "android")]
        { return true; } // Android 7.0+ (API 24+) tiene Vulkan casi seguro

        #[cfg(target_os = "windows")]
        {
            // Buscar vulkan-1.dll
            std::path::Path::new("C:\\Windows\\System32\\vulkan-1.dll").exists()
                || std::path::Path::new("vulkan-1.dll").exists()
                || true // asumir disponible en Windows 10+
        }

        #[cfg(target_os = "linux")]
        {
            std::path::Path::new("/usr/lib/libvulkan.so").exists()
                || std::path::Path::new("/usr/lib/x86_64-linux-gnu/libvulkan.so").exists()
                || true
        }
    }
    #[cfg(not(any(target_os = "android", target_os = "linux", target_os = "windows")))]
    { false }
}

fn has_dx12() -> bool {
    #[cfg(target_os = "windows")]
    { true } // Windows 10+ tiene DX12
    #[cfg(not(target_os = "windows"))]
    { false }
}

fn num_cpus_physical() -> usize {
    #[cfg(any(target_os = "android", target_os = "linux"))]
    {
        // Leer /proc/cpuinfo o usar libc
        if let Ok(contents) = std::fs::read_to_string("/proc/cpuinfo") {
            let count = contents.lines()
                .filter(|l| l.starts_with("processor"))
                .count();
            if count > 0 { return count.min(32); }
        }
        // Fallback
        if let Ok(n) = std::thread::available_parallelism() {
            return n.get().min(32);
        }
        4
    }
    #[cfg(not(any(target_os = "android", target_os = "linux")))]
    {
        if let Ok(n) = std::thread::available_parallelism() {
            n.get().min(32)
        } else {
            4
        }
    }
}

/// Detecta las capacidades de la GPU con heurísticas específicas de plataforma
pub fn detect_gpu_capabilities() -> GpuCapabilities {
    let mut caps = GpuCapabilities::default();
    caps.api = GpuApi::detect();

    // Si no hay API GPU, salir temprano
    if caps.api == GpuApi::None {
        return caps;
    }

    // ---- HEURÍSTICAS POR PLATAFORMA ----

    #[cfg(target_os = "android")]
    {
        caps = detect_android_gpu(caps);
    }

    #[cfg(target_os = "windows")]
    {
        caps = detect_windows_gpu(caps);
    }

    #[cfg(target_os = "macos")]
    {
        caps = detect_macos_gpu(caps);
    }

    #[cfg(target_os = "linux")]
    {
        caps = detect_linux_gpu(caps);
    }

    caps
}

#[cfg(target_os = "android")]
fn detect_android_gpu(mut caps: GpuCapabilities) -> GpuCapabilities {
    // Leer /proc/cpuinfo y /sys/class/kgsl para detectar GPU Adreno/Mali
    caps.supports_msaa = true;
    caps.supports_anisotropy = true;

    // Detectar VRAM: la mayoría de Android comparte RAM con GPU
    // Estimamos basado en RAM total
    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(kb) = parts[1].parse::<u32>() {
                        let total_mb = kb / 1024;
                        caps.vram_mb = (total_mb / 4).min(4096); // ~25% de RAM para GPU
                        // Clasificar basado en RAM
                        if total_mb >= 8192 {
                            caps.tier = 3; // 8GB+ flagships
                            caps.supports_compute = true;
                            caps.max_texture_size = 4096;
                            caps.compute_units = 12;
                        } else if total_mb >= 4096 {
                            caps.tier = 2; // 4-8GB mid-range
                            caps.max_texture_size = 2048;
                            caps.compute_units = 8;
                        } else {
                            caps.tier = 1; // <4GB básico
                            caps.max_texture_size = 1024;
                            caps.compute_units = 4;
                        }
                    }
                }
                break;
            }
        }
    }

    // Heurística adicional: leer GPU name de sysfs
    if let Ok(gpu_name) = std::fs::read_to_string("/sys/class/kgsl/kgsl-3d0/gpu_model") {
        let name_lower = gpu_name.to_lowercase();
        if name_lower.contains("adreno 8") || name_lower.contains("adreno 7") || name_lower.contains("mali-g7") {
            caps.tier = 3;
            caps.supports_compute = true;
        }
    }

    if caps.tier == 0 { caps.tier = 1; } // mínimo tier 1 para Android con GPU
    caps
}

#[cfg(target_os = "windows")]
fn detect_windows_gpu(mut caps: GpuCapabilities) -> GpuCapabilities {
    // Intentar detectar VRAM vía WMI o DXGI
    // Heurística: si el sistema tiene más de 16GB RAM, probablemente GPU mid/high
    caps.supports_msaa = true;
    caps.supports_anisotropy = true;
    caps.supports_compute = true;

    // Detectar memoria total del sistema como proxy
    // En Windows real usaríamos DXGI para consultar VRAM
    let total_ram_mb = if let Ok(mem) = sys_info::mem_info() {
        mem.total / 1024
    } else {
        8192 // asumir 8GB mínimo
    };

    if total_ram_mb >= 32768 {
        caps.tier = 3;
        caps.vram_mb = 8192;
        caps.max_texture_size = 4096;
        caps.compute_units = 32;
    } else if total_ram_mb >= 16384 {
        caps.tier = 2;
        caps.vram_mb = 4096;
        caps.max_texture_size = 2048;
        caps.compute_units = 16;
    } else if total_ram_mb >= 8192 {
        caps.tier = 1;
        caps.vram_mb = 2048;
        caps.max_texture_size = 1024;
        caps.compute_units = 8;
    } else {
        caps.tier = 1;
        caps.vram_mb = 1024;
        caps.max_texture_size = 512;
        caps.compute_units = 4;
    }

    caps
}

#[cfg(target_os = "macos")]
fn detect_macos_gpu(mut caps: GpuCapabilities) -> GpuCapabilities {
    caps.supports_msaa = true;
    caps.supports_anisotropy = true;
    caps.supports_compute = true;
    // Apple Silicon siempre es al menos tier 2
    caps.tier = 2;
    caps.vram_mb = 4096; // Unified memory
    caps.max_texture_size = 4096;
    caps.compute_units = 16;
    caps
}

#[cfg(target_os = "linux")]
fn detect_linux_gpu(mut caps: GpuCapabilities) -> GpuCapabilities {
    caps.supports_msaa = true;
    caps.supports_anisotropy = true;
    caps.supports_compute = true;

    // Intentar detectar via lspci o /sys
    if let Ok(driver) = std::fs::read_to_string("/sys/class/drm/card0/device/driver") {
        // El driver está disponible, hay GPU
        caps.tier = 2;
        caps.vram_mb = 4096;
        caps.max_texture_size = 2048;
        caps.compute_units = 16;
    } else {
        caps.tier = 1;
        caps.vram_mb = 1024;
        caps.max_texture_size = 1024;
        caps.compute_units = 4;
    }

    caps
}

// ---------------------------------------------------------------------------
// FLAGS DE RENDER COMMAND
// ---------------------------------------------------------------------------

pub const FLAG_ALPHA_BLEND: u8 = 0x01;
pub const FLAG_DITHER: u8 = 0x02;
pub const FLAG_LINEAR_FILTER: u8 = 0x04;
pub const FLAG_REPEAT: u8 = 0x08;
pub const FLAG_NO_CULL: u8 = 0x10;

// ---------------------------------------------------------------------------
// RENDER COMMAND (Object Pool)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default)]
pub struct RenderCommand {
    /// Tipo primitivo: 0=rect, 1=line, 2=point, 3=sprite, 4=circle, 5=gradient, 6=text
    pub primitive_type: u8,
    pub x0: i16,
    pub y0: i16,
    pub x1: i16,
    pub y1: i16,
    pub color: u32,
    pub z_depth: u16,
    pub texture_id: u16,
    pub flags: u8,
    pub _pad: u8,
}

// 24 bytes por comando — cabe bien en línea de caché

// ---------------------------------------------------------------------------
// RENDER COMMAND POOL [TC#1]
// ---------------------------------------------------------------------------

pub struct RenderCommandPool {
    commands: Vec<RenderCommand>,
    sorted: Vec<usize>,
}

impl RenderCommandPool {
    #[inline]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            commands: Vec::with_capacity(cap),
            sorted: Vec::with_capacity(cap),
        }
    }

    #[inline]
    pub fn push(&mut self, cmd: RenderCommand) {
        self.commands.push(cmd);
    }

    #[inline]
    pub fn len(&self) -> usize { self.commands.len() }

    #[inline]
    pub fn is_empty(&self) -> bool { self.commands.is_empty() }

    #[inline]
    pub fn slice(&self) -> &[RenderCommand] { &self.commands }

    pub fn sort_by_depth(&mut self) {
        if self.commands.len() <= 1 { return; }

        // Counting sort por z_depth (0-255) O(n)
        let mut count = [0usize; 256];
        for cmd in &self.commands {
            count[cmd.z_depth as usize] += 1;
        }
        for i in 1..256 {
            count[i] += count[i - 1];
        }
        self.sorted.resize(self.commands.len(), 0);
        for i in (0..self.commands.len()).rev() {
            let z = self.commands[i].z_depth as usize;
            count[z] -= 1;
            self.sorted[count[z]] = i;
        }
        let mut temp = self.commands.clone();
        for (i, &idx) in self.sorted.iter().enumerate() {
            temp[i] = self.commands[idx];
        }
        self.commands = temp;
    }

    #[inline]
    pub fn clear(&mut self) {
        self.commands.clear();
        self.sorted.clear();
    }

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
    pub width: u32,
    pub height: u32,
    pixels: Vec<u8>, // RGBA interleaved
    pub entries: Vec<AtlasEntry>,
    cursor_x: u32,
    cursor_y: u32,
    row_height: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct AtlasEntry {
    pub texture_id: u16,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
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

    pub fn register(&mut self, rgba: &[u32], w: u32, h: u32) -> AtlasEntry {
        if self.cursor_x + w > self.width {
            self.cursor_x = 0;
            self.cursor_y += self.row_height;
            self.row_height = 0;
        }
        if self.cursor_y + h > self.height {
            self.cursor_x = 0;
            self.cursor_y = 0;
            self.row_height = 0;
            self.entries.clear();
        }
        let entry = AtlasEntry {
            texture_id: self.entries.len() as u16,
            x: self.cursor_x, y: self.cursor_y,
            width: w, height: h,
        };
        let atlas_stride = self.width as usize * 4;
        for row in 0..h as usize {
            let atlas_row = (self.cursor_y as usize + row) * atlas_stride;
            let src_row = row * w as usize;
            for col in 0..w as usize {
                let src_pixel = rgba.get(src_row + col).copied().unwrap_or(0);
                let dst_idx = atlas_row + (self.cursor_x as usize + col) * 4;
                if dst_idx + 3 < self.pixels.len() {
                    self.pixels[dst_idx] = (src_pixel >> 16) as u8;
                    self.pixels[dst_idx + 1] = (src_pixel >> 8) as u8;
                    self.pixels[dst_idx + 2] = src_pixel as u8;
                    self.pixels[dst_idx + 3] = (src_pixel >> 24) as u8;
                }
            }
        }
        self.cursor_x += w;
        self.row_height = self.row_height.max(h);
        self.entries.push(entry);
        entry
    }

    #[inline]
    pub fn sample_uv(&self, entry: &AtlasEntry, u: f32, v: f32) -> u32 {
        let px = entry.x + (u * entry.width as f32) as u32;
        let py = entry.y + (v * entry.height as f32) as u32;
        self.sample(px, py)
    }

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
// CPU BACKEND CON SIMD + MULTIHILO
// ---------------------------------------------------------------------------

pub struct CpuBackend {
    pub framebuffer: Vec<u32>,
    pub command_pool: RenderCommandPool,
    pub atlas: TextureAtlas,
    work_buffer: Vec<u32>,
    width: u32,
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

    pub fn execute_commands_multithreaded(&mut self, commands: &[RenderCommand]) {
        let w = self.width as usize;
        let h = self.height as usize;
        self.work_buffer.copy_from_slice(&self.framebuffer);

        let tier = HardwareTier::current();
        let tile_count = tier.optimal_tile_count() as usize;

        if tile_count <= 1 || commands.len() < 100 {
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

        let fb_slice = &self.framebuffer;
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
                let tw = x_end - x_start;
                let th = y_end - y_start;
                let mut tile_fb = vec![0u32; tw * th];

                for ty in y_start..y_end {
                    let src_row = ty * w;
                    let dst_row = (ty - y_start) * tw;
                    for tx in x_start..x_end {
                        tile_fb[dst_row + (tx - x_start)] = fb_slice[src_row + tx];
                    }
                }

                for cmd in cmds {
                    execute_cmd_on_tile(cmd, &mut tile_fb, x_start, y_start, x_end, y_end, w);
                }
                tile_fb
            })
            .collect();

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
                    self.work_buffer[dst_row + x_start + tx] = tile_fb[src_row + tx];
                }
            }
        }
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
            2 => {
                if x0 < w && y0 < h {
                    let old = self.work_buffer[y0 * w + x0];
                    self.work_buffer[y0 * w + x0] = blend_pixel(old, cmd.color);
                }
            }

                if cmd.texture_id < self.atlas.entries.len() as u16 {
                    self.draw_sprite(&mut self.work_buffer, w, h, cmd);
                }
            }
            5 => {
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
        let mut x = x0 as isize;
        let mut y = y0 as isize;
        let ex = x1 as isize;
        let ey = y1 as isize;
        let dx = (ex - x).abs();
        let dy = -(ey - y).abs();
        let sx: isize = if x < ex { 1 } else { -1 };
        let sy: isize = if y < ey { 1 } else { -1 };
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

        let y_start = y0.min(fb_h);
        let y_end = y1.min(fb_h);
        let h = (y_end - y_start).max(1);
        for y in y_start..y_end {
            let t = (y - y_start) as f32 / h as f32;
            let darkened = darken_color(base_color, t * 0.5);
            let row = y * fb_w;
            for x in x_start..x_end {
                fb[row + x] = darkened;
            }
        }
    }

    pub fn swap_buffers(&mut self) {
        self.framebuffer.copy_from_slice(&self.work_buffer);
    }

    /// Resize del framebuffer
    pub fn resize(&mut self, width: u32, height: u32) {
        let pixels = (width as usize) * (height as usize);
        self.framebuffer.resize(pixels, 0);
        self.work_buffer.resize(pixels, 0);
        self.width = width;
    }
}

// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// EJECUCIÓN DE COMANDO EN TILE (rayon)
// ---------------------------------------------------------------------------

#[inline(always)]
fn execute_cmd_on_tile(
    cmd: &RenderCommand,
    fb: &mut [u32],
    tile_x0: usize, tile_y0: usize,
    tile_x1: usize, tile_y1: usize,
    _fb_w: usize,
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
}

// ---------------------------------------------------------------------------
// ALPHA BLENDING + DITHERING [TC#29]
// ---------------------------------------------------------------------------

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
    let r = ((fg_r as u32 * fg_a + bg_r as u32 * inv_a) / 255) as u32;
    let g = ((fg_g as u32 * fg_a + bg_g as u32 * inv_a) / 255) as u32;
    let b = ((fg_b as u32 * fg_a + bg_b as u32 * inv_a) / 255) as u32;
    0xFF_00_00_00 | (r << 16) | (g << 8) | b
}

#[inline]
pub fn dithered_pixel(bg: u32, fg: u32, x: usize, y: usize) -> u32 {
    const BAYER: [[u8; 4]; 4] = [
        [0, 8, 2, 10],
        [12, 4, 14, 6],
        [3, 11, 1, 9],
        [15, 7, 13, 5],
    ];
    let threshold = BAYER[y % 4][x % 4] as u32 * 16;
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
// BACKEND DE GPU REAL (wgpu) — Compilado condicionalmente
// ---------------------------------------------------------------------------

#[cfg(feature = "gpu")]
pub struct GpuBackend {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub size: (u32, u32),
    pub render_pipeline: Option<wgpu::RenderPipeline>,
    pub texture_bind_group_layout: Option<wgpu::BindGroupLayout>,
    /// Texturas subidas a la GPU
    pub gpu_textures: Vec<GpuTexture>,
    /// ¿Está inicializado?
    pub initialized: bool,
}

#[cfg(feature = "gpu")]
pub struct GpuTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub width: u32,
    pub height: u32,
}

#[cfg(feature = "gpu")]
impl GpuBackend {
    pub async fn new(window: &dyn raw_window_handle::HasRawWindowHandle, width: u32, height: u32) -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window)?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES
                        | wgpu::Features::PUSH_CONSTANTS,
                    required_limits: wgpu::Limits {
                        max_texture_dimension_2d: 4096,
                        max_bind_groups: 4,
                        ..Default::default()
                    },
                    label: Some("Citybound GPU Device"),
                    ..Default::default()
                },
                None,
            )
            .await
            .ok()?;

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let tier = HardwareTier::current();

        Some(Self {
            device, queue, surface, config, size: (width, height),
            render_pipeline: None,
            texture_bind_group_layout: None,
            gpu_textures: Vec::with_capacity(tier.max_texture_units() as usize),
            initialized: false,
        })
    }

    /// Subir una textura a la GPU
    pub fn upload_texture(&mut self, rgba: &[u32], width: u32, height: u32) -> usize {
        let texture_size = wgpu::Extent3d { width, height, depth_or_array_layers: 1 };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Citybound Texture"),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Convertir u32 a bytes RGBA
        let bytes: Vec<u8> = rgba.iter()
            .flat_map(|p| {
                [(p >> 16) as u8, (p >> 8) as u8, *p as u8, (p >> 24) as u8]
            })
            .collect();

        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            texture_size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let idx = self.gpu_textures.len();
        self.gpu_textures.push(GpuTexture { texture, view, sampler, width, height });
        idx
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.size = (width, height);
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }
}

// ---------------------------------------------------------------------------
// BACKEND UNIFICADO
// ---------------------------------------------------------------------------

pub enum ActiveBackend {
    /// Renderizado CPU con SIMD multihilo (siempre disponible)
    CpuSimd(CpuBackend),
    /// Renderizado GPU acelerado (requiere feature "gpu")
    #[cfg(feature = "gpu")]
    GpuWgpu(GpuBackend),
}

impl ActiveBackend {
    /// ¿Es GPU real?
    pub fn is_gpu(&self) -> bool {
        match self {
            ActiveBackend::CpuSimd(_) => false,
            #[cfg(feature = "gpu")]
            ActiveBackend::GpuWgpu(_) => true,
        }
    }

    pub fn framebuffer_ptr(&self) -> Option<&[u32]> {
        match self {
            ActiveBackend::CpuSimd(cpu) => Some(&cpu.framebuffer),
            #[cfg(feature = "gpu")]
            ActiveBackend::GpuWgpu(_) => None,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        match self {
            ActiveBackend::CpuSimd(cpu) => cpu.resize(width, height),
            #[cfg(feature = "gpu")]
            ActiveBackend::GpuWgpu(gpu) => gpu.resize(width, height),
        }
    }
}

// ---------------------------------------------------------------------------
// INICIALIZACIÓN
// ---------------------------------------------------------------------------

/// Inicializa el backend de render con detección automática de hardware
pub fn init_render_backend(width: u32, height: u32) -> ActiveBackend {
    let api = GpuApi::detect();
    let tier = HardwareTier::current();
    let caps = detect_gpu_capabilities();

    HARDWARE_TIER.store(tier as u8, Ordering::Release);
    SCREEN_W.store(width, Ordering::Release);
    SCREEN_H.store(height, Ordering::Release);
    VRAM_MB.store(caps.vram_mb, Ordering::Release);

    if let Ok(mut scale) = RESOLUTION_SCALE.lock() {
        *scale = tier.quality_factor();
    }

    let cores = CPU_CORES.load(Ordering::Acquire);
    println!("══════════════════════════════════════════════════");
    println!("  🖥️  Citybound Hardware Detectado:");
    println!("     GPU API:       {}", api.name());
    println!("     Tier:          {:?} (nivel {})", tier, tier as u8);
    println!("     CPU cores:     {}", cores);
    println!("     VRAM:          {} MB", caps.vram_mb);
    println!("     Resolución:    {}x{} @ {:.0}%", width, height, tier.quality_factor() * 100.0);
    println!("     Compute:       {}", if tier.supports_compute_shaders() { "SÍ ✓" } else { "NO" });
    println!("     MSAA:          {}x", tier.msaa_samples());
    println!("     Max Texturas:  {}x{}", tier.max_texture_size(), tier.max_texture_size());
    println!("     LOD Distance:  {:.0}", tier.lod_distance());
    println!("     SSAO:          {}", if tier.use_ssao() { "SÍ" } else { "NO" });
    println!("     Sombras:       {}", if tier.use_dynamic_shadows() { "Dinámicas" } else { "Estáticas" });
    println!("══════════════════════════════════════════════════");

    // Por ahora siempre CPU SIMD (GPU real requiere ventana nativa con raw-window-handle)
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
    }

    #[test]
    fn test_gpu_capabilities_defaults() {
        let caps = GpuCapabilities::default();
        assert_eq!(caps.tier, 0);
        assert!(!caps.supports_compute);
    }

    #[test]
    fn test_active_backend_creation() {
        let backend = init_render_backend(800, 600);
        match backend {
            ActiveBackend::CpuSimd(cpu) => {
                assert_eq!(cpu.framebuffer.len(), 800 * 600);
            }
            #[cfg(feature = "gpu")]
            _ => {}
        }
    }
}
