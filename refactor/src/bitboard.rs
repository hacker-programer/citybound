// Bitboards para Colisiones en Grilla
//
// TÉCNICA INNOVADORA #6 (juegos): Bitboards
// Representa ocupación de celdas como bits en enteros de 64 bits.
// Las comprobaciones de colisión se vuelven operaciones bit a bit
// de O(1) en lugar de búsquedas espaciales O(log N).
//
// Un solo u64 puede representar 64 celdas (8x8).
// Para la grilla de 128x128, usamos 16x16 = 256 u64 = 2KB.
// Esto cabe completamente en caché L1.
//
// Operaciones soportadas:
// - set/clear/test de bits individuales
// - AND/OR/XOR entre bitboards (colisiones masivas)
// - conteo de población (cuántas celdas ocupadas)
// - escaneo de bits (encontrar primera celda ocupada)

// ---------------------------------------------------------------------------
// TIPOS PRINCIPALES
// ---------------------------------------------------------------------------

/// Bitboard de 64 bits para un bloque de 8x8 celdas
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct Bitboard64(pub u64);

impl Bitboard64 {
    pub const EMPTY: Self = Bitboard64(0);
    pub const FULL: Self = Bitboard64(u64::MAX);

    /// Setea un bit en (x, y) dentro del bloque 8x8
    #[inline(always)]
    pub fn set(&mut self, x: u8, y: u8) {
        debug_assert!(x < 8 && y < 8);
        self.0 |= 1u64 << (y * 8 + x);
    }

    /// Limpia un bit
    #[inline(always)]
    pub fn clear(&mut self, x: u8, y: u8) {
        debug_assert!(x < 8 && y < 8);
        self.0 &= !(1u64 << (y * 8 + x));
    }

    /// Verifica si un bit está activo
    #[inline(always)]
    pub fn test(&self, x: u8, y: u8) -> bool {
        debug_assert!(x < 8 && y < 8);
        (self.0 & (1u64 << (y * 8 + x))) != 0
    }

    /// Cuenta bits activos (usando popcnt nativo)
    #[inline(always)]
    pub fn count(&self) -> u32 {
        self.0.count_ones()
    }

    /// ¿Está vacío?
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    /// Encuentra el primer bit activo (LSB)
    #[inline(always)]
    pub fn first_set(&self) -> Option<u8> {
        if self.0 == 0 {
            None
        } else {
            Some(self.0.trailing_zeros() as u8)
        }
    }

    /// Itera sobre todos los bits activos
    #[inline]
    pub fn iter_set(&self) -> BitIterator {
        BitIterator { bits: self.0 }
    }
}

// Operadores bit a bit entre bitboards
impl std::ops::BitAnd for Bitboard64 {
    type Output = Self;
    #[inline(always)]
    fn bitand(self, rhs: Self) -> Self {
        Bitboard64(self.0 & rhs.0)
    }
}

impl std::ops::BitOr for Bitboard64 {
    type Output = Self;
    #[inline(always)]
    fn bitor(self, rhs: Self) -> Self {
        Bitboard64(self.0 | rhs.0)
    }
}

impl std::ops::BitXor for Bitboard64 {
    type Output = Self;
    #[inline(always)]
    fn bitxor(self, rhs: Self) -> Self {
        Bitboard64(self.0 ^ rhs.0)
    }
}

impl std::ops::Not for Bitboard64 {
    type Output = Self;
    #[inline(always)]
    fn not(self) -> Self {
        Bitboard64(!self.0)
    }
}

// ---------------------------------------------------------------------------
// ITERADOR DE BITS
// ---------------------------------------------------------------------------

pub struct BitIterator {
    bits: u64,
}

impl Iterator for BitIterator {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        if self.bits == 0 {
            None
        } else {
            let idx = self.bits.trailing_zeros() as u8;
            self.bits &= self.bits - 1; // Limpiar el bit más bajo
            Some(idx)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.bits.count_ones() as usize;
        (count, Some(count))
    }
}

impl ExactSizeIterator for BitIterator {}

// ---------------------------------------------------------------------------
// GRID DE BITBOARDS (128x128 = 16x16 bloques de 8x8)
// ---------------------------------------------------------------------------

/// Tamaño de la grilla en celdas
pub const BIT_GRID_SIZE: usize = 128;
/// Bloques de 8x8 necesarios: 128/8 = 16 por dimensión
pub const BIT_BLOCKS: usize = BIT_GRID_SIZE / 8;
/// Total de bitboards: 16 x 16 = 256
pub const BIT_TOTAL_BLOCKS: usize = BIT_BLOCKS * BIT_BLOCKS;

/// Grid de ocupación usando bitboards
/// Grid de ocupación usando bitboards
/// Cada bit representa una celda de 1x1 en el mundo.
/// Capas disponibles:
/// - 0: Ocupación general (edificios, obstáculos)
/// - 1: Zonas residenciales
/// - 2: Zonas comerciales
/// - 3: Zonas industriales
/// - 4: Carreteras
/// - 5: Tráfico (coches)
pub struct BitGrid {
    /// Capas de bitboards en heap: índice = layer * BIT_TOTAL_BLOCKS + block_idx
    pub layers: Vec<Bitboard64>,
}
impl BitGrid {
    /// Crea un grid vacío con todas las capas
    pub fn new() -> Self {
        BitGrid {
            layers: vec![Bitboard64::EMPTY; 6 * BIT_TOTAL_BLOCKS],
        }
    }

    /// Convierte coordenadas mundiales a (block_idx, bit_x, bit_y)
    #[inline(always)]
    fn world_to_bit(wx: f32, wy: f32) -> (usize, u8, u8) {
        let gx = wx as usize % BIT_GRID_SIZE;
        let gy = wy as usize % BIT_GRID_SIZE;
        let block_x = gx / 8;
        let block_y = gy / 8;
        let bit_x = (gx % 8) as u8;
        let bit_y = (gy % 8) as u8;
        (block_y * BIT_BLOCKS + block_x, bit_x, bit_y)
    }

    /// Setea ocupación en una capa
    #[inline(always)]
    pub fn set(&mut self, layer: usize, wx: f32, wy: f32) {
        debug_assert!(layer < 6);
        let (block_idx, bx, by) = Self::world_to_bit(wx, wy);
        self.layers[layer * BIT_TOTAL_BLOCKS + block_idx].set(bx, by);
    }

    /// Limpia ocupación
    #[inline(always)]
    pub fn clear(&mut self, layer: usize, wx: f32, wy: f32) {
        debug_assert!(layer < 6);
        let (block_idx, bx, by) = Self::world_to_bit(wx, wy);
        self.layers[layer * BIT_TOTAL_BLOCKS + block_idx].clear(bx, by);
    }

    /// Verifica ocupación en una capa
    #[inline(always)]
    pub fn test(&self, layer: usize, wx: f32, wy: f32) -> bool {
        debug_assert!(layer < 6);
        let (block_idx, bx, by) = Self::world_to_bit(wx, wy);
        self.layers[layer * BIT_TOTAL_BLOCKS + block_idx].test(bx, by)
    }

    /// Verifica si hay colisión con capa de obstáculos (layer 0)
    #[inline(always)]
    pub fn is_obstacle(&self, wx: f32, wy: f32) -> bool {
        self.test(0, wx, wy)
    }

    /// Cuenta celdas ocupadas en una capa
    pub fn count_layer(&self, layer: usize) -> u32 {
        debug_assert!(layer < 6);
        let start = layer * BIT_TOTAL_BLOCKS;
        let end = start + BIT_TOTAL_BLOCKS;
        let mut total: u32 = 0;
        for block in &self.layers[start..end] {
            total += block.count();
        }
        total
    }

    /// Limpia una capa completa (ej: tráfico cada frame)
    pub fn clear_layer(&mut self, layer: usize) {
        debug_assert!(layer < 6);
        let start = layer * BIT_TOTAL_BLOCKS;
        let end = start + BIT_TOTAL_BLOCKS;
        for block in &mut self.layers[start..end] {
            *block = Bitboard64::EMPTY;
        }
    }

    /// Encuentra todas las celdas ocupadas en una capa
    pub fn query_layer(&self, layer: usize) -> Vec<(f32, f32)> {
        debug_assert!(layer < 6);
        let mut result = Vec::with_capacity(256);
        let layer_offset = layer * BIT_TOTAL_BLOCKS;

        for block_idx in 0..BIT_TOTAL_BLOCKS {
            let block = &self.layers[layer_offset + block_idx];
            if block.is_empty() {
                continue;
            }

            let block_y = (block_idx / BIT_BLOCKS) * 8;
            let block_x = (block_idx % BIT_BLOCKS) * 8;

            for bit_idx in block.iter_set() {
                let bx = bit_idx % 8;
                let by = bit_idx / 8;
                result.push(((block_x + bx as usize) as f32, (block_y + by as usize) as f32));
            }
        }

        result
    }

    /// Colisión masiva: AND entre dos capas en todos los bloques
    /// Retorna true si hay al menos un bit en común
    pub fn layers_intersect(&self, layer_a: usize, layer_b: usize) -> bool {
        debug_assert!(layer_a < 6 && layer_b < 6);
        let off_a = layer_a * BIT_TOTAL_BLOCKS;
        let off_b = layer_b * BIT_TOTAL_BLOCKS;
        for i in 0..BIT_TOTAL_BLOCKS {
            if !(self.layers[off_a + i] & self.layers[off_b + i]).is_empty() {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bitboard_set_clear_test() {
        let mut bb = Bitboard64::EMPTY;
        assert!(!bb.test(0, 0));
        bb.set(0, 0);
        assert!(bb.test(0, 0));
        assert_eq!(bb.count(), 1);
        bb.clear(0, 0);
        assert!(!bb.test(0, 0));
        assert_eq!(bb.count(), 0);
    }

    #[test]
    fn test_bitboard_full() {
        let mut bb = Bitboard64::EMPTY;
        for y in 0..8u8 {
            for x in 0..8u8 {
                bb.set(x, y);
            }
        }
        assert_eq!(bb, Bitboard64::FULL);
        assert_eq!(bb.count(), 64);
    }

    #[test]
    fn test_bitboard_operations() {
        let mut a = Bitboard64::EMPTY;
        a.set(0, 0);
        a.set(1, 1);

        let mut b = Bitboard64::EMPTY;
        b.set(0, 0);
        b.set(2, 2);

        assert_eq!((a & b).count(), 1); // Solo (0,0)
        assert_eq!((a | b).count(), 3); // (0,0), (1,1), (2,2)
        assert_eq!((a ^ b).count(), 2); // (1,1), (2,2)
    }

    #[test]
    fn test_bit_iterator() {
        let mut bb = Bitboard64::EMPTY;
        bb.set(3, 5);
        bb.set(7, 0);
        bb.set(0, 7);

        let bits: Vec<u8> = bb.iter_set().collect();
        assert_eq!(bits.len(), 3);
    }

    #[test]
    fn test_first_set() {
        let mut bb = Bitboard64::EMPTY;
        assert_eq!(bb.first_set(), None);

        bb.set(5, 0); // bit 5
        assert_eq!(bb.first_set(), Some(5));

        bb.set(0, 0); // bit 0
        assert_eq!(bb.first_set(), Some(0)); // LSB primero
    }

    #[test]
    fn test_bit_grid_set_test() {
        let mut grid = BitGrid::new();

        grid.set(0, 10.0, 20.0);
        assert!(grid.test(0, 10.0, 20.0));
        assert!(!grid.test(0, 10.0, 21.0));

        grid.set(5, 64.0, 64.0);
        assert!(grid.test(5, 64.0, 64.0));
        assert!(!grid.test(0, 64.0, 64.0)); // Capa diferente

        grid.clear(0, 10.0, 20.0);
        assert!(!grid.test(0, 10.0, 20.0));
    }

    #[test]
    fn test_bit_grid_clear_layer() {
        let mut grid = BitGrid::new();

        for i in 0..100 {
            grid.set(3, i as f32, i as f32);
        }

        let before = grid.count_layer(3);
        assert!(before >= 100);

        grid.clear_layer(3);
        assert_eq!(grid.count_layer(3), 0);
    }

    #[test]
    fn test_bit_grid_layers_intersect() {
        let mut grid = BitGrid::new();

        grid.set(0, 5.0, 5.0);
        grid.set(1, 5.0, 5.0);
        grid.set(1, 10.0, 10.0);

        assert!(grid.layers_intersect(0, 1)); // (5,5) en ambas
        assert!(!grid.layers_intersect(0, 5)); // capa 0 y 5 sin intersección
    }

    #[test]
    fn test_bit_grid_query_layer() {
        let mut grid = BitGrid::new();

        grid.set(2, 10.0, 10.0);
        grid.set(2, 20.0, 30.0);
        grid.set(2, 100.0, 100.0);

        let cells = grid.query_layer(2);
        assert_eq!(cells.len(), 3);
        assert!(cells.contains(&(10.0, 10.0)));
        assert!(cells.contains(&(20.0, 30.0)));
        assert!(cells.contains(&(100.0, 100.0)));
    }

    #[test]
    fn test_bit_grid_is_obstacle() {
        let mut grid = BitGrid::new();

        grid.set(0, 50.0, 50.0);
        assert!(grid.is_obstacle(50.0, 50.0));
        assert!(!grid.is_obstacle(51.0, 51.0));
    }

    #[test]
    fn test_world_to_bit() {
        let (block, bx, by) = BitGrid::world_to_bit(0.0, 0.0);
        assert_eq!(block, 0);
        assert_eq!(bx, 0);
        assert_eq!(by, 0);

        let (block, bx, by) = BitGrid::world_to_bit(8.0, 0.0);
        assert_eq!(block, 1); // Segundo bloque horizontal
        assert_eq!(bx, 0);
        assert_eq!(by, 0);

        let (block, bx, by) = BitGrid::world_to_bit(7.0, 15.0);
        assert_eq!(block, 16); // Fila 1, columna 0 = bloque 16
        assert_eq!(bx, 7);
        assert_eq!(by, 7);
    }
}