// Módulo de Atlas de Texturas v0.19.0 — Sprites Reales
//
// Sistema de spritesheets con carga PNG, extracción de tiles,
// categorización automática, blit con alpha blending,
// y carga de texturas completas (ground textures).
//
// TÉCNICAS:
// [TC#5]  Look-Up Tables: tiles pre-extraídos indexados O(1)
// [TC#17] Culling viewport en blit
// [TC#21] Distancias² para clipping
// [TI#28] Acceso unchecked tras validación inicial

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

// ---------------------------------------------------------------------------
// CATEGORÍAS DE TILES
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TileCategory {
    Grass,
    Dirt,
    Road,
    Sand,
    Water,
    Building(BuildingTileStyle),
    Vehicle,
    Decoration,
    Character,
    Unknown,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BuildingTileStyle {
    House,
    Shop,
    Factory,
    Apartment,
    Office,
    Farm,
    Hospital,
    School,
    Police,
    Generic,
}

// ---------------------------------------------------------------------------
// TILE Y ATLAS
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct SpriteTile {
    pub pixels: Vec<u32>,
    pub width: u32,
    pub height: u32,
    pub category: TileCategory,
}

impl SpriteTile {
    pub fn empty(w: u32, h: u32) -> Self {
        Self {
            pixels: vec![0x00_00_00_00; (w * h) as usize],
            width: w,
            height: h,
            category: TileCategory::Unknown,
        }
    }
}

pub struct TextureAtlas {
    pub tiles: Vec<SpriteTile>,
    pub banks: Vec<TileBank>,
    pub fallback_idx: usize,
    pub categories: CategoryMap,
}

#[derive(Clone)]
pub struct TileBank {
    pub name: String,
    pub start_idx: usize,
    pub end_idx: usize,
    pub tile_w: u32,
    pub tile_h: u32,
}

#[derive(Clone)]
pub struct CategoryMap {
    pub grass: Vec<usize>,
    pub dirt: Vec<usize>,
    pub road: Vec<usize>,
    pub sand: Vec<usize>,
    pub water: Vec<usize>,
    pub buildings: HashMap<BuildingTileStyle, Vec<usize>>,
    pub vehicles: Vec<usize>,
    pub decorations: Vec<usize>,
    pub characters: Vec<usize>,
}

impl CategoryMap {
    pub fn new() -> Self {
        let mut buildings = HashMap::new();
        buildings.insert(BuildingTileStyle::House, Vec::new());
        buildings.insert(BuildingTileStyle::Shop, Vec::new());
        buildings.insert(BuildingTileStyle::Factory, Vec::new());
        buildings.insert(BuildingTileStyle::Apartment, Vec::new());
        buildings.insert(BuildingTileStyle::Office, Vec::new());
        buildings.insert(BuildingTileStyle::Farm, Vec::new());
        buildings.insert(BuildingTileStyle::Hospital, Vec::new());
        buildings.insert(BuildingTileStyle::School, Vec::new());
        buildings.insert(BuildingTileStyle::Police, Vec::new());
        buildings.insert(BuildingTileStyle::Generic, Vec::new());

        Self {
            grass: Vec::new(),
            dirt: Vec::new(),
            road: Vec::new(),
            sand: Vec::new(),
            water: Vec::new(),
            buildings,
            vehicles: Vec::new(),
            decorations: Vec::new(),
            characters: Vec::new(),
        }
    }

    pub fn random_terrain(&self, base: TerrainTileType, rng: &mut impl FnMut() -> usize) -> usize {
        let list = match base {
            TerrainTileType::Grass => &self.grass,
            TerrainTileType::Dirt => &self.dirt,
            TerrainTileType::Road => &self.road,
            TerrainTileType::Sand => &self.sand,
            TerrainTileType::Water => &self.water,
        };
        if list.is_empty() { 0 } else { list[rng() % list.len()] }
    }

    pub fn building_sprite(&self, style: BuildingTileStyle) -> usize {
        self.buildings.get(&style)
            .and_then(|v| if v.is_empty() { None } else { Some(v[0]) })
            .or_else(|| self.buildings.get(&BuildingTileStyle::Generic)
                .and_then(|v| if v.is_empty() { None } else { Some(v[0]) }))
            .unwrap_or(0)
    }

    pub fn random_vehicle(&self, rng: &mut impl FnMut() -> usize) -> usize {
        if self.vehicles.is_empty() { 0 } else { self.vehicles[rng() % self.vehicles.len()] }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TerrainTileType {
    Grass,
    Dirt,
    Road,
    Sand,
    Water,
}

impl TextureAtlas {
    pub fn new() -> Self {
        let fallback = {
            let mut pixels = vec![0u32; 256];
            for i in 0..256 {
                let x = i % 16;
                let y = i / 16;
                pixels[i] = if (x + y) % 2 == 0 { 0xFF_4A_4A_4A } else { 0xFF_3A_3A_3A };
            }
            SpriteTile { pixels, width: 16, height: 16, category: TileCategory::Unknown }
        };

        Self {
            tiles: vec![fallback],
            banks: Vec::new(),
            fallback_idx: 0,
            categories: CategoryMap::new(),
        }
    }

    pub fn load_spritesheet(
        &mut self,
        png_path: &Path,
        bank_name: &str,
        tile_w: u32,
        tile_h: u32,
        margin: u32,
    ) -> Result<(usize, usize), String> {
        let (img_w, img_h, pixels) = load_png(png_path)?;

        let stride = tile_w + margin;
        let tiles_per_row = (img_w + margin) / stride;
        let tiles_per_col = (img_h + margin) / stride;

        let start_idx = self.tiles.len();
        let mut count = 0usize;

        for row in 0..tiles_per_col {
            for col in 0..tiles_per_row {
                let src_x = col * stride;
                let src_y = row * stride;

                if src_x + tile_w > img_w || src_y + tile_h > img_h {
                    continue;
                }

                let mut tile_pixels = vec![0u32; (tile_w * tile_h) as usize];
                let mut has_content = false;
                let mut sum_r: u32 = 0;
                let mut sum_g: u32 = 0;
                let mut sum_b: u32 = 0;
                let mut pixel_count: u32 = 0;
                let mut edge_pixels: u32 = 0;
                let mut corner_dark: u32 = 0;

                for py in 0..tile_h {
                    for px in 0..tile_w {
                        let src_idx = ((src_y + py) * img_w + (src_x + px)) as usize;
                        let pixel = pixels[src_idx];
                        let alpha = (pixel >> 24) & 0xFF;

                        if alpha > 30 && pixel & 0x00_FF_FF_FF != 0x00_FF_00_FF {
                            has_content = true;
                            let r = (pixel >> 16) & 0xFF;
                            let g = (pixel >> 8) & 0xFF;
                            let b = pixel & 0xFF;
                            sum_r += r;
                            sum_g += g;
                            sum_b += b;
                            pixel_count += 1;

                            if px == 0 || px == tile_w - 1 || py == 0 || py == tile_h - 1 {
                                edge_pixels += 1;
                                if (r + g + b) < 120 {
                                    corner_dark += 1;
                                }
                            }
                        }
                        tile_pixels[(py * tile_w + px) as usize] = pixel;
                    }
                }

                if !has_content {
                    continue;
                }

                let avg_r = (sum_r / pixel_count.max(1)) as f32;
                let avg_g = (sum_g / pixel_count.max(1)) as f32;
                let avg_b = (sum_b / pixel_count.max(1)) as f32;
                let avg_brightness = (avg_r + avg_g + avg_b) / 3.0;

                let category = categorize_tile(
                    avg_r, avg_g, avg_b,
                    avg_brightness,
                    pixel_count,
                    edge_pixels,
                    corner_dark,
                    tile_w,
                    row,
                );

                self.tiles.push(SpriteTile {
                    pixels: tile_pixels,
                    width: tile_w,
                    height: tile_h,
                    category,
                });

                let tile_idx = self.tiles.len() - 1;
                match category {
                    TileCategory::Grass => self.categories.grass.push(tile_idx),
                    TileCategory::Dirt => self.categories.dirt.push(tile_idx),
                    TileCategory::Road => self.categories.road.push(tile_idx),
                    TileCategory::Sand => self.categories.sand.push(tile_idx),
                    TileCategory::Water => self.categories.water.push(tile_idx),
                    TileCategory::Building(style) => {
                        self.categories.buildings
                            .entry(style)
                            .or_insert_with(Vec::new)
                            .push(tile_idx);
                    }
                    TileCategory::Vehicle => self.categories.vehicles.push(tile_idx),
                    TileCategory::Decoration => self.categories.decorations.push(tile_idx),
                    TileCategory::Character => self.categories.characters.push(tile_idx),
                    TileCategory::Unknown => {}
                }

                count += 1;
            }
        }

        let end_idx = self.tiles.len();
        self.banks.push(TileBank {
            name: bank_name.to_string(),
            start_idx,
            end_idx,
            tile_w,
            tile_h,
        });

        Ok((start_idx, count))
    }

    // -----------------------------------------------------------------------
    // Blit de sprites (con alpha blending)
    // -----------------------------------------------------------------------

    #[inline(always)]
    pub fn blit_sprite(
        &self,
        sprite_idx: usize,
        fb: &mut [u32],
        fb_w: usize,
        fb_h: usize,
        cx: i32,
        cy: i32,
        scale: f32,
    ) {
        if sprite_idx >= self.tiles.len() {
            return;
        }

        let tile = &self.tiles[sprite_idx];
        let tw = tile.width as i32;
        let th = tile.height as i32;
        let draw_w = (tw as f32 * scale) as i32;
        let draw_h = (th as f32 * scale) as i32;

        let x0 = cx - draw_w / 2;
        let y0 = cy - draw_h / 2;
        let x1 = x0 + draw_w;
        let y1 = y0 + draw_h;

        if x1 <= 0 || x0 >= fb_w as i32 || y1 <= 0 || y0 >= fb_h as i32 {
            return;
        }

        let clip_x0 = x0.max(0);
        let clip_y0 = y0.max(0);
        let clip_x1 = x1.min(fb_w as i32);
        let clip_y1 = y1.min(fb_h as i32);

        if clip_x0 >= clip_x1 || clip_y0 >= clip_y1 {
            return;
        }

        let step_x = tw as f32 / draw_w as f32;
        let step_y = th as f32 / draw_h as f32;

        for py in clip_y0..clip_y1 {
            let ty = ((py - y0) as f32 * step_y) as i32;
            if ty < 0 || ty >= th { continue; }
            let row_start = (py as usize) * fb_w;
            let tile_row = (ty as usize) * tw as usize;

            for px in clip_x0..clip_x1 {
                let tx = ((px - x0) as f32 * step_x) as i32;
                if tx < 0 || tx >= tw { continue; }

                let src = tile.pixels[tile_row + tx as usize];
                let sa = ((src >> 24) & 0xFF) as u32;

                if sa == 0 { continue; }

                if sa == 255 {
                    unsafe {
                        *fb.get_unchecked_mut(row_start + px as usize) = src;
                    }
                } else {
                    let dst = unsafe { *fb.get_unchecked(row_start + px as usize) };
                    let sr = (src >> 16) & 0xFF;
                    let sg = (src >> 8) & 0xFF;
                    let sb = src & 0xFF;
                    let dr = (dst >> 16) & 0xFF;
                    let dg = (dst >> 8) & 0xFF;
                    let db = dst & 0xFF;

                    let inv_a = 255 - sa;
                    let r = ((sr * sa + dr * inv_a) / 255) & 0xFF;
                    let g = ((sg * sa + dg * inv_a) / 255) & 0xFF;
                    let b = ((sb * sa + db * inv_a) / 255) & 0xFF;

                    unsafe {
                        *fb.get_unchecked_mut(row_start + px as usize) =
                            0xFF_00_00_00 | (r << 16) | (g << 8) | b;
                    }
                }
            }
        }
    }

    #[inline(always)]
    pub fn blit_sprite_opaque(
        &self,
        sprite_idx: usize,
        fb: &mut [u32],
        fb_w: usize,
        fb_h: usize,
        x: i32,
        y: i32,
        scale: f32,
    ) {
        if sprite_idx >= self.tiles.len() {
            return;
        }

        let tile = &self.tiles[sprite_idx];
        let tw = tile.width as i32;
        let th = tile.height as i32;
        let draw_w = (tw as f32 * scale) as i32;
        let draw_h = (th as f32 * scale) as i32;

        let x1 = x + draw_w;
        let y1 = y + draw_h;

        if x1 <= 0 || x >= fb_w as i32 || y1 <= 0 || y >= fb_h as i32 {
            return;
        }

        let clip_x0 = x.max(0);
        let clip_y0 = y.max(0);
        let clip_x1 = x1.min(fb_w as i32);
        let clip_y1 = y1.min(fb_h as i32);

        if clip_x0 >= clip_x1 || clip_y0 >= clip_y1 {
            return;
        }

        let step_x = tw as f32 / draw_w as f32;
        let step_y = th as f32 / draw_h as f32;

        for py in clip_y0..clip_y1 {
            let ty = ((py - y) as f32 * step_y) as i32;
            if ty < 0 || ty >= th { continue; }
            let row_start = (py as usize) * fb_w;
            let tile_row = (ty as usize) * tw as usize;

            for px in clip_x0..clip_x1 {
                let tx = ((px - x) as f32 * step_x) as i32;
                if tx < 0 || tx >= tw { continue; }
                unsafe {
                    *fb.get_unchecked_mut(row_start + px as usize) =
                        tile.pixels[tile_row + tx as usize];
                }
            }
        }
    }

    /// Blit de tile repetido (para rellenar una región con un patrón)
    pub fn blit_tile_pattern(
        &self,
        sprite_idx: usize,
        fb: &mut [u32],
        fb_w: usize,
        fb_h: usize,
        dst_x: i32,
        dst_y: i32,
        dst_w: i32,
        dst_h: i32,
    ) {
        if sprite_idx >= self.tiles.len() { return; }

        let tile = &self.tiles[sprite_idx];
        let tw = tile.width as i32;
        let th = tile.height as i32;

        let x1 = (dst_x + dst_w).min(fb_w as i32);
        let y1 = (dst_y + dst_h).min(fb_h as i32);
        let x0 = dst_x.max(0);
        let y0 = dst_y.max(0);

        for py in y0..y1 {
            let row_start = (py as usize) * fb_w;
            let ty = ((py - dst_y).rem_euclid(th)) as usize;
            let tile_row = ty * tw as usize;

            for px in x0..x1 {
                let tx = ((px - dst_x).rem_euclid(tw)) as usize;
                let src = tile.pixels[tile_row + tx];
                let sa = ((src >> 24) & 0xFF) as u32;

                if sa == 0 { continue; }
                if sa == 255 {
                    unsafe {
                        *fb.get_unchecked_mut(row_start + px as usize) = src;
                    }
                } else {
                    let dst_px = unsafe { *fb.get_unchecked(row_start + px as usize) };
                    let sr = (src >> 16) & 0xFF;
                    let sg = (src >> 8) & 0xFF;
                    let sb = src & 0xFF;
                    let dr = (dst_px >> 16) & 0xFF;
                    let dg = (dst_px >> 8) & 0xFF;
                    let db = dst_px & 0xFF;
                    let inv_a = 255 - sa;
                    let r = ((sr * sa + dr * inv_a) / 255) & 0xFF;
                    let g = ((sg * sa + dg * inv_a) / 255) & 0xFF;
                    let b = ((sb * sa + db * inv_a) / 255) & 0xFF;
                    unsafe {
                        *fb.get_unchecked_mut(row_start + px as usize) =
                            0xFF_00_00_00 | (r << 16) | (g << 8) | b;
                    }
                }
            }
        }
    }

    /// Carga una textura completa (no un spritesheet) como un solo tile.
    /// Útil para texturas de terreno de fondo.
    pub fn load_full_texture(&mut self, png_path: &Path, name: &str) -> Result<usize, String> {
        let (img_w, img_h, pixels) = load_png(png_path)?;

        let tile = SpriteTile {
            pixels,
            width: img_w,
            height: img_h,
            category: TileCategory::Grass,
        };

        let idx = self.tiles.len();
        self.tiles.push(tile);
        self.categories.grass.push(idx);

        self.banks.push(TileBank {
            name: name.to_string(),
            start_idx: idx,
            end_idx: idx + 1,
            tile_w: img_w,
            tile_h: img_h,
        });

        Ok(idx)
    }

    pub fn get_tile(&self, idx: usize) -> &SpriteTile {
        if idx < self.tiles.len() {
            &self.tiles[idx]
        } else {
            &self.tiles[self.fallback_idx]
        }
    }

    pub fn len(&self) -> usize { self.tiles.len() }

    pub fn bank_start(&self, name: &str) -> Option<usize> {
        self.banks.iter().find(|b| b.name == name).map(|b| b.start_idx)
    }

    pub fn print_stats(&self) {
        println!("[ATLAS] {} tiles totales en {} banks", self.tiles.len(), self.banks.len());
        println!("  Grass:     {}", self.categories.grass.len());
        println!("  Dirt:      {}", self.categories.dirt.len());
        println!("  Road:      {}", self.categories.road.len());
        println!("  Sand:      {}", self.categories.sand.len());
        println!("  Water:     {}", self.categories.water.len());
        println!("  Buildings: {} total", self.categories.buildings.values().map(|v| v.len()).sum::<usize>());
        for (style, list) in &self.categories.buildings {
            if !list.is_empty() {
                println!("    {:?}: {}", style, list.len());
            }
        }
        println!("  Vehicles:  {}", self.categories.vehicles.len());
        println!("  Decorations: {}", self.categories.decorations.len());
    }
}

// ---------------------------------------------------------------------------
// CATEGORIZACIÓN AUTOMÁTICA POR COLOR
// ---------------------------------------------------------------------------

fn categorize_tile(
    avg_r: f32, avg_g: f32, avg_b: f32,
    avg_brightness: f32,
    pixel_count: u32,
    edge_pixels: u32,
    corner_dark: u32,
    tile_size: u32,
    grid_row: u32,
) -> TileCategory {
    let fill_ratio = pixel_count as f32 / (tile_size * tile_size) as f32;

    if fill_ratio < 0.15 || pixel_count < 10 {
        return TileCategory::Unknown;
    }

    if avg_r > 150.0 && avg_b > 150.0 && avg_g < 100.0 && (avg_r + avg_b) > 2.5 * avg_g {
        return TileCategory::Character;
    }

    if avg_g > avg_r + 15.0 && avg_g > avg_b + 10.0 && avg_g > 70.0 {
        if fill_ratio < 0.5 && avg_brightness < 80.0 {
            return TileCategory::Decoration;
        }
        return TileCategory::Grass;
    }

    if avg_b > avg_r + 20.0 && avg_b > avg_g + 15.0 && avg_b > 90.0 {
        if fill_ratio > 0.6 {
            return TileCategory::Water;
        }
        if edge_pixels > 10 && corner_dark > 2 {
            return TileCategory::Building(BuildingTileStyle::Police);
        }
    }

    if avg_r > avg_g + 10.0 && avg_r > avg_b + 15.0 && avg_r > 120.0 {
        if fill_ratio > 0.4 && edge_pixels > 8 && corner_dark > 1 {
            if avg_brightness < 100.0 {
                return TileCategory::Building(BuildingTileStyle::Factory);
            }
            if avg_g > 90.0 {
                return TileCategory::Building(BuildingTileStyle::Farm);
            }
            return TileCategory::Building(BuildingTileStyle::House);
        }
        if avg_brightness < 140.0 && avg_r < 180.0 {
            return TileCategory::Dirt;
        }
        return TileCategory::Building(BuildingTileStyle::Shop);
    }

    if (avg_r - avg_g).abs() < 20.0 && (avg_g - avg_b).abs() < 20.0 {
        if avg_brightness > 170.0 {
            return TileCategory::Sand;
        }
        if avg_brightness < 110.0 && edge_pixels > 5 {
            return TileCategory::Building(BuildingTileStyle::Apartment);
        }
        if edge_pixels > 8 && corner_dark > 2 {
            if avg_brightness > 130.0 {
                return TileCategory::Building(BuildingTileStyle::Office);
            }
            return TileCategory::Building(BuildingTileStyle::Generic);
        }
        if fill_ratio > 0.5 && avg_brightness > 100.0 && avg_brightness < 175.0 {
            return TileCategory::Road;
        }
        if fill_ratio < 0.5 && avg_brightness < 90.0 && pixel_count < 100 {
            return TileCategory::Vehicle;
        }
        return TileCategory::Road;
    }

    if avg_r > 170.0 && avg_g > 160.0 && avg_b < 130.0 && edge_pixels > 5 {
        return TileCategory::Building(BuildingTileStyle::School);
    }

    if avg_brightness > 170.0 && edge_pixels > 8 && corner_dark > 1 {
        if (avg_r - avg_b).abs() < 30.0 {
            return TileCategory::Building(BuildingTileStyle::Hospital);
        }
    }

    if grid_row < 4 {
        if avg_g > 90.0 && avg_g > avg_b { return TileCategory::Grass; }
        if avg_r > 140.0 && avg_g < 130.0 { return TileCategory::Dirt; }
        if avg_brightness > 130.0 && avg_brightness < 170.0 { return TileCategory::Road; }
        if avg_brightness > 170.0 { return TileCategory::Sand; }
        return TileCategory::Dirt;
    }

    if grid_row >= 4 && grid_row <= 14 {
        if fill_ratio > 0.3 && edge_pixels > 4 {
            return TileCategory::Building(BuildingTileStyle::Generic);
        }
        if fill_ratio < 0.4 && avg_g > 80.0 {
            return TileCategory::Decoration;
        }
    }

    if fill_ratio < 0.45 && pixel_count > 20 && pixel_count < 120 {
        if avg_brightness < 140.0 {
            return TileCategory::Vehicle;
        }
        if avg_g > 90.0 && avg_brightness < 120.0 {
            return TileCategory::Decoration;
        }
    }

    TileCategory::Unknown
}

// ---------------------------------------------------------------------------
// CARGA DE PNG
// ---------------------------------------------------------------------------

fn load_png(path: &Path) -> Result<(u32, u32, Vec<u32>), String> {
    let mut file = File::open(path)
        .map_err(|e| format!("No se pudo abrir {}: {}", path.display(), e))?;

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("Error leyendo {}: {}", path.display(), e))?;

    let decoder = png::Decoder::new(&bytes[..]);
    let mut decoder = decoder;
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("Error decodificando PNG {}: {}", path.display(), e))?;

    let info = reader.info();
    let (width, height) = (info.width, info.height);
    let color_type = info.color_type;

    let output_buffer_size = reader.output_buffer_size();
    let mut buf = vec![0u8; output_buffer_size];
    let frame_info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("Error leyendo frame {}: {}", path.display(), e))?;

    let actual_bytes_per_pixel = match color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        png::ColorType::Indexed => {
            let bpp = frame_info.buffer_size() as u32 / (width * height);
            if bpp >= 4 { 4 } else { 3 }
        }
    };

    let pixel_count = (width * height) as usize;
    let mut pixels = vec![0u32; pixel_count];
    let row_bytes = frame_info.buffer_size() / height as usize;

    for y in 0..height as usize {
        let row_start = y * row_bytes;
        for x in 0..width as usize {
            let idx = y * width as usize + x;
            match actual_bytes_per_pixel {
                4 => {
                    let r = buf[row_start + x * 4] as u32;
                    let g = buf[row_start + x * 4 + 1] as u32;
                    let b = buf[row_start + x * 4 + 2] as u32;
                    let a = buf[row_start + x * 4 + 3] as u32;
                    pixels[idx] = (a << 24) | (r << 16) | (g << 8) | b;
                }
                3 => {
                    let r = buf[row_start + x * 3] as u32;
                    let g = buf[row_start + x * 3 + 1] as u32;
                    let b = buf[row_start + x * 3 + 2] as u32;
                    pixels[idx] = 0xFF_00_00_00 | (r << 16) | (g << 8) | b;
                }
                2 => {
                    let g = buf[row_start + x * 2] as u32;
                    let a = buf[row_start + x * 2 + 1] as u32;
                    pixels[idx] = (a << 24) | (g << 16) | (g << 8) | g;
                }
                1 => {
                    let g = buf[row_start + x] as u32;
                    pixels[idx] = 0xFF_00_00_00 | (g << 16) | (g << 8) | g;
                }
                _ => {
                    if row_start + x * 4 + 3 < buf.len() {
                        let r = buf[row_start + x * 4] as u32;
                        let g = buf[row_start + x * 4 + 1] as u32;
                        let b = buf[row_start + x * 4 + 2] as u32;
                        let a = buf[row_start + x * 4 + 3] as u32;
                        pixels[idx] = (a << 24) | (r << 16) | (g << 8) | b;
                    } else {
                        pixels[idx] = 0xFF_FF_00_FF;
                    }
                }
            }
        }
    }

    Ok((width, height, pixels))
}

// ---------------------------------------------------------------------------
// GENERACIÓN PROCEDURAL DE TEXTURAS (FALLBACK)
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// GENERACIÓN PROCEDURAL DE TEXTURAS v0.20 — TILES GRANDES (64×64)
// ---------------------------------------------------------------------------
// Edificios con ventanas, techos, puertas, sombras, y detalles arquitectónicos.
// Terreno con textura de pasto detallada.
// Carreteras con marcas de carril.
// Vehículos con forma de coche y ventanas.
// TODOS los tiles son 64×64 para máxima visibilidad.

use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;

const TILE_SIZE: u32 = 64;

fn hash_xy(x: u32, y: u32, seed: u64) -> u32 {
    let mut h = DefaultHasher::new();
    x.hash(&mut h);
    y.hash(&mut h);
    seed.hash(&mut h);
    h.finish() as u32
}

fn lerp_u32(a: u32, b: u32, t: f32) -> u32 {
    ((a as f32) + ((b as f32) - (a as f32)) * t) as u32
}

// ═══════════════════════════════════════════════════════════
// TERRENO: HIERBA
// ═══════════════════════════════════════════════════════════

pub fn generate_grass_tile(variant: u32) -> SpriteTile {
    let mut pixels = vec![0u32; (TILE_SIZE * TILE_SIZE) as usize];
    
    // Colores base de pasto: varios tonos de verde
    let grass_colors: [(u32, u32, u32); 5] = [
        (55, 130, 45),   // verde oscuro
        (65, 140, 50),   // verde medio
        (75, 150, 55),   // verde claro
        (50, 120, 40),   // verde sombra
        (85, 155, 60),   // verde brillante
    ];
    
    for y in 0u32..TILE_SIZE {
        for x in 0u32..TILE_SIZE {
            let h = hash_xy(x + variant * 1000, y, 42);
            let noise = (h % 100) as f32 / 100.0;
            
            // Elegir color base con ruido
            let ci = ((h >> 4) % 5) as usize;
            let (br, bg, bb) = grass_colors[ci];
            
            // Micro-variación por posición
            let r = (br as f32 + (noise - 0.5) * 20.0).clamp(0.0, 255.0) as u32;
            let g = (bg as f32 + (noise - 0.5) * 25.0).clamp(0.0, 255.0) as u32;
            let b = (bb as f32 + (noise - 0.5) * 15.0).clamp(0.0, 255.0) as u32;
            
            // Algunas briznas de hierba (líneas verticales sutiles)
            let blade = if (x + y / 3) % 13 == 0 && noise > 0.6 {
                0.15
            } else {
                0.0
            };
            
            let sr = (r as f32 * (1.0 + blade)).min(255.0) as u32;
            let sg = (g as f32 * (1.0 + blade * 1.5)).min(255.0) as u32;
            let sb = (b as f32 * (1.0 + blade)).min(255.0) as u32;
            
            pixels[(y * TILE_SIZE + x) as usize] = 
                0xFF_00_00_00 | (sr << 16) | (sg << 8) | sb;
        }
    }
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Grass }
}

// ═══════════════════════════════════════════════════════════
// TERRENO: TIERRA
// ═══════════════════════════════════════════════════════════

pub fn generate_dirt_tile(variant: u32) -> SpriteTile {
    let mut pixels = vec![0u32; (TILE_SIZE * TILE_SIZE) as usize];
    
    for y in 0u32..TILE_SIZE {
        for x in 0u32..TILE_SIZE {
            let h = hash_xy(x + variant * 2000, y, 99);
            let noise = (h % 100) as f32 / 100.0;
            let r = (155.0 + (noise - 0.5) * 30.0).clamp(0.0, 255.0) as u32;
            let g = (130.0 + (noise - 0.5) * 25.0).clamp(0.0, 255.0) as u32;
            let b = (100.0 + (noise - 0.5) * 20.0).clamp(0.0, 255.0) as u32;
            pixels[(y * TILE_SIZE + x) as usize] = 
                0xFF_00_00_00 | (r << 16) | (g << 8) | b;
        }
    }
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Dirt }
}

// ═══════════════════════════════════════════════════════════
// TERRENO: ARENA
// ═══════════════════════════════════════════════════════════

pub fn generate_sand_tile(variant: u32) -> SpriteTile {
    let mut pixels = vec![0u32; (TILE_SIZE * TILE_SIZE) as usize];
    
    for y in 0u32..TILE_SIZE {
        for x in 0u32..TILE_SIZE {
            let h = hash_xy(x + variant * 3000, y, 55);
            let noise = (h % 100) as f32 / 100.0;
            let r = (210.0 + (noise - 0.5) * 20.0).clamp(0.0, 255.0) as u32;
            let g = (195.0 + (noise - 0.5) * 18.0).clamp(0.0, 255.0) as u32;
            let b = (150.0 + (noise - 0.5) * 15.0).clamp(0.0, 255.0) as u32;
            pixels[(y * TILE_SIZE + x) as usize] = 
                0xFF_00_00_00 | (r << 16) | (g << 8) | b;
        }
    }
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Sand }
}

// ═══════════════════════════════════════════════════════════
// CARRETERA
// ═══════════════════════════════════════════════════════════

pub fn generate_road_tile() -> SpriteTile {
    let mut pixels = vec![0u32; (TILE_SIZE * TILE_SIZE) as usize];
    let mid = TILE_SIZE / 2;
    
    for y in 0u32..TILE_SIZE {
        for x in 0u32..TILE_SIZE {
            let h = hash_xy(x, y, 77);
            let noise = (h % 40) as f32 / 40.0;
            let v = (95.0 + (noise - 0.5) * 15.0).clamp(0.0, 255.0) as u32;
            
            // Línea central discontinua
            let is_center_line = x >= mid - 1 && x <= mid + 1 && (y / 8) % 2 == 0;
            
            let (r, g, b) = if is_center_line {
                (220, 220, 100) // línea amarilla
            } else {
                (v, v, v) // asfalto gris
            };
            
            pixels[(y * TILE_SIZE + x) as usize] = 
                0xFF_00_00_00 | (r << 16) | (g << 8) | b;
        }
    }
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Road }
}

// ═══════════════════════════════════════════════════════════
// AGUA (ANIMADA)
// ═══════════════════════════════════════════════════════════

pub fn generate_water_tile(frame: u32) -> SpriteTile {
    let mut pixels = vec![0u32; (TILE_SIZE * TILE_SIZE) as usize];
    
    for y in 0u32..TILE_SIZE {
        for x in 0u32..TILE_SIZE {
            let wave1 = ((x as f32 + frame as f32 * 2.0) * 0.15).sin() * 8.0;
            let wave2 = ((y as f32 + frame as f32 * 1.5) * 0.12).cos() * 8.0;
            let wave = (wave1 + wave2) as u32;
            
            let r = (25u32 + wave).min(35);
            let g = (60u32 + wave * 2).min(80);
            let b = (130u32 + wave * 3).min(180);
            
            pixels[(y * TILE_SIZE + x) as usize] = 
                0xFF_00_00_00 | (r << 16) | (g << 8) | b;
        }
    }
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Water }
}

// ═══════════════════════════════════════════════════════════
// EDIFICIOS — ARQUITECTURA DETALLADA
// ═══════════════════════════════════════════════════════════

fn fill_rect_tile(pixels: &mut [u32], tw: u32, x0: u32, y0: u32, w: u32, h: u32, color: u32) {
    for y in y0..(y0+h).min(tw) {
        for x in x0..(x0+w).min(tw) {
            if y < tw && x < tw {
                pixels[(y * tw + x) as usize] = color;
            }
        }
    }
}

fn darken_color(color: u32, amount: u32) -> u32 {
    let r = (((color >> 16) & 0xFF).saturating_sub(amount)) as u32;
    let g = (((color >> 8) & 0xFF).saturating_sub(amount)) as u32;
    let b = ((color & 0xFF).saturating_sub(amount)) as u32;
    0xFF_00_00_00 | (r << 16) | (g << 8) | b
}

fn lighten_color(color: u32, amount: u32) -> u32 {
    let r = (((color >> 16) & 0xFF).saturating_add(amount)).min(255) as u32;
    let g = (((color >> 8) & 0xFF).saturating_add(amount)).min(255) as u32;
    let b = ((color & 0xFF).saturating_add(amount)).min(255) as u32;
    0xFF_00_00_00 | (r << 16) | (g << 8) | b
}

/// Dibuja un edificio genérico con cuerpo, techo, ventanas y puerta
fn draw_building_tile(pixels: &mut [u32], body_color: u32, roof_color: u32, 
                       window_color: u32, door_color: u32, style: BuildingTileStyle) {
    let s = TILE_SIZE;
    // Margen para que no toque los bordes
    let margin = 6u32;
    let bw = s - margin * 2;       // ancho del edificio
    let body_top = s / 5;           // donde empieza el cuerpo (debajo del techo)
    let roof_height = s / 6;        // altura del techo
    let body_bottom = s - margin;   // base
    
    // Sombra del edificio (lado derecho + abajo)
    let shadow_color = 0x44_00_00_00u32;
    fill_rect_tile(pixels, s, margin + 4, body_top + 4, bw, body_bottom - body_top, shadow_color);
    
    // Cuerpo del edificio
    fill_rect_tile(pixels, s, margin, body_top, bw, body_bottom - body_top, body_color);
    
    // Borde del cuerpo
    let border = darken_color(body_color, 40);
    // Línea superior
    fill_rect_tile(pixels, s, margin, body_top, bw, 2, border);
    // Línea inferior
    fill_rect_tile(pixels, s, margin, body_bottom - 2, bw, 2, border);
    // Líneas laterales
    fill_rect_tile(pixels, s, margin, body_top, 2, body_bottom - body_top, border);
    fill_rect_tile(pixels, s, s - margin - 2, body_top, 2, body_bottom - body_top, border);
    
    // Techo
    let roof_width = bw + 8;
    let roof_x = margin - 4;
    fill_rect_tile(pixels, s, roof_x, 0, roof_width, roof_height, roof_color);
    // Alero del techo (sombra debajo)
    fill_rect_tile(pixels, s, roof_x, roof_height, roof_width, 2, darken_color(roof_color, 50));
    // Borde del techo
    fill_rect_tile(pixels, s, roof_x, 0, roof_width, 2, lighten_color(roof_color, 30));
    
    // Ventanas (grid)
    let win_w = 8u32;
    let win_h = 10u32;
    let win_spacing_x = 14u32;
    let win_spacing_y = 16u32;
    let win_start_x = margin + 6;
    let win_start_y = body_top + 6;
    
    let cols = ((bw - 12) / win_spacing_x).min(4);
    let rows = ((body_bottom - body_top - 12) / win_spacing_y).min(3);
    
    for row in 0..rows {
        for col in 0..cols {
            let wx = win_start_x + col * win_spacing_x;
            let wy = win_start_y + row * win_spacing_y;
            // Marco de ventana
            fill_rect_tile(pixels, s, wx, wy, win_w, win_h, darken_color(body_color, 60));
            // Vidrio
            fill_rect_tile(pixels, s, wx + 1, wy + 1, win_w - 2, win_h - 2, window_color);
            // Reflejo en el vidrio
            fill_rect_tile(pixels, s, wx + 1, wy + 1, 3, 3, lighten_color(window_color, 60));
        }
    }
    
    // Puerta
    let door_w = 10u32;
    let door_h = 16u32;
    let door_x = margin + (bw - door_w) / 2;
    let door_y = body_bottom - door_h;
    fill_rect_tile(pixels, s, door_x, door_y, door_w, door_h, door_color);
    // Marco de puerta
    fill_rect_tile(pixels, s, door_x - 1, door_y - 1, door_w + 2, door_h + 2, darken_color(door_color, 40));
    fill_rect_tile(pixels, s, door_x, door_y, door_w, door_h, door_color);
    // Picaporte
    fill_rect_tile(pixels, s, door_x + door_w - 3, door_y + door_h / 2, 2, 2, 0xFF_FF_D7_00);
}

pub fn generate_building_tile(color: u32, _height_px: u32) -> SpriteTile {
    let mut pixels = vec![0x00_00_00_00u32; (TILE_SIZE * TILE_SIZE) as usize];
    
    let roof_color = darken_color(color, 80);
    let window_color = 0xFF_88_CC_FF; // azul cielo para ventanas
    let door_color = darken_color(color, 40);
    
    draw_building_tile(&mut pixels, color, roof_color, window_color, door_color, BuildingTileStyle::Generic);
    
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Building(BuildingTileStyle::Generic) }
}

/// Casa residencial: cálida, techo a dos aguas, jardín
pub fn generate_house_tile() -> SpriteTile {
    let mut pixels = vec![0x00_00_00_00u32; (TILE_SIZE * TILE_SIZE) as usize];
    let body = 0xFF_E8_C8_A8;      // beige cálido
    let roof = 0xFF_8B_45_3A;      // teja roja
    let windows = 0xFF_AA_DD_FF;   // azul claro
    let door = 0xFF_6B_3A_2A;      // marrón puerta
    
    draw_building_tile(&mut pixels, body, roof, windows, door, BuildingTileStyle::House);
    
    // Jardín en la base
    let s = TILE_SIZE;
    let grass_color = 0xFF_55_AA_45;
    fill_rect_tile(&mut pixels, s, 0, s - 8, s, 8, grass_color);
    // Arbustos
    fill_rect_tile(&mut pixels, s, 8, s - 8, 10, 6, 0xFF_3A_7A_2A);
    fill_rect_tile(&mut pixels, s, s - 18, s - 8, 10, 6, 0xFF_3A_7A_2A);
    
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Building(BuildingTileStyle::House) }
}

/// Apartamento: gris, más alto, muchas ventanas
pub fn generate_apartment_tile() -> SpriteTile {
    let mut pixels = vec![0x00_00_00_00u32; (TILE_SIZE * TILE_SIZE) as usize];
    let body = 0xFF_C0_C0_C8;
    let roof = 0xFF_60_60_68;
    let windows = 0xFF_CC_EE_FF;
    let door = 0xFF_50_50_58;
    
    draw_building_tile(&mut pixels, body, roof, windows, door, BuildingTileStyle::Apartment);
    
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Building(BuildingTileStyle::Apartment) }
}

/// Tienda: colorida, escaparates grandes
pub fn generate_shop_tile() -> SpriteTile {
    let mut pixels = vec![0x00_00_00_00u32; (TILE_SIZE * TILE_SIZE) as usize];
    let body = 0xFF_5C_B8_A0;
    let roof = 0xFF_3A_8A_70;
    let windows = 0xFF_FF_FF_CC;
    let door = 0xFF_3A_6A_58;
    
    draw_building_tile(&mut pixels, body, roof, windows, door, BuildingTileStyle::Shop);
    
    // Cartel en el techo
    let s = TILE_SIZE;
    fill_rect_tile(&mut pixels, s, s/3, 0, s/3, 5, 0xFF_FF_44_44);
    
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Building(BuildingTileStyle::Shop) }
}

/// Oficina: azul grisáceo, corporativa
pub fn generate_office_tile() -> SpriteTile {
    let mut pixels = vec![0x00_00_00_00u32; (TILE_SIZE * TILE_SIZE) as usize];
    let body = 0xFF_8A_A0_B8;
    let roof = 0xFF_5A_70_88;
    let windows = 0xFF_DD_EE_FF;
    let door = 0xFF_4A_60_78;
    
    draw_building_tile(&mut pixels, body, roof, windows, door, BuildingTileStyle::Office);
    
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Building(BuildingTileStyle::Office) }
}

/// Fábrica: marrón industrial, chimeneas
pub fn generate_factory_tile() -> SpriteTile {
    let mut pixels = vec![0x00_00_00_00u32; (TILE_SIZE * TILE_SIZE) as usize];
    let body = 0xFF_8A_7A_6E;
    let roof = 0xFF_5A_4A_3E;
    let windows = 0xFF_CC_CC_88;
    let door = 0xFF_4A_3A_2E;
    
    draw_building_tile(&mut pixels, body, roof, windows, door, BuildingTileStyle::Factory);
    
    // Chimenea
    let s = TILE_SIZE;
    fill_rect_tile(&mut pixels, s, s - 16, 4, 8, 16, 0xFF_6A_5A_5E);
    // Humo
    fill_rect_tile(&mut pixels, s, s - 14, 2, 4, 4, 0x88_CC_CC_CC);
    
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Building(BuildingTileStyle::Factory) }
}

/// Granja: verde, techo rojo de granero
pub fn generate_farm_tile() -> SpriteTile {
    let mut pixels = vec![0x00_00_00_00u32; (TILE_SIZE * TILE_SIZE) as usize];
    let body = 0xFF_8C_A8_6A;
    let roof = 0xFF_AA_44_33;
    let windows = 0xFF_EE_FF_CC;
    let door = 0xFF_5A_4A_2A;
    
    draw_building_tile(&mut pixels, body, roof, windows, door, BuildingTileStyle::Farm);
    
    // Granero pequeño al lado
    let s = TILE_SIZE;
    fill_rect_tile(&mut pixels, s, 2, s/2, 14, s/2 - 4, 0xFF_8A_6A_4A);
    fill_rect_tile(&mut pixels, s, 0, s/2 - 4, 18, 6, 0xFF_AA_44_33);
    
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Building(BuildingTileStyle::Farm) }
}

/// Hospital: blanco, cruz roja
pub fn generate_hospital_tile() -> SpriteTile {
    let mut pixels = vec![0x00_00_00_00u32; (TILE_SIZE * TILE_SIZE) as usize];
    let body = 0xFF_F0_F0_F8;
    let roof = 0xFF_D0_D0_D8;
    let windows = 0xFF_CC_DD_FF;
    let door = 0xFF_A0_A0_B0;
    
    draw_building_tile(&mut pixels, body, roof, windows, door, BuildingTileStyle::Hospital);
    
    // Cruz roja en el techo
    let s = TILE_SIZE;
    let cx = s/2;
    let cy = 8;
    fill_rect_tile(&mut pixels, s, cx - 2, cy - 6, 4, 12, 0xFF_FF_22_22);
    fill_rect_tile(&mut pixels, s, cx - 6, cy - 2, 12, 4, 0xFF_FF_22_22);
    
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Building(BuildingTileStyle::Hospital) }
}

/// Escuela: amarillo, patio
pub fn generate_school_tile() -> SpriteTile {
    let mut pixels = vec![0x00_00_00_00u32; (TILE_SIZE * TILE_SIZE) as usize];
    let body = 0xFF_F0_E8_A0;
    let roof = 0xFF_C0_B8_70;
    let windows = 0xFF_FF_FF_DD;
    let door = 0xFF_8A_7A_40;
    
    draw_building_tile(&mut pixels, body, roof, windows, door, BuildingTileStyle::School);
    
    // Patio de recreo
    let s = TILE_SIZE;
    fill_rect_tile(&mut pixels, s, 2, s - 6, 12, 6, 0xFF_CC_AA_88);
    
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Building(BuildingTileStyle::School) }
}

/// Policía: azul oscuro, escudo
pub fn generate_police_tile() -> SpriteTile {
    let mut pixels = vec![0x00_00_00_00u32; (TILE_SIZE * TILE_SIZE) as usize];
    let body = 0xFF_5C_70_C4;
    let roof = 0xFF_3C_50_A4;
    let windows = 0xFF_BB_CC_FF;
    let door = 0xFF_3C_40_84;
    
    draw_building_tile(&mut pixels, body, roof, windows, door, BuildingTileStyle::Police);
    
    // Estrella/escudo en el techo
    let s = TILE_SIZE;
    fill_rect_tile(&mut pixels, s, s/2 - 5, 3, 10, 6, 0xFF_FF_D7_00);
    
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Building(BuildingTileStyle::Police) }
}

// ═══════════════════════════════════════════════════════════
// VEHÍCULOS
// ═══════════════════════════════════════════════════════════

pub fn generate_vehicle_tile(color: u32) -> SpriteTile {
    let mut pixels = vec![0x00_00_00_00u32; (TILE_SIZE * TILE_SIZE) as usize];
    let s = TILE_SIZE;
    
    // Coche visto desde arriba (para city builder)
    let car_body_top = s / 3;
    let car_body_bottom = s * 2 / 3;
    let car_left = s / 5;
    let car_right = s * 4 / 5;
    let car_w = car_right - car_left;
    let car_h = car_body_bottom - car_body_top;
    
    // Sombra
    fill_rect_tile(&mut pixels, s, car_left + 2, car_body_top + 2, car_w, car_h, 0x44_00_00_00);
    
    // Carrocería
    fill_rect_tile(&mut pixels, s, car_left, car_body_top, car_w, car_h, color);
    
    // Ventanas (parabrisas)
    let glass = 0xFF_88_CC_EE;
    fill_rect_tile(&mut pixels, s, car_left + 4, car_body_top + 3, car_w - 8, car_h / 3, glass);
    fill_rect_tile(&mut pixels, s, car_left + 4, car_body_bottom - car_h / 3 - 3, car_w - 8, car_h / 3, glass);
    
    // Borde de la carrocería
    let body_dark = darken_color(color, 60);
    fill_rect_tile(&mut pixels, s, car_left, car_body_top, car_w, 1, body_dark);
    fill_rect_tile(&mut pixels, s, car_left, car_body_bottom - 1, car_w, 1, body_dark);
    fill_rect_tile(&mut pixels, s, car_left, car_body_top, 1, car_h, body_dark);
    fill_rect_tile(&mut pixels, s, car_right - 1, car_body_top, 1, car_h, body_dark);
    
    // Ruedas
    let wheel_color = 0xFF_33_33_33;
    fill_rect_tile(&mut pixels, s, car_left + 3, car_body_top - 2, 6, 4, wheel_color);
    fill_rect_tile(&mut pixels, s, car_right - 9, car_body_top - 2, 6, 4, wheel_color);
    fill_rect_tile(&mut pixels, s, car_left + 3, car_body_bottom - 2, 6, 4, wheel_color);
    fill_rect_tile(&mut pixels, s, car_right - 9, car_body_bottom - 2, 6, 4, wheel_color);
    
    SpriteTile { pixels, width: TILE_SIZE, height: TILE_SIZE, category: TileCategory::Vehicle }
}

