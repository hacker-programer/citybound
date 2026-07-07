// Presión Hídrica y Caída de Voltaje
//
// MECÁNICA #3: Física de Servicios
//
// Los servicios sufren pérdidas por distancia. Si construyes
// lejos de la estación de bombeo o central eléctrica, no llega
// presión de agua o hay apagones cuando la red se sobrecarga.
//
// Algoritmo de propagación en grilla gruesa (32x32).
// Cada celda de distancia desde la fuente resta una fracción
// de presión/voltaje. Edificios sin umbral mínimo sufren
// interrupciones que afectan su ResourceStorage.
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad
// [TA#9]  Alineación a 64B
// [TA#17] Acceso unchecked en bucles validados

use crate::ecs::{GameWorld, Position, ConstructionState, BuildingType, ResourceStorage};

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Tamaño de grilla de utilidades (32x32, cada celda = 4x4 del mundo)
pub const UTILITY_GRID_SIZE: usize = 32;
/// Máxima presión/voltaje en fuente
pub const MAX_PRESSURE: f32 = 100.0;
/// Pérdida por celda de distancia (Manhattan)
pub const PRESSURE_LOSS_PER_CELL: f32 = 5.0;
/// Umbral mínimo para funcionamiento
pub const MIN_PRESSURE_THRESHOLD: f32 = 20.0;
/// Ticks entre actualizaciones de utilidades
pub const UTILITY_UPDATE_INTERVAL: u64 = 60;

// ---------------------------------------------------------------------------
// TIPOS
// ---------------------------------------------------------------------------

/// Tipo de fuente de utilidad
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UtilitySourceType {
    WaterTower,
    PowerPlant,
    WaterTreatment,
    Substation,
}

// [FIXED] Grilla de utilidades
#[repr(align(64))]
pub struct UtilityGrid {
    pub values: Vec<f32>,
    pub source_type: UtilitySourceType,
    pub sources: Vec<(f32, f32)>,
}

impl UtilityGrid {
    pub fn new(source_type: UtilitySourceType) -> Self {
        UtilityGrid {
            values: vec![0.0_f32; UTILITY_GRID_SIZE * UTILITY_GRID_SIZE],
            source_type,
            sources: Vec::with_capacity(8),
        }
    }

    /// Agrega una fuente en coordenadas de mundo
    pub fn add_source(&mut self, world_x: f32, world_y: f32) {
        let gx = (world_x / 4.0) as usize;
        let gy = (world_y / 4.0) as usize;
        if gx < UTILITY_GRID_SIZE && gy < UTILITY_GRID_SIZE {
            self.sources.push((world_x, world_y));
            self.values[gy * UTILITY_GRID_SIZE + gx] = MAX_PRESSURE;
        }
    }

    /// Propaga presión/voltaje desde las fuentes usando distancia Manhattan
    pub fn propagate(&mut self) {
        let total = UTILITY_GRID_SIZE * UTILITY_GRID_SIZE;
        let mut new_values = vec![0.0_f32; total];

        // Marcar fuentes
        for (wx, wy) in &self.sources {
            let gx = (*wx / 4.0) as usize;
            let gy = (*wy / 4.0) as usize;
            if gx < UTILITY_GRID_SIZE && gy < UTILITY_GRID_SIZE {
                new_values[gy * UTILITY_GRID_SIZE + gx] = MAX_PRESSURE;
            }
        }

        // Propagar: cada celda toma el máximo de (vecino - pérdida)
        for _iteration in 0..8 {
            let mut next = new_values.clone();

            for gy in 0..UTILITY_GRID_SIZE {
                for gx in 0..UTILITY_GRID_SIZE {
                    let idx = gy * UTILITY_GRID_SIZE + gx;
                    let mut max_neighbor = 0.0_f32;

                    if gy > 0 {
                        max_neighbor = max_neighbor.max(new_values[(gy-1) * UTILITY_GRID_SIZE + gx] - PRESSURE_LOSS_PER_CELL);
                    }
                    if gy < UTILITY_GRID_SIZE - 1 {
                        max_neighbor = max_neighbor.max(new_values[(gy+1) * UTILITY_GRID_SIZE + gx] - PRESSURE_LOSS_PER_CELL);
                    }
                    if gx > 0 {
                        max_neighbor = max_neighbor.max(new_values[gy * UTILITY_GRID_SIZE + (gx-1)] - PRESSURE_LOSS_PER_CELL);
                    }
                    if gx < UTILITY_GRID_SIZE - 1 {
                        max_neighbor = max_neighbor.max(new_values[gy * UTILITY_GRID_SIZE + (gx+1)] - PRESSURE_LOSS_PER_CELL);
                    }

                    next[idx] = next[idx].max(max_neighbor);
                }
            }

            new_values = next;
        }

        self.values = new_values;
    }

    /// Obtiene presión en coordenadas de mundo
    #[inline(always)]
    pub fn get_pressure(&self, world_x: f32, world_y: f32) -> f32 {
        let gx = (world_x / 4.0) as usize;
        let gy = (world_y / 4.0) as usize;
        if gx < UTILITY_GRID_SIZE && gy < UTILITY_GRID_SIZE {
            unsafe { *self.values.get_unchecked(gy * UTILITY_GRID_SIZE + gx) }
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// SISTEMA PRINCIPAL
// ---------------------------------------------------------------------------

/// Tick de utilidades: propagar y aplicar efectos
pub fn tick_utilities(gw: &mut GameWorld) {
    if gw.sim_tick % UTILITY_UPDATE_INTERVAL != 0 {
        return;
    }

    // Encontrar fuentes (edificios que generan utilidad)
    update_sources(gw);

    // Propagar agua y electricidad
    gw.water_grid.propagate();
    gw.power_grid.propagate();

    // Aplicar efectos a edificios
    apply_utility_effects(gw);
}

fn update_sources(gw: &mut GameWorld) {
    gw.water_grid.sources.clear();
    gw.power_grid.sources.clear();

    for (_entity, (pos, construction)) in gw.world
        .query::<(&Position, &ConstructionState)>()
        .iter()
    {
        match construction.building_type {
            BuildingType::Farm => {
                // Granjas actúan como pequeñas fuentes de agua
                gw.water_grid.add_source(pos.x, pos.y);
            }
            BuildingType::Factory => {
                // Fábricas generan electricidad (pequeña)
                gw.power_grid.add_source(pos.x, pos.y);
            }
            _ => {}
        }
    }

    // Si no hay fuentes, agregar una fuente central por defecto
    if gw.water_grid.sources.is_empty() {
        gw.water_grid.add_source(64.0, 64.0); // Centro del mapa
    }
    if gw.power_grid.sources.is_empty() {
        gw.power_grid.add_source(64.0, 64.0);
    }
}

fn apply_utility_effects(gw: &mut GameWorld) {
    for (_entity, (pos, construction, storage)) in gw.world
        .query::<(&Position, &ConstructionState, &mut ResourceStorage)>()
        .iter()
    {
        if construction.progress < 0.5 {
            continue;
        }

        let water = gw.water_grid.get_pressure(pos.x, pos.y);
        let power = gw.power_grid.get_pressure(pos.x, pos.y);

        // Sin agua → producción reducida
        if water < MIN_PRESSURE_THRESHOLD {
            storage.food *= 0.95; // Pérdida de alimentos
            storage.goods *= 0.98; // Menos producción
        }

        // Sin electricidad → penalización fuerte
        if power < MIN_PRESSURE_THRESHOLD {
            storage.money *= 0.90; // Grandes pérdidas económicas
            storage.goods *= 0.85; // Producción muy reducida
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
    fn test_utility_grid_new() {
        let grid = UtilityGrid::new(UtilitySourceType::WaterTower);
        assert_eq!(grid.sources.len(), 0);
        assert!((grid.get_pressure(0.0, 0.0) - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_add_source() {
        let mut grid = UtilityGrid::new(UtilitySourceType::WaterTower);
        grid.add_source(64.0, 64.0);
        assert_eq!(grid.sources.len(), 1);
        assert!(grid.get_pressure(64.0, 64.0) > 50.0);
    }

    #[test]
    fn test_propagation() {
        let mut grid = UtilityGrid::new(UtilitySourceType::PowerPlant);
        grid.add_source(64.0, 64.0);
        grid.propagate();

        // Cerca de la fuente debe haber presión
        let near = grid.get_pressure(68.0, 64.0);
        assert!(near > MIN_PRESSURE_THRESHOLD,
            "Presión cerca de fuente: {}", near);

        // Lejos de la fuente debe caer
        let far = grid.get_pressure(0.0, 0.0);
        assert!(far < near || far < 50.0,
            "Presión lejos: {} vs cerca: {}", far, near);
    }

    #[test]
    fn test_pressure_falloff() {
        let mut grid = UtilityGrid::new(UtilitySourceType::WaterTower);
        grid.add_source(64.0, 64.0);
        grid.propagate();

        let p0 = grid.get_pressure(64.0, 64.0);
        let p1 = grid.get_pressure(80.0, 64.0);
        let p2 = grid.get_pressure(120.0, 64.0);

        assert!(p0 >= p1, "Presión debe decrecer con distancia");
        assert!(p1 >= p2 || p2 < 20.0, "Presión lejana debe ser baja");
    }

    #[test]
    fn test_multiple_sources() {
        let mut grid = UtilityGrid::new(UtilitySourceType::WaterTower);
        grid.add_source(48.0, 48.0);
        grid.add_source(80.0, 80.0);
        grid.propagate();

        // El centro entre dos fuentes debe tener buena presión
        let mid = grid.get_pressure(64.0, 64.0);
        assert!(mid > 20.0, "Entre dos fuentes debe haber presión: {}", mid);
    }

    #[test]
    fn test_bounds_check() {
        let grid = UtilityGrid::new(UtilitySourceType::WaterTower);
        let out_of_bounds = grid.get_pressure(500.0, 500.0);
        assert!((out_of_bounds - 0.0).abs() < 0.01);
    }
}
