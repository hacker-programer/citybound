// Módulo de Renderizado Software v0.18.0 — Refactor Visual Completo
//
// ARQUITECTURA:
// - Terreno: usa TerrainMap.baked_colors (Perlin noise suave, sin ruido de tiles)
// - Edificios: formas arquitectónicas dibujadas proceduralmente:
//   * Casas: rectángulo + tejado triangular + puerta + ventanas
//   * Apartamentos: torre alta + ventanas en cuadrícula
//   * Tiendas: base ancha + toldo/awning + escaparate
//   * Oficinas: torre de cristal + líneas horizontales
//   * Fábricas: nave industrial + chimeneas
//   * Granjas: granero + silo
//   * Hospital: edificio blanco + cruz roja
//   * Escuela: edificio amarillo + campana
//   * Policía: edificio azul + estrella
// - Zonas: solo bordes punteados sutiles (sin relleno)
// - Carriles: líneas grises finas
// - Vehículos: pequeños rectángulos coloreados con dirección
//
// PALETA: Tonos tierra armonizados, alpha reducido, cero magenta.

use crate::ecs::{GameWorld, Camera, ConstructionState, TrafficCar, Position, ZoneComponent, ZoneType, BuildingType};
use crate::texture_atlas::TextureAtlas;
use crate::simd_render;

// ═══════════════════════════════════════════════════════════════════════════
// PALETA DE COLORES UNIFICADA (ARGB) — tonos tierra, muted
// ═══════════════════════════════════════════════════════════════════════════

// Terreno (usamos baked_colors del TerrainMap, estos son fallback)
pub const COLOR_GRASS:       u32 = 0xFF_4A_7C_3F;
pub const COLOR_DIRT:        u32 = 0xFF_9B_8C_70;
pub const COLOR_SAND:        u32 = 0xFF_C4_B8_8C;
pub const COLOR_WATER:       u32 = 0xFF_2A_5A_8A;
pub const COLOR_BACKGROUND:  u32 = 0xFF_1A_1A_2E;

// Zonas (alpha ~10-15%, muy sutiles)
pub const COLOR_ZONE_RESIDENTIAL: u32 = 0x22_7B_A0_5C;
pub const COLOR_ZONE_COMMERCIAL:  u32 = 0x22_5C_8A_B8;
pub const COLOR_ZONE_INDUSTRIAL:  u32 = 0x22_B0_7A_6E;
pub const COLOR_ZONE_AGRICULTURAL:u32 = 0x22_8C_A8_6A;

// Edificios - colores base (luego cada forma usa variantes más claras/oscuras)
pub const COLOR_HOUSE:       u32 = 0xFF_C4_8E_6A; // terracota
pub const COLOR_APARTMENT:   u32 = 0xFF_A0_A0_A8; // gris medio
pub const COLOR_SHOP:        u32 = 0xFF_5C_A0_B8; // azul comercio
pub const COLOR_OFFICE:      u32 = 0xFF_7A_8B_98; // gris azulado
pub const COLOR_FACTORY:     u32 = 0xFF_8A_7A_6E; // marrón industrial
pub const COLOR_FARM:        u32 = 0xFF_8C_A8_6A; // verde rural
pub const COLOR_HOSPITAL:    u32 = 0xFF_E8_E8_F0; // blanco
pub const COLOR_SCHOOL:      u32 = 0xFF_E8_D8_8C; // amarillo
pub const COLOR_POLICE:      u32 = 0xFF_5C_70_C4; // azul policial

// Construcción
pub const COLOR_CONSTRUCTION: u32 = 0xFF_C8_A0_30;
pub const COLOR_SCAFFOLDING:  u32 = 0xFF_8A_7A_5A;

// Tráfico
pub const COLOR_LANE_LINE:   u32 = 0x33_AA_AA_AA;
pub const COLOR_CAR:         u32 = 0xFF_CC_CC_CC;
pub const COLOR_CAR_ALT:     u32 = 0xFF_4A_6A_8A;

// UI
pub const COLOR_UI_TEXT:     u32 = 0xFF_FF_FF_FF;
pub const COLOR_UI_BG:       u32 = 0xAA_00_00_00;

// ═══════════════════════════════════════════════════════════════════════════
// RENDER PRINCIPAL
// ═══════════════════════════════════════════════════════════════════════════

pub fn render_world_cached(
    game_world: &GameWorld,
    _atlas: &TextureAtlas,
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

    let cell_size = 32.0; // CELL_SIZE
    let scale = cell_size * cam_zoom;
    let offset_x = (width as f32 / 2.0) - cam_offset_x * scale;
    let offset_y = (height as f32 / 2.0) - cam_offset_y * scale;

    // 1. FONDO: terreno suave desde TerrainMap
    render_terrain_smooth(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // 2. ZONAS: bordes punteados sutiles
    render_zones_subtle(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // 3. CARRILES: líneas grises finas
    render_lanes_subtle(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // 4. ENTIDADES: edificios con formas arquitectónicas
    render_entities_architectural(game_world, framebuffer, width, height, offset_x, offset_y, scale);

    // 5. UI
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

// ═══════════════════════════════════════════════════════════════════════════
// 1. TERRENO SUAVE (desde TerrainMap baked_colors)
// ═══════════════════════════════════════════════════════════════════════════

fn render_terrain_smooth(
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
            unsafe {
                simd_render::fill_rect_simd(fb, w, h, 0, py, w_i32, 1, COLOR_BACKGROUND);
            }
            continue;
        }

        let ty = (world_y as usize).min(127);
        let row_start = (py as usize) * w;

        for px in 0..w_i32 {
            let world_x = (px as f32 - ox) / scale;
            if world_x < 0.0 || world_x >= grid_size {
                unsafe {
                    *fb.get_unchecked_mut(row_start + px as usize) = COLOR_BACKGROUND;
                }
                continue;
            }

            let tx = (world_x as usize).min(127);
            // Usar baked_colors del TerrainMap (Perlin noise horneado)
            let color = gw.terrain.baked_color(tx, ty);
            unsafe {
                *fb.get_unchecked_mut(row_start + px as usize) = color;
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. ZONAS SUTILES
// ═══════════════════════════════════════════════════════════════════════════

fn render_zones_subtle(
    gw: &GameWorld,
    fb: &mut [u32],
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    scale: f32,
) {
    if scale < 1.2 { return; } // Solo visible con zoom suficiente

    for (_entity, (pos, zone)) in gw.world.query::<(&Position, &ZoneComponent)>().iter() {
        if zone.density <= 0 { continue; }

        let sx = (pos.x * scale + ox) as i32;
        let sy = (pos.y * scale + oy) as i32;
        let cell_w = scale as i32;

        // Solo dibujar borde punteado (cada 4 píxeles)
        let color = match zone.zone_type {
            ZoneType::Residential => COLOR_ZONE_RESIDENTIAL,
            ZoneType::Commercial  => COLOR_ZONE_COMMERCIAL,
            ZoneType::Industrial  => COLOR_ZONE_INDUSTRIAL,
            ZoneType::Agricultural=> COLOR_ZONE_AGRICULTURAL,
            _ => 0,
        };

        if color == 0 { continue; }

        // Borde superior e inferior punteado
        let x0 = sx.max(0);
        let x1 = (sx + cell_w).min(w as i32);
        for x in (x0..x1).step_by(4) {
            if sy >= 0 && sy < h as i32 {
                unsafe { blend_pixel(fb, w, x, sy, color); }
            }
            let sy2 = sy + cell_w;
            if sy2 >= 0 && sy2 < h as i32 {
                unsafe { blend_pixel(fb, w, x, sy2, color); }
            }
        }

        // Borde izquierdo y derecho punteado
        let y0 = sy.max(0);
        let y1 = (sy + cell_w).min(h as i32);
        for y in (y0..y1).step_by(4) {
            if sx >= 0 && sx < w as i32 {
                unsafe { blend_pixel(fb, w, sx, y, color); }
            }
            let sx2 = sx + cell_w;
            if sx2 >= 0 && sx2 < w as i32 {
                unsafe { blend_pixel(fb, w, sx2, y, color); }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. CARRILES SUTILES
// ═══════════════════════════════════════════════════════════════════════════

fn render_lanes_subtle(
    gw: &GameWorld,
    fb: &mut [u32],
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    scale: f32,
) {
    if scale < 0.6 { return; }

    for lane in &gw.lane_manager.lanes {
        let sx1 = (lane.start_x * scale + ox) as i32;
        let sy1 = (lane.start_y * scale + oy) as i32;
        let sx2 = (lane.end_x * scale + ox) as i32;
        let sy2 = (lane.end_y * scale + oy) as i32;

        // Línea de medio carril (gris sutil)
        draw_line(fb, w, h, sx1, sy1, sx2, sy2, COLOR_LANE_LINE);

        // Bordes del carril (más sutiles aún)
        let nx = -(lane.end_y - lane.start_y);
        let ny = lane.end_x - lane.start_x;
        let len = ((nx * nx + ny * ny) as f32).sqrt();
        if len < 0.01 { continue; }
        let nx = (nx as f32 / len * scale * 0.2) as i32;
        let ny = (ny as f32 / len * scale * 0.2) as i32;

        let edge_color = COLOR_LANE_LINE & 0x00_FF_FF_FF | 0x18_00_00_00; // aún más sutil
        draw_line(fb, w, h, sx1 + nx, sy1 + ny, sx2 + nx, sy2 + ny, edge_color);
        draw_line(fb, w, h, sx1 - nx, sy1 - ny, sx2 - nx, sy2 - ny, edge_color);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. ENTIDADES ARQUITECTÓNICAS
// ═══════════════════════════════════════════════════════════════════════════

fn render_entities_architectural(
    gw: &GameWorld,
    fb: &mut [u32],
    w: usize,
    h: usize,
    ox: f32,
    oy: f32,
    scale: f32,
) {
    // Edificios (con ConstructionState para saber el tipo)
    for (_entity, (pos, renderable, cs)) in gw.world.query::<(&Position, &Renderable, &ConstructionState)>().iter() {
        let cx = (pos.x * scale + ox) as i32;
    // Vehículos
    for (_entity, (pos, car)) in gw.world.query::<(&Position, &TrafficCar)>().iter() {
        let cx = (pos.x * scale + ox) as i32;
        let cy = (pos.y * scale + oy) as i32;
        let car_size = (scale * 0.35) as i32;

        if cx < -car_size || cx > w as i32 + car_size || cy < -car_size || cy > h as i32 + car_size {
            continue;
        }

        // Orientación basada en speed/lane_id (simplificado)
        let angle = (car.lane_id as f32 * 1.7).sin(); // pseudo-dirección
        draw_car(fb, w, h, cx, cy, car_size.max(2), angle);
    }
}
    }

    // Entidades sin ConstructionState (fallback: usar color)
    for (_entity, (pos, renderable)) in gw.world.query::<(&Position, &Renderable)>().iter() {
        let has_cs = gw.world.query_one::<&ConstructionState>(_entity).is_ok();
        if has_cs { continue; } // Ya dibujado arriba

        let cx = (pos.x * scale + ox) as i32;
        let cy = (pos.y * scale + oy) as i32;
        let size = (renderable.size_x * scale) as i32;

        if cx + size < 0 || cx - size > w as i32 || cy + size < 0 || cy - size > h as i32 {
            continue;
        }

        // Fallback: dibujar según color
        let btype = guess_building_type_from_color(renderable.color);
        draw_building(fb, w, h, cx, cy, size, btype, 1.0);
    }

    // Vehículos
    for (_entity, (pos, car)) in gw.world.query::<(&Position, &TrafficCar)>().iter() {
        let cx = (pos.x * scale + ox) as i32;
        let cy = (pos.y * scale + oy) as i32;
        let car_size = (scale * 0.35) as i32;

        if cx < -car_size || cx > w as i32 + car_size || cy < -car_size || cy > h as i32 + car_size {
            continue;
        }

        draw_car(fb, w, h, cx, cy, car_size.max(2), car.direction_x, car.direction_y);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// DIBUJO DE EDIFICIOS POR TIPO
// ═══════════════════════════════════════════════════════════════════════════

fn draw_building(fb: &mut [u32], fb_w: usize, fb_h: usize,
                 cx: i32, cy: i32, size: i32, btype: BuildingType, progress: f32) {
    let s = (size as f32 * progress) as i32;
    if s < 2 { return; }

    match btype {
        BuildingType::House    => draw_house(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Apartment=> draw_apartment(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Shop     => draw_shop(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Office   => draw_office(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Factory  => draw_factory(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Farm     => draw_farm(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Hospital => draw_hospital(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::School   => draw_school(fb, fb_w, fb_h, cx, cy, s),
        BuildingType::Police   => draw_police(fb, fb_w, fb_h, cx, cy, s),
    }
}

// ---- CASA: rectángulo + tejado triangular + puerta + ventanas ----

fn draw_house(fb: &mut [u32], fb_w: usize, fb_h: usize,
              cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 3 / 4;
    let roof_h = size / 3;

    // Paredes
    fill_rect(fb, fb_w, fb_h, cx - hw, cy - hw + roof_h, size, body_h, COLOR_HOUSE);
    // Borde
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - hw + roof_h, size, body_h, darken(COLOR_HOUSE, 30));

    // Tejado triangular
    let roof_color = 0xFF_8B_45_3A; // rojo tejado
    let apex_y = cy - hw;
    for row in 0..roof_h {
        let y = apex_y + row;
        let row_w = (row * size / roof_h.max(1)) as i32;
        let x0 = cx - row_w / 2;
        if row_w > 0 {
            fill_rect(fb, fb_w, fb_h, x0, y, row_w, 1, roof_color);
        }
    }

    // Puerta
    let door_w = size / 5;
    let door_h = body_h * 2 / 3;
    fill_rect(fb, fb_w, fb_h, cx - door_w / 2, cy + hw - door_h, door_w, door_h, darken(COLOR_HOUSE, 60));

    // Ventanas
    let win_size = size / 6;
    if win_size > 1 {
        fill_rect(fb, fb_w, fb_h, cx - hw + size/5, cy - hw + roof_h + size/5,
                  win_size, win_size, 0xFF_E8_D8_8C); // amarillo cálido
        fill_rect(fb, fb_w, fb_h, cx + hw - size/5 - win_size, cy - hw + roof_h + size/5,
                  win_size, win_size, 0xFF_E8_D8_8C);
    }
}

// ---- APARTAMENTO: torre alta con ventanas en cuadrícula ----

fn draw_apartment(fb: &mut [u32], fb_w: usize, fb_h: usize,
                  cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 5 / 4; // más alto que ancho

    // Paredes
    fill_rect(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, COLOR_APARTMENT);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, darken(COLOR_APARTMENT, 40));

    // Ventanas en cuadrícula
    let win_s = size / 7;
    if win_s > 1 {
        let cols = 3;
        let rows = 5;
        let start_x = cx - hw + size / 6;
        let start_y = cy - body_h + hw + size / 8;
        for r in 0..rows {
            for c in 0..cols {
                let wx = start_x + c * (size / 3);
                let wy = start_y + r * (body_h / 6);
                let lit = (r + c) % 3 != 0; // algunas ventanas iluminadas
                let wc = if lit { 0xFF_E8_D8_8C } else { 0xFF_3A_3A_4A };
                fill_rect(fb, fb_w, fb_h, wx, wy, win_s, win_s, wc);
            }
        }
    }
}

// ---- TIENDA: base ancha + toldo + escaparate ----

fn draw_shop(fb: &mut [u32], fb_w: usize, fb_h: usize,
             cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 2 / 3;

    // Paredes
    fill_rect(fb, fb_w, fb_h, cx - hw, cy - hw + hw/2, size, body_h, COLOR_SHOP);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - hw + hw/2, size, body_h, darken(COLOR_SHOP, 30));

    // Toldo (franja de color)
    let awning_color = 0xFF_E8_5C_3A; // naranja
    fill_rect(fb, fb_w, fb_h, cx - hw, cy - hw, size, size/4, awning_color);
    // Rayas del toldo
    for i in 0..4 {
        let sx = cx - hw + i * size / 4;
        fill_rect(fb, fb_w, fb_h, sx, cy - hw, 2, size/4, 0xFF_FF_FF_CC);
    }

    // Escaparate
    fill_rect(fb, fb_w, fb_h, cx - hw + 3, cy - hw + size/3, size - 6, body_h/2, 0xFF_CC_DD_EE);
}

// ---- OFICINA: torre con líneas horizontales ----

fn draw_office(fb: &mut [u32], fb_w: usize, fb_h: usize,
               cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 5 / 4;

    fill_rect(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, COLOR_OFFICE);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, darken(COLOR_OFFICE, 40));

    // Líneas horizontales (pisos)
    for floor in 0..6 {
        let fy = cy - body_h + hw + floor * body_h / 6;
        fill_rect(fb, fb_w, fb_h, cx - hw + 2, fy, size - 4, 1, darken(COLOR_OFFICE, 20));
    }

    // Ventanas reflectantes
    let win_s = size / 6;
    if win_s > 1 {
        for r in 0..5 {
            for c in 0..3 {
                let wx = cx - hw + 3 + c * (size / 3);
                let wy = cy - body_h + hw + 3 + r * (body_h / 6);
                fill_rect(fb, fb_w, fb_h, wx, wy, win_s, win_s/2, 0xFF_BB_CC_DD);
            }
        }
    }
}

// ---- FÁBRICA: nave industrial + chimeneas ----

fn draw_factory(fb: &mut [u32], fb_w: usize, fb_h: usize,
                cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 2 / 3;

    // Nave principal
    fill_rect(fb, fb_w, fb_h, cx - hw, cy - hw + size/4, size, body_h, COLOR_FACTORY);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - hw + size/4, size, body_h, darken(COLOR_FACTORY, 40));

    // Techo en diente de sierra (sawtooth)
    for i in 0..3 {
        let sx = cx - hw + i * size / 3;
        let ex = sx + size / 6;
        let roof_y = cy - hw + size / 4;
        fill_rect(fb, fb_w, fb_h, sx, roof_y - 1, size / 6, 1, darken(COLOR_FACTORY, 50));
        fill_rect(fb, fb_w, fb_h, ex, roof_y - size / 8, 2, size / 8, darken(COLOR_FACTORY, 30));
    }

    // Chimeneas
    let chimney_w = size / 8;
    for i in 0..2 {
        let ch_x = cx - hw + size / 3 + i * size / 3;
        fill_rect(fb, fb_w, fb_h, ch_x, cy - hw - size/4, chimney_w, size/3, 0xFF_6A_5A_4E);
        // Humo (puntitos grises)
        for j in 0..3 {
            let sx = ch_x + chimney_w/2 + (j as i32 - 1) * 2;
            let sy = cy - hw - size/4 - 2 - j * 3;
            if sx >= 0 && sy >= 0 {
                unsafe {
                    let idx = sy as usize * fb_w + sx as usize;
                    if idx < fb.len() { *fb.get_unchecked_mut(idx) = blend_argb(*fb.get_unchecked(idx), 0x66_AA_AA_AA); }
                }
            }
        }
    }
}

// ---- GRANJA: granero + silo ----

fn draw_farm(fb: &mut [u32], fb_w: usize, fb_h: usize,
             cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 3 / 4;

    // Granero
    fill_rect(fb, fb_w, fb_h, cx - hw + size/6, cy - hw + size/4, size * 2/3, body_h, COLOR_FARM);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw + size/6, cy - hw + size/4, size * 2/3, body_h, darken(COLOR_FARM, 40));

    // Tejado granero
    let roof_color = 0xFF_7A_4A_3A;
    let roof_h = size / 4;
    for row in 0..roof_h {
        let y = cy - hw + size/4 - roof_h + row;
        let row_w = (row * (size * 2/3) / roof_h.max(1)) as i32;
        let x0 = cx - row_w / 2;
        if row_w > 0 {
            fill_rect(fb, fb_w, fb_h, x0, y, row_w, 1, roof_color);
        }
    }

    // Silo
    let silo_x = cx + hw - size / 4;
    let silo_w = size / 5;
    let silo_h = size * 3 / 4;
    fill_rect(fb, fb_w, fb_h, silo_x, cy - silo_h + hw - size/8, silo_w, silo_h, 0xFF_AA_AA_B0);
    draw_rect_outline(fb, fb_w, fb_h, silo_x, cy - silo_h + hw - size/8, silo_w, silo_h, 0xFF_88_88_90);
    // Cúpula del silo
    fill_rect(fb, fb_w, fb_h, silo_x - 1, cy - silo_h + hw - size/8 - 2, silo_w + 2, 3, 0xFF_88_88_90);
}

// ---- HOSPITAL: edificio blanco + cruz roja ----

fn draw_hospital(fb: &mut [u32], fb_w: usize, fb_h: usize,
                 cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size;

    // Edificio principal
    fill_rect(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, COLOR_HOSPITAL);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, 0xFF_C0_C0_C8);

    // Ala izquierda
    fill_rect(fb, fb_w, fb_h, cx - hw - size/4, cy - body_h/2 + hw, size/4, body_h/2, COLOR_HOSPITAL);

    // Cruz roja
    let cross_w = size / 5;
    let cross_h = size / 3;
    let cross_cx = cx;
    let cross_cy = cy - body_h/2 + hw;
    fill_rect(fb, fb_w, fb_h, cross_cx - cross_w/2, cross_cy - cross_h/2, cross_w, cross_h, 0xFF_CC_33_33);
    fill_rect(fb, fb_w, fb_h, cross_cx - cross_h/2, cross_cy - cross_w/2, cross_h, cross_w, 0xFF_CC_33_33);

    // Ventanas
    let win_s = size / 7;
    if win_s > 1 {
        for r in 0..3 {
            for c in 0..2 {
                let wx = cx - hw + size/5 + c * size * 3/5;
                let wy = cy - body_h + hw + size/5 + r * body_h/4;
                fill_rect(fb, fb_w, fb_h, wx, wy, win_s, win_s, 0xFF_BB_DD_EE);
            }
        }
    }
}

// ---- ESCUELA: edificio amarillo + campana ----

fn draw_school(fb: &mut [u32], fb_w: usize, fb_h: usize,
               cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 4 / 5;

    fill_rect(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, COLOR_SCHOOL);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, darken(COLOR_SCHOOL, 40));

    // Tejado plano con cornisa
    fill_rect(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, 3, darken(COLOR_SCHOOL, 20));

    // Entrada
    fill_rect(fb, fb_w, fb_h, cx - size/6, cy + hw - body_h/3, size/3, body_h/3, darken(COLOR_SCHOOL, 30));

    // Ventanas
    let win_s = size / 6;
    if win_s > 1 {
        for c in 0..4 {
            let wx = cx - hw + size/8 + c * size/4;
            fill_rect(fb, fb_w, fb_h, wx, cy - body_h + hw + size/5, win_s, win_s, 0xFF_E8_F0_FF);
        }
    }
}

// ---- POLICÍA: edificio azul + estrella ----

fn draw_police(fb: &mut [u32], fb_w: usize, fb_h: usize,
               cx: i32, cy: i32, size: i32) {
    let hw = size / 2;
    let body_h = size * 4 / 5;

    fill_rect(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, COLOR_POLICE);
    draw_rect_outline(fb, fb_w, fb_h, cx - hw, cy - body_h + hw, size, body_h, darken(COLOR_POLICE, 40));

    // Barra de luz roja/azul en el techo
    fill_rect(fb, fb_w, fb_h, cx - hw/3, cy - body_h + hw, hw*2/3, 2, 0xFF_CC_3333);
    fill_rect(fb, fb_w, fb_h, cx, cy - body_h + hw, hw/3, 2, 0xFF_3333_CC);

    // Insignia/estrella simple
    let badge_y = cy - body_h/2 + hw;
    fill_rect(fb, fb_w, fb_h, cx - 3, badge_y - 3, 6, 6, 0xFF_FF_D700);

    // Ventanas
    let win_s = size / 6;
    if win_s > 1 {
        for c in 0..2 {
            let wx = cx - hw + size/4 + c * size/3;
            fill_rect(fb, fb_w, fb_h, wx, cy - body_h + hw + size/4, win_s, win_s, 0xFF_E8_F0_FF);
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// VEHÍCULOS
// ═══════════════════════════════════════════════════════════════════════════

fn draw_car(fb: &mut [u32], fb_w: usize, fb_h: usize,
            cx: i32, cy: i32, size: i32, dir_x: f32, dir_y: f32) {
    let hw = size;
    let hh = size * 2 / 3;

    // Determinar orientación
    let horizontal = dir_x.abs() > dir_y.abs();

    let (car_w, car_h) = if horizontal { (hw * 2, hh) } else { (hh, hw * 2) };

    // Carrocería
    fill_rect(fb, fb_w, fb_h, cx - car_w/2, cy - car_h/2, car_w, car_h, COLOR_CAR);
    draw_rect_outline(fb, fb_w, fb_h, cx - car_w/2, cy - car_h/2, car_w, car_h, 0xFF_88_88_88);

    // Cabina (más oscura)
    if horizontal {
        fill_rect(fb, fb_w, fb_h, cx - car_w/4, cy - car_h/2, car_w/2, car_h, COLOR_CAR_ALT);
    } else {
        fill_rect(fb, fb_w, fb_h, cx - car_w/2, cy - car_h/4, car_w, car_h/2, COLOR_CAR_ALT);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// UI
// ═══════════════════════════════════════════════════════════════════════════

fn render_ui(gw: &GameWorld, fb: &mut [u32], w: usize, h: usize) {
    let x = 10;
    let mut y = 10;

    let pop = gw.world.query::<&ConstructionState>().iter().count();
    let cars = gw.world.query::<&TrafficCar>().iter().count();
    let zones = gw.world.query::<&ZoneComponent>().iter().count();

    draw_text(fb, w, h, x, y, &format!("Pop: {} | Cars: {} | Zones: {}", pop, cars, zones), COLOR_UI_TEXT);
    y += 12;
    draw_text(fb, w, h, x, y, &format!("Tick: {} | Time: {:02}:{:02}",
        gw.sim_tick, gw.time_of_day / 60, gw.time_of_day % 60), COLOR_UI_TEXT);
}

pub fn render_stats_panel(gw: &GameWorld, fb: &mut [u32], w: usize, h: usize, fps: u32) {
    let x = 10;
    let y = h as i32 - 30;
    let pop = gw.world.query::<&ConstructionState>().iter().count();
    draw_text(fb, w, h, x, y, &format!("FPS: {} | Pop: {} | Tick: {}", fps, pop, gw.sim_tick), COLOR_UI_TEXT);
}

// ═══════════════════════════════════════════════════════════════════════════
// PRIMITIVAS DE DIBUJO
// ═══════════════════════════════════════════════════════════════════════════

fn fill_rect(fb: &mut [u32], fb_w: usize, fb_h: usize,
             x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x + rw).min(fb_w as i32);
    let y2 = (y + rh).min(fb_h as i32);
    if x1 >= x2 || y1 >= y2 { return; }
    for py in y1..y2 {
        unsafe {
            let row_start = py as usize * fb_w;
            for px in x1..x2 {
                *fb.get_unchecked_mut(row_start + px as usize) = color;
            }
        }
    }
}

fn draw_rect_outline(fb: &mut [u32], fb_w: usize, fb_h: usize,
                     x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x + rw).min(fb_w as i32);
    let y2 = (y + rh).min(fb_h as i32);
    if x1 >= x2 || y1 >= y2 { return; }

    // Top
    if y >= 0 && y < fb_h as i32 {
        for px in x1..x2 {
            unsafe { *fb.get_unchecked_mut(y as usize * fb_w + px as usize) = color; }
        }
    }
    // Bottom
    if y + rh >= 0 && y + rh < fb_h as i32 {
        for px in x1..x2 {
            unsafe { *fb.get_unchecked_mut((y + rh - 1) as usize * fb_w + px as usize) = color; }
        }
    }
    // Left
    if x >= 0 && x < fb_w as i32 {
        for py in y1..y2 {
            unsafe { *fb.get_unchecked_mut(py as usize * fb_w + x as usize) = color; }
        }
    }
    // Right
    if x + rw >= 0 && x + rw < fb_w as i32 {
        for py in y1..y2 {
            unsafe { *fb.get_unchecked_mut(py as usize * fb_w + (x + rw - 1) as usize) = color; }
        }
    }
}

fn draw_line(fb: &mut [u32], fb_w: usize, fb_h: usize,
             x0: i32, y0: i32, x1: i32, y1: i32, color: u32) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;

    loop {
        if x >= 0 && x < fb_w as i32 && y >= 0 && y < fb_h as i32 {
            unsafe { blend_pixel(fb, fb_w, x, y, color); }
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
        *fb.get_unchecked_mut(idx) = blend_argb(*fb.get_unchecked(idx), color);
    }
}

#[inline(always)]
fn blend_argb(bg: u32, fg: u32) -> u32 {
    let fa = (fg >> 24) & 0xFF;
    if fa == 0 { return bg; }
    if fa == 255 { return fg; }

    let fb_r = (fg >> 16) & 0xFF;
    let fg_g = (fg >> 8) & 0xFF;
    let fg_b = fg & 0xFF;

    let bg_r = (bg >> 16) & 0xFF;
    let bg_g = (bg >> 8) & 0xFF;
    let bg_b = bg & 0xFF;

    let out_r = ((fb_r * fa + bg_r * (255 - fa)) / 255) as u32;
    let out_g = ((fg_g * fa + bg_g * (255 - fa)) / 255) as u32;
    let out_b = ((fg_b * fa + bg_b * (255 - fa)) / 255) as u32;

    0xFF_00_00_00 | (out_r << 16) | (out_g << 8) | out_b
}

#[inline(always)]
fn darken(color: u32, amount: u32) -> u32 {
    let r = ((color >> 16) & 0xFF).saturating_sub(amount);
    let g = ((color >> 8) & 0xFF).saturating_sub(amount);
    let b = (color & 0xFF).saturating_sub(amount);
    (color & 0xFF_00_00_00) | (r << 16) | (g << 8) | b
}

fn guess_building_type_from_color(color: u32) -> BuildingType {
    let r = (color >> 16) & 0xFF;
    let g = (color >> 8) & 0xFF;
    let b = color & 0xFF;

    // Mapeo rápido por tono dominante
    if g > r && g > b && r < 180 { return BuildingType::Farm; }       // verde
    if b > r && b > g + 20 { return BuildingType::Police; }           // azul
    if r > 200 && g > 180 && b < 150 { return BuildingType::School; } // amarillo
    if r > 200 && g < 100 && b < 100 { return BuildingType::Hospital; }// rojo (cruz)
    if r > 180 && g < 130 && b < 100 { return BuildingType::House; }  // terracota
    if r > 160 && g > 160 && b > 180 { return BuildingType::Office; } // gris claro
    if r > 140 && g > 120 && b < 100 { return BuildingType::Factory; }// marrón
    if b > 150 && g > 140 && r < 130 { return BuildingType::Shop; }   // azul verdoso

    BuildingType::House
}

// ═══════════════════════════════════════════════════════════════════════════
// TEXTO (bitmap font 5x7)
// ═══════════════════════════════════════════════════════════════════════════

fn draw_text(fb: &mut [u32], fb_w: usize, _fb_h: usize,
             x: i32, y: i32, text: &str, color: u32) {
    let mut cx = x;
    for ch in text.chars() {
        draw_char(fb, fb_w, cx, y, ch, color);
        cx += 6;
        if cx > fb_w as i32 { break; }
    }
}

fn draw_char(fb: &mut [u32], fb_w: usize, x: i32, y: i32, ch: char, color: u32) {
    let glyph: [u8; 7] = match ch {
        'A'..='Z' => FONT_ALPHA[(ch as u8 - b'A') as usize],
        'a'..='z' => FONT_ALPHA[(ch as u8 - b'a') as usize],
        '0'..='9' => FONT_DIGITS[(ch as u8 - b'0') as usize],
        ' ' => [0; 7],
        ':' => [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
        '/' => [0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b00000, 0b00000],
        _   => [0; 7],
    };

    for row in 0..7 {
        let bits = glyph[row];
        for col in 0..5 {
            if (bits >> (4 - col)) & 1 != 0 {
                let px = x + col as i32;
                let py = y + row as i32;
                if px >= 0 && px < fb_w as i32 && py >= 0 {
                    unsafe {
                        let idx = py as usize * fb_w + px as usize;
                        if idx < fb.len() {
                            *fb.get_unchecked_mut(idx) = color;
                        }
                    }
                }
            }
        }
    }
}

const FONT_ALPHA: [[u8; 7]; 26] = [
    [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001], // A
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110], // B
    [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110], // C
    [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110], // D
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111], // E
    [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000], // F
    [0b01110, 0b10001, 0b10000, 0b10011, 0b10001, 0b10001, 0b01110], // G
    [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001], // H
    [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // I
    [0b00111, 0b00001, 0b00001, 0b00001, 0b10001, 0b10001, 0b01110], // J
    [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001], // K
    [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111], // L
    [0b10001, 0b11011, 0b10101, 0b10001, 0b10001, 0b10001, 0b10001], // M
    [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001], // N
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110], // O
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000], // P
    [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101], // Q
    [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001], // R
    [0b01110, 0b10001, 0b10000, 0b01110, 0b00001, 0b10001, 0b01110], // S
    [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100], // T
    [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110], // U
    [0b10001, 0b10001, 0b10001, 0b01010, 0b01010, 0b00100, 0b00100], // V
    [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001], // W
    [0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b01010, 0b10001], // X
    [0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100], // Y
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111], // Z
];

const FONT_DIGITS: [[u8; 7]; 10] = [
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110], // 0
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110], // 1
    [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111], // 2
    [0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110], // 3
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010], // 4
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110], // 5
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110], // 6
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000], // 7
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110], // 8
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100], // 9
];