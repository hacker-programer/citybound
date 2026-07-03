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

use std::sync::atomic::{AtomicUsize, Ordering};

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

static mut RNG_POOL: RngPool = RngPool { data: [0.0_f32; RNG_POOL_SIZE] };
static RNG_INDEX: AtomicUsize = AtomicUsize::new(0);
static RNG_POOL_READY: AtomicUsize = AtomicUsize::new(0); // 0=no init, 1=ready

// ---------------------------------------------------------------------------
// INICIALIZACIÓN
// ---------------------------------------------------------------------------

/// Inicializa el pool de RNG durante la carga.
/// SAFETY: Debe llamarse una vez antes de cualquier llamada a rng_fast().
pub fn init_rng_pool(seed: u64) {
    if RNG_POOL_READY.load(Ordering::Acquire) == 1 {
        return;
    }

    let mut state = seed;

    unsafe {
        for i in 0..RNG_POOL_SIZE {
            // SplitMix64: rápido, buen período, pasa DieHard
            state = state.wrapping_add(0x9E3779B97F4A7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
            z = z ^ (z >> 31);

            // Convertir a f32 en [0, 1)
            RNG_POOL.data[i] = (z as f32) / (u64::MAX as f32 + 1.0);
        }
    }

    RNG_POOL_READY.store(1, Ordering::Release);
}

// ---------------------------------------------------------------------------
// ACCESO RÁPIDO
// ---------------------------------------------------------------------------

/// Obtiene un valor aleatorio del pool pre-generado.
/// Cicla sobre el array con acceso atómico lock-free [TI#4].
#[inline(always)]
pub fn rng_fast() -> f32 {
    // [TI#4]: Lock-free con Ordering::Relaxed - sin barreras de memoria
    let idx = RNG_INDEX.fetch_add(1, Ordering::Relaxed) % RNG_POOL_SIZE;

    // SAFETY: el pool está inicializado antes de cualquier llamada
    // y solo es de lectura después de init. Usamos addr_of! para evitar
    // crear referencias compartidas al static mut.
    unsafe {
        let ptr = std::ptr::addr_of!(RNG_POOL.data) as *const f32;
        *ptr.add(idx)
    }
}

#[inline(always)]
pub fn rng_range(min: f32, max: f32) -> f32 {
    min + rng_fast() * (max - min)
}

/// Obtiene un valor entero en rango [0, max)
#[inline(always)]
pub fn rng_int(max: u32) -> u32 {
    (rng_fast() * max as f32) as u32
}

/// Probabilidad: retorna true con probabilidad p (0.0 a 1.0)
#[inline(always)]
pub fn rng_chance(p: f32) -> bool {
    rng_fast() < p
}

// ---------------------------------------------------------------------------
// GENERADOR DETERMINISTA DE RESPALDO (para valores no críticos)
// ---------------------------------------------------------------------------

/// SplitMix64 rápido inline (sin dependencia de crate).
/// Para casos donde no se puede usar el pool (ej: generación procedural
/// que requiere secuencia determinista basada en coordenadas).
#[inline(always)]
pub fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}
/// Versión f32 de splitmix64
#[inline(always)]
pub fn splitmix64_f32(state: &mut u64) -> f32 {
    (splitmix64(state) as f32) / (u64::MAX as f32 + 1.0)
}

/// Fuerza la carga del pool de RNG en caché L1.
pub fn warm_rng_cache() {
    let base: *const f32 = unsafe { std::ptr::addr_of!(RNG_POOL.data) as *const f32 };
    unsafe {
        for i in (0..RNG_POOL_SIZE).step_by(16) {
            let _val = std::ptr::read_volatile(base.add(i));
            std::ptr::read_volatile(base.add(i + 4));
            std::ptr::read_volatile(base.add(i + 8));
            std::ptr::read_volatile(base.add(i + 12));
        }
    }
}

// ---------------------------------------------------------------------------
// TESTS


// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rng_pool_init() {
        init_rng_pool(42);
        assert_eq!(RNG_POOL_READY.load(Ordering::Acquire), 1);
    }

    #[test]
    fn test_rng_fast_range() {
        init_rng_pool(42);
        for _ in 0..10000 {
            let val = rng_fast();
            assert!(val >= 0.0 && val < 1.0, "Valor fuera de [0,1): {}", val);
        }
    }

    #[test]
    fn test_rng_range() {
        init_rng_pool(42);
        for _ in 0..10000 {
            let val = rng_range(5.0, 10.0);
            assert!(val >= 5.0 && val < 10.0, "rng_range fuera de bounds: {}", val);
        }
    }

    #[test]
    fn test_rng_int() {
        init_rng_pool(42);
        for _ in 0..10000 {
            let val = rng_int(100);
            assert!(val < 100, "rng_int fuera de bounds: {}", val);
        }
    }

    #[test]
    fn test_rng_chance() {
        init_rng_pool(42);
        let mut true_count = 0;
        for _ in 0..100000 {
            if rng_chance(0.3) {
                true_count += 1;
            }
        }
        // ~30% de 100000 = 30000, con margen amplio
        assert!(true_count > 25000 && true_count < 35000,
            "rng_chance(0.3): {} true de 100000 (esperado ~30000)", true_count);
    }

    #[test]
    fn test_splitmix64_determinism() {
        let mut s1: u64 = 12345;
        let mut s2: u64 = 12345;

        for _ in 0..100 {
            assert_eq!(splitmix64(&mut s1), splitmix64(&mut s2));
        }
    }

    #[test]
    fn test_rng_pool_double_init_safe() {
        init_rng_pool(42);
        init_rng_pool(99); // No debe panic ni cambiar datos
        assert_eq!(RNG_POOL_READY.load(Ordering::Acquire), 1);
    }

    #[test]
    fn test_warm_rng_cache() {
        init_rng_pool(42);
        warm_rng_cache(); // No debe panic
    }
}
