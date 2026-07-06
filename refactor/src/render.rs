// Módulo de Renderizado Software v0.17.0 — Fase 9: Terreno con Texturas
//
// FASE 9:
// - Terreno renderizado con tiles reales del atlas
// - Sprites de edificios desde categorías del atlas
// - Sprites de vehículos desde atlas
// - Fallback a colores planos si no hay texturas
//
// TÉCNICAS:
// [TC#3]  Baking de iluminación (terrain baked_colors como fallback)
// [TC#5]  LUTs trigonométricas
// [TC#10] Pre-multiplicación cámara
// [TC#17] Culling viewport
// [TC#21] Distancias²
// [TC#23] Pre-orden Z-Index (capas vía RenderCache)
// [TI#28] Texturas pre-extraídas en atlas indexado

use crate::ecs::{GameWorld, Renderable, Camera, ConstructionState, BuildingType, TrafficCar};
use crate::texture_atlas::{TextureAtlas, TerrainTileType};
use crate::simd_render;

// ---------------------------------------------------------------------------
// PALETA DE COLORES (ARGB) — fallback
// ---------------------------------------------------------------------------

pub const COLOR_GRASS: u32 = 0xFF_2D_5A_27;
pub const COLOR_DIRT: u32 = 0xFF_8B_73_55;
pub const COLOR_ROAD: u32 = 0xFF_55_55_55;
pub const COLOR_SIDEWALK: u32 = 0xFF_AA_AA_AA;
pub const COLOR_WATER: u32 = 0xFF_1A_3A_6A;
pub const COLOR_ZONE_RESIDENTIAL: u32 = 0x18_7B_A0_5C;
pub const COLOR_ZONE_COMMERCIAL: u32 = 0x18_5C_8A_B8;
pub const COLOR_ZONE_INDUSTRIAL: u32 = 0x18_B0_7A_6E;
pub const COLOR_ZONE_AGRICULTURAL: u32 = 0x18_8C_A8_6A;
pub const COLOR_ZONE_ROAD: u32 = 0x18_6B_6B_6B;
pub const COLOR_ZONE_PARK: u32 = 0x18_5A_8C_4A;
pub const COLOR_BUILDING_HOUSE: u32 = 0xFF_C4_8E_6A;
pub const COLOR_BUILDING_APARTMENT: u32 = 0xFF_A8_A8_B0;
pub const COLOR_BUILDING_SHOP: u32 = 0xFF_5C_A0_B8;
pub const COLOR_BUILDING_OFFICE: u32 = 0xFF_8A_9B_A8;
pub const COLOR_BUILDING_FACTORY: u32 = 0xFF_8A_7A_6E;
pub const COLOR_BUILDING_FARM: u32 = 0xFF_8C_A8_6A;
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
// RENDER PRINCIPAL (con TextureAtlas)
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

    // Fondo con tiles de terreno del atlas
    render_terrain_tiled(game_world, atlas, framebuffer, width, height, offset_x, offset_y, scale);

    // Red de carriles
    render_lane_network(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // Entidades desde RenderCache con sprites
    render_from_cache(atlas, &game_world.render_cache, framebuffer, width, height, offset_x, offset_y, scale);

    // Indicadores de congestión
    render_congestion_indicators(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // UI overlay
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
// [FASE 9]: TERRENO CON TILES REALES
// ---------------------------------------------------------------------------

fn render_terrain_tiled(
    gw: &GameWorld,
    atlas: &TextureAtlas,
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

    // Si el atlas tiene pocos tiles de terreno, usar fallback de colores planos
    let use_tiles = atlas.categories.grass.len() >= 1
        && atlas.categories.dirt.len() >= 1
        && atlas.categories.road.len() >= 1;

    if !use_tiles {
        // Fallback: colores planos del TerrainMap
        render_background_simd(gw, fb, w, h, ox, oy, scale);
        return;
    }

    // Pre-seleccionar tiles base para cada tipo de terreno
    let grass_tile = atlas.categories.random_terrain(TerrainTileType::Grass, &mut || 0);
    let dirt_tile = atlas.categories.random_terrain(TerrainTileType::Dirt, &mut || 0);
    let road_tile = atlas.categories.random_terrain(TerrainTileType::Road, &mut || 0);
    let sand_tile = atlas.categories.random_terrain(TerrainTileType::Sand, &mut || 0);
    let water_tile = atlas.categories.random_terrain(TerrainTileType::Water, &mut || 0);

    // Variantes para evitar repetición (mosaico)
    let grass2 = if atlas.categories.grass.len() > 1 { atlas.categories.grass[1] } else { grass_tile };
    let dirt2 = if atlas.categories.dirt.len() > 1 { atlas.categories.dirt[1] } else { dirt_tile };

    for py in 0..h_i32 {
        let world_y = (py as f32 - oy) / scale;

        if world_y < 0.0 || world_y >= grid_size {
            unsafe {
                simd_render::fill_rect_simd(fb, w, h, 0, py, w_i32, 1, COLOR_BACKGROUND);
            }
            continue;
        }

        let ty = world_y as usize;
        let row_start = (py as usize) * w;

        for px in 0..w_i32 {
            let world_x = (px as f32 - ox) / scale;

            if world_x < 0.0 || world_x >= grid_size {
                unsafe {
                    *fb.get_unchecked_mut(row_start + px as usize) = COLOR_BACKGROUND;
                }
                continue;
            }

            let tx = world_x as usize;
            let terrain_type = gw.terrain.terrain_types[ty * 128 + tx];

            // Si el zoom es suficiente, usar tiles; si no, color plano
            if scale >= 0.8 {
                // Seleccionar tile según tipo de terreno y posición (para variar)
                let tile_idx = match terrain_type {
                    0 => water_tile,                                    // agua
                    1 => if (tx + ty) % 2 == 0 { sand_tile } else { sand_tile }, // arena
                    2 => if (tx / 4 + ty / 4) % 2 == 0 { grass_tile } else { grass2 }, // pasto
                    3 => if (tx / 4 + ty / 4) % 2 == 0 { grass_tile } else { dirt_tile }, // bosque
                    4 => if (tx + ty) % 2 == 0 { dirt2 } else { dirt_tile }, // roca
                    _ => grass_tile,
                };

                if tile_idx > 0 {
                    // Obtener el píxel del tile correspondiente
                    let tile = &atlas.tiles[tile_idx];
                    let tw = tile.width as usize;
                    let tx_px = (world_x * scale + ox) as usize % tw;
                    let ty_px = (world_y * scale + oy) as usize % tile.height as usize;

                    unsafe {
                        *fb.get_unchecked_mut(row_start + px as usize) =
                            tile.pixels[ty_px * tw + tx_px];
                    }
                } else {
                    unsafe {
                        *fb.get_unchecked_mut(row_start + px as usize) =
                            gw.terrain.baked_color(tx, ty);
                    }
                }
            } else {
                // Zoom alejado: colores planos (más rápido y legible)
                unsafe {
                    *fb.get_unchecked_mut(row_start + px as usize) =
                        gw.terrain.baked_color(tx, ty);
                }
            }
        }
    }
}

/// Fallback: terreno con colores planos SIMD
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
// RENDER DESDE CACHE (con sprites)
// ---------------------------------------------------------------------------

fn render_from_cache(
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

        // Viewport culling
        let size_px = entry.size_x * scale;
        if sx < -size_px || sx > w_f + size_px || sy < -size_px || sy > h_f + size_px {
            continue;
        }
        let sx_i = sx as i32;
        let sy_i = sy as i32;
        let size_i = size_px as i32;

        // Si tiene sprite, usar textura del atlas
        if entry.sprite_index > 0 {
            atlas.blit_sprite(
                entry.sprite_index as usize,
                fb, w, h,
                sx_i, sy_i,
                scale / CELL_SIZE, // Normalizar escala para sprite
            );
        } else if entry.color & 0xFF_00_00_00 == 0x44_00_00_00 {
            // Es una zona (alpha bajo) — rectángulo semi-transparente
            unsafe {
                simd_render::fill_rect_alpha_simd(fb, w, h, sx_i, sy_i, size_i, size_i, entry.color);
            }
        } else {
            // Fallback a formas geométricas (sin sprite)
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

    let title = format!("Citybound v0.17 | {} | {:02}:{:02} | T:{}",
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
        simd_render::fill_rect_alpha_simd(fb, w, h, panel_x, panel_y, panel_w, panel_h, 0xCC_15_15_30);
    }

    let mut y = panel_y + 5;
    let x = panel_x + 5;

    draw_text(fb, w, h, x, y, "ESTADISTICAS", 0xFF_FF_D7_00);
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

    draw_text(fb, w, h, x, y, &format!("Tesoro: ${:.0}", gw.finance.treasury), 0xFF_00_FF_00);
    y += 10;

    let approval = gw.politics.global_approval;
    let approval_color = if approval > 0.5 { 0xFF_00_FF_00 } else if approval > 0.3 { 0xFF_FF_FF_00 } else { 0xFF_FF_44_44 };
    draw_text(fb, w, h, x, y, &format!("Aprob: {:.0}%", approval * 100.0), approval_color);
    y += 10;

    draw_text(fb, w, h, x, y, &format!("Hora: {:02}:{:02}", gw.time_of_day / 60, gw.time_of_day % 60), COLOR_UI_TEXT);
    y += 10;

    draw_text(fb, w, h, x, y, &format!("Tick: {}", gw.sim_tick), 0xFF_88_88_88);

    y += 8;
    draw_text(fb, w, h, x, y, "------------", 0xFF_44_44_44);
    y += 10;

    let congestion = if !gw.lane_manager.lanes.is_empty() {
        gw.lane_manager.lanes.iter().map(|l| l.congestion).sum::<f32>() / gw.lane_manager.lanes.len() as f32
    } else { 0.0 };
    let cong_color = if congestion > 0.5 { 0xFF_FF_44_44 } else { 0xFF_44_FF_44 };
    draw_text(fb, w, h, x, y, &format!("Traf: {:.0}%", congestion * 100.0), cong_color);
    y += 10;

    let lv = gw.land_value_map.get(64, 64);
    draw_text(fb, w, h, x, y, &format!("Suelo: ${:.0}", lv), 0xFF_CC_AA_88);
}

// ---------------------------------------------------------------------------
// FUNCIONES DE DIBUJO
// ---------------------------------------------------------------------------

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
// TEXTO (5x7 pixel font)
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

#[rustfmt::skip]
fn get_glyph(ch: char) -> [u8; 7] {
    match ch {
        'A' => [0x0E, 0x11, 0x11, 0x1F, 0x11, 0x11, 0x11],
        'B' => [0x1E, 0x11, 0x1E, 0x11, 0x11, 0x1E, 0x00],
        'C' => [0x0E, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0E],
        'D' => [0x1E, 0x11, 0x11, 0x11, 0x11, 0x1E, 0x00],
        'E' => [0x1F, 0x10, 0x1E, 0x10, 0x10, 0x1F, 0x00],
        'F' => [0x1F, 0x10, 0x1E, 0x10, 0x10, 0x10, 0x00],
        'G' => [0x0E, 0x11, 0x10, 0x13, 0x11, 0x11, 0x0E],
        'H' => [0x11, 0x11, 0x1F, 0x11, 0x11, 0x11, 0x00],
        'I' => [0x0E, 0x04, 0x04, 0x04, 0x04, 0x0E, 0x00],
        'J' => [0x07, 0x02, 0x02, 0x02, 0x12, 0x0C, 0x00],
        'K' => [0x11, 0x12, 0x1C, 0x12, 0x12, 0x11, 0x00],
        'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x1F, 0x00],
        'M' => [0x11, 0x1B, 0x15, 0x11, 0x11, 0x11, 0x00],
        'N' => [0x11, 0x19, 0x15, 0x13, 0x11, 0x11, 0x00],
        'O' => [0x0E, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0E],
        'P' => [0x1E, 0x11, 0x11, 0x1E, 0x10, 0x10, 0x00],
        'Q' => [0x0E, 0x11, 0x11, 0x15, 0x12, 0x0D, 0x00],
        'R' => [0x1E, 0x11, 0x11, 0x1E, 0x14, 0x12, 0x11],
        'S' => [0x0F, 0x10, 0x0E, 0x01, 0x01, 0x1E, 0x00],
        'T' => [0x1F, 0x04, 0x04, 0x04, 0x04, 0x04, 0x00],
        'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0E, 0x00],
        'V' => [0x11, 0x11, 0x11, 0x11, 0x0A, 0x04, 0x00],
        'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x1B, 0x00],
        'X' => [0x11, 0x11, 0x0A, 0x04, 0x0A, 0x11, 0x11],
        'Y' => [0x11, 0x11, 0x0A, 0x04, 0x04, 0x04, 0x00],
        'Z' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x1F, 0x00],
        '0' => [0x0E, 0x13, 0x15, 0x15, 0x15, 0x19, 0x0E],
        '1' => [0x04, 0x0C, 0x04, 0x04, 0x04, 0x04, 0x0E],
        '2' => [0x0E, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1F],
        '3' => [0x0E, 0x11, 0x01, 0x06, 0x01, 0x11, 0x0E],
        '4' => [0x12, 0x12, 0x12, 0x1F, 0x02, 0x02, 0x00],
        '5' => [0x1F, 0x10, 0x1E, 0x01, 0x01, 0x1E, 0x00],
        '6' => [0x0E, 0x10, 0x1E, 0x11, 0x11, 0x0E, 0x00],
        '7' => [0x1F, 0x01, 0x02, 0x04, 0x08, 0x08, 0x00],
        '8' => [0x0E, 0x11, 0x11, 0x0E, 0x11, 0x11, 0x0E],
        '9' => [0x0E, 0x11, 0x11, 0x0F, 0x01, 0x11, 0x0E],
        ' ' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
        '.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0C, 0x0C],
        ',' => [0x00, 0x00, 0x00, 0x00, 0x04, 0x08, 0x00],
        ':' => [0x00, 0x0C, 0x0C, 0x00, 0x0C, 0x0C, 0x00],
        '-' => [0x00, 0x00, 0x00, 0x1E, 0x00, 0x00, 0x00],
        '/' => [0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x00],
        '|' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        '[' => [0x0E, 0x08, 0x08, 0x08, 0x08, 0x0E, 0x00],
        ']' => [0x0E, 0x02, 0x02, 0x02, 0x02, 0x0E, 0x00],
        '$' => [0x04, 0x0F, 0x14, 0x0E, 0x05, 0x1E, 0x04],
        '%' => [0x19, 0x1A, 0x02, 0x04, 0x0B, 0x13, 0x00],
        '+' => [0x00, 0x04, 0x04, 0x1F, 0x04, 0x04, 0x00],
        '!' => [0x04, 0x04, 0x04, 0x04, 0x00, 0x04, 0x00],
        _   => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    }
}
