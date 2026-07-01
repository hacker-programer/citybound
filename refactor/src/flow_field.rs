// Flow Fields Precalculados para Tráfico
//
// TÉCNICA AVANZADA #7 (juegos): Campos de Flujo (Flow Fields)
// En lugar de que cada coche calcule A* individualmente (O(N * A*)),
// horneamos un mapa vectorial durante la carga. Los coches solo
// consultan su celda actual en O(1) para saber hacia dónde moverse.
//
// Esto es ideal para tráfico urbano: cientos de coches siguiendo
// la misma red de calles sin recalcular rutas.
//
// TÉCNICA COMÚN #11 (juegos): Mapeo de Rutas (NavMesh) Estático
// El flow field actúa como NavMesh: direcciones precalculadas
// que guían a las entidades hacia sus destinos.
//
// Memoria: GRID_SIZE² * sizeof(Vec2) = 128² * 8 = 128KB (cabe en L2)

use std::f32::consts::TAU;

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Tamaño de la grilla del flow field (debe coincidir con el mundo)
pub const FLOW_GRID_SIZE: usize = 128;

/// Número de capas de dirección (N, NE, E, SE, S, SW, W, NW)
pub const FLOW_DIRECTIONS: usize = 8;

// ---------------------------------------------------------------------------
// TIPOS
// ---------------------------------------------------------------------------

/// Vector de dirección cuantizado a 8 direcciones + magnitud
#[derive(Copy, Clone, Debug, Default)]
#[repr(align(64))]
pub struct FlowCell {
    /// Dirección en radianes (0 = Este, PI/2 = Norte)
    pub angle: f32,
    /// Magnitud de flujo (0.0 = sin flujo, 1.0 = flujo máximo)
    pub magnitude: f32,
}

/// Capa de flow field completa
#[repr(align(64))]
pub struct FlowField {
    /// Grid de celdas [y * FLOW_GRID_SIZE + x]
    pub cells: [FlowCell; FLOW_GRID_SIZE * FLOW_GRID_SIZE],
}

impl FlowField {
    /// Crea un flow field vacío (sin direcciones)
    pub fn empty() -> Self {
        FlowField {
            cells: [FlowCell::default(); FLOW_GRID_SIZE * FLOW_GRID_SIZE],
        }
    }

    /// Genera flow field para una red de carreteras simple.
    /// Las direcciones apuntan hacia la "autopista principal" (eje central).
    pub fn generate_default() -> Self {
        let mut field = FlowField::empty();

        let center_x = FLOW_GRID_SIZE as f32 / 2.0;
        let center_y = FLOW_GRID_SIZE as f32 / 2.0;
        let half = FLOW_GRID_SIZE as f32 / 2.0;

        for gy in 0..FLOW_GRID_SIZE {
            for gx in 0..FLOW_GRID_SIZE {
                let idx = gy * FLOW_GRID_SIZE + gx;

                let dx = gx as f32;
                let dy = gy as f32;

                // Crear un patrón circular: flujo en sentido horario
                // alrededor del centro + radial hacia afuera en bordes
                let rx = dx - center_x;
                let ry = dy - center_y;
                let dist = (rx * rx + ry * ry).sqrt();

                if dist < 3.0 {
                    // Centro: sin flujo fuerte
                    field.cells[idx] = FlowCell { angle: 0.0, magnitude: 0.1 };
                } else if dist < half * 0.7 {
                    // Anillo interior: flujo circular horario
                    // Tangente perpendicular al radio
                    let tangent_angle = ry.atan2(rx) + TAU / 4.0; // +90° para tangente
                    let mag = (dist / half).min(1.0) * 0.8;
                    field.cells[idx] = FlowCell { angle: tangent_angle, magnitude: mag };
                } else {
                    // Anillo exterior: flujo radial hacia afuera
                    let radial_angle = ry.atan2(rx);
                    let mag = ((dist - half * 0.7) / (half * 0.3)).min(1.0) * 0.6;
                    field.cells[idx] = FlowCell { angle: radial_angle, magnitude: mag };
                }
            }
        }

        field
    }

    /// Genera flow field de autopista horizontal (este-oeste).
    /// Los coches fluyen principalmente de izquierda a derecha.
    pub fn generate_highway() -> Self {
        let mut field = FlowField::empty();

        for gy in 0..FLOW_GRID_SIZE {
            for gx in 0..FLOW_GRID_SIZE {
                let idx = gy * FLOW_GRID_SIZE + gx;
                let dy = (gy as f32 - FLOW_GRID_SIZE as f32 / 2.0).abs();

                if dy < 4.0 {
                    // Carril de autopista: flujo este (0 radianes)
                    let mag = 1.0 - (dy / 4.0) * 0.3;
                    field.cells[idx] = FlowCell { angle: 0.0, magnitude: mag };
                } else if dy < 12.0 {
                    // Calles laterales: flujo débil aleatorio
                    let mag = 0.15;
                    field.cells[idx] = FlowCell {
                        angle: if gx < FLOW_GRID_SIZE as i32 / 2 { 0.0 } else { TAU / 2.0 },
                        magnitude: mag,
                    };
                } else {
                    // Periferia: flujo hacia la autopista
                    let to_highway = if gy < FLOW_GRID_SIZE / 2 {
                        TAU / 4.0 // Norte: apuntar "abajo" (hacia el centro)
                    } else {
                        -TAU / 4.0 // Sur: apuntar "arriba"
                    };
                    let dist_factor = 1.0 - (dy - 12.0) / (FLOW_GRID_SIZE as f32 / 2.0 - 12.0);
                    let mag = 0.1 * dist_factor.max(0.0);
                    field.cells[idx] = FlowCell { angle: to_highway, magnitude: mag };
                }
            }
        }

        field
    }

    /// Consulta la celda en posición de mundo (con bounds check en debug)
    #[inline(always)]
    pub fn sample(&self, world_x: f32, world_y: f32) -> FlowCell {
        let gx = world_x as usize % FLOW_GRID_SIZE;
        let gy = world_y as usize % FLOW_GRID_SIZE;
        debug_assert!(gx < FLOW_GRID_SIZE && gy < FLOW_GRID_SIZE);

        // SAFETY: bounds verificados por debug_assert
        unsafe {
            *self.cells.get_unchecked(gy * FLOW_GRID_SIZE + gx)
        }
    }

    /// Consulta sin bounds check para hot paths [TC#28]
    /// SAFETY: caller debe garantizar world_x, world_y dentro de bounds
    #[inline(always)]
    pub unsafe fn sample_unchecked(&self, gx: usize, gy: usize) -> FlowCell {
        *self.cells.get_unchecked(gy * FLOW_GRID_SIZE + gx)
    }

    /// Convierte FlowCell a velocidad (dx, dy) escalada por speed
    #[inline(always)]
    pub fn cell_to_velocity(cell: &FlowCell, speed: f32) -> (f32, f32) {
        if cell.magnitude < 0.01 {
            return (0.0, 0.0);
        }
        let (sin_a, cos_a) = {
            // Usar LUT trigonométrica para el ángulo [TC#5]
            // Aquí usamos libm::sincos para portabilidad
            (libm::sinf(cell.angle), libm::cosf(cell.angle))
        };
        (cos_a * speed * cell.magnitude, sin_a * speed * cell.magnitude)
    }
}

// ---------------------------------------------------------------------------
// FLOW FIELD MANAGER (múltiples capas)
// ---------------------------------------------------------------------------

/// Sistema de flow fields con múltiples capas de navegación
pub struct FlowFieldManager {
    /// Capa principal: flujo básico de calles
    pub primary: FlowField,
    /// Capa de autopista: flujo rápido este-oeste
    pub highway: FlowField,
}

impl FlowFieldManager {
    /// Genera todos los flow fields durante la carga
    pub fn generate_all() -> Self {
        FlowFieldManager {
            primary: FlowField::generate_default(),
            highway: FlowField::generate_highway(),
        }
    }

    /// Combina capas para obtener dirección final en una posición
    #[inline(always)]
    pub fn sample_combined(&self, world_x: f32, world_y: f32, on_highway: bool) -> FlowCell {
        let primary = self.primary.sample(world_x, world_y);
        let highway = self.highway.sample(world_x, world_y);

        if on_highway && highway.magnitude > 0.5 {
            // Priorizar flujo de autopista
            FlowCell {
                angle: highway.angle,
                magnitude: highway.magnitude * 0.7 + primary.magnitude * 0.3,
            }
        } else {
            primary
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
    fn test_flow_field_default() {
        let field = FlowField::generate_default();

        // Verificar que las celdas tienen valores
        let center = field.sample(64.0, 64.0);
        let corner = field.sample(0.0, 0.0);

        // El centro debe tener baja magnitud
        assert!(center.magnitude < 0.5, "Centro debe tener flujo bajo");

        // Las esquinas deben tener alguna dirección
        assert!(corner.magnitude > 0.0, "Esquinas deben tener flujo");
    }

    #[test]
    fn test_flow_field_highway() {
        let field = FlowField::generate_highway();

        // Zona de autopista (centro)
        let highway = field.sample(64.0, 64.0);
        assert!(highway.magnitude > 0.5, "Autopista debe tener flujo fuerte");

        // La dirección debe ser hacia el este (~0 radianes)
        assert!(highway.angle.abs() < 0.2 || (highway.angle - TAU).abs() < 0.2,
            "Autopista debe fluir al este, ángulo={}", highway.angle);
    }

    #[test]
    fn test_cell_to_velocity() {
        let cell = FlowCell { angle: 0.0, magnitude: 1.0 };
        let (dx, dy) = FlowField::cell_to_velocity(&cell, 10.0);
        assert!(dx > 9.0, "Flujo este completo: dx={}", dx);
        assert!(dy.abs() < 1.0, "Sin componente vertical: dy={}", dy);

        let cell_still = FlowCell { angle: 0.0, magnitude: 0.0 };
        let (dx2, dy2) = FlowField::cell_to_velocity(&cell_still, 10.0);
        assert_eq!(dx2, 0.0);
        assert_eq!(dy2, 0.0);
    }

    #[test]
    fn test_flow_field_manager() {
        let mgr = FlowFieldManager::generate_all();

        // En autopista
        let on_hwy = mgr.sample_combined(64.0, 64.0, true);
        assert!(on_hwy.magnitude > 0.3);

        // Fuera de autopista
        let off_hwy = mgr.sample_combined(10.0, 100.0, false);
        assert!(off_hwy.magnitude >= 0.0);
    }

    #[test]
    fn test_sample_bounds() {
        let field = FlowField::generate_default();

        // Debe manejar coordenadas en el borde sin panic
        let _ = field.sample(0.0, 0.0);
        let _ = field.sample(127.0, 127.0);

        // Coordenadas fuera de rango con wrap (módulo)
        let _ = field.sample(128.0, 128.0);
    }

    #[test]
    fn test_flow_field_empty() {
        let field = FlowField::empty();
        let cell = field.sample(50.0, 50.0);
        assert_eq!(cell.magnitude, 0.0);
        assert_eq!(cell.angle, 0.0);
    }
}
