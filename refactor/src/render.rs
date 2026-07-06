// Módulo de Renderizado Software v0.18.0 — Refactor Visual Completo
//
// REFACTOR VISUAL v0.18.0:
// - Terreno: colores suaves horneados (Perlin noise) sin ruido de tiles
// - Edificios: formas arquitectónicas reconocibles (casas con tejado, tiendas,
//   fábricas con chimeneas, etc.) en vez de círculos/cuadrados
// - Zonas: bordes sutiles punteados, sin rellenos saturados
// - Carriles: líneas finas grises, sin blanco chillón
// - Paleta de colores unificada, tonos tierra, cero magenta
//
// TÉCNICAS:
// [TC#3]  Baking de iluminación (terrain baked_colors)
// [TC#5]  LUTs trigonométricas
// [TC#10] Pre-multiplicación cámara
// [TC#17] Culling viewport
// [TC#21] Distancias²
// [TC#23] Pre-orden Z-Index (capas vía RenderCache)
use crate::ecs::{GameWorld, Camera, ConstructionState, TrafficCar};
use crate::texture_atlas::TextureAtlas;
use crate::simd_render;
// NUEVA PALETA DE COLORES (ARGB) — tonos tierra, muted, cero magenta
// ---------------------------------------------------------------------------

// Terreno
pub const COLOR_GRASS:       u32 = 0xFF_4A_7C_3F;  // verde oliva suave
pub const COLOR_DIRT:        u32 = 0xFF_9B_8C_70;  // marrón arena
pub const COLOR_ROAD:        u32 = 0xFF_6B_6B_6B;  // gris medio
pub const COLOR_SIDEWALK:    u32 = 0xFF_9E_9E_9E;  // gris claro
pub const COLOR_WATER:       u32 = 0xFF_2A_5A_8C;  // azul profundo
pub const COLOR_SAND:        u32 = 0xFF_D4_C5_9A;  // beige arena
pub const COLOR_ROCK:        u32 = 0xFF_7A_7A_7A;  // gris roca
pub const COLOR_SNOW:        u32 = 0xFF_E8_E8_F0;  // blanco nieve
pub const COLOR_BACKGROUND:  u32 = 0xFF_1C_1C_24;  // fondo oscuro (fuera del mapa)
pub const COLOR_FOG:         u32 = 0xFF_2A_2A_35;  // niebla de guerra

// Zonas — ahora son bordes sutiles, no rellenos
pub const COLOR_ZONE_RESIDENTIAL: u32 = 0x88_7B_A0_5C;  // verde apagado
pub const COLOR_ZONE_COMMERCIAL:  u32 = 0x88_5C_8F_A0;  // azul apagado
pub const COLOR_ZONE_INDUSTRIAL:  u32 = 0x88_A0_6C_5C;  // rojo apagado
pub const COLOR_ZONE_AGRICULTURAL:u32 = 0x88_8F_A0_5C;  // amarillo apagado
pub const COLOR_ZONE_PARK:        u32 = 0x88_5C_A0_6C;  // verde menta

// Edificios — colores de fallback si no hay sprites
pub const COLOR_BUILDING_HOUSE:     u32 = 0xFF_C4_8E_6A;  // terracota suave
pub const COLOR_BUILDING_APARTMENT: u32 = 0xFF_A8_A8_B0;  // gris medio
pub const COLOR_BUILDING_SHOP:      u32 = 0xFF_5C_A0_B8;  // azul comercio
pub const COLOR_BUILDING_OFFICE:    u32 = 0xFF_8A_9B_A8;  // gris azulado
pub const COLOR_BUILDING_FACTORY:   u32 = 0xFF_8A_7A_6E;  // marrón industrial
pub const COLOR_BUILDING_FARM:      u32 = 0xFF_8C_A8_6A;  // verde rural
pub const COLOR_BUILDING_HOSPITAL:  u32 = 0xFF_E8_E8_F0;  // blanco hospital
pub const COLOR_BUILDING_SCHOOL:    u32 = 0xFF_E8_D8_8C;  // amarillo educativo
pub const COLOR_BUILDING_POLICE:    u32 = 0xFF_5C_70_C4;  // azul policial

// UI
pub const COLOR_UI_TEXT:      u32 = 0xFF_E8_E4_DC;  // crema
pub const COLOR_UI_BG:        u32 = 0xCC_18_18_20;  // fondo UI oscuro
pub const COLOR_UI_ACCENT:    u32 = 0xFF_C8_A8_5C;  // dorado
pub const COLOR_UI_POSITIVE:  u32 = 0xFF_6C_B8_6C;  // verde positivo
pub const COLOR_UI_NEGATIVE:  u32 = 0xFF_C8_5C_5C;  // rojo negativo
pub const COLOR_UI_WARNING:   u32 = 0xFF_C8_B8_5C;  // amarillo advertencia
pub const COLOR_UI_NEUTRAL:   u32 = 0xFF_88_88_88;  // gris neutral

// Tráfico
pub const COLOR_LANE_NORMAL:  u32 = 0x44_88_88_88;  // casi invisible
pub const COLOR_LANE_MEDIUM:  u32 = 0x66_A0_A0_A0;  // apenas visible
pub const COLOR_CONGESTION_LOW:  u32 = 0x88_6C_A0_5C;
pub const COLOR_CONGESTION_MED:  u32 = 0x88_C8_A8_5C;
pub const COLOR_CONGESTION_HIGH: u32 = 0x88_C8_5C_5C;

const CELL_SIZE: f32 = 4.0;

// ---------------------------------------------------------------------------
// RENDER PRINCIPAL
// ---------------------------------------------------------------------------

pub fn render_world_cached(
    game_world: &GameWorld,
    atlas: &TextureAtlas,
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

    // 1. Terreno suave (fondo)
    render_terrain_smooth(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // 2. Zonas (bordes sutiles)
    render_zones_subtle(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // 3. Red de carriles (líneas finas)
    render_lane_network_subtle(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // 4. Entidades con sprites o formas arquitectónicas
    render_from_cache_v2(atlas, &game_world.render_cache, framebuffer, width, height, offset_x, offset_y, scale);

    // 5. Indicadores de congestión
    render_congestion_indicators(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // 6. UI overlay
    render_ui(game_world, framebuffer, width, height);
}

pub fn render_world(
    game_world: &GameWorld,
    framebuffer: &mut [u32],
    width: usize,
    height: usize,
) {
    let atlas = TextureAtlas::new();
    render_world_cached(game_world, &atlas, framebuffer, width, height);
}

// ---------------------------------------------------------------------------
// [v0.18] TERRENO SUAVE — usa baked_colors horneados, sin ruido
// ---------------------------------------------------------------------------

fn render_terrain_smooth(
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
        let world_y = (py as f32 - oy) / scale;
        let row_start = (py as usize) * w;

        // Fuera del mapa → color de fondo
        if world_y < 0.0 || world_y >= grid_size {
            unsafe {
                simd_render::fill_rect_simd(fb, w, h, 0, py, w_i32, 1, COLOR_BACKGROUND);
            }
            continue;
        }

        let ty = world_y as usize;

        for px in 0..w_i32 {
            let world_x = (px as f32 - ox) / scale;

            if world_x < 0.0 || world_x >= grid_size {
                unsafe {
                    *fb.get_unchecked_mut(row_start + px as usize) = COLOR_BACKGROUND;
                }
                continue;
            }

            let tx = world_x as usize;
            // Usar el color horneado del terrain map (ya incluye iluminación)
            unsafe {
                *fb.get_unchecked_mut(row_start + px as usize) = gw.terrain.baked_color(tx, ty);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// [v0.18] ZONAS SUTILES — solo bordes punteados, sin relleno saturado
// ---------------------------------------------------------------------------

fn render_zones_subtle(
    gw: &GameWorld,
    fb: &mut [u32],
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    scale: f32,
) {
    if scale < 1.2 {
    if scale < 1.2 {
        return;
    }

    for (_entity, (pos, zone)) in gw.world.query::<(&crate::ecs::Position, &crate::ecs::ZoneComponent)>().iter() {
        if zone.density <= 0 {
            continue;
        }
        let sx = (pos.x * scale + ox) as i32;
        let sy = (pos.y * scale + oy) as i32;
        let size = (4.0 * scale) as i32;

        // Solo borde inferior (línea sutil)
        let border_color = zone_border_color(zone.zone_type);

        // Dibujar esquinas como pequeños puntos para sugerir la zona
        // sin llenar todo el rectángulo
        let dot_size = (size / 6).max(1);
        if scale > 2.0 {
            // Zoom alto: línea punteada en el perímetro
            draw_dotted_rect(fb, w, h, sx, sy, size, size, border_color, scale);
        } else if scale > 1.5 {
            // Zoom medio: solo esquinas
            unsafe {
                simd_render::fill_rect_simd(fb, w, h, sx, sy, dot_size, dot_size, border_color);
                simd_render::fill_rect_simd(fb, w, h, sx + size - dot_size, sy, dot_size, dot_size, border_color);
                simd_render::fill_rect_simd(fb, w, h, sx, sy + size - dot_size, dot_size, dot_size, border_color);
                simd_render::fill_rect_simd(fb, w, h, sx + size - dot_size, sy + size - dot_size, dot_size, dot_size, border_color);
            }
        } else {
            // Zoom bajo: solo una línea fina en el borde inferior
            let alpha_color = (border_color & 0x00_FF_FF_FF) | 0x44_00_00_00;
            unsafe {
                simd_render::fill_rect_simd(fb, w, h, sx, sy + size - 1, size, 1, alpha_color);
            }
        }
    }
}

fn zone_border_color(ztype: crate::ecs::ZoneType) -> u32 {
    match ztype {
        crate::ecs::ZoneType::Residential  => 0x88_7B_A0_5C,
        crate::ecs::ZoneType::Commercial   => 0x88_5C_8F_A0,
        crate::ecs::ZoneType::Industrial   => 0x88_A0_6C_5C,
        crate::ecs::ZoneType::Agricultural => 0x88_8F_A0_5C,
        crate::ecs::ZoneType::Road         => 0x88_88_88_88,
        crate::ecs::ZoneType::Park         => 0x88_5C_A0_6C,
    }
}

// ---------------------------------------------------------------------------
// [v0.18] CARRILES SUTILES — líneas finas grises
// ---------------------------------------------------------------------------

fn render_lane_network_subtle(
    gw: &GameWorld,
    fb: &mut [u32],
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    scale: f32,
) {
    if scale < 0.6 {
        return; // No dibujar carriles con zoom muy alejado
    }

    for lane in &gw.lane_manager.lanes {
        let sx1 = (lane.start_x * scale + ox) as i32;
        let sy1 = (lane.start_y * scale + oy) as i32;
        let sx2 = (lane.end_x * scale + ox) as i32;
        let sy2 = (lane.end_y * scale + oy) as i32;

        // Color basado en congestión, pero siempre sutil
        let lane_color = if lane.congestion > 0.7 {
            COLOR_CONGESTION_HIGH
        } else if lane.congestion > 0.3 {
            COLOR_CONGESTION_MED
        } else {
            COLOR_LANE_NORMAL
        };

        draw_line(fb, w, h, sx1, sy1, sx2, sy2, lane_color);
    }

    // Intersecciones (semáforos)
    if scale > 1.5 {
        for intersection in &gw.lane_manager.intersections {
            let ix = (intersection.x * scale + ox) as i32;
            let iy = (intersection.y * scale + oy) as i32;
            let phase_color = match intersection.phase {
                crate::traffic_lanes::TrafficLightPhase::Green  => 0xFF_5C_A0_5C,
                crate::traffic_lanes::TrafficLightPhase::Yellow => 0xFF_C8_B8_5C,
                crate::traffic_lanes::TrafficLightPhase::Red    => 0xFF_C8_5C_5C,
            };

            let r = (scale * 1.5) as i32;
            unsafe {
                simd_render::fill_rect_simd(fb, w, h, ix - r, iy - r, r * 2 + 1, r * 2 + 1, phase_color);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// [v0.18] RENDER DESDE CACHE — sprites reales o formas arquitectónicas
// ---------------------------------------------------------------------------

fn render_from_cache_v2(
    atlas: &TextureAtlas,
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

        let size_px = entry.size_x * scale;
        if sx < -size_px || sx > w_f + size_px || sy < -size_px || sy > h_f + size_px {
            continue;
        }
        let sx_i = sx as i32;
        let sy_i = sy as i32;
        let size_i = size_px as i32;

        // Si tiene sprite válido del atlas → usarlo
        if entry.sprite_index > 0 && entry.sprite_index < atlas.tiles.len() as u16 {
            atlas.blit_sprite(
                entry.sprite_index as usize,
                fb, w, h,
                sx_i, sy_i,
                scale / CELL_SIZE,
            );
        } else if entry.layer == crate::render_cache::LAYER_BUILDINGS
               || entry.layer == crate::render_cache::LAYER_CONSTRUCTION {
            // Edificio sin sprite → dibujar forma arquitectónica
            draw_building_shape(fb, w, h, sx_i, sy_i, size_i, entry.color, entry.layer);
        } else if entry.layer == crate::render_cache::LAYER_TRAFFIC {
            // Vehículo sin sprite → rectángulo pequeño redondeado
            draw_vehicle_shape(fb, w, h, sx_i, sy_i, size_i, entry.color);
        } else {
            // Otros: círculo pequeño (peatones, decoraciones)
            if size_i <= 6 {
                let px = (sx_i + size_i / 2).clamp(0, w as i32 - 1);
                let py = (sy_i + size_i / 2).clamp(0, h as i32 - 1);
                let dot_size = (size_i / 2).max(1);
                unsafe {
                    simd_render::fill_rect_simd(fb, w, h, px - dot_size, py - dot_size, dot_size * 2, dot_size * 2, entry.color);
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// [v0.18] FORMAS ARQUITECTÓNICAS — edificios reconocibles
// ---------------------------------------------------------------------------

/// Dibuja un edificio con forma arquitectónica según su color (que codifica el tipo)
fn draw_building_shape(
    fb: &mut [u32],
    fb_w: usize,
    fb_h: usize,
    cx: i32,
    cy: i32,
    size: i32,
    color: u32,
    layer: u8,
) {
    let _half = size / 2;
    let _third = size / 3;
    let _quarter = size / 4;
    // Determinar tipo de edificio por color
    let r = (color >> 16) & 0xFF;
    let g = (color >> 8) & 0xFF;
    let b = color & 0xFF;

    // Colores derivados para detalles
    let darker = darken(color, 40);
    let lighter = lighten(color, 30);
    let roof_color = darken(color, 60);
    let window_color = 0xFF_E8_E4_DC; // crema para ventanas

    // Clasificar por color
    if is_color_match(r, g, b, 0xC4, 0x8E, 0x6A, 30) {
        // CASA: rectángulo con tejado triangular
        draw_house(fb, fb_w, fb_h, cx, cy, size, color, roof_color, window_color, darker);
    } else if is_color_match(r, g, b, 0xA8, 0xA8, 0xB0, 25) {
        // APARTAMENTO: torre alta con muchas ventanas
        draw_apartment(fb, fb_w, fb_h, cx, cy, size, color, darker, window_color);
    } else if is_color_match(r, g, b, 0x5C, 0xA0, 0xB8, 30) {
        // TIENDA: rectángulo ancho con toldo
        draw_shop(fb, fb_w, fb_h, cx, cy, size, color, darker, lighter);
    } else if is_color_match(r, g, b, 0x8A, 0x9B, 0xA8, 25) {
        // OFICINA: torre con ventanas regulares
        draw_office(fb, fb_w, fb_h, cx, cy, size, color, darker, window_color);
    } else if is_color_match(r, g, b, 0x8A, 0x7A, 0x6E, 25) {
        // FÁBRICA: gran rectángulo con chimeneas
        draw_factory(fb, fb_w, fb_h, cx, cy, size, color, darker, roof_color);
    } else if is_color_match(r, g, b, 0x8C, 0xA8, 0x6A, 30) {
        // GRANJA: granero con silo
        draw_farm(fb, fb_w, fb_h, cx, cy, size, color, roof_color, darker);
    } else if is_color_match(r, g, b, 0xE8, 0xE8, 0xF0, 30) {
        // HOSPITAL: gran rectángulo blanco con cruz roja
        draw_hospital(fb, fb_w, fb_h, cx, cy, size, color, darker);
    } else if is_color_match(r, g, b, 0xE8, 0xD8, 0x8C, 30) {
        // ESCUELA: varios módulos conectados
        draw_school(fb, fb_w, fb_h, cx, cy, size, color, darker, window_color);
    } else if is_color_match(r, g, b, 0x5C, 0x70, 0xC4, 30) {
        // COMISARÍA: edificio con columnas
        draw_police(fb, fb_w, fb_h, cx, cy, size, color, darker);
    } else if layer == crate::render_cache::LAYER_CONSTRUCTION {
        // CONSTRUCCIÓN: andamio
        draw_construction(fb, fb_w, fb_h, cx, cy, size, color);
    } else {
        // GENÉRICO: rectángulo simple
        draw_generic_building(fb, fb_w, fb_h, cx, cy, size, color, darker);
    }
}

#[inline(always)]
fn is_color_match(r: u32, g: u32, b: u32, tr: u32, tg: u32, tb: u32, tolerance: u32) -> bool {
    (r as i32 - tr as i32).abs() < tolerance as i32
        && (g as i32 - tg as i32).abs() < tolerance as i32
        && (b as i32 - tb as i32).abs() < tolerance as i32
}

#[inline(always)]
fn darken(color: u32, amount: u32) -> u32 {
    let r = ((color >> 16) & 0xFF).saturating_sub(amount);
    let g = ((color >> 8) & 0xFF).saturating_sub(amount);
    let b = (color & 0xFF).saturating_sub(amount);
    0xFF_00_00_00 | (r << 16) | (g << 8) | b
}

#[inline(always)]
fn lighten(color: u32, amount: u32) -> u32 {
    let r = (((color >> 16) & 0xFF) + amount).min(255);
    let g = (((color >> 8) & 0xFF) + amount).min(255);
    let b = ((color & 0xFF) + amount).min(255);
    0xFF_00_00_00 | (r << 16) | (g << 8) | b
}

// ---- FORMAS DE EDIFICIOS ----

fn draw_house(fb: &mut [u32], fb_w: usize, fb_h: usize,
              cx: i32, cy: i32, size: i32,
              color: u32, roof: u32, window: u32, dark: u32) {
    let hw = size / 2;
    let body_h = size * 2 / 3;
    let roof_h = size - body_h;

    // Cuerpo
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw, cy - hw + roof_h, size, body_h, color);
    // Borde del cuerpo
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - hw + roof_h, size, body_h, dark);

    // Tejado triangular
    let apex_y = cy - hw;
    let _base_y = cy - hw + roof_h;
    for row in 0..roof_h {
        let y = apex_y + row;
        let row_width = (row * size / roof_h.max(1)) as i32;
        let x0 = cx - row_width / 2;
        if row_width > 0 {
            fill_rect_clipped(fb, fb_w, fb_h, x0, y, row_width, 1, roof);
        }
    }

    // Ventana
    let win_s = (size / 5).max(2);
    let win_x = cx - win_s;
    let win_y = cy - hw + roof_h + body_h / 3;
    fill_rect_clipped(fb, fb_w, fb_h, win_x, win_y, win_s * 2, win_s * 2, window);
    // Cruz de la ventana
    fill_rect_clipped(fb, fb_w, fb_h, win_x + win_s - 1, win_y, 2, win_s * 2, dark);
    fill_rect_clipped(fb, fb_w, fb_h, win_x, win_y + win_s - 1, win_s * 2, 2, dark);

    // Puerta
    let door_w = (size / 5).max(2);
    let door_h = body_h / 2;
    fill_rect_clipped(fb, fb_w, fb_h, cx - door_w / 2, cy - hw + roof_h + body_h - door_h, door_w, door_h, dark);
}

fn draw_apartment(fb: &mut [u32], fb_w: usize, fb_h: usize,
                  cx: i32, cy: i32, size: i32,
                  color: u32, dark: u32, window: u32) {
    let hw = size / 2;
    let hh = size;

    // Cuerpo principal
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw, cy - hh, size, size * 2, color);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - hh, size, size * 2, dark);

    // Ventanas en grilla
    let win_s = (size / 5).max(2);
    let gap = size / 3;
    for wy in 0..4 {
        for wx in 0..2 {
            let wx_pos = cx - hw + size / 4 + wx * gap;
            let wy_pos = cy - hh + size / 4 + wy * gap;
            fill_rect_clipped(fb, fb_w, fb_h, wx_pos, wy_pos, win_s, win_s, window);
            fill_rect_clipped(fb, fb_w, fb_h, wx_pos + win_s / 2, wy_pos, 1, win_s, dark);
            fill_rect_clipped(fb, fb_w, fb_h, wx_pos, wy_pos + win_s / 2, win_s, 1, dark);
        }
    }

    // Entrada
    let door_w = (size / 3).max(3);
    fill_rect_clipped(fb, fb_w, fb_h, cx - door_w / 2, cy + hh - size / 4, door_w, size / 4, dark);
}

fn draw_shop(fb: &mut [u32], fb_w: usize, fb_h: usize,
             cx: i32, cy: i32, size: i32,
             color: u32, dark: u32, light: u32) {
    let hw = size / 2;
    let hh = size / 2;

    // Cuerpo
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw, cy - hh, size, size, color);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - hh, size, size, dark);

    // Toldo (rayas horizontales)
    let awning_h = size / 4;
    for row in 0..awning_h {
        let stripe_color = if row % 2 == 0 { light } else { dark };
        fill_rect_clipped(fb, fb_w, fb_h, cx - hw + 1, cy - hh + row, size - 2, 1, stripe_color);
    }

    // Escaparate
    let display_h = size / 3;
    let display_w = size / 2;
    fill_rect_clipped(fb, fb_w, fb_h, cx - display_w / 2, cy + hh - display_h - 2, display_w, display_h, 0xFF_D8_E8_F0);

    // Puerta
    let door_w = (size / 4).max(3);
    fill_rect_clipped(fb, fb_w, fb_h, cx + display_w / 2 - door_w, cy + hh - display_h, door_w, display_h + 2, dark);
}

fn draw_office(fb: &mut [u32], fb_w: usize, fb_h: usize,
               cx: i32, cy: i32, size: i32,
               color: u32, dark: u32, window: u32) {
    let hw = size / 2;
    let hh = size;

    // Torre
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw, cy - hh, size, size * 2, color);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - hh, size, size * 2, dark);

    // Ventanas tipo oficina (bandas horizontales)
    let band_h = (size / 5).max(2);
    for row in 0..4 {
        let y = cy - hh + size / 3 + row * (size / 3);
        fill_rect_clipped(fb, fb_w, fb_h, cx - hw + 2, y, size - 4, band_h, window);
        fill_rect_clipped(fb, fb_w, fb_h, cx - hw + 2, y + band_h, size - 4, 1, dark);
    }

    // Entrada
    fill_rect_clipped(fb, fb_w, fb_h, cx - size / 4, cy + hh - size / 4, size / 2, size / 4, dark);
}

fn draw_factory(fb: &mut [u32], fb_w: usize, fb_h: usize,
                cx: i32, cy: i32, size: i32,
                color: u32, dark: u32, roof: u32) {
    let hw = size;
    let hh = size / 2;

    // Nave principal
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw, cy - hh, size * 2, size, color);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - hh, size * 2, size, dark);

    // Techo dentado (sawtooth)
    let saw_w = size / 3;
    for i in 0..3 {
        let sx = cx - hw + i * saw_w;
        for row in 0..saw_w / 2 {
            fill_rect_clipped(fb, fb_w, fb_h, sx + row, cy - hh - row - 1, 1, row + 1, roof);
        }
    }

    // Chimeneas
    let stack_w = (size / 6).max(2);
    let stack_h = size / 2;
    fill_rect_clipped(fb, fb_w, fb_h, cx + hw - stack_w * 3, cy - hh - stack_h, stack_w, stack_h, dark);
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw + stack_w, cy - hh - stack_h * 2 / 3, stack_w, stack_h * 2 / 3, dark);

    // Puerta industrial
    let door_w = size / 2;
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw + size / 4, cy + hh - size / 3, door_w, size / 3, dark);
}

fn draw_farm(fb: &mut [u32], fb_w: usize, fb_h: usize,
             cx: i32, cy: i32, size: i32,
             color: u32, roof: u32, dark: u32) {
    let hw = size / 2;
    let hh = size / 2;

    // Granero (cuerpo principal)
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw, cy - hh, size, size, color);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - hh, size, size, dark);

    // Tejado del granero
    let roof_h = size / 3;
    for row in 0..roof_h {
        let row_w = (row * size / roof_h.max(1)) as i32;
        let x0 = cx - row_w / 2;
        fill_rect_clipped(fb, fb_w, fb_h, x0, cy - hh - row - 1, row_w, 1, roof);
    }

    // Silo
    let silo_w = size / 4;
    let silo_h = size * 2 / 3;
    fill_rect_clipped(fb, fb_w, fb_h, cx + hw - silo_w, cy - hh - silo_h / 2, silo_w, silo_h, dark);
    // Techo del silo (domo)
    for row in 0..silo_w / 2 {
        fill_rect_clipped(fb, fb_w, fb_h, cx + hw - silo_w + row, cy - hh - silo_h / 2 - row - 1, silo_w - row * 2, 1, roof);
    }

    // Puerta del granero
    let door_w = size / 3;
    fill_rect_clipped(fb, fb_w, fb_h, cx - door_w / 2, cy + hh - size / 3, door_w, size / 3, dark);
}

fn draw_hospital(fb: &mut [u32], fb_w: usize, fb_h: usize,
                 cx: i32, cy: i32, size: i32,
                 color: u32, dark: u32) {
    let hw = size;
    let hh = size / 2;

    // Edificio principal
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw, cy - hh, size * 2, size, color);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - hh, size * 2, size, dark);

    // Ala izquierda
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw - size / 3, cy - hh + size / 4, size / 3, size * 3 / 4, color);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw - size / 3, cy - hh + size / 4, size / 3, size * 3 / 4, dark);

    // Cruz roja (símbolo hospital)
    let cross_size = size / 3;
    let cross_cx = cx;
    let cross_cy = cy - hh / 2;
    let cross_hw = cross_size / 4;
    fill_rect_clipped(fb, fb_w, fb_h, cross_cx - cross_hw, cross_cy - cross_size / 2, cross_hw * 2, cross_size, 0xFF_C8_5C_5C);
    fill_rect_clipped(fb, fb_w, fb_h, cross_cx - cross_size / 2, cross_cy - cross_hw, cross_size, cross_hw * 2, 0xFF_C8_5C_5C);

    // Entrada
    fill_rect_clipped(fb, fb_w, fb_h, cx - size / 4, cy + hh - size / 4, size / 2, size / 4, dark);
}

fn draw_school(fb: &mut [u32], fb_w: usize, fb_h: usize,
               cx: i32, cy: i32, size: i32,
               color: u32, dark: u32, window: u32) {
    let hw = size;
    let hh = size / 2;

    // Edificio principal
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw, cy - hh, size * 2, size, color);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - hh, size * 2, size, dark);

    // Ventanas
    let win_s = (size / 5).max(2);
    for wx in 0..4 {
        let wx_pos = cx - hw + size / 3 + wx * size / 2;
        fill_rect_clipped(fb, fb_w, fb_h, wx_pos, cy - hh + size / 4, win_s, win_s, window);
    }

    // Techo plano con borde
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw - 1, cy - hh - 1, size * 2 + 2, 2, dark);

    // Patio/bandera
    let pole_h = size * 2 / 3;
    fill_rect_clipped(fb, fb_w, fb_h, cx + hw - size / 4, cy - hh - pole_h, 1, pole_h, dark);
    fill_rect_clipped(fb, fb_w, fb_h, cx + hw - size / 4, cy - hh - pole_h, size / 4, size / 5, 0xFF_C8_A8_5C);
}

fn draw_police(fb: &mut [u32], fb_w: usize, fb_h: usize,
               cx: i32, cy: i32, size: i32,
               color: u32, dark: u32) {
    let hw = size / 2;
    let hh = size / 2;

    // Edificio principal
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw, cy - hh, size, size, color);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - hh, size, size, dark);

    // Columnas frontales
    let col_w = (size / 8).max(1);
    for i in 0..3 {
        let col_x = cx - hw + size / 4 + i * size / 4;
        fill_rect_clipped(fb, fb_w, fb_h, col_x, cy - hh, col_w, size, dark);
    }

    // Escudo/placa
    let badge_s = size / 3;
    fill_rect_clipped(fb, fb_w, fb_h, cx - badge_s / 2, cy - hh + size / 5, badge_s, badge_s, 0xFF_C8_A8_5C);
    // Estrella simple
    fill_rect_clipped(fb, fb_w, fb_h, cx - 1, cy - hh + size / 5 + badge_s / 4, 3, badge_s / 2, dark);
    fill_rect_clipped(fb, fb_w, fb_h, cx - badge_s / 4, cy - hh + size / 5 + badge_s / 3, badge_s / 2, 3, dark);
}

fn draw_generic_building(fb: &mut [u32], fb_w: usize, fb_h: usize,
                         cx: i32, cy: i32, size: i32,
                         color: u32, dark: u32) {
    let hw = size / 2;
    let hh = size / 2;
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw, cy - hh, size, size, color);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - hh, size, size, dark);
    // Línea de techo
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw, cy - hh, size, 1, dark);
}

fn draw_construction(fb: &mut [u32], fb_w: usize, fb_h: usize,
                     cx: i32, cy: i32, size: i32, color: u32) {
    let hw = size / 2;
    let hh = size / 2;

    // Estructura de andamio (líneas cruzadas)
    let scaffold_color = 0xFF_A0_8C_6C;

    // Postes verticales
    for i in 0..3 {
        let x = cx - hw + i * hw;
        fill_rect_clipped(fb, fb_w, fb_h, x, cy - hh, 1, size, scaffold_color);
    }

    // Travesaños horizontales
    for i in 0..3 {
        let y = cy - hh + i * hh;
        fill_rect_clipped(fb, fb_w, fb_h, cx - hw, y, size, 1, scaffold_color);
    }

    // Indicador de construcción (triángulo amarillo)
    let warn_s = size / 3;
    for row in 0..warn_s {
        let row_w = (row * 2 + 1).min(warn_s * 2);
        fill_rect_clipped(fb, fb_w, fb_h, cx - row_w / 2, cy - row, row_w, 1, color);
    }
}

fn draw_vehicle_shape(fb: &mut [u32], fb_w: usize, fb_h: usize,
                      cx: i32, cy: i32, size: i32, color: u32) {
    // Coche: rectángulo redondeado (aproximado)
    let hw = size / 2;
    let hh = size / 3;
    let dark = darken(color, 40);

    // Carrocería
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw, cy - hh, size, size * 2 / 3, color);
    // Cabina
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw / 3, cy - hh - size / 6, size / 2, size / 3, dark);
    // Ruedas
    let wheel_r = (size / 5).max(1);
    fill_rect_clipped(fb, fb_w, fb_h, cx - hw + wheel_r, cy + hh - wheel_r, wheel_r * 2, wheel_r, 0xFF_3A_3A_3A);
    fill_rect_clipped(fb, fb_w, fb_h, cx + hw - wheel_r * 3, cy + hh - wheel_r, wheel_r * 2, wheel_r, 0xFF_3A_3A_3A);
}

// ---------------------------------------------------------------------------
// CONGESTIÓN (sutil)
// ---------------------------------------------------------------------------

fn render_congestion_indicators(
    gw: &GameWorld,
    fb: &mut [u32],
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    scale: f32,
) {
    if scale <= 1.5 { return; }
    for lane in &gw.lane_manager.lanes {
        if lane.vehicle_count > 2 {
            let (mx, my) = lane.position_at(0.5);
            let sx = (mx * scale + ox) as i32;
            let sy = (my * scale + oy) as i32;
            let cong_color = if lane.congestion > 0.7 {
                COLOR_CONGESTION_HIGH
            } else if lane.congestion > 0.3 {
                COLOR_CONGESTION_MED
            } else {
                COLOR_CONGESTION_LOW
            };
            let r = (scale * 0.8) as i32;
            unsafe {
                simd_render::fill_rect_simd(fb, w, h, sx - r, sy - r, r * 2 + 1, r * 2 + 1, cong_color);
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
// RECTÁNGULO PUNTEADO (para zonas sutiles)
// ---------------------------------------------------------------------------

fn draw_dotted_rect(fb: &mut [u32], fb_w: usize, fb_h: usize,
                    x: i32, y: i32, rw: i32, rh: i32, color: u32, scale: f32) {
    let dot_spacing = (scale * 0.8) as i32;
    let dot_size = (scale * 0.3).max(1.0) as i32;

    // Borde superior
    let mut px = x;
    while px <= x + rw {
        unsafe {
            simd_render::fill_rect_simd(fb, fb_w, fb_h, px, y, dot_size, dot_size, color);
        }
        px += dot_spacing;
    }
    // Borde inferior
    let mut px = x;
    while px <= x + rw {
        unsafe {
            simd_render::fill_rect_simd(fb, fb_w, fb_h, px, y + rh - dot_size, dot_size, dot_size, color);
        }
        px += dot_spacing;
    }
    // Borde izquierdo
    let mut py = y + dot_spacing;
    while py < y + rh {
        unsafe {
            simd_render::fill_rect_simd(fb, fb_w, fb_h, x, py, dot_size, dot_size, color);
        }
        py += dot_spacing;
    }
    // Borde derecho
    let mut py = y + dot_spacing;
    while py < y + rh {
        unsafe {
            simd_render::fill_rect_simd(fb, fb_w, fb_h, x + rw - dot_size, py, dot_size, dot_size, color);
        }
        py += dot_spacing;
    }
}

// ---------------------------------------------------------------------------
// UTILIDADES DE DIBUJO
// ---------------------------------------------------------------------------

#[inline(always)]
fn fill_rect_clipped(fb: &mut [u32], fb_w: usize, fb_h: usize,
                     x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    if rw <= 0 || rh <= 0 { return; }
    unsafe {
        simd_render::fill_rect_simd(fb, fb_w, fb_h, x, y, rw, rh, color);
    }
}

fn draw_rect_outline(fb: &mut [u32], fb_w: usize, fb_h: usize,
                     x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    if rw <= 0 || rh <= 0 { return; }
    unsafe {
        simd_render::fill_rect_simd(fb, fb_w, fb_h, x, y, rw, 1, color);
        simd_render::fill_rect_simd(fb, fb_w, fb_h, x, y + rh - 1, rw, 1, color);
        simd_render::fill_rect_simd(fb, fb_w, fb_h, x, y, 1, rh, color);
        simd_render::fill_rect_simd(fb, fb_w, fb_h, x + rw - 1, y, 1, rh, color);
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

#[allow(dead_code)]
fn fill_circle(fb: &mut [u32], fb_w: usize, fb_h: usize,
               cx: i32, cy: i32, radius: i32, color: u32) {
    let r2 = radius * radius;
    let x0 = (cx - radius).max(0);
    let y0 = (cy - radius).max(0);
    let x1 = (cx + radius).min(fb_w as i32);
    let y1 = (cy + radius).min(fb_h as i32);
    for py in y0..y1 {
        let dy = py - cy;
        let row_start = (py as usize) * fb_w;
        for px in x0..x1 {
            let dx = px - cx;
            if dx * dx + dy * dy <= r2 {
                unsafe {
                    *fb.get_unchecked_mut(row_start + px as usize) = color;
                }
            }
        }
    }
}

#[allow(dead_code)]
fn fill_triangle(fb: &mut [u32], fb_w: usize, fb_h: usize,
                 cx: i32, cy: i32, size: i32, color: u32) {
    let h = size;
    let hw = size / 2;
    for row in 0..h {
        let row_w = (row * hw * 2 / h.max(1)) as i32;
        let y = cy - h / 2 + row;
        let x0 = (cx - row_w / 2).max(0);
        let x1 = (cx + row_w / 2).min(fb_w as i32);
        if x0 < x1 {
            unsafe {
                simd_render::fill_rect_simd(fb, fb_w, fb_h, x0, y, x1 - x0, 1, color);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// UI
// ---------------------------------------------------------------------------

fn render_ui(gw: &GameWorld, fb: &mut [u32], w: usize, h: usize) {
    let w_i32 = w as i32;
    let h_i32 = h as i32;

    // Barra superior
    unsafe {
        simd_render::fill_rect_simd(fb, w, h, 0, 0, w_i32, 24, COLOR_UI_BG);
    }

    let mode_str = if gw.design_tool.active {
        match gw.design_tool.mode {
            crate::interactive::DesignMode::PaintZone => "PINTAR ZONAS",
            crate::interactive::DesignMode::PlaceBuilding => "CONSTRUIR",
            crate::interactive::DesignMode::Inspect => "INSPECCIONAR",
            _ => "DISENO",
        }
    } else {
        "SIMULACION"
    };

    let title = format!("Citybound v0.18 | {} | {:02}:{:02} | T:{}",
        mode_str, gw.time_of_day / 60, gw.time_of_day % 60, gw.sim_tick);
    draw_text(fb, w, h, 8, 4, &title, COLOR_UI_TEXT);

    // Barra inferior
    unsafe {
        simd_render::fill_rect_simd(fb, w, h, 0, h_i32 - 20, w_i32, 20, COLOR_UI_BG);
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
        simd_render::fill_rect_simd(fb, w, h, mm_x, mm_y, 64, 64, COLOR_UI_BG);
    }
    draw_rect(fb, w, h, mm_x - 1, mm_y - 1, 66, 66, 0xFF_88_88_88);
}

// ---------------------------------------------------------------------------
// STATS PANEL
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
    let panel_h = 210;

    unsafe {
        simd_render::fill_rect_simd(fb, w, h, panel_x, panel_y, panel_w, panel_h, 0xCC_15_15_20);
    }

    let mut y = panel_y + 5;
    let x = panel_x + 5;

    draw_text(fb, w, h, x, y, "ESTADISTICAS", COLOR_UI_ACCENT);
    y += 12;

    let fps_str = format!("FPS: {}", fps);
    draw_text(fb, w, h, x, y, &fps_str, COLOR_UI_TEXT);
    y += 10;

    let pop = gw.world.query::<&ConstructionState>().iter().count();
    draw_text(fb, w, h, x, y, &format!("Pob: {}", pop), COLOR_UI_TEXT);
    y += 10;

    let cars = gw.world.query::<&TrafficCar>().iter().count();
    draw_text(fb, w, h, x, y, &format!("Coches: {}", cars), COLOR_UI_TEXT);
    y += 10;

    draw_text(fb, w, h, x, y, &format!("Tesoro: ${:.0}", gw.finance.treasury), COLOR_UI_POSITIVE);
    y += 10;

    let approval = gw.politics.global_approval;
    let approval_color = if approval > 0.5 { COLOR_UI_POSITIVE }
        else if approval > 0.3 { COLOR_UI_WARNING }
        else { COLOR_UI_NEGATIVE };
    draw_text(fb, w, h, x, y, &format!("Aprob: {:.0}%", approval * 100.0), approval_color);
    y += 10;

    draw_text(fb, w, h, x, y, &format!("Hora: {:02}:{:02}", gw.time_of_day / 60, gw.time_of_day % 60), COLOR_UI_TEXT);
    y += 10;

    draw_text(fb, w, h, x, y, &format!("Tick: {}", gw.sim_tick), COLOR_UI_NEUTRAL);

    y += 8;
    draw_text(fb, w, h, x, y, "------------", 0xFF_44_44_44);
    y += 10;

    let congestion = if !gw.lane_manager.lanes.is_empty() {
        gw.lane_manager.lanes.iter().map(|l| l.congestion).sum::<f32>() / gw.lane_manager.lanes.len() as f32
    } else { 0.0 };
    let cong_color = if congestion > 0.5 { COLOR_UI_NEGATIVE } else { COLOR_UI_POSITIVE };
    draw_text(fb, w, h, x, y, &format!("Traf: {:.0}%", congestion * 100.0), cong_color);
    y += 10;

    let lv = gw.land_value_map.get(64, 64);
    draw_text(fb, w, h, x, y, &format!("Suelo: ${:.0}", lv), 0xFF_CC_AA_88);
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
        if cx + 6 >= fb_w as i32 { break; }
    }
}

fn draw_char(fb: &mut [u32], fb_w: usize, fb_h: usize,
             x: i32, y: i32, ch: char, color: u32) {
    let bitmap = match ch {
        'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001],
        'B' => [0b11110, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
        'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10001, 0b01110],
        'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
        'E' => [0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
        'F' => [0b11111, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
        'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b01110],
        'H' => [0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
        'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        'J' => [0b00111, 0b00001, 0b00001, 0b00001, 0b10001, 0b01110],
        'K' => [0b10001, 0b10010, 0b11100, 0b10010, 0b10001, 0b10001],
        'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
        'M' => [0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001],
        'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001],
        'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'P' => [0b11110, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
        'Q' => [0b01110, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
        'R' => [0b11110, 0b10001, 0b11110, 0b10010, 0b10001, 0b10001],
        'S' => [0b01111, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
        'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
        'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
        'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
        'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b11011, 0b10001],
        'X' => [0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
        'Y' => [0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
        'Z' => [0b11111, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00010, 0b00100, 0b01000, 0b11111],
        '3' => [0b01110, 0b10001, 0b00110, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b10001, 0b01110],
        '6' => [0b01110, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000],
        '8' => [0b01110, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b01110],
        ' ' => [0, 0, 0, 0, 0, 0],
        '.' => [0, 0, 0, 0, 0, 0b00100],
        ',' => [0, 0, 0, 0, 0b00100, 0b01000],
        ':' => [0, 0b00100, 0, 0b00100, 0, 0],
        '-' => [0, 0, 0b01110, 0, 0, 0],
        '/' => [0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0],
        '\\'=> [0b10000, 0b01000, 0b00100, 0b00010, 0b00001, 0],
        '[' => [0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110],
        ']' => [0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110],
        '(' => [0b00110, 0b01000, 0b10000, 0b10000, 0b01000, 0b00110],
        ')' => [0b01100, 0b00010, 0b00001, 0b00001, 0b00010, 0b01100],
        '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00000, 0b00100],
        '?' => [0b01110, 0b10001, 0b00010, 0b00100, 0b00000, 0b00100],
        '$' => [0b00100, 0b01111, 0b10100, 0b01110, 0b00101, 0b11110],
        '%' => [0b11000, 0b11001, 0b00010, 0b00100, 0b01011, 0b00011],
        '+' => [0, 0b00100, 0b01110, 0b00100, 0, 0],
        '=' => [0, 0b01110, 0, 0b01110, 0, 0],
        '<' => [0b00010, 0b00100, 0b01000, 0b00100, 0b00010, 0],
        '>' => [0b01000, 0b00100, 0b00010, 0b00100, 0b01000, 0],
        '_' => [0, 0, 0, 0, 0, 0b11111],
        _ => [0, 0, 0, 0, 0, 0],
    };

    for (row, bits) in bitmap.iter().enumerate() {
        let mut mask = 0b10000u32;
        for col in 0..5 {
            if bits & mask != 0 {
                let px = x + col;
                let py = y + row as i32;
                if px >= 0 && px < fb_w as i32 && py >= 0 && py < fb_h as i32 {
                    unsafe {
                        *fb.get_unchecked_mut((py as usize) * fb_w + px as usize) = color;
                    }
                }
            }
            mask >>= 1;
        }
    }
}
