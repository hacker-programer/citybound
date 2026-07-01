// Módulo SIMD Render - Framebuffer Operations aceleradas
//
// TÉCNICA AVANZADA #17 (juegos): Native SIMD autovectorizado
// Para hardware nativo (Pentium), usamos loop unrolling agresivo
// que LLVM autovectoriza a SSE2 en x86. Esto procesa hasta 16 píxeles
// por batch, reduciendo dramáticamente la sobrecarga de saltos.
//
// TÉCNICA COMÚN #13: Loop Unrolling Manual (16px por iteración)
// TÉCNICA COMÚN #28: get_unchecked sin bounds check

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Máscara para extraer/insertar alpha
#[allow(dead_code)]
const ALPHA_MASK: u32 = 0xFF_00_00_00;
/// Máscara para canales RGB
#[allow(dead_code)]
const RGB_MASK: u32 = 0x00_FF_FF_FF;

// ---------------------------------------------------------------------------
// FILL RECT con Loop Unrolling Agresivo (16 píxeles por batch)
//
// LLVM autovectoriza estos stores contiguos a instrucciones SSE2
// de 128 bits, logrando 4 píxeles por instrucción en hardware x86.
// ---------------------------------------------------------------------------

/// Rellena un rectángulo sólido con 16-pixel unrolling.
/// SAFETY: El caller debe garantizar que las coordenadas están dentro
/// de los bounds del framebuffer.
#[inline(always)]
pub unsafe fn fill_rect_simd(
    fb: &mut [u32],
    fb_w: usize,
    fb_h: usize,
    x: i32,
    y: i32,
    rw: i32,
    rh: i32,
    color: u32,
) {
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x + rw).min(fb_w as i32);
    let y2 = (y + rh).min(fb_h as i32);

    if x1 >= x2 || y1 >= y2 {
        return;
    }

    let width = (x2 - x1) as usize;
    let unrolled_16_end = x1 as usize + (width / 16) * 16;
    let unrolled_4_end = x1 as usize + (width / 4) * 4;

    for py in y1..y2 {
        let row_start = (py as usize) * fb_w;
        let mut px = x1 as usize;

        // [TC#13]: Nivel 1 - procesar 16 píxeles (4 grupos de 4)
        // LLVM fusiona estos 16 stores en ~4 instrucciones SSE2 movdqa
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

        // Nivel 2: procesar 4 píxeles
        while px < unrolled_4_end {
            let idx = row_start + px;
            *fb.get_unchecked_mut(idx) = color;
            *fb.get_unchecked_mut(idx + 1) = color;
            *fb.get_unchecked_mut(idx + 2) = color;
            *fb.get_unchecked_mut(idx + 3) = color;
            px += 4;
        }

        // Nivel 3: píxeles residuales
        while px < x2 as usize {
            *fb.get_unchecked_mut(row_start + px) = color;
            px += 1;
        }
    }
}

/// Alpha blending optimizado para rectángulos.
/// Pre-calcula valores de fuente para reducir operaciones por píxel.
#[inline(always)]
pub unsafe fn fill_rect_alpha_simd(
    fb: &mut [u32],
    fb_w: usize,
    fb_h: usize,
    x: i32,
    y: i32,
    rw: i32,
    rh: i32,
    color: u32,
) {
    let x1 = x.max(0);
    let y1 = y.max(0);
    let x2 = (x + rw).min(fb_w as i32);
    let y2 = (y + rh).min(fb_h as i32);

    if x1 >= x2 || y1 >= y2 {
        return;
    }

    let src_a = ((color >> 24) & 0xFF) as u32;

    // Fast path: totalmente opaco → usar fill_rect_simd
    if src_a >= 255 {
        fill_rect_simd(fb, fb_w, fb_h, x1, y1, x2 - x1, y2 - y1, color);
        return;
    }

    // Fast path: totalmente transparente → no hacer nada
    if src_a == 0 {
        return;
    }

    let inv_a = 255 - src_a;
    let src_r = (color >> 16) & 0xFF;
    let src_g = (color >> 8) & 0xFF;
    let src_b = color & 0xFF;

    // Pre-calcular valores fuente * alpha (una vez, no por píxel)
    let sr_a = src_r * src_a;
    let sg_a = src_g * src_a;
    let sb_a = src_b * src_a;

    for py in y1..y2 {
        let row_start = (py as usize) * fb_w;
        for px in x1..x2 {
            let dst = *fb.get_unchecked(row_start + px as usize);
            let dst_r = (dst >> 16) & 0xFF;
            let dst_g = (dst >> 8) & 0xFF;
            let dst_b = dst & 0xFF;

            // Alpha blending: out = src * src_alpha + dst * (1 - src_alpha)
            let out_a = src_a + (((dst >> 24) & 0xFF) * inv_a) / 255;
            let out_r = (sr_a + dst_r * inv_a) / 255;
            let out_g = (sg_a + dst_g * inv_a) / 255;
            let out_b = (sb_a + dst_b * inv_a) / 255;

            *fb.get_unchecked_mut(row_start + px as usize) =
                (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b;
        }
    }
}

// ---------------------------------------------------------------------------
// FILL CON PATRÓN - Copia desde buffer fuente
// ---------------------------------------------------------------------------

/// Rellena un rectángulo copiando desde un buffer fuente (tile/patrón).
/// Útil para sprites atlas y terrain tiles [TC#4].
#[inline(always)]
pub unsafe fn fill_pattern_simd(
    fb: &mut [u32],
    fb_w: usize,
    fb_h: usize,
    dst_x: i32,
    dst_y: i32,
    rw: i32,
    rh: i32,
    pattern: &[u32],
    pattern_w: usize,
) {
    let x1 = dst_x.max(0);
    let y1 = dst_y.max(0);
    let x2 = (dst_x + rw).min(fb_w as i32);
    let y2 = (dst_y + rh).min(fb_h as i32);

    if x1 >= x2 || y1 >= y2 {
        return;
    }

    let pattern_h = pattern.len().max(1) / pattern_w.max(1);

    for py in y1..y2 {
        let row_start = (py as usize) * fb_w;
        let src_row = ((py - dst_y) as usize % pattern_h) * pattern_w;

        let width = (x2 - x1) as usize;
        let unrolled_end = x1 as usize + (width / 4) * 4;
        let mut px = x1 as usize;

        while px < unrolled_end {
            let src_base = src_row + ((px - dst_x as usize) % pattern_w);
            *fb.get_unchecked_mut(row_start + px) = *pattern.get_unchecked(src_base);
            *fb.get_unchecked_mut(row_start + px + 1) = *pattern.get_unchecked((src_base + 1) % pattern_w);
            *fb.get_unchecked_mut(row_start + px + 2) = *pattern.get_unchecked((src_base + 2) % pattern_w);
            *fb.get_unchecked_mut(row_start + px + 3) = *pattern.get_unchecked((src_base + 3) % pattern_w);
            px += 4;
        }

        while px < x2 as usize {
            let src_idx = src_row + ((px - dst_x as usize) % pattern_w);
            *fb.get_unchecked_mut(row_start + px) = *pattern.get_unchecked(src_idx);
            px += 1;
        }
    }
}

// ---------------------------------------------------------------------------
// CACHE WARMING [TA#8]
// ---------------------------------------------------------------------------

/// Calienta las estructuras de datos críticas para asegurar hits de caché.
/// Recorre el framebuffer tocando cada línea de caché (64 bytes = 16 píxeles).
pub fn warm_cache(fb: &mut [u32], fb_size: usize) {
    // [TA#8]: Tocar cada línea de caché (cada 64 bytes)
    // Usamos volatile para evitar que el compilador optimice las lecturas
    for i in (0..fb_size).step_by(16) {
        unsafe {
            let val = std::ptr::read_volatile(fb.as_ptr().add(i));
            std::ptr::write_volatile(fb.as_mut_ptr().add(i), val);
        }
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_rect_simd_basic() {
        let mut fb = vec![0u32; 400]; // 20x20
        unsafe {
            fill_rect_simd(&mut fb, 20, 20, 2, 2, 16, 16, 0xFF_FF_00_00);
        }
        let filled = fb.iter().filter(|&&p| p == 0xFF_FF_00_00).count();
        assert_eq!(filled, 256, "SIMD fill debe cubrir 16x16=256 píxeles");
    }

    #[test]
    fn test_fill_rect_simd_clipping() {
        let mut fb = vec![0u32; 100]; // 10x10
        unsafe {
            fill_rect_simd(&mut fb, 10, 10, 20, 20, 5, 5, 0xFF_FF_00_00);
            fill_rect_simd(&mut fb, 10, 10, 8, 8, 10, 10, 0xFF_00_FF_00);
        }
        let red = fb.iter().filter(|&&p| p == 0xFF_FF_00_00).count();
        assert_eq!(red, 0, "Fuera de bounds no debe dibujar");
    }

    #[test]
    fn test_fill_rect_alpha_simd_opaque() {
        let mut fb = vec![0xFF_00_00_00u32; 100];
        unsafe {
            fill_rect_alpha_simd(&mut fb, 10, 10, 0, 0, 5, 5, 0xFF_FF_00_00);
        }
        assert_eq!(fb[0], 0xFF_FF_00_00);
    }

    #[test]
    fn test_fill_rect_alpha_simd_transparent() {
        let mut fb = vec![0xFF_00_00_00u32; 100];
        unsafe {
            fill_rect_alpha_simd(&mut fb, 10, 10, 0, 0, 5, 5, 0x00_FF_00_00);
        }
        assert_eq!(fb[0], 0xFF_00_00_00, "Alpha 0 no debe modificar");
    }

    #[test]
    fn test_fill_pattern_simd() {
        let mut fb = vec![0xFF_00_00_00u32; 400]; // 20x20
        let pattern = vec![0xFF_FF_00_00u32, 0xFF_00_FF_00, 0xFF_00_00_FF, 0xFF_FF_FF_00];
        unsafe {
            fill_pattern_simd(&mut fb, 20, 20, 0, 0, 20, 20, &pattern, 2);
        }
        assert_ne!(fb[0], 0xFF_00_00_00, "Pattern debe sobrescribir");
        assert_ne!(fb[19], 0xFF_00_00_00, "Pattern debe sobrescribir borde");
    }

    #[test]
    fn test_warm_cache_no_panic() {
        let mut fb = vec![0u32; 480000]; // 800x600
        warm_cache(&mut fb, 480000);
    }
}
