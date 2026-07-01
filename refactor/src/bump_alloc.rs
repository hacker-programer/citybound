// TÉCNICA AVANZADA #4 (juegos): Bump Allocator por Frame
//
// Un bump allocator es el asignador más simple y rápido posible:
// mantiene un puntero que solo avanza. Al final del frame,
// el puntero se resetea a cero, "liberando" toda la memoria
// en una sola operación. Cero free() individual, cero fragmentación.
//
// Uso: datos temporales del frame actual que no necesitan persistir.
//
// TÉCNICA INNOVADORA #3 (juegos): Este allocator está diseñado
// específicamente para el hardware objetivo (Pentium 4GB).
// El bloque de memoria se alinea a página (4KB) para aprovechar
// el TLB (Translation Lookaside Buffer).

use std::alloc::{alloc_zeroed, dealloc, Layout};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Tamaño del bump allocator: 16MB es suficiente para datos temporales
/// de un frame sin presión de memoria en 4GB
const BUMP_SIZE: usize = 16 * 1024 * 1024; // 16 MB
/// Alineación a página para eficiencia de TLB
const BUMP_ALIGN: usize = 4096;

/// Bump Allocator thread-local para datos del frame actual
pub struct BumpAllocator {
    /// Bloque de memoria preasignado
    memory: NonNull<u8>,
    /// Offset actual (solo avanza)
    offset: AtomicUsize,
    /// Tamaño total
    size: usize,
}

// SAFETY: El allocator es thread-safe gracias a AtomicUsize
unsafe impl Send for BumpAllocator {}
unsafe impl Sync for BumpAllocator {}

impl BumpAllocator {
    /// Crea un nuevo bump allocator con memoria preasignada
    pub fn new() -> Self {
        let layout = Layout::from_size_align(BUMP_SIZE, BUMP_ALIGN)
            .expect("Layout inválido para bump allocator");

        // SAFETY: Layout está validado, alloc_zeroed es segura
        let memory = unsafe {
            let ptr = alloc_zeroed(layout);
            NonNull::new(ptr).expect("Fallo al asignar bump allocator")
        };

        BumpAllocator {
            memory,
            offset: AtomicUsize::new(0),
            size: BUMP_SIZE,
        }
    }

    /// Asigna `size` bytes con `align` requerida.
    /// Retorna None si no hay espacio.
    #[inline]
    pub fn allocate(&self, size: usize, align: usize) -> Option<NonNull<u8>> {
        let current = self.offset.load(Ordering::Relaxed);
        // Calcular dirección alineada
        let aligned = (current + align - 1) & !(align - 1);
        let next = aligned + size;

        if next > self.size {
            return None; // Sin espacio
        }

        // Intentar reservar el espacio con CAS
        if self
            .offset
            .compare_exchange(current, next, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            // SAFETY: aligned + size está verificado dentro del bloque
            unsafe {
                let ptr = self.memory.as_ptr().add(aligned);
                Some(NonNull::new_unchecked(ptr))
            }
        } else {
            // Otro thread ganó la reserva, reintentar recursivamente
            self.allocate(size, align)
        }
    }

    /// Resetea el allocator al inicio del bloque.
    /// Debe llamarse UNA vez por frame, al inicio.
    #[inline]
    pub fn reset(&self) {
        // Técnica innovadora: reset atómico, sin locks
        self.offset.store(0, Ordering::Release);
    }

    /// Bytes usados actualmente
    #[inline]
    pub fn used(&self) -> usize {
        self.offset.load(Ordering::Relaxed)
    }

    /// Bytes libres
    #[inline]
    pub fn free(&self) -> usize {
        self.size - self.used()
    }
}

impl Drop for BumpAllocator {
    fn drop(&mut self) {
        let layout = Layout::from_size_align(BUMP_SIZE, BUMP_ALIGN)
            .expect("Layout inválido en drop");
        // SAFETY: memoria fue asignada con este layout en new()
        unsafe {
            dealloc(self.memory.as_ptr(), layout);
        }
    }
}

// ---------------------------------------------------------------------------
// BumpAllocator global para el frame actual
// Técnica de optimización: variable global mutable controlada
// ---------------------------------------------------------------------------
static mut FRAME_ALLOCATOR: Option<BumpAllocator> = None;

/// Inicializa el bump allocator global. Llamar una vez en main().
pub fn init_frame_allocator() {
    unsafe {
        FRAME_ALLOCATOR = Some(BumpAllocator::new());
    }
}

/// Obtiene referencia al bump allocator del frame actual
#[inline(always)]
pub fn frame_allocator() -> &'static BumpAllocator {
    // SAFETY: inicializado en main() antes de cualquier uso
    unsafe { FRAME_ALLOCATOR.as_ref().expect("BumpAllocator no inicializado") }
}

/// Resetea el bump allocator. Llamar al inicio de cada frame.
#[inline(always)]
pub fn reset_frame() {
    frame_allocator().reset();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bump_allocate() {
        let bump = BumpAllocator::new();
        assert_eq!(bump.used(), 0);

        let ptr1 = bump.allocate(1024, 8);
        assert!(ptr1.is_some());
        assert!(bump.used() >= 1024);

        let ptr2 = bump.allocate(512, 8);
        assert!(ptr2.is_some());

        // Verificar que las direcciones no se solapan
        let addr1 = ptr1.unwrap().as_ptr() as usize;
        let addr2 = ptr2.unwrap().as_ptr() as usize;
        assert!(addr2 >= addr1 + 1024);
    }

    #[test]
    fn test_bump_reset() {
        let bump = BumpAllocator::new();
        bump.allocate(1024 * 1024, 8);
        assert!(bump.used() >= 1024 * 1024);

        bump.reset();
        assert_eq!(bump.used(), 0);

        // Debe poder asignar de nuevo
        assert!(bump.allocate(1024, 8).is_some());
    }

    #[test]
    fn test_bump_alignment() {
        let bump = BumpAllocator::new();
        for align in &[4, 8, 16, 32, 64, 128, 256, 512, 1024, 4096] {
            let ptr = bump.allocate(128, *align).unwrap();
            assert_eq!(ptr.as_ptr() as usize % align, 0, "Fallo alineación {}", align);
        }
    }

    #[test]
    fn test_bump_exhaustion() {
        let bump = BumpAllocator::new();
        // Intentar asignar más que BUMP_SIZE
        let result = bump.allocate(BUMP_SIZE + 1, 8);
        assert!(result.is_none());
    }

    #[test]
    fn test_bump_large_allocation() {
        let bump = BumpAllocator::new();
        // Asignar cerca del límite
        let result = bump.allocate(BUMP_SIZE - 4096, 8);
        assert!(result.is_some());

        // Ya casi no queda espacio
        let result2 = bump.allocate(8192, 8);
        assert!(result2.is_none());
    }
}
