// TÉCNICA COMÚN #1: Object Pooling Masivo
//
// En lugar de usar `new` o `Vec::push` en tiempo de ejecución,
// preasignamos 10,000 entidades vacías durante la carga.
// El pool recicla entidades: cuando una "muere", vuelve al pool
// en lugar de liberar memoria. Esto elimina la presión sobre
// el allocator del sistema y mantiene los datos en caché.
//
// TÉCNICA AVANZADA #4 (juegos): Bump Allocators por Frame
// Para datos temporales de un solo frame, usamos bump allocation
// en el módulo bump_alloc.rs

/// Marcador de entidad en el pool
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PoolHandle(u32);

impl PoolHandle {
    pub const INVALID: Self = PoolHandle(u32::MAX);

    #[inline(always)]
    pub fn index(self) -> usize {
        self.0 as usize
    }

    #[inline(always)]
    pub fn is_valid(self) -> bool {
        self.0 != u32::MAX
    }
}

/// Estado de un slot en el pool
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum SlotState {
    Free,
    Alive,
}

/// Pool de entidades genéricas preasignadas
/// [TC#2]: Capacidad pre-reservada en Vec::with_capacity
pub struct EntityPool {
    /// Marcadores de estado: Free o Alive
    states: Vec<SlotState>,
    /// Índices libres para reciclaje rápido (stack LIFO)
    free_list: Vec<u32>,
    /// Contador de entidades vivas
    alive_count: usize,
}

impl EntityPool {
    /// Crea un pool con `capacity` slots preasignados.
    /// Toda la memoria se reserva de una vez.
    pub fn new(capacity: usize) -> Self {
        // [TC#2]: Pre-reserva de capacidad exacta
        let mut states = Vec::with_capacity(capacity);
        let mut free_list = Vec::with_capacity(capacity);

        // Inicializar todos los slots como libres
        states.resize(capacity, SlotState::Free);

        // Llenar la free_list en orden inverso para LIFO
        for i in (0..capacity as u32).rev() {
            free_list.push(i);
        }

        EntityPool {
            states,
            free_list,
            alive_count: 0,
        }
    }

    /// Adquiere un slot del pool. Retorna PoolHandle::INVALID si está lleno.
    #[inline(always)]
    pub fn acquire(&mut self) -> PoolHandle {
        match self.free_list.pop() {
            Some(idx) => {
                // SAFETY: idx viene de free_list, garantizado dentro de bounds
                unsafe {
                    *self.states.get_unchecked_mut(idx as usize) = SlotState::Alive;
                }
                self.alive_count += 1;
                PoolHandle(idx)
            }
            None => PoolHandle::INVALID,
        }
    }

    /// Libera un slot de vuelta al pool
    #[inline(always)]
    pub fn release(&mut self, handle: PoolHandle) {
        if !handle.is_valid() {
            return;
        }
        let idx = handle.index();
        if idx < self.states.len() {
            // SAFETY: idx validado contra len
            unsafe {
                if *self.states.get_unchecked(idx) == SlotState::Alive {
                    *self.states.get_unchecked_mut(idx) = SlotState::Free;
                    self.free_list.push(idx as u32);
                    self.alive_count -= 1;
                }
            }
        }
    }

    /// Verifica si un handle sigue siendo válido
    #[inline(always)]
    pub fn is_alive(&self, handle: PoolHandle) -> bool {
        if !handle.is_valid() {
            return false;
        }
        let idx = handle.index();
        idx < self.states.len() && self.states[idx as usize] == SlotState::Alive
    }

    /// Número de entidades vivas
    #[inline(always)]
    pub fn alive_count(&self) -> usize {
        self.alive_count
    }

    /// Capacidad máxima del pool
    #[inline(always)]
    pub fn capacity(&self) -> usize {
        self.states.len()
    }

    /// Slots libres disponibles
    #[inline(always)]
    pub fn free_count(&self) -> usize {
        self.free_list.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_acquire_release() {
        let mut pool = EntityPool::new(100);
        assert_eq!(pool.free_count(), 100);
        assert_eq!(pool.alive_count(), 0);

        let h1 = pool.acquire();
        assert!(h1.is_valid());
        assert_eq!(pool.alive_count(), 1);
        assert_eq!(pool.free_count(), 99);

        let h2 = pool.acquire();
        assert!(h2.is_valid());
        assert_ne!(h1, h2);

        pool.release(h1);
        assert_eq!(pool.alive_count(), 1);
        assert_eq!(pool.free_count(), 99);
        assert!(!pool.is_alive(h1));
        assert!(pool.is_alive(h2));

        // Adquirir de nuevo debe reciclar h1 (LIFO: último liberado = primero readquirido)
        let h3 = pool.acquire();
        assert_eq!(h3, h1);
    }

    #[test]
    fn test_pool_exhaustion() {
        let mut pool = EntityPool::new(5);
        for _ in 0..5 {
            assert!(pool.acquire().is_valid());
        }
        assert_eq!(pool.acquire(), PoolHandle::INVALID);
        assert_eq!(pool.free_count(), 0);
    }

    #[test]
    fn test_release_invalid() {
        let mut pool = EntityPool::new(10);
        // No debe panic
        pool.release(PoolHandle::INVALID);
        pool.release(PoolHandle(99999));
    }

    #[test]
    fn test_double_release() {
        let mut pool = EntityPool::new(10);
        let h = pool.acquire();
        pool.release(h);
        pool.release(h); // No debe panic, solo ignora
        assert_eq!(pool.free_count(), 10);
    }

    #[test]
    fn test_pool_capacity() {
        let pool = EntityPool::new(1000);
        assert_eq!(pool.capacity(), 1000);
    }

    #[test]
    fn test_pool_lifo_order() {
        let mut pool = EntityPool::new(10);
        // Adquirir 5 handles (el free_list inicial está en [9,8,7,6,5,4,3,2,1,0])
        // así que pop() da: 9, 8, 7, 6, 5
        let handles: Vec<_> = (0..5).map(|_| pool.acquire()).collect();
        // handles = [PoolHandle(9), PoolHandle(8), PoolHandle(7), PoolHandle(6), PoolHandle(5)]

        // Liberar en orden inverso al de adquisición: [5, 6, 7, 8, 9]
        for h in handles.iter().rev() {
            pool.release(*h);
        }
        // free_list queda: [5, 6, 7, 8, 9]

        // Readquirir: LIFO -> [9, 8, 7, 6, 5] (mismo orden de adquisición original)
        for h in handles.iter() {
            assert_eq!(pool.acquire(), *h,
                "LIFO debe restaurar el orden de adquisición original");
        }
    }
}
