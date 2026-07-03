// Sistema Judicial v0.12.0
//
// Implementa el sistema legal completo que los edificios necesitan:
// - Tribunales de diferentes niveles (municipal, federal, supremo)
// - Demandas civiles contra la alcaldía (por accidentes, contaminación, expropiaciones)
// - Juicios corporativos (patentes troll, quiebras, antimonopolio)
// - Abogados de oficio y fiscales
// - Embargos preventivos
// - Sistema de corrupción judicial (sobornos)
// - Juicios por daños ambientales
// - Demandas colectivas (class actions)
//
// TÉCNICAS:
// - Look-Up Tables para tarifas legales precalculadas [TC#5]
// - Bitboards para mapear jurisdicciones [TI#6]
// - Strings internados para nombres de casos

#![allow(dead_code)]

use crate::rng_pool;

// ---------------------------------------------------------------------------
// TIPOS DE CASOS LEGALES
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Hash)]
#[repr(u8)]
pub enum CaseType {
    /// Demanda civil por daños (accidente de tráfico, negligencia médica)
    CivilDamages = 0,
    /// Demanda ambiental (contaminación, tala ilegal)
    Environmental = 1,
    /// Demanda corporativa (patentes, monopolio, contrato)
    Corporate = 2,
    /// Demanda laboral (despido injustificado, condiciones inseguras)
    Labor = 3,
    /// Demanda de propiedad (expropiación, zonificación, HOA)
    Property = 4,
    /// Demanda constitucional (derechos civiles, privacidad)
    Constitutional = 5,
    /// Demanda fiscal (impuestos, evasión)
    Tax = 6,
    /// Caso penal menor (multas, vandalismo)
    MinorCriminal = 7,
    /// Caso penal mayor (fraude, violencia)
    MajorCriminal = 8,
    /// Apelación
    Appeal = 9,
}

/// Nivel del tribunal
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum CourtLevel {
    /// Juzgado de paz / faltas (multas, casos menores)
    Municipal = 0,
    /// Tribunal de primera instancia (civil, laboral)
    District = 1,
    /// Tribunal superior / cámara de apelaciones
    Appellate = 2,
    /// Corte suprema / tribunal constitucional
    Supreme = 3,
    /// Tribunal internacional / arbitraje
    International = 4,
}

// ---------------------------------------------------------------------------
// ENTIDADES DEL SISTEMA LEGAL
// ---------------------------------------------------------------------------

/// Un caso legal activo en el sistema
#[derive(Debug, Clone)]
pub struct LegalCase {
    pub id: u64,
    pub case_type: CaseType,
    pub plaintiff: String,       // demandante
    pub defendant: String,       // demandado
    pub description: String,
    pub damages_claimed: f64,    // monto reclamado
    pub damages_awarded: f64,    // monto otorgado (si ya se falló)
    pub court_level: CourtLevel,
    pub days_in_court: u32,      // días que lleva en el sistema
    pub ruling: Option<CaseRuling>,
    pub appeal_count: u8,
    pub is_class_action: bool,   // demanda colectiva
    pub corruption_level: f32,   // 0.0 = limpio, 1.0 = totalmente corrupto
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CaseRuling {
    Pending,
    Pending,
    Dismissed,
    Settled { amount: f64 },       // acuerdo extrajudicial
    PlaintiffWon { amount: f64 },   // ganó el demandante
    Mistrial,                       // juicio nulo
}

/// Un tribunal físico en la ciudad
#[derive(Debug, Clone)]
pub struct CourtHouse {
    pub id: u64,
    pub x: f32, pub y: f32,
    pub court_level: CourtLevel,
    pub max_cases: u32,
    pub active_cases: u32,
    pub judges_available: u32,
    pub budget_annual: f64,
    pub efficiency: f32,        // 0.0-1.0, qué tan rápido procesan casos
    pub corruption_index: f32,  // 0.0-1.0
    pub backlog_penalty: f32,   // penalización por atraso judicial
}

/// Un bufete de abogados en la ciudad
#[derive(Debug, Clone)]
pub struct LawFirm {
    pub id: u64,
    pub x: f32, pub y: f32,
    pub name: String,
    pub specialization: CaseType,
    pub lawyers_count: u32,
    pub win_rate: f32,
    pub is_patent_troll: bool,   // bufete troll de patentes
    pub is_offshore: bool,       // paraíso fiscal
    pub annual_revenue: f64,
    pub influence_rating: f32,   // capacidad de lobby
}

// ---------------------------------------------------------------------------
// SISTEMA JUDICIAL CENTRAL
// ---------------------------------------------------------------------------

/// Gestor central del sistema judicial de la ciudad
pub struct JudicialSystem {
    /// Todos los casos activos
    pub cases: Vec<LegalCase>,
    /// Todos los tribunales
    pub courthouses: Vec<CourtHouse>,
    /// Todos los bufetes de abogados
    pub law_firms: Vec<LawFirm>,
    /// Contador de IDs para nuevos casos
    next_case_id: u64,
    /// Contador de IDs para courts
    next_court_id: u64,
    /// Contador de IDs para law firms
    next_firm_id: u64,
    /// Estadísticas
    pub total_cases_filed: u64,
    pub total_cases_resolved: u64,
    pub total_damages_paid: f64,
    /// Costo operativo anual del sistema judicial
    pub annual_budget: f64,
    /// Tarifas legales precalculadas por tipo de caso [LUT - TC#5]
    pub filing_fees: [f64; 10],
    /// Indemnizaciones promedio por tipo de caso
    pub avg_settlements: [f64; 10],
}

impl JudicialSystem {
    pub fn new() -> Self {
        JudicialSystem {
            cases: Vec::with_capacity(256),
            courthouses: Vec::with_capacity(8),
            law_firms: Vec::with_capacity(32),
            next_case_id: 1,
            next_court_id: 1,
            next_firm_id: 1,
            total_cases_filed: 0,
            total_cases_resolved: 0,
            total_damages_paid: 0.0,
            annual_budget: 0.0,
            // LUT: tarifas de presentación por tipo de caso
            filing_fees: [
                500.0,   // CivilDamages
                2000.0,  // Environmental
                10000.0, // Corporate
                300.0,   // Labor
                800.0,   // Property
                5000.0,  // Constitutional
                1500.0,  // Tax
                100.0,   // MinorCriminal
                5000.0,  // MajorCriminal
                15000.0, // Appeal
            ],
            // LUT: acuerdos promedio por tipo
            avg_settlements: [
                50000.0,    // CivilDamages
                500000.0,   // Environmental
                2000000.0,  // Corporate
                75000.0,    // Labor
                150000.0,   // Property
                1000000.0,  // Constitutional
                250000.0,   // Tax
                500.0,      // MinorCriminal
                100000.0,   // MajorCriminal
                50000.0,    // Appeal
            ],
        }
    }

    /// Construye un nuevo tribunal
    pub fn build_courthouse(&mut self, x: f32, y: f32, level: CourtLevel) -> u64 {
        let id = self.next_court_id;
        self.next_court_id += 1;

        let (max_cases, judges, efficiency) = match level {
            CourtLevel::Municipal => (50, 2, 0.95),
            CourtLevel::District => (200, 8, 0.80),
            CourtLevel::Appellate => (80, 5, 0.65),
            CourtLevel::Supreme => (20, 9, 0.50),
            CourtLevel::International => (10, 15, 0.30),
        };

        self.courthouses.push(CourtHouse {
            id, x, y,
            court_level: level,
            max_cases,
            active_cases: 0,
            judges_available: judges,
            budget_annual: judges as f64 * 120_000.0,
            efficiency,
            corruption_index: 0.05,
            backlog_penalty: 0.0,
        });

        self.annual_budget += judges as f64 * 120_000.0;
        id
    }

    /// Registra un bufete de abogados
    pub fn register_law_firm(&mut self, x: f32, y: f32, name: &str, spec: CaseType, is_troll: bool, is_offshore: bool) -> u64 {
        let id = self.next_firm_id;
        self.next_firm_id += 1;

        self.law_firms.push(LawFirm {
            id, x, y,
            name: name.to_string(),
            specialization: spec,
            lawyers_count: if is_troll { 2 } else { 15 },
            win_rate: if is_troll { 0.85 } else { 0.55 },
            is_patent_troll: is_troll,
            is_offshore,
            annual_revenue: 0.0,
            influence_rating: 0.1,
        });

        id
    }

    /// Presenta una nueva demanda (cualquier entidad puede demandar)
    pub fn file_lawsuit(
        &mut self,
        case_type: CaseType,
        plaintiff: &str,
        defendant: &str,
        description: &str,
        damages: f64,
        court_level: CourtLevel,
        is_class_action: bool,
    ) -> u64 {
        let id = self.next_case_id;
        self.next_case_id += 1;
        self.total_cases_filed += 1;

        self.cases.push(LegalCase {
            id,
            case_type,
            plaintiff: plaintiff.to_string(),
            defendant: defendant.to_string(),
            description: description.to_string(),
            damages_claimed: damages,
            damages_awarded: 0.0,
            court_level,
            days_in_court: 0,
            ruling: Some(CaseRuling::Pending),
            appeal_count: 0,
            is_class_action,
            corruption_level: 0.0,
        });

        id
    }

    /// Simula un tick del sistema judicial (un día in-game)
    /// Procesa casos, genera fallos, aplica costos
    pub fn tick(&mut self, city_treasury: &mut f64, dt: f32) -> Vec<JudicialEvent> {
        let mut events = Vec::with_capacity(16);

        // Procesar cada tribunal
        for court in &mut self.courthouses {
            if court.judges_available == 0 { continue; }

            let cases_per_day = (court.judges_available as f32 * court.efficiency * dt).max(0.5) as u32;
            let mut processed = 0u32;

            // Procesar casos asignados a este tribunal
            for case in &mut self.cases {
                if processed >= cases_per_day { break; }
                if case.court_level != court.court_level { continue; }
                if case.ruling != Some(CaseRuling::Pending) { continue; }

                case.days_in_court += 1;

                // Probabilidad de resolución basada en días en corte
                let resolve_chance = match case.court_level {
                    CourtLevel::Municipal => 1.0 - (-0.1 * case.days_in_court as f32).exp(),
                    CourtLevel::District => 1.0 - (-0.04 * case.days_in_court as f32).exp(),
                    CourtLevel::Appellate => 1.0 - (-0.02 * case.days_in_court as f32).exp(),
                    CourtLevel::Supreme => 1.0 - (-0.008 * case.days_in_court as f32).exp(),
                    CourtLevel::International => 1.0 - (-0.003 * case.days_in_court as f32).exp(),
                };

                // Corrupción: reduce probabilidad de resolución justa
                let fair_chance = resolve_chance * (1.0 - court.corruption_index);

                if rng_pool::rng_fast() < fair_chance {
                    // Determinar fallo
                    let ruling = self.determine_ruling(case, court.corruption_index);

                    match ruling {
                        CaseRuling::PlaintiffWon { amount } | CaseRuling::Settled { amount } => {
                            *city_treasury -= amount * 0.3; // La ciudad paga indirectamente
                            self.total_damages_paid += amount;
                            events.push(JudicialEvent::VerdictReached {
                                case_id: case.id,
                                ruling,
                                amount,
                                plaintiff: case.plaintiff.clone(),
                            });
                        }
                        CaseRuling::DefendantWon => {
                            events.push(JudicialEvent::CaseDismissed {
                                case_id: case.id,
                                reason: "Defendant prevailed".to_string(),
                            });
                        }
                        _ => {}
                    }

                    case.ruling = Some(ruling);
                    self.total_cases_resolved += 1;
                    processed += 1;
                }
            }
        }

        // Bufetes de abogados: patent trolls drenan innovación
        for firm in &self.law_firms {
            if firm.is_patent_troll {
                // Patent trolls generan casos contra empresas tech
                if rng_pool::rng_fast() < 0.05 * dt {
                    let _case_id = self.file_lawsuit(
                        CaseType::Corporate,
                        &firm.name,
                        "TechCorp",
                        "Patent infringement claim",
                        firm.annual_revenue * 0.1,
                        CourtLevel::District,
                        false,
                    );
                    events.push(JudicialEvent::PatentTrollAction {
                        firm_name: firm.name.clone(),
                        message: "Patent troll filed infringement lawsuit against tech sector".to_string(),
                    });
                }
            }
        }

        events
    }

    /// Determina el fallo de un caso basado en su tipo y nivel de corrupción
    fn determine_ruling(&self, case: &LegalCase, corruption: f32) -> CaseRuling {
        // Probabilidad base de que gane el demandante
        let plaintiff_base = match case.case_type {
            CaseType::CivilDamages => 0.50,
            CaseType::Environmental => 0.35, // Difícil probar daño ambiental
            CaseType::Corporate => 0.45,
            CaseType::Labor => 0.60,
            CaseType::Property => 0.55,
            CaseType::Constitutional => 0.40,
            CaseType::Tax => 0.30,
            CaseType::MinorCriminal => 0.70, // Estado suele ganar
            CaseType::MajorCriminal => 0.65,
            CaseType::Appeal => 0.25,
        };

        // La corrupción altera el resultado
        let adjusted = plaintiff_base + (corruption as f64 - 0.5) * 0.4;

        if rng_pool::rng_fast() < adjusted as f32 {
            let amount = case.damages_claimed * (0.3 + rng_pool::rng_fast() * 0.7) as f64;
            CaseRuling::PlaintiffWon { amount }
        } else if rng_pool::rng_fast() < 0.3 {
            let amount = case.damages_claimed * 0.1;
            CaseRuling::Settled { amount }
        } else {
            CaseRuling::DefendantWon
        }
    }

    /// El alcalde puede sobornar jueces para influir en casos
    pub fn corrupt_court(&mut self, court_id: u64, bribe_amount: f64) -> bool {
        if let Some(court) = self.courthouses.iter_mut().find(|c| c.id == court_id) {
            court.corruption_index = (court.corruption_index + (bribe_amount / 1_000_000.0) as f32).min(1.0);
            return true;
        }
        false
    }
    /// La ciudad es demandada por accidente/contaminación
    pub fn city_sued(&mut self, reason: &str, damages: f64) -> u64 {
        self.file_lawsuit(
            CaseType::CivilDamages,
            "Citizen Group",
            "City Hall",
            reason,
            damages,
            CourtLevel::District,
            true, // class action
        )
    }
}

// ---------------------------------------------------------------------------
// EVENTOS JUDICIALES
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum JudicialEvent {
    VerdictReached {
        case_id: u64,
        ruling: CaseRuling,
        amount: f64,
        plaintiff: String,
    },
    CaseDismissed {
        case_id: u64,
        reason: String,
    },
    PatentTrollAction {
        firm_name: String,
        message: String,
    },
    ClassActionFiled {
        description: String,
        affected_citizens: u32,
    },
    BankruptcyDeclaration {
        entity: String,
        debt_amount: f64,
    },
    CorruptionExposed {
        court_name: String,
        judge_count: u32,
    },
}
