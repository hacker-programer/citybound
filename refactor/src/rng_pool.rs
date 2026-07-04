// RNG Pool Pre-generado
//
// TÉCNICA COMÚN #22 (juegos): Inicialización Determinista de RNG
// En lugar de llamar al generador de números aleatorios en cada frame,
// pre-generamos un bloque de 4096 valores aleatorios durante la carga
// e iteramos sobre ellos cíclicamente. Esto elimina el costo de
// generación de números aleatorios en tiempo de ejecución.
//
// También implementamos un generador rápido splitmix64 para valores
// que no están en el pool (baja frecuencia).
//
// TÉCNICA COMÚN #25 (juegos): Uso exclusivo de f32
//
// [FIX] static mut reemplazado por OnceLock para thread-safety y
// eliminación de undefined behavior.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Tamaño del pool de RNG: 4096 valores (16 KB en L1)
/// Suficiente para 4096 consultas aleatorias sin repetir ciclo.
pub const RNG_POOL_SIZE: usize = 4096;

/// Pool de valores aleatorios pre-generados alineado a 64B [TA#9]
#[repr(align(64))]
struct RngPool {
    data: [f32; RNG_POOL_SIZE],
}

static RNG_POOL: OnceLock<RngPool> = OnceLock::new();
static RNG_INDEX: AtomicUsize = AtomicUsize::new(0);

// ---------------------------------------------------------------------------
// INICIALIZACIÓN
// ---------------------------------------------------------------------------

/// Inicializa el pool de RNG durante la carga.
/// Thread-safe: puede llamarse múltiples veces, solo inicializa una vez.
pub fn init_rng_pool(seed: u64) {
    RNG_POOL.get_or_init(|| {
        let mut data = [0.0_f32; RNG_POOL_SIZE];
        let mut state = seed;

        for i in 0..RNG_POOL_SIZE {
            // SplitMix64: rápido, buen período, pasa DieHard
            state = state.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z = z ^ (z >> 31);
            // Normalizar a [0, 1)
            data[i] = (z as u32 as f64 / u32::MAX as f64) as f32;
        }

        RngPool { data }
    });

    // Reset index
    RNG_INDEX.store(0, Ordering::Release);
}

// ---------------------------------------------------------------------------
// ACCESO RÁPIDO
// ---------------------------------------------------------------------------

/// Obtiene un valor aleatorio del pool pre-generado.
/// Avance cíclico: cuando llega al final, vuelve al inicio.
/// Thread-safe via AtomicUsize con Ordering::Relaxed.
#[inline(always)]
pub fn rng_fast() -> f32 {
    let pool = RNG_POOL.get().expect("RNG_POOL no inicializado. Llama a init_rng_pool() primero.");
    let idx = RNG_INDEX.fetch_add(1, Ordering::Relaxed) % RNG_POOL_SIZE;
    pool.data[idx]
}

/// Genera un valor aleatorio en [0, max) usando el pool.
#[inline(always)]
pub fn rng_range(max: f32) -> f32 {
    rng_fast() * max
}

/// Genera un valor aleatorio en [min, max) usando el pool.
#[inline(always)]
pub fn rng_range_inclusive(min: f32, max: f32) -> f32 {
    min + rng_fast() * (max - min)
}

/// Genera un entero aleatorio en [0, max) usando el pool.
#[inline(always)]
pub fn rng_usize(max: usize) -> usize {
    (rng_fast() * max as f32) as usize
}

/// Devuelve true con la probabilidad dada (0.0 - 1.0).
#[inline(always)]
pub fn rng_chance(probability: f32) -> bool {
    rng_fast() < probability
}
}

// ---------------------------------------------------------------------------
// SPLITMIX64 DIRECTO (para valores que no deberían consumir el pool)
// ---------------------------------------------------------------------------

/// Generador SplitMix64 directo que avanza su propio estado.
/// Útil para valores que no deberían consumir el pool cíclico
/// (ej: seeds, inicialización de entidades).
#[inline]
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Devuelve un f32 en [0, 1) usando splitmix64.
#[inline]
pub fn splitmix64_f32(state: &mut u64) -> f32 {
    (splitmix64(state) as u32 as f64 / u32::MAX as f64) as f32
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_and_read() {
        init_rng_pool(12345);
        let v = rng_fast();
        assert!(v >= 0.0 && v < 1.0);
    }

    #[test]
    fn test_range() {
        init_rng_pool(12345);
        for _ in 0..100 {
            let v = rng_range(10.0);
            assert!(v >= 0.0 && v < 10.0);
        }
    }

    #[test]
    fn test_range_inclusive() {
        init_rng_pool(12345);
        for _ in 0..100 {
            let v = rng_range_inclusive(5.0, 15.0);
            assert!(v >= 5.0 && v < 15.0);
        }
    }

    #[test]
    fn test_splitmix64() {
        let mut state = 42;
        let v1 = splitmix64(&mut state);
        let v2 = splitmix64(&mut state);
        assert_ne!(v1, v2);
    }

    #[test]
    fn test_double_init() {
        init_rng_pool(999);
        let a = rng_fast();
        init_rng_pool(999);
        let b = rng_fast();
        // Después de reinicializar, el índice vuelve a 0
        // El primer valor debería ser el mismo que a
        assert!((a - b).abs() < 0.001);
    }
}
