// Módulo de Terreno con Ruido Perlin Pre-generado
//
// TÉCNICA COMÚN #14: Pre-generación de Ruido Perlin (Texture Baking)
// Generamos el mapa de altura del terreno durante la carga y lo almacenamos
// en Vecs heap-allocated (no en el stack).
//
// TÉCNICA COMÚN #3: Baking de iluminación a texturas (el color se deriva
// de la altura durante la carga, no en renderizado)
//
// [FIX STACK OVERFLOW]: heights, terrain_types, baked_colors usan Vec<T>
// (heap) en lugar de arrays fijos [T; 16384] (stack = 144KB).
// Esto es crítico porque GameWorld contiene un TerrainMap inline.

/// Tamaño del mapa de terreno (debe coincidir con la grilla del mundo)
pub const TERRAIN_SIZE: usize = 128;

/// Mapa de alturas pre-generado (valores normalizados 0.0 - 1.0)
/// Todos los arrays masivos están en heap via Vec<T>.
#[repr(align(64))]
pub struct TerrainMap {
    /// Alturas en cada celda [y * TERRAIN_SIZE + x]
    pub heights: Vec<f32>,
    /// Mapa de tipos de terreno derivados: 0=agua, 1=arena, 2=pasto, 3=bosque, 4=roca
    pub terrain_types: Vec<u8>,
    /// Colores precalculados ARGB para cada celda [TC#3: Baking]
    pub baked_colors: Vec<u32>,
}

impl TerrainMap {
    /// Genera el mapa de terreno completo durante la carga.
    /// Usa Fractal Brownian Motion (fBm) con 4 octavas de ruido.
    pub fn generate(seed: u64) -> Self {
        let total_cells = TERRAIN_SIZE * TERRAIN_SIZE;
        let mut heights = vec![0.0_f32; total_cells];
        let mut terrain_types = vec![0u8; total_cells];
        let mut baked_colors = vec![0u32; total_cells];

        // Parámetros de generación
        let base_freq: f32 = 0.03;
        let octaves: usize = 4;
        let persistence: f32 = 0.5;
        let lacunarity: f32 = 2.0;

        // RNG determinista
        let mut rng = crate::rng_pool::SimpleRng::new(seed);

        // Pre-generar 4 octavas de ruido
        for y in 0..TERRAIN_SIZE {
            for x in 0..TERRAIN_SIZE {
                let mut amplitude = 1.0_f32;
                let mut frequency = base_freq;
                let mut noise_value = 0.0_f32;
                let mut max_value = 0.0_f32;

                for _ in 0..octaves {
                    // Ruido de valor simple (Value Noise)
                    let nx = x as f32 * frequency;
                    let ny = y as f32 * frequency;
                    let ix = nx as usize;
                    let iy = ny as usize;
                    let fx = nx - ix as f32;
                    let fy = ny - iy as f32;

                    // 4 esquinas con hash determinista
                    let v00 = hash_2d(ix as u32, iy as u32, &mut rng);
                    let v10 = hash_2d((ix + 1) as u32, iy as u32, &mut rng);
                    let v01 = hash_2d(ix as u32, (iy + 1) as u32, &mut rng);
                    let v11 = hash_2d((ix + 1) as u32, (iy + 1) as u32, &mut rng);

                    // Interpolación bilineal
                    let sx = smoothstep(fx);
                    let sy = smoothstep(fy);
                    let top = v00 + sx * (v10 - v00);
                    let bottom = v01 + sx * (v11 - v01);
                    let val = top + sy * (bottom - top);

                    noise_value += val * amplitude;
                    max_value += amplitude;
                    amplitude *= persistence;
                    frequency *= lacunarity;
                }

                // Normalizar
                let idx = y * TERRAIN_SIZE + x;
                heights[idx] = (noise_value / max_value).clamp(0.0, 1.0);
            }
        }

        // Derivar tipos de terreno y colores
        for y in 0..TERRAIN_SIZE {
            for x in 0..TERRAIN_SIZE {
                let idx = y * TERRAIN_SIZE + x;
                let h = heights[idx];

                let (ttype, color) = if h < 0.25 {
                    // Agua profunda a somera
                    if h < 0.15 {
                        (0u8, 0xFF_1A_3A_6Au32)
                    } else {
                        (0u8, 0xFF_2A_5A_AAu32)
                    }
                } else if h < 0.32 {
                    // Arena / playa
                    (1u8, 0xFF_C8_BF_7Au32)
                } else if h < 0.60 {
                    // Pastizales
                    let shade = ((h - 0.32) * 80.0) as u32;
                    let g = 0x4A + shade;
                    (2u8, (0xFF_00_00_00u32) | ((0x2D as u32) << 16) | (g << 8) | (0x1A as u32))
                } else if h < 0.80 {
                    // Bosque
                    (3u8, 0xFF_1A_4A_1Au32)
                } else {
                    // Roca / montaña
                    let shade = ((h - 0.80) * 200.0) as u32;
                    let gray = 0x60 + shade;
                    (4u8, (0xFF_000000u32) | (gray << 16) | (gray << 8) | gray)
                };

                terrain_types[idx] = ttype;
                baked_colors[idx] = color;
            }
        }

        TerrainMap { heights, terrain_types, baked_colors }
    }

    /// Obtiene la altura en una posición (interpolación bilineal)
    #[inline]
    pub fn height_at(&self, x: f32, y: f32) -> f32 {
        let fx = x.clamp(0.0, (TERRAIN_SIZE - 1) as f32);
        let fy = y.clamp(0.0, (TERRAIN_SIZE - 1) as f32);
        let ix = fx as usize;
        let iy = fy as usize;
        let fx_frac = fx - ix as f32;
        let fy_frac = fy - iy as f32;

        let get = |cx: usize, cy: usize| -> f32 {
            if cx >= TERRAIN_SIZE || cy >= TERRAIN_SIZE { 0.0 }
            else { self.heights[cy * TERRAIN_SIZE + cx] }
        };

        let top = get(ix, iy) + fx_frac * (get(ix + 1, iy) - get(ix, iy));
        let bottom = get(ix, iy + 1) + fx_frac * (get(ix + 1, iy + 1) - get(ix, iy + 1));
        top + fy_frac * (bottom - top)
    }

    /// Obtiene el tipo de terreno en una posición
    #[inline]
    pub fn terrain_type_at(&self, x: f32, y: f32) -> u8 {
        let ix = x.clamp(0.0, (TERRAIN_SIZE - 1) as f32) as usize;
        let iy = y.clamp(0.0, (TERRAIN_SIZE - 1) as f32) as usize;
        self.terrain_types[iy * TERRAIN_SIZE + ix]
    }

    /// Obtiene el color precalculado (baked) en una posición
    #[inline]
    pub fn color_at(&self, x: f32, y: f32) -> u32 {
        let ix = x.clamp(0.0, (TERRAIN_SIZE - 1) as f32) as usize;
        let iy = y.clamp(0.0, (TERRAIN_SIZE - 1) as f32) as usize;
        self.baked_colors[iy * TERRAIN_SIZE + ix]
    }
}

// ---------------------------------------------------------------------------
// HASH 2D determinista para ruido Perlin
// ---------------------------------------------------------------------------

fn hash_2d(x: u32, y: u32, rng: &mut crate::rng_pool::SimpleRng) -> f32 {
    // Mezclar coordenadas con el estado del RNG
    let h = (x.wrapping_mul(374761393).wrapping_add(y.wrapping_mul(668265263)))
        .wrapping_add(rng.state as u32);
    let h = (h ^ (h >> 13)).wrapping_mul(1274126177);
    let h = h ^ (h >> 16);
    (h as f32) / (u32::MAX as f32)
}

#[inline(always)]
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_generation() {
        let terrain = TerrainMap::generate(42);
        assert_eq!(terrain.heights.len(), TERRAIN_SIZE * TERRAIN_SIZE);
        assert_eq!(terrain.terrain_types.len(), TERRAIN_SIZE * TERRAIN_SIZE);
        assert_eq!(terrain.baked_colors.len(), TERRAIN_SIZE * TERRAIN_SIZE);

        // Verificar que hay variedad de alturas
        let mut min_h = 1000.0_f32;
        let mut max_h = 0.0_f32;
        for &h in &terrain.heights {
            min_h = min_h.min(h);
            max_h = max_h.max(h);
        }
        assert!(min_h < 0.3, "Debe haber zonas bajas (agua)");
        assert!(max_h > 0.5, "Debe haber zonas altas");
    }

    #[test]
    fn test_height_at() {
        let terrain = TerrainMap::generate(42);
        let h1 = terrain.height_at(64.0, 64.0);
        let h2 = terrain.height_at(64.5, 64.5);
        assert!((h1 - h2).abs() < 0.1, "Interpolación debe ser suave");
    }

    #[test]
    fn test_terrain_types_variety() {
        let terrain = TerrainMap::generate(42);
        let mut types_seen = std::collections::HashSet::new();
        for &t in &terrain.terrain_types {
            types_seen.insert(t);
        }
        // Debe haber al menos 3 tipos diferentes
        assert!(types_seen.len() >= 3, "Debe haber variedad: tipos={}", types_seen.len());
    }

    #[test]
    fn test_baked_colors_not_black() {
        let terrain = TerrainMap::generate(42);
        let non_black = terrain.baked_colors.iter()
            .filter(|&&c| c != 0xFF_00_00_00 && c != 0x00_00_00_00)
            .count();
        assert_eq!(non_black, 16384, "Todos los colores deben estar horneados");
    }
}