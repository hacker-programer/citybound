// Sistema de Tráfico con Carriles - Conceptos de A/B Street v0.9.1
//
// FASE 6: HashMap<u32, IdmParams> reemplazado por [Option<IdmParams>; MAX_VEHICLES]
// - Acceso O(1) directo sin hash lookup
// - Cero allocations en get/set de params
// - 256 slots = 256 * 24 bytes = 6KB (cabe en L1)
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad en todos los vectores
// [TC#9]  Hitboxes pre-simplificadas (carriles = rectángulos)
// [TC#26] Inlining agresivo en funciones críticas
// [TA#5]  Fixed-point para velocidades
// [TA#7]  Flow Fields para navegación base
// [TA#17] Acceso unchecked en bucles validados
// [FASE6] Array fijo O(1) sin hash

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Número máximo de carriles en el mundo
pub const MAX_LANES: usize = 1024;
/// Número máximo de intersecciones
pub const MAX_INTERSECTIONS: usize = 256;
/// Número máximo de vehículos con params IDM [FASE 6]
pub const MAX_VEHICLES: usize = 256;
/// Velocidad base en zona urbana (m/s)
pub const URBAN_SPEED_LIMIT: f32 = 8.33; // 30 km/h
/// Velocidad en avenida (m/s)
pub const AVENUE_SPEED_LIMIT: f32 = 13.89; // 50 km/h
/// Velocidad en autopista (m/s)
pub const HIGHWAY_LANE_SPEED: f32 = 20.0; // 72 km/h
/// Distancia mínima entre coches (metros)
pub const MIN_GAP: f32 = 2.0;
/// Tiempo de reacción del conductor IDM (segundos)
pub const IDM_REACTION_TIME: f32 = 1.5;
/// Aceleración máxima confortable (m/s²)
pub const IDM_MAX_ACCEL: f32 = 2.0;
/// Desaceleración confortable (m/s²)
pub const IDM_COMFORT_DECEL: f32 = 1.5;

// ---------------------------------------------------------------------------
// DIRECCIÓN DE CARRIL
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum LaneDirection {
    North, South, East, West,
    NorthEast, NorthWest, SouthEast, SouthWest,
}

impl LaneDirection {
    #[inline(always)]
    pub fn to_vector(self) -> (f32, f32) {
        match self {
            LaneDirection::North => (0.0, -1.0),
            LaneDirection::South => (0.0, 1.0),
            LaneDirection::East => (1.0, 0.0),
            LaneDirection::West => (-1.0, 0.0),
            LaneDirection::NorthEast => (0.707, -0.707),
            LaneDirection::NorthWest => (-0.707, -0.707),
            LaneDirection::SouthEast => (0.707, 0.707),
            LaneDirection::SouthWest => (-0.707, 0.707),
        }
    }

    #[inline(always)]
    pub fn to_angle(self) -> f32 {
        match self {
            LaneDirection::East => 0.0,
            LaneDirection::NorthEast => std::f32::consts::PI / 4.0,
            LaneDirection::North => std::f32::consts::PI / 2.0,
            LaneDirection::NorthWest => 3.0 * std::f32::consts::PI / 4.0,
            LaneDirection::West => std::f32::consts::PI,
            LaneDirection::SouthWest => -3.0 * std::f32::consts::PI / 4.0,
            LaneDirection::South => -std::f32::consts::PI / 2.0,
            LaneDirection::SouthEast => -std::f32::consts::PI / 4.0,
        }
    }
}

// ---------------------------------------------------------------------------
// CARRIL
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Lane {
    pub id: u32,
    pub start_x: f32, pub start_y: f32,
    pub end_x: f32, pub end_y: f32,
    pub direction: LaneDirection,
    pub speed_limit: f32,
    pub width: f32,
    pub from_intersection: Option<u32>,
    pub to_intersection: Option<u32>,
    pub left_lane: Option<u32>,
    pub right_lane: Option<u32>,
    pub congestion: f32,
    pub vehicle_count: u32,
    pub is_turn_lane: bool,
    pub turn_direction: Option<LaneDirection>,
    pub length: f32,
}

impl Lane {
    pub fn new(id: u32, start_x: f32, start_y: f32,
               end_x: f32, end_y: f32, direction: LaneDirection,
               speed_limit: f32) -> Self {
        let dx = end_x - start_x;
        let dy = end_y - start_y;
        let length = (dx * dx + dy * dy).sqrt();
        Lane {
            id, start_x, start_y, end_x, end_y,
            direction, speed_limit, width: 3.0,
            from_intersection: None, to_intersection: None,
            left_lane: None, right_lane: None,
            congestion: 0.0, vehicle_count: 0,
            is_turn_lane: false, turn_direction: None,
            length,
        }
    }

    #[inline(always)]
    pub fn project(&self, x: f32, y: f32) -> (f32, f32, f32) {
        let dx = self.end_x - self.start_x;
        let dy = self.end_y - self.start_y;
        let len_sq = dx * dx + dy * dy;
        if len_sq < 0.0001 { return (0.0, self.start_x, self.start_y); }
        let t = ((x - self.start_x) * dx + (y - self.start_y) * dy) / len_sq;
        let t_clamped = t.max(0.0).min(1.0);
        (t_clamped, self.start_x + t_clamped * dx, self.start_y + t_clamped * dy)
    }

    #[inline(always)]
    pub fn position_at(&self, t: f32) -> (f32, f32) {
        let dx = self.end_x - self.start_x;
        let dy = self.end_y - self.start_y;
        (self.start_x + t * dx, self.start_y + t * dy)
    }
}

// ---------------------------------------------------------------------------
// INTERSECCIÓN
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TrafficLightPhase { Red, Yellow, Green }

#[derive(Clone, Debug)]
pub struct Intersection {
    pub id: u32,
    pub x: f32, pub y: f32,
    pub incoming_lanes: Vec<u32>,
    pub outgoing_lanes: Vec<u32>,
    pub phase: TrafficLightPhase,
    pub phase_time_remaining: f32,
    pub green_duration: f32,
    pub yellow_duration: f32,
    pub red_duration: f32,
    pub cycle_counter: u32,
}

impl Intersection {
    pub fn new(id: u32, x: f32, y: f32) -> Self {
        Intersection {
            id, x, y,
            incoming_lanes: Vec::with_capacity(4),
            outgoing_lanes: Vec::with_capacity(4),
            phase: TrafficLightPhase::Green,
            phase_time_remaining: 30.0,
            green_duration: 30.0, yellow_duration: 3.0, red_duration: 30.0,
            cycle_counter: 0,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        self.phase_time_remaining -= dt;
        if self.phase_time_remaining <= 0.0 {
            match self.phase {
                TrafficLightPhase::Green => {
                    self.phase = TrafficLightPhase::Yellow;
                    self.phase_time_remaining = self.yellow_duration;
                }
                TrafficLightPhase::Yellow => {
                    self.phase = TrafficLightPhase::Red;
                    self.phase_time_remaining = self.red_duration;
                }
                TrafficLightPhase::Red => {
                    self.phase = TrafficLightPhase::Green;
                    self.phase_time_remaining = self.green_duration;
                    self.cycle_counter += 1;
                }
            }
        }
    }

    #[inline(always)]
    pub fn can_proceed(&self, _lane_id: u32) -> bool {
        self.phase == TrafficLightPhase::Green
    }
}

// ---------------------------------------------------------------------------
// IDM: Intelligent Driver Model
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug)]
pub struct IdmParams {
    pub desired_speed: f32,
    pub time_headway: f32,
    pub min_gap: f32,
    pub max_accel: f32,
    pub comfort_decel: f32,
    pub accel_exponent: f32,
}

impl Default for IdmParams {
    fn default() -> Self {
        IdmParams {
            desired_speed: URBAN_SPEED_LIMIT,
            time_headway: IDM_REACTION_TIME,
            min_gap: MIN_GAP,
            max_accel: IDM_MAX_ACCEL,
            comfort_decel: IDM_COMFORT_DECEL,
            accel_exponent: 4.0,
        }
    }
}

#[inline]
pub fn idm_acceleration(
    speed: f32, gap: f32, speed_diff: f32, params: &IdmParams,
) -> f32 {
    let speed_ratio = if params.desired_speed > 0.01 { speed / params.desired_speed } else { 1.0 };
    let free_accel = params.max_accel * (1.0 - speed_ratio.powf(params.accel_exponent));
    let desired_gap = params.min_gap
        + speed * params.time_headway
        + (speed * speed_diff) / (2.0 * (params.max_accel * params.comfort_decel).sqrt());
    let interaction_term = if gap > 0.001 {
        let ratio = desired_gap / gap;
        ratio * ratio
    } else {
        1e6_f32
    };
    (free_accel - params.max_accel * interaction_term).clamp(-2.0 * params.max_accel, params.max_accel)
}

// ---------------------------------------------------------------------------
// MOBIL
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LaneChangeDecision { Stay, ChangeLeft, ChangeRight }

#[derive(Copy, Clone, Debug)]
pub struct MobilParams {
    pub politeness: f32,
    pub lane_change_threshold: f32,
    pub max_safe_decel: f32,
    pub right_lane_bias: f32,
}

impl Default for MobilParams {
    fn default() -> Self {
        MobilParams { politeness: 0.5, lane_change_threshold: 0.2, max_safe_decel: 4.0, right_lane_bias: 0.2 }
    }
}

#[inline]
pub fn mobil_decision(
    new_lane_accel: f32, current_lane_accel: f32,
    _follower_new_accel: f32, _follower_current_accel: f32,
    params: &MobilParams,
) -> LaneChangeDecision {
    if new_lane_accel < -params.max_safe_decel { return LaneChangeDecision::Stay; }
    let self_benefit = new_lane_accel - current_lane_accel;
    if self_benefit > params.lane_change_threshold { LaneChangeDecision::ChangeRight }
    else if self_benefit < -params.lane_change_threshold && new_lane_accel > current_lane_accel {
        LaneChangeDecision::ChangeLeft
    } else { LaneChangeDecision::Stay }
}

// ---------------------------------------------------------------------------
// GESTOR DE CARRILES v0.10.0 — spatial_grid en heap [FIX STACK OVERFLOW]
// ---------------------------------------------------------------------------
pub struct LaneManager {
    pub lanes: Vec<Lane>,
    pub intersections: Vec<Intersection>,
    /// Grilla espacial 128x128 en heap: índice = y * 128 + x
    pub spatial_grid: Vec<Vec<u32>>,
    pub idm_params: [Option<IdmParams>; MAX_VEHICLES],
    pub mobil_params: MobilParams,
}

impl LaneManager {
    pub fn new() -> Self {
        let spatial_grid: Vec<Vec<u32>> = (0..128*128)
            .map(|_| Vec::with_capacity(4))
            .collect();

        LaneManager {
            lanes: Vec::with_capacity(MAX_LANES),
            intersections: Vec::with_capacity(MAX_INTERSECTIONS),
            spatial_grid,
            idm_params: [None; MAX_VEHICLES],
            mobil_params: MobilParams::default(),
        }
    }
    pub fn generate_default_network(&mut self) {
        let mut next_id: u32 = 0;
        let highway_y: f32 = 64.0;

        // Autopista horizontal central
        self.lanes.push(Lane::new(next_id, 0.0, highway_y - 3.0, 128.0, highway_y - 3.0, LaneDirection::East, HIGHWAY_LANE_SPEED));
        next_id += 1;
        self.lanes.push(Lane::new(next_id, 0.0, highway_y - 0.5, 128.0, highway_y - 0.5, LaneDirection::East, HIGHWAY_LANE_SPEED));
        next_id += 1;
        self.lanes.push(Lane::new(next_id, 128.0, highway_y + 0.5, 0.0, highway_y + 0.5, LaneDirection::West, HIGHWAY_LANE_SPEED));
        next_id += 1;
        self.lanes.push(Lane::new(next_id, 128.0, highway_y + 3.0, 0.0, highway_y + 3.0, LaneDirection::West, HIGHWAY_LANE_SPEED));
        next_id += 1;

        self.lanes[0].right_lane = Some(1); self.lanes[1].left_lane = Some(0);
        self.lanes[2].left_lane = Some(3); self.lanes[3].right_lane = Some(2);

        // Avenidas verticales
        for i in 0..6 {
            let ave_x = 20.0 + i as f32 * 20.0;
            let id_n = next_id;
            self.lanes.push(Lane::new(id_n, ave_x - 1.0, 100.0, ave_x - 1.0, 20.0, LaneDirection::North, AVENUE_SPEED_LIMIT));
            next_id += 1;
            let id_s = next_id;
            self.lanes.push(Lane::new(id_s, ave_x + 1.0, 20.0, ave_x + 1.0, 100.0, LaneDirection::South, AVENUE_SPEED_LIMIT));
            next_id += 1;
            self.lanes[id_n as usize].right_lane = Some(id_s);
            self.lanes[id_s as usize].left_lane = Some(id_n);

            let intersection = Intersection::new(next_id, ave_x, highway_y);
            self.intersections.push(intersection);
            let intersection_id = next_id;
            next_id += 1;
            self.lanes[id_n as usize].to_intersection = Some(intersection_id);
            self.lanes[id_s as usize].to_intersection = Some(intersection_id);
        }

        // Calles residenciales horizontales
        for row in 0..8 {
            let street_y = 10.0 + row as f32 * 15.0;
            if (street_y - highway_y).abs() < 10.0 { continue; }
            let id_e = next_id;
            self.lanes.push(Lane::new(id_e, 0.0, street_y, 128.0, street_y, LaneDirection::East, URBAN_SPEED_LIMIT));
            next_id += 1;
            let id_w = next_id;
            self.lanes.push(Lane::new(id_w, 128.0, street_y + 2.0, 0.0, street_y + 2.0, LaneDirection::West, URBAN_SPEED_LIMIT));
            self.lanes.push(Lane::new(id_w, 128.0, street_y + 2.0, 0.0, street_y + 2.0, LaneDirection::West, URBAN_SPEED_LIMIT));
            next_id += 1;
            self.lanes[id_e as usize].right_lane = Some(id_w);
            self.lanes[id_w as usize].left_lane = Some(id_e);
        }

        self.build_spatial_grid();
        println!("Red de carriles: {} carriles, {} intersecciones", self.lanes.len(), self.intersections.len());
    }

    fn build_spatial_grid(&mut self) {
                let t = i as f32 / steps as f32;
                let (x, y) = lane.position_at(t);
                let gx = x as usize % 128;
                let gy = y as usize % 128;
                if gx < 128 && gy < 128 {
                    let cell = &mut self.spatial_grid[gy * 128 + gx];
                    if !cell.contains(&lane.id) { cell.push(lane.id); }
                }
            }
        }
    }

    #[inline]
    pub fn lanes_near(&self, x: f32, y: f32, radius: f32) -> Vec<u32> {
        let gx = x as usize % 128;
        let gy = y as usize % 128;
        let r = radius.ceil() as usize;
        let mut result = Vec::with_capacity(8);
        let min_x = if gx >= r { gx - r } else { 0 };
        let max_x = (gx + r + 1).min(127);
        let min_y = if gy >= r { gy - r } else { 0 };
        let max_y = (gy + r + 1).min(127);
        for py in min_y..=max_y {
            for px in min_x..=max_x {
                for &lane_id in &self.spatial_grid[py * 128 + px] {
                    if !result.contains(&lane_id) { result.push(lane_id); }
                }
            }
        }
        result
    }

    pub fn closest_lane(&self, x: f32, y: f32) -> Option<&Lane> {
    pub fn closest_lane(&self, x: f32, y: f32) -> Option<&Lane> {
        let nearby = self.lanes_near(x, y, 5.0);
        let mut best: Option<&Lane> = None;
        let mut best_dist: f32 = f32::MAX;
        for id in nearby {
            let lane = &self.lanes[id as usize];
            let (_t, px, py) = lane.project(x, y);
            let dist = ((x - px) * (x - px) + (y - py) * (y - py)).sqrt();
            if dist < best_dist { best_dist = dist; best = Some(lane); }
        }
        best
    }

    pub fn update_congestion(&mut self) {
        for lane in self.lanes.iter_mut() { lane.vehicle_count = 0; }
        for lane in self.lanes.iter_mut() {
            let density = lane.vehicle_count as f32 / (lane.length / MIN_GAP).max(1.0);
            lane.congestion = density.min(1.0);
        }
    }

    #[inline(always)]
    pub fn set_vehicle_params(&mut self, vehicle_id: u32, params: IdmParams) {
        let idx = (vehicle_id as usize) % MAX_VEHICLES;
        unsafe { *self.idm_params.get_unchecked_mut(idx) = Some(params); }
    }

    #[inline(always)]
    pub fn get_idm_params(&self, vehicle_id: u32) -> IdmParams {
        let idx = (vehicle_id as usize) % MAX_VEHICLES;
        unsafe {
            self.idm_params.get_unchecked(idx).unwrap_or_default()
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
    fn test_lane_creation() {
        let lane = Lane::new(0, 0.0, 0.0, 10.0, 0.0, LaneDirection::East, 10.0);
        assert_eq!(lane.id, 0);
        assert_eq!(lane.speed_limit, 10.0);
        assert!((lane.length - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_lane_project() {
        let lane = Lane::new(0, 0.0, 0.0, 10.0, 0.0, LaneDirection::East, 10.0);
        let (t, px, py) = lane.project(5.0, 0.0);
        assert!((t - 0.5).abs() < 0.01);
        assert!((px - 5.0).abs() < 0.01);
        assert!(py.abs() < 0.01);
    }

    #[test]
    fn test_lane_position_at() {
        let lane = Lane::new(0, 0.0, 0.0, 20.0, 10.0, LaneDirection::East, 10.0);
        let (x, y) = lane.position_at(0.5);
        assert!((x - 10.0).abs() < 0.01 && (y - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_lane_direction_to_vector() {
        let (dx, dy) = LaneDirection::East.to_vector();
        assert!((dx - 1.0).abs() < 0.01 && dy.abs() < 0.01);
        let (dx, dy) = LaneDirection::North.to_vector();
        assert!(dx.abs() < 0.01 && (dy + 1.0).abs() < 0.01);
    }

    #[test]
    fn test_idm_free_flow() {
        let params = IdmParams::default();
        let accel = idm_acceleration(0.0, 100.0, 0.0, &params);
        assert!(accel > 0.0, "Debe acelerar en vía libre: {}", accel);
    }

    #[test]
    fn test_idm_approach_leader() {
        let params = IdmParams::default();
        let accel = idm_acceleration(10.0, 5.0, 3.0, &params);
        assert!(accel < 0.0, "Debe frenar al acercarse: {}", accel);
    }

    #[test]
    fn test_idm_stopped_traffic() {
        let params = IdmParams::default();
        let accel = idm_acceleration(3.0, 1.0, 3.0, &params);
        assert!(accel < -1.0, "Debe frenar fuerte: {}", accel);
    }

    #[test]
    fn test_intersection_phase_cycle() {
        let mut intersection = Intersection::new(0, 10.0, 10.0);
        assert_eq!(intersection.phase, TrafficLightPhase::Green);
        while intersection.phase == TrafficLightPhase::Green { intersection.tick(1.0); }
        assert_eq!(intersection.phase, TrafficLightPhase::Yellow);
        while intersection.phase == TrafficLightPhase::Yellow { intersection.tick(1.0); }
        assert_eq!(intersection.phase, TrafficLightPhase::Red);
        while intersection.phase == TrafficLightPhase::Red { intersection.tick(1.0); }
        assert_eq!(intersection.phase, TrafficLightPhase::Green);
        assert_eq!(intersection.cycle_counter, 1);
    }

    #[test]
    fn test_lane_manager_generation() {
        let mut manager = LaneManager::new();
        manager.generate_default_network();
        assert!(!manager.lanes.is_empty());
        assert!(!manager.intersections.is_empty());
        assert!(manager.lanes.len() <= MAX_LANES);
    }

    #[test]
    fn test_lanes_near() {
        let mut manager = LaneManager::new();
        manager.generate_default_network();
        let nearby = manager.lanes_near(64.0, 64.0, 5.0);
        assert!(!nearby.is_empty());
    }

    #[test]
    fn test_closest_lane() {
        let mut manager = LaneManager::new();
        manager.generate_default_network();
        let lane = manager.closest_lane(64.0, 61.0);
        assert!(lane.is_some());
    }

    #[test]
    fn test_mobil_default() {
        let params = MobilParams::default();
        let decision = mobil_decision(2.0, 1.0, 0.0, 0.0, &params);
        assert!(decision != LaneChangeDecision::Stay);
    }

    #[test]
    fn test_idm_params_array() {
        let mut manager = LaneManager::new();
        let params = IdmParams { desired_speed: 15.0, ..IdmParams::default() };
        manager.set_vehicle_params(42, params);
        let retrieved = manager.get_idm_params(42);
        assert_eq!(retrieved.desired_speed, 15.0);

        let default_params = manager.get_idm_params(999);
        assert_eq!(default_params.desired_speed, URBAN_SPEED_LIMIT);
    }
}
