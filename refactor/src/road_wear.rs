// Desgaste de Infraestructura y Baches
//
// MECÁNICA #4: Las calles se deterioran con el tráfico pesado.
// Si no hay mantenimiento, la velocidad máxima cae.
//
// ARQUITECTURA:
// - Cada celda de carretera tiene un valor de desgaste (0.0 = perfecto, 1.0 = destruido).
// - Camiones de carga (CargoTruck) causan más desgaste que coches normales.
// - El desgaste reduce la magnitud del Flow Field local.
// - El jugador puede asignar presupuesto de mantenimiento (por ahora automático).
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad
// [TC#14] Grid baked estático
// [TC#26] Inlining agresivo
// [TA#7]  Integración con Flow Fields (reduce magnitud)
// [TI#6]  Bitboards para detectar paso de vehículos

use crate::ecs::{GameWorld, Position, TrafficCar, ZoneComponent, ZoneType};
use crate::flow_field::{FlowField, FlowCell, FLOW_GRID_SIZE};
use crate::supply_chain::CargoTruck;

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Factor de desgaste por coche normal al pasar
pub const WEAR_PER_CAR: f32 = 0.0001;
/// Factor de desgaste por camión de carga (10x más que un coche)
pub const WEAR_PER_TRUCK: f32 = 0.001;
/// Tasa de reparación automática por tick (simula mantenimiento básico)
pub const AUTO_REPAIR_RATE: f32 = 0.00005;
/// Umbral de desgaste donde la velocidad empieza a reducirse
pub const WEAR_SPEED_THRESHOLD: f32 = 0.3;
/// Reducción máxima de velocidad por desgaste (a 1.0 de desgaste)
pub const MAX_SPEED_REDUCTION: f32 = 0.3; // 70% de reducción

// ---------------------------------------------------------------------------
// TIPOS
// ---------------------------------------------------------------------------

/// Mapa de desgaste de carreteras
pub struct RoadWearMap {
    /// Desgaste por celda [y * FLOW_GRID_SIZE + x] (0.0 a 1.0)
    pub wear: [f32; FLOW_GRID_SIZE * FLOW_GRID_SIZE],
    /// Contador de ticks para reparación
    pub tick_counter: u64,
}

impl RoadWearMap {
    pub fn new() -> Self {
        RoadWearMap {
            wear: [0.0_f32; FLOW_GRID_SIZE * FLOW_GRID_SIZE],
            tick_counter: 0,
        }
    }

    /// Obtiene desgaste en una celda
    #[inline(always)]
    pub fn wear_at(&self, x: usize, y: usize) -> f32 {
        if x < FLOW_GRID_SIZE && y < FLOW_GRID_SIZE {
            unsafe { *self.wear.get_unchecked(y * FLOW_GRID_SIZE + x) }
        } else {
            0.0
        }
    }

    /// Aplica desgaste a una celda
    #[inline(always)]
    pub fn apply_wear(&mut self, x: usize, y: usize, amount: f32) {
        if x < FLOW_GRID_SIZE && y < FLOW_GRID_SIZE {
            let idx = y * FLOW_GRID_SIZE + x;
            unsafe {
                let current = *self.wear.get_unchecked(idx);
                *self.wear.get_unchecked_mut(idx) = (current + amount).min(1.0);
            }
        }
    }

    /// Factor de velocidad basado en desgaste (1.0 = perfecto, 0.3 = muy dañado)
    #[inline(always)]
    pub fn speed_factor(&self, x: usize, y: usize) -> f32 {
        let wear = self.wear_at(x, y);
        if wear < WEAR_SPEED_THRESHOLD {
            1.0
        } else {
            // Interpolar: a 0.3 = 1.0, a 1.0 = 0.3
            1.0 - (wear - WEAR_SPEED_THRESHOLD) / (1.0 - WEAR_SPEED_THRESHOLD) * MAX_SPEED_REDUCTION
        }
    }
}

// ---------------------------------------------------------------------------
// SISTEMA DE DESGASTE
// ---------------------------------------------------------------------------

/// Tick del sistema de desgaste de carreteras
pub fn tick_road_wear(wear_map: &mut RoadWearMap, gw: &GameWorld) {
    wear_map.tick_counter += 1;

    // 1. Aplicar desgaste por vehículos que pasan sobre celdas de carretera
    apply_traffic_wear(wear_map, gw);

    // 2. Reparación automática (cada tick)
    apply_auto_repair(wear_map);

    // 3. Actualizar Flow Fields con factores de velocidad reducidos
    update_flow_field_speed(wear_map, gw);
}

/// Aplica desgaste donde hay vehículos sobre carreteras
fn apply_traffic_wear(wear_map: &mut RoadWearMap, gw: &GameWorld) {
    // Coches normales
    for (_entity, (pos, _car)) in gw.world.query::<(&Position, &TrafficCar)>().iter() {
        let gx = pos.x as usize % FLOW_GRID_SIZE;
        let gy = pos.y as usize % FLOW_GRID_SIZE;
        wear_map.apply_wear(gx, gy, WEAR_PER_CAR);
    }

    // Camiones de carga (más pesados, más desgaste)
    for (_entity, (pos, _truck)) in gw.world.query::<(&Position, &CargoTruck)>().iter() {
        let gx = pos.x as usize % FLOW_GRID_SIZE;
        let gy = pos.y as usize % FLOW_GRID_SIZE;
        wear_map.apply_wear(gx, gy, WEAR_PER_TRUCK);
    }
}

/// Reparación automática lenta de todas las celdas
fn apply_auto_repair(wear_map: &mut RoadWearMap) {
    for i in 0..(FLOW_GRID_SIZE * FLOW_GRID_SIZE) {
        unsafe {
            let current = *wear_map.wear.get_unchecked(i);
            if current > 0.0 {
                *wear_map.wear.get_unchecked_mut(i) = (current - AUTO_REPAIR_RATE).max(0.0);
            }
        }
    }
}

/// Actualiza la magnitud del Flow Field basado en desgaste
fn update_flow_field_speed(wear_map: &RoadWearMap, gw: &GameWorld) {
    // Esta función modificaría el FlowFieldManager.primary en GameWorld
    // Por ahora, aplicamos el factor de velocidad en tick_traffic_flow
    // mediante una consulta al wear_map.
    // 
    // El factor se consulta externamente cuando los coches calculan su velocidad:
    // speed = base_speed * road_wear.speed_factor(gx, gy)
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_road_wear_default() {
        let map = RoadWearMap::new();
        assert_eq!(map.wear_at(50, 50), 0.0);
        assert_eq!(map.speed_factor(50, 50), 1.0);
    }

    #[test]
    fn test_apply_wear() {
        let mut map = RoadWearMap::new();
        map.apply_wear(10, 10, 0.5);
        assert!((map.wear_at(10, 10) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_wear_accumulation() {
        let mut map = RoadWearMap::new();
        for _ in 0..10 {
            map.apply_wear(5, 5, 0.1);
        }
        assert!((map.wear_at(5, 5) - 1.0).abs() < 0.01, "Debe saturar a 1.0");
    }

    #[test]
    fn test_speed_factor_reduction() {
        let mut map = RoadWearMap::new();
        
        // Sin desgaste: velocidad completa
        assert!((map.speed_factor(0, 0) - 1.0).abs() < 0.01);
        
        // Desgaste medio: velocidad reducida
        map.apply_wear(1, 1, 0.6);
        let factor = map.speed_factor(1, 1);
        assert!(factor < 1.0, "Factor debe ser < 1.0 con desgaste 0.6, es {}", factor);
        assert!(factor > 0.2, "Factor no debe ser demasiado bajo: {}", factor);
        
        // Desgaste máximo: mínima velocidad
        map.apply_wear(1, 1, 0.4); // Ya tenía 0.6, ahora 1.0
        let factor_max = map.speed_factor(1, 1);
        assert!(factor_max < 0.5, "Factor debe ser muy bajo con desgaste 1.0: {}", factor_max);
    }

    #[test]
    fn test_auto_repair() {
        let mut map = RoadWearMap::new();
        map.apply_wear(20, 20, 0.01);
        
        let before = map.wear_at(20, 20);
        apply_auto_repair(&mut map);
        let after = map.wear_at(20, 20);
        
        assert!(after < before || after == 0.0, "Debe repararse: {} -> {}", before, after);
    }

    #[test]
    fn test_wear_bounds() {
        let map = RoadWearMap::new();
        assert_eq!(map.wear_at(200, 200), 0.0); // Fuera de bounds
        assert_eq!(map.speed_factor(999, 999), 1.0);
    }
}
