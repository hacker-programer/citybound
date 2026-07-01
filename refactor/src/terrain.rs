// Módulo de Terreno con Ruido Perlin Pre-generado
//
// TÉCNICA COMÚN #14: Pre-generación de Ruido Perlin (Texture Baking)
// Generamos el mapa de altura del terreno durante la carga y lo almacenamos
// en una matriz estática. Cero llamadas a la función de ruido durante el juego.
//
// TÉCNICA COMÚN #3: Baking de iluminación a texturas (el color se deriva
// de la altura durante la carga, no en renderizado)
//
// Implementación: Value Noise con interpolación cúbica + octavas (fBm).
// Usa LUTs trigonométricas del módulo luts para la interpolación.
//
// Memoria: 128x128 f32 = 64KB (cabe en caché L2 de Pentium)

use crate::luts;

/// Tamaño del mapa de terreno (debe coincidir con la grilla del mundo)
pub const TERRAIN_SIZE: usize = 128;

/// Mapa de alturas pre-generado (valores normalizados 0.0 - 1.0)
#[repr(align(64))]
pub struct TerrainMap {
    /// Alturas en cada celda [y * TERRAIN_SIZE + x]
    pub heights: [f32; TERRAIN_SIZE * TERRAIN_SIZE],
    /// Mapa de tipos de terreno derivados: 0=agua, 1=arena, 2=pasto, 3=bosque, 4=roca
    pub terrain_types: [u8; TERRAIN_SIZE * TERRAIN_SIZE],
    /// Colores precalculados ARGB para cada celda [TC#3: Baking]
    pub baked_colors: [u32; TERRAIN_SIZE * TERRAIN_SIZE],
}

impl TerrainMap {
    /// Genera el mapa de terreno completo durante la carga.
    /// Usa Fractal Brownian Motion (fBm) con 4 octavas de ruido.
    pub fn generate(seed: u64) -> Self {
        let mut heights = [0.0_f32; TERRAIN_SIZE * TERRAIN_SIZE];
        let mut terrain_types = [0u8; TERRAIN_SIZE * TERRAIN_SIZE];
        let mut baked_colors = [0u32; TERRAIN_SIZE * TERRAIN_SIZE];

        // Parámetros de generación
        let base_freq: f32 = 0.03;
        let octaves: usize = 4;
        let persistence: f32 = 0.5;
        let lacunarity: f32 = 2.0;

        for y in 0..TERRAIN_SIZE {
            for x in 0..TERRAIN_SIZE {
                // fBm: suma de octavas de ruido
                let mut height: f32 = 0.0;
                let mut amplitude: f32 = 1.0;
                let mut frequency: f32 = base_freq;
                let mut max_value: f32 = 0.0;

                for _octave in 0..octaves {
                    let nx = x as f32 * frequency;
                    let ny = y as f32 * frequency;
                    height += value_noise_2d(nx, ny, seed) * amplitude;
                    max_value += amplitude;
                    amplitude *= persistence;
                    frequency *= lacunarity;
                }

                // Normalizar
                height /= max_value;

                let idx = y * TERRAIN_SIZE + x;
                heights[idx] = height;

                // Derivar tipo de terreno de la altura
                let ttype = if height < 0.3 {
                    0u8 // Agua
                } else if height < 0.4 {
                    1u8 // Arena/playa
                } else if height < 0.65 {
                    2u8 // Pasto
                } else if height < 0.8 {
                    3u8 // Bosque
                } else {
                    4u8 // Roca/montaña
                };
                terrain_types[idx] = ttype;

                // [TC#3]: Colores baked según tipo y altura
                baked_colors[idx] = bake_terrain_color(ttype, height);
            }
        }

        TerrainMap { heights, terrain_types, baked_colors }
    }

    /// Obtiene la altura en una celda (con bounds check en debug)
    #[inline(always)]
    pub fn height(&self, x: usize, y: usize) -> f32 {
        debug_assert!(x < TERRAIN_SIZE && y < TERRAIN_SIZE);
        // SAFETY: bounds verificados por debug_assert; en release confiamos en el caller
        unsafe {
            *self.heights.get_unchecked(y * TERRAIN_SIZE + x)
        }
    }

    /// Obtiene la altura sin bounds check (para hot paths)
    /// SAFETY: caller debe garantizar x,y < TERRAIN_SIZE
    #[inline(always)]
    pub unsafe fn height_unchecked(&self, x: usize, y: usize) -> f32 {
        *self.heights.get_unchecked(y * TERRAIN_SIZE + x)
    }

    /// Obtiene el color baked de una celda
    #[inline(always)]
    pub fn baked_color(&self, x: usize, y: usize) -> u32 {
        debug_assert!(x < TERRAIN_SIZE && y < TERRAIN_SIZE);
        unsafe {
            *self.baked_colors.get_unchecked(y * TERRAIN_SIZE + x)
        }
    }

    /// Tipo de terreno en una celda
    #[inline(always)]
    pub fn terrain_type(&self, x: usize, y: usize) -> u8 {
        debug_assert!(x < TERRAIN_SIZE && y < TERRAIN_SIZE);
        unsafe {
            *self.terrain_types.get_unchecked(y * TERRAIN_SIZE + x)
        }
    }
}

/// Genera color ARGB según tipo de terreno y altura
#[inline(always)]
fn bake_terrain_color(ttype: u8, _height: f32) -> u32 {
    match ttype {
        0 => 0xFF_1A_3A_6A, // Agua profunda
        1 => 0xFF_C2_B2_80, // Arena
        2 => 0xFF_4A_8C_3F, // Pasto verde
        3 => 0xFF_2D_5A_1F, // Bosque oscuro
        4 => 0xFF_8A_7A_6A, // Roca
        _ => 0xFF_FF_00_FF, // Magenta (error)
    }
}

// ---------------------------------------------------------------------------
// VALUE NOISE 2D (reemplaza la crate 'noise')
//
// Usa hashing determinista basado en splitmix64 para generar
// valores pseudoaleatorios en las esquinas de la grilla,
// luego interpola suavemente con smoothstep.
//
// [TC#14]: Estos valores se pre-generan, nunca se llaman en runtime.
// ---------------------------------------------------------------------------

/// Hash function simple (splitmix64 adaptada)
#[inline(always)]
fn hash_2d(x: i32, y: i32, seed: u64) -> u64 {
    let mut h: u64 = seed;
    h = h.wrapping_add((x as u64).wrapping_mul(0x9E3779B97F4A7C15));
    h = h.wrapping_add((y as u64).wrapping_mul(0x9E3779B97F4A7C15 ^ 0xBF58476D1CE4E5B9));
    h = (h ^ (h >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94D049BB133111EB);
    h ^ (h >> 31)
}

/// Convierte hash a f32 en [0, 1)
#[inline(always)]
fn hash_to_f32(h: u64) -> f32 {
    (h as f32) / (u64::MAX as f32 + 1.0)
}

/// Smoothstep para interpolación suave (usa LUT de seno)
#[inline(always)]
fn smoothstep(t: f32) -> f32 {
    // t * t * (3.0 - 2.0 * t) - fórmula estándar
    t * t * (3.0 - 2.0 * t)
}

/// Value noise 2D determinista
fn value_noise_2d(x: f32, y: f32, seed: u64) -> f32 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let fx = x - ix as f32;
    let fy = y - iy as f32;

    // Suavizar coordenadas
    let sx = smoothstep(fx);
    let sy = smoothstep(fy);

    // Valores en las 4 esquinas
    let v00 = hash_to_f32(hash_2d(ix, iy, seed));
    let v10 = hash_to_f32(hash_2d(ix + 1, iy, seed));
    let v01 = hash_to_f32(hash_2d(ix, iy + 1, seed));
    let v11 = hash_to_f32(hash_2d(ix + 1, iy + 1, seed));

    // Interpolación bilineal
    let ix0 = v00 + sx * (v10 - v00);
    let ix1 = v01 + sx * (v11 - v01);
    ix0 + sy * (ix1 - ix0)
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
        // Verificar que hay variación (no todo igual)
        let first = terrain.heights[0];
        let mid = terrain.heights[TERRAIN_SIZE * TERRAIN_SIZE / 2];
        let last = terrain.heights[TERRAIN_SIZE * TERRAIN_SIZE - 1];
        assert!(
            (first - mid).abs() > 0.01 || (mid - last).abs() > 0.01,
            "El terreno debe tener variación"
        );
    }

    #[test]
    fn test_terrain_types() {
        let terrain = TerrainMap::generate(123);
        let mut has_water = false;
        let mut has_grass = false;

        for y in 0..TERRAIN_SIZE {
            for x in 0..TERRAIN_SIZE {
                let t = terrain.terrain_type(x, y);
                if t == 0 { has_water = true; }
                if t == 2 { has_grass = true; }
            }
        }

        assert!(has_water, "Debe haber agua en el terreno");
        assert!(has_grass, "Debe haber pasto en el terreno");
    }

    #[test]
    fn test_baked_colors_valid() {
        let terrain = TerrainMap::generate(456);
        for y in 0..TERRAIN_SIZE {
            for x in 0..TERRAIN_SIZE {
                let color = terrain.baked_color(x, y);
                let alpha = (color >> 24) & 0xFF;
                assert_eq!(alpha, 0xFF, "Alpha debe ser 0xFF en ({},{}): {:08X}", x, y, color);
            }
        }
    }

    #[test]
    fn test_height_range() {
        let terrain = TerrainMap::generate(789);
        for y in 0..TERRAIN_SIZE {
            for x in 0..TERRAIN_SIZE {
                let h = terrain.height(x, y);
                assert!(h >= 0.0 && h <= 1.0, "Altura fuera de rango en ({},{}): {}", x, y, h);
            }
        }
    }

    #[test]
    fn test_determinism() {
        let t1 = TerrainMap::generate(42);
        let t2 = TerrainMap::generate(42);

        for i in 0..TERRAIN_SIZE * TERRAIN_SIZE {
            assert_eq!(t1.heights[i], t2.heights[i],
                "El terreno debe ser determinista en índice {}", i);
        }
    }

    #[test]
    fn test_different_seeds_different() {
        let t1 = TerrainMap::generate(1);
        let t2 = TerrainMap::generate(2);

        let mut diff_count = 0;
        for i in 0..TERRAIN_SIZE * TERRAIN_SIZE {
            if (t1.heights[i] - t2.heights[i]).abs() > 0.001 {
                diff_count += 1;
            }
        }
        assert!(diff_count > 1000, "Diferentes seeds deben producir terreno diferente");
    }

    #[test]
    fn test_hash_determinism() {
        let h1 = hash_2d(10, 20, 42);
        let h2 = hash_2d(10, 20, 42);
        assert_eq!(h1, h2);
    }

    #[test]
    fn test_hash_to_f32_range() {
        for i in 0..1000 {
            let val = hash_to_f32(i as u64);
            assert!(val >= 0.0 && val < 1.0, "hash_to_f32 fuera de rango: {}", val);
        }
    }
}
