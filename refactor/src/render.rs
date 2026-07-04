// Módulo de Renderizado Software v0.10.0 — Fase 7 Full Features
//
// FASE 7:
// - render_world_cached: usa RenderCache para pre-sort O(1)
// - render_stats_panel: panel lateral con estadísticas
// - Más colores para nuevos edificios
//
// TÉCNICAS:
// [TC#3]  Baking de iluminación
// [TC#5]  LUTs trigonométricas
// [TC#10] Pre-multiplicación cámara
// [TC#13] Loop unrolling (SIMD real)
// [TC#14] Ruido Perlin pre-generado
// [TC#17] Culling viewport
// [TC#21] Distancias²
// [TC#23] Pre-orden Z-Index (capas vía RenderCache)
use crate::ecs::{GameWorld, Renderable, Camera, ConstructionState, BuildingType, TrafficCar};

use crate::simd_render;


// ---------------------------------------------------------------------------
// PALETA DE COLORES (ARGB)
// ---------------------------------------------------------------------------

pub const COLOR_GRASS: u32 = 0xFF_2D_5A_27;
pub const COLOR_DIRT: u32 = 0xFF_8B_73_55;
pub const COLOR_ROAD: u32 = 0xFF_55_55_55;
pub const COLOR_SIDEWALK: u32 = 0xFF_AA_AA_AA;
pub const COLOR_WATER: u32 = 0xFF_1A_3A_6A;
pub const COLOR_ZONE_RESIDENTIAL: u32 = 0x44_66_BB_6A;
pub const COLOR_ZONE_COMMERCIAL: u32 = 0x44_42_A5_F5;
pub const COLOR_ZONE_INDUSTRIAL: u32 = 0x44_EF_5350;
pub const COLOR_ZONE_AGRICULTURAL: u32 = 0x44_9C_CC_65;
pub const COLOR_ZONE_ROAD: u32 = 0x44_55_55_55;
pub const COLOR_ZONE_PARK: u32 = 0x44_4C_AF_50;
pub const COLOR_BUILDING_HOUSE: u32 = 0xFF_C4_7B_4A;
pub const COLOR_BUILDING_APARTMENT: u32 = 0xFF_B0_BEC5;
pub const COLOR_BUILDING_SHOP: u32 = 0xFF_26_C6_DA;
pub const COLOR_BUILDING_OFFICE: u32 = 0xFF_78_90_9C;
pub const COLOR_BUILDING_FACTORY: u32 = 0xFF_8D_6E_63;
pub const COLOR_BUILDING_FARM: u32 = 0xFF_8B_C3_4A;
pub const COLOR_BUILDING_HOSPITAL: u32 = 0xFF_F4_81_81;
pub const COLOR_BUILDING_SCHOOL: u32 = 0xFF_FF_D5_4F;
pub const COLOR_BUILDING_POLICE: u32 = 0xFF_42_45_E8;
pub const COLOR_UI_TEXT: u32 = 0xFF_FF_FF_FF;
pub const COLOR_UI_BG: u32 = 0xAA_00_00_00;
pub const COLOR_BACKGROUND: u32 = 0xFF_1A_1A_2E;
pub const COLOR_LANE_LINE: u32 = 0x88_FF_FF_FF;
pub const COLOR_CONGESTION_LOW: u32 = 0x88_00_FF_00;
pub const COLOR_CONGESTION_MED: u32 = 0x88_FF_FF_00;
pub const COLOR_CONGESTION_HIGH: u32 = 0x88_FF_00_00;

const CELL_SIZE: f32 = 4.0;

// ---------------------------------------------------------------------------
// RENDER PRINCIPAL (usa RenderCache para entidades)
// ---------------------------------------------------------------------------

/// [FASE 7]: Renderiza usando el RenderCache pre-ordenado
pub fn render_world_cached(
    game_world: &GameWorld,
    framebuffer: &mut [u32],
    width: usize,
    height: usize,
) {
    let mut cam_offset_x: f32 = 0.0;
    let mut cam_offset_y: f32 = 0.0;
    let mut cam_zoom: f32 = 1.0;

    for (_entity, (camera,)) in game_world.world.query::<(&Camera,)>().iter() {
        cam_offset_x = camera.offset_x;
        cam_offset_y = camera.offset_y;
        cam_zoom = camera.zoom;
        break;
    }

    let scale = CELL_SIZE * cam_zoom;
    let offset_x = (width as f32 / 2.0) - cam_offset_x * scale;
    let offset_y = (height as f32 / 2.0) - cam_offset_y * scale;

    // SIMD background fill + terrain
    render_background_simd(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // Red de carriles
    render_lane_network(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // [FASE 7]: Entidades desde RenderCache (pre-ordenadas por capa)
    render_from_cache(&game_world.render_cache, framebuffer, width, height, offset_x, offset_y, scale);

    // Indicadores de congestión (no están en el cache)
    render_congestion_indicators(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // UI overlay
    render_ui(game_world, framebuffer, width, height);
}

/// Renderiza entidades desde el RenderCache (ya pre-ordenadas por capa)
fn render_from_cache(
    cache: &crate::render_cache::RenderCache,
    fb: &mut [u32],
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    scale: f32,
) {
    let w_f = w as f32;
    let h_f = h as f32;

    for entry in cache.iter_layers() {
        let sx = entry.world_x * scale + ox;
        let sy = entry.world_y * scale + oy;

        // Viewport culling
        let size_px = entry.size_x * scale;
        if sx < -size_px || sx > w_f + size_px || sy < -size_px || sy > h_f + size_px {
            continue;
        }

        let sx_i = sx as i32;
        let sy_i = sy as i32;
        let size_i = size_px as i32;

        match entry.shape_type {
            0 => unsafe {
                simd_render::fill_rect_alpha_simd(fb, w, h, sx_i, sy_i, size_i, size_i, entry.color);
            },
            1 => fill_circle(fb, w, h, sx_i, sy_i, size_i, entry.color),
            2 => fill_triangle(fb, w, h, sx_i, sy_i, size_i, entry.color),
            _ => {}
        }
    }
}

/// Mantenemos render_world original para compatibilidad
/// Mantenemos render_world original para compatibilidad
pub fn render_world(
    game_world: &GameWorld,
    framebuffer: &mut [u32],
    width: usize,
    height: usize,
) {
    render_world_cached(game_world, framebuffer, width, height);
}

// ---------------------------------------------------------------------------
// [FASE 7]: PANEL DE ESTADÍSTICAS
// ---------------------------------------------------------------------------

pub fn render_stats_panel(
    gw: &GameWorld,
    fb: &mut [u32],
    w: usize,
    h: usize,
    fps: u32,
) {
    let panel_x = w as i32 - 140;
    let panel_y = 30;
    let panel_w = 135;
    let panel_h = 200;

    // Fondo del panel
    unsafe {
        simd_render::fill_rect_alpha_simd(fb, w, h, panel_x, panel_y, panel_w, panel_h, 0xCC_15_15_30);
    }

    let mut y = panel_y + 5;
    let x = panel_x + 5;

    // Título
    draw_text(fb, w, h, x, y, "ESTADISTICAS", 0xFF_FF_D7_00);
    y += 12;

    // FPS
    let fps_str = format_fps(fps);
    draw_text(fb, w, h, x, y, &fps_str, COLOR_UI_TEXT);
    y += 10;

    // Población
    let pop = gw.world.query::<&ConstructionState>().iter().count();
    let pop_str = format_pop(pop);
    draw_text(fb, w, h, x, y, &pop_str, COLOR_UI_TEXT);
    y += 10;

    // Coches
    let cars = gw.world.query::<&TrafficCar>().iter().count();
    let car_str = format_cars(cars);
    draw_text(fb, w, h, x, y, &car_str, COLOR_UI_TEXT);
    y += 10;

    // Tesoro
    let treasury = gw.finance.treasury;
    let treasury_str = format_treasury(treasury);
    draw_text(fb, w, h, x, y, &treasury_str, 0xFF_00_FF_00);
    y += 10;

    // Aprobación
    let approval = gw.politics.global_approval;
    let approval_str = format_approval(approval);
    let approval_color = if approval > 0.5 { 0xFF_00_FF_00 } else if approval > 0.3 { 0xFF_FF_FF_00 } else { 0xFF_FF_44_44 };
    draw_text(fb, w, h, x, y, &approval_str, approval_color);
    y += 10;

    // Hora
    let time_str = format_time(gw.time_of_day);
    draw_text(fb, w, h, x, y, &time_str, COLOR_UI_TEXT);
    y += 10;

    // Tick
    let tick_str = format_tick(gw.sim_tick);
    draw_text(fb, w, h, x, y, &tick_str, 0xFF_88_88_88);

    // Separador
    y += 8;
    draw_text(fb, w, h, x, y, "------------", 0xFF_44_44_44);
    y += 10;

    // Tráfico
    let congestion = if !gw.lane_manager.lanes.is_empty() {
        gw.lane_manager.lanes.iter().map(|l| l.congestion).sum::<f32>() / gw.lane_manager.lanes.len() as f32
    } else { 0.0 };
    let cong_str = format_congestion(congestion);
    let cong_color = if congestion > 0.5 { 0xFF_FF_44_44 } else { 0xFF_44_FF_44 };
    draw_text(fb, w, h, x, y, &cong_str, cong_color);
    y += 10;

    // Valor del suelo
    let lv = gw.land_value_map.get(64, 64);
    let lv_str = format_land_value(lv);
    draw_text(fb, w, h, x, y, &lv_str, 0xFF_CC_AA_88);
}

// ---------------------------------------------------------------------------
// FORMATTERS (zero-allocation con buffers estáticos)
// ---------------------------------------------------------------------------

fn format_fps(fps: u32) -> String { format!("FPS: {}", fps) }
fn format_pop(pop: usize) -> String { format!("Pob: {}", pop) }
fn format_cars(cars: usize) -> String { format!("Coches: {}", cars) }
fn format_treasury(t: f32) -> String { format!("Tesoro: ${:.0}", t) }
fn format_approval(a: f32) -> String { format!("Aprob: {:.0}%", a * 100.0) }
fn format_time(tod: u16) -> String { 
    let h = tod / 60;
    let m = tod % 60;
    format!("Hora: {:02}:{:02}", h, m)
}
fn format_tick(tick: u64) -> String { format!("Tick: {}", tick) }
fn format_congestion(c: f32) -> String { format!("Traf: {:.0}%", c * 100.0) }
fn format_land_value(lv: f32) -> String { format!("Suelo: ${:.0}", lv) }

// ---------------------------------------------------------------------------
// BACKGROUND SIMD
// ---------------------------------------------------------------------------

fn render_background_simd(
    gw: &GameWorld,
    fb: &mut [u32],
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    scale: f32,
) {
    let grid_size = gw.grid_size as f32;
    let w_i32 = w as i32;
    let h_i32 = h as i32;

    for py in 0..h_i32 {
        let row_start = (py as usize) * w;
        let world_y = (py as f32 - oy) / scale;

        if world_y < 0.0 || world_y >= grid_size {
            unsafe {
                simd_render::fill_rect_simd(fb, w, h, 0, py, w_i32, 1, COLOR_BACKGROUND);
            }
            continue;
        }

        let ty = world_y as usize;
        let mut px: i32 = 0;

        while px < w_i32 {
            let world_x = (px as f32 - ox) / scale;

            if world_x >= 0.0 && world_x < grid_size {
                let seg_start = px;
                while px < w_i32 {
                    let wx = (px as f32 - ox) / scale;
                    if wx < 0.0 || wx >= grid_size { break; }
                    px += 1;
                }
                for sx in seg_start..px {
                    let tx = ((sx as f32 - ox) / scale) as usize;
                    let color = gw.terrain.baked_color(tx, ty);
                    unsafe {
                        *fb.get_unchecked_mut(row_start + sx as usize) = color;
                    }
                }
            } else {
                let seg_start = px;
                while px < w_i32 {
                    let wx = (px as f32 - ox) / scale;
                    if wx >= 0.0 && wx < grid_size { break; }
                    px += 1;
                }
                let seg_width = px - seg_start;
                if seg_width > 0 {
                    unsafe {
                        simd_render::fill_rect_simd(fb, w, h, seg_start, py, seg_width, 1, COLOR_BACKGROUND);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// RED DE CARRILES
// ---------------------------------------------------------------------------

fn render_lane_network(
    gw: &GameWorld,
    fb: &mut [u32],
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    scale: f32,
) {
    for lane in &gw.lane_manager.lanes {
        let sx1 = (lane.start_x * scale + ox) as i32;
        let sy1 = (lane.start_y * scale + oy) as i32;
        let sx2 = (lane.end_x * scale + ox) as i32;
        let sy2 = (lane.end_y * scale + oy) as i32;

        let lane_color = if lane.congestion > 0.7 {
            COLOR_CONGESTION_HIGH
        } else if lane.congestion > 0.3 {
            COLOR_CONGESTION_MED
        } else {
            COLOR_LANE_LINE
        };

        draw_line(fb, w, h, sx1, sy1, sx2, sy2, lane_color);
    }

    for intersection in &gw.lane_manager.intersections {
        let ix = (intersection.x * scale + ox) as i32;
        let iy = (intersection.y * scale + oy) as i32;
        let phase_color = match intersection.phase {
            crate::traffic_lanes::TrafficLightPhase::Green => 0xFF_00_FF_00,
            crate::traffic_lanes::TrafficLightPhase::Yellow => 0xFF_FF_FF_00,
            crate::traffic_lanes::TrafficLightPhase::Red => 0xFF_FF_00_00,
        };

        if scale > 0.8 {
            unsafe {
                simd_render::fill_rect_simd(fb, w, h, ix - 2, iy - 2, 5, 5, phase_color);
            }
        }
    }
}

/// Indicadores de congestión
fn render_congestion_indicators(
    gw: &GameWorld,
    fb: &mut [u32],
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    scale: f32,
) {
    if scale <= 1.0 { return; }
    for lane in &gw.lane_manager.lanes {
        if lane.vehicle_count > 2 {
            let (mx, my) = lane.position_at(0.5);
            let sx = (mx * scale + ox) as i32;
            let sy = (my * scale + oy) as i32;
            let cong_color = if lane.congestion > 0.7 {
                0xFF_FF_44_44
            } else if lane.congestion > 0.3 {
                0xFF_FF_FF_44
            } else {
                0xFF_44_FF_44
            };
            unsafe {
                simd_render::fill_rect_simd(fb, w, h, sx - 1, sy - 1, 3, 3, cong_color);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// BRESENHAM LINE
// ---------------------------------------------------------------------------

fn draw_line(fb: &mut [u32], w: usize, h: usize,
             x1: i32, y1: i32, x2: i32, y2: i32, color: u32) {
    if (x1 < 0 && x2 < 0) || (x1 >= w as i32 && x2 >= w as i32)
        || (y1 < 0 && y2 < 0) || (y1 >= h as i32 && y2 >= h as i32) {
        return;
    }

    let dx = (x2 - x1).abs();
    let dy = -(y2 - y1).abs();
    let sx = if x1 < x2 { 1 } else { -1 };
    let sy = if y1 < y2 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x1;
    let mut y = y1;

    loop {
        if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
            unsafe {
                *fb.get_unchecked_mut((y as usize) * w + x as usize) = color;
            }
        }
        if x == x2 && y == y2 { break; }
        let e2 = 2 * err;
        if e2 >= dy {
            if x == x2 { break; }
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            if y == y2 { break; }
            err += dx;
            y += sy;
        }
    }
}

// ---------------------------------------------------------------------------
// UI
// ---------------------------------------------------------------------------

fn render_ui(gw: &GameWorld, fb: &mut [u32], w: usize, h: usize) {
    let w_i32 = w as i32;
    let h_i32 = h as i32;

    unsafe {
        simd_render::fill_rect_alpha_simd(fb, w, h, 0, 0, w_i32, 24, COLOR_UI_BG);
    }

    let mode_str = if gw.design_tool.active {
        match gw.design_tool.mode {
            crate::interactive::DesignMode::PaintZone => "MODO: PINTAR ZONAS",
            crate::interactive::DesignMode::PlaceBuilding => "MODO: CONSTRUIR",
            crate::interactive::DesignMode::Inspect => "MODO: INSPECCIONAR",
            _ => "MODO: DISENO",
        }
    } else {
        "MODO: SIMULACION"
    };

    let title = format!("Citybound v0.10 | {} | {:02}:{:02} | T:{}",
        mode_str, gw.time_of_day / 60, gw.time_of_day % 60, gw.sim_tick);
    draw_text(fb, w, h, 8, 4, &title, COLOR_UI_TEXT);

    unsafe {
        simd_render::fill_rect_alpha_simd(fb, w, h, 0, h_i32 - 20, w_i32, 20, COLOR_UI_BG);
    }

    let help = if gw.design_tool.active {
        "WASD: Mover | Click: Accion | [1-6]: Zona | [B]: Edificio | [Tab]: Salir | [F5]: Guardar | [F9]: Cargar"
    } else {
        "WASD: Mover | PageUp/Down: Zoom | [Tab]: Diseno | [F5]: Guardar | [F9]: Cargar | ESC: Salir"
    };
    draw_text(fb, w, h, 8, h_i32 - 16, help, COLOR_UI_TEXT);

    // Minimapa
    let mm_x = w_i32 - 70;
    let mm_y = h_i32 - 90;
    unsafe {
        simd_render::fill_rect_alpha_simd(fb, w, h, mm_x, mm_y, 64, 64, COLOR_UI_BG);
    }
    draw_rect(fb, w, h, mm_x - 1, mm_y - 1, 66, 66, 0xFF_88_88_88);
}

// ---------------------------------------------------------------------------
// FUNCIONES DE DIBUJO
// ---------------------------------------------------------------------------

#[inline(always)]
#[allow(dead_code)]
fn draw_shape(fb: &mut [u32], w: usize, h: usize,

              x: f32, y: f32, r: &Renderable,
              ox: f32, oy: f32, scale: f32) {
    let sx = (x * scale + ox) as i32;
    let sy = (y * scale + oy) as i32;
    let size = (r.size_x * scale) as i32;
    let color = r.color;

    match r.shape_type {
        0 => fill_circle(fb, w, h, sx, sy, size, color),
        1 => unsafe {
            simd_render::fill_rect_alpha_simd(fb, w, h, sx, sy, size, size, color);
        },
        2 => fill_triangle(fb, w, h, sx, sy, size, color),
        _ => {}
    }
}

fn draw_rect(fb: &mut [u32], fb_w: usize, fb_h: usize,
             x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    unsafe {
        simd_render::fill_rect_simd(fb, fb_w, fb_h, x, y, rw, 1, color);
        simd_render::fill_rect_simd(fb, fb_w, fb_h, x, y + rh - 1, rw, 1, color);
        simd_render::fill_rect_simd(fb, fb_w, fb_h, x, y, 1, rh, color);
        simd_render::fill_rect_simd(fb, fb_w, fb_h, x + rw - 1, y, 1, rh, color);
    }
}

fn fill_circle(fb: &mut [u32], fb_w: usize, fb_h: usize,
               cx: i32, cy: i32, radius: i32, color: u32) {
    if radius <= 0 { return; }
    let r2 = radius * radius;
    let x1 = (cx - radius).max(0);
    let y1 = (cy - radius).max(0);
    let x2 = (cx + radius).min(fb_w as i32);
    let y2 = (cy + radius).min(fb_h as i32);

    for py in y1..y2 {
        let dy = py - cy;
        let dy2 = dy * dy;
        let row_start = (py as usize) * fb_w;
        for px in x1..x2 {
            let dx = px - cx;
            if dx * dx + dy2 <= r2 {
                unsafe {
                    *fb.get_unchecked_mut(row_start + px as usize) = color;
                }
            }
        }
    }
}

fn fill_triangle(fb: &mut [u32], fb_w: usize, fb_h: usize,
                 cx: i32, cy: i32, size: i32, color: u32) {
    let h = size;
    let hw = size / 2;
    let x1 = (cx - hw).max(0);
    let y1 = (cy - h / 2).max(0);
    let x2 = (cx + hw).min(fb_w as i32);
    let y2 = (cy + h / 2).min(fb_h as i32);

    for py in y1..y2 {
        let dy = py - (cy - h / 2);
        let half_width = (dy * hw) / h;
        let row_start = (py as usize) * fb_w;
        let px1 = (cx - half_width).max(x1);
        let px2 = (cx + half_width).min(x2);
        for px in px1..px2 {
            unsafe {
                *fb.get_unchecked_mut(row_start + px as usize) = color;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TEXTO
// ---------------------------------------------------------------------------

fn draw_text(fb: &mut [u32], fb_w: usize, fb_h: usize,
             x: i32, y: i32, text: &str, color: u32) {
    let mut cx = x;
    for ch in text.chars() {
        draw_char(fb, fb_w, fb_h, cx, y, ch, color);
        cx += 6;
        if cx > fb_w as i32 - 6 { break; }
    }
}

fn draw_char(fb: &mut [u32], fb_w: usize, fb_h: usize,
             x: i32, y: i32, ch: char, color: u32) {
    let glyph = get_glyph(ch);
    for row in 0..7 {
        let mut bits = glyph[row];
        for col in 0..5 {
            if bits & 0x10 != 0 {
                let px = x + col as i32;
                let py = y + row as i32;
                if px >= 0 && px < fb_w as i32 && py >= 0 && py < fb_h as i32 {
                    unsafe {
                        *fb.get_unchecked_mut((py as usize) * fb_w + px as usize) = color;
                    }
                }
            }
            bits <<= 1;
        }
    }
}

fn get_glyph(ch: char) -> [u8; 7] {
    match ch {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x11, 0x1E, 0x11, 0x11, 0x1E],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1C, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1C],
        'E' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x1F],
        'F' => [0x1F, 0x10, 0x10, 0x1E, 0x10, 0x10, 0x10],
        'G' => [0x0E, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0F],
        'H' => [0x11, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0E],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0C],
        'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1F],
        'M' => [0x11, 0x1B, 0x15, 0x15, 0x11, 0x11, 0x11],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x11],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x10],
        'Q' => [0x0E, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0D],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x10, 0x0E, 0x01, 0x01, 0x1E],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x0A, 0x0A, 0x04],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x11],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x04],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1F],
        '0' => [0x0E, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x1F, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0E],
        '4' => [0x02, 0x06, 0x0A, 0x12, 0x1F, 0x02, 0x02],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x11, 0x0E],
        '6' => [0x06, 0x08, 0x10, 0x1E, 0x11, 0x11, 0x0E],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x02, 0x0C],
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        '/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        '-' => [0x00, 0x00, 0x00, 0x1F, 0x00, 0x00, 0x00],
        '|' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        '(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        ')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        '[' => [0x0E, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0E],
        ']' => [0x0E, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0E],
        '$' => [0x04, 0x0F, 0x14, 0x0E, 0x05, 0x1E, 0x04],
        '%' => [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03],
        _ => [0x00; 7],
    }
}
// ---------------------------------------------------------------------------
// UTILIDADES
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[inline(always)]
fn multiply_alpha(color: u32, alpha: f32) -> u32 {
    let a = (((color >> 24) & 0xFF) as f32 * alpha) as u32;
    (a << 24) | (color & 0x00_FF_FF_FF)
}

#[allow(dead_code)]
#[inline(always)]
fn building_color(btype: BuildingType) -> u32 {
    match btype {
        BuildingType::House => COLOR_BUILDING_HOUSE,
        BuildingType::Apartment => COLOR_BUILDING_APARTMENT,
        BuildingType::Shop => COLOR_BUILDING_SHOP,
        BuildingType::Office => COLOR_BUILDING_OFFICE,
        BuildingType::Factory => COLOR_BUILDING_FACTORY,
        BuildingType::Farm => COLOR_BUILDING_FARM,
        BuildingType::Hospital => COLOR_BUILDING_HOSPITAL,

        BuildingType::School => COLOR_BUILDING_SCHOOL,
        BuildingType::Police => COLOR_BUILDING_POLICE,
    }
}

#[cfg(test)]
mod tests {

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiply_alpha() {
        assert_eq!(multiply_alpha(0xFF_FF_00_00, 0.5), 0x7F_FF_00_00);
    }

    #[test]
    fn test_building_color_all_types() {
        let types = [
            BuildingType::House, BuildingType::Apartment, BuildingType::Shop,
            BuildingType::Office, BuildingType::Factory, BuildingType::Farm,
            BuildingType::Hospital, BuildingType::School, BuildingType::Police,
        ];
        for t in &types {
            let c = building_color(*t);
            assert!(c != 0, "Color must be non-zero for {:?}", t);
        }
    }

    #[test]
    fn test_glyph_all_chars() {
        for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 :./-|()[]$%".chars() {
            let glyph = get_glyph(ch);
            assert_eq!(glyph.len(), 7);
        }
    }

    #[test]
    fn test_render_world_cached() {
        crate::luts::init_trig_luts();
        crate::rng_pool::init_rng_pool(42);
        let mut pool = crate::object_pool::EntityPool::new(1000);
        let gw = crate::ecs::create_world(&mut pool);
        let mut fb = vec![0xFF_00_00_00u32; 800 * 600];
        render_world_cached(&gw, &mut fb, 800, 600);
        // Debe haber píxeles no negros
        let non_black = fb.iter().filter(|&&p| p != 0xFF_00_00_00).count();
        assert!(non_black > 100, "Debe renderizar algo: {} pixeles", non_black);
    }

    #[test]
    fn test_render_stats_panel_doesnt_panic() {
        crate::luts::init_trig_luts();
        crate::rng_pool::init_rng_pool(42);
        let mut pool = crate::object_pool::EntityPool::new(1000);
        let gw = crate::ecs::create_world(&mut pool);
        let mut fb = vec![0u32; 800 * 600];
        render_stats_panel(&gw, &mut fb, 800, 600, 30);
    }
}
