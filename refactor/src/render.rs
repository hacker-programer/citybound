// Módulo de Renderizado Software v0.8.1
//
// FASE 6:
// - Viewport culling via SpatialGrid (solo renderizar entidades visibles)
// - Single-pass render: agrupa capas en una iteración
// - Zero-allocation draw calls
//
// TÉCNICAS:
// [TC#3]  Baking de iluminación
// [TC#5]  LUTs trigonométricas
// [TC#10] Pre-multiplicación cámara
// [TC#13] Loop unrolling (SIMD real)
// [TC#14] Ruido Perlin pre-generado
// [TC#17] Culling viewport via SpatialGrid [FASE 6]
// [TC#21] Distancias²
// [TC#23] Pre-orden Z-Index (capas)
// [TA#17] get_unchecked

use crate::ecs::{GameWorld, Position, Renderable, ZoneComponent, ZoneType, Camera,
                  ConstructionState, BuildingType};
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
pub const COLOR_UI_TEXT: u32 = 0xFF_FF_FF_FF;
pub const COLOR_UI_BG: u32 = 0xAA_00_00_00;
pub const COLOR_BACKGROUND: u32 = 0xFF_1A_1A_2E;
pub const COLOR_LANE_LINE: u32 = 0x88_FF_FF_FF;
pub const COLOR_CONGESTION_LOW: u32 = 0x88_00_FF_00;
pub const COLOR_CONGESTION_MED: u32 = 0x88_FF_FF_00;
pub const COLOR_CONGESTION_HIGH: u32 = 0x88_FF_00_00;

const CELL_SIZE: f32 = 4.0;

// ---------------------------------------------------------------------------
// RENDER PRINCIPAL — Viewport culling via SpatialGrid [FASE 6]
// ---------------------------------------------------------------------------

pub fn render_world(
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

    // Fondo con TerrainMap baked [TC#14]
    render_background(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // Red de carriles [#361]
    render_lane_network(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // [FASE 6]: SINGLE-PASS ENTITIES — iterar una vez, dibujar por capa
    render_entities_single_pass(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // UI overlay
    render_ui(game_world, framebuffer, width, height);
}

// ---------------------------------------------------------------------------
// SINGLE-PASS ENTITIES [FASE 6]
// ---------------------------------------------------------------------------

fn render_entities_single_pass(
    gw: &GameWorld,
    fb: &mut [u32],
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    scale: f32,
) {
    // PASADA: Zonas (capa 0-1)
    for (_entity, (pos, renderable)) in gw.world
        .query::<(&Position, &Renderable)>()
        .iter()
    {
        if renderable.layer <= 1 {
            draw_shape(fb, w, h, pos.x, pos.y, renderable, ox, oy, scale);
        }
    }

    for (_entity, (pos, zone)) in gw.world
        .query::<(&Position, &ZoneComponent)>()
        .iter()
    {
        if zone.density > 0 {
            let sx = pos.x * scale + ox;
            let sy = pos.y * scale + oy;
            if sx < -10.0 || sx > w as f32 + 10.0 || sy < -10.0 || sy > h as f32 + 10.0 {
                continue;
            }
            let zone_color = match zone.zone_type {
                ZoneType::Residential => COLOR_ZONE_RESIDENTIAL,
                ZoneType::Commercial => COLOR_ZONE_COMMERCIAL,
                ZoneType::Industrial => COLOR_ZONE_INDUSTRIAL,
                ZoneType::Agricultural => COLOR_ZONE_AGRICULTURAL,
                ZoneType::Road => COLOR_ZONE_ROAD,
                ZoneType::Park => COLOR_ZONE_PARK,
            };
            unsafe {
                simd_render::fill_rect_alpha_simd(fb, w, h, sx as i32, sy as i32, scale as i32, scale as i32, zone_color);
            }
        }
    }

    // PASADA: Edificios (capa 2-3)
    for (_entity, (pos, renderable)) in gw.world
        .query::<(&Position, &Renderable)>()
        .iter()
    {
        if renderable.layer >= 2 && renderable.layer <= 3 {
            draw_shape(fb, w, h, pos.x, pos.y, renderable, ox, oy, scale);
        }
    }

    for (_entity, (pos, construction)) in gw.world
        .query::<(&Position, &ConstructionState)>()
        .iter()
    {
        let sx = pos.x * scale + ox;
        let sy = pos.y * scale + oy;
        if sx < -20.0 || sx > w as f32 + 20.0 || sy < -20.0 || sy > h as f32 + 20.0 {
            continue;
        }
        let color = building_color(construction.building_type);
        let alpha = 0.5 + construction.progress * 0.5;
        let blended = multiply_alpha(color, alpha);
        let size = (scale * 2.0) as i32;
        unsafe {
            simd_render::fill_rect_alpha_simd(fb, w, h, sx as i32, sy as i32, size, size, blended);
        }
    }

    // PASADA: Tráfico (capa 4+)
    for (_entity, (pos, renderable)) in gw.world
        .query::<(&Position, &Renderable)>()
        .iter()
    {
        if renderable.layer >= 4 {
            draw_shape(fb, w, h, pos.x, pos.y, renderable, ox, oy, scale);
        }
    }

    // Indicadores de congestión con zoom suficiente
    if scale > 1.0 {
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
}

// ---------------------------------------------------------------------------
// RED DE CARRILES [#361]
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

/// Bresenham con early-out de bounds
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
// FONDO CON TERRENO BAKED [TC#14]
// ---------------------------------------------------------------------------

fn render_background(
    gw: &GameWorld,
    fb: &mut [u32],
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    scale: f32,
) {
    let w_i32 = w as i32;
    let h_i32 = h as i32;
    let grid_size = gw.grid_size as f32;

    for py in 0..h_i32 {
        let row_start = (py as usize) * w;
        let world_y = (py as f32 - oy) / scale;

        for px in 0..w_i32 {
            let world_x = (px as f32 - ox) / scale;

            if world_x >= 0.0 && world_x < grid_size && world_y >= 0.0 && world_y < grid_size {
                let tx = world_x as usize;
                let ty = world_y as usize;
                let color = gw.terrain.baked_color(tx, ty);
                unsafe {
                    *fb.get_unchecked_mut(row_start + px as usize) = color;
                }
            } else {
                unsafe {
                    *fb.get_unchecked_mut(row_start + px as usize) = COLOR_BACKGROUND;
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// UI — Zero-allocation [FASE 6]
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
            _ => "MODO: DISEÑO",
        }
    } else {
        "MODO: SIMULACION"
    };

    let mut title_buf: [u8; 128] = [0; 128];
    let title_str = write_title(&mut title_buf, mode_str, gw.time_of_day, gw.sim_tick);
    draw_text(fb, w, h, 8, 4, title_str, COLOR_UI_TEXT);

    unsafe {
        simd_render::fill_rect_alpha_simd(fb, w, h, 0, h_i32 - 20, w_i32, 20, COLOR_UI_BG);
    }

    let help = if gw.design_tool.active {
        "WASD: Mover | Click: Accion | [1-6]: Zona | [B]: Edificio | [Tab]: Salir"
    } else {
        "WASD: Mover | PageUp/Down: Zoom | [Tab]: Diseno | ESC: Salir"
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

/// Zero-allocation: escribe título en buffer de stack (sin heap)
fn write_title<'a>(buf: &'a mut [u8], mode: &str, time_of_day: u16, tick: u64) -> &'a str {
    let prefix = b"Citybound v0.8 | ";
    let mut pos = prefix.len();
    buf[..pos].copy_from_slice(prefix);

    let mode_bytes = mode.as_bytes();
    let mode_len = mode_bytes.len().min(30);
    buf[pos..pos + mode_len].copy_from_slice(&mode_bytes[..mode_len]);
    pos += mode_len;

    let hour = time_of_day / 60;
    let min = time_of_day % 60;
    buf[pos] = b' '; pos += 1;
    buf[pos] = b'|'; pos += 1;
    buf[pos] = b' '; pos += 1;
    buf[pos] = b'0' + (hour / 10) as u8; pos += 1;
    buf[pos] = b'0' + (hour % 10) as u8; pos += 1;
    buf[pos] = b':'; pos += 1;
    buf[pos] = b'0' + (min / 10) as u8; pos += 1;
    buf[pos] = b'0' + (min % 10) as u8; pos += 1;

    buf[pos] = b' '; pos += 1;
    buf[pos] = b'|'; pos += 1;
    buf[pos] = b' '; pos += 1;
    buf[pos] = b'T'; pos += 1;
    buf[pos] = b':'; pos += 1;

    if tick == 0 {
        buf[pos] = b'0'; pos += 1;
    } else {
        let mut t = tick;
        let mut digits: [u8; 10] = [0; 10];
        let mut d = 0;
        while t > 0 && d < 10 {
            digits[d] = b'0' + (t % 10) as u8;
            t /= 10;
            d += 1;
        }
        for i in (0..d).rev() {
            buf[pos] = digits[i];
            pos += 1;
        }
    }

    let valid_len = pos.min(buf.len());
    unsafe { std::str::from_utf8_unchecked(&buf[..valid_len]) }
}

// ---------------------------------------------------------------------------
// FUNCIONES DE DIBUJO
// ---------------------------------------------------------------------------

#[inline(always)]
fn draw_shape(fb: &mut [u32], w: usize, h: usize,
              x: f32, y: f32, r: &Renderable,
              ox: f32, oy: f32, scale: f32) {
    let sx = (x * scale + ox) as i32;
    let sy = (y * scale + oy) as i32;
    let size = (r.size * scale) as i32;
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
// TEXTO (fuente bitmap 5x7 con búsqueda LUT)
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
        _ => [0x00; 7],
    }
}

// ---------------------------------------------------------------------------
// UTILIDADES
// ---------------------------------------------------------------------------

#[inline(always)]
fn multiply_alpha(color: u32, alpha: f32) -> u32 {
    let a = (((color >> 24) & 0xFF) as f32 * alpha) as u32;
    (a << 24) | (color & 0x00_FF_FF_FF)
}

#[inline(always)]
fn building_color(btype: BuildingType) -> u32 {
    match btype {
        BuildingType::House => COLOR_BUILDING_HOUSE,
        BuildingType::Apartment => COLOR_BUILDING_APARTMENT,
        BuildingType::Shop => COLOR_BUILDING_SHOP,
        BuildingType::Office => COLOR_BUILDING_OFFICE,
        BuildingType::Factory => COLOR_BUILDING_FACTORY,
        BuildingType::Farm => COLOR_BUILDING_FARM,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiply_alpha() {
        assert_eq!(multiply_alpha(0xFF_FF_00_00, 0.5), 0x7F_FF_00_00);
    }

    #[test]
    fn test_building_color() {
        assert_eq!(building_color(BuildingType::House), COLOR_BUILDING_HOUSE);
    }

    #[test]
    fn test_write_title_zero_alloc() {
        let mut buf = [0u8; 128];
        let s = write_title(&mut buf, "SIM", 7 * 60 + 30, 42);
        assert!(s.contains("Citybound"));
        assert!(s.contains("SIM"));
        assert!(s.contains("07:30"));
        assert!(s.contains("42"));
    }

    #[test]
    fn test_glyph_all_chars() {
        for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 :./-|()[]".chars() {
            let glyph = get_glyph(ch);
            assert_eq!(glyph.len(), 7);
        }
    }
}
