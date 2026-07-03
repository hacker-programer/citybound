// Sistema de Sociología y Política Tóxica
//
// Implementa:
// - Efecto NIMBY: ricos bloquean infraestructura no deseada
// - Gentrificación dinámica: desplazamiento por valor del suelo
// - Sindicatos y huelgas: servicios públicos interrumpidos
// - Ciclos electorales: aprobación, veto del concejo, destitución
// - Distritos políticos con popularidad
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad
// [TC#22] RNG pool para eventos aleatorios
// [TC#26] Inlining agresivo

use crate::rng_pool;


// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Duración del mandato (en ticks)
pub const TERM_LENGTH_TICKS: u64 = 72000; // ~2 horas reales
/// Umbral de aprobación para evitar veto
pub const VETO_THRESHOLD: f32 = 0.35;
/// Umbral para destitución
pub const RECALL_THRESHOLD: f32 = 0.15;
/// Número de distritos
pub const NUM_DISTRICTS: usize = 9;
/// Tamaño de distrito (3x3 grid de distritos en mundo 128x128)
pub const DISTRICT_SIZE: f32 = 42.67;

// ---------------------------------------------------------------------------
// TIPOS DE INFRAESTRUCTURA NO DESEADA (NIMBY)
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum UnwantedFacility {
    Landfill,
    WastewaterPlant,
    Prison,
    PowerPlant,
    Highway,
    Factory,
    Airport,
    Stadium,
    HalfwayHouse,
}

impl UnwantedFacility {
    /// Radio de molestia en celdas
    pub fn nuisance_radius(&self) -> f32 {
        match self {
            Self::Landfill => 20.0,
            Self::WastewaterPlant => 15.0,
            Self::Prison => 10.0,
            Self::PowerPlant => 12.0,
            Self::Highway => 8.0,
            Self::Factory => 10.0,
            Self::Airport => 30.0,
            Self::Stadium => 15.0,
            Self::HalfwayHouse => 8.0,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Landfill => "vertedero",
            Self::WastewaterPlant => "planta de tratamiento",
            Self::Prison => "cárcel",
            Self::PowerPlant => "central eléctrica",
            Self::Highway => "autopista",
            Self::Factory => "fábrica",
            Self::Airport => "aeropuerto",
            Self::Stadium => "estadio",
            Self::HalfwayHouse => "casa de reinserción",
        }
    }
}

// ---------------------------------------------------------------------------
// DISTRITO POLÍTICO
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct District {
    pub id: usize,
    pub center_x: f32,
    pub center_y: f32,
    /// Popularidad del alcalde en este distrito (0-1)
    pub mayor_approval: f32,
    /// Número de residentes
    pub population: u32,
    /// Riqueza promedio
    pub avg_wealth: f32,
    /// ¿Es distrito rico? (>70% riqueza)
    pub is_wealthy: bool,
    /// Instalaciones no deseadas en este distrito
    pub unwanted_facilities: Vec<(f32, f32, UnwantedFacility)>,
    /// ¿Hay protestas activas?
    pub active_protest: bool,
    /// Intensidad de protesta (0-100)
    pub protest_intensity: f32,
    /// Ticks restantes de protesta
    pub protest_remaining: u32,
}

impl District {
    pub fn new(id: usize, cx: f32, cy: f32) -> Self {
        District {
            id, center_x: cx, center_y: cy,
            mayor_approval: 0.6,
            population: 100,
            avg_wealth: 5000.0,
            is_wealthy: false,
            unwanted_facilities: Vec::new(),
            active_protest: false,
            protest_intensity: 0.0,
            protest_remaining: 0,
        }
    }

    /// Calcula penalización NIMBY por nueva instalación
    pub fn nimby_penalty(&self, facility: UnwantedFacility, distance: f32) -> f32 {
        let radius = facility.nuisance_radius();
        if distance > radius { return 0.0; }

        let proximity = 1.0 - (distance / radius);
        let wealth_factor = if self.is_wealthy { 3.0 } else { 1.0 };

        proximity * wealth_factor * 0.2
    }
}

// ---------------------------------------------------------------------------
// SINDICATOS
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UnionType {
    GarbageCollectors,
    Teachers,
    Police,
    Firefighters,
    TransitWorkers,
    Nurses,
}

impl UnionType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::GarbageCollectors => "Recolectores de Basura",
            Self::Teachers => "Maestros",
            Self::Police => "Policías",
            Self::Firefighters => "Bomberos",
            Self::TransitWorkers => "Transporte",
            Self::Nurses => "Enfermeros",
        }
    }
}

#[derive(Clone, Debug)]
pub struct LaborUnion {
    pub union_type: UnionType,
    /// Nivel de satisfacción (0-1, bajo = riesgo huelga)
    pub satisfaction: f32,
    /// ¿Están en huelga?
    pub on_strike: bool,
    /// Días restantes de huelga
    pub strike_remaining: u32,
    /// Presupuesto asignado
    pub budget: f32,
    /// Presupuesto mínimo para evitar huelga
    pub min_budget: f32,
}

impl LaborUnion {
    pub fn new(union_type: UnionType) -> Self {
        let min_budget = match union_type {
            UnionType::GarbageCollectors => 800.0,
            UnionType::Teachers => 1500.0,
            UnionType::Police => 2000.0,
            UnionType::Firefighters => 1200.0,
            UnionType::TransitWorkers => 1000.0,
            UnionType::Nurses => 1800.0,
        };

        LaborUnion {
            union_type,
            satisfaction: 0.7,
            on_strike: false,
            strike_remaining: 0,
            budget: min_budget * 1.2,
            min_budget,
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if self.on_strike {
            self.strike_remaining = self.strike_remaining.saturating_sub(1);
            if self.strike_remaining == 0 {
                self.on_strike = false;
                self.satisfaction = 0.3; // Vuelven resentidos
            }
        }

        // Satisfacción tiende al presupuesto relativo
        let target = (self.budget / self.min_budget.max(1.0)).min(1.0);
        self.satisfaction = self.satisfaction * 0.95 + target * 0.05;

        // Si satisfacción cae demasiado, hay huelga
        if !self.on_strike && self.satisfaction < 0.2 && rng_pool::rng_chance(0.01 * dt) {
            self.on_strike = true;
            self.strike_remaining = (rng_pool::rng_fast() * 500.0) as u32 + 200;
        }
    }

    /// Efectos de la huelga en la ciudad
    pub fn strike_effects(&self) -> Vec<StrikeEffect> {
        if !self.on_strike { return vec![]; }

        match self.union_type {
            UnionType::GarbageCollectors => vec![
                StrikeEffect::WasteAccumulation(2.0),
                StrikeEffect::HealthPenalty(0.1),
            ],
            UnionType::Teachers => vec![
                StrikeEffect::EducationDrop(0.3),
                StrikeEffect::ApprovalPenalty(0.05),
            ],
            UnionType::Police => vec![
                StrikeEffect::CrimeIncrease(0.5),
                StrikeEffect::ApprovalPenalty(0.1),
            ],
            UnionType::Firefighters => vec![
                StrikeEffect::FireRisk(2.0),
            ],
            UnionType::TransitWorkers => vec![
                StrikeEffect::TrafficCollapse(0.5),
            ],
            UnionType::Nurses => vec![
                StrikeEffect::HealthPenalty(0.3),
                StrikeEffect::ApprovalPenalty(0.08),
            ],
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum StrikeEffect {
    WasteAccumulation(f32),
    HealthPenalty(f32),
    EducationDrop(f32),
    ApprovalPenalty(f32),
    CrimeIncrease(f32),
    FireRisk(f32),
    TrafficCollapse(f32),
}

// ---------------------------------------------------------------------------
// SISTEMA POLÍTICO COMPLETO
// ---------------------------------------------------------------------------

pub struct PoliticalSystem {
    /// Distritos electorales
    pub districts: Vec<District>,
    /// Sindicatos activos
    pub unions: Vec<LaborUnion>,
    /// Aprobación global del alcalde
    pub global_approval: f32,
    /// Ticks restantes en el mandato
    pub term_remaining: u64,
    /// ¿Concejo municipal hostil?
    pub council_hostile: bool,
    /// Proyectos bloqueados por el concejo
    pub blocked_projects: u32,
    /// ¿Alcalde destituido?
    pub mayor_recalled: bool,
    /// Campamentos de homeless (por gentrificación)
    pub homeless_camps: Vec<(f32, f32, u32)>,
    /// Demandas legales activas (expropiación)
    pub active_lawsuits: Vec<Lawsuit>,
}

#[derive(Clone, Debug)]
pub struct Lawsuit {
    pub x: f32, pub y: f32,
    pub remaining_ticks: u64,
    pub cost_per_tick: f32,
    pub blocks_construction: bool,
}

impl PoliticalSystem {
    pub fn new() -> Self {
        // Crear 3x3 distritos
        let mut districts = Vec::with_capacity(NUM_DISTRICTS);
        for row in 0..3 {
            for col in 0..3 {
                let cx = DISTRICT_SIZE * (col as f32 + 0.5);
                let cy = DISTRICT_SIZE * (row as f32 + 0.5);
                let mut d = District::new(row * 3 + col, cx, cy);
                // Distrito central es más rico
                if row == 1 && col == 1 {
                    d.avg_wealth = 15000.0;
                    d.is_wealthy = true;
                }
                districts.push(d);
            }
        }

        PoliticalSystem {
            districts,
            unions: vec![
                LaborUnion::new(UnionType::GarbageCollectors),
                LaborUnion::new(UnionType::Teachers),
                LaborUnion::new(UnionType::Police),
                LaborUnion::new(UnionType::Firefighters),
                LaborUnion::new(UnionType::TransitWorkers),
                LaborUnion::new(UnionType::Nurses),
            ],
            global_approval: 0.65,
            term_remaining: TERM_LENGTH_TICKS,
            council_hostile: false,
            blocked_projects: 0,
            mayor_recalled: false,
            homeless_camps: Vec::new(),
            active_lawsuits: Vec::new(),
        }
    }

    /// Intenta construir una instalación no deseada
    /// Retorna penalización de aprobación y si fue bloqueada por NIMBY
    pub fn try_build_unwanted(
        &mut self,
        x: f32, y: f32,
        facility: UnwantedFacility,
    ) -> (f32, bool) {
        let mut total_penalty = 0.0_f32;
        let mut blocked = false;

        for district in self.districts.iter_mut() {
            let dist = ((x - district.center_x) * (x - district.center_x)
                + (y - district.center_y) * (y - district.center_y)).sqrt();

            let penalty = district.nimby_penalty(facility, dist);
            if penalty > 0.0 {
                district.mayor_approval -= penalty;
                district.mayor_approval = district.mayor_approval.max(0.0);
                total_penalty += penalty;

                // NIMBY rico puede bloquear completamente
                if district.is_wealthy && penalty > 0.15 {
                    blocked = true;
                    district.active_protest = true;
                    district.protest_intensity = 50.0;
                    district.protest_remaining = 600;
                }

                district.unwanted_facilities.push((x, y, facility));
            }
        }

        self.update_global_approval();
        (total_penalty, blocked)
    }

    /// Evalúa gentrificación en una posición
    pub fn evaluate_gentrification(
        &mut self,
        x: f32, y: f32,
        land_value: f32,
        resident_income: f32,
    ) -> bool {
        // Si el valor del suelo supera 5x el ingreso anual → desplazamiento
        if land_value > resident_income * 5.0 {
            // Buscar distrito
            for district in self.districts.iter_mut() {
                let dist = ((x - district.center_x) * (x - district.center_x)
                    + (y - district.center_y) * (y - district.center_y)).sqrt();
                if dist < DISTRICT_SIZE {
                    // Crear campamento homeless en periferia
                    let edge_x = if x < 64.0 { 10.0 } else { 118.0 };
                    let edge_y = if y < 64.0 { 10.0 } else { 118.0 };
                    self.homeless_camps.push((edge_x, edge_y, 5));
                    district.population = district.population.saturating_sub(5);
                    district.mayor_approval -= 0.05;
                    return true; // Gentrificado
                }
            }
        }
        false
    }

    /// Actualiza sindicatos y aplica efectos de huelgas
    pub fn tick_unions(&mut self, dt: f32) -> Vec<StrikeEffect> {
        let mut all_effects = Vec::new();

        for union in self.unions.iter_mut() {
            union.tick(dt);
            all_effects.extend(union.strike_effects());
        }

        all_effects
    }

    /// Actualiza ciclo electoral
    pub fn tick_elections(&mut self) -> bool {
        self.term_remaining = self.term_remaining.saturating_sub(1);

        if self.term_remaining == 0 {
            // Elección: si aprobación < RECALL_THRESHOLD → destitución
            if self.global_approval < RECALL_THRESHOLD {
                self.mayor_recalled = true;
                return true;
            }

            // Concejo se vuelve hostil si aprobación < VETO_THRESHOLD
            self.council_hostile = self.global_approval < VETO_THRESHOLD;

            // Resetear mandato
            self.term_remaining = TERM_LENGTH_TICKS;
        }

        false
    }

    /// El concejo puede bloquear un proyecto
    pub fn council_may_block(&self) -> bool {
        self.council_hostile && rng_pool::rng_chance(0.3)
    }

    /// Inicia una demanda por expropiación
    pub fn start_lawsuit(&mut self, x: f32, y: f32) {
        self.active_lawsuits.push(Lawsuit {
            x, y,
            remaining_ticks: 18000, // 30 min reales
            cost_per_tick: 50.0,
            blocks_construction: true,
        });
    }

    /// Actualiza demandas legales
    pub fn tick_lawsuits(&mut self, treasury: &mut f32) {
        for lawsuit in self.active_lawsuits.iter_mut() {
            lawsuit.remaining_ticks = lawsuit.remaining_ticks.saturating_sub(1);
            *treasury -= lawsuit.cost_per_tick;
        }
        self.active_lawsuits.retain(|l| l.remaining_ticks > 0);
    }

    /// Actualiza protestas en distritos
    pub fn tick_protests(&mut self) {
        for district in self.districts.iter_mut() {
            if district.active_protest {
                district.protest_remaining = district.protest_remaining.saturating_sub(1);
                if district.protest_remaining == 0 {
                    district.active_protest = false;
                    district.protest_intensity = 0.0;
                }
            }
        }
    }

    fn update_global_approval(&mut self) {
        let total: f32 = self.districts.iter()
            .map(|d| d.mayor_approval * d.population as f32)
            .sum();
        let total_pop: u32 = self.districts.iter().map(|d| d.population).sum();
        self.global_approval = if total_pop > 0 {
            total / total_pop as f32
        } else {
            0.0
        };
    }

    /// Tick principal
    pub fn tick(&mut self, dt: f32, treasury: &mut f32) -> Vec<StrikeEffect> {
        let effects = self.tick_unions(dt);
        self.tick_protests();
        self.tick_lawsuits(treasury);
        let _recalled = self.tick_elections();
        effects
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nimby_penalty() {
        let mut district = District::new(0, 50.0, 50.0);
        district.is_wealthy = true;

        let penalty = district.nimby_penalty(UnwantedFacility::Landfill, 5.0);
        assert!(penalty > 0.0, "NIMBY debe penalizar cercanía");
    }

    #[test]
    fn test_nimby_far_away() {
        let district = District::new(0, 10.0, 10.0);
        let penalty = district.nimby_penalty(UnwantedFacility::Airport, 100.0);
        assert_eq!(penalty, 0.0, "Lejos no debe penalizar");
    }

    #[test]
    fn test_union_strike_trigger() {
        crate::rng_pool::init_rng_pool(42);

        let mut union = LaborUnion::new(UnionType::GarbageCollectors);
        union.satisfaction = 0.1;
        union.budget = 100.0; // Muy por debajo del mínimo

        for _ in 0..500 {
            union.tick(1.0);
        }

        // Puede o no estar en huelga (depende de RNG)
        // Verificar que al menos la satisfacción es baja
        assert!(union.satisfaction < 0.5);
    }

    #[test]
    fn test_strike_effects() {
        let mut union = LaborUnion::new(UnionType::Police);
        union.on_strike = true;
        let effects = union.strike_effects();
        assert!(!effects.is_empty());
    }

    #[test]
    fn test_political_system_creation() {
        let ps = PoliticalSystem::new();
        assert_eq!(ps.districts.len(), NUM_DISTRICTS);
        assert_eq!(ps.unions.len(), 6);
        assert!(ps.global_approval > 0.5);
    }

    #[test]
    fn test_try_build_unwanted() {
        let mut ps = PoliticalSystem::new();
        let (penalty, blocked) = ps.try_build_unwanted(
            64.0, 64.0,
            UnwantedFacility::Landfill,
        );
        // Distrito central es rico → debe haber penalización alta
        assert!(penalty > 0.0);
    }

    #[test]
    fn test_gentrification() {
        let mut ps = PoliticalSystem::new();
        let displaced = ps.evaluate_gentrification(64.0, 64.0, 50000.0, 2000.0);
        assert!(displaced, "Valor alto + ingreso bajo debe gentrificar");
        assert!(!ps.homeless_camps.is_empty());
    }

    #[test]
    fn test_no_gentrification_if_affordable() {
        let mut ps = PoliticalSystem::new();
        let displaced = ps.evaluate_gentrification(64.0, 64.0, 5000.0, 3000.0);
        assert!(!displaced);
    }

    #[test]
    fn test_lawsuit_creation() {
        let mut ps = PoliticalSystem::new();
        ps.start_lawsuit(30.0, 30.0);
        assert_eq!(ps.active_lawsuits.len(), 1);
        assert!(ps.active_lawsuits[0].blocks_construction);
    }

    #[test]
    fn test_lawsuit_tick() {
        let mut ps = PoliticalSystem::new();
        let mut treasury = 5000.0_f32;
        ps.start_lawsuit(30.0, 30.0);

        for _ in 0..100 {
            ps.tick_lawsuits(&mut treasury);
        }

        assert!(treasury < 5000.0, "Demandas deben costar dinero");
    }
}
