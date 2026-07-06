// Módulo de Atlas de Texturas v0.17.0 — Fase 9: Categorización de Sprites
//
// Sistema de spritesheets con carga PNG, extracción de tiles,
// y categorización automática por color dominante para mapeo
// de entidades a sprites reales.
//
// NOVEDADES v0.17.0:
// - TileCategory enum con categorías de terreno y edificios
// - Categorización automática durante load_spritesheet
// - CategoryMap para lookup O(1) de sprites por categoría
// - Métodos get_sprite_for_* para entidades
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
// CATEGORÍAS DE TILES (para mapeo automático)
// ---------------------------------------------------------------------------

/// Categoría de un tile según su contenido visual
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum TileCategory {
    /// Hierba/vegetación verde
    Grass,
    /// Tierra/camino marrón
    Dirt,
    /// Carretera/asfalto gris
    Road,
    /// Arena/playa beige
    Sand,
    /// Agua azul
    Water,
    /// Edificio (con sub-estilo)
    Building(BuildingTileStyle),
    /// Vehículo (coche, camión)
    Vehicle,
    /// Decoración (árbol, farola, banco, etc.)
    Decoration,
    /// Personaje/peatón
    Character,
    /// Desconocido (se usa como fallback)
    Unknown,
}

/// Sub-estilo de edificio para mapear a tipos de BuildingType
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum BuildingTileStyle {
    /// Casa residencial (cálida, pequeña)
    House,
    /// Tienda/comercio (escaparate)
    Shop,
    /// Fábrica/industrial (oscura, grande)
    Factory,
    /// Apartamento (gris, alto)
    Apartment,
    /// Oficina (gris, ventanas)
    Office,
    /// Granja (rural, cálida)
    Farm,
    /// Hospital (grande, blanco/rojo)
    Hospital,
    /// Escuela (media, amarillo/marrón)
    School,
    /// Comisaría (media, azul)
    Police,
    /// Edificio genérico
    Generic,
}

// ---------------------------------------------------------------------------
// TILE Y ATLAS (mejorados con categorización)
// ---------------------------------------------------------------------------

/// Un tile individual como array plano de píxeles ARGB
#[derive(Clone)]
pub struct SpriteTile {
    pub pixels: Vec<u32>,
    pub width: u32,
    pub height: u32,
    /// Categoría asignada durante la carga
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

/// Atlas de texturas con categorización automática
pub struct TextureAtlas {
    /// Todos los tiles de todas las spritesheets, indexados secuencialmente
    pub tiles: Vec<SpriteTile>,
    /// Rango [start, end) de tiles para cada spritesheet
    pub banks: Vec<TileBank>,
    /// Índice por defecto para entidades no mapeadas
    pub fallback_idx: usize,
    /// Mapa de categorías para lookup rápido
    pub categories: CategoryMap,
}

/// Banco de tiles de un spritesheet específico
#[derive(Clone)]
pub struct TileBank {
    pub name: String,
    pub start_idx: usize,
    pub end_idx: usize,
    pub tile_w: u32,
    pub tile_h: u32,
}

/// Mapa de categorías: cada categoría tiene un Vec de índices
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

    /// Obtiene un sprite aleatorio para una categoría de terreno
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

    /// Obtiene un sprite para un estilo de edificio
    pub fn building_sprite(&self, style: BuildingTileStyle) -> usize {
        self.buildings.get(&style)
            .and_then(|v| if v.is_empty() { None } else { Some(v[0]) })
            .or_else(|| self.buildings.get(&BuildingTileStyle::Generic)
                .and_then(|v| if v.is_empty() { None } else { Some(v[0]) }))
            .unwrap_or(0)
    }

    /// Obtiene sprite de vehículo aleatorio
    pub fn random_vehicle(&self, rng: &mut impl FnMut() -> usize) -> usize {
        if self.vehicles.is_empty() { 0 } else { self.vehicles[rng() % self.vehicles.len()] }
    }
}

/// Tipos de terreno para mapeo
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
    pub fn new() -> Self {
        let fallback = {
            let mut pixels = vec![0u32; 256];
            for i in 0..256 {
                let x = i % 16;
                let y = i / 16;
                // Gris neutro suave con patrón sutil (no magenta)
                pixels[i] = if (x + y) % 2 == 0 { 0xFF_4A_4A_4A } else { 0xFF_3A_3A_3A };
            }
            SpriteTile { pixels, width: 16, height: 16, category: TileCategory::Unknown }
        };
    }

    /// Carga un spritesheet PNG, extrae tiles y los categoriza automáticamente.
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

                            // Detectar bordes (píxeles en el perímetro)
                            if px == 0 || px == tile_w - 1 || py == 0 || py == tile_h - 1 {
                                edge_pixels += 1;
                                // Bordes oscuros sugieren edificio
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

                // Determinar categoría
                let category = categorize_tile(
                    avg_r, avg_g, avg_b,
                    avg_brightness,
                    pixel_count,
                    edge_pixels,
                    corner_dark,
                    tile_w,
                    row,
                    col,
                );

                self.tiles.push(SpriteTile {
                    pixels: tile_pixels,
                    width: tile_w,
                    height: tile_h,
                    category,
                });

                // Registrar en el mapa de categorías
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
    // Blit de sprites
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

    /// Imprime estadísticas de categorización
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

/// Analiza el color promedio y características de un tile para categorizarlo
fn categorize_tile(
    avg_r: f32, avg_g: f32, avg_b: f32,
    avg_brightness: f32,
    pixel_count: u32,
    edge_pixels: u32,
    corner_dark: u32,
    tile_size: u32,
    grid_row: u32,
    _grid_col: u32,
) -> TileCategory {
    let fill_ratio = pixel_count as f32 / (tile_size * tile_size) as f32;

    // Tiles casi vacíos
    if fill_ratio < 0.15 || pixel_count < 10 {
        return TileCategory::Unknown;
    }

    // Púrpura/magenta = probablemente personajes o UI
    if avg_r > 150.0 && avg_b > 150.0 && avg_g < 100.0 && (avg_r + avg_b) > 2.5 * avg_g {
        return TileCategory::Character;
    }

    // Verde dominante = hierba/vegetación
    if avg_g > avg_r + 15.0 && avg_g > avg_b + 10.0 && avg_g > 70.0 {
        // Si es muy oscuro y compacto, podría ser un arbusto/árbol
        if fill_ratio < 0.5 && avg_brightness < 80.0 {
            return TileCategory::Decoration;
        }
        return TileCategory::Grass;
    }

    // Azul dominante = agua
    if avg_b > avg_r + 20.0 && avg_b > avg_g + 15.0 && avg_b > 90.0 {
        if fill_ratio > 0.6 {
            return TileCategory::Water;
        }
        // Azul con bordes podría ser edificio azul (policía, hospital)
        if edge_pixels > 10 && corner_dark > 2 {
            return TileCategory::Building(BuildingTileStyle::Police);
        }
    }

    // Rojo/marrón cálido con alto fill = edificio de ladrillo
    if avg_r > avg_g + 10.0 && avg_r > avg_b + 15.0 && avg_r > 120.0 {
        if fill_ratio > 0.4 && edge_pixels > 8 && corner_dark > 1 {
            // Edificio cálido - determinar sub-estilo
            if avg_brightness < 100.0 {
                return TileCategory::Building(BuildingTileStyle::Factory);
            }
            if avg_g > 90.0 {
                return TileCategory::Building(BuildingTileStyle::Farm);
            }
            return TileCategory::Building(BuildingTileStyle::House);
        }
        // Marrón tierra
        if avg_brightness < 140.0 && avg_r < 180.0 {
            return TileCategory::Dirt;
        }
        return TileCategory::Building(BuildingTileStyle::Shop);
    }

    // Gris/neutro = carretera o edificio de oficinas
    if (avg_r - avg_g).abs() < 20.0 && (avg_g - avg_b).abs() < 20.0 {
        if avg_brightness > 170.0 {
            return TileCategory::Sand; // beige claro = arena
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
        // Gris medio con alto fill = carretera
        if fill_ratio > 0.5 && avg_brightness > 100.0 && avg_brightness < 175.0 {
            return TileCategory::Road;
        }
        // Gris oscuro compacto = vehículo
        if fill_ratio < 0.5 && avg_brightness < 90.0 && pixel_count < 100 {
            return TileCategory::Vehicle;
        }
        return TileCategory::Road;
    }

    // Amarillo/marrón claro = edificio escolar
    if avg_r > 170.0 && avg_g > 160.0 && avg_b < 130.0 && edge_pixels > 5 {
        return TileCategory::Building(BuildingTileStyle::School);
    }

    // Blanco/gris claro con bordes = hospital
    if avg_brightness > 170.0 && edge_pixels > 8 && corner_dark > 1 {
        if (avg_r - avg_b).abs() < 30.0 {
            return TileCategory::Building(BuildingTileStyle::Hospital);
        }
    }

    // Por posición en grilla: primeras filas = terreno, filas medias = edificios
    if grid_row < 4 {
        // Filas superiores = terreno
        if avg_g > 90.0 && avg_g > avg_b { return TileCategory::Grass; }
        if avg_r > 140.0 && avg_g < 130.0 { return TileCategory::Dirt; }
        if avg_brightness > 130.0 && avg_brightness < 170.0 { return TileCategory::Road; }
        if avg_brightness > 170.0 { return TileCategory::Sand; }
        return TileCategory::Dirt;
    }

    if grid_row >= 4 && grid_row <= 14 {
        // Filas de edificios
        if fill_ratio > 0.3 && edge_pixels > 4 {
            return TileCategory::Building(BuildingTileStyle::Generic);
        }
        // Decoraciones (árboles pequeños, arbustos)
        if fill_ratio < 0.4 && avg_g > 80.0 {
            return TileCategory::Decoration;
        }
    }

    // Vehículos: compactos, no llenan el tile
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
// CARGA DE PNG (usando crate `png`)
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

pub fn generate_grass_tile(variant: u32) -> SpriteTile {
    let size = 16u32;
    let mut pixels = vec![0u32; (size * size) as usize];
    let base_r = 45 + (variant % 15) as u32;
    let base_g = 90 + (variant % 20) as u32;
    let base_b = 40 + (variant % 10) as u32;
    for y in 0u32..size {
        for x in 0u32..size {
            let noise = ((x.wrapping_mul(7) ^ y.wrapping_mul(13)) % 20) as u32;
            let r = base_r + noise / 2;
            let g = base_g + noise;
            let b = base_b + noise / 3;
            pixels[(y * size + x) as usize] =
                0xFF_00_00_00 | (r << 16) | (g << 8) | b;
        }
    }
    SpriteTile { pixels, width: size, height: size, category: TileCategory::Grass }
}

pub fn generate_water_tile(frame: u32) -> SpriteTile {
    let size = 16;
    let mut pixels = vec![0u32; (size * size) as usize];
    for y in 0..size {
        for x in 0..size {
            let wave = ((x as u32).wrapping_mul(3 + frame) ^ (y as u32).wrapping_mul(5 + frame)) % 8;
            let r = 20 + wave;
            let g = 40 + wave * 3;
            let b = 100 + wave * 5;
            pixels[(y * size + x) as usize] =
                0xFF_00_00_00 | (r << 16) | (g << 8) | b;
        }
    }
    SpriteTile { pixels, width: size, height: size, category: TileCategory::Water }
}

pub fn generate_road_tile() -> SpriteTile {
    let size = 16u32;
    let mut pixels = vec![0u32; (size * size) as usize];
    for y in 0u32..size {
        for x in 0u32..size {
            let v = 80 + ((x.wrapping_mul(3) ^ y.wrapping_mul(7)) % 10) as u32;
            pixels[(y * size + x) as usize] =
                0xFF_00_00_00 | (v << 16) | (v << 8) | v;
        }
    }
    SpriteTile { pixels, width: size, height: size, category: TileCategory::Road }
}

pub fn generate_building_tile(color: u32, height_px: u32) -> SpriteTile {
    let size = 16;
    let mut pixels = vec![0u32; (size * size) as usize];
    let roof_y = size - height_px;
    let r = (color >> 16) & 0xFF;
    let g = (color >> 8) & 0xFF;
    let b = color & 0xFF;

    for y in 0..size {
        for x in 0..size {
            let pixel = if y >= roof_y {
                let shade = if x == 0 || x == size - 1 || y == roof_y {
                    70
                } else {
                    if (x / 4 + y / 4) % 2 == 0 && x > 1 && x < size - 2 && y > roof_y + 1 && y < size - 2 {
                        255
                    } else {
                        100
                    }
                };
                let sr = (r * shade / 255).min(255);
                let sg = (g * shade / 255).min(255);
                let sb = (b * shade / 255).min(255);
                0xFF_00_00_00 | (sr << 16) | (sg << 8) | sb
            } else {
                let sr = (r * 60 / 255).min(255);
                let sg = (g * 60 / 255).min(255);
                let sb = (b * 60 / 255).min(255);
                0xFF_00_00_00 | (sr << 16) | (sg << 8) | sb
            };
            pixels[(y * size + x) as usize] = pixel;
        }
    }
    SpriteTile { pixels, width: size, height: size, category: TileCategory::Building(BuildingTileStyle::Generic) }
}
