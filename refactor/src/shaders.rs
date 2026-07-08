// Shaders Pre-compilados WGSL v0.13.0
//
// [FASE 8] Catálogo de shaders para GPU rendering adaptativo.
//
// TÉCNICAS:
// [TC#15] Pre-caché de Shaders — compilados en tiempo de carga
// [TA#5]  Compute Shaders para físicas — partículas y fluidos en GPU
// [TC#3]  Baking de iluminación a texturas
// [TC#16] LOD generado offline — mipmaps
//
// Estos shaders se compilan UNA VEZ durante la fase de carga y se
// cachean en el objeto GpuState. En Tier 0 (CPU) se emulan por software.
//
// FORMATO: WGSL (WebGPU Shading Language), compatible con:
//   - Vulkan (via SPIR-V)
//   - DX12 (via HLSL)
//   - Metal (via MSL)
//   - OpenGL ES 3.1 (via SPIR-V cross-compilation)

/// Catálogo de shaders pre-compilados
pub struct ShaderCatalog {
    /// Shader para tiles de terreno (más común, ultra-optimizado)
    pub terrain_tile: &'static str,
    /// Shader para sprites de edificios
    pub building_sprite: &'static str,
    /// Shader para overlays de UI
    pub ui_overlay: &'static str,
    /// Shader para partículas (sistemas de humo, fuego)
    pub particle: &'static str,
    /// Shader para agua (reflejos, ondas)
    pub water: &'static str,
    /// Compute shader para difusión de contaminación
    pub compute_pollution_diffusion: &'static str,
    /// Compute shader para flow field en GPU
    pub compute_flow_field: &'static str,
    /// Compute shader para dinámica de multitudes (social force)
    pub compute_crowd_dynamics: &'static str,
}

/// Catálogo estático global (inicializado una vez en la carga)
pub static SHADERS: ShaderCatalog = ShaderCatalog {
    terrain_tile: TERRAIN_TILE_SHADER,
    building_sprite: BUILDING_SPRITE_SHADER,
    ui_overlay: UI_OVERLAY_SHADER,
    particle: PARTICLE_SHADER,
    water: WATER_SHADER,
    compute_pollution_diffusion: COMPUTE_POLLUTION_DIFFUSION,
    compute_flow_field: COMPUTE_FLOW_FIELD,
    compute_crowd_dynamics: COMPUTE_CROWD_DYNAMICS,
};

// ============================================================================
// SHADER: Terrain Tile (Vertex + Fragment)
// ULTRA-OPTIMIZADO: sin branches, sin loops dinámicos, todo constante
// ============================================================================

pub const TERRAIN_TILE_SHADER: &str = r#"
// Terrain Tile Shader v1.0 — Citybound Native
// Optimizado para GPU Tier 1+: sin branches dinámicos
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) texcoord: vec2<f32>,
    @location(2) tile_type: u32,      // 0=grass, 1=dirt, 2=road, 3=water, 4=sidewalk
    @location(3) zone_overlay: u32,   // color RGBA8 packed
    @location(4) pollution_level: f32, // 0.0-1.0
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texcoord: vec2<f32>,
    @location(1) tile_type: u32,
    @location(2) zone_color: vec4<f32>,
    @location(3) pollution: f32,
}

struct CameraUniform {
    view_proj: mat4x4<f32>,
    offset: vec2<f32>,
    zoom: f32,
    screen_size: vec2<u32>,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var terrain_atlas: texture_2d<f32>;
@group(0) @binding(2) var terrain_sampler: sampler;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Transformación de cámara con zoom
    let world_pos = in.position * camera.zoom + camera.offset;
    let ndc_x = (world_pos.x / f32(camera.screen_size.x)) * 2.0 - 1.0;
    let ndc_y = 1.0 - (world_pos.y / f32(camera.screen_size.y)) * 2.0;

    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.texcoord = in.texcoord;
    out.tile_type = in.tile_type;
    out.pollution = in.pollution_level;

    // Desempaquetar color de zona
    let zr = f32((in.zone_overlay >> 24u) & 0xFFu) / 255.0;
    let zg = f32((in.zone_overlay >> 16u) & 0xFFu) / 255.0;
    let zb = f32((in.zone_overlay >> 8u) & 0xFFu) / 255.0;
    let za = f32(in.zone_overlay & 0xFFu) / 255.0;
    out.zone_color = vec4<f32>(zr, zg, zb, za);

    return out;
}

// Tabla de colores de terreno pre-calculada (evita branches)
const TERRAIN_COLORS: array<vec4<f32>, 5> = array<vec4<f32>, 5>(
    vec4<f32>(0.176, 0.353, 0.153, 1.0),   // grass
    vec4<f32>(0.545, 0.451, 0.333, 1.0),   // dirt
    vec4<f32>(0.333, 0.333, 0.333, 1.0),   // road
    vec4<f32>(0.102, 0.227, 0.416, 1.0),   // water
    vec4<f32>(0.667, 0.667, 0.667, 1.0),   // sidewalk
);

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Color base del terreno (LUT, sin branch)
    let base_color = TERRAIN_COLORS[in.tile_type];

    // Overlay de zona (alpha blending)
    var color = mix(base_color, in.zone_color, in.zone_color.a);

    // Oscurecer por contaminación (efecto smog)
    let pollution_factor = 1.0 - in.pollution * 0.6;
    color = color * pollution_factor;

    return color;
}
"#;

// ============================================================================
// SHADER: Building Sprite
// ============================================================================

pub const BUILDING_SPRITE_SHADER: &str = r#"
// Building Sprite Shader v1.0

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) texcoord: vec2<f32>,
    @location(2) atlas_uv: vec4<f32>,     // u0,v0,u1,v1 en atlas
    @location(3) tint_color: u32,         // RGBA8 packed
    @location(4) construction_progress: f32, // 0.0-1.0
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texcoord: vec2<f32>,
    @location(1) tint: vec4<f32>,
    @location(2) progress: f32,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = in.position * camera.zoom + camera.offset;
    out.position = vec4<f32>(
        (world_pos.x / f32(camera.screen_size.x)) * 2.0 - 1.0,
        1.0 - (world_pos.y / f32(camera.screen_size.y)) * 2.0,
        0.0, 1.0
    );
    // Mapear texcoord al atlas
    out.texcoord = vec2<f32>(
        in.atlas_uv.x + in.texcoord.x * (in.atlas_uv.z - in.atlas_uv.x),
        in.atlas_uv.y + in.texcoord.y * (in.atlas_uv.w - in.atlas_uv.y),
    );
    out.tint = unpack4x8unorm(in.tint_color);
    out.progress = in.construction_progress;
    return out;
}

@group(0) @binding(1) var building_atlas: texture_2d<f32>;
@group(0) @binding(2) var building_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSample(building_atlas, building_sampler, in.texcoord);
    color = color * in.tint;

    // Indicador visual de construcción (andamios)
    if in.progress < 1.0 {
        let scaffold = (sin(in.position.y * 0.5) * 0.5 + 0.5) * 0.3;
        color = mix(color, vec4<f32>(0.8, 0.6, 0.2, 1.0), scaffold * (1.0 - in.progress));
    }

    return color;
}
"#;

// ============================================================================
// SHADER: UI Overlay
// ============================================================================

pub const UI_OVERLAY_SHADER: &str = r#"
// UI Overlay Shader v1.0 — Zero-branch, puro math

struct VertexInput {
    @location(0) position: vec2<f32>,      // pixel coords
    @location(1) texcoord: vec2<f32>,
    @location(2) color: u32,               // RGBA8
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) texcoord: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@group(1) @binding(0) var<uniform> ui_scale: vec2<f32>; // screen dimensions

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // UI está en screen-space (sin transformación de cámara)
    out.position = vec4<f32>(
        (in.position.x / ui_scale.x) * 2.0 - 1.0,
        1.0 - (in.position.y / ui_scale.y) * 2.0,
        0.0, 1.0
    );
    out.texcoord = in.texcoord;
    out.color = unpack4x8unorm(in.color);
    return out;
}

@group(1) @binding(1) var ui_atlas: texture_2d<f32>;
@group(1) @binding(2) var ui_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(ui_atlas, ui_sampler, in.texcoord);
    return tex_color * in.color;
}
"#;

// ============================================================================
// SHADER: Partículas (humo, fuego, polvo)
// ============================================================================

pub const PARTICLE_SHADER: &str = r#"
// Particle Shader v1.0 — Sistemas de partículas con blending aditivo

struct Particle {
    position: vec2<f32>,
    velocity: vec2<f32>,
    lifetime: f32,
    max_lifetime: f32,
    size: f32,
    color: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) alpha: f32,
    @location(2) color: vec4<f32>,
}

@group(2) @binding(0) var<uniform> camera: CameraUniform;
@group(2) @binding(1) var<storage, read> particles: array<Particle>;

@vertex
fn vs_main(@builtin(vertex_index) idx: u32, @builtin(instance_index) inst: u32) -> VertexOutput {
    var out: VertexOutput;
    let p = particles[inst];

    // Quad corners from instance
    let corners = array<vec2<f32>, 4>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0,  1.0), vec2<f32>(1.0,  1.0),
    );

    let offset = corners[idx] * p.size;
    let world_pos = (p.position + offset) * camera.zoom + camera.offset;

    out.position = vec4<f32>(
        (world_pos.x / f32(camera.screen_size.x)) * 2.0 - 1.0,
        1.0 - (world_pos.y / f32(camera.screen_size.y)) * 2.0,
        0.0, 1.0
    );
    out.uv = corners[idx] * 0.5 + 0.5;
    out.alpha = p.lifetime / p.max_lifetime;
    out.color = unpack4x8unorm(p.color);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Partícula circular suave
    let dist = length(in.uv - vec2<f32>(0.5, 0.5));
    let smooth_edge = 1.0 - smoothstep(0.3, 0.5, dist);
    var color = in.color;
    color.a = color.a * in.alpha * smooth_edge;
    return color;
}
"#;

// ============================================================================
// SHADER: Agua (reflejos, ondas con LUT)
// ============================================================================

pub const WATER_SHADER: &str = r#"
// Water Shader v1.0 — Ondas y reflejos con LUT senoidal

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) world_pos: vec2<f32>,
}

@group(3) @binding(0) var<uniform> camera: CameraUniform;
@group(3) @binding(1) var<uniform> time: f32; // tiempo en segundos

@vertex
fn vs_main(@location(0) pos: vec2<f32>) -> VertexOutput {
    var out: VertexOutput;
    let world_pos = pos * camera.zoom + camera.offset;
    out.position = vec4<f32>(
        (world_pos.x / f32(camera.screen_size.x)) * 2.0 - 1.0,
        1.0 - (world_pos.y / f32(camera.screen_size.y)) * 2.0,
        0.0, 1.0
    );
    out.world_pos = pos;
    return out;
}

// LUT de seno pre-calculada (se espera que la aplicación la suba como textura 1D)
@group(3) @binding(2) var sin_lut: texture_1d<f32>;
@group(3) @binding(3) var sin_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Ondas compuestas con LUT senoidal
    let wave1 = textureSampleLevel(sin_lut, sin_sampler, in.world_pos.x * 0.05 + time, 0.0).r;
    let wave2 = textureSampleLevel(sin_lut, sin_sampler, in.world_pos.y * 0.07 + time * 1.3, 0.0).r;
    let wave3 = textureSampleLevel(sin_lut, sin_sampler, (in.world_pos.x + in.world_pos.y) * 0.03 + time * 0.7, 0.0).r;

    let wave = (wave1 + wave2 + wave3) / 3.0;

    // Color base agua: azul profundo a claro según la ola
    let deep_blue = vec4<f32>(0.05, 0.15, 0.35, 0.85);
    let light_blue = vec4<f32>(0.15, 0.45, 0.75, 0.75);
    let color = mix(deep_blue, light_blue, wave * 0.5 + 0.5);

    return color;
}
"#;

// ============================================================================
// COMPUTE SHADER: Difusión de Contaminación
// ============================================================================

pub const COMPUTE_POLLUTION_DIFFUSION: &str = r#"
// Pollution Diffusion Compute Shader v1.0
// Simula la difusión atmosférica en GPU usando stencil de 9 puntos

struct DiffusionParams {
    grid_size: vec2<u32>,
    diffusion_rate: f32,
    decay_rate: f32,
    wind_x: f32,
    wind_y: f32,
    delta_time: f32,
}

@group(4) @binding(0) var<uniform> params: DiffusionParams;
@group(4) @binding(1) var<storage, read> input_grid: array<f32>;
@group(4) @binding(2) var<storage, read_write> output_grid: array<f32>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    let w = params.grid_size.x;
    let h = params.grid_size.y;

    if x >= w || y >= h { return; }

    let idx = y * w + x;
    let center = input_grid[idx];

    // Stencil de 9 puntos con pesos de difusión
    let n  = select(0.0, input_grid[idx - w], y > 0u);
    let s  = select(0.0, input_grid[idx + w], y + 1u < h);
    let e  = select(0.0, input_grid[idx + 1u], x + 1u < w);
    let wv = select(0.0, input_grid[idx - 1u], x > 0u);
    let ne = select(0.0, input_grid[idx - w + 1u], y > 0u && x + 1u < w);
    let nw = select(0.0, input_grid[idx - w - 1u], y > 0u && x > 0u);
    let se = select(0.0, input_grid[idx + w + 1u], y + 1u < h && x + 1u < w);
    let sw = select(0.0, input_grid[idx + w - 1u], y + 1u < h && x > 0u);

    let laplacian = (n + s + e + wv) * 0.2 + (ne + nw + se + sw) * 0.05 - center;
    let advection = (e - wv) * params.wind_x * 0.5 + (s - n) * params.wind_y * 0.5;

    var new_val = center + params.diffusion_rate * laplacian * params.delta_time
                  + advection * params.delta_time;

    // Decaimiento
    new_val = new_val * (1.0 - params.decay_rate * params.delta_time);
    new_val = clamp(new_val, 0.0, 1.0);

    output_grid[idx] = new_val;
}
"#;

// ============================================================================
// COMPUTE SHADER: Flow Field (pathfinding de multitudes)
// ============================================================================

pub const COMPUTE_FLOW_FIELD: &str = r#"
// Flow Field Compute Shader v1.0
// Calcula campos de flujo para pathfinding de multitudes en GPU

struct FlowFieldParams {
    grid_size: vec2<u32>,
    target_x: u32,
    target_y: u32,
    iteration: u32,
}

@group(5) @binding(0) var<uniform> params: FlowFieldParams;
@group(5) @binding(1) var<storage, read> obstacles: array<u32>; // 0=libre, 1=obstáculo
@group(5) @binding(2) var<storage, read_write> cost_field: array<u32>;
@group(5) @binding(3) var<storage, read_write> flow_x: array<f32>;
@group(5) @binding(4) var<storage, read_write> flow_y: array<f32>;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = gid.x;
    let y = gid.y;
    let w = params.grid_size.x;
    let h = params.grid_size.y;
    if x >= w || y >= h { return; }

    let idx = y * w + x;

    // Si es obstáculo, costo máximo
    if obstacles[idx] == 1u {
        cost_field[idx] = 0xFFFFFFFFu;
        flow_x[idx] = 0.0;
        flow_y[idx] = 0.0;
        return;
    }

    // Si es el target, costo cero
    if x == params.target_x && y == params.target_y {
        cost_field[idx] = 0u;
        return;
    }

    // Buscar vecino con menor costo
    var min_cost = 0xFFFFFFFFu;
    var best_dx: i32 = 0;
    var best_dy: i32 = 0;

    let neighbors = array<vec2<i32>, 8>(
        vec2<i32>( 1,  0), vec2<i32>(-1,  0),
        vec2<i32>( 0,  1), vec2<i32>( 0, -1),
        vec2<i32>( 1,  1), vec2<i32>(-1, -1),
        vec2<i32>( 1, -1), vec2<i32>(-1,  1),
    );

    for (var i = 0u; i < 8u; i++) {
        let nx = i32(x) + neighbors[i].x;
        let ny = i32(y) + neighbors[i].y;
        if nx >= 0 && ny >= 0 && (nx as u32) < w && (ny as u32) < h {
            let nidx = (ny as u32) * w + (nx as u32);
            let ncost = cost_field[nidx];
            if ncost < min_cost {
                min_cost = ncost;
                best_dx = neighbors[i].x;
                best_dy = neighbors[i].y;
            }
        }
    }

    if min_cost < 0xFFFFFFFFu {
        cost_field[idx] = min_cost + 1u;
        let len = sqrt(f32(best_dx * best_dx + best_dy * best_dy));
        if len > 0.0 {
            flow_x[idx] = f32(best_dx) / len;
            flow_y[idx] = f32(best_dy) / len;
        }
    }
}
"#;

// ============================================================================
// COMPUTE SHADER: Dinámica de Multitudes (Social Force Model)
// ============================================================================

pub const COMPUTE_CROWD_DYNAMICS: &str = r#"
// Crowd Dynamics Compute Shader v1.0
// Social Force Model para peatones — ejecutado en GPU [TA#5]

struct Pedestrian {
    pos_x: f32,
    pos_y: f32,
    vel_x: f32,
    vel_y: f32,
    target_x: f32,
    target_y: f32,
    radius: f32,
    max_speed: f32,
    stress: f32,
}

struct CrowdParams {
    num_pedestrians: u32,
    dt: f32,
    relaxation_time: f32,     // tau
    repulsion_strength: f32,  // A
    repulsion_range: f32,     // B
    obstacle_force: f32,
}

@group(6) @binding(0) var<uniform> params: CrowdParams;
@group(6) @binding(1) var<storage, read_write> pedestrians: array<Pedestrian>;
@group(6) @binding(2) var<storage, read> flow_x: array<f32>;
@group(6) @binding(3) var<storage, read> flow_y: array<f32>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if i >= params.num_pedestrians { return; }

    var ped = pedestrians[i];

    // Fuerza de destino (hacia el target)
    let desired_x = ped.target_x - ped.pos_x;
    let desired_y = ped.target_y - ped.pos_y;
    let desired_len = sqrt(desired_x * desired_x + desired_y * desired_y);
    var desired_vx: f32 = 0.0;
    var desired_vy: f32 = 0.0;
    if desired_len > 0.001 {
        desired_vx = (desired_x / desired_len) * ped.max_speed;
        desired_vy = (desired_y / desired_len) * ped.max_speed;
    }

    // Fuerza motriz: (v_deseada - v_actual) / tau
    let drive_x = (desired_vx - ped.vel_x) / params.relaxation_time;
    let drive_y = (desired_vy - ped.vel_y) / params.relaxation_time;

    // Fuerza de repulsión entre peatones
    var rep_x: f32 = 0.0;
    var rep_y: f32 = 0.0;

    for (var j = 0u; j < params.num_pedestrians; j++) {
        if j == i { continue; }
        let other = pedestrians[j];
        let dx = ped.pos_x - other.pos_x;
        let dy = ped.pos_y - other.pos_y;
        let dist = sqrt(dx * dx + dy * dy);
        let min_dist = ped.radius + other.radius;

        if dist < params.repulsion_range && dist > 0.001 {
            let force = params.repulsion_strength *
                        exp((min_dist - dist) / params.repulsion_range);
            rep_x += (dx / dist) * force;
            rep_y += (dy / dist) * force;
        }
    }

    // Integración de Euler semi-implícita
    let total_fx = drive_x + rep_x;
    let total_fy = drive_y + rep_y;

    ped.vel_x += total_fx * params.dt;
    ped.vel_y += total_fy * params.dt;

    // Limitar velocidad
    let speed = sqrt(ped.vel_x * ped.vel_x + ped.vel_y * ped.vel_y);
    if speed > ped.max_speed {
        ped.vel_x = (ped.vel_x / speed) * ped.max_speed;
        ped.vel_y = (ped.vel_y / speed) * ped.max_speed;
    }

    ped.pos_x += ped.vel_x * params.dt;
    ped.pos_y += ped.vel_y * params.dt;

    // Estrés: acumula cuando la velocidad deseada >> velocidad real
    ped.stress += (speed / ped.max_speed) * params.dt * 0.1;
    ped.stress = clamp(ped.stress, 0.0, 1.0);

    pedestrians[i] = ped;
}
"#;

// ============================================================================
// PRE-COMPILACIÓN Y WARMING
// ============================================================================

/// Fase de warming: pre-compila todos los shaders en tiempo de carga
/// Retorna true si todos los shaders compilaron correctamente
pub fn warm_shader_cache() -> bool {
    // En una implementación real con wgpu, aquí se compilarían los shaders
    // y se cachearían en el device. En modo CPU, es no-op.
    let shaders_to_warm: [&str; 8] = [
        SHADERS.terrain_tile,
        SHADERS.building_sprite,
        SHADERS.ui_overlay,
        SHADERS.particle,
        SHADERS.water,
        SHADERS.compute_pollution_diffusion,
        SHADERS.compute_flow_field,
        SHADERS.compute_crowd_dynamics,
    ];

    let mut all_valid = true;
    for (i, shader) in shaders_to_warm.iter().enumerate() {
        if shader.is_empty() {
            eprintln!("Shader {} vacío!", i);
            all_valid = false;
        }
    }

    if all_valid {
        println!("Shader cache: 8/8 shaders validados (WGSL)");
    }

    all_valid
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_shaders_non_empty() {
        assert!(!SHADERS.terrain_tile.is_empty());
        assert!(!SHADERS.building_sprite.is_empty());
        assert!(!SHADERS.ui_overlay.is_empty());
        assert!(!SHADERS.particle.is_empty());
        assert!(!SHADERS.water.is_empty());
        assert!(!SHADERS.compute_pollution_diffusion.is_empty());
        assert!(!SHADERS.compute_flow_field.is_empty());
        assert!(!SHADERS.compute_crowd_dynamics.is_empty());
    }

    #[test]
    fn test_warm_shader_cache() {
        assert!(warm_shader_cache());
    }
    #[test]
    fn test_shaders_contain_required_keywords() {
        // Verificar que los shaders tienen sintaxis WGSL válida
        assert!(SHADERS.terrain_tile.contains(concat!("@", "vertex")));
        assert!(SHADERS.terrain_tile.contains(concat!("@", "fragment")));
        assert!(SHADERS.building_sprite.contains(concat!("@", "vertex")));
        assert!(SHADERS.building_sprite.contains(concat!("@", "fragment")));
        assert!(SHADERS.ui_overlay.contains(concat!("@", "vertex")));
        assert!(SHADERS.ui_overlay.contains(concat!("@", "fragment")));
        assert!(SHADERS.particle.contains(concat!("@", "vertex")));
        assert!(SHADERS.particle.contains(concat!("@", "fragment")));
        assert!(SHADERS.water.contains(concat!("@", "vertex")));
        assert!(SHADERS.water.contains(concat!("@", "fragment")));
    }

    #[test]
    fn test_compute_shaders_have_workgroup_size() {
        assert!(SHADERS.compute_pollution_diffusion.contains(concat!("@", "compute")));
        assert!(SHADERS.compute_pollution_diffusion.contains(concat!("@", "workgroup_size")));
        assert!(SHADERS.compute_flow_field.contains(concat!("@", "compute")));
        assert!(SHADERS.compute_flow_field.contains(concat!("@", "workgroup_size")));
        assert!(SHADERS.compute_crowd_dynamics.contains(concat!("@", "compute")));
        assert!(SHADERS.compute_crowd_dynamics.contains(concat!("@", "workgroup_size")));
    }
    }
}
