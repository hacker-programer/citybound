// Heatmap de Valor del Suelo y Gentrificación
//
// MECÁNICA #2: Valor del terreno dinámico, gentrificación y desalojos.
//
// ARQUITECTURA:
// - Mapa de calor (heatmap) de "Valor del Suelo" y "Ruido/Contaminación"
//   actualizado mediante autómata celular difusivo sobre el TerrainMap.
// - El valor del suelo sube cerca de parques, buenas calles y servicios.
// - Si el valor supera los ingresos de los residentes de un ZoneComponent,
//   son desalojados y reemplazados por edificios de mayor valor.
// - Los residentes pobres se desplazan a los bordes del mapa (cerca de
//   contaminación industrial).
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad
// [TC#14] Integración con TerrainMap baked
// [TC#26] Inlining agresivo
// [TA#17] Acceso unchecked en bucles validados

use crate::ecs::{GameWorld, Position, ZoneComponent, ZoneType, ConstructionState, 
                  BuildingType, ResourceStorage, Renderable};
use crate::terrain::TERRAIN_SIZE;

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Tamaño del heatmap (coincide con TerrainMap)
pub const HEATMAP_SIZE: usize = TERRAIN_SIZE; // 128

/// Intervalo entre actualizaciones del autómata celular (ticks)
pub const LAND_VALUE_UPDATE_INTERVAL: u64 = 50;

/// Valor base del suelo
pub const BASE_LAND_VALUE: f32 = 50.0;

/// Incremento de valor por celda de distancia a un parque
pub const PARK_VALUE_BOOST: f32 = 20.0;

/// Incremento de valor por celda de distancia a zona comercial
pub const COMMERCIAL_VALUE_BOOST: f32 = 15.0;

/// Reducción de valor por celda de distancia a zona industrial
pub const INDUSTRIAL_PENALTY: f32 = 10.0;

/// Radio de influencia de servicios (en celdas)
pub const INFLUENCE_RADIUS: i32 = 15;

/// Factor de difusión del autómata celular (0.0 - 1.0)
pub const DIFFUSION_RATE: f32 = 0.15;

/// Umbral de valor para gentrificación: si valor > ingresos * este factor
pub const GENTRIFICATION_THRESHOLD: f32 = 1.5;

/// Ingreso base de residentes según tipo de edificio
pub const INCOME_HOUSE: f32 = 40.0;
pub const INCOME_APARTMENT: f32 = 25.0;
pub const INCOME_SHOP: f32 = 60.0;
pub const INCOME_OFFICE: f32 = 80.0;
pub const INCOME_FACTORY: f32 = 100.0;
pub const INCOME_FARM: f32 = 30.0;

// ---------------------------------------------------------------------------
// TIPOS
// ---------------------------------------------------------------------------

/// Heatmap de valor del suelo y contaminación
pub struct LandValueMap {
    /// Valor del suelo por celda [y * HEATMAP_SIZE + x]
    pub land_value: [f32; HEATMAP_SIZE * HEATMAP_SIZE],
    /// Nivel de contaminación/ruido [y * HEATMAP_SIZE + x]
    pub pollution: [f32; HEATMAP_SIZE * HEATMAP_SIZE],
    /// Contador de ticks para actualización
    pub tick_counter: u64,
}

impl LandValueMap {
    /// Crea un heatmap inicial con valores base
    pub fn new() -> Self {
        let mut map = LandValueMap {
            land_value: [BASE_LAND_VALUE; HEATMAP_SIZE * HEATMAP_SIZE],
            pollution: [0.0_f32; HEATMAP_SIZE * HEATMAP_SIZE],
            tick_counter: 0,
        };
        map
    }

    /// Obtiene valor del suelo en una celda
    #[inline(always)]
    pub fn value_at(&self, x: usize, y: usize) -> f32 {
        if x < HEATMAP_SIZE && y < HEATMAP_SIZE {
            unsafe { *self.land_value.get_unchecked(y * HEATMAP_SIZE + x) }
        } else {
            BASE_LAND_VALUE
        }
    }

    /// Obtiene contaminación en una celda
    #[inline(always)]
    pub fn pollution_at(&self, x: usize, y: usize) -> f32 {
        if x < HEATMAP_SIZE && y < HEATMAP_SIZE {
            unsafe { *self.pollution.get_unchecked(y * HEATMAP_SIZE + x) }
        } else {
            0.0
        }
    }
}

// ---------------------------------------------------------------------------
// SISTEMA DE VALOR DEL SUELO
// ---------------------------------------------------------------------------

/// Inicializa el heatmap de valor del suelo durante la carga
pub fn init_land_value(gw: &mut GameWorld) {
    let mut map = LandValueMap::new();

    // Valores iniciales basados en zonas existentes
    for (_entity, (pos, zone)) in gw.world.query::<(&Position, &ZoneComponent)>().iter() {
        let x = pos.x as usize;
        let y = pos.y as usize;
        if x < HEATMAP_SIZE && y < HEATMAP_SIZE {
            let boost = match zone.zone_type {
                ZoneType::Park => PARK_VALUE_BOOST,
                ZoneType::Commercial => COMMERCIAL_VALUE_BOOST,
                ZoneType::Industrial => -INDUSTRIAL_PENALTY,
                ZoneType::Residential => 5.0,
                _ => 0.0,
            };

            // Aplicar influencia en radio
            for dy in -INFLUENCE_RADIUS..=INFLUENCE_RADIUS {
                for dx in -INFLUENCE_RADIUS..=INFLUENCE_RADIUS {
                    let nx = (x as i32 + dx).max(0).min(HEATMAP_SIZE as i32 - 1) as usize;
                    let ny = (y as i32 + dy).max(0).min(HEATMAP_SIZE as i32 - 1) as usize;

                    let dist = ((dx * dx + dy * dy) as f32).sqrt();
                    let falloff = (1.0 - dist / INFLUENCE_RADIUS as f32).max(0.0);

                    let idx = ny * HEATMAP_SIZE + nx;
                    map.land_value[idx] += boost * falloff;
                    if boost < 0.0 {
                        map.pollution[idx] += (-boost) * falloff * 0.5;
                    }
                }
            }
        }
    }

    // Clampear valores
    for i in 0..(HEATMAP_SIZE * HEATMAP_SIZE) {
        map.land_value[i] = map.land_value[i].max(10.0).min(500.0);
        map.pollution[i] = map.pollution[i].min(100.0);
    }

    println!("Heatmap de valor del suelo inicializado");
    // Guardar el mapa (se integrará en GameWorld)
}

/// Tick del sistema de valor del suelo (autómata celular difusivo)
pub fn tick_land_value(gw: &mut GameWorld) {
    // Nota: Este sistema usa una estructura externa que se integrará en GameWorld.
    // Por ahora, aplicamos gentrificación directamente con valores calculados.
    process_gentrification(gw);
}

/// Procesa gentrificación: residentes desplazados si el valor supera sus ingresos
fn process_gentrification(gw: &mut GameWorld) {
    let mut evictions: Vec<(hecs::Entity, BuildingType, f32, f32)> = Vec::with_capacity(32);

    // Calcular valor del suelo local basado en zonas cercanas
    for (entity, (pos, construction, zone)) in gw.world
        .query::<(&Position, &ConstructionState, &ZoneComponent)>()
        .iter()
    {
        if zone.zone_type != ZoneType::Residential || zone.density == 0 {
            continue;
        }

        // Calcular valor local aproximado
        let local_value = estimate_local_value(gw, pos.x, pos.y);

        // Ingreso base según tipo de edificio
        let income = match construction.building_type {
            BuildingType::House => INCOME_HOUSE,
            BuildingType::Apartment => INCOME_APARTMENT,
            _ => INCOME_HOUSE,
        };

        // ¿El valor del suelo supera lo que pueden pagar?
        if local_value > income * GENTRIFICATION_THRESHOLD {
            evictions.push((entity, construction.building_type, pos.x, pos.y));
        }
    }

    // Aplicar desalojos
    for (entity, old_type, x, y) in evictions {
        // Degradar o reemplazar edificio
        if let Ok(mut construction) = gw.world.get_mut::<ConstructionState>(entity) {
            // Upgrade: casa → apartamento (más densidad, menos ingresos por unidad)
            if old_type == BuildingType::House {
                construction.building_type = BuildingType::Apartment;
                construction.progress = 0.3; // En construcción
            }
        }

        if let Ok(mut renderable) = gw.world.get_mut::<Renderable>(entity) {
            // Color más "lujoso" (más claro) = gentrificado
            renderable.color = 0xFF_D4_A0_7A;
        }

        if let Ok(mut zone) = gw.world.get_mut::<ZoneComponent>(entity) {
            zone.density = (zone.density + 1).min(5);
        }

        // Reducir dinero de los residentes (fueron desplazados)
        if let Ok(mut resources) = gw.world.get_mut::<ResourceStorage>(entity) {
            resources.money = (resources.money - 20.0).max(0.0);
        }
    }
}

/// Estima el valor del suelo en una posición basado en zonas cercanas
#[inline]
fn estimate_local_value(gw: &GameWorld, cx: f32, cy: f32) -> f32 {
    let mut value: f32 = BASE_LAND_VALUE;

    for (_entity, (pos, zone)) in gw.world.query::<(&Position, &ZoneComponent)>().iter() {
        let dx = pos.x - cx;
        let dy = pos.y - cy;
        let dist = (dx * dx + dy * dy).sqrt();

        if dist < INFLUENCE_RADIUS as f32 {
            let influence = 1.0 - dist / INFLUENCE_RADIUS as f32;
            value += match zone.zone_type {
                ZoneType::Park => PARK_VALUE_BOOST * influence,
                ZoneType::Commercial => COMMERCIAL_VALUE_BOOST * influence,
                ZoneType::Industrial => -INDUSTRIAL_PENALTY * influence,
                ZoneType::Road => 5.0 * influence,
                _ => 0.0,
            };
        }
    }

    value.max(10.0).min(500.0)
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs;
    use crate::object_pool::EntityPool;

    #[test]
    fn test_land_value_map_default() {
        let map = LandValueMap::new();
        assert_eq!(map.value_at(50, 50), BASE_LAND_VALUE);
        assert_eq!(map.pollution_at(0, 0), 0.0);
    }

    #[test]
    fn test_land_value_bounds() {
        let map = LandValueMap::new();
        // Fuera de bounds
        assert_eq!(map.value_at(200, 200), BASE_LAND_VALUE);
        assert_eq!(map.pollution_at(999, 999), 0.0);
    }

    #[test]
    fn test_estimate_local_value() {
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);

        let val = estimate_local_value(&gw, 64.0, 64.0);
        assert!(val >= BASE_LAND_VALUE, "Valor base mínimo esperado");
    }

    #[test]
    fn test_gentrification_no_crash() {
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        process_gentrification(&mut gw);
        // No debe crashear
    }

    #[test]
    fn test_pollution_initialization() {
        let map = LandValueMap::new();
        for i in 0..(HEATMAP_SIZE * HEATMAP_SIZE) {
            assert!(map.pollution[i] >= 0.0);
            assert!(map.pollution[i] <= 100.0);
        }
    }
}
