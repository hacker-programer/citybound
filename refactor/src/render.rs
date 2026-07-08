// Rycimmu v0.19.0 — Renderizado con Sprites Reales
//
// Usa el TextureAtlas para blitear sprites reales de los spritesheets
// Kenney CC0. El terreno se tilea con la textura de fondo
// (whispers_avalon_ground). Edificios y vehículos usan atlas.blit_sprite().
//
// PALETA: Tonos tierra armonizados (fallback procedural).

use crate::ecs::{GameWorld, Camera, ConstructionState, TrafficCar, Position, ZoneComponent, ZoneType, BuildingType};
use crate::texture_atlas::{TextureAtlas, BuildingTileStyle};
use crate::simd_render;

// ═══════════════════════════════════════════════════════════
// PALETA DE COLORES (ARGB) — fallback procedural
// ═══════════════════════════════════════════════════════════

pub const COLOR_GRASS:       u32 = 0xFF_4A_7C_3F;
pub const COLOR_DIRT:        u32 = 0xFF_9B_8C_70;
pub const COLOR_SAND:        u32 = 0xFF_C4_B8_8C;
pub const COLOR_WATER:       u32 = 0xFF_2A_5A_8A;
pub const COLOR_BACKGROUND:  u32 = 0xFF_1A_1A_2E;

pub const COLOR_ZONE_RESIDENTIAL: u32 = 0x22_7B_A0_5C;
pub const COLOR_ZONE_COMMERCIAL:  u32 = 0x22_5C_8A_B8;
pub const COLOR_ZONE_INDUSTRIAL:  u32 = 0x22_B0_7A_6E;
pub const COLOR_ZONE_AGRICULTURAL:u32 = 0x22_8C_A8_6A;

pub const COLOR_LANE_LINE:   u32 = 0x33_AA_AA_AA;
pub const COLOR_CAR:         u32 = 0xFF_CC_CC_CC;
pub const COLOR_CAR_ALT:     u32 = 0xFF_4A_6A_8A;

pub const COLOR_UI_TEXT:     u32 = 0xFF_FF_FF_FF;
pub const COLOR_UI_BG:       u32 = 0xAA_00_00_00;

// Constantes legacy
pub const COLOR_ZONE_ROAD:   u32 = 0x18_6B_6B_6B;
pub const COLOR_ZONE_PARK:   u32 = 0x18_5A_8C_4A;
pub const COLOR_BUILDING_HOUSE:     u32 = 0xFF_C4_8E_6A;
pub const COLOR_BUILDING_APARTMENT: u32 = 0xFF_A0_A0_A8;
pub const COLOR_BUILDING_SHOP:      u32 = 0xFF_5C_A0_B8;
pub const COLOR_BUILDING_OFFICE:    u32 = 0xFF_7A_8B_98;
pub const COLOR_BUILDING_FACTORY:   u32 = 0xFF_8A_7A_6E;
pub const COLOR_BUILDING_FARM:      u32 = 0xFF_8C_A8_6A;
pub const COLOR_BUILDING_HOSPITAL:  u32 = 0xFF_E8_E8_F0;
pub const COLOR_BUILDING_SCHOOL:    u32 = 0xFF_E8_D8_8C;
pub const COLOR_BUILDING_POLICE:    u32 = 0xFF_5C_70_C4;
pub const COLOR_CONGESTION_LOW:     u32 = 0x44_4C_AF_50;
pub const COLOR_CONGESTION_MED:     u32 = 0x44_FF_C1_07;
pub const COLOR_CONGESTION_HIGH:    u32 = 0x44_EF_53_50;

// ═══════════════════════════════════════════════════════════
// RENDER CON SPRITES REALES (NUEVO v0.19)
// ═══════════════════════════════════════════════════════════

/// Renderiza el mundo usando sprites del atlas de texturas.
/// `ground_idx` es el índice de la textura de terreno de fondo.
pub fn render_world_sprites(
    game_world: &GameWorld,
    atlas: &TextureAtlas,
    framebuffer: &mut [u32],
    width: usize,
    height: usize,
    ground_idx: usize,
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

    // Tiles de 64×64 — cell_size = 64 para que 1 tile = 1 celda a zoom 1.0
    let cell_size = 64.0;
    let scale = cell_size * cam_zoom;
    let offset_x = (width as f32 / 2.0) - cam_offset_x * scale;
    let offset_y = (height as f32 / 2.0) - cam_offset_y * scale;

    // Capa 0: Terreno con textura tileada
    if ground_idx > 0 && atlas.get_tile(ground_idx).width >= 32 {
        render_terrain_tiled(atlas, framebuffer, width, height, ground_idx, offset_x, offset_y, scale);
    } else {
        render_terrain_procedural(game_world, framebuffer, width, height, offset_x, offset_y, scale);
    }

    // Capa 1: Zonas
    render_zones(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // Capa 2: Carriles
    render_lanes(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // Capa 3: Edificios y vehículos CON SPRITES
    render_entities_sprites(game_world, atlas, framebuffer, width, height, offset_x, offset_y, scale);

    // Capa 4: UI
    // Capa 4: UI
    render_ui(game_world, framebuffer, width, height);
}

// ═══════════════════════════════════════════════════════════
// RENDER LEGACY (backward compat)
// ═══════════════════════════════════════════════════════════

pub fn render_world_cached(
    game_world: &GameWorld,
    atlas: &TextureAtlas,
    framebuffer: &mut [u32],
    width: usize,
    height: usize,
) {
    render_world_sprites(game_world, atlas, framebuffer, width, height, 0);
}

// ═══════════════════════════════════════════════════════════
// TERRENO TILEADO CON TEXTURA
// ═══════════════════════════════════════════════════════════
    h: usize,
    ground_idx: usize,
    offset_x: f32,
    offset_y: f32,
    scale: f32,
) {
    // La textura de fondo se tilea en un grid alineado con offset_x/offset_y
    let tile = atlas.get_tile(ground_idx);
    let tw = tile.width as i32;
    let th = tile.height as i32;

    // Calcular offset de tileo para scroll suave
    let tile_offset_x = (offset_x.rem_euclid(tw as f32 * scale)) as i32;
    let tile_offset_y = (offset_y.rem_euclid(th as f32 * scale)) as i32;

    let w_i32 = w as i32;
    let h_i32 = h as i32;

    for py in 0..h_i32 {
        let src_y = ((py - tile_offset_y).rem_euclid((th as f32 * scale) as i32) as f32 / scale) as i32;
        if src_y < 0 || src_y >= th { continue; }
        let row_start = (py as usize) * w;
        let tile_row = (src_y as usize) * tw as usize;

        for px in 0..w_i32 {
            let src_x = ((px - tile_offset_x).rem_euclid((tw as f32 * scale) as i32) as f32 / scale) as i32;
            if src_x < 0 || src_x >= tw { continue; }
            unsafe {
                *fb.get_unchecked_mut(row_start + px as usize) = tile.pixels[tile_row + src_x as usize];
            }
        }
    }
}

fn render_terrain_procedural(
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
        let world_y = (py as f32 - oy) / scale;
        if world_y < 0.0 || world_y >= grid_size {
            for px in 0..w_i32 {
                unsafe { *fb.get_unchecked_mut((py as usize) * w + px as usize) = COLOR_BACKGROUND; }
            }
            continue;
        }

        let ty = (world_y as usize).min(127);
        let row_start = (py as usize) * w;

        for px in 0..w_i32 {
            let world_x = (px as f32 - ox) / scale;
            if world_x < 0.0 || world_x >= grid_size {
                unsafe { *fb.get_unchecked_mut(row_start + px as usize) = COLOR_BACKGROUND; }
                continue;
            }
            let tx = (world_x as usize).min(127);
            unsafe { *fb.get_unchecked_mut(row_start + px as usize) = gw.terrain.baked_color(tx, ty); }
        }
    }
}

// ═══════════════════════════════════════════════════════════
// ZONAS
// ═══════════════════════════════════════════════════════════

fn render_zones(gw: &GameWorld, fb: &mut [u32], w: usize, h: usize,
                ox: f32, oy: f32, scale: f32) {
    if scale < 1.2 { return; }

    for (_entity, (pos, zone)) in gw.world.query::<(&Position, &ZoneComponent)>().iter() {
        if zone.density <= 0 { continue; }

        let sx = (pos.x * scale + ox) as i32;
        let sy = (pos.y * scale + oy) as i32;
        let cell_w = scale as i32;

        let color = match zone.zone_type {
            ZoneType::Residential  => COLOR_ZONE_RESIDENTIAL,
            ZoneType::Commercial   => COLOR_ZONE_COMMERCIAL,
            ZoneType::Industrial   => COLOR_ZONE_INDUSTRIAL,
            ZoneType::Agricultural => COLOR_ZONE_AGRICULTURAL,
            _ => continue,
        };

        for x in (sx.max(0)..(sx + cell_w).min(w as i32)).step_by(4) {
            if sy >= 0 && sy < h as i32 { unsafe { blend_pixel(fb, w, x, sy, color); } }
            let by = sy + cell_w;
            if by >= 0 && by < h as i32 { unsafe { blend_pixel(fb, w, x, by, color); } }
        }
        for y in (sy.max(0)..(sy + cell_w).min(h as i32)).step_by(4) {
            if sx >= 0 && sx < w as i32 { unsafe { blend_pixel(fb, w, sx, y, color); } }
            let bx = sx + cell_w;
            if bx >= 0 && bx < w as i32 { unsafe { blend_pixel(fb, w, bx, y, color); } }
        }
    }
}

// ═══════════════════════════════════════════════════════════
// CARRILES
// ═══════════════════════════════════════════════════════════

fn render_lanes(gw: &GameWorld, fb: &mut [u32], w: usize, h: usize,
                ox: f32, oy: f32, scale: f32) {
    if scale < 0.6 { return; }

    for lane in &gw.lane_manager.lanes {
        let sx1 = (lane.start_x * scale + ox) as i32;
        let sy1 = (lane.start_y * scale + oy) as i32;
        let sx2 = (lane.end_x * scale + ox) as i32;
        let sy2 = (lane.end_y * scale + oy) as i32;

        let color = if lane.congestion > 0.7 {
            COLOR_CONGESTION_HIGH
        } else if lane.congestion > 0.3 {
            COLOR_CONGESTION_MED
        } else {
            COLOR_LANE_LINE
        };

        draw_line(fb, w, h, sx1, sy1, sx2, sy2, color);
    }
}

// ═══════════════════════════════════════════════════════════
// ENTIDADES CON SPRITES (NUEVO)
// ═══════════════════════════════════════════════════════════

fn render_entities_sprites(
    gw: &GameWorld,
    atlas: &TextureAtlas,
    fb: &mut [u32],
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    scale: f32,
) {
    // Edificios — usar sprites del atlas
    for (_entity, (pos, _renderable, cs)) in gw.world.query::<(&Position, &crate::ecs::Renderable, &ConstructionState)>().iter() {
        let cx = (pos.x * scale + ox) as i32;
        let cy = (pos.y * scale + oy) as i32;

        // Mapear BuildingType → BuildingTileStyle
        let style = building_type_to_style(cs.building_type);
        let sprite_idx = atlas.categories.building_sprite(style);

        if sprite_idx > 0 {
            // Usar escala: 16px tile → cell_size (32px a zoom 1.0)
            let sprite_scale = scale / 16.0;
            atlas.blit_sprite(sprite_idx, fb, w, h, cx, cy, sprite_scale);
        } else {
            // Fallback procedural si no hay sprite
            let size = (3.0 * scale) as i32;
            if size >= 2 {
                draw_building(fb, w, h, cx, cy, size, cs.building_type, cs.progress);
            }
        }
    }

    // Vehículos — usar sprites del atlas
    for (_entity, (pos, _car)) in gw.world.query::<(&Position, &TrafficCar)>().iter() {
        let cx = (pos.x * scale + ox) as i32;
        let cy = (pos.y * scale + oy) as i32;

        if cx < -20 || cx > w as i32 + 20 || cy < -20 || cy > h as i32 + 20 { continue; }

        let vehicle_idx = if atlas.categories.vehicles.is_empty() {
            0
        } else {
            // Usar posición como seed pseudo-aleatorio para variedad
            let seed = (pos.x as usize).wrapping_mul(7) ^ (pos.y as usize).wrapping_mul(13);
            atlas.categories.vehicles[seed % atlas.categories.vehicles.len()]
        };

        if vehicle_idx > 0 {
            let sprite_scale = scale / 16.0 * 0.7; // coches más pequeños que celdas
            atlas.blit_sprite(vehicle_idx, fb, w, h, cx, cy, sprite_scale);
        } else {
            let car_size = (scale * 0.35) as i32;
            if car_size >= 2 {
                draw_car(fb, w, h, cx, cy, car_size);
            }
        }
    }
}

/// Mapea BuildingType → BuildingTileStyle para lookup en CategoryMap
fn building_type_to_style(bt: BuildingType) -> BuildingTileStyle {
    match bt {
        BuildingType::House => BuildingTileStyle::House,
        BuildingType::Apartment => BuildingTileStyle::Apartment,
        BuildingType::Shop => BuildingTileStyle::Shop,
        BuildingType::Office => BuildingTileStyle::Office,
        BuildingType::Factory => BuildingTileStyle::Factory,
        BuildingType::Farm => BuildingTileStyle::Farm,
        BuildingType::Hospital => BuildingTileStyle::Hospital,
        BuildingType::School => BuildingTileStyle::School,
        BuildingType::Police => BuildingTileStyle::Police,
        _ => BuildingTileStyle::Generic,
    }
}

// ═══════════════════════════════════════════════════════════
fn draw_building(fb: &mut [u32], fb_w: usize, fb_h: usize,
                 cx: i32, cy: i32, size: i32, btype: BuildingType, progress: f32) {
    let s = (size as f32 * progress) as i32;
    if s < 2 { return; }

    match btype {
        BuildingType::House     => draw_house(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Apartment => draw_apartment(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Shop      => draw_shop(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Office    => draw_office(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Factory   => draw_factory(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Farm      => draw_farm(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Hospital  => draw_hospital(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::School    => draw_school(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Police    => draw_police(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Church
        | BuildingType::Warehouse
        | BuildingType::Park
        | BuildingType::Generic => draw_house(fb, fb_w, fb_h, cx, cy, s),
    }
}

fn draw_house(fb: &mut [u32], fb_w: usize, fb_h: usize, cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 3 / 4;
    let roof_h = size / 3;

    fill_rect(fb, fb_w, fb_h, cx - hw, cy - hw + roof_h, size, body_h, COLOR_BUILDING_HOUSE);
    rect_outline(fb, fb_w, fb_h, cx - hw, cy - hw + roof_h, size, body_h, darken(COLOR_BUILDING_HOUSE, 30));

    let roof = 0xFF_8B_45_3A;
    let apex_y = cy - hw;
    for row in 0..roof_h {
        let y = apex_y + row;
        let rw = (row * size / roof_h.max(1)) as i32;
        if rw > 0 { fill_rect(fb, fb_w, fb_h, cx - rw/2, y, rw, 1, roof); }
    }

    let ws = size / 6;
    if ws > 1 {
        fill_rect(fb, fb_w, fb_h, cx - hw + size/5, cy - hw + roof_h + size/5, ws, ws, 0xFF_E8_D8_8C);
        fill_rect(fb, fb_w, fb_h, cx + hw - size/5 - ws, cy - hw + roof_h + size/5, ws, ws, 0xFF_E8_D8_8C);
    }
}
}

fn draw_apartment(fb: &mut [u32], fb_w: usize, fb_h: usize, cx: i32, cy: i32, size: i32) {
    let body_h = size * 5 / 4;
    fill_rect(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, COLOR_BUILDING_APARTMENT);
    rect_outline(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, darken(COLOR_BUILDING_APARTMENT, 40));
}

fn draw_shop(fb: &mut [u32], fb_w: usize, fb_h: usize, cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 2 / 3;
    fill_rect(fb, fb_w, fb_h, cx - hw, cy - hw + hw/2, size, body_h, COLOR_BUILDING_SHOP);
    rect_outline(fb, fb_w, fb_h, cx - hw, cy - hw + hw/2, size, body_h, darken(COLOR_BUILDING_SHOP, 30));
}

fn draw_office(fb: &mut [u32], fb_w: usize, fb_h: usize, cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 5 / 4;
    fill_rect(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, COLOR_BUILDING_OFFICE);
    rect_outline(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, darken(COLOR_BUILDING_OFFICE, 40));
}

fn draw_factory(fb: &mut [u32], fb_w: usize, fb_h: usize, cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 2 / 3;
    fill_rect(fb, fb_w, fb_h, cx - hw, cy - hw + size/4, size, body_h, COLOR_BUILDING_FACTORY);
    rect_outline(fb, fb_w, fb_h, cx - hw, cy - hw + size/4, size, body_h, darken(COLOR_BUILDING_FACTORY, 40));
}

fn draw_farm(fb: &mut [u32], fb_w: usize, fb_h: usize, cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 3 / 4;
    fill_rect(fb, fb_w, fb_h, cx - hw + size/6, cy - hw + size/4, size*2/3, body_h, COLOR_BUILDING_FARM);
    rect_outline(fb, fb_w, fb_h, cx - hw + size/6, cy - hw + size/4, size*2/3, body_h, darken(COLOR_BUILDING_FARM, 40));
}

fn draw_hospital(fb: &mut [u32], fb_w: usize, fb_h: usize, cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size;
    fill_rect(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, COLOR_BUILDING_HOSPITAL);
    rect_outline(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, 0xFF_C0_C0_C8);
}

fn draw_school(fb: &mut [u32], fb_w: usize, fb_h: usize, cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 4 / 5;
    fill_rect(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, COLOR_BUILDING_SCHOOL);
    rect_outline(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, darken(COLOR_BUILDING_SCHOOL, 40));
}

fn draw_police(fb: &mut [u32], fb_w: usize, fb_h: usize, cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 4 / 5;
    fill_rect(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, COLOR_BUILDING_POLICE);
    rect_outline(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, darken(COLOR_BUILDING_POLICE, 40));
}

// ═══════════════════════════════════════════════════════════
// VEHÍCULO (FALLBACK)
// ═══════════════════════════════════════════════════════════

fn draw_car(fb: &mut [u32], fb_w: usize, fb_h: usize, cx: i32, cy: i32, size: i32) {
    let cw = size * 2;
    let ch = size * 2 / 3;
    fill_rect(fb, fb_w, fb_h, cx - cw/2, cy - ch/2, cw, ch, COLOR_CAR);
    rect_outline(fb, fb_w, fb_h, cx - cw/2, cy - ch/2, cw, ch, 0xFF_88_88_88);
    fill_rect(fb, fb_w, fb_h, cx - cw/4, cy - ch/2, cw/2, ch, COLOR_CAR_ALT);
}

// ═══════════════════════════════════════════════════════════
// UI
// ═══════════════════════════════════════════════════════════

fn render_ui(gw: &GameWorld, fb: &mut [u32], w: usize, h: usize) {
    let w_i32 = w as i32;
    let h_i32 = h as i32;

    unsafe { simd_render::fill_rect_alpha_simd(fb, w, h, 0, 0, w_i32, 22, COLOR_UI_BG); }

    let mode = if gw.design_tool.active { "DISENO" } else { "SIMULACION" };
    let title = format!("Rycimmu v0.19 | {} | {:02}:{:02} | T:{}",
        mode, gw.time_of_day / 60, gw.time_of_day % 60, gw.sim_tick);
    draw_text(fb, w, h, 8, 4, &title, COLOR_UI_TEXT);

    unsafe { simd_render::fill_rect_alpha_simd(fb, w, h, 0, h_i32 - 18, w_i32, 18, COLOR_UI_BG); }

    let help = if gw.design_tool.active {
        "WASD: Mover | Click: Construir | [Tab]: Salir | ESC: Cerrar"
    } else {
        "WASD: Mover | Rueda: Zoom | [Tab]: Disenar | ESC: Salir"
    };
    draw_text(fb, w, h, 8, h_i32 - 14, help, COLOR_UI_TEXT);

    // Minimapa
    let mm_x = w_i32 - 68;
    let mm_y = h_i32 - 88;
    unsafe { simd_render::fill_rect_alpha_simd(fb, w, h, mm_x, mm_y, 64, 64, COLOR_UI_BG); }
    rect_outline(fb, w, h, mm_x - 1, mm_y - 1, 66, 66, 0xFF_88_88_88);
}

pub fn render_stats_panel(gw: &GameWorld, fb: &mut [u32], w: usize, h: usize, fps: u32) {
    let pop = gw.world.query::<&ConstructionState>().iter().count();
    let txt = format!("FPS: {} | Pop: {} | Tick: {}", fps, pop, gw.sim_tick);
    draw_text(fb, w, h, 8, h as i32 - 28, &txt, COLOR_UI_TEXT);
}

// ═══════════════════════════════════════════════════════════
// PRIMITIVAS DE DIBUJO
// ═══════════════════════════════════════════════════════════

fn fill_rect(fb: &mut [u32], fb_w: usize, fb_h: usize,
             x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x + rw).min(fb_w as i32);
    let y2 = (y + rh).min(fb_h as i32);
    if x1 >= x2 || y1 >= y2 { return; }
    for py in y1..y2 {
        unsafe {
            let row = py as usize * fb_w;
            for px in x1..x2 { *fb.get_unchecked_mut(row + px as usize) = color; }
        }
    }
}

fn rect_outline(fb: &mut [u32], fb_w: usize, fb_h: usize,
                x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x + rw - 1).min(fb_w as i32 - 1);
    let y2 = (y + rh - 1).min(fb_h as i32 - 1);
    if x1 > x2 || y1 > y2 { return; }
    for px in x1..=x2 {
        if y >= 0 && y < fb_h as i32 { unsafe { *fb.get_unchecked_mut(y as usize * fb_w + px as usize) = color; } }
        if y2 >= 0 && y2 < fb_h as i32 { unsafe { *fb.get_unchecked_mut(y2 as usize * fb_w + px as usize) = color; } }
    }
    for py in y1..=y2 {
        if x >= 0 && x < fb_w as i32 { unsafe { *fb.get_unchecked_mut(py as usize * fb_w + x as usize) = color; } }
        if x2 >= 0 && x2 < fb_w as i32 { unsafe { *fb.get_unchecked_mut(py as usize * fb_w + x2 as usize) = color; } }
    }
}

fn draw_line(fb: &mut [u32], w: usize, h: usize,
             x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        if x >= 0 && x < w as i32 && y >= 0 && y < h as i32 {
            unsafe { blend_pixel(fb, w, x, y, color); }
        }
        if x == x1 && y == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x += sx; }
        if e2 <= dx { err += dx; y += sy; }
    }
}

#[inline(always)]
unsafe fn blend_pixel(fb: &mut [u32], fb_w: usize, x: i32, y: i32, color: u32) {
    let idx = y as usize * fb_w + x as usize;
    if idx < fb.len() {
        *fb.get_unchecked_mut(idx) = blend(*fb.get_unchecked(idx), color);
    }
}

#[inline(always)]
fn blend(bg: u32, fg: u32) -> u32 {
    let fa = (fg >> 24) & 0xFF;
    if fa == 0 { return bg; }
    if fa == 255 { return fg; }
    let r = (((fg >> 16) & 0xFF) * fa + ((bg >> 16) & 0xFF) * (255 - fa)) / 255;
    let g = (((fg >> 8) & 0xFF) * fa + ((bg >> 8) & 0xFF) * (255 - fa)) / 255;
    let b = ((fg & 0xFF) * fa + (bg & 0xFF) * (255 - fa)) / 255;
    0xFF_00_00_00 | (r << 16) | (g << 8) | b
}

#[inline(always)]
fn darken(color: u32, amount: u32) -> u32 {
    let r = ((color >> 16) & 0xFF).saturating_sub(amount);
    let g = ((color >> 8) & 0xFF).saturating_sub(amount);
    let b = (color & 0xFF).saturating_sub(amount);
    (color & 0xFF_00_00_00) | (r << 16) | (g << 8) | b
}

// ═══════════════════════════════════════════════════════════
// TEXTO BITMAP
// ═══════════════════════════════════════════════════════════

fn draw_text(fb: &mut [u32], fb_w: usize, _fb_h: usize,
             x: i32, y: i32, text: &str, color: u32) {
    let mut cx = x;
    for ch in text.chars() {
        if cx > fb_w as i32 { break; }
        let glyph: [u8; 7] = match ch {
            'A'..='Z' => FONT[(ch as u8 - b'A') as usize],
            'a'..='z' => FONT[(ch as u8 - b'a') as usize],
            '0'..='9' => DIGITS[(ch as u8 - b'0') as usize],
            ' ' => [0; 7],
            ':' => [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0],
            '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
            '/' => [0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0, 0],
            '[' => [0b01110, 0b01000, 0b01000, 0b01000, 0b01000, 0b01000, 0b01110],
            ']' => [0b01110, 0b00010, 0b00010, 0b00010, 0b00010, 0b00010, 0b01110],
            '|' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
            _ => [0; 7],
        };
        for row in 0..7 {
            let bits = glyph[row];
            for col in 0..5 {
                if (bits >> (4 - col)) & 1 != 0 {
                    let px = cx + col as i32;
                    let py = y + row as i32;
                    if px >= 0 && px < fb_w as i32 && py >= 0 {
                        unsafe {
                            let idx = py as usize * fb_w + px as usize;
                            if idx < fb.len() { *fb.get_unchecked_mut(idx) = color; }
                        }
                    }
                }
            }
        }
        cx += 6;
    }
}

const FONT: [[u8; 7]; 26] = [
    [0b01110,0b10001,0b10001,0b11111,0b10001,0b10001,0b10001], // A
    [0b11110,0b10001,0b10001,0b11110,0b10001,0b10001,0b11110], // B
    [0b01110,0b10001,0b10000,0b10000,0b10000,0b10001,0b01110], // C
    [0b11110,0b10001,0b10001,0b10001,0b10001,0b10001,0b11110], // D
    [0b11111,0b10000,0b10000,0b11110,0b10000,0b10000,0b11111], // E
    [0b11111,0b10000,0b10000,0b11110,0b10000,0b10000,0b10000], // F
    [0b01110,0b10001,0b10000,0b10011,0b10001,0b10001,0b01110], // G
    [0b10001,0b10001,0b10001,0b11111,0b10001,0b10001,0b10001], // H
    [0b01110,0b00100,0b00100,0b00100,0b00100,0b00100,0b01110], // I
    [0b00111,0b00001,0b00001,0b00001,0b10001,0b10001,0b01110], // J
    [0b10001,0b10010,0b10100,0b11000,0b10100,0b10010,0b10001], // K
    [0b10000,0b10000,0b10000,0b10000,0b10000,0b10000,0b11111], // L
    [0b10001,0b11011,0b10101,0b10001,0b10001,0b10001,0b10001], // M
    [0b10001,0b11001,0b10101,0b10011,0b10001,0b10001,0b10001], // N
    [0b01110,0b10001,0b10001,0b10001,0b10001,0b10001,0b01110], // O
    [0b11110,0b10001,0b10001,0b11110,0b10000,0b10000,0b10000], // P
    [0b01110,0b10001,0b10001,0b10001,0b10101,0b10010,0b01101], // Q
    [0b11110,0b10001,0b10001,0b11110,0b10100,0b10010,0b10001], // R
    [0b01110,0b10001,0b10000,0b01110,0b00001,0b10001,0b01110], // S
    [0b11111,0b00100,0b00100,0b00100,0b00100,0b00100,0b00100], // T
    [0b10001,0b10001,0b10001,0b10001,0b10001,0b10001,0b01110], // U
    [0b10001,0b10001,0b10001,0b01010,0b01010,0b00100,0b00100], // V
    [0b10001,0b10001,0b10001,0b10101,0b10101,0b11011,0b10001], // W
    [0b10001,0b01010,0b00100,0b00100,0b00100,0b01010,0b10001], // X
    [0b10001,0b01010,0b00100,0b00100,0b00100,0b00100,0b00100], // Y
    [0b11111,0b00001,0b00010,0b00100,0b01000,0b10000,0b11111], // Z
];

const DIGITS: [[u8; 7]; 10] = [
    [0b01110,0b10001,0b10011,0b10101,0b11001,0b10001,0b01110], // 0
    [0b00100,0b01100,0b00100,0b00100,0b00100,0b00100,0b01110], // 1
    [0b01110,0b10001,0b00001,0b00010,0b00100,0b01000,0b11111], // 2
    [0b11111,0b00010,0b00100,0b00010,0b00001,0b10001,0b01110], // 3
    [0b00010,0b00110,0b01010,0b10010,0b11111,0b00010,0b00010], // 4
    [0b11111,0b10000,0b11110,0b00001,0b00001,0b10001,0b01110], // 5
    [0b00110,0b01000,0b10000,0b11110,0b10001,0b10001,0b01110], // 6
    [0b11111,0b00001,0b00010,0b00100,0b01000,0b01000,0b01000], // 7
    [0b01110,0b10001,0b10001,0b01110,0b10001,0b10001,0b01110], // 8
    [0b01110,0b10001,0b10001,0b01111,0b00001,0b00010,0b01100], // 9
];
