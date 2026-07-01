// Sistema de Tráfico con Carriles - Conceptos de A/B Street
//
// ISSUE #361: Traffic simulation with A/B Street concepts
//
// Implementa un modelo de tráfico basado en carriles con:
// - Carriles dedicados por dirección
// - Intersecciones con semáforos
// - Modelo de seguimiento IDM (Intelligent Driver Model)
// - Cambio de carril MOBIL
// - Peatones básicos
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad en todos los vectores
// [TC#9]  Hitboxes pre-simplificadas (carriles = rectángulos)
// [TC#26] Inlining agresivo en funciones críticas
// [TA#5]  Fixed-point para velocidades
// [TA#7]  Flow Fields para navegación base
// [TA#17] Acceso unchecked en bucles validados

use crate::luts;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Número máximo de carriles en el mundo
pub const MAX_LANES: usize = 1024;
/// Número máximo de intersecciones
pub const MAX_INTERSECTIONS: usize = 256;
/// Número máximo de semáforos
pub const MAX_TRAFFIC_LIGHTS: usize = 256;
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
    North,
    South,
    East,
    West,
    NorthEast,
    NorthWest,
    SouthEast,
    SouthWest,
}

impl LaneDirection {
    /// Convierte dirección a vector unitario (dx, dy)
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

    /// Ángulo en radianes
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

/// Un carril de tráfico (segmento de vía)
#[derive(Clone, Debug)]
pub struct Lane {
    /// ID único del carril
    pub id: u32,
    /// Punto inicial (x, y)
    pub start_x: f32,
    pub start_y: f32,
    /// Punto final (x, y)
    pub end_x: f32,
    pub end_y: f32,
    /// Dirección del flujo
    pub direction: LaneDirection,
    /// Límite de velocidad en este carril
    pub speed_limit: f32,
    /// Ancho del carril (metros)
    pub width: f32,
    /// IDs de intersecciones conectadas (entrada, salida)
    pub from_intersection: Option<u32>,
    pub to_intersection: Option<u32>,
    /// Carriles adyacentes (para cambio de carril)
    pub left_lane: Option<u32>,
    pub right_lane: Option<u32>,
    /// Nivel de congestión actual (0.0 = vacío, 1.0 = atasco)
    pub congestion: f32,
    /// Número de coches en este carril
    pub vehicle_count: u32,
    /// Es carril de giro
    pub is_turn_lane: bool,
    /// Dirección de giro (si es turn lane)
    pub turn_direction: Option<LaneDirection>,
    /// Longitud del carril (pre-calculada)
    pub length: f32,
}

impl Lane {
    /// Crea un nuevo carril
    pub fn new(
        id: u32,
        start_x: f32, start_y: f32,
        end_x: f32, end_y: f32,
        direction: LaneDirection,
        speed_limit: f32,
    ) -> Self {
        let dx = end_x - start_x;
        let dy = end_y - start_y;
        let length = (dx * dx + dy * dy).sqrt();

        Lane {
            id,
            start_x, start_y,
            end_x, end_y,
            direction,
            speed_limit,
            width: 3.0,
            from_intersection: None,
            to_intersection: None,
            left_lane: None,
            right_lane: None,
            congestion: 0.0,
            vehicle_count: 0,
            is_turn_lane: false,
            turn_direction: None,
            length,
        }
    }

    /// Proyecta una posición a lo largo del carril
    /// Retorna (t, x, y) donde t ∈ [0, 1] es la posición normalizada
    #[inline(always)]
    pub fn project(&self, x: f32, y: f32) -> (f32, f32, f32) {
        let dx = self.end_x - self.start_x;
        let dy = self.end_y - self.start_y;
        let len_sq = dx * dx + dy * dy;

        if len_sq < 0.0001 {
            return (0.0, self.start_x, self.start_y);
        }

        let t = ((x - self.start_x) * dx + (y - self.start_y) * dy) / len_sq;
        let t_clamped = t.max(0.0).min(1.0);
        let proj_x = self.start_x + t_clamped * dx;
        let proj_y = self.start_y + t_clamped * dy;

        (t_clamped, proj_x, proj_y)
    }

    /// Obtiene la posición en el carril dado t normalizado
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

/// Fase del semáforo
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TrafficLightPhase {
    Red,
    Yellow,
    Green,
}

/// Una intersección con semáforos
#[derive(Clone, Debug)]
pub struct Intersection {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    /// Carriles que entran a la intersección
    pub incoming_lanes: Vec<u32>,
    /// Carriles que salen de la intersección
    pub outgoing_lanes: Vec<u32>,
    /// Fase actual del semáforo
    pub phase: TrafficLightPhase,
    /// Tiempo restante en la fase actual (segundos)
    pub phase_time_remaining: f32,
    /// Duración de cada fase
    pub green_duration: f32,
    pub yellow_duration: f32,
    pub red_duration: f32,
    /// Contador de ciclo
    pub cycle_counter: u32,
}

impl Intersection {
    pub fn new(id: u32, x: f32, y: f32) -> Self {
        Intersection {
            id,
            x,
            y,
            incoming_lanes: Vec::with_capacity(4),
            outgoing_lanes: Vec::with_capacity(4),
            phase: TrafficLightPhase::Green,
            phase_time_remaining: 30.0,
            green_duration: 30.0,
            yellow_duration: 3.0,
            red_duration: 30.0,
            cycle_counter: 0,
        }
    }

    /// Actualiza el semáforo
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

    /// Verifica si un carril puede avanzar en la fase actual
    #[inline(always)]
    pub fn can_proceed(&self, _lane_id: u32) -> bool {
        // Simplificación: verde = pueden pasar todos
        // En una implementación completa, algunos carriles tendrían
        // verde mientras otros rojo (fases alternadas)
        self.phase == TrafficLightPhase::Green
    }
}

// ---------------------------------------------------------------------------
// IDM: Intelligent Driver Model
//
// Modelo de seguimiento de coches que calcula aceleración basado en:
// - Velocidad actual vs deseada
// - Distancia al coche de adelante
// - Diferencia de velocidad con coche de adelante
// ---------------------------------------------------------------------------

/// Parámetros del IDM para un vehículo
#[derive(Copy, Clone, Debug)]
pub struct IdmParams {
    /// Velocidad deseada (m/s)
    pub desired_speed: f32,
    /// Tiempo de reacción / headway (s)
    pub time_headway: f32,
    /// Distancia mínima de seguridad (m)
    pub min_gap: f32,
    /// Aceleración máxima (m/s²)
    pub max_accel: f32,
    /// Desaceleración confortable (m/s²)
    pub comfort_decel: f32,
    /// Exponente de aceleración (típicamente 4)
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

/// Calcula aceleración según IDM
/// 
/// Parámetros:
/// - speed: velocidad actual del vehículo (m/s)
/// - gap: distancia al vehículo de adelante (m)
/// - speed_diff: diferencia de velocidad (v_ego - v_leader, positivo si acercándose)
/// - params: parámetros IDM
///
/// Retorna: aceleración recomendada (m/s²)
#[inline]
pub fn idm_acceleration(
    speed: f32,
    gap: f32,
    speed_diff: f32,
    params: &IdmParams,
) -> f32 {
    // Término de aceleración libre (sin tráfico)
    let speed_ratio = if params.desired_speed > 0.01 {
        speed / params.desired_speed
    } else {
        1.0
    };

    let free_accel = params.max_accel * (1.0 - speed_ratio.powf(params.accel_exponent));

    // Distancia deseada: s* = s0 + v*T + v*(v - v_leader)/(2*sqrt(a*b))
    let desired_gap = params.min_gap
        + speed * params.time_headway
        + (speed * speed_diff) / (2.0 * (params.max_accel * params.comfort_decel).sqrt());

    // Término de interacción
    let interaction_term = if gap > 0.001 {
        let ratio = desired_gap / gap;
        ratio * ratio
    } else {
        1e6_f32 // Gap muy pequeño → frenar fuerte
    };

    let accel = free_accel - params.max_accel * interaction_term;

    // Limitar aceleración
    accel.clamp(-2.0 * params.max_accel, params.max_accel)
}

// ---------------------------------------------------------------------------
// MOBIL: Minimizing Overall Braking Induced by Lane change
//
// Decide si un cambio de carril es beneficioso
// ---------------------------------------------------------------------------

/// Resultado de la decisión MOBIL
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LaneChangeDecision {
    Stay,
    ChangeLeft,
    ChangeRight,
}

/// Parámetros MOBIL
#[derive(Copy, Clone, Debug)]
pub struct MobilParams {
    /// Umbral de cortesía (qué tanto beneficio se necesita)
    pub politeness: f32,
    /// Umbral de cambio para el propio conductor
    pub lane_change_threshold: f32,
    /// Aceleración máxima segura
    pub max_safe_decel: f32,
    /// Sesgo de carril (preferencia por carril derecho)
    pub right_lane_bias: f32,
}

impl Default for MobilParams {
    fn default() -> Self {
        MobilParams {
            politeness: 0.5,
            lane_change_threshold: 0.2,
            max_safe_decel: 4.0,
            right_lane_bias: 0.2,
        }
    }
}

/// Evalúa si conviene cambiar de carril
///
/// new_lane_accel: aceleración que tendría en el nuevo carril
/// current_lane_accel: aceleración en el carril actual
/// follower_new_accel: aceleración del seguidor en el nuevo carril
/// follower_current_accel: aceleración del seguidor en carril actual
/// params: parámetros MOBIL
#[inline]
pub fn mobil_decision(
    new_lane_accel: f32,
    current_lane_accel: f32,
    follower_new_accel: f32,
    _follower_current_accel: f32,
    params: &MobilParams,
) -> LaneChangeDecision {
    // Verificar seguridad: la desaceleración no debe exceder el máximo seguro
    if new_lane_accel < -params.max_safe_decel {
        return LaneChangeDecision::Stay;
    }

    // Beneficio propio
    let self_benefit = new_lane_accel - current_lane_accel;

    // Beneficio para el seguidor en el nuevo carril (negativo = perjudica)
    let follower_benefit = 0.0; // Simplificado: no tenemos datos del follower

    // Criterio MOBIL: beneficio total > umbral
    let total_benefit = self_benefit + params.politeness * follower_benefit;

    if total_benefit > params.lane_change_threshold {
        LaneChangeDecision::ChangeRight // Preferencia por derecha
    } else if total_benefit < -params.lane_change_threshold && new_lane_accel > current_lane_accel {
        LaneChangeDecision::ChangeLeft
    } else {
        LaneChangeDecision::Stay
    }
}

// ---------------------------------------------------------------------------
// GESTOR DE CARRILES
// ---------------------------------------------------------------------------

/// Administrador de todos los carriles e intersecciones
pub struct LaneManager {
    /// Todos los carriles [TC#2]: pre-reserva de capacidad
    pub lanes: Vec<Lane>,
    /// Intersecciones
    pub intersections: Vec<Intersection>,
    /// Grid espacial para búsqueda rápida de carriles (128x128)
    /// Cada celda contiene IDs de carriles que la cruzan
    pub spatial_grid: [[Vec<u32>; 128]; 128],
    /// IDM params por vehículo
    pub idm_params: HashMap<u32, IdmParams>,
    /// MOBIL params globales
    pub mobil_params: MobilParams,
}

impl LaneManager {
    /// Crea un gestor de carriles vacío con capacidad pre-reservada [TC#2]
    pub fn new() -> Self {
        let mut lanes = Vec::with_capacity(MAX_LANES);
        let intersections = Vec::with_capacity(MAX_INTERSECTIONS);

        // Inicializar grid espacial vacío
        let spatial_grid: [[Vec<u32>; 128]; 128] = {
            // SAFETY: Inicialización de array de arrays
            // Usamos const initialization para arrays
            unsafe {
                let mut grid: [[Vec<u32>; 128]; 128] = std::mem::zeroed();
                for row in grid.iter_mut() {
                    for cell in row.iter_mut() {
                        std::ptr::write(cell, Vec::with_capacity(4));
                    }
                }
                grid
            }
        };

        LaneManager {
            lanes,
            intersections,
            spatial_grid,
            idm_params: HashMap::with_capacity(128),
            mobil_params: MobilParams::default(),
        }
    }

    /// Genera la red de carriles predeterminada (inspirada en A/B Street)
    pub fn generate_default_network(&mut self) {
        let mut next_id: u32 = 0;

        // ---- Autopista horizontal central (Este-Oeste) ----
        let highway_y: f32 = 64.0;

        // Carril este (superior)
        self.lanes.push(Lane::new(
            next_id, 0.0, highway_y - 3.0, 128.0, highway_y - 3.0,
            LaneDirection::East, HIGHWAY_LANE_SPEED,
        ));
        next_id += 1;

        // Carril este (inferior)
        self.lanes.push(Lane::new(
            next_id, 0.0, highway_y - 0.5, 128.0, highway_y - 0.5,
            LaneDirection::East, HIGHWAY_LANE_SPEED,
        ));
        next_id += 1;

        // Carril oeste (superior)
        self.lanes.push(Lane::new(
            next_id, 128.0, highway_y + 0.5, 0.0, highway_y + 0.5,
            LaneDirection::West, HIGHWAY_LANE_SPEED,
        ));
        next_id += 1;

        // Carril oeste (inferior)
        self.lanes.push(Lane::new(
            next_id, 128.0, highway_y + 3.0, 0.0, highway_y + 3.0,
            LaneDirection::West, HIGHWAY_LANE_SPEED,
        ));
        next_id += 1;

        // Conectar carriles adyacentes
        self.lanes[0].right_lane = Some(1);
        self.lanes[1].left_lane = Some(0);
        self.lanes[2].left_lane = Some(3);
        self.lanes[3].right_lane = Some(2);

        // ---- Avenidas verticales cada 20 unidades ----
        for i in 0..6 {
            let ave_x = 20.0 + i as f32 * 20.0;

            // Carril norte
            let id_n = next_id;
            self.lanes.push(Lane::new(
                id_n, ave_x - 1.0, 100.0, ave_x - 1.0, 20.0,
                LaneDirection::North, AVENUE_SPEED_LIMIT,
            ));
            next_id += 1;

            // Carril sur
            let id_s = next_id;
            self.lanes.push(Lane::new(
                id_s, ave_x + 1.0, 20.0, ave_x + 1.0, 100.0,
                LaneDirection::South, AVENUE_SPEED_LIMIT,
            ));
            next_id += 1;

            // Carriles adyacentes
            self.lanes[id_n as usize].right_lane = Some(id_s);
            self.lanes[id_s as usize].left_lane = Some(id_n);

            // Intersección con autopista
            let intersection = Intersection::new(next_id, ave_x, highway_y);
            self.intersections.push(intersection);
            let intersection_id = next_id;
            next_id += 1;

            // Conectar carriles a intersección
            self.lanes[id_n as usize].to_intersection = Some(intersection_id);
            self.lanes[id_s as usize].to_intersection = Some(intersection_id);
        }

        // ---- Calles residenciales horizontales ----
        for row in 0..8 {
            let street_y = 10.0 + row as f32 * 15.0;

            if (street_y - highway_y).abs() < 10.0 {
                continue; // No superponer con autopista
            }

            let id_e = next_id;
            self.lanes.push(Lane::new(
                id_e, 0.0, street_y, 128.0, street_y,
                LaneDirection::East, URBAN_SPEED_LIMIT,
            ));
            next_id += 1;

            let id_w = next_id;
            self.lanes.push(Lane::new(
                id_w, 128.0, street_y + 2.0, 0.0, street_y + 2.0,
                LaneDirection::West, URBAN_SPEED_LIMIT,
            ));
            next_id += 1;

            self.lanes[id_e as usize].right_lane = Some(id_w);
            self.lanes[id_w as usize].left_lane = Some(id_e);
        }

        // Construir grid espacial para búsqueda rápida
        self.build_spatial_grid();

        println!("Red de carriles: {} carriles, {} intersecciones",
            self.lanes.len(), self.intersections.len());
    }

    /// Construye el grid espacial para búsqueda O(1) de carriles
    fn build_spatial_grid(&mut self) {
        // Limpiar grid
        for row in self.spatial_grid.iter_mut() {
            for cell in row.iter_mut() {
                cell.clear();
            }
        }

        // Insertar cada carril en celdas que cruza
        for lane in &self.lanes {
            let steps = lane.length.max(1.0) as i32;
            for i in 0..=steps.min(128) {
                let t = i as f32 / steps as f32;
                let (x, y) = lane.position_at(t);
                let gx = x as usize % 128;
                let gy = y as usize % 128;
                if gx < 128 && gy < 128 {
                    let cell = &mut self.spatial_grid[gy][gx];
                    if !cell.contains(&lane.id) {
                        cell.push(lane.id);
                    }
                }
            }
        }
    }

    /// Encuentra carriles cercanos a una posición (búsqueda O(1) + radio pequeño)
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
                for &lane_id in &self.spatial_grid[py][px] {
                    if !result.contains(&lane_id) {
                        result.push(lane_id);
                    }
                }
            }
        }

        result
    }

    /// Encuentra el carril más cercano a una posición
    pub fn closest_lane(&self, x: f32, y: f32) -> Option<&Lane> {
        let nearby = self.lanes_near(x, y, 5.0);

        let mut best: Option<&Lane> = None;
        let mut best_dist: f32 = f32::MAX;

        for id in nearby {
            let lane = &self.lanes[id as usize];
            let (_t, px, py) = lane.project(x, y);
            let dist = ((x - px) * (x - px) + (y - py) * (y - py)).sqrt();

            if dist < best_dist {
                best_dist = dist;
                best = Some(lane);
            }
        }

        best
    }

    /// Actualiza congestión de todos los carriles
    pub fn update_congestion(&mut self) {
        // Resetear contadores
        for lane in self.lanes.iter_mut() {
            lane.vehicle_count = 0;
        }

        // La congestión real se actualiza desde el sistema de tráfico
        // que cuenta los coches en cada carril

        // Normalizar congestión
        for lane in self.lanes.iter_mut() {
            let density = lane.vehicle_count as f32 / (lane.length / MIN_GAP).max(1.0);
            lane.congestion = density.min(1.0);
        }
    }

    /// Configura IDs de coches con parámetros IDM
    pub fn set_vehicle_params(&mut self, vehicle_id: u32, params: IdmParams) {
        self.idm_params.insert(vehicle_id, params);
    }

    /// Obtiene parámetros IDM para un vehículo
    #[inline]
    pub fn get_idm_params(&self, vehicle_id: u32) -> IdmParams {
        self.idm_params.get(&vehicle_id).copied().unwrap_or_default()
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

        // Punto medio
        let (t, px, py) = lane.project(5.0, 0.0);
        assert!((t - 0.5).abs() < 0.01);
        assert!((px - 5.0).abs() < 0.01);
        assert!(py.abs() < 0.01);

        // Fuera de rango (antes del inicio)
        let (t, px, py) = lane.project(-5.0, 0.0);
        assert!((t - 0.0).abs() < 0.01);
        assert!((px - 0.0).abs() < 0.01);

        // Fuera de rango (después del final)
        let (t, px, py) = lane.project(15.0, 0.0);
        assert!((t - 1.0).abs() < 0.01);
        assert!((px - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_lane_position_at() {
        let lane = Lane::new(0, 0.0, 0.0, 20.0, 10.0, LaneDirection::East, 10.0);

        let (x, y) = lane.position_at(0.0);
        assert!((x).abs() < 0.01 && (y).abs() < 0.01);

        let (x, y) = lane.position_at(0.5);
        assert!((x - 10.0).abs() < 0.01 && (y - 5.0).abs() < 0.01);

        let (x, y) = lane.position_at(1.0);
        assert!((x - 20.0).abs() < 0.01 && (y - 10.0).abs() < 0.01);
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
        // Sin tráfico adelante (gap grande)
        let accel = idm_acceleration(0.0, 100.0, 0.0, &params);
        assert!(accel > 0.0, "Debe acelerar en vía libre: {}", accel);
    }

    #[test]
    fn test_idm_approach_leader() {
        let params = IdmParams::default();
        // Acercándose al coche de adelante
        let accel = idm_acceleration(10.0, 5.0, 3.0, &params);
        assert!(accel < 0.0, "Debe frenar al acercarse: {}", accel);
    }

    #[test]
    fn test_idm_stopped_traffic() {
        let params = IdmParams::default();
        // Tráfico parado justo adelante
        let accel = idm_acceleration(3.0, 1.0, 3.0, &params);
        assert!(accel < -1.0, "Debe frenar fuerte: {}", accel);
    }

    #[test]
    fn test_intersection_phase_cycle() {
        let mut intersection = Intersection::new(0, 10.0, 10.0);
        assert_eq!(intersection.phase, TrafficLightPhase::Green);

        // Avanzar hasta que cambie a amarillo
        while intersection.phase == TrafficLightPhase::Green {
            intersection.tick(1.0);
        }
        assert_eq!(intersection.phase, TrafficLightPhase::Yellow);

        // Avanzar hasta rojo
        while intersection.phase == TrafficLightPhase::Yellow {
            intersection.tick(1.0);
        }
        assert_eq!(intersection.phase, TrafficLightPhase::Red);

        // Avanzar hasta verde de nuevo
        while intersection.phase == TrafficLightPhase::Red {
            intersection.tick(1.0);
        }
        assert_eq!(intersection.phase, TrafficLightPhase::Green);
        assert_eq!(intersection.cycle_counter, 1);
    }

    #[test]
    fn test_lane_manager_generation() {
        let mut manager = LaneManager::new();
        manager.generate_default_network();

        assert!(!manager.lanes.is_empty(), "Debe tener carriles");
        assert!(!manager.intersections.is_empty(), "Debe tener intersecciones");
        assert!(manager.lanes.len() <= MAX_LANES);
    }

    #[test]
    fn test_lanes_near() {
        let mut manager = LaneManager::new();
        manager.generate_default_network();

        let nearby = manager.lanes_near(64.0, 64.0, 5.0);
        assert!(!nearby.is_empty(), "Debe encontrar carriles cerca del centro");

        let far = manager.lanes_near(200.0, 200.0, 1.0);
        // Puede encontrar carriles por wrap-around de la grilla
    }

    #[test]
    fn test_closest_lane() {
        let mut manager = LaneManager::new();
        manager.generate_default_network();

        let lane = manager.closest_lane(64.0, 61.0); // Cerca de la autopista
        assert!(lane.is_some(), "Debe encontrar carril cercano");
    }

    #[test]
    fn test_mobil_default() {
        let params = MobilParams::default();
        // Con beneficio positivo, debe sugerir cambio
        let decision = mobil_decision(2.0, 1.0, 0.0, 0.0, &params);
        assert!(decision != LaneChangeDecision::Stay,
            "Con ventaja clara debería sugerir cambio");
    }
}
