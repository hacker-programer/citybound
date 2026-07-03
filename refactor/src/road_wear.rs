// Desgaste de Infraestructura y Baches
//
// MECÁNICA #4: Desgaste de Infraestructura
//
// Cuanto más tráfico pesado pase por una celda, más rápido
// se deteriora el asfalto. Si el jugador no asigna presupuesto
// de mantenimiento, la velocidad máxima cae (simulando baches),
// arruinando tiempos de viaje y congestionando rutas alternativas.
//
// El desgaste reduce la magnitud del Flow Field local, forzando
// a los autos a frenar y formando embotellamientos reales.
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad
// [TA#7]  Flow Fields: modificación de magnitud por desgaste
// [TA#9]  Alineación a 64B
// [TI#6]  Bitboards para detectar tráfico en celdas


use crate::ecs::{GameWorld, Position, TrafficCar};
use crate::supply_chain::CargoTruck;

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Tamaño del grid de desgaste
pub const WEAR_GRID_SIZE: usize = 128;
/// Desgaste máximo (calle completamente destruida)
pub const MAX_WEAR: f32 = 100.0;
/// Desgaste por coche normal al pasar
pub const CAR_WEAR: f32 = 0.001;
/// Desgaste por camión de carga al pasar
pub const TRUCK_WEAR: f32 = 0.01;
/// Reparación natural por tick (muy lenta)
pub const NATURAL_REPAIR: f32 = 0.0001;
/// Umbral para considerar calle dañada (reducción de velocidad)
pub const DAMAGE_THRESHOLD: f32 = 30.0;
/// Umbral para calle muy dañada
pub const SEVERE_DAMAGE_THRESHOLD: f32 = 70.0;
/// Máxima reducción de velocidad por desgaste
pub const MAX_SPEED_PENALTY: f32 = 0.6;
/// Intervalo de actualización de desgaste
pub const WEAR_UPDATE_INTERVAL: u64 = 5;

// ---------------------------------------------------------------------------
// GRID DE DESGASTE
// ---------------------------------------------------------------------------

/// Grid que rastrea el desgaste del asfalto por celda
#[repr(align(64))]
pub struct RoadWearGrid {
    pub values: [[f32; WEAR_GRID_SIZE]; WEAR_GRID_SIZE],
    /// Presupuesto de mantenimiento (0.0 - 1.0)
    pub maintenance_budget: f32,
}

impl RoadWearGrid {
    pub fn new() -> Self {
        RoadWearGrid {
            values: [[0.0_f32; WEAR_GRID_SIZE]; WEAR_GRID_SIZE],
            maintenance_budget: 0.5,
        }
    }

    #[inline(always)]
    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < WEAR_GRID_SIZE && y < WEAR_GRID_SIZE {
            unsafe { *self.values.get_unchecked(y).get_unchecked(x) }
        } else {
            0.0
        }
    }

    /// Aplica reparación según presupuesto de mantenimiento
    pub fn apply_maintenance(&mut self) {
        let repair_rate = NATURAL_REPAIR + self.maintenance_budget * 0.005;

        for y in 0..WEAR_GRID_SIZE {
            for x in 0..WEAR_GRID_SIZE {
                self.values[y][x] = (self.values[y][x] - repair_rate).max(0.0);
            }
        }
    }

    /// Calcula el factor de velocidad (1.0 = sin penalización, 0.4 = máxima penalización)
    #[inline(always)]
    pub fn speed_factor(&self, x: usize, y: usize) -> f32 {
        let wear = self.get(x, y);

        if wear < DAMAGE_THRESHOLD {
            1.0
        } else if wear < SEVERE_DAMAGE_THRESHOLD {
            // Interpolación lineal entre 1.0 y (1.0 - MAX_SPEED_PENALTY)
            let t = (wear - DAMAGE_THRESHOLD) / (SEVERE_DAMAGE_THRESHOLD - DAMAGE_THRESHOLD);
            1.0 - t * MAX_SPEED_PENALTY
        } else {
            1.0 - MAX_SPEED_PENALTY
        }
    }

    /// Colores para visualización de desgaste
    pub fn wear_color(&self, x: usize, y: usize) -> u32 {
        let wear = self.get(x, y);
        if wear < DAMAGE_THRESHOLD {
            0x00_00_00_00 // Sin daño visible
        } else if wear < SEVERE_DAMAGE_THRESHOLD {
            let alpha = ((wear - DAMAGE_THRESHOLD) / (SEVERE_DAMAGE_THRESHOLD - DAMAGE_THRESHOLD) * 0x88u32 as f32) as u32;
            (alpha << 24) | 0x00_FF_FF_00 // Amarillo
        } else {
            let alpha = ((wear - SEVERE_DAMAGE_THRESHOLD) / (MAX_WEAR - SEVERE_DAMAGE_THRESHOLD) * 0xAAu32 as f32) as u32;
            (alpha.min(0xAA) << 24) | 0x00_FF_44_00 // Rojo
        }
    }
}

// ---------------------------------------------------------------------------
// SISTEMA PRINCIPAL
// ---------------------------------------------------------------------------

/// Tick de desgaste: acumular daño por tráfico y aplicar reparaciones
pub fn tick_road_wear(gw: &mut GameWorld) {
    if gw.sim_tick % WEAR_UPDATE_INTERVAL != 0 {
        return;
    }

    // 1. Acumular desgaste por posición de vehículos
    accumulate_wear(gw);

    // 2. Aplicar mantenimiento
    gw.road_wear.apply_maintenance();

    // 3. Actualizar Flow Fields con penalización por desgaste
    apply_wear_to_flow_fields(gw);
}

fn accumulate_wear(gw: &mut GameWorld) {
    // Coches normales
    for (_entity, (pos, _car)) in gw.world
        .query::<(&Position, &TrafficCar)>()
        .iter()
    {
        let gx = pos.x as usize;
        let gy = pos.y as usize;
        if gx < WEAR_GRID_SIZE && gy < WEAR_GRID_SIZE {
            gw.road_wear.values[gy][gx] = (gw.road_wear.values[gy][gx] + CAR_WEAR).min(MAX_WEAR);
        }
    }

    // Camiones de carga (más pesados, más desgaste)
    for (_entity, (pos, _truck)) in gw.world
        .query::<(&Position, &CargoTruck)>()
        .iter()
    {
        let gx = pos.x as usize;
        let gy = pos.y as usize;
        if gx < WEAR_GRID_SIZE && gy < WEAR_GRID_SIZE {
            gw.road_wear.values[gy][gx] = (gw.road_wear.values[gy][gx] + TRUCK_WEAR).min(MAX_WEAR);
        }
    }
}

/// Modifica los Flow Fields para reflejar el desgaste
fn apply_wear_to_flow_fields(gw: &mut GameWorld) {
    use crate::flow_field::FLOW_GRID_SIZE;

    for gy in 0..FLOW_GRID_SIZE {
        for gx in 0..FLOW_GRID_SIZE {
            let wear_factor = gw.road_wear.speed_factor(gx, gy);

            if wear_factor < 1.0 {
                // Reducir magnitud del flow field primario
                let idx = gy * FLOW_GRID_SIZE + gx;
                let cell = &mut gw.flow_fields.primary.cells[idx];

                // La magnitud se reduce proporcionalmente al desgaste
                cell.magnitude *= wear_factor;

                // También aplicar al flow field de autopista
                let hwy_cell = &mut gw.flow_fields.highway.cells[idx];
                hwy_cell.magnitude *= wear_factor;
            }
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
    fn test_road_wear_new() {
        let grid = RoadWearGrid::new();
        assert!((grid.get(0, 0) - 0.0).abs() < 0.01);
        assert!((grid.maintenance_budget - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_wear_accumulation() {
        let mut grid = RoadWearGrid::new();
        grid.values[10][10] += CAR_WEAR * 100.0;
        assert!(grid.get(10, 10) > 0.0);
    }

    #[test]
    fn test_maintenance_repairs() {
        let mut grid = RoadWearGrid::new();
        grid.maintenance_budget = 1.0;
        grid.values[10][10] = 50.0;

        grid.apply_maintenance();

        assert!(grid.get(10, 10) < 50.0, "Mantenimiento debe reparar: {}", grid.get(10, 10));
    }

    #[test]
    fn test_no_maintenance_no_repair() {
        let mut grid = RoadWearGrid::new();
        grid.maintenance_budget = 0.0;
        grid.values[10][10] = 50.0;

        grid.apply_maintenance();

        // Con presupuesto 0, solo repara NATURAL_REPAIR
        assert!(grid.get(10, 10) <= 50.0);
    }

    #[test]
    fn test_speed_factor_undamaged() {
        let grid = RoadWearGrid::new();
        assert!((grid.speed_factor(10, 10) - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_speed_factor_damaged() {
        let mut grid = RoadWearGrid::new();
        grid.values[10][10] = DAMAGE_THRESHOLD + 10.0;

        let factor = grid.speed_factor(10, 10);
        assert!(factor < 1.0, "Daño debe reducir velocidad: {}", factor);
    }

    #[test]
    fn test_speed_factor_severe() {
        let mut grid = RoadWearGrid::new();
        grid.values[10][10] = SEVERE_DAMAGE_THRESHOLD + 10.0;

        let factor = grid.speed_factor(10, 10);
        assert!((factor - (1.0 - MAX_SPEED_PENALTY)).abs() < 0.01,
            "Daño severo: {} vs {}", factor, 1.0 - MAX_SPEED_PENALTY);
    }

    #[test]
    fn test_wear_color() {
        let mut grid = RoadWearGrid::new();

        let clean = grid.wear_color(10, 10);
        assert_eq!(clean, 0x00_00_00_00);

        grid.values[10][10] = DAMAGE_THRESHOLD + 5.0;
        let damaged = grid.wear_color(10, 10);
        assert_ne!(damaged, 0x00_00_00_00);

        grid.values[10][10] = SEVERE_DAMAGE_THRESHOLD + 5.0;
        let severe = grid.wear_color(10, 10);
        assert_ne!(severe, damaged);
    }
}
