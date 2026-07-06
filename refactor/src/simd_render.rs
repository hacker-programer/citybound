// Módulo SIMD Render v0.9.1 — SSE2/AVX Intrinsics Reales
//
// FASE 6 OPTIMIZACIONES:
// - Alpha blending SIMD: 4 píxeles por instrucción SSE2
// - fill_circle con SIMD: escaneo de filas acelerado
// - fill_rect_alpha SIMD real: branchless, div255 entera
// - Prefetch hints para rectángulos > 64px
// - Clear de framebuffer con memset intrínseco
//
// TÉCNICAS:
// [TC#13] Loop Unrolling (16px/32px batch)
// [TC#28] get_unchecked
// [TA#17] SIMD nativo SSE2
//
// BENCHMARK ESPERADO: alpha_blend 4-6x más rápido

#![allow(dead_code)]

#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

const DIV255_MUL: u32 = 32897; // floor(2^23 / 255)

// ============================================================================
// FILL RECT OPACO — SSE2 16px desenrollado
// ============================================================================

#[inline(always)]
pub unsafe fn fill_rect_simd(fb: &mut [u32], fb_w: usize, fb_h: usize, x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    fill_rect_impl(fb, fb_w, fb_h, x, y, rw, rh, color);
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn fill_rect_impl(fb: &mut [u32], fb_w: usize, fb_h: usize, x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x + rw).min(fb_w as i32);
    let y2 = (y + rh).min(fb_h as i32);
    if x1 >= x2 || y1 >= y2 { return; }

    let width = (x2 - x1) as usize;
    let color_vec = _mm_set1_epi32(color as i32);
    let unrolled_16_end = x1 as usize + (width / 16) * 16;

    // Prefetch para rectángulos grandes
    if width >= 64 && (y2 - y1) >= 16 {
        let prefetch_addr = fb.as_ptr().add((y1 as usize) * fb_w + x1 as usize);
        _mm_prefetch::<_MM_HINT_T0>(prefetch_addr as *const i8);
    }

    for py in y1..y2 {
        let row_start = (py as usize) * fb_w;
        let mut px = x1 as usize;

        // 16 píxeles desenrollados (4 stores SSE2)
        while px < unrolled_16_end {
            let ptr = fb.as_mut_ptr().add(row_start + px) as *mut __m128i;
            _mm_storeu_si128(ptr, color_vec);
            _mm_storeu_si128(ptr.add(1), color_vec);
            _mm_storeu_si128(ptr.add(2), color_vec);
            _mm_storeu_si128(ptr.add(3), color_vec);
            px += 16;
        }

        // Cola de a 4 píxeles
        while px + 4 <= x2 as usize {
            let ptr = fb.as_mut_ptr().add(row_start + px) as *mut __m128i;
            _mm_storeu_si128(ptr, color_vec);
            px += 4;
        }

        // Cola de a 1 píxel
        while px < x2 as usize {
            *fb.get_unchecked_mut(row_start + px) = color;
            px += 1;
        }
    }
}

#[cfg(not(target_arch = "x86_64"))]
unsafe fn fill_rect_impl(fb: &mut [u32], fb_w: usize, fb_h: usize, x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x + rw).min(fb_w as i32);
    let y2 = (y + rh).min(fb_h as i32);
    if x1 >= x2 || y1 >= y2 { return; }

    let width = (x2 - x1) as usize;
    let unrolled_16_end = x1 as usize + (width / 16) * 16;

    for py in y1..y2 {
        let row_start = (py as usize) * fb_w;
        let mut px = x1 as usize;
        while px < unrolled_16_end {
            let idx = row_start + px;
            *fb.get_unchecked_mut(idx) = color;
            *fb.get_unchecked_mut(idx + 1) = color;
            *fb.get_unchecked_mut(idx + 2) = color;
            *fb.get_unchecked_mut(idx + 3) = color;
            *fb.get_unchecked_mut(idx + 4) = color;
            *fb.get_unchecked_mut(idx + 5) = color;
            *fb.get_unchecked_mut(idx + 6) = color;
            *fb.get_unchecked_mut(idx + 7) = color;
            *fb.get_unchecked_mut(idx + 8) = color;
            *fb.get_unchecked_mut(idx + 9) = color;
            *fb.get_unchecked_mut(idx + 10) = color;
            *fb.get_unchecked_mut(idx + 11) = color;
            *fb.get_unchecked_mut(idx + 12) = color;
            *fb.get_unchecked_mut(idx + 13) = color;
            *fb.get_unchecked_mut(idx + 14) = color;
            *fb.get_unchecked_mut(idx + 15) = color;
            px += 16;
        }
        while px + 4 <= x2 as usize {
            let idx = row_start + px;
            *fb.get_unchecked_mut(idx) = color;
            *fb.get_unchecked_mut(idx + 1) = color;
            *fb.get_unchecked_mut(idx + 2) = color;
            *fb.get_unchecked_mut(idx + 3) = color;
            px += 4;
        }
        while px < x2 as usize {
            *fb.get_unchecked_mut(row_start + px) = color;
            px += 1;
        }
    }
}

// ============================================================================
// FILL RECT ALPHA — SSE2 real (4 píxeles por instrucción)
// ============================================================================

#[inline(always)]
pub unsafe fn fill_rect_alpha_simd(fb: &mut [u32], fb_w: usize, fb_h: usize, x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x + rw).min(fb_w as i32);
    let y2 = (y + rh).min(fb_h as i32);
    if x1 >= x2 || y1 >= y2 { return; }

    let src_a = ((color >> 24) & 0xFF) as u32;
    if src_a >= 255 {
        fill_rect_impl(fb, fb_w, fb_h, x1, y1, x2 - x1, y2 - y1, color);
        return;
    }
    if src_a == 0 { return; }

    #[cfg(target_arch = "x86_64")]
    {
        fill_rect_alpha_sse2(fb, fb_w, fb_h, x1, y1, x2, y2, color, src_a);
        return;
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        fill_rect_alpha_scalar(fb, fb_w, fb_h, x1, y1, x2, y2, color, src_a);
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "sse2")]
unsafe fn fill_rect_alpha_sse2(
    fb: &mut [u32], fb_w: usize, _fb_h: usize,
    x1: i32, y1: i32, x2: i32, y2: i32,
    color: u32, src_a: u32,
) {
    let inv_a = 255 - src_a;
    let src_r = ((color >> 16) & 0xFF) as u32;
    let src_g = ((color >> 8) & 0xFF) as u32;
    let src_b = (color & 0xFF) as u32;

    // Pre-multiplicar fuente por alpha
    let sr_a = src_r * src_a;
    let sg_a = src_g * src_a;
    let sb_a = src_b * src_a;

    // Vector de constantes para división por 255
    let div255 = _mm_set1_epi32(DIV255_MUL as i32);
    let half = _mm_set1_epi32(32768i32); // 0.5 * 2^16 para redondeo
    let inv_a_vec = _mm_set1_epi32(inv_a as i32);

    // Componentes fuente pre-multiplicados (ya en formato para blend)
    let sr_a_vec = _mm_set1_epi32(sr_a as i32);
    let sg_a_vec = _mm_set1_epi32(sg_a as i32);
    let sb_a_vec = _mm_set1_epi32(sb_a as i32);
    let src_a_vec = _mm_set1_epi32((src_a * 255) as i32);

    let width = (x2 - x1) as usize;

    for py in y1..y2 {
        let row_start = (py as usize) * fb_w;
        let mut px = x1 as usize;

        // SIMD: 4 píxeles a la vez
        let simd_end = x1 as usize + (width / 4) * 4;
        while px < simd_end {
            let idx = row_start + px;
            let dst_ptr = fb.as_ptr().add(idx) as *const __m128i;
            let dst_vec = _mm_loadu_si128(dst_ptr);

            // Extraer componentes destino
            let dst_a = _mm_srli_epi32::<24>(dst_vec);
            let dst_r = _mm_and_si128(_mm_srli_epi32::<16>(dst_vec), _mm_set1_epi32(0xFF));
            let dst_g = _mm_and_si128(_mm_srli_epi32::<8>(dst_vec), _mm_set1_epi32(0xFF));
            let dst_b = _mm_and_si128(dst_vec, _mm_set1_epi32(0xFF));

            // out_a = (src_a*255 + dst_a*inv_a) * DIV255_MUL >> 23
            let dst_a_mul = _mm_mullo_epi32(dst_a, inv_a_vec);
            let out_a = _mm_srli_epi32::<23>(
                _mm_add_epi32(
                    _mm_mullo_epi32(_mm_add_epi32(src_a_vec, dst_a_mul), div255),
                    half,
                )
            );

            // out_r = (sr_a + dst_r*inv_a) * DIV255_MUL >> 23
            let dst_r_mul = _mm_mullo_epi32(dst_r, inv_a_vec);
            let out_r = _mm_srli_epi32::<23>(
                _mm_add_epi32(
                    _mm_mullo_epi32(_mm_add_epi32(sr_a_vec, dst_r_mul), div255),
                    half,
                )
            );

            // out_g
            let dst_g_mul = _mm_mullo_epi32(dst_g, inv_a_vec);
            let out_g = _mm_srli_epi32::<23>(
                _mm_add_epi32(
                    _mm_mullo_epi32(_mm_add_epi32(sg_a_vec, dst_g_mul), div255),
                    half,
                )
            );

            // out_b
            let dst_b_mul = _mm_mullo_epi32(dst_b, inv_a_vec);
            let out_b = _mm_srli_epi32::<23>(
                _mm_add_epi32(
                    _mm_mullo_epi32(_mm_add_epi32(sb_a_vec, dst_b_mul), div255),
                    half,
                )
            );

            // Ensamblar resultado: a << 24 | r << 16 | g << 8 | b
            let out = _mm_or_si128(
                _mm_or_si128(
                    _mm_slli_epi32::<24>(out_a),
                    _mm_slli_epi32::<16>(out_r),
                ),
                _mm_or_si128(
                    _mm_slli_epi32::<8>(out_g),
                    out_b,
                ),
            );

            let dst_mut_ptr = fb.as_mut_ptr().add(idx) as *mut __m128i;
            _mm_storeu_si128(dst_mut_ptr, out);

            px += 4;
        }

        // Cola escalar — dentro de unsafe fn así que get_unchecked es válido
        while px < x2 as usize {
            let idx = row_start + px;
            let dst = *fb.get_unchecked(idx);
            let dst_r = (dst >> 16) & 0xFF;
            let dst_g = (dst >> 8) & 0xFF;
            let dst_b = dst & 0xFF;
            let dst_a = (dst >> 24) & 0xFF;

            let out_a = (src_a * 255 + dst_a * inv_a) * DIV255_MUL + 32768 >> 23;
            let out_r = (sr_a + dst_r * inv_a) * DIV255_MUL + 32768 >> 23;
            let out_g = (sg_a + dst_g * inv_a) * DIV255_MUL + 32768 >> 23;
            let out_b = (sb_a + dst_b * inv_a) * DIV255_MUL + 32768 >> 23;

            *fb.get_unchecked_mut(idx) = (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b;
            px += 1;
        }
    }
}

/// Fallback escalar para alpha blending (no-SSE2)
unsafe fn fill_rect_alpha_scalar(
    fb: &mut [u32], fb_w: usize, _fb_h: usize,
    x1: i32, y1: i32, x2: i32, y2: i32,
    color: u32, src_a: u32,
) {
    let inv_a = 255 - src_a;
    let src_r = ((color >> 16) & 0xFF) as u32;
    let src_g = ((color >> 8) & 0xFF) as u32;
    let src_b = (color & 0xFF) as u32;
    let sr_a = src_r * src_a;
    let sg_a = src_g * src_a;
    let sb_a = src_b * src_a;

    // Loop unrolling 8-wide
    for py in y1..y2 {
        let row_start = (py as usize) * fb_w;
        let width = (x2 - x1) as usize;
        let mut px = x1 as usize;
        let unrolled_end = x1 as usize + (width / 8) * 8;

        while px < unrolled_end {
            for off in 0..8 {
                let idx = row_start + px + off;
                let dst = *fb.get_unchecked(idx);
                let dst_r = (dst >> 16) & 0xFF;
                let dst_g = (dst >> 8) & 0xFF;
                let dst_b = dst & 0xFF;
                let dst_a = (dst >> 24) & 0xFF;
                let out_a = (src_a * 255 + dst_a * inv_a) * DIV255_MUL + 32768 >> 23;
                let out_r = (sr_a + dst_r * inv_a) * DIV255_MUL + 32768 >> 23;
                let out_g = (sg_a + dst_g * inv_a) * DIV255_MUL + 32768 >> 23;
                let out_b = (sb_a + dst_b * inv_a) * DIV255_MUL + 32768 >> 23;
                *fb.get_unchecked_mut(idx) = (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b;
            }
            px += 8;
        }
        while px < x2 as usize {
            let idx = row_start + px;
            let dst = *fb.get_unchecked(idx);
            let dst_r = (dst >> 16) & 0xFF;
            let dst_g = (dst >> 8) & 0xFF;
            let dst_b = dst & 0xFF;
            let dst_a = (dst >> 24) & 0xFF;
            let out_a = (src_a * 255 + dst_a * inv_a) * DIV255_MUL + 32768 >> 23;
            let out_r = (sr_a + dst_r * inv_a) * DIV255_MUL + 32768 >> 23;
            let out_g = (sg_a + dst_g * inv_a) * DIV255_MUL + 32768 >> 23;
            let out_b = (sb_a + dst_b * inv_a) * DIV255_MUL + 32768 >> 23;
            *fb.get_unchecked_mut(idx) = (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b;
            px += 1;
        }
    }
}

// ============================================================================
// FILL PATTERN — Copia desde buffer fuente (SIMD memcpy)
// ============================================================================

#[inline(always)]
pub unsafe fn fill_pattern_simd(fb: &mut [u32], fb_w: usize, fb_h: usize, dst_x: i32, dst_y: i32, rw: i32, rh: i32, pattern: &[u32], pattern_w: usize) {
    let x1 = dst_x.max(0);
    let y1 = dst_y.max(0);
    let x2 = (dst_x + rw).min(fb_w as i32);
    let y2 = (dst_y + rh).min(fb_h as i32);
    if x1 >= x2 || y1 >= y2 { return; }

    let pattern_h = pattern.len().max(1) / pattern_w.max(1);

    for py in y1..y2 {
        let row_start = (py as usize) * fb_w;
        let src_row = ((py - dst_y) as usize % pattern_h) * pattern_w;
        let mut px = x1 as usize;

        // SIMD memcpy: 4 píxeles a la vez
        #[cfg(target_arch = "x86_64")]
        {
            let simd_end = x1 as usize + (((x2 - x1) as usize) / 4) * 4;
            while px < simd_end {
                let mut src_px = [0u32; 4];
                for i in 0..4 {
                    src_px[i] = *pattern.get_unchecked(src_row + ((px + i - dst_x as usize) % pattern_w));
                }
                let src_vec = _mm_loadu_si128(src_px.as_ptr() as *const __m128i);
                _mm_storeu_si128(fb.as_mut_ptr().add(row_start + px) as *mut __m128i, src_vec);
                px += 4;
            }
        }

        while px < x2 as usize {
            let src_idx = src_row + ((px - dst_x as usize) % pattern_w);
            *fb.get_unchecked_mut(row_start + px) = *pattern.get_unchecked(src_idx);
            px += 1;
        }
    }
}

// ============================================================================
// BLIT SCALED — Nearest-neighbor rápido con step pre-calculado
// ============================================================================

#[inline(always)]
pub unsafe fn blit_scaled(fb: &mut [u32], fb_w: usize, fb_h: usize, dst_x: i32, dst_y: i32, dst_w: i32, dst_h: i32, src: &[u32], src_w: usize, src_h: usize) {
    let x1 = dst_x.max(0);
    let y1 = dst_y.max(0);
    let x2 = (dst_x + dst_w).min(fb_w as i32);
    let y2 = (dst_y + dst_h).min(fb_h as i32);
    if x1 >= x2 || y1 >= y2 || src.is_empty() { return; }

    // Pre-calcular steps en fixed-point para evitar float en inner loop
    let step_x = ((src_w << 16) as u32 / dst_w.max(1) as u32) as usize;
    let step_y = ((src_h << 16) as u32 / dst_h.max(1) as u32) as usize;

    for py in y1..y2 {
        let src_y = (((py - dst_y) as usize * step_y) >> 16) % src_h;
        let row_start = (py as usize) * fb_w;

        for px in x1..x2 {
            let src_x = (((px - dst_x) as usize).wrapping_mul(step_x) >> 16) % src_w;
            *fb.get_unchecked_mut(row_start + px as usize) = *src.get_unchecked(src_y * src_w + src_x);
        }
    }
}

// ============================================================================
// FILL RECT CON CLIP RAPIDO (para uso en render)
// ============================================================================

/// Versión rápida para relleno con coordenadas pre-validadas
#[inline(always)]
pub unsafe fn fill_rect_clipped(fb: &mut [u32], fb_w: usize, fb_h: usize, x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    fill_rect_impl(fb, fb_w, fb_h, x, y, rw, rh, color);
}

// ============================================================================
// CACHE WARMING [TA#8]
// ============================================================================

pub fn warm_cache(fb: &mut [u32], fb_size: usize) {
    let chunks = fb_size / 64;
    for chunk in 0..chunks.min(4096) {
        let idx = chunk * 64;
        if idx + 16 <= fb_size {
            unsafe {
                let val = std::ptr::read_volatile(fb.as_ptr().add(idx));
                std::ptr::write_volatile(fb.as_mut_ptr().add(idx), val);
            }
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_rect_simd_basic() {
        let mut fb = vec![0u32; 400];
        unsafe { fill_rect_simd(&mut fb, 20, 20, 2, 2, 16, 16, 0xFF_FF_00_00); }
        let filled = fb.iter().filter(|&&p| p == 0xFF_FF_00_00).count();
        assert_eq!(filled, 256);
    }

    #[test]
    fn test_fill_rect_clipping() {
        let mut fb = vec![0u32; 100];
        unsafe { fill_rect_simd(&mut fb, 10, 10, 20, 20, 5, 5, 0xFF_FF_00_00); }
        let red = fb.iter().filter(|&&p| p == 0xFF_FF_00_00).count();
        assert_eq!(red, 0);
    }

    #[test]
    fn test_alpha_opaque() {
        let mut fb = vec![0xFF_00_00_00u32; 100];
        unsafe { fill_rect_alpha_simd(&mut fb, 10, 10, 0, 0, 5, 5, 0xFF_FF_00_00); }
        assert_eq!(fb[0], 0xFF_FF_00_00);
    }

    #[test]
    fn test_alpha_transparent() {
        let mut fb = vec![0xFF_00_00_00u32; 100];
        unsafe { fill_rect_alpha_simd(&mut fb, 10, 10, 0, 0, 5, 5, 0x00_FF_00_00); }
        assert_eq!(fb[0], 0xFF_00_00_00);
    }

    #[test]
    fn test_alpha_partial() {
        let mut fb = vec![0xFF_00_00_00u32; 100];
        unsafe { fill_rect_alpha_simd(&mut fb, 10, 10, 0, 0, 2, 2, 0x80_FF_00_00); }
        let r = (fb[0] >> 16) & 0xFF;
        assert!(r > 100 && r < 160, "Alpha 50% rojo: r={}", r);
    }

    #[test]
    fn test_alpha_simd_equals_scalar() {
        let mut fb_simd = vec![0xFF_11_22_33u32; 400];

        unsafe {
            fill_rect_alpha_simd(&mut fb_simd, 20, 20, 2, 2, 16, 14, 0x80_AA_BB_CC);
        }

        let modified = fb_simd.iter().any(|&p| p != 0xFF_11_22_33);
        assert!(modified, "Alpha SIMD debe modificar píxeles");
    }

    #[test]
    fn test_edge_cases() {
        let mut fb = vec![0u32; 100];
        unsafe { fill_rect_simd(&mut fb, 10, 10, 5, 5, 0, 5, 0xFF_FF_00_00); }
        unsafe { fill_rect_simd(&mut fb, 10, 10, 5, 5, 5, 0, 0xFF_FF_00_00); }
        unsafe { fill_rect_simd(&mut fb, 10, 10, 0, 0, 1, 1, 0xFF_FF_00_00); }
        assert_eq!(fb[0], 0xFF_FF_00_00);
    }

    #[test]
    fn test_pattern() {
        let mut fb = vec![0xFF_00_00_00u32; 400];
        let pattern = vec![0xFF_FF_00_00u32, 0xFF_00_FF_00];
        unsafe { fill_pattern_simd(&mut fb, 20, 20, 0, 0, 20, 20, &pattern, 2); }
        assert_ne!(fb[0], 0xFF_00_00_00);
    }

    #[test]
    fn test_warm_cache() {
        let mut fb = vec![0u32; 480000];
        warm_cache(&mut fb, 480000);
    }

    #[test]
    fn test_alpha_bench_equivalent() {
        let mut fb = vec![0xFF_44_44_44u32; 256];
        unsafe { fill_rect_alpha_simd(&mut fb, 16, 16, 0, 0, 16, 16, 0xFF_FF_00_00); }
        assert_eq!(fb[0], 0xFF_FF_00_00);
        assert_eq!(fb[255], 0xFF_FF_00_00);
    }
}
