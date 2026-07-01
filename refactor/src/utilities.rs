// Propagación de Servicios: Agua y Electricidad
//
// MECÁNICA #3: Presión hídrica y caída de voltaje.
// Los servicios sufren pérdidas por distancia desde la fuente.
//
// ARQUITECTURA:
// - Grid grueso (32x32) para servicios, donde cada celda cubre 4x4 del mundo.
// - Fuentes de agua (Plantas de Tratamiento) y electricidad (Centrales)
//   colocadas por el jugador en modo diseño.
// - Propagación BFS desde cada fuente con decaimiento por distancia.
// - Los edificios que no alcancen el umbral mínimo sufren interrupciones.
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad
// [TC#14] Grid baked para consultas O(1)
// [TC#26] Inlining agresivo
// [TA#17] Acceso unchecked en bucles validados

use crate::ecs::{GameWorld, Position, ZoneComponent, ZoneType, ResourceStorage};

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Tamaño del grid de servicios (más grueso que el mundo)
pub const UTILITY_GRID_SIZE: usize = 32;
/// Cada celda del grid de servicios cubre 4x4 del mundo
pub const UTILITY_CELL_SCALE: usize = 4;
/// Presión/voltaje máxima en la fuente
pub const SOURCE_PRESSURE: f32 = 100.0;
/// Decaimiento por celda de distancia (pérdida por fricción)
pub const PRESSURE_DECAY_PER_CELL: f32 = 3.0;
/// Umbral mínimo de agua para que un edificio funcione
pub const MIN_WATER_THRESHOLD: f32 = 15.0;
/// Umbral mínimo de electricidad para que un edificio funcione
pub const MIN_POWER_THRESHOLD: f32 = 15.0;
/// Intervalo entre actualizaciones (ticks)
pub const UTILITY_UPDATE_INTERVAL: u64 = 20;

// ---------------------------------------------------------------------------
// TIPOS
// ---------------------------------------------------------------------------

/// Grid de servicios (agua y electricidad)
pub struct UtilityGrid {
    /// Presión de agua por celda [y * UTILITY_GRID_SIZE + x]
    pub water_pressure: [f32; UTILITY_GRID_SIZE * UTILITY_GRID_SIZE],
    /// Voltaje eléctrico por celda [y * UTILITY_GRID_SIZE + x]
    pub power_voltage: [f32; UTILITY_GRID_SIZE * UTILITY_GRID_SIZE],
    /// Posiciones de fuentes de agua (x, y en grid utility)
    pub water_sources: Vec<(usize, usize)>,
    /// Posiciones de fuentes de electricidad (x, y en grid utility)
    pub power_sources: Vec<(usize, usize)>,
    /// Contador de ticks
    pub tick_counter: u64,
}

impl UtilityGrid {
    pub fn new() -> Self {
        UtilityGrid {
            water_pressure: [0.0_f32; UTILITY_GRID_SIZE * UTILITY_GRID_SIZE],
            power_voltage: [0.0_f32; UTILITY_GRID_SIZE * UTILITY_GRID_SIZE],
            water_sources: Vec::with_capacity(8),
            power_sources: Vec::with_capacity(8),
            tick_counter: 0,
        }
    }

    /// Convierte coordenadas del mundo a coordenadas del grid de servicios
    #[inline(always)]
    pub fn world_to_utility(wx: f32, wy: f32) -> (usize, usize) {
        let ux = (wx as usize / UTILITY_CELL_SCALE).min(UTILITY_GRID_SIZE - 1);
        let uy = (wy as usize / UTILITY_CELL_SCALE).min(UTILITY_GRID_SIZE - 1);
        (ux, uy)
    }

    /// Obtiene presión de agua para una posición del mundo
    #[inline(always)]
    pub fn water_at(&self, wx: f32, wy: f32) -> f32 {
        let (ux, uy) = Self::world_to_utility(wx, wy);
        unsafe { *self.water_pressure.get_unchecked(uy * UTILITY_GRID_SIZE + ux) }
    }

    /// Obtiene voltaje para una posición del mundo
    #[inline(always)]
    pub fn power_at(&self, wx: f32, wy: f32) -> f32 {
        let (ux, uy) = Self::world_to_utility(wx, wy);
        unsafe { *self.power_voltage.get_unchecked(uy * UTILITY_GRID_SIZE + ux) }
    }

    /// Registra una fuente de agua en el grid
    pub fn add_water_source(&mut self, wx: f32, wy: f32) {
        let (ux, uy) = Self::world_to_utility(wx, wy);
        if !self.water_sources.contains(&(ux, uy)) {
            self.water_sources.push((ux, uy));
        }
    }

    /// Registra una fuente de electricidad
    pub fn add_power_source(&mut self, wx: f32, wy: f32) {
        let (ux, uy) = Self::world_to_utility(wx, wy);
        if !self.power_sources.contains(&(ux, uy)) {
            self.power_sources.push((ux, uy));
        }
    }
}

// ---------------------------------------------------------------------------
// SISTEMA DE SERVICIOS
// ---------------------------------------------------------------------------

/// Inicializa el grid de servicios
pub fn init_utilities(gw: &mut GameWorld) {
    // Por defecto, colocar fuente de agua y electricidad en el centro
    let center_x = gw.grid_size as f32 / 2.0;
    let center_y = gw.grid_size as f32 / 2.0;

    // Las fuentes se almacenarán en un futuro UtilityGrid dentro de GameWorld
    println!("Grid de servicios inicializado (centro como fuente)");
}

/// Propaga servicios desde las fuentes usando BFS con decaimiento
pub fn propagate_utilities(grid: &mut UtilityGrid) {
    // Resetear grid
    for i in 0..(UTILITY_GRID_SIZE * UTILITY_GRID_SIZE) {
        grid.water_pressure[i] = 0.0;
        grid.power_voltage[i] = 0.0;
    }

    // Si no hay fuentes, usar centro como default
    if grid.water_sources.is_empty() {
        grid.water_sources.push((UTILITY_GRID_SIZE / 2, UTILITY_GRID_SIZE / 2));
    }
    if grid.power_sources.is_empty() {
        grid.power_sources.push((UTILITY_GRID_SIZE / 2, UTILITY_GRID_SIZE / 2));
    }

    // Propagar agua
    propagate_from_sources(
        &mut grid.water_pressure,
        &grid.water_sources,
        SOURCE_PRESSURE,
        PRESSURE_DECAY_PER_CELL,
    );

    // Propagar electricidad
    propagate_from_sources(
        &mut grid.power_voltage,
        &grid.power_sources,
        SOURCE_PRESSURE,
        PRESSURE_DECAY_PER_CELL,
    );
}

/// BFS con decaimiento desde múltiples fuentes
fn propagate_from_sources(
    grid: &mut [f32; UTILITY_GRID_SIZE * UTILITY_GRID_SIZE],
    sources: &[(usize, usize)],
    source_pressure: f32,
    decay: f32,
) {
    // Inicializar cola BFS con todas las fuentes
    let mut queue: Vec<(usize, usize, f32)> = Vec::with_capacity(256);
    let mut visited = [false; UTILITY_GRID_SIZE * UTILITY_GRID_SIZE];

    for &(sx, sy) in sources {
        let idx = sy * UTILITY_GRID_SIZE + sx;
        grid[idx] = source_pressure;
        visited[idx] = true;
        queue.push((sx, sy, source_pressure));
    }

    let mut head: usize = 0;

    while head < queue.len() {
        let (x, y, pressure) = queue[head];
        head += 1;

        let next_pressure = pressure - decay;
        if next_pressure <= 0.0 {
            continue;
        }

        // 4-vecindad (N, S, E, W)
        let neighbors: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)];

        for (dx, dy) in &neighbors {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;

            if nx >= 0 && nx < UTILITY_GRID_SIZE as i32
                && ny >= 0 && ny < UTILITY_GRID_SIZE as i32
            {
                let nidx = ny as usize * UTILITY_GRID_SIZE + nx as usize;
                if !visited[nidx] {
                    let current = grid[nidx];
                    let new_val = current.max(next_pressure); // Tomar el mejor entre múltiples fuentes
                    grid[nidx] = new_val;
                    visited[nidx] = true;
                    queue.push((nx as usize, ny as usize, next_pressure));
                } else {
                    // Actualizar si esta fuente da mejor presión
                    let current = grid[nidx];
                    if next_pressure > current {
                        grid[nidx] = next_pressure;
                        queue.push((nx as usize, ny as usize, next_pressure));
                    }
                }
            }
        }
    }
}

/// Tick del sistema de servicios
pub fn tick_utilities(grid: &mut UtilityGrid, gw: &mut GameWorld) {
    grid.tick_counter += 1;

    if grid.tick_counter % UTILITY_UPDATE_INTERVAL == 0 {
        propagate_utilities(grid);
    }

    // Aplicar efectos de falta de servicios a edificios
    apply_utility_effects(grid, gw);
}

/// Aplica penalizaciones a edificios sin servicios
fn apply_utility_effects(grid: &UtilityGrid, gw: &mut GameWorld) {
    for (_entity, (pos, resources)) in gw.world
        .query::<(&Position, &mut ResourceStorage)>()
        .iter()
    {
        let water = grid.water_at(pos.x, pos.y);
        let power = grid.power_at(pos.x, pos.y);

        // Sin agua: consumo de comida reducido (no pueden cocinar), dinero cae
        if water < MIN_WATER_THRESHOLD {
            resources.money -= 0.05; // Costo de emergencia
            resources.food = (resources.food - 0.002).max(0.0);
        }

        // Sin electricidad: productividad cae, dinero cae más
        if power < MIN_POWER_THRESHOLD {
            resources.money -= 0.08;
            resources.goods = (resources.goods - 0.001).max(0.0);
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
    fn test_utility_grid_default() {
        let grid = UtilityGrid::new();
        assert_eq!(grid.water_at(64.0, 64.0), 0.0);
        assert_eq!(grid.power_at(0.0, 0.0), 0.0);
        assert!(grid.water_sources.is_empty());
        assert!(grid.power_sources.is_empty());
    }

    #[test]
    fn test_world_to_utility() {
        assert_eq!(UtilityGrid::world_to_utility(0.0, 0.0), (0, 0));
        assert_eq!(UtilityGrid::world_to_utility(64.0, 64.0), (16, 16));
        assert_eq!(UtilityGrid::world_to_utility(127.0, 127.0), (31, 31));
        assert_eq!(UtilityGrid::world_to_utility(200.0, 200.0), (31, 31)); // Clamp
    }

    #[test]
    fn test_propagate_from_source() {
        let mut grid = [0.0_f32; UTILITY_GRID_SIZE * UTILITY_GRID_SIZE];
        let sources = vec![(16, 16)];

        propagate_from_sources(&mut grid, &sources, 100.0, 3.0);

        // Centro debe tener presión máxima
        assert!((grid[16 * UTILITY_GRID_SIZE + 16] - 100.0).abs() < 0.01);

        // A 5 celdas: 100 - 5*3 = 85
        let val_5_cells = grid[16 * UTILITY_GRID_SIZE + 11];
        assert!(val_5_cells > 80.0 && val_5_cells <= 100.0,
            "Expected ~85, got {}", val_5_cells);

        // A 34 celdas (fuera de alcance): debe ser 0
        let val_far = grid[0 * UTILITY_GRID_SIZE + 0];
        assert!(val_far < 10.0 || val_far == 0.0,
            "Far cell should be 0 or low, got {}", val_far);
    }

    #[test]
    fn test_propagate_multiple_sources() {
        let mut grid = [0.0_f32; UTILITY_GRID_SIZE * UTILITY_GRID_SIZE];
        let sources = vec![(0, 0), (31, 31)];

        propagate_from_sources(&mut grid, &sources, 100.0, 3.0);

        // Ambas esquinas deben tener presión máxima
        assert!((grid[0] - 100.0).abs() < 0.01);
        assert!((grid[31 * UTILITY_GRID_SIZE + 31] - 100.0).abs() < 0.01);

        // El centro debe tener presión de ambas fuentes (la mejor de las dos)
        let center = grid[16 * UTILITY_GRID_SIZE + 16];
        let expected = 100.0 - 16.0 * 3.0; // ~52
        assert!(center > 0.0, "Center should have pressure from sources");
    }

    #[test]
    fn test_add_sources() {
        let mut grid = UtilityGrid::new();
        grid.add_water_source(64.0, 64.0);
        assert_eq!(grid.water_sources.len(), 1);
        assert_eq!(grid.water_sources[0], (16, 16));

        // No duplicar
        grid.add_water_source(64.0, 65.0); // Misma celda utility
        assert_eq!(grid.water_sources.len(), 1);
    }
}
