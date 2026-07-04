// Valor del Suelo, Gentrificación y Contaminación
//
// MECÁNICA #2: Gentrificación, Valor del Suelo y Desalojos
//
// El valor del terreno fluctúa dinámicamente. Si pones un parque
// o mejores calles, el valor sube. Si el valor sube, los impuestos
// suben. Residentes originales no pueden pagar → desalojados.
// Edificios lujosos ocupan su lugar, asentamientos pobres migran
// a bordes del mapa (cerca de contaminación).
//
// Autómata Celular Difusivo: el valor del suelo y la contaminación
// se propagan como difusión en cada tick usando el TerrainMap.
//
// TÉCNICAS APLICADAS:
// [TC#3]  Baking de iluminación (colores heatmap precalculados)
// [TC#14] Ruido Perlin pre-generado como base
// [TA#9]  Structs alineados a 64B
// [TI#6]  Bitboards para consultas rápidas de zona

use crate::ecs::{GameWorld, Position, ZoneComponent, ZoneType, ConstructionState,
                  BuildingType, ResourceStorage, Renderable};


// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Tamaño del heatmap (debe coincidir con grid_size)
pub const HEATMAP_SIZE: usize = 128;
/// Factor de difusión por tick
pub const DIFFUSION_RATE: f32 = 0.15;
/// Decaimiento de contaminación por tick
pub const POLLUTION_DECAY: f32 = 0.001;
/// Incremento de valor por parque cercano
pub const PARK_VALUE_BOOST: f32 = 0.05;
/// Incremento de valor por buena calle
pub const ROAD_VALUE_BOOST: f32 = 0.02;
/// Reducción de valor por contaminación
pub const POLLUTION_VALUE_PENALTY: f32 = 0.03;
/// Reducción de valor por zona industrial
pub const INDUSTRIAL_VALUE_PENALTY: f32 = 0.02;
/// Umbral para gentrificación (valor > ingresos * factor)
pub const GENTRIFICATION_THRESHOLD: f32 = 1.5;
/// Máximo valor del suelo
pub const MAX_LAND_VALUE: f32 = 100.0;
/// Ticks entre actualizaciones de heatmap (cada ~10 segundos sim)
pub const HEATMAP_UPDATE_INTERVAL: u64 = 30;

// ---------------------------------------------------------------------------
// LAND VALUE HEATMAP
// ---------------------------------------------------------------------------

/// Mapa de calor del valor del suelo.
/// Valores > 50 son zona cara, < 20 son zona barata.
#[repr(align(64))]
pub struct LandValueHeatmap {
    pub values: Vec<f32>,
}

impl LandValueHeatmap {
    pub fn new() -> Self {
        let total = HEATMAP_SIZE * HEATMAP_SIZE;
        let mut values = Vec::with_capacity(total);
        values.resize(total, 10.0_f32);
        LandValueHeatmap { values }
    }

    #[inline(always)]
    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < HEATMAP_SIZE && y < HEATMAP_SIZE {
            self.values[y * HEATMAP_SIZE + x]
        } else {
            0.0
        }
    }

    /// Difusión: cada celda promedia con sus 4 vecinos
    pub fn diffuse(&mut self) {
        let mut new_values = self.values.clone();
        for y in 1..HEATMAP_SIZE - 1 {
            for x in 1..HEATMAP_SIZE - 1 {
                let idx = y * HEATMAP_SIZE + x;
                let avg = (
                    self.values[(y-1)*HEATMAP_SIZE + x] +
                    self.values[(y+1)*HEATMAP_SIZE + x] +
                    self.values[y*HEATMAP_SIZE + (x-1)] +
                    self.values[y*HEATMAP_SIZE + (x+1)]
                ) * 0.25;
                new_values[idx] = self.values[idx] * (1.0 - DIFFUSION_RATE) + avg * DIFFUSION_RATE;
            }
        }
        self.values.copy_from_slice(&new_values);
    }
}

/// Mapa de contaminación atmosférica y terrestre.
/// Mapa de contaminación atmosférica y terrestre.
/// 0 = limpio, 10 = inhabitable.
#[repr(align(64))]
pub struct PollutionHeatmap {
    pub values: Vec<f32>,
}

impl PollutionHeatmap {
    pub fn new() -> Self {
        let total = HEATMAP_SIZE * HEATMAP_SIZE;
        PollutionHeatmap { values: vec![0.0_f32; total] }
    }

    #[inline(always)]
    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < HEATMAP_SIZE && y < HEATMAP_SIZE {
            self.values[y * HEATMAP_SIZE + x]
        } else {
            0.0
        }
    }

    /// Difusión y decaimiento de contaminación
    pub fn diffuse_and_decay(&mut self) {
        let mut new_values = vec![0.0_f32; HEATMAP_SIZE * HEATMAP_SIZE];
        for y in 1..HEATMAP_SIZE - 1 {
            for x in 1..HEATMAP_SIZE - 1 {
                let idx = y * HEATMAP_SIZE + x;
                let current = self.values[idx];
                let avg = (
                    self.values[(y-1)*HEATMAP_SIZE + x] +
                    self.values[(y+1)*HEATMAP_SIZE + x] +
                    self.values[y*HEATMAP_SIZE + (x-1)] +
                    self.values[y*HEATMAP_SIZE + (x+1)]
                ) * 0.25;
                new_values[idx] = (current * (1.0 - DIFFUSION_RATE) + avg * DIFFUSION_RATE)
                    * (1.0 - POLLUTION_DECAY);
            }
        }
        self.values.copy_from_slice(&new_values);
    }
}

// ---------------------------------------------------------------------------
// ACTUALIZACIÓN DEL SISTEMA
// ---------------------------------------------------------------------------

/// Actualiza contaminación y valor del suelo.
/// Se llama cada HEATMAP_UPDATE_INTERVAL ticks.
pub fn update_heatmaps(gw: &mut GameWorld) {
    // ---- Fase 1: Generar contaminación (industrias, fábricas, tráfico) ----
    for (_entity, (pos, building)) in gw.world
        .query::<(&Position, &ConstructionState)>()
        .iter()
    {
        let gx = pos.x as usize;
        let gy = pos.y as usize;
        if gx >= HEATMAP_SIZE || gy >= HEATMAP_SIZE { continue; }

        let pollution_gen = match building.building_type {
            BuildingType::Factory => 0.5,
            _ => 0.0,
        };

        if pollution_gen > 0.0 {
            for dy in -3i32..=3 {
                for dx in -3i32..=3 {
                    let nx = (gx as i32 + dx).max(0).min(HEATMAP_SIZE as i32 - 1) as usize;
                    let ny = (gy as i32 + dy).max(0).min(HEATMAP_SIZE as i32 - 1) as usize;
                    let dist = ((dx * dx + dy * dy) as f32).sqrt().max(1.0);
                    let idx = ny * HEATMAP_SIZE + nx;
                    gw.pollution_map.values[idx] = 
                        (gw.pollution_map.values[idx] + pollution_gen / dist).min(10.0);
                }
            }
        }
    }

    // Tráfico también genera contaminación
    for (_entity, (pos, _car)) in gw.world
        .query::<(&Position, &crate::ecs::TrafficCar)>()
        .iter()
    {
        let gx = pos.x as usize;
        let gy = pos.y as usize;
        if gx < HEATMAP_SIZE && gy < HEATMAP_SIZE {
            let idx = gy * HEATMAP_SIZE + gx;
            gw.pollution_map.values[idx] = (gw.pollution_map.values[idx] + 0.01).min(10.0);
        }
    }

    // ---- Fase 2: Difusión y decaimiento de contaminación ----
    let mut new_pollution = vec![0.0_f32; HEATMAP_SIZE * HEATMAP_SIZE];
    for y in 1..HEATMAP_SIZE - 1 {
        for x in 1..HEATMAP_SIZE - 1 {
            let idx = y * HEATMAP_SIZE + x;
            let current = gw.pollution_map.values[idx];
            let avg = (
                gw.pollution_map.values[(y-1)*HEATMAP_SIZE + x] +
                gw.pollution_map.values[(y+1)*HEATMAP_SIZE + x] +
                gw.pollution_map.values[y*HEATMAP_SIZE + (x-1)] +
                gw.pollution_map.values[y*HEATMAP_SIZE + (x+1)]
            ) * 0.25;
            new_pollution[idx] = (current * (1.0 - DIFFUSION_RATE) + avg * DIFFUSION_RATE)
                * (1.0 - POLLUTION_DECAY);
        }
    }
    gw.pollution_map.values.copy_from_slice(&new_pollution);

    // ---- Fase 3: Actualizar valor del suelo ----
    update_land_values(gw);
}

/// Recalcula el valor del suelo basado en contaminación, parques y calles.
fn update_land_values(gw: &mut GameWorld) {
    let base_value: f32 = 10.0;

    for y in 0..HEATMAP_SIZE {

            // Valor base penalizado por contaminación
            let mut value = base_value - pollution * POLLUTION_VALUE_PENALTY;

            // Penalización por zona industrial (usando terrain_types del terreno)
            let terrain_type = gw.terrain.terrain_types.get(idx).copied().unwrap_or(0);
            if terrain_type >= 4 {
                // Roca/base industrial
                value -= INDUSTRIAL_VALUE_PENALTY * 5.0;
            }

            gw.land_value_map.values[idx] = value.max(1.0);
        }
    }

    // Bonus por parques y buenas calles
    for (_entity, (pos, zone)) in gw.world
        .query::<(&Position, &ZoneComponent)>()
        .iter()
    {
        let gx = pos.x as usize;
        let gy = pos.y as usize;
        if gx >= HEATMAP_SIZE || gy >= HEATMAP_SIZE { continue; }

        match zone.zone_type {
            ZoneType::Park => {
                for dy in -3i32..=3 {
                    for dx in -3i32..=3 {
                        let nx = (gx as i32 + dx).max(0).min(HEATMAP_SIZE as i32 - 1) as usize;
                        let ny = (gy as i32 + dy).max(0).min(HEATMAP_SIZE as i32 - 1) as usize;
                        let dist = ((dx * dx + dy * dy) as f32).sqrt().max(1.0);
                        gw.land_value_map.values[ny * HEATMAP_SIZE + nx] += PARK_VALUE_BOOST / dist;
                    }
                }
            }
            ZoneType::Commercial => {
                // Las zonas comerciales suben el valor en 1 celda
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let nx = (gx as i32 + dx).max(0).min(HEATMAP_SIZE as i32 - 1) as usize;
                        let ny = (gy as i32 + dy).max(0).min(HEATMAP_SIZE as i32 - 1) as usize;
                        gw.land_value_map.values[ny * HEATMAP_SIZE + nx] += ROAD_VALUE_BOOST;
                    }
                }
            }
            _ => {}
        }
    }

    for v in gw.land_value_map.values.iter_mut() {
        *v = (*v).min(MAX_LAND_VALUE);
    }
}

/// Alias para compatibilidad con código existente
/// Alias para compatibilidad con código existente
#[inline]
pub fn tick_land_value(gw: &mut GameWorld) {
    update_heatmaps(gw);
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_land_value_new() {
        let lv = LandValueHeatmap::new();
        assert_eq!(lv.values.len(), HEATMAP_SIZE * HEATMAP_SIZE);
        // Todos deberían ser 10.0
        for v in lv.values.iter() {
            assert!((*v - 10.0).abs() < 0.001);
        }
    }

    #[test]
    fn test_pollution_new() {
        let p = PollutionHeatmap::new();
        assert_eq!(p.values.len(), HEATMAP_SIZE * HEATMAP_SIZE);
        for v in p.values.iter() {
            assert!(*v < 0.001);
        }
    }

    #[test]
    fn test_land_value_get() {
        let lv = LandValueHeatmap::new();
        assert!((lv.get(10, 10) - 10.0).abs() < 0.001);
        assert_eq!(lv.get(500, 500), 0.0); // fuera de rango
    }

    #[test]
    fn test_pollution_get() {
        let p = PollutionHeatmap::new();
        assert!(p.get(10, 10) < 0.001);
        assert_eq!(p.get(500, 500), 0.0);
    }
}