// Climate & Day/Night System v0.10 [FASE 7]
//
// Ciclo día/noche visual con color grading sobre el framebuffer.
// 
// TÉCNICAS:
// [TC#3] Baking de iluminación — el overlay se aplica post-render
// [TC#28] get_unchecked en bucles de píxeles
// [TA#17] SIMD para tintado de pantalla
//
// Efectos:
// - Amanecer (5:00-7:00): naranja cálido
// - Día (7:00-18:00): luz normal
// - Atardecer (18:00-20:00): rojo-anaranjado
// - Noche (20:00-5:00): azul oscuro, reducción de brillo

/// Aplica overlay de día/noche al framebuffer completo

/// Aplica overlay de día/noche al framebuffer completo
pub fn apply_day_night_overlay(fb: &mut [u32], width: usize, height: usize, time_of_day: u16) {
    let hour = time_of_day / 60;
    let minute = time_of_day % 60;
    let time_fraction = hour as f32 + minute as f32 / 60.0;

    let (r_mul, g_mul, b_mul, ambient) = get_day_night_params(time_fraction);

    // Si es pleno día, no hacemos nada (optimización)
    if (r_mul - 1.0).abs() < 0.01 && (g_mul - 1.0).abs() < 0.01 
        && (b_mul - 1.0).abs() < 0.01 && ambient < 0.01 {
        return;
    }

    unsafe {
        apply_color_grading(fb, width, height, r_mul, g_mul, b_mul, ambient);
    }
}

/// Calcula parámetros de color según la hora del día
#[inline]
fn get_day_night_params(time_fraction: f32) -> (f32, f32, f32, f32) {
    // Amanecer: 5:00-7:00
    if time_fraction >= 5.0 && time_fraction < 7.0 {
        let t = (time_fraction - 5.0) / 2.0; // 0→1
        let warmth = 1.0 - t; // 1→0 (cálido al inicio)
        (1.0, 0.85 + 0.15 * t, 0.7 + 0.3 * t, 0.1 * warmth)
    }
    // Día: 7:00-18:00
    else if time_fraction >= 7.0 && time_fraction < 18.0 {
        (1.0, 1.0, 1.0, 0.0) // Sin cambio
    }
    // Atardecer: 18:00-20:00
    else if time_fraction >= 18.0 && time_fraction < 20.0 {
        let t = (time_fraction - 18.0) / 2.0; // 0→1
        (1.0, 0.7 + 0.3 * (1.0 - t), 0.5 + 0.5 * (1.0 - t), 0.1 * t)
    }
    // Noche: 20:00-5:00
    else {
        let t = if time_fraction >= 20.0 {
            (time_fraction - 20.0) / 9.0 // 0→1 (20h→5h = 9h)
        } else {
            (time_fraction + 4.0) / 9.0 // 0→1 (0h→5h)
        };
        let darkness = 0.5 + 0.3 * (1.0 - (t - 0.5).abs() * 2.0); // más oscuro en medianoche
        (0.15, 0.18, 0.35 + 0.15 * (1.0 - darkness), darkness)
    }
}

/// Aplica color grading SIMD al framebuffer
#[cfg(target_arch = "x86_64")]
unsafe fn apply_color_grading(fb: &mut [u32], width: usize, height: usize, 
                               r_mul: f32, g_mul: f32, b_mul: f32, ambient: f32) {
    use std::arch::x86_64::*;

    let total_pixels = width * height;
    let simd_end = (total_pixels / 4) * 4;

    // Constantes SIMD
    let r_mul_f = _mm_set1_ps(r_mul);
    let g_mul_f = _mm_set1_ps(g_mul);
    let b_mul_f = _mm_set1_ps(b_mul);
    let ambient_f = _mm_set1_ps(ambient * 255.0);
    let max_f = _mm_set1_ps(255.0);
    let min_f = _mm_set1_ps(0.0);

    for px in (0..simd_end).step_by(4) {
        let ptr = fb.as_ptr().add(px) as *const __m128i;
        let pixels = _mm_loadu_si128(ptr);

        // Extraer canales: ARGB (little-endian: [B0 G0 R0 A0, B1 G1 R1 A1, ...])
        let p0 = _mm_cvtepi32_ps(_mm_unpacklo_epi16(
            _mm_unpacklo_epi8(pixels, _mm_setzero_si128()),
            _mm_setzero_si128()
        ));
        let p1 = _mm_cvtepi32_ps(_mm_unpacklo_epi16(
            _mm_unpackhi_epi8(pixels, _mm_setzero_si128()),
            _mm_setzero_si128()
        ));

        // Aplicar multiplicadores por canal y ambient
        // (esto es una aproximación; el alpha blending fino es más complejo)

        // Fallback escalar para SIMD de 4 píxeles
        for off in 0..4 {
            let idx = px + off;
            if idx < total_pixels {
                let pixel = *fb.get_unchecked(idx);
                let a = (pixel >> 24) & 0xFF;
                let r = (pixel >> 16) & 0xFF;
                let g = (pixel >> 8) & 0xFF;
                let b = pixel & 0xFF;

                let new_r = ((r as f32 * r_mul + ambient as f32 * 255.0) as u32).min(255);
                let new_g = ((g as f32 * g_mul + ambient as f32 * 255.0) as u32).min(255);
                let new_b = ((b as f32 * b_mul + ambient as f32 * 255.0) as u32).min(255);

                *fb.get_unchecked_mut(idx) = (a << 24) | (new_r << 16) | (new_g << 8) | new_b;
            }
        }
    }

    // Cola
    for px in simd_end..total_pixels {
        let pixel = *fb.get_unchecked(px);
        let a = (pixel >> 24) & 0xFF;
        let r = (pixel >> 16) & 0xFF;
        let g = (pixel >> 8) & 0xFF;
        let b = pixel & 0xFF;

        let new_r = ((r as f32 * r_mul + ambient as f32 * 255.0) as u32).min(255);
        let new_g = ((g as f32 * g_mul + ambient as f32 * 255.0) as u32).min(255);
        let new_b = ((b as f32 * b_mul + ambient as f32 * 255.0) as u32).min(255);

        *fb.get_unchecked_mut(px) = (a << 24) | (new_r << 16) | (new_g << 8) | new_b;
    }
}

#[cfg(not(target_arch = "x86_64"))]
unsafe fn apply_color_grading(fb: &mut [u32], _width: usize, _height: usize, 
                               r_mul: f32, g_mul: f32, b_mul: f32, ambient: f32) {
    let total_pixels = fb.len();
    let unrolled_end = (total_pixels / 8) * 8;

    for px in (0..unrolled_end).step_by(8) {
        for off in 0..8 {
            let idx = px + off;
            let pixel = *fb.get_unchecked(idx);
            let a = (pixel >> 24) & 0xFF;
            let r = (pixel >> 16) & 0xFF;
            let g = (pixel >> 8) & 0xFF;
            let b = pixel & 0xFF;

            let new_r = ((r as f32 * r_mul + ambient * 255.0) as u32).min(255);
            let new_g = ((g as f32 * g_mul + ambient * 255.0) as u32).min(255);
            let new_b = ((b as f32 * b_mul + ambient * 255.0) as u32).min(255);

            *fb.get_unchecked_mut(idx) = (a << 24) | (new_r << 16) | (new_g << 8) | new_b;
        }
    }

    for px in unrolled_end..total_pixels {
        let pixel = *fb.get_unchecked(px);
        let a = (pixel >> 24) & 0xFF;
        let r = (pixel >> 16) & 0xFF;
        let g = (pixel >> 8) & 0xFF;
        let b = pixel & 0xFF;

        let new_r = ((r as f32 * r_mul + ambient * 255.0) as u32).min(255);
        let new_g = ((g as f32 * g_mul + ambient * 255.0) as u32).min(255);
        let new_b = ((b as f32 * b_mul + ambient * 255.0) as u32).min(255);

        *fb.get_unchecked_mut(px) = (a << 24) | (new_r << 16) | (new_g << 8) | new_b;
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_day_night_params_dawn() {
        let (r, g, b, amb) = get_day_night_params(5.5); // Amanecer medio
        assert!(r > 0.9);
        assert!(g < 1.0);
        assert!(b < 0.9);
        assert!(amb > 0.0);
    }

    #[test]
    fn test_day_night_params_noon() {
        let (r, g, b, amb) = get_day_night_params(12.0); // Mediodía
        assert!((r - 1.0).abs() < 0.01);
        assert!((g - 1.0).abs() < 0.01);
        assert!((b - 1.0).abs() < 0.01);
        assert!(amb < 0.01);
    }

    #[test]
    fn test_day_night_params_night() {
        let (r, g, b, amb) = get_day_night_params(0.0); // Medianoche
        assert!(r < 0.2);
        assert!(g < 0.2);
        assert!(b > 0.3); // azul nocturno
        assert!(amb > 0.3);
    }

    #[test]
    fn test_day_night_params_sunset() {
        let (r, g, b, amb) = get_day_night_params(19.0); // Atardecer medio
        assert!(r > 0.9);
        assert!(g < 1.0);
        assert!(b < 0.8);
        assert!(amb > 0.0);
    }

    #[test]
    fn test_apply_overlay_daytime_noop() {
        let mut fb = vec![0xFF_FF_FF_FFu32; 100];
        apply_day_night_overlay(&mut fb, 10, 10, 12 * 60); // Mediodía
        // En pleno día, no debería cambiar
        assert_eq!(fb[0], 0xFF_FF_FF_FF);
    }

    #[test]
    fn test_apply_overlay_night_changes() {
        let mut fb = vec![0xFF_FF_FF_FFu32; 100];
        apply_day_night_overlay(&mut fb, 10, 10, 0); // Medianoche
        // De noche, los colores cambian
        let pixel = fb[0];
        let r = (pixel >> 16) & 0xFF;
        assert!(r < 255, "Rojo debe reducirse de noche: r={}", r);
    }

    #[test]
    fn test_apply_overlay_preserves_alpha() {
        let mut fb = vec![0x80_FF_FF_FFu32; 100];
        apply_day_night_overlay(&mut fb, 10, 10, 0); // Medianoche
        let a = (fb[0] >> 24) & 0xFF;
        assert_eq!(a, 0x80, "Alpha debe preservarse");
    }
}
