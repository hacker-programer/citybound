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
use crate::bitboard::BitGrid;

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
// HEATMAPS
// ---------------------------------------------------------------------------

/// Mapa de calor de valor del suelo
#[repr(align(64))]
pub struct LandValueHeatmap {
    pub values: [[f32; HEATMAP_SIZE]; HEATMAP_SIZE],
}

impl LandValueHeatmap {
    pub fn new() -> Self {
        LandValueHeatmap {
            values: [[10.0_f32; HEATMAP_SIZE]; HEATMAP_SIZE],
        }
    }

    #[inline(always)]
    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < HEATMAP_SIZE && y < HEATMAP_SIZE {
            unsafe { *self.values.get_unchecked(y).get_unchecked(x) }
        } else {
            10.0
        }
    }

    #[inline(always)]
    pub fn set(&mut self, x: usize, y: usize, value: f32) {
        if x < HEATMAP_SIZE && y < HEATMAP_SIZE {
            self.values[y][x] = value.clamp(0.0, MAX_LAND_VALUE);
        }
    }

    /// Difusión: cada celda promedia con sus 4 vecinos
    pub fn diffuse(&mut self) {
        let mut new_values = self.values;

        for y in 1..(HEATMAP_SIZE - 1) {
            for x in 1..(HEATMAP_SIZE - 1) {
                let center = self.values[y][x];
                let avg_neighbors = (
                    self.values[y-1][x] + self.values[y+1][x] +
                    self.values[y][x-1] + self.values[y][x+1]
                ) / 4.0;

                new_values[y][x] = center * (1.0 - DIFFUSION_RATE)
                    + avg_neighbors * DIFFUSION_RATE;
            }
        }

        self.values = new_values;
    }
}

/// Mapa de calor de contaminación
#[repr(align(64))]
pub struct PollutionHeatmap {
    pub values: [[f32; HEATMAP_SIZE]; HEATMAP_SIZE],
}

impl PollutionHeatmap {
    pub fn new() -> Self {
        PollutionHeatmap {
            values: [[0.0_f32; HEATMAP_SIZE]; HEATMAP_SIZE],
        }
    }

    #[inline(always)]
    pub fn get(&self, x: usize, y: usize) -> f32 {
        if x < HEATMAP_SIZE && y < HEATMAP_SIZE {
            unsafe { *self.values.get_unchecked(y).get_unchecked(x) }
        } else {
            0.0
        }
    }

    /// Difusión de contaminación + decaimiento
    pub fn diffuse_and_decay(&mut self) {
        let mut new_values = self.values;

        for y in 1..(HEATMAP_SIZE - 1) {
            for x in 1..(HEATMAP_SIZE - 1) {
                let center = self.values[y][x];
                let avg_neighbors = (
                    self.values[y-1][x] + self.values[y+1][x] +
                    self.values[y][x-1] + self.values[y][x+1]
                ) / 4.0;

                // Difusión + decaimiento
                let diffused = center * (1.0 - DIFFUSION_RATE * 0.5)
                    + avg_neighbors * DIFFUSION_RATE * 0.5;
                new_values[y][x] = (diffused - POLLUTION_DECAY).max(0.0);
            }
        }

        self.values = new_values;
    }
}

// ---------------------------------------------------------------------------
// SISTEMA PRINCIPAL
// ---------------------------------------------------------------------------

/// Actualiza heatmaps y aplica gentrificación
pub fn tick_land_value(gw: &mut GameWorld) {
    // Solo actualizar cada cierto intervalo
    if gw.sim_tick % HEATMAP_UPDATE_INTERVAL != 0 {
        return;
    }

    // 1. Generar contaminación desde zonas industriales
    generate_pollution(gw);

    // 2. Difundir contaminación
    gw.pollution_map.diffuse_and_decay();

    // 3. Actualizar valor del suelo basado en zonas, servicios, contaminación
    update_land_values(gw);

    // 4. Difundir valor del suelo
    gw.land_value_map.diffuse();

    // 5. Aplicar gentrificación
    apply_gentrification(gw);
}

fn generate_pollution(gw: &mut GameWorld) {
    for (_entity, (pos, zone)) in gw.world
        .query::<(&Position, &ZoneComponent)>()
        .iter()
    {
        if zone.zone_type == ZoneType::Industrial && zone.density > 0 {
            let gx = pos.x as usize;
            let gy = pos.y as usize;
            if gx < HEATMAP_SIZE && gy < HEATMAP_SIZE {
                gw.pollution_map.values[gy][gx] = (gw.pollution_map.values[gy][gx] + 0.5).min(10.0);
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
            gw.pollution_map.values[gy][gx] = (gw.pollution_map.values[gy][gx] + 0.01).min(10.0);
        }
    }
}

fn update_land_values(gw: &mut GameWorld) {
    for y in 0..HEATMAP_SIZE {
        for x in 0..HEATMAP_SIZE {
            let mut value = gw.land_value_map.values[y][x];

            // Penalización por contaminación
            let pollution = gw.pollution_map.values[y][x];
            value -= pollution * POLLUTION_VALUE_PENALTY;

            gw.land_value_map.values[y][x] = value.max(1.0);
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
                // Parque aumenta valor en radio de 3 celdas
                for dy in -3i32..=3 {
                    for dx in -3i32..=3 {
                        let nx = (gx as i32 + dx).max(0).min(HEATMAP_SIZE as i32 - 1) as usize;
                        let ny = (gy as i32 + dy).max(0).min(HEATMAP_SIZE as i32 - 1) as usize;
                        let dist = ((dx * dx + dy * dy) as f32).sqrt().max(1.0);
                        gw.land_value_map.values[ny][nx] += PARK_VALUE_BOOST / dist;
                    }
                }
            }
            ZoneType::Road => {
                gw.land_value_map.values[gy][gx] += ROAD_VALUE_BOOST;
            }
            ZoneType::Industrial => {
                if zone.density > 0 {
                    // Penalización en radio de 2 celdas
                    for dy in -2i32..=2 {
                        for dx in -2i32..=2 {
                            let nx = (gx as i32 + dx).max(0).min(HEATMAP_SIZE as i32 - 1) as usize;
                            let ny = (gy as i32 + dy).max(0).min(HEATMAP_SIZE as i32 - 1) as usize;
                            gw.land_value_map.values[ny][nx] -= INDUSTRIAL_VALUE_PENALTY;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // Clampear valores
    for y in 0..HEATMAP_SIZE {
        for x in 0..HEATMAP_SIZE {
            gw.land_value_map.values[y][x] = gw.land_value_map.values[y][x].clamp(1.0, MAX_LAND_VALUE);
        }
    }
}

/// Gentrificación: si valor del suelo supera ingresos del residente,
/// el edificio se degrada
fn apply_gentrification(gw: &mut GameWorld) {
    let mut to_degrade: Vec<(f32, f32)> = Vec::with_capacity(32);

    for (_entity, (pos, construction, storage)) in gw.world
        .query::<(&Position, &ConstructionState, &ResourceStorage)>()
        .iter()
    {
        if construction.building_type != BuildingType::House
            && construction.building_type != BuildingType::Apartment
        {
            continue;
        }

        let gx = pos.x as usize;
        let gy = pos.y as usize;
        if gx >= HEATMAP_SIZE || gy >= HEATMAP_SIZE { continue; }

        let land_value = gw.land_value_map.values[gy][gx];
        let income = storage.money.max(1.0);

        // Si el valor del suelo es muy alto comparado con ingresos
        if land_value > income * GENTRIFICATION_THRESHOLD {
            to_degrade.push((pos.x, pos.y));
        }
    }

    for (x, y) in to_degrade {
        // Degradar edificio: reducir progreso (se vuelve ruinoso)
        for (_entity, (_pos, construction, _storage)) in gw.world
            .query::<(&Position, &mut ConstructionState, &ResourceStorage)>()
            .iter()
        {
            if (_pos.x - x).abs() < 1.0 && (_pos.y - y).abs() < 1.0
                && construction.progress > 0.3
            {
                construction.progress -= 0.05;
                if construction.progress < 0.1 {
                    construction.progress = 0.0;
                    // Marcar como abandonado
                    gw.world.spawn((
                        Position::new(x, y),
                        crate::supply_chain::AbandonedBuilding { abandoned_ticks: 0 },
                        Renderable::rect(0xFF_66_44_44, 3.0, 3),
                    ));
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// VISUALIZACIÓN EN RENDER
// ---------------------------------------------------------------------------

/// Colores ARGB para heatmap de valor del suelo
pub fn land_value_color(value: f32) -> u32 {
    let normalized = (value / MAX_LAND_VALUE).min(1.0);
    if normalized < 0.2 {
        // Verde = barato
        let g = (normalized * 5.0 * 255.0) as u32;
        0x66_00_00_00 | (g << 8)
    } else if normalized < 0.5 {
        // Amarillo = medio
        let r = ((normalized - 0.2) * 3.33 * 255.0) as u32;
        let g = 200u32;
        0x66_00_00_00 | (r << 16) | (g << 8)
    } else if normalized < 0.8 {
        // Naranja = caro
        let r = 255u32;
        let g = ((1.0 - (normalized - 0.5) * 3.33) * 200.0) as u32;
        0x66_00_00_00 | (r << 16) | (g << 8)
    } else {
        // Rojo = muy caro (gentrificación)
        0x66_FF_22_22
    }
}

/// Colores ARGB para heatmap de contaminación
pub fn pollution_color(value: f32) -> u32 {
    let normalized = (value / 10.0).min(1.0);
    if normalized < 0.3 {
        0x00_00_00_00 // Transparente
    } else if normalized < 0.6 {
        let alpha = ((normalized - 0.3) * 3.33 * 0x66) as u32;
        (alpha << 24) | 0x00_AA_AA_00 // Amarillo sucio
    } else {
        let alpha = ((normalized - 0.6) * 2.5 * 0x88) as u32;
        (alpha << 24) | 0x00_FF_44_00 // Rojo contaminación
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_land_value_heatmap_new() {
        let map = LandValueHeatmap::new();
        assert!((map.get(0, 0) - 10.0).abs() < 0.01);
        assert!((map.get(64, 64) - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_land_value_diffusion() {
        let mut map = LandValueHeatmap::new();
        // Crear un pico artificial
        map.set(64, 64, 100.0);

        map.diffuse();
        map.diffuse();
        map.diffuse();

        // Después de difusión, los vecinos deben tener valores más altos
        let center = map.get(64, 64);
        let neighbor = map.get(65, 64);
        assert!(center < 100.0, "Centro debe perder valor por difusión");
        assert!(neighbor > 10.0, "Vecino debe ganar valor");
    }

    #[test]
    fn test_pollution_decay() {
        let mut map = PollutionHeatmap::new();
        map.values[10][10] = 5.0;

        map.diffuse_and_decay();
        let after = map.get(10, 10);
        assert!(after < 5.0, "Contaminación debe decaer: {}", after);
    }

    #[test]
    fn test_pollution_spread() {
        let mut map = PollutionHeatmap::new();
        map.values[10][10] = 10.0;

        for _ in 0..5 {
            map.diffuse_and_decay();
        }

        let neighbor = map.get(11, 10);
        assert!(neighbor > 0.0, "Contaminación debe difundirse a vecinos");
    }

    #[test]
    fn test_land_value_color() {
        let cheap = land_value_color(5.0);
        let mid = land_value_color(40.0);
        let expensive = land_value_color(90.0);

        assert_ne!(cheap, mid);
        assert_ne!(mid, expensive);
    }

    #[test]
    fn test_pollution_color() {
        let clean = pollution_color(0.0);
        let dirty = pollution_color(8.0);

        assert_eq!(clean, 0x00_00_00_00);
        assert_ne!(dirty, 0x00_00_00_00);
    }

    #[test]
    fn test_heatmap_bounds() {
        let map = LandValueHeatmap::new();
        // Fuera de bounds
        let v = map.get(200, 200);
        assert!((v - 10.0).abs() < 0.01);

        let pmap = PollutionHeatmap::new();
        let p = pmap.get(500, 500);
        assert!((p - 0.0).abs() < 0.01);
    }
}
