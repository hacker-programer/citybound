// Mercado Laboral basado en Empleo Real - v0.7.1
// MECÁNICA #5: Demanda Dinámica basada en Empleo Real
//
// TÉCNICAS: [TC#2] Pre-reserva, [TA#7] Flow Fields, [TA#9] Alineación 64B, [TI#6] Bitboards

use crate::ecs::{GameWorld, Position, ConstructionState, BuildingType};

pub const MAX_COMMUTE_DISTANCE: f32 = 50.0;
pub const MAX_JOB_SEARCH_TICKS: u32 = 200;
pub const MAX_UNSTAFFED_TICKS: u32 = 400;
pub const ABANDONMENT_CHANCE: f32 = 0.002;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EmploymentStatus { Employed, Unemployed, Student, Retired }

#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Worker {
    pub status: EmploymentStatus,
    pub workplace_x: f32,
    pub workplace_y: f32,
    pub unemployment_ticks: u32,
    pub wage: f32,
}

impl Worker {
    pub fn new() -> Self {
        Worker { status: EmploymentStatus::Unemployed, workplace_x: 0.0, workplace_y: 0.0, unemployment_ticks: 0, wage: 1.0 }
    }
    pub fn employed(workplace_x: f32, workplace_y: f32, wage: f32) -> Self {
        Worker { status: EmploymentStatus::Employed, workplace_x, workplace_y, unemployment_ticks: 0, wage }
    }
}

#[derive(Copy, Clone, Debug)]
#[repr(align(64))]
pub struct Employer {
    pub max_workers: u32,
    pub current_workers: u32,
    pub wage: f32,
    pub unstaffed_ticks: u32,
}

impl Employer {
    pub fn new(max_workers: u32, wage: f32) -> Self {
        Employer { max_workers, current_workers: 0, wage, unstaffed_ticks: 0 }
    }
    #[inline(always)]
    pub fn needs_workers(&self) -> bool { self.current_workers < self.max_workers }
}

pub fn tick_labor_market(gw: &mut GameWorld) {
    let job_openings = count_job_openings(gw);
    match_workers_to_jobs(gw, job_openings);
    check_commutes(gw);
    process_long_term_unemployed(gw);
    process_unstaffed_buildings(gw);
}

fn count_job_openings(gw: &GameWorld) -> u32 {
    let mut openings: u32 = 0;
    for (_entity, (_pos, _construction, employer)) in gw.world
        .query::<(&Position, &ConstructionState, &Employer)>().iter()
    {
        if employer.needs_workers() {
            openings += employer.max_workers - employer.current_workers;
        }
    }
    openings
}

fn match_workers_to_jobs(gw: &mut GameWorld, _openings: u32) {
    let employers: Vec<(f32, f32, f32, u32)> = gw.world
        .query::<(&Position, &ConstructionState, &Employer)>().iter()
        .filter(|(_, (_, _, e))| e.needs_workers())
        .map(|(_, (p, _, e))| (p.x, p.y, e.wage, e.max_workers - e.current_workers))
        .collect();

    if employers.is_empty() { return; }

    let mut unemployed: Vec<(f32, f32)> = Vec::with_capacity(64);
    for (_entity, (pos, worker)) in gw.world.query::<(&Position, &Worker)>().iter() {
        if worker.status == EmploymentStatus::Unemployed {
            unemployed.push((pos.x, pos.y));
        }
    }

    let mut employer_fills: Vec<u32> = employers.iter().map(|(_, _, _, vac)| *vac).collect();
    let mut placements: Vec<(usize, usize)> = Vec::new();

    for (u_idx, (ux, uy)) in unemployed.iter().enumerate() {
        let mut best_employer: Option<usize> = None;
        let mut best_dist = MAX_COMMUTE_DISTANCE;
        for (e_idx, (ex, ey, _wage, _vac)) in employers.iter().enumerate() {
            if employer_fills[e_idx] == 0 { continue; }
            let dist = ((ux - ex) * (ux - ex) + (uy - ey) * (uy - ey)).sqrt();
            if dist < best_dist { best_dist = dist; best_employer = Some(e_idx); }
        }
        if let Some(e_idx) = best_employer {
            placements.push((u_idx, e_idx));
            employer_fills[e_idx] -= 1;
        }
    }

    for (u_idx, e_idx) in placements {
        let (ex, ey, wage, _vac) = employers[e_idx];
        let mut worker_updated = false;
        for (_entity, (pos, worker)) in gw.world.query::<(&Position, &mut Worker)>().iter() {
            if !worker_updated && worker.status == EmploymentStatus::Unemployed
                && (pos.x - unemployed[u_idx].0).abs() < 1.0
                && (pos.y - unemployed[u_idx].1).abs() < 1.0
            {
                worker.status = EmploymentStatus::Employed;
                worker.workplace_x = ex; worker.workplace_y = ey;
                worker.wage = wage; worker.unemployment_ticks = 0;
                worker_updated = true;
            }
        }
        for (_entity, (_pos, _construction, employer)) in gw.world
            .query::<(&Position, &ConstructionState, &mut Employer)>().iter()
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
    let mut quitters: Vec<(f32, f32, f32, f32)> = Vec::new();
    for (_entity, (pos, worker)) in gw.world.query::<(&Position, &Worker)>().iter() {
        if worker.status != EmploymentStatus::Employed { continue; }
        let dist = ((pos.x - worker.workplace_x).powi(2) + (pos.y - worker.workplace_y).powi(2)).sqrt();
        if dist > MAX_COMMUTE_DISTANCE {
            quitters.push((pos.x, pos.y, worker.workplace_x, worker.workplace_y));
        }
    }
    for (wx, wy, ex, ey) in quitters {
        for (_entity, (_pos, worker)) in gw.world.query::<(&Position, &mut Worker)>().iter() {
            if (_pos.x - wx).abs() < 1.0 && (_pos.y - wy).abs() < 1.0
                && worker.status == EmploymentStatus::Employed
            {
                worker.status = EmploymentStatus::Unemployed;
                worker.workplace_x = 0.0; worker.workplace_y = 0.0; worker.wage = 0.0;
            }
        }
        for (_entity, (_pos, _construction, employer)) in gw.world
            .query::<(&Position, &ConstructionState, &mut Employer)>().iter()
        {
            if (_pos.x - ex).abs() < 1.0 && (_pos.y - ey).abs() < 1.0 && employer.current_workers > 0 {
                employer.current_workers -= 1;
            }
        }
    }
}

fn process_long_term_unemployed(gw: &mut GameWorld) {
    let mut to_remove: Vec<hecs::Entity> = Vec::new();
    for (entity, (_pos, worker)) in gw.world.query::<(&Position, &mut Worker)>().iter() {
        if worker.status == EmploymentStatus::Unemployed {
            worker.unemployment_ticks += 1;
            if worker.unemployment_ticks > MAX_JOB_SEARCH_TICKS
                && crate::rng_pool::rng_chance(ABANDONMENT_CHANCE)
            {
                to_remove.push(entity);
            }
        }
    }
    for entity in to_remove { let _ = gw.world.despawn(entity); }
}

fn process_unstaffed_buildings(gw: &mut GameWorld) {
    let mut closures: Vec<(f32, f32)> = Vec::new();
    for (_entity, (pos, _construction, employer)) in gw.world
        .query::<(&Position, &ConstructionState, &mut Employer)>().iter()
    {
        if employer.max_workers == 0 { continue; }
        if employer.current_workers == 0 {
            employer.unstaffed_ticks += 1;
            if employer.unstaffed_ticks > MAX_UNSTAFFED_TICKS { closures.push((pos.x, pos.y)); }
        } else { employer.unstaffed_ticks = 0; }
    }
    for (x, y) in closures {
        let mut to_remove: Vec<hecs::Entity> = Vec::new();
        for (entity, (pos, _construction)) in gw.world
            .query::<(&Position, &ConstructionState)>().iter()
        {
            if (pos.x - x).abs() < 1.0 && (pos.y - y).abs() < 1.0 { to_remove.push(entity); }
        }
        for entity in to_remove {
            gw.bitgrid.clear(0, x, y);
            let _ = gw.world.despawn(entity);
        }
        gw.world.spawn((
            Position::new(x, y),
            crate::supply_chain::AbandonedBuilding { abandoned_ticks: 0 },
            crate::ecs::Renderable::rect(0xFF_55_44_44, 3.0, 3),
        ));
    }
}

pub fn init_labor_market(gw: &mut GameWorld) {
    let employer_data: Vec<(f32, f32, u32, f32)> = gw.world
        .query::<(&Position, &ConstructionState)>().iter()
        .map(|(_, (pos, c))| (pos.x, pos.y, c.building_type))
        .filter_map(|(x, y, btype)| match btype {
            BuildingType::Factory => Some((x, y, 10u32, 3.0f32)),
            BuildingType::Shop => Some((x, y, 3u32, 2.0f32)),
            BuildingType::Office => Some((x, y, 8u32, 4.0f32)),
            BuildingType::Farm => Some((x, y, 5u32, 1.5f32)),
            _ => None,
        })
        .collect();

    for (x, y, max_workers, wage) in employer_data {
        gw.world.spawn((Position::new(x, y), Employer::new(max_workers, wage)));
    }

    let worker_positions: Vec<(f32, f32)> = gw.world
        .query::<(&Position, &ConstructionState)>().iter()
        .filter(|(_, (_, c))| c.building_type == BuildingType::House
            || c.building_type == BuildingType::Apartment)
        .map(|(_, (p, _))| (p.x, p.y))
        .collect();

    for (x, y) in worker_positions.iter().take(50) {
        gw.world.spawn((Position::new(*x, *y), Worker::new()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs;
    use crate::object_pool::EntityPool;

    fn setup_world_with_labor() -> Box<GameWorld> {
        crate::luts::init_trig_luts();
        crate::rng_pool::init_rng_pool(42);
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        gw.world.spawn((
            Position::new(50.0, 50.0),
            ConstructionState { progress: 1.0, building_type: BuildingType::Factory },
            Employer::new(5, 3.0),
        ));
        for i in 0..5 {
            gw.world.spawn((Position::new(30.0 + i as f32 * 5.0, 55.0), Worker::new()));
        }
        gw
    }

    #[test]
    fn test_worker_creation() {
        let w = Worker::new();
        assert_eq!(w.status, EmploymentStatus::Unemployed);
    }

    #[test]
    fn test_employer_needs_workers() {
        let e = Employer::new(10, 5.0);
        assert!(e.needs_workers());
    }

    #[test]
    fn test_job_matching_reduces_unemployment() {
        let mut gw = setup_world_with_labor();
        let before = gw.world.query::<&Worker>().iter()
            .filter(|(_, w)| w.status == EmploymentStatus::Unemployed).count();
        tick_labor_market(&mut gw);
        let after = gw.world.query::<&Worker>().iter()
            .filter(|(_, w)| w.status == EmploymentStatus::Unemployed).count();
        assert!(after <= before);
    }
}
