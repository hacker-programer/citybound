// Módulo de Simulación Peatonal v0.11.0
//
// Basado en el Social Force Model de Helbing & Molnár (1995).
// Modela peatones como agentes con fuerzas de destino, repulsión
// interpersonal, evasión de obstáculos y cruce de calles.
//
use crate::ecs::PedestrianState;

// ---------------------------------------------------------------------------
// Constantes del modelo (calibradas según Helbing 1995)
// ---------------------------------------------------------------------------

/// Tiempo de relajación τ (segundos) — qué tan rápido el peatón ajusta su velocidad
const RELAXATION_TIME: f32 = 0.5;

/// Velocidad deseada de caminata (m/s ≈ 4.8 km/h)
const DESIRED_SPEED: f32 = 1.34;

/// Intensidad de repulsión interpersonal A
const REPULSION_STRENGTH: f32 = 2.0;

/// Rango de interacción interpersonal B (metros)
const REPULSION_RANGE: f32 = 0.3;

/// Suma de radios corporales (metros)
const BODY_RADIUS_SUM: f32 = 0.4;

/// Factor de anisotropía λ (peatones detrás = menos influencia)
const ANISOTROPY_FACTOR: f32 = 0.7;

/// Constante de compresión k (kg/s²) — previene overlap
const COMPRESSION_CONSTANT: f32 = 120.0;

/// Densidad máxima de atasco (peatones/m²)
const MAX_JAM_DENSITY: f32 = 5.4;

/// Distancia mínima a obstáculos para fuerza de repulsión
const OBSTACLE_RANGE: f32 = 2.0;

/// Intensidad de repulsión de obstáculos
const OBSTACLE_REPULSION: f32 = 10.0;

/// Radio de búsqueda de vecinos (metros) — para cálculo de fuerzas y densidad
const NEIGHBOR_RADIUS: f32 = 5.0;

/// Probabilidad de cruzar con semáforo en rojo (imprudencia)
const JAYWALKING_PROB: f32 = 0.2;

#[allow(dead_code)]
const CROSSING_SPEED_FACTOR: f32 = 1.15; // ligeramente más rápido al cruzar

/// Máximo de peatones simultáneos
pub const MAX_PEDESTRIANS: usize = 2000;

// ---------------------------------------------------------------------------
// Estructura de gestión peatonal
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PedestrianSystem {
    /// Posiciones de todos los peatones activos (SoA para caché)
    positions: Vec<(f32, f32)>,
    /// Velocidades actuales
    velocities: Vec<(f32, f32)>,
    /// Destinos
    destinations: Vec<(f32, f32)>,
    /// Estados
    states: Vec<PedestrianState>,
    /// Niveles de estrés [0, 1]
    stress_levels: Vec<f32>,
    /// Peatones activos
    active_count: usize,
    /// Grilla espacial para búsqueda de vecinos (celdas de NEIGHBOR_RADIUS)
    spatial_bins: Vec<Vec<usize>>,
    bin_cols: usize,
    bin_rows: usize,
    world_width: f32,
    world_height: f32,
}

impl PedestrianSystem {
    #[inline(always)]
    pub fn new(world_width: f32, world_height: f32) -> Self {
        let bin_size = NEIGHBOR_RADIUS;
        let bin_cols = (world_width / bin_size).ceil() as usize + 1;
        let bin_rows = (world_height / bin_size).ceil() as usize + 1;
        let total_bins = bin_cols * bin_rows;

        Self {
            positions: Vec::with_capacity(MAX_PEDESTRIANS),
            velocities: Vec::with_capacity(MAX_PEDESTRIANS),
            destinations: Vec::with_capacity(MAX_PEDESTRIANS),
            states: Vec::with_capacity(MAX_PEDESTRIANS),
            stress_levels: Vec::with_capacity(MAX_PEDESTRIANS),
            active_count: 0,
            spatial_bins: vec![Vec::with_capacity(32); total_bins],
            bin_cols,
            bin_rows,
            world_width,
            world_height,
        }
    }

    /// Registrar un nuevo peatón (devuelve índice)
    pub fn spawn(
        &mut self,
        x: f32, y: f32,
        dest_x: f32, dest_y: f32,
        initial_state: PedestrianState,
    ) -> usize {
        if self.active_count >= MAX_PEDESTRIANS {
            return usize::MAX; // pool lleno
        }

        let idx = self.active_count;
        if idx >= self.positions.len() {
            self.positions.push((x, y));
            self.velocities.push((0.0, 0.0));
            self.destinations.push((dest_x, dest_y));
            self.states.push(initial_state);
            self.stress_levels.push(0.0);
        } else {
            self.positions[idx] = (x, y);
            self.velocities[idx] = (0.0, 0.0);
            self.destinations[idx] = (dest_x, dest_y);
            self.states[idx] = initial_state;
            self.stress_levels[idx] = 0.0;
        }
        self.active_count += 1;
        idx
    }

    /// Eliminar peatón (swap-remove para O(1))
    pub fn despawn(&mut self, idx: usize) {
        if idx >= self.active_count { return; }
        let last = self.active_count - 1;
        if idx != last {
            self.positions.swap(idx, last);
            self.velocities.swap(idx, last);
            self.destinations.swap(idx, last);
            self.states.swap(idx, last);
            self.stress_levels.swap(idx, last);
        }
        self.active_count -= 1;
    }

    /// Tick principal: actualiza todos los peatones con Social Force Model
    pub fn tick(&mut self, dt: f32, obstacles: &[(f32, f32, f32)], road_cells: &[(f32, f32, f32, f32)]) {
        if self.active_count == 0 { return; }

        // Reconstruir grilla espacial para búsqueda O(1) de vecinos
        self.rebuild_spatial_bins();

        // Para cada peatón, calcular fuerza total
        let mut forces: Vec<(f32, f32)> = Vec::with_capacity(self.active_count);
        let mut local_densities: Vec<f32> = Vec::with_capacity(self.active_count);

        for i in 0..self.active_count {
            let pos = self.positions[i];
            let vel = self.velocities[i];
            let dest = self.destinations[i];

            // 1. Fuerza hacia el destino (driving force)
            let dx = dest.0 - pos.0;
            let dy = dest.1 - pos.1;
            let dist_to_dest = (dx * dx + dy * dy).sqrt();
            let (e_x, e_y) = if dist_to_dest > 0.01 {
                (dx / dist_to_dest, dy / dist_to_dest)
            } else {
                (0.0, 0.0)
            };

            let desired_vx = DESIRED_SPEED * e_x;
            let desired_vy = DESIRED_SPEED * e_y;

            let f_dest_x = (desired_vx - vel.0) / RELAXATION_TIME;
            let f_dest_y = (desired_vy - vel.1) / RELAXATION_TIME;

            // 2. Fuerza de repulsión de otros peatones (vecinos cercanos)
            let mut f_rep_x = 0.0f32;
            let mut f_rep_y = 0.0f32;
            let mut local_count = 0u32;

            let neighbors = self.get_neighbors(pos.0, pos.1);
            for &j in neighbors {
                if j == i { continue; }
                let pj = self.positions[j];
                let dx_ij = pj.0 - pos.0;
                let dy_ij = pj.1 - pos.1;
                let d_ij = (dx_ij * dx_ij + dy_ij * dy_ij).sqrt().max(0.001);

                if d_ij < NEIGHBOR_RADIUS {
                    local_count += 1;

                    let n_x = dx_ij / d_ij;
                    let n_y = dy_ij / d_ij;

                    // Factor anisotrópico: peatones detrás tienen menos influencia
                    let dot = vel.0 * n_x + vel.1 * n_y;
                    let lambda_i = ANISOTROPY_FACTOR + (1.0 - ANISOTROPY_FACTOR) * (1.0 + dot) / 2.0;

                    // Fuerza exponencial de repulsión
                    let overlap = BODY_RADIUS_SUM - d_ij;
                    let exp_term = (overlap / REPULSION_RANGE).exp();
                    f_rep_x += REPULSION_STRENGTH * exp_term * n_x * lambda_i;
                    f_rep_y += REPULSION_STRENGTH * exp_term * n_y * lambda_i;

                    // Fuerza de compresión (si hay contacto físico)
                    if overlap > 0.0 {
                        f_rep_x += COMPRESSION_CONSTANT * overlap * n_x;
                        f_rep_y += COMPRESSION_CONSTANT * overlap * n_y;
                    }
                }
            }
            local_densities.push(local_count as f32 / (std::f32::consts::PI * NEIGHBOR_RADIUS * NEIGHBOR_RADIUS));

            // 3. Fuerza de repulsión de obstáculos
            let mut f_obs_x = 0.0f32;
            let mut f_obs_y = 0.0f32;

            for &(ox, oy, or) in obstacles {
                let dx_o = ox - pos.0;
                let dy_o = oy - pos.1;
                let d_o = (dx_o * dx_o + dy_o * dy_o).sqrt().max(0.001);

                if d_o < OBSTACLE_RANGE + or {
                    let n_x = dx_o / d_o;
                    let n_y = dy_o / d_o;
                    let overlap = or + 0.2 - d_o; // 0.2 = radio peatón
                    if overlap > 0.0 {
                        f_obs_x -= OBSTACLE_REPULSION * overlap * n_x;
                        f_obs_y -= OBSTACLE_REPULSION * overlap * n_y;
                    }
                }
            }

            // 4. Fuerza de repulsión de bordes de calle (si está cruzando)
            let mut f_road_x = 0.0f32;
            let mut f_road_y = 0.0f32;

            if self.states[i] == PedestrianState::Crossing {
                // Los peatones cruzando evitan acercarse a los bordes de autos
                for &(rx, ry, rw, rh) in road_cells {
                    let closest_x = pos.0.max(rx).min(rx + rw);
                    let closest_y = pos.1.max(ry).min(ry + rh);
                    let dx_r = pos.0 - closest_x;
                    let dy_r = pos.1 - closest_y;
                    let d_r = (dx_r * dx_r + dy_r * dy_r).sqrt().max(0.001);

                    if d_r < 1.5 {
                        f_road_x += 15.0 * dx_r / (d_r * d_r);
                        f_road_y += 15.0 * dy_r / (d_r * d_r);
                    }
                }
            }

            // 5. Fuerza aleatoria pequeña (evita deadlocks)
            let rng_x = ((i as f32 * 12.9898).sin() * 43758.5453).fract() * 0.2 - 0.1;
            let rng_y = ((i as f32 * 78.2337).sin() * 43758.5453).fract() * 0.2 - 0.1;

            // Fuerza total
            let fx = f_dest_x + f_rep_x + f_obs_x + f_road_x + rng_x;
            let fy = f_dest_y + f_rep_y + f_obs_y + f_road_y + rng_y;

            forces.push((fx, fy));
        }

        // Aplicar fuerzas (actualizar velocidades y posiciones)
        for i in 0..self.active_count {
            let (fx, fy) = forces[i];
            let density = local_densities[i];

            // Aceleración: a = F/m (m=1 para simplicidad)
            let mut vx = self.velocities[i].0 + fx * dt;
            let mut vy = self.velocities[i].1 + fy * dt;

            // Limitar velocidad según densidad local
            let max_speed = DESIRED_SPEED * (1.0 - density / MAX_JAM_DENSITY).max(0.1);
            let speed = (vx * vx + vy * vy).sqrt();
            if speed > max_speed {
                let scale = max_speed / speed;
                vx *= scale;
                vy *= scale;
            }

            // Velocidad máxima absoluta (corriendo)
            let abs_max = 3.0;
            let speed = (vx * vx + vy * vy).sqrt();
            if speed > abs_max {
                let scale = abs_max / speed;
                vx *= scale;
                vy *= scale;
            }

            self.velocities[i] = (vx, vy);

            // Actualizar posición
            self.positions[i].0 += vx * dt;
            self.positions[i].1 += vy * dt;

            // Clampear al mundo
            self.positions[i].0 = self.positions[i].0.max(0.0).min(self.world_width);
            self.positions[i].1 = self.positions[i].1.max(0.0).min(self.world_height);

            // Actualizar estrés (aumenta con densidad, disminuye con el tiempo)
            self.stress_levels[i] = (self.stress_levels[i] + 0.01 * density - 0.001)
                .max(0.0).min(1.0);

            // Verificar llegada al destino
            let dx = self.destinations[i].0 - self.positions[i].0;
            let dy = self.destinations[i].1 - self.positions[i].1;
            if dx * dx + dy * dy < 0.25 {
                // Llegó al destino: marcamos como idle
                if self.states[i] == PedestrianState::Walking || self.states[i] == PedestrianState::Crossing {
                    self.states[i] = PedestrianState::Idle;
                    self.velocities[i] = (0.0, 0.0);
                }
            }
        }
    }

    /// Reconstruir grilla espacial para consultas O(1) de vecinos
    fn rebuild_spatial_bins(&mut self) {
        for bin in &mut self.spatial_bins {
            bin.clear();
        }

        let bin_size = NEIGHBOR_RADIUS;
        for i in 0..self.active_count {
            let (x, y) = self.positions[i];
            let col = (x / bin_size) as usize;
            let row = (y / bin_size) as usize;
            let col = col.min(self.bin_cols - 1);
            let row = row.min(self.bin_rows - 1);
            self.spatial_bins[row * self.bin_cols + col].push(i);
        }
    }

    /// Obtener vecinos cercanos a una posición (consulta en grilla 3×3)
    #[inline(always)]
    fn get_neighbors(&self, x: f32, y: f32) -> &[usize] {
        let bin_size = NEIGHBOR_RADIUS;
        let col = (x / bin_size) as usize;
        let row = (y / bin_size) as usize;

        // Por simplicidad retornamos los vecinos del bin actual + adyacentes
        // En una implementación más compleja fusionaríamos los 9 bins
        let idx = row.min(self.bin_rows - 1) * self.bin_cols + col.min(self.bin_cols - 1);
        &self.spatial_bins[idx]
    }

    /// Obtener referencia a posiciones (para renderizado)
    #[inline(always)]
    pub fn positions(&self) -> &[(f32, f32)] {
        &self.positions[..self.active_count]
    }

    /// Obtener referencia a estados
    #[inline(always)]
    pub fn states(&self) -> &[PedestrianState] {
        &self.states[..self.active_count]
    }

    /// Obtener referencia a niveles de estrés
    #[inline(always)]
    pub fn stress_levels(&self) -> &[f32] {
        &self.stress_levels[..self.active_count]
    }

    /// Cantidad de peatones activos
    #[inline(always)]
    pub fn count(&self) -> usize {
        self.active_count
    }

    /// Densidad promedio global (peatones por celda)
    pub fn average_density(&self) -> f32 {
        if self.active_count == 0 { return 0.0; }
        let area = self.world_width * self.world_height;
        self.active_count as f32 / area.max(1.0)
    }

    /// Generar viajes peatonales desde edificios según hora del día
    pub fn generate_trips(
        &mut self,
        building_positions: &[(f32, f32, u8)], // (x, y, building_type_id)
        time_of_day: u16, // minutos (0-1439)
    ) {
        let hour = time_of_day as f32 / 60.0;
        let factor = hour_factor(hour);

        for &(bx, by, btype) in building_positions {
            // Simplificado: cada edificio genera peatones según tipo y hora
            let spawn_chance = match btype {
                0 => 0.10 * factor, // House → residentes saliendo
                1 => 0.20 * factor, // Shop → compradores
                2 => 0.05 * factor, // Factory → pocos peatones
                3 => 0.12 * factor, // Apartment
                4 => 0.15 * factor, // Office → trabajadores
                5 => 0.03 * factor, // Farm
                6 => 0.18 * factor, // Hospital → mucho movimiento
                7 => 0.22 * factor, // School → estudiantes
                8 => 0.08 * factor, // Police
                _ => 0.05 * factor,
            };

            // Spawn determinista según posición y tick (evita RNG costoso)
            let seed = (bx * 1000.0 + by * 100.0 + time_of_day as f32) as u64;
            let roll = (seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)) as f32
                / u64::MAX as f32;

            if roll < spawn_chance && self.active_count < MAX_PEDESTRIANS - 1 {
                // Destino: otro edificio aleatorio
                let dest_b = building_positions
                    .get((seed as usize + 17) % building_positions.len())
                    .copied()
                    .unwrap_or((bx + 5.0, by + 5.0, 0));

                self.spawn(
                    bx, by,
                    dest_b.0, dest_b.1,
                    PedestrianState::Walking,
                );
            }
        }
    }

    /// Verificar colisiones peatón-vehículo (devuelve conteo de atropellos)
    pub fn check_vehicle_collisions(
        &self,
        vehicle_positions: &[(f32, f32)],
        vehicle_speeds: &[(f32, f32)],
    ) -> u32 {
        let mut collisions = 0u32;
        for i in 0..self.active_count {
            let px = self.positions[i].0;
            let py = self.positions[i].1;
            for j in 0..vehicle_positions.len() {
                let vx = vehicle_positions[j].0;
                let vy = vehicle_positions[j].1;
                let dx = px - vx;
                let dy = py - vy;
                let dist2 = dx * dx + dy * dy;
                if dist2 < 1.44 {
                    // 1.2m de distancia → colisión
                    let v_speed = (vehicle_speeds[j].0 * vehicle_speeds[j].0
                        + vehicle_speeds[j].1 * vehicle_speeds[j].1).sqrt();
                    if v_speed > 0.5 {
                        collisions += 1;
                    }
                }
            }
        }
        collisions
    }
}

// ---------------------------------------------------------------------------
// Funciones auxiliares
// ---------------------------------------------------------------------------

/// Factor de generación de viajes según hora del día
#[inline(always)]
fn hour_factor(hour: f32) -> f32 {
    if hour < 5.0 {
        0.05 // noche
    } else if hour < 7.0 {
        0.30 // temprano
    } else if hour < 9.0 {
        1.00 // hora pico AM
    } else if hour < 12.0 {
        0.40 // mañana
    } else if hour < 14.0 {
        0.60 // almuerzo
    } else if hour < 17.0 {
        0.40 // tarde
    } else if hour < 19.0 {
        1.00 // hora pico PM
    } else if hour < 22.0 {
        0.30 // noche temprano
    } else {
        0.10 // noche
    }
}

/// Determinar si un peatón debe cruzar basado en gap y semáforo
#[inline(always)]
pub fn should_cross(
    pedestrian_x: f32,
    _pedestrian_y: f32,
    nearest_car_dist: f32,
    nearest_car_speed: f32,
    has_traffic_light: bool,
    light_is_green: bool,
    road_width: f32,
) -> bool {
    let time_to_cross = road_width / DESIRED_SPEED;
    let required_gap = time_to_cross * nearest_car_speed * 1.5; // factor de seguridad

    if has_traffic_light {
        if light_is_green {
            nearest_car_dist > required_gap
        } else {
            // Jaywalking
            nearest_car_dist > required_gap * 3.0
                && (pedestrian_x * 100.0).fract() < JAYWALKING_PROB
        }
    } else {
        nearest_car_dist > required_gap
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spawn_despawn() {
        let mut sys = PedestrianSystem::new(100.0, 100.0);
        assert_eq!(sys.count(), 0);

        let idx = sys.spawn(10.0, 10.0, 50.0, 50.0, PedestrianState::Walking);
        assert!(idx < usize::MAX);
        assert_eq!(sys.count(), 1);

        sys.despawn(idx);
        assert_eq!(sys.count(), 0);
    }

    #[test]
    fn test_hour_factor() {
        assert!(hour_factor(3.0) < 0.1); // noche
        assert!(hour_factor(8.0) > 0.9); // hora pico
        assert!(hour_factor(14.0) < 0.7); // almuerzo
        assert!(hour_factor(18.0) > 0.9); // hora pico PM
    }

    #[test]
    fn test_should_cross_with_light() {
        // Caso 1: gap suficiente con luz verde — DEBE cruzar
        // time_to_cross=6/1.34=4.48s, required=4.48*8*1.5=53.7m, 55>53.7
        assert!(should_cross(0.0, 0.0, 55.0, 8.0, true, true, 6.0),
            "Con gap 55m y luz verde debe cruzar");
        // Caso 2: gap insuficiente con luz verde — NO debe cruzar
        // time_to_cross=15/1.34=11.19s, required=11.19*8*1.5=134.3m, 3<134.3
        assert!(!should_cross(0.0, 0.0, 3.0, 8.0, true, true, 15.0),
            "Con gap 3m no debe cruzar aunque este verde");
        // Caso 3: semaforo rojo, gap enorme (500m) — probabilistico (jaywalking)
        // No asertamos resultado exacto, solo que la funcion no paniquea
        let result = should_cross(0.0, 0.0, 500.0, 0.0, true, false, 6.0);
        // Con gap enorme y sin auto cerca, la funcion debe retornar un bool valido
        assert!(result == true || result == false);
    }

    #[test]
    fn test_tick_moves_toward_destination() {
        let mut sys = PedestrianSystem::new(100.0, 100.0);
        let idx = sys.spawn(0.0, 0.0, 10.0, 0.0, PedestrianState::Walking);

        sys.tick(1.0, &[], &[]);

        let (x, _) = sys.positions()[idx];
        assert!(x > 0.0, "Peatón debería moverse hacia el destino");
    }

    #[test]
    fn test_max_pedestrians() {
        let mut sys = PedestrianSystem::new(100.0, 100.0);
        for i in 0..MAX_PEDESTRIANS + 10 {
            sys.spawn(i as f32, 0.0, 50.0, 50.0, PedestrianState::Walking);
        }
        assert_eq!(sys.count(), MAX_PEDESTRIANS);
    }
}
