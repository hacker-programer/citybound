// Módulo de Atlas de Texturas v0.16.0
//
// Sistema de spritesheets con carga PNG y extracción de tiles.
// Soporta múltiples spritesheets (Roguelike Modern City, Tiny Town, Pico-8 City).
//
// TÉCNICAS:
// [TC#5]  Look-Up Tables: tiles pre-extraídos indexados O(1)
// [TC#17] Culling viewport en blit
// [TC#21] Distancias² para clipping
// [TI#28] Acceso unchecked tras validación inicial
//
// SPRITESHEETS SOPORTADOS:
// - roguelikeCity_transparent.png: 16x16 tiles, 1px margin, ~1036 sprites
// - tiny_town/tilemap_packed.png:   16x16 tiles, 1px margin, 132 sprites
// - pico8_city/tilemap_packed.png:  8x8 tiles,   1px margin, 360 sprites
// - lpc/terrain.png:               32x32 tiles (variable), terrain atlas

use std::fs::File;
use std::io::Read;
use std::path::Path;

// ---------------------------------------------------------------------------
// TEXTURE ATLAS — Almacena todos los tiles de múltiples spritesheets
// ---------------------------------------------------------------------------

/// Un tile individual como array plano de píxeles ARGB
#[derive(Clone)]
pub struct SpriteTile {
    pub pixels: Vec<u32>,    // ARGB pixels
    pub width: u32,
    pub height: u32,
}

impl SpriteTile {
    /// Tile vacío (negro transparente) como fallback
    pub fn empty(w: u32, h: u32) -> Self {
        Self {
            pixels: vec![0x00_00_00_00; (w * h) as usize],
            width: w,
            height: h,
        }
    }
}

/// Atlas de texturas que almacena todos los tiles extraídos
pub struct TextureAtlas {
    /// Todos los tiles de todas las spritesheets, indexados secuencialmente
    pub tiles: Vec<SpriteTile>,
    /// Rango [start, end) de tiles para cada spritesheet
    pub banks: Vec<TileBank>,
    /// Índice por defecto para edificios no mapeados
    pub fallback_idx: usize,
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

impl TextureAtlas {
    /// Crea un atlas vacío
    pub fn new() -> Self {
        // El tile 0 es siempre un fallback (cuadrado magenta de "falta textura")
        let fallback = {
            let mut pixels = vec![0u32; 256]; // 16x16
            for i in 0..256 {
                let x = i % 16;
                let y = i / 16;
                // Patrón checker magenta/negro para identificar tiles faltantes
                pixels[i] = if (x + y) % 2 == 0 { 0xFF_FF_00_FF } else { 0xFF_00_00_00 };
            }
            SpriteTile { pixels, width: 16, height: 16 }
        };

        Self {
            tiles: vec![fallback],
            banks: Vec::new(),
            fallback_idx: 0,
        }
    }

    /// Carga un spritesheet PNG, extrae tiles y los añade al atlas.
    /// Retorna (start_idx, count) del banco añadido.
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

                // Verificar que el tile está dentro de la imagen
                if src_x + tile_w > img_w || src_y + tile_h > img_h {
                    continue;
                }

                let mut tile_pixels = vec![0u32; (tile_w * tile_h) as usize];
                let mut has_content = false;

                for py in 0..tile_h {
                    for px in 0..tile_w {
                        let src_idx = ((src_y + py) * img_w + (src_x + px)) as usize;
                        let pixel = pixels[src_idx];
                        // Verificar si el píxel no es completamente transparente o magenta
                        let alpha = (pixel >> 24) & 0xFF;
                        if alpha > 0 && pixel & 0x00_FF_FF_FF != 0x00_FF_00_FF {
                            has_content = true;
                        }
                        tile_pixels[(py * tile_w + px) as usize] = pixel;
                    }
                }

                if has_content {
                    self.tiles.push(SpriteTile {
                        pixels: tile_pixels,
                        width: tile_w,
                        height: tile_h,
                    });
                    count += 1;
                }
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

    /// Blit de un sprite centrado en (cx, cy) con escala `scale` al framebuffer.
    /// Usa alpha blending sobre el fondo existente.
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

        // Viewport culling
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

        // Factor de muestreo desde el tile al framebuffer
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

                if sa == 0 {
                    // Totalmente transparente
                    continue;
                }

                if sa == 255 {
                    // Opaco — escritura directa
                    unsafe {
                        *fb.get_unchecked_mut(row_start + px as usize) = src;
                    }
                } else {
                    // Alpha blending
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

    /// Blit rápido sin alpha blending (para tiles de terreno)
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

        // Viewport culling
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

    /// Obtiene referencia a un tile
    pub fn get_tile(&self, idx: usize) -> &SpriteTile {
        if idx < self.tiles.len() {
            &self.tiles[idx]
        } else {
            &self.tiles[self.fallback_idx]
        }
    }

    /// Número total de tiles cargados
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Devuelve el índice de inicio del banco por nombre
    pub fn bank_start(&self, name: &str) -> Option<usize> {
        self.banks.iter().find(|b| b.name == name).map(|b| b.start_idx)
    }
}

// ---------------------------------------------------------------------------
// CARGA DE PNG (usando crate `png`)
// ---------------------------------------------------------------------------

/// Carga un archivo PNG y devuelve (ancho, alto, Vec<u32> ARGB)
fn load_png(path: &Path) -> Result<(u32, u32, Vec<u32>), String> {
    let mut file = File::open(path)
        .map_err(|e| format!("No se pudo abrir {}: {}", path.display(), e))?;

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("Error leyendo {}: {}", path.display(), e))?;

    let decoder = png::Decoder::new(&bytes[..]);
    let mut reader = decoder
        .read_info()
        .map_err(|e| format!("Error decodificando PNG {}: {}", path.display(), e))?;
    let color_type = info.color_type;
    // info se libera aquí automáticamente (es una referencia, no necesita drop explícito)
    let output_buffer_size = reader.output_buffer_size();
    // Drop the immutable borrow on reader before calling next_frame
    let output_buffer_size = reader.output_buffer_size();
    drop(info);

    // Configurar transformaciones para obtener RGBA
    let mut buf = vec![0u8; output_buffer_size];
    let frame_info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("Error leyendo frame {}: {}", path.display(), e))?;

    let bytes_per_pixel = match color_type {
        png::ColorType::Rgba => 4,
        png::ColorType::Rgb => 3,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Grayscale => 1,
        png::ColorType::Indexed => 4,
        _ => 4, // Fallback: asumir RGBA
    };

    // La salida del decoder ya está expandida a RGBA si usamos las transformaciones adecuadas
    // pero para simplificar, leemos los bytes manualmente
    let pixel_count = (width * height) as usize;
    let mut pixels = vec![0u32; pixel_count];

    // El decoder de png 0.17 expande automáticamente a RGBA con las transformaciones.
    // El buffer de salida tiene `output_buffer_size()` bytes = width*height*4.
    // Pero usamos `next_frame` que rellena `buf` con los bytes crudos según el formato.
    // Para RGBA: 4 bytes por píxel. Para RGB: 3 bytes. etc.
    let row_bytes = frame_info.buffer_size() / height as usize;

    for y in 0..height as usize {
        let row_start = y * row_bytes;
        for x in 0..width as usize {
            let idx = y * width as usize + x;
            match bytes_per_pixel {
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
                    // Fallback: asumir RGBA
                    if row_start + x * 4 + 3 < buf.len() {
                        let r = buf[row_start + x * 4] as u32;
                        let g = buf[row_start + x * 4 + 1] as u32;
                        let b = buf[row_start + x * 4 + 2] as u32;
                        let a = buf[row_start + x * 4 + 3] as u32;
                        pixels[idx] = (a << 24) | (r << 16) | (g << 8) | b;
                    } else {
                        pixels[idx] = 0xFF_FF_00_FF; // Magenta de error
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

/// Genera un tile de terreno procedural (para cuando no hay assets)
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
    SpriteTile { pixels, width: size, height: size }
}

/// Genera un tile de agua procedural
/// Genera un tile de agua procedural
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
    SpriteTile { pixels, width: size, height: size }
}

/// Genera un tile de carretera procedural
/// Genera un tile de carretera procedural
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
    SpriteTile { pixels, width: size, height: size }
}
/// Genera tile de edificio procedural según tipo
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
                // Cuerpo del edificio
                let shade = if x == 0 || x == size - 1 || y == roof_y {
                    70 // Borde
                } else {
                    // Ventanas en patrón
                    if (x / 4 + y / 4) % 2 == 0 && x > 1 && x < size - 2 && y > roof_y + 1 && y < size - 2 {
                        255 // Ventana iluminada
                    } else {
                        100 // Pared
                    }
                };
                let sr = (r * shade / 255).min(255);
                let sg = (g * shade / 255).min(255);
                let sb = (b * shade / 255).min(255);
                0xFF_00_00_00 | (sr << 16) | (sg << 8) | sb
            } else {
                // Techo (más oscuro)
                let sr = (r * 60 / 255).min(255);
                let sg = (g * 60 / 255).min(255);
                let sb = (b * 60 / 255).min(255);
                0xFF_00_00_00 | (sr << 16) | (sg << 8) | sb
            };
            pixels[(y * size + x) as usize] = pixel;
        }
    }
    SpriteTile { pixels, width: size, height: size }
}
