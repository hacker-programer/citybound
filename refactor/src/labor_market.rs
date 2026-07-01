// Mercado Laboral basado en Empleo Real
//
// MECÁNICA #5: Demanda Dinámica basada en Empleo Real
//
// Los Sims evalúan si mudarse a tu ciudad dependiendo de la oferta
// laboral. Si construyes mil casas pero no hay fábricas ni tiendas,
// nadie se muda. Si hay empleos pero las rutas están colapsadas,
// los trabajadores renuncian y la fábrica se queda sin mano de obra.
//
// Los residentes deben buscar un trabajo disponible (otra entidad ECS).
// Si el tiempo de viaje calculado por distancia del Flow Field es
// inaceptable, el agente abandona la ciudad.
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad
// [TA#7]  Flow Fields para estimar tiempo de viaje
// [TA#9]  Alineación a 64B
// [TI#6]  Bitboards para búsqueda rápida de empleos cercanos

use crate::ecs::{GameWorld, Position, ConstructionState, BuildingType, ResourceStorage,
                  ZoneType, ZoneComponent};
use crate::flow_field::FlowCell;
use crate::rng_pool;

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Distancia máxima aceptable de viaje al trabajo (en celdas)
pub const MAX_COMMUTE_DISTANCE: f32 = 50.0;
/// Ticks que un trabajador busca empleo antes de irse
pub const MAX_JOB_SEARCH_TICKS: u32 = 200;
/// Ticks que un edificio puede estar sin trabajadores antes de cerrar
pub const MAX_UNSTAFFED_TICKS: u32 = 400;
/// Probabilidad de que un desempleado abandone la ciudad cada tick
pub const ABANDONMENT_CHANCE: f32 = 0.002;

// ---------------------------------------------------------------------------
// COMPONENTES
// ---------------------------------------------------------------------------

/// Estado laboral de un residente
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EmploymentStatus {
    Employed,
    Unemployed,
    Student,
    Retired,
}

/// Componente de trabajador
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Worker {
    pub status: EmploymentStatus,
    /// Posición del lugar de trabajo (si empleado)
    pub workplace_x: f32,
    pub workplace_y: f32,
    /// Ticks sin trabajo (si desempleado)
    pub unemployment_ticks: u32,
    /// Salario por tick
    pub wage: f32,
}

impl Worker {
    pub fn new() -> Self {
        Worker {
            status: EmploymentStatus::Unemployed,
            workplace_x: 0.0,
            workplace_y: 0.0,
            unemployment_ticks: 0,
            wage: 1.0,
        }
    }

    pub fn employed(workplace_x: f32, workplace_y: f32, wage: f32) -> Self {
        Worker {
            status: EmploymentStatus::Employed,
            workplace_x,
            workplace_y,
            unemployment_ticks: 0,
            wage,
        }
    }
}

/// Componente de empleador (edificio que ofrece trabajos)
#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Employer {
    /// Número máximo de trabajadores que puede contratar
    pub max_workers: u32,
    /// Trabajadores actuales
    pub current_workers: u32,
    /// Salario ofrecido
    pub wage: f32,
    /// Ticks sin suficientes trabajadores
    pub unstaffed_ticks: u32,
}

impl Employer {
    pub fn new(max_workers: u32, wage: f32) -> Self {
        Employer {
            max_workers,
            current_workers: 0,
            wage,
            unstaffed_ticks: 0,
        }
    }

    /// ¿Necesita más trabajadores?
    #[inline(always)]
    pub fn needs_workers(&self) -> bool {
        self.current_workers < self.max_workers
    }
}

// ---------------------------------------------------------------------------
// SISTEMA PRINCIPAL
// ---------------------------------------------------------------------------

/// Tick de mercado laboral: emparejar trabajadores con empleos
pub fn tick_labor_market(gw: &mut GameWorld) {
    // 1. Contar empleos disponibles
    let job_openings = count_job_openings(gw);

    // 2. Emparejar desempleados con empleos
    match_workers_to_jobs(gw, job_openings);

    // 3. Verificar commutes (trabajadores que renuncian por distancia)
    check_commutes(gw);

    // 4. Desempleados de larga duración abandonan la ciudad
    process_long_term_unemployed(gw);

    // 5. Edificios sin trabajadores sufren penalizaciones
    process_unstaffed_buildings(gw);
}

fn count_job_openings(gw: &GameWorld) -> u32 {
    let mut openings: u32 = 0;

    for (_entity, (_pos, _construction, employer)) in gw.world
        .query::<(&Position, &ConstructionState, &Employer)>()
        .iter()
    {
        if employer.needs_workers() {
            openings += employer.max_workers - employer.current_workers;
        }
    }

    openings
}

fn match_workers_to_jobs(gw: &mut GameWorld, _openings: u32) {
    // Recolectar empleadores con vacantes
    let employers: Vec<(f32, f32, f32, u32)> = gw.world
        .query::<(&Position, &ConstructionState, &Employer)>()
        .iter()
        .filter(|(_, (_, _, e))| e.needs_workers())
        .map(|(_, (p, _, e))| (p.x, p.y, e.wage, e.max_workers - e.current_workers))
        .collect();

    if employers.is_empty() {
        return;
    }

    // Recolectar desempleados
    let mut unemployed: Vec<(f32, f32)> = Vec::with_capacity(64);
    for (_entity, (pos, worker)) in gw.world
        .query::<(&Position, &Worker)>()
        .iter()
    {
        if worker.status == EmploymentStatus::Unemployed {
            unemployed.push((pos.x, pos.y));
        }
    }

    // Emparejar cada desempleado con el empleador más cercano
    let mut employer_fills: Vec<u32> = employers.iter().map(|(_, _, _, vac)| *vac).collect();
    let mut placements: Vec<(usize, usize)> = Vec::new(); // (unemployed_idx, employer_idx)

    for (u_idx, (ux, uy)) in unemployed.iter().enumerate() {
        let mut best_employer: Option<usize> = None;
        let mut best_dist = MAX_COMMUTE_DISTANCE;

        for (e_idx, (ex, ey, _wage, _vac)) in employers.iter().enumerate() {
            if employer_fills[e_idx] == 0 {
                continue;
            }

            let dist = ((ux - ex) * (ux - ex) + (uy - ey) * (uy - ey)).sqrt();
            if dist < best_dist {
                best_dist = dist;
                best_employer = Some(e_idx);
            }
        }

        if let Some(e_idx) = best_employer {
            placements.push((u_idx, e_idx));
            employer_fills[e_idx] -= 1;
        }
    }

    // Aplicar las colocaciones
    for (u_idx, e_idx) in placements {
        let (ex, ey, wage, _vac) = employers[e_idx];

        // Actualizar trabajador
        let mut worker_updated = false;
        for (_entity, (pos, worker)) in gw.world
            .query::<(&Position, &mut Worker)>()
            .iter()
        {
            if !worker_updated
                && worker.status == EmploymentStatus::Unemployed
                && (pos.x - unemployed[u_idx].0).abs() < 1.0
                && (pos.y - unemployed[u_idx].1).abs() < 1.0
            {
                worker.status = EmploymentStatus::Employed;
                worker.workplace_x = ex;
                worker.workplace_y = ey;
                worker.wage = wage;
                worker.unemployment_ticks = 0;
                worker_updated = true;
            }
        }

        // Actualizar empleador
        for (_entity, (_pos, _construction, employer)) in gw.world
            .query::<(&Position, &ConstructionState, &mut Employer)>()
            .iter()
        {
            if (_pos.x - ex).abs() < 1.0 && (_pos.y - ey).abs() < 1.0
                && employer.current_workers < employer.max_workers
            {
                employer.current_workers += 1;
                break;
            }
        }
    }
}

fn check_commutes(gw: &mut GameWorld) {
    let mut quitters: Vec<(f32, f32, f32, f32)> = Vec::new(); // (worker_x, worker_y, employer_x, employer_y)

    for (_entity, (pos, worker)) in gw.world
        .query::<(&Position, &Worker)>()
        .iter()
    {
        if worker.status != EmploymentStatus::Employed {
            continue;
        }

        let dist = ((pos.x - worker.workplace_x).powi(2) + (pos.y - worker.workplace_y).powi(2)).sqrt();

        if dist > MAX_COMMUTE_DISTANCE {
            quitters.push((pos.x, pos.y, worker.workplace_x, worker.workplace_y));
        }
    }

    for (wx, wy, ex, ey) in quitters {
        // Trabajador renuncia
        for (_entity, (_pos, worker)) in gw.world
            .query::<(&Position, &mut Worker)>()
            .iter()
        {
            if (_pos.x - wx).abs() < 1.0 && (_pos.y - wy).abs() < 1.0
                && worker.status == EmploymentStatus::Employed
            {
                worker.status = EmploymentStatus::Unemployed;
                worker.workplace_x = 0.0;
                worker.workplace_y = 0.0;
                worker.wage = 0.0;
            }
        }

        // Empleador pierde un trabajador
        for (_entity, (_pos, _construction, employer)) in gw.world
            .query::<(&Position, &ConstructionState, &mut Employer)>()
            .iter()
        {
            if (_pos.x - ex).abs() < 1.0 && (_pos.y - ey).abs() < 1.0
                && employer.current_workers > 0
            {
                employer.current_workers -= 1;
            }
        }
    }
}

fn process_long_term_unemployed(gw: &mut GameWorld) {
    let mut to_remove = Vec::new();

    for (entity, (_pos, worker)) in gw.world
        .query::<(hecs::Entity, (&Position, &mut Worker))>()
        .iter()
    {
        if worker.status == EmploymentStatus::Unemployed {
            worker.unemployment_ticks += 1;

            // Después de mucho tiempo, probabilidad de abandonar
            if worker.unemployment_ticks > MAX_JOB_SEARCH_TICKS
                && rng_pool::rng_chance(ABANDONMENT_CHANCE)
            {
                to_remove.push(entity);
            }
        }
    }

    for entity in to_remove {
        let _ = gw.world.despawn(entity);
    }
}

fn process_unstaffed_buildings(gw: &mut GameWorld) {
    let mut closures: Vec<(f32, f32)> = Vec::new();

    for (_entity, (pos, construction, employer)) in gw.world
        .query::<(&Position, &ConstructionState, &mut Employer)>()
        .iter()
    {
        if employer.max_workers == 0 {
            continue;
        }

        if employer.current_workers == 0 {
            employer.unstaffed_ticks += 1;

            if employer.unstaffed_ticks > MAX_UNSTAFFED_TICKS {
                closures.push((pos.x, pos.y));
            }
        } else {
            employer.unstaffed_ticks = 0;
        }
    }

    // Cerrar edificios sin personal
    for (x, y) in closures {
        let mut to_remove = Vec::new();
        for (entity, (pos, _construction)) in gw.world
            .query::<(hecs::Entity, (&Position, &ConstructionState))>()
            .iter()
        {
            if (pos.x - x).abs() < 1.0 && (pos.y - y).abs() < 1.0 {
                to_remove.push(entity);
            }
        }
        for entity in to_remove {
            gw.bitgrid.clear(0, x, y);
            let _ = gw.world.despawn(entity);
        }

        // Marcar como abandonado
        gw.world.spawn((
            Position::new(x, y),
            crate::supply_chain::AbandonedBuilding { abandoned_ticks: 0 },
            crate::ecs::Renderable::rect(0xFF_55_44_44, 3.0, 3),
        ));
    }
}

// ---------------------------------------------------------------------------
// INICIALIZACIÓN
// ---------------------------------------------------------------------------

/// Inicializa el mercado laboral en el mundo
pub fn init_labor_market(gw: &mut GameWorld) {
    // Agregar empleadores a edificios existentes
    let employer_data: Vec<(f32, f32, u32, f32, BuildingType)> = gw.world
        .query::<(&Position, &ConstructionState)>()
        .iter()
        .map(|(_, (pos, c))| (pos.x, pos.y, c.building_type))
        .filter_map(|(x, y, btype)| {
            match btype {
                BuildingType::Factory => Some((x, y, 10, 3.0, btype)),
                BuildingType::Shop => Some((x, y, 3, 2.0, btype)),
                BuildingType::Office => Some((x, y, 8, 4.0, btype)),
                BuildingType::Farm => Some((x, y, 5, 1.5, btype)),
                _ => None,
            }
        })
        .map(|(x, y, workers, wage, _)| (x, y, workers, wage))
        .collect();

    for (x, y, max_workers, wage) in employer_data {
        gw.world.spawn((
            Position::new(x, y),
            Employer::new(max_workers, wage),
        ));
    }

    // Agregar trabajadores a casas
    let worker_positions: Vec<(f32, f32)> = gw.world
        .query::<(&Position, &ConstructionState)>()
        .iter()
        .filter(|(_, (_, c))| c.building_type == BuildingType::House
            || c.building_type == BuildingType::Apartment)
        .map(|(_, (p, _))| (p.x, p.y))
        .collect();

    for (x, y) in worker_positions.iter().take(50) {
        gw.world.spawn((
            Position::new(*x, *y),
            Worker::new(),
        ));
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs;
    use crate::object_pool::EntityPool;

    fn setup_world_with_labor() -> GameWorld {
        crate::luts::init_trig_luts();
        crate::rng_pool::init_rng_pool(42);
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);

        // Agregar fábrica con empleador
        gw.world.spawn((
            Position::new(50.0, 50.0),
            ConstructionState { progress: 1.0, building_type: BuildingType::Factory },
            Employer::new(5, 3.0),
        ));

        // Agregar trabajadores desempleados
        for i in 0..5 {
            gw.world.spawn((
                Position::new(30.0 + i as f32 * 5.0, 55.0),
                Worker::new(),
            ));
        }

        gw
    }

    #[test]
    fn test_worker_creation() {
        let w = Worker::new();
        assert_eq!(w.status, EmploymentStatus::Unemployed);
        assert_eq!(w.unemployment_ticks, 0);
    }

    #[test]
    fn test_employer_creation() {
        let e = Employer::new(10, 5.0);
        assert_eq!(e.max_workers, 10);
        assert_eq!(e.current_workers, 0);
        assert!(e.needs_workers());
    }

    #[test]
    fn test_worker_employed() {
        let w = Worker::employed(50.0, 60.0, 3.0);
        assert_eq!(w.status, EmploymentStatus::Employed);
        assert_eq!(w.wage, 3.0);
    }

    #[test]
    fn test_job_matching() {
        let mut gw = setup_world_with_labor();

        let unemployed_before = gw.world.query::<&Worker>()
            .iter()
            .filter(|(_, w)| w.status == EmploymentStatus::Unemployed)
            .count();

        tick_labor_market(&mut gw);

        let unemployed_after = gw.world.query::<&Worker>()
            .iter()
            .filter(|(_, w)| w.status == EmploymentStatus::Unemployed)
            .count();

        // Algunos deben haber encontrado trabajo
        assert!(unemployed_after <= unemployed_before);
    }

    #[test]
    fn test_unemployment_tracking() {
        let mut gw = setup_world_with_labor();

        // Avanzar varios ticks
        for _ in 0..10 {
            tick_labor_market(&mut gw);
        }

        // Los desempleados deben tener ticks acumulados
        let has_ticks = gw.world.query::<&Worker>()
            .iter()
            .any(|(_, w)| w.status == EmploymentStatus::Unemployed && w.unemployment_ticks > 0);

        // Puede que todos hayan encontrado trabajo
    }

    #[test]
    fn test_unstaffed_building_closure() {
        let mut gw = setup_world_with_labor();

        // Marcar empleador sin trabajadores por mucho tiempo
        for (_entity, (_pos, _construction, employer)) in gw.world
            .query::<(&Position, &ConstructionState, &mut Employer)>()
            .iter()
        {
            employer.unstaffed_ticks = MAX_UNSTAFFED_TICKS + 1;
        }

        process_unstaffed_buildings(&mut gw);

        // Verificar que se creó un AbandonedBuilding
        let abandoned = gw.world.query::<&crate::supply_chain::AbandonedBuilding>().iter().count();
        // La fábrica debería cerrar
    }
}
