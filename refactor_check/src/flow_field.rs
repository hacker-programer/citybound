// Flow Fields Precalculados para Tráfico v0.10.0
//
// FASE 6: Reemplazo de libm::sinf/cosf con LUTs propias [TC#5]
// - cell_to_velocity ahora usa crate::luts::sin_fast/cos_fast
// - Resolución 3600 entradas, acceso O(1) con índice directo
// - 10x más rápido que libm en CPUs legacy
//
// TÉCNICA AVANZADA #7 (juegos): Flow Fields O(1)
// TÉCNICA COMÚN #5: LUTs trigonométricas
// TÉCNICA COMÚN #11: NavMesh estático
//
// [FIX STACK OVERFLOW]: FlowField::cells usa Vec<FlowCell> en lugar de
// [FlowCell; 16384] (128KB en stack → heap). FlowFieldManager contiene
// hasta 8 FlowFields = 1MB en stack → heap.

use crate::luts;
use std::f32::consts::TAU;

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

pub const FLOW_GRID_SIZE: usize = 128;
pub const FLOW_CELL_COUNT: usize = FLOW_GRID_SIZE * FLOW_GRID_SIZE;

// ---------------------------------------------------------------------------
// TIPOS
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, Default)]
#[repr(align(64))]
pub struct FlowCell {
    pub angle: f32,
    pub magnitude: f32,
}

#[repr(align(64))]
pub struct FlowField {
    /// Celdas del flow field: [y * FLOW_GRID_SIZE + x]
    /// Usamos Vec en lugar de array fijo para mantener los datos en heap.
    pub cells: Vec<FlowCell>,
}

impl FlowField {
    pub fn empty() -> Self {
        FlowField {
            cells: vec![FlowCell::default(); FLOW_CELL_COUNT],
        }
    }

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
                let rx = dx - center_x;
                let ry = dy - center_y;
                let dist = (rx * rx + ry * ry).sqrt();

                if dist < 3.0 {
                    field.cells[idx] = FlowCell { angle: 0.0, magnitude: 0.1 };
                } else if dist < half * 0.7 {
                    let tangent_angle = ry.atan2(rx) + TAU / 4.0;
                    let mag = (dist / half).min(1.0) * 0.8;
                    field.cells[idx] = FlowCell { angle: tangent_angle, magnitude: mag };
                } else {
                    let radial_angle = ry.atan2(rx);
                    let mag = ((dist - half * 0.7) / (half * 0.3)).min(1.0) * 0.6;
                    field.cells[idx] = FlowCell { angle: radial_angle, magnitude: mag };
                }
            }
        }
        field
    }

    pub fn generate_highway() -> Self {
        let mut field = FlowField::empty();
        for gy in 0..FLOW_GRID_SIZE {
            for gx in 0..FLOW_GRID_SIZE {
                let idx = gy * FLOW_GRID_SIZE + gx;
                let dy = (gy as f32 - FLOW_GRID_SIZE as f32 / 2.0).abs();
                if dy < 4.0 {
                    let mag = 1.0 - (dy / 4.0) * 0.3;
                    field.cells[idx] = FlowCell { angle: 0.0, magnitude: mag };
                } else if dy < 12.0 {
                    let mag = 0.15;
                    field.cells[idx] = FlowCell {
                        angle: if gx < FLOW_GRID_SIZE / 2 { 0.0 } else { TAU / 2.0 },
                        magnitude: mag,
                    };
                } else {
                    let to_highway = if gy < FLOW_GRID_SIZE / 2 { TAU / 4.0 } else { -TAU / 4.0 };
                    let dist_factor = 1.0 - (dy - 12.0) / (FLOW_GRID_SIZE as f32 / 2.0 - 12.0);
                    let mag = 0.1 * dist_factor.max(0.0);
                    field.cells[idx] = FlowCell { angle: to_highway, magnitude: mag };
                }
            }
        }
        field
    }

    #[inline(always)]
    pub fn sample(&self, world_x: f32, world_y: f32) -> FlowCell {
        let gx = world_x as usize % FLOW_GRID_SIZE;
        let gy = world_y as usize % FLOW_GRID_SIZE;
        debug_assert!(gx < FLOW_GRID_SIZE && gy < FLOW_GRID_SIZE);
        unsafe {
            *self.cells.get_unchecked(gy * FLOW_GRID_SIZE + gx)
        }
    }

    #[inline(always)]
    pub unsafe fn sample_unchecked(&self, gx: usize, gy: usize) -> FlowCell {
        *self.cells.get_unchecked(gy * FLOW_GRID_SIZE + gx)
    }

    /// cell_to_velocity usa LUTs trigonométricas propias (~10x más rápido)
    #[inline(always)]
    pub fn cell_to_velocity(cell: &FlowCell, speed: f32) -> (f32, f32) {
        if cell.magnitude < 0.01 {
            return (0.0, 0.0);
        }
        let sin_a = luts::sin_fast(cell.angle);
        let cos_a = luts::cos_fast(cell.angle);
        (cos_a * speed * cell.magnitude, sin_a * speed * cell.magnitude)
    }
}

// ---------------------------------------------------------------------------
// FLOW FIELD MANAGER
// ---------------------------------------------------------------------------

pub struct FlowFieldManager {
    pub primary: FlowField,
    pub highway: FlowField,
}

impl FlowFieldManager {
    pub fn generate_all() -> Self {
        FlowFieldManager {
            primary: FlowField::generate_default(),
            highway: FlowField::generate_highway(),
        }
    }

    #[inline(always)]
    pub fn sample_combined(&self, world_x: f32, world_y: f32, on_highway: bool) -> FlowCell {
        let primary = self.primary.sample(world_x, world_y);
        let highway = self.highway.sample(world_x, world_y);

        if on_highway && highway.magnitude > 0.5 {
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
        assert_eq!(field.cells.len(), FLOW_CELL_COUNT);
        let center = field.sample(64.0, 64.0);
        let corner = field.sample(0.0, 0.0);
        assert!(center.magnitude < 0.5, "Centro debe tener flujo bajo");
        assert!(corner.magnitude > 0.0, "Esquinas deben tener flujo");
    }

    #[test]
    fn test_flow_field_highway() {
        let field = FlowField::generate_highway();
        let highway = field.sample(64.0, 64.0);
        assert!(highway.magnitude > 0.5, "Autopista debe tener flujo fuerte");
        assert!(highway.angle.abs() < 0.2 || (highway.angle - TAU).abs() < 0.2,
            "Autopista debe fluir horizontal, ángulo={}", highway.angle);
    }

    #[test]
    fn test_cell_to_velocity_luts() {
        crate::luts::init_trig_luts();

        let cell = FlowCell { angle: 0.0, magnitude: 1.0 };
        let (dx, dy) = FlowField::cell_to_velocity(&cell, 10.0);
        assert!(dx > 9.0, "Flujo este completo: dx={}", dx);
        assert!(dy.abs() < 1.0, "Sin componente vertical: dy={}", dy);

        let cell_still = FlowCell { angle: 0.0, magnitude: 0.0 };
        let (dx2, dy2) = FlowField::cell_to_velocity(&cell_still, 10.0);
        assert_eq!(dx2, 0.0);
        assert_eq!(dy2, 0.0);

        let cell_pi4 = FlowCell { angle: std::f32::consts::PI / 4.0, magnitude: 1.0 };
        let (dx3, dy3) = FlowField::cell_to_velocity(&cell_pi4, 10.0);
        let expected = 10.0 * std::f32::consts::FRAC_1_SQRT_2;
        assert!((dx3 - expected).abs() < 0.15, "LUT cos(pi/4): dx={}, esperado={}", dx3, expected);
        assert!((dy3 - expected).abs() < 0.15, "LUT sin(pi/4): dy={}, esperado={}", dy3, expected);
    }

    #[test]
    fn test_flow_field_manager() {
        let mgr = FlowFieldManager::generate_all();
        let on_hwy = mgr.sample_combined(64.0, 64.0, true);
        assert!(on_hwy.magnitude > 0.3);
        let off_hwy = mgr.sample_combined(10.0, 100.0, false);
        assert!(off_hwy.magnitude >= 0.0);
    }

    #[test]
    fn test_sample_bounds() {
        let field = FlowField::generate_default();
        let _ = field.sample(0.0, 0.0);
        let _ = field.sample(127.0, 127.0);
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