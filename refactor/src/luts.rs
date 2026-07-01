// TÉCNICA COMÚN #5: Look-Up Tables (LUTs) Trigonométricas
//
// Precalcula todos los valores de seno y coseno en arrays estáticos
// durante la inicialización. Acceder por índice es ~10x más rápido
// que llamar a f32::sin() en CPUs antiguas como Pentium.
//
// Resolución: 3600 entradas (0.1° de precisión), suficiente para renderizado 2D.
// Memoria: 3600 * 4 bytes * 2 = ~28KB, cabe en caché L1.

#![allow(dead_code)]\n\npub const TRIG_RESOLUTION: usize = 3600;
pub const RAD_TO_IDX: f32 = (TRIG_RESOLUTION as f32) / (2.0 * std::f32::consts::PI);

// [TA#9]: Wrapper alineado a 64 bytes para arrays LUT
#[repr(align(64))]
struct AlignedLut {
    data: [f32; TRIG_RESOLUTION],
}

// Usamos static mut con acceso vía punteros crudos (Rust 2024 compatible)
// SAFETY: Inicializadas en init_trig_luts() antes de cualquier acceso de lectura,
// y nunca modificadas después. El acceso concurrente de solo-lectura es seguro
// en todas las arquitecturas objetivo.
static mut SIN_LUT: AlignedLut = AlignedLut { data: [0.0_f32; TRIG_RESOLUTION] };
static mut COS_LUT: AlignedLut = AlignedLut { data: [0.0_f32; TRIG_RESOLUTION] };

use std::sync::atomic::{AtomicBool, Ordering};

static LUT_INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Inicializa las LUTs trigonométricas. Llamar una vez en main().
pub fn init_trig_luts() {
    if LUT_INITIALIZED.load(Ordering::Acquire) {
        return;
    }

    // SAFETY: Acceso exclusivo durante inicialización, single-thread.
    // Después de init_trig_luts(), las LUTs nunca se modifican, por lo que
    // las lecturas concurrentes posteriores son seguras.
    unsafe {
        let sin_ptr = std::ptr::addr_of_mut!(SIN_LUT);
        let cos_ptr = std::ptr::addr_of_mut!(COS_LUT);
        for i in 0..TRIG_RESOLUTION {
            let angle = (i as f32) * (2.0 * std::f32::consts::PI) / (TRIG_RESOLUTION as f32);
            (*sin_ptr).data[i] = libm::sinf(angle);
            (*cos_ptr).data[i] = libm::cosf(angle);
        }
    }

    LUT_INITIALIZED.store(true, Ordering::Release);
}

/// Seno rápido usando LUT. Ángulo en radianes.
#[inline(always)]
pub fn sin_fast(angle: f32) -> f32 {
    let normalized = angle.rem_euclid(2.0 * std::f32::consts::PI);
    let idx = (normalized * RAD_TO_IDX) as usize % TRIG_RESOLUTION;
    // SAFETY: LUT ya inicializada (init_trig_luts llamado en main), idx en [0, TRIG_RESOLUTION)
    unsafe {
        let lut_ptr = std::ptr::addr_of!(SIN_LUT);
        *(*lut_ptr).data.as_ptr().add(idx)
    }
}

/// Coseno rápido usando LUT. Ángulo en radianes.
#[inline(always)]
pub fn cos_fast(angle: f32) -> f32 {
    let normalized = angle.rem_euclid(2.0 * std::f32::consts::PI);
    let idx = (normalized * RAD_TO_IDX) as usize % TRIG_RESOLUTION;
    // SAFETY: LUT ya inicializada, idx validado
    unsafe {
        let lut_ptr = std::ptr::addr_of!(COS_LUT);
        *(*lut_ptr).data.as_ptr().add(idx)
    }
}

/// Sin bounds check para hot paths.
/// SAFETY: caller debe garantizar idx < TRIG_RESOLUTION y LUT inicializada
#[inline(always)]
pub unsafe fn sin_unchecked(idx: usize) -> f32 {
    let lut_ptr = std::ptr::addr_of!(SIN_LUT);
    *(*lut_ptr).data.as_ptr().add(idx)
}

/// Sin bounds check para hot paths.
/// SAFETY: caller debe garantizar idx < TRIG_RESOLUTION y LUT inicializada
#[inline(always)]
pub unsafe fn cos_unchecked(idx: usize) -> f32 {
    let lut_ptr = std::ptr::addr_of!(COS_LUT);
    *(*lut_ptr).data.as_ptr().add(idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lut_initialization() {
        init_trig_luts();
        assert!(LUT_INITIALIZED.load(Ordering::Acquire));
    }

    #[test]
    fn test_sin_fast_accuracy() {
        init_trig_luts();
        let eps: f32 = 0.01;
        assert!((sin_fast(0.0) - 0.0).abs() < eps);
        assert!((sin_fast(std::f32::consts::PI / 2.0) - 1.0).abs() < eps);
        assert!((sin_fast(std::f32::consts::PI) - 0.0).abs() < eps);
        assert!((sin_fast(3.0 * std::f32::consts::PI / 2.0) - (-1.0)).abs() < eps);
    }

    #[test]
    fn test_cos_fast_accuracy() {
        init_trig_luts();
        let eps: f32 = 0.01;
        assert!((cos_fast(0.0) - 1.0).abs() < eps);
        assert!((cos_fast(std::f32::consts::PI / 2.0) - 0.0).abs() < eps);
        assert!((cos_fast(std::f32::consts::PI) - (-1.0)).abs() < eps);
    }

    #[test]
    fn test_sin_cos_identity() {
        init_trig_luts();
        let eps: f32 = 0.02;
        for i in 0..100 {
            let angle = (i as f32) * 0.1;
            let val = sin_fast(angle).powi(2) + cos_fast(angle).powi(2);
            assert!((val - 1.0).abs() < eps, "sin²+cos²={} at angle={}", val, angle);
        }
    }
}
