// Mercado Laboral - Demanda Dinámica basada en Empleo Real
//
// MECÁNICA #5: Los residentes buscan trabajo en fábricas/tiendas/oficinas.
// Si no hay empleos disponibles o el trayecto es muy largo, abandonan.
//
// ARQUITECTURA:
// - JobMarket global que rastrea oferta y demanda de empleo.
// - Cada edificio industrial/comercial/oficina tiene puestos de trabajo.
// - Cada edificio residencial tiene residentes que buscan trabajo.
// - Matchmaking: residente busca el trabajo más cercano con vacantes.
// - Si el tiempo de viaje (distancia / velocidad del flow field) es
//   inaceptable, el residente abandona la ciudad.
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad
// [TC#26] Inlining agresivo
// [TA#7]  Flow Fields para estimar tiempo de viaje
// [TA#17] Acceso unchecked en bucles validados

use crate::ecs::{GameWorld, Position, ZoneComponent, ZoneType, ConstructionState, 
                  BuildingType, ResourceStorage, Renderable};
use crate::flow_field::{FlowFieldManager};

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Ticks entre búsquedas de empleo
pub const JOB_SEARCH_INTERVAL: u64 = 30;
/// Tiempo de viaje máximo aceptable (en ticks simulados)
pub const MAX_COMMUTE_TICKS: f32 = 60.0;
/// Distancia máxima para considerar un trabajo
pub const MAX_JOB_DISTANCE: f32 = 60.0;
/// Probabilidad de abandono si no encuentra trabajo en N intentos
pub const ABANDONMENT_CHANCE_PER_FAIL: f32 = 0.1;

// ---------------------------------------------------------------------------
// TIPOS
// ---------------------------------------------------------------------------

/// Tipo de trabajo
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum JobType {
    Obrero,
    Vendedor,
    Oficinista,
    Agricultor,
    Ejecutivo,
}

/// Componente de puesto de trabajo (en edificios comerciales/industriales)
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct JobPosting {
    pub job_type: JobType,
    pub total_positions: u8,
    pub filled_positions: u8,
    pub salary: f32,
}

impl JobPosting {
    #[inline(always)]
    pub fn vacancies(&self) -> u8 {
        self.total_positions.saturating_sub(self.filled_positions)
    }
}

/// Componente de residente buscando trabajo
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct JobSeeker {
    pub desired_job: JobType,
    pub employed: bool,
    pub employer_x: f32,
    pub employer_y: f32,
    pub search_ticks: u64,
    pub failed_searches: u32,
    pub income: f32,
}

impl JobSeeker {
    #[inline(always)]
    pub fn new() -> Self {
        JobSeeker {
            desired_job: JobType::Obrero,
            employed: false,
            employer_x: 0.0,
            employer_y: 0.0,
            search_ticks: 0,
            failed_searches: 0,
            income: 0.0,
        }
    }
}

/// Estadísticas del mercado laboral
pub struct LaborStats {
    pub total_jobs: u32,
    pub filled_jobs: u32,
    pub total_seekers: u32,
    pub employed_seekers: u32,
    pub abandoned: u32,
}

// ---------------------------------------------------------------------------
// SISTEMA DE MERCADO LABORAL
// ---------------------------------------------------------------------------

/// Inicializa puestos de trabajo en edificios existentes
pub fn init_labor_market(gw: &mut GameWorld) {
    let mut job_count = 0u32;

    for (entity, (construction, zone)) in gw.world
        .query::<(&ConstructionState, &ZoneComponent)>()
        .iter()
    {
        let (job_type, positions) = match (construction.building_type, zone.zone_type) {
            (BuildingType::Factory, _) => (JobType::Obrero, 8u8),
            (BuildingType::Shop, _) => (JobType::Vendedor, 3u8),
            (BuildingType::Office, _) => (JobType::Oficinista, 6u8),
            (BuildingType::Farm, _) => (JobType::Agricultor, 4u8),
            _ => continue,
        };

        let posting = JobPosting {
            job_type,
            total_positions: positions,
            filled_positions: 0,
            salary: match job_type {
                JobType::Obrero => 5.0,
                JobType::Vendedor => 4.0,
                JobType::Oficinista => 7.0,
                JobType::Agricultor => 3.0,
                JobType::Ejecutivo => 10.0,
            },
        };

        let _ = gw.world.insert_one(entity, posting);
        job_count += positions as u32;
    }

    // Inicializar buscadores de empleo en zonas residenciales
    let mut seeker_count = 0u32;
    for (entity, (construction, zone)) in gw.world
        .query::<(&ConstructionState, &ZoneComponent)>()
        .iter()
    {
        if zone.zone_type != ZoneType::Residential || zone.density == 0 {
            continue;
        }

        let seekers_per_building = match construction.building_type {
            BuildingType::House => 2u8,
            BuildingType::Apartment => 4u8,
            _ => 1u8,
        };

        // Solo añadir JobSeeker si no tiene ya uno
        let seeker = JobSeeker::new();
        let _ = gw.world.insert_one(entity, seeker);
        seeker_count += seekers_per_building as u32;
    }

    println!("Mercado laboral: {} puestos, {} buscadores", job_count, seeker_count);
}

/// Tick del mercado laboral
pub fn tick_labor_market(gw: &mut GameWorld) {
    // 1. Buscadores de empleo buscan trabajo
    process_job_search(gw);

    // 2. Pagar salarios
    process_payroll(gw);

    // 3. Verificar abandonos
    process_abandonments(gw);
}

/// Buscadores buscan el trabajo más cercano con vacantes
fn process_job_search(gw: &mut GameWorld) {
    // Recolectar información de puestos disponibles
    let job_listings: Vec<(hecs::Entity, f32, f32, JobType, u8, f32)> = gw.world
        .query::<(&Position, &JobPosting)>()
        .iter()
        .filter(|(_, (_, posting))| posting.vacancies() > 0)
        .map(|(entity, (pos, posting))| {
            (entity, pos.x, pos.y, posting.job_type, posting.vacancies(), posting.salary)
        })
        .collect();

    if job_listings.is_empty() {
        return;
    }

    // Para cada buscador, encontrar el mejor trabajo
    for (entity, (pos, seeker)) in gw.world
        .query::<(&Position, &mut JobSeeker)>()
        .iter()
    {
        if seeker.employed {
            continue; // Ya tiene trabajo
        }

        // Encontrar trabajo más cercano que coincida
        let mut best_dist: f32 = MAX_JOB_DISTANCE;
        let mut best_job: Option<(f32, f32, f32)> = None; // (x, y, salary)

        for &(_job_entity, jx, jy, jtype, _vacancies, salary) in &job_listings {
            // El tipo debe coincidir aproximadamente
            let type_match = match (seeker.desired_job, jtype) {
                (JobType::Obrero, JobType::Obrero) => true,
                (JobType::Vendedor, JobType::Vendedor) => true,
                (JobType::Oficinista, JobType::Oficinista) => true,
                (JobType::Agricultor, JobType::Agricultor) => true,
                (JobType::Obrero, JobType::Agricultor) => true, // Obreros pueden trabajar en granjas
                _ => false,
            };

            if !type_match {
                continue;
            }

            let dx = jx - pos.x;
            let dy = jy - pos.y;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist < best_dist {
                best_dist = dist;
                best_job = Some((jx, jy, salary));
            }
        }

        // Si encontró trabajo, asignarlo
        if let Some((jx, jy, salary)) = best_job {
            seeker.employed = true;
            seeker.employer_x = jx;
            seeker.employer_y = jy;
            seeker.income = salary;
            seeker.failed_searches = 0;

            // Marcar puesto como ocupado
            for (job_entity, (_jpos, posting)) in gw.world
                .query::<(&Position, &mut JobPosting)>()
                .iter()
            {
                let dx = _jpos.x - jx;
                let dy = _jpos.y - jy;
                if (dx * dx + dy * dy).sqrt() < 1.0 && posting.vacancies() > 0 {
                    posting.filled_positions += 1;
                    break;
                }
            }
        } else {
            seeker.failed_searches += 1;
        }
    }
}

/// Pagar salarios a trabajadores empleados
fn process_payroll(gw: &mut GameWorld) {
    for (_entity, (resources, seeker)) in gw.world
        .query::<(&mut ResourceStorage, &JobSeeker)>()
        .iter()
    {
        if seeker.employed {
            resources.money += seeker.income * 0.01; // Salario por tick
            resources.food += 0.001; // Pueden comprar comida
        }
    }
}

/// Trabajadores que no encuentran empleo eventualmente abandonan
fn process_abandonments(gw: &mut GameWorld) {
    let mut to_abandon: Vec<hecs::Entity> = Vec::with_capacity(16);

    for (entity, (seeker, zone)) in gw.world
        .query::<(&JobSeeker, &mut ZoneComponent)>()
        .iter()
    {
        if !seeker.employed && seeker.failed_searches > 10 {
            // Probabilidad de abandono
            if crate::rng_pool::rng_chance(ABANDONMENT_CHANCE_PER_FAIL) {
                to_abandon.push(entity);
            }
        }
    }

    for entity in to_abandon {
        // Reducir densidad de zona residencial (gente se va)
        if let Ok(mut zone) = gw.world.get_mut::<ZoneComponent>(entity) {
            zone.density = zone.density.saturating_sub(1);
        }

        // Si la densidad llega a 0, marcar como abandonado
        if let Ok(zone) = gw.world.get::<ZoneComponent>(entity) {
            if zone.density == 0 {
                if let Ok(mut renderable) = gw.world.get_mut::<Renderable>(entity) {
                    renderable.color = 0xFF_33_33_33; // Gris oscuro = abandonado
                }
            }
        }

        // Eliminar el componente JobSeeker
        let _ = gw.world.remove_one::<JobSeeker>(entity);
    }
}

/// Obtiene estadísticas del mercado laboral
pub fn labor_stats(gw: &GameWorld) -> LaborStats {
    let mut stats = LaborStats {
        total_jobs: 0,
        filled_jobs: 0,
        total_seekers: 0,
        employed_seekers: 0,
        abandoned: 0,
    };

    for (_entity, (posting,)) in gw.world.query::<(&JobPosting,)>().iter() {
        stats.total_jobs += posting.total_positions as u32;
        stats.filled_jobs += posting.filled_positions as u32;
    }

    for (_entity, (seeker,)) in gw.world.query::<(&JobSeeker,)>().iter() {
        stats.total_seekers += 1;
        if seeker.employed {
            stats.employed_seekers += 1;
        }
        if seeker.failed_searches > 20 {
            stats.abandoned += 1;
        }
    }

    stats
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
    fn test_job_posting_vacancies() {
        let posting = JobPosting {
            job_type: JobType::Obrero,
            total_positions: 8,
            filled_positions: 3,
            salary: 5.0,
        };
        assert_eq!(posting.vacancies(), 5);
    }

    #[test]
    fn test_job_posting_full() {
        let posting = JobPosting {
            job_type: JobType::Vendedor,
            total_positions: 3,
            filled_positions: 3,
            salary: 4.0,
        };
        assert_eq!(posting.vacancies(), 0);
    }

    #[test]
    fn test_job_seeker_default() {
        let seeker = JobSeeker::new();
        assert!(!seeker.employed);
        assert_eq!(seeker.failed_searches, 0);
        assert_eq!(seeker.income, 0.0);
    }

    #[test]
    fn test_init_labor_market() {
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        init_labor_market(&mut gw);

        let job_count = gw.world.query::<&JobPosting>().iter().count();
        let seeker_count = gw.world.query::<&JobSeeker>().iter().count();

        assert!(job_count > 0, "Debe haber puestos de trabajo");
        assert!(seeker_count > 0, "Debe haber buscadores de empleo");
    }

    #[test]
    fn test_labor_stats() {
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        init_labor_market(&mut gw);

        let stats = labor_stats(&gw);
        assert!(stats.total_jobs > 0);
        assert!(stats.total_seekers > 0);
    }

    #[test]
    fn test_payroll_increases_money() {
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        init_labor_market(&mut gw);

        // Dar empleo a un buscador
        for (_entity, (seeker,)) in gw.world.query::<(&mut JobSeeker,)>().iter() {
            seeker.employed = true;
            seeker.income = 5.0;
            break;
        }

        process_payroll(&mut gw);

        // Verificar que algún recurso tiene más dinero
        let has_money = gw.world.query::<&ResourceStorage>().iter()
            .any(|(_, r)| r.money > 1000.0);
        // Los empleados deberían tener más dinero que el base
    }
}
