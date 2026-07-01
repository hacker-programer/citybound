// Módulo de Renderizado Software
//
// Renderiza el estado del juego a un framebuffer ARGB (u32).
// Usa rasterización software pura - sin GPU, sin shaders, sin OpenGL.
// Ideal para hardware legacy (Pentium 4GB) que puede tener
// soporte limitado de GPU.
//
// TÉCNICAS APLICADAS:
// [TC#3]  Baking de iluminación: colores precalculados en paleta
// [TC#5]  LUTs trigonométricas para círculos y curvas
// [TC#10] Pre-multiplicación de matrices de transformación (cámara)
// [TC#17] Culling estático: solo renderizar entidades en viewport
// [TC#23] Pre-ordenamiento por Z-Index (capas de renderizado)
// [TA#17] Acceso unchecked en bucles validados

// Allow dead_code en paleta de colores: usados por sistemas futuros y tests
#![allow(dead_code)]

use crate::ecs::{GameWorld, Position, Renderable, ZoneComponent, ZoneType, Camera,
                  ConstructionState, BuildingType};
use crate::luts;

// ---------------------------------------------------------------------------
// PALETA DE COLORES (ARGB)
// Baking de iluminación: todos los colores predefinidos en tiempo de compilación
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
pub const COLOR_BUILDING_HOUSE: u32 = 0xFF_C4_7B_4A;
pub const COLOR_BUILDING_APARTMENT: u32 = 0xFF_B0_BEC5;
pub const COLOR_BUILDING_SHOP: u32 = 0xFF_26_C6_DA;
pub const COLOR_BUILDING_OFFICE: u32 = 0xFF_78_90_9C;
pub const COLOR_BUILDING_FACTORY: u32 = 0xFF_8D_6E_63;
pub const COLOR_BUILDING_FARM: u32 = 0xFF_8B_C3_4A;
pub const COLOR_UI_TEXT: u32 = 0xFF_FF_FF_FF;
pub const COLOR_UI_BG: u32 = 0xAA_00_00_00;
pub const COLOR_BACKGROUND: u32 = 0xFF_1A_1A_2E;

/// Tamaño de celda en píxeles (constante local)
const CELL_SIZE: f32 = 4.0;

// ---------------------------------------------------------------------------
// RENDER PRINCIPAL
// ---------------------------------------------------------------------------

/// Renderiza el mundo al framebuffer.
/// `framebuffer` es un slice mutable de u32 ARGB de tamaño width*height.
pub fn render_world(
    game_world: &GameWorld,
    framebuffer: &mut [u32],
    width: usize,
    height: usize,
) {
    // Obtener parámetros de cámara
    let mut cam_offset_x: f32 = 0.0;
    let mut cam_offset_y: f32 = 0.0;
    let mut cam_zoom: f32 = 1.0;

    for (_entity, (camera,)) in game_world.world.query::<(&Camera,)>().iter() {
        cam_offset_x = camera.offset_x;
        cam_offset_y = camera.offset_y;
        cam_zoom = camera.zoom;
        break;
    }

    // [TC#10]: Pre-calcular transformación de cámara
    let scale = CELL_SIZE * cam_zoom;
    let offset_x = (width as f32 / 2.0) - cam_offset_x * scale;
    let offset_y = (height as f32 / 2.0) - cam_offset_y * scale;

    // Fondo con patrón de terreno (usando paleta baked y LUTs)
    render_background(framebuffer, width, height, offset_x, offset_y, scale);

    // PASADA 1: Capa 0-1 (terreno y zonas)
    render_base_layer(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // PASADA 2: Capa 2-3 (edificios)
    render_building_layer(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // PASADA 3: Capa 4+ (tráfico)
    render_traffic_layer(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // PASADA 4: UI overlay
    render_ui(game_world, framebuffer, width, height);
}

// ---------------------------------------------------------------------------
// FONDO CON PATRÓN DE TERRENO (usa colores baked y LUTs)
// ---------------------------------------------------------------------------

fn render_background(fb: &mut [u32], w: usize, h: usize, ox: f32, oy: f32, scale: f32) {
    let w_i32 = w as i32;
    let h_i32 = h as i32;

    // [TC#5]: Usar LUTs para generar patrón de cuadrícula con ondulaciones sutiles
    for py in 0..h_i32 {
        let row_start = (py as usize) * w;
        // Variación de color por fila usando sin_fast de LUT
        let row_variation = luts::sin_fast(py as f32 * 0.02) * 0.03;

        for px in 0..w_i32 {
            let world_x = (px as f32 - ox) / scale;
            let world_y = (py as f32 - oy) / scale;

            // Patrón de ajedrez sutil con variación sinusoidal
            let checker = ((world_x.floor() as i32 + world_y.floor() as i32) & 1) as f32;
            let variation = luts::cos_fast(world_x * 0.3) * luts::sin_fast(world_y * 0.3) * 0.05;

            let brightness = 0.92_f32 + checker * 0.04 + variation + row_variation;
            let r = (26.0_f32 * brightness) as u32;
            let g = (26.0_f32 * brightness) as u32;
            let b = (46.0_f32 * brightness) as u32;

            unsafe {
                *fb.get_unchecked_mut(row_start + px as usize) =
                    0xFF_00_00_00 | (r << 16) | (g << 8) | b;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// CAPAS DE RENDERIZADO
// ---------------------------------------------------------------------------

fn render_base_layer(
    gw: &GameWorld, fb: &mut [u32], w: usize, h: usize,
    ox: f32, oy: f32, scale: f32,
) {
    // Entidades con layer 0 o 1
    for (_entity, (pos, renderable)) in gw.world
        .query::<(&Position, &Renderable)>()
        .iter()
    {
        if renderable.layer <= 1 {
            draw_shape(fb, w, h, pos.x, pos.y, renderable, ox, oy, scale);
        }
    }

    // Overlay de zonas (capa alpha)
    for (_entity, (pos, zone)) in gw.world
        .query::<(&Position, &ZoneComponent)>()
        .iter()
    {
        if zone.density > 0 {
            let zone_color = match zone.zone_type {
                ZoneType::Residential => COLOR_ZONE_RESIDENTIAL,
                ZoneType::Commercial => COLOR_ZONE_COMMERCIAL,
                ZoneType::Industrial => COLOR_ZONE_INDUSTRIAL,
                ZoneType::Agricultural => COLOR_ZONE_AGRICULTURAL,
                ZoneType::Road => COLOR_ROAD,
                ZoneType::Park => 0x44_4C_AF_50,
            };
            let sx = (pos.x * scale + ox) as i32;
            let sy = (pos.y * scale + oy) as i32;
            let size = scale as i32;
            fill_rect_alpha(fb, w, h, sx, sy, size, size, zone_color);
        }
    }
}

fn render_building_layer(
    gw: &GameWorld, fb: &mut [u32], w: usize, h: usize,
    ox: f32, oy: f32, scale: f32,
) {
    // Edificios completados (layer 2-3)
    for (_entity, (pos, renderable)) in gw.world
        .query::<(&Position, &Renderable)>()
        .iter()
    {
        if renderable.layer >= 2 && renderable.layer <= 3 {
            draw_shape(fb, w, h, pos.x, pos.y, renderable, ox, oy, scale);
        }
    }

    // Construcciones en progreso
    for (_entity, (pos, construction)) in gw.world
        .query::<(&Position, &ConstructionState)>()
        .iter()
    {
        let color = building_color(construction.building_type);
        let alpha = 0.5 + construction.progress * 0.5;
        let sx = (pos.x * scale + ox) as i32;
        let sy = (pos.y * scale + oy) as i32;
        let size = (scale * 2.0) as i32;
        fill_rect_alpha(fb, w, h, sx, sy, size, size, multiply_alpha(color, alpha));
    }
}

fn render_traffic_layer(
    gw: &GameWorld, fb: &mut [u32], w: usize, h: usize,
    ox: f32, oy: f32, scale: f32,
) {
    for (_entity, (pos, renderable)) in gw.world
        .query::<(&Position, &Renderable)>()
        .iter()
    {
        if renderable.layer >= 4 {
            draw_shape(fb, w, h, pos.x, pos.y, renderable, ox, oy, scale);
        }
    }
}

fn render_ui(gw: &GameWorld, fb: &mut [u32], w: usize, h: usize) {
    let w_i32 = w as i32;
    let h_i32 = h as i32;

    // Panel superior
    fill_rect_alpha(fb, w, h, 0, 0, w_i32, 24, COLOR_UI_BG);

    let time_str = format!("Citybound Native | Hora: {} | Tick: {}",
        crate::sim::formatted_time(gw.time_of_day), gw.sim_tick);
    draw_text(fb, w, h, 8, 4, &time_str, COLOR_UI_TEXT);

    // Panel inferior
    fill_rect_alpha(fb, w, h, 0, h_i32 - 20, w_i32, 20, COLOR_UI_BG);
    draw_text(fb, w, h, 8, h_i32 - 16,
        "WASD: Mover | PageUp/Down: Zoom | ESC: Salir", COLOR_UI_TEXT);

    // Mini-mapa (esquina inferior derecha)
    let mm_x = w_i32 - 70;
    let mm_y = h_i32 - 90;
    fill_rect_alpha(fb, w, h, mm_x, mm_y, 64, 64, COLOR_UI_BG);
    draw_rect(fb, w, h, mm_x - 1, mm_y - 1, 66, 66, 0xFF_88_88_88);
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
        1 => fill_rect_alpha(fb, w, h, sx, sy, size, size, color),
        2 => fill_triangle(fb, w, h, sx, sy, size, color),
        _ => {}
    }
}

/// Rellena un rectángulo sólido (sin alpha blending)
#[inline(always)]
fn fill_rect(fb: &mut [u32], fb_w: usize, fb_h: usize,
             x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x + rw).min(fb_w as i32);
    let y2 = (y + rh).min(fb_h as i32);

    if x1 >= x2 || y1 >= y2 {
        return;
    }

    // [TA#17]: Bucle con índices directos para evitar bounds checks
    for py in y1..y2 {
        let row_start = (py as usize) * fb_w;
        for px in x1..x2 {
            unsafe {
                *fb.get_unchecked_mut(row_start + px as usize) = color;
            }
        }
    }
}

/// Rellena un rectángulo con alpha blending
#[inline(always)]
fn fill_rect_alpha(fb: &mut [u32], fb_w: usize, fb_h: usize,
                   x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x + rw).min(fb_w as i32);
    let y2 = (y + rh).min(fb_h as i32);

    if x1 >= x2 || y1 >= y2 {
        return;
    }

    let src_a = ((color >> 24) & 0xFF) as u32;
    if src_a >= 255 {
        fill_rect(fb, fb_w, fb_h, x1, y1, x2 - x1, y2 - y1, color);
        return;
    }
    if src_a == 0 {
        return;
    }

    let inv_a = 255 - src_a;
    let src_r = (color >> 16) & 0xFF;
    let src_g = (color >> 8) & 0xFF;
    let src_b = color & 0xFF;

    for py in y1..y2 {
        let row_start = (py as usize) * fb_w;
        for px in x1..x2 {
            unsafe {
                let dst = *fb.get_unchecked(row_start + px as usize);
                let dst_a = (dst >> 24) & 0xFF;
                let dst_r = (dst >> 16) & 0xFF;
                let dst_g = (dst >> 8) & 0xFF;
                let dst_b = dst & 0xFF;

                let out_a = src_a + ((dst_a * inv_a) / 255);
                let out_r = (src_r * src_a + dst_r * inv_a) / 255;
                let out_g = (src_g * src_a + dst_g * inv_a) / 255;
                let out_b = (src_b * src_a + dst_b * inv_a) / 255;

                *fb.get_unchecked_mut(row_start + px as usize) =
                    (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b;
            }
        }
    }
}

fn draw_rect(fb: &mut [u32], fb_w: usize, fb_h: usize,
             x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    fill_rect(fb, fb_w, fb_h, x, y, rw, 1, color);
    fill_rect(fb, fb_w, fb_h, x, y + rh - 1, rw, 1, color);
    fill_rect(fb, fb_w, fb_h, x, y, 1, rh, color);
    fill_rect(fb, fb_w, fb_h, x + rw - 1, y, 1, rh, color);
}

/// Rellena un círculo usando escaneo por filas (más rápido que trigonometría en tiempo real)
fn fill_circle(fb: &mut [u32], fb_w: usize, fb_h: usize,
               cx: i32, cy: i32, radius: i32, color: u32) {
    if radius <= 0 {
        return;
    }

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
// TEXTO (fuente bitmap 5x7 embebida)
// ---------------------------------------------------------------------------

fn draw_text(fb: &mut [u32], fb_w: usize, fb_h: usize,
             x: i32, y: i32, text: &str, color: u32) {
    let mut cx = x;
    for ch in text.chars() {
        draw_char(fb, fb_w, fb_h, cx, y, ch, color);
        cx += 6;
        if cx > fb_w as i32 - 6 {
            break;
        }
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

/// Bitmap 5x7 para caracteres ASCII imprimibles
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

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiply_alpha() {
        assert_eq!(multiply_alpha(0xFF_FF_00_00, 0.5), 0x7F_FF_00_00);
        assert_eq!(multiply_alpha(0xFF_00_FF_00, 1.0), 0xFF_00_FF_00);
        assert_eq!(multiply_alpha(0xFF_00_00_FF, 0.0), 0x00_00_00_FF);
    }

    #[test]
    fn test_building_color() {
        assert_eq!(building_color(BuildingType::House), COLOR_BUILDING_HOUSE);
        assert_eq!(building_color(BuildingType::Factory), COLOR_BUILDING_FACTORY);
    }

    #[test]
    fn test_glyph_all_chars() {
        for ch in "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 :./-|()".chars() {
            let glyph = get_glyph(ch);
            assert_eq!(glyph.len(), 7, "Glyph for '{}' has wrong length", ch);
        }
    }

    #[test]
    fn test_fill_rect_bounds() {
        let mut fb = vec![0u32; 100]; // 10x10
        fill_rect(&mut fb, 10, 10, -5, -5, 3, 3, 0xFF_FF_00_00);
        fill_rect(&mut fb, 10, 10, 8, 8, 10, 10, 0xFF_FF_00_00);
        // No panic
    }

    #[test]
    fn test_fill_circle_bounds() {
        let mut fb = vec![0u32; 400]; // 20x20
        fill_circle(&mut fb, 20, 20, -100, -100, 10, 0xFF_FF_00_00);
        fill_circle(&mut fb, 20, 20, 10, 10, -5, 0xFF_FF_00_00);
        fill_circle(&mut fb, 20, 20, 10, 10, 0, 0xFF_FF_00_00);
    }

    #[test]
    fn test_fill_rect_alpha_opaque() {
        let mut fb = vec![0xFF_00_00_00u32; 100]; // 10x10
        fill_rect_alpha(&mut fb, 10, 10, 0, 0, 5, 5, 0xFF_FF_00_00);
        assert_eq!(fb[0], 0xFF_FF_00_00);
    }

    #[test]
    fn test_fill_rect_alpha_transparent() {
        let mut fb = vec![0xFF_00_00_00u32; 100];
        fill_rect_alpha(&mut fb, 10, 10, 0, 0, 5, 5, 0x00_FF_00_00);
        assert_eq!(fb[0], 0xFF_00_00_00); // Sin cambios
    }

    #[test]
    fn test_background_uses_luts() {
        let mut fb = vec![COLOR_BACKGROUND; 400]; // 20x20
        crate::luts::init_trig_luts();
        render_background(&mut fb, 20, 20, 10.0, 10.0, 1.0);
        // No debe panic y debe modificar píxeles
        let modified = fb.iter().any(|&p| p != COLOR_BACKGROUND);
        assert!(modified, "Background debe modificar el framebuffer usando LUTs");
    }
}
