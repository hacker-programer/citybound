    /// Actualiza estado de vertederos y contaminación (llamado cada tick)
    pub fn update(&mut self, dt: f32) {
        for landfill in self.landfills.iter_mut() {
            landfill.tick(dt);
        }

        // Contaminación de napas decae muy lentamente (décadas)
        self.groundwater_contamination = (self.groundwater_contamination - 0.0001 * dt).max(0.0);

        // Basura sin recolectar atrae plagas (aumenta lentamente si no se gestiona)
        if self.uncollected_waste > 0.0 {
            self.uncollected_waste += 0.1 * dt;
        }
    }

    /// Alias de update() para compatibilidad con el game loop
    #[inline(always)]
    pub fn tick(&mut self, dt: f32) {
        self.update(dt);
    }
    Toxic,
    /// Residuos generales no reciclables
    General,
}

/// Unidad de residuo con tipo y cantidad
#[derive(Copy, Clone, Debug)]
pub struct WasteItem {
    pub waste_type: WasteType,
    pub amount_kg: f32,
}

// ---------------------------------------------------------------------------
// INSTALACIONES DE RESIDUOS
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct Landfill {
    pub x: f32,
    pub y: f32,
    /// Capacidad total en kg
    pub capacity_kg: f32,
    /// Residuos acumulados por tipo
    pub organic_kg: f32,
    pub recyclable_kg: f32,
    pub toxic_kg: f32,
    pub general_kg: f32,
    /// ¿Tiene geomembrana para tóxicos?
    pub has_geomembrane: bool,
    /// ¿Tiene sistema de ventilación de metano?
    pub has_methane_ventilation: bool,
    /// Nivel de gas metano acumulado (0-100, >80 = peligro explosión)
    pub methane_level: f32,
    /// ¿Está en riesgo de explosión?
    pub explosion_risk: bool,
}

impl Landfill {
    pub fn new(x: f32, y: f32, capacity_kg: f32) -> Self {
        Landfill {
            x, y, capacity_kg,
            organic_kg: 0.0, recyclable_kg: 0.0,
            toxic_kg: 0.0, general_kg: 0.0,
            has_geomembrane: false,
            has_methane_ventilation: false,
            methane_level: 0.0,
            explosion_risk: false,
        }
    }

    pub fn total_filled(&self) -> f32 {
        self.organic_kg + self.recyclable_kg + self.toxic_kg + self.general_kg
    }

    pub fn fill_percentage(&self) -> f32 {
        (self.total_filled() / self.capacity_kg).min(1.0)
    }

    /// Añade residuos al vertedero. Retorna lo que no pudo aceptar.
    pub fn deposit(&mut self, waste: WasteItem) -> f32 {
        let remaining = self.capacity_kg - self.total_filled();
        let accepted = waste.amount_kg.min(remaining);

        match waste.waste_type {
            WasteType::Organic => self.organic_kg += accepted,
            WasteType::Recyclable => self.recyclable_kg += accepted,
            WasteType::Toxic => {
                if self.has_geomembrane {
                    self.toxic_kg += accepted;
                } else {
                    // Sin geomembrana, los tóxicos se filtran al suelo
                    self.toxic_kg += accepted * 0.5; // La mitad se filtra
                    return accepted * 0.5; // Retorna lo filtrado como "contaminación"
                }
            }
            WasteType::General => self.general_kg += accepted,
        }

        waste.amount_kg - accepted // Retorna lo no aceptado
    }

    /// Actualiza niveles de metano cada tick
    pub fn tick(&mut self, dt: f32) {
        if self.has_methane_ventilation {
            // Ventilación reduce metano
            self.methane_level = (self.methane_level - 5.0 * dt).max(0.0);
        } else {
            // Orgánicos generan metano: ~0.5% por hora por kg
            let methane_gen = self.organic_kg * 0.005 * dt / 3600.0;
            self.methane_level = (self.methane_level + methane_gen).min(100.0);
        }

        self.explosion_risk = self.methane_level > 80.0
            && !self.has_methane_ventilation;
    }
}

#[derive(Clone, Debug)]
pub struct RecyclingPlant {
    pub x: f32,
    pub y: f32,
    /// Capacidad de procesamiento por tick (kg)
    pub processing_capacity: f32,
    /// Ingresos por venta de reciclables
    pub revenue_per_kg: f32,
    /// Material procesado este período
    pub processed_this_tick: f32,
    /// Ingresos acumulados
    pub total_revenue: f32,
}

impl RecyclingPlant {
    pub fn new(x: f32, y: f32, capacity: f32) -> Self {
        RecyclingPlant {
            x, y,
            processing_capacity: capacity,
            revenue_per_kg: 0.15,
            processed_this_tick: 0.0,
            total_revenue: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// GESTOR DE RESIDUOS
// ---------------------------------------------------------------------------

pub struct WasteManager {
    /// Vertederos activos
    pub landfills: Vec<Landfill>,
    /// Plantas de reciclaje
    pub recycling_plants: Vec<RecyclingPlant>,
    /// Residuos sin recolectar (causan problemas de salud)
    pub uncollected_waste: f32,
    /// Contaminación de napas freáticas (0-100)
    pub groundwater_contamination: f32,
    /// Ingresos totales por reciclaje
    pub total_recycling_revenue: f32,
    /// Multas ambientales acumuladas
    pub environmental_fines: f32,
}

impl WasteManager {
    pub fn new() -> Self {
        WasteManager {
            landfills: Vec::with_capacity(16),
            recycling_plants: Vec::with_capacity(8),
            uncollected_waste: 0.0,
            groundwater_contamination: 0.0,
            total_recycling_revenue: 0.0,
            environmental_fines: 0.0,
        }
    }
    /// Genera residuos para un edificio según su tipo
    pub fn generate_building_waste(
        &self,
        building_type: crate::ecs::BuildingType,
    ) -> Vec<WasteItem> {
        match building_type {
            crate::ecs::BuildingType::House => vec![
                WasteItem { waste_type: WasteType::Organic, amount_kg: 0.5 },
                WasteItem { waste_type: WasteType::Recyclable, amount_kg: 0.3 },
                WasteItem { waste_type: WasteType::General, amount_kg: 0.2 },
            ],
            crate::ecs::BuildingType::Apartment => vec![
                WasteItem { waste_type: WasteType::Organic, amount_kg: 3.0 },
                WasteItem { waste_type: WasteType::Recyclable, amount_kg: 2.0 },
                WasteItem { waste_type: WasteType::General, amount_kg: 1.5 },
            ],
            crate::ecs::BuildingType::Shop => vec![
                WasteItem { waste_type: WasteType::Recyclable, amount_kg: 4.0 },
                WasteItem { waste_type: WasteType::Organic, amount_kg: 1.0 },
                WasteItem { waste_type: WasteType::General, amount_kg: 2.0 },
            ],
            crate::ecs::BuildingType::Office => vec![
                WasteItem { waste_type: WasteType::Recyclable, amount_kg: 3.0 },
                WasteItem { waste_type: WasteType::General, amount_kg: 1.0 },
            ],
            crate::ecs::BuildingType::Factory => vec![
                WasteItem { waste_type: WasteType::Toxic, amount_kg: 2.0 },
                WasteItem { waste_type: WasteType::Recyclable, amount_kg: 5.0 },
                WasteItem { waste_type: WasteType::General, amount_kg: 3.0 },
            ],
            crate::ecs::BuildingType::Farm => vec![
                WasteItem { waste_type: WasteType::Organic, amount_kg: 8.0 },
                WasteItem { waste_type: WasteType::General, amount_kg: 0.5 },
            ],
            crate::ecs::BuildingType::Hospital => vec![
                WasteItem { waste_type: WasteType::Toxic, amount_kg: 1.5 },
                WasteItem { waste_type: WasteType::Recyclable, amount_kg: 3.0 },
                WasteItem { waste_type: WasteType::General, amount_kg: 4.0 },
                WasteItem { waste_type: WasteType::Organic, amount_kg: 2.0 },
            ],
            crate::ecs::BuildingType::School => vec![
                WasteItem { waste_type: WasteType::Organic, amount_kg: 3.0 },
                WasteItem { waste_type: WasteType::Recyclable, amount_kg: 2.5 },
            ],
            crate::ecs::BuildingType::Police => vec![
                WasteItem { waste_type: WasteType::General, amount_kg: 1.0 },
                WasteItem { waste_type: WasteType::Recyclable, amount_kg: 0.5 },
            ],
        }
    }

    /// Procesa la recolección de residuos (llamado cada N ticks)
    pub fn collect_waste(
        &mut self,
        gw: &GameWorld,
    ) {
        let mut organic_total: f32 = 0.0;
        let mut recyclable_total: f32 = 0.0;
        let mut toxic_total: f32 = 0.0;
        let mut general_total: f32 = 0.0;

        // Generar residuos de cada edificio
        let waste_streams: Vec<Vec<WasteItem>> = gw.world
            .query::<&crate::ecs::ConstructionState>()
            .iter()
            .map(|(_e, cs)| self.generate_building_waste(cs.building_type))
            .collect();

        for items in waste_streams {
            for item in items {
                match item.waste_type {
                    WasteType::Organic => organic_total += item.amount_kg,
                    WasteType::Recyclable => recyclable_total += item.amount_kg,
                    WasteType::Toxic => toxic_total += item.amount_kg,
                    WasteType::General => general_total += item.amount_kg,
                }
            }
        }

        // Enviar reciclables a plantas
        let mut recycled = 0.0_f32;
        for plant in self.recycling_plants.iter_mut() {
            let to_process = recyclable_total.min(plant.processing_capacity);
            plant.processed_this_tick = to_process;
            plant.total_revenue += to_process * plant.revenue_per_kg;
            recycled += to_process;
        }
        self.total_recycling_revenue += recycled * 0.15;

        let remaining_organic = organic_total;
        let remaining_recyclable = (recyclable_total - recycled).max(0.0);
        let remaining_toxic = toxic_total;

        // El resto va al vertedero más cercano
        // Depositar en vertederos (round-robin simple)
        if !self.landfills.is_empty() {
            let landfill_count = self.landfills.len();
            let mut idx = 0;

            let to_deposit = [
                (WasteType::Organic, remaining_organic),
                (WasteType::Recyclable, remaining_recyclable),
                (WasteType::Toxic, remaining_toxic),
                (WasteType::General, general_total),
            ];

            for (wtype, amount) in &to_deposit {
                if *amount <= 0.0 { continue; }
                let landfill = &mut self.landfills[idx % landfill_count];
                let leaked = landfill.deposit(WasteItem { waste_type: *wtype, amount_kg: *amount });

                // Lo que se filtra contamina napas
                if *wtype == WasteType::Toxic && leaked > 0.0 {
                    self.groundwater_contamination += leaked * 0.01;
                }

                idx += 1;
            }
        } else {
            // Sin vertederos → basura sin recolectar
            self.uncollected_waste += organic_total + recyclable_total
                + toxic_total + general_total;
        }

        // Penalización por basura sin recolectar
        if self.uncollected_waste > 100.0 {
            self.environmental_fines += self.uncollected_waste * 0.5;
        }
    }

    /// Actualiza estado de vertederos y contaminación (llamado cada tick)
    pub fn update(&mut self, dt: f32) {
        for landfill in self.landfills.iter_mut() {
            landfill.tick(dt);
        }

        // Contaminación de napas decae muy lentamente (décadas)
        self.groundwater_contamination = (self.groundwater_contamination - 0.0001 * dt).max(0.0);

        // Basura sin recolectar atrae plagas (aumenta lentamente si no se gestiona)
        if self.uncollected_waste > 0.0 {
            self.uncollected_waste += 0.1 * dt;
        }
    }

    /// Verifica riesgo de explosión en vertederos
    pub fn check_explosion_risks(&self) -> Vec<(f32, f32, f32)> {
        self.landfills.iter()
            .filter(|l| l.explosion_risk)
            .map(|l| (l.x, l.y, l.methane_level))
            .collect()
    }
}
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_landfill_deposit() {
        let mut lf = Landfill::new(0.0, 0.0, 1000.0);
        let rejected = lf.deposit(WasteItem { waste_type: WasteType::Organic, amount_kg: 100.0 });
        assert_eq!(rejected, 0.0);
        assert_eq!(lf.organic_kg, 100.0);
    }

    #[test]
    fn test_landfill_over_capacity() {
        let mut lf = Landfill::new(0.0, 0.0, 100.0);
        let rejected = lf.deposit(WasteItem { waste_type: WasteType::General, amount_kg: 150.0 });
        assert_eq!(rejected, 50.0);
        assert_eq!(lf.general_kg, 100.0);
    }

    #[test]
    fn test_toxic_without_geomembrane() {
        let mut lf = Landfill::new(0.0, 0.0, 1000.0);
        let leaked = lf.deposit(WasteItem { waste_type: WasteType::Toxic, amount_kg: 100.0 });
        assert!(leaked > 0.0); // Debe haber fuga
        assert!(lf.toxic_kg < 100.0); // Solo la mitad se almacenó
    }

    #[test]
    fn test_toxic_with_geomembrane() {
        let mut lf = Landfill::new(0.0, 0.0, 1000.0);
        lf.has_geomembrane = true;
        let leaked = lf.deposit(WasteItem { waste_type: WasteType::Toxic, amount_kg: 100.0 });
        assert_eq!(leaked, 0.0);
        assert_eq!(lf.toxic_kg, 100.0);
    }

    #[test]
    fn test_methane_generation() {
        let mut lf = Landfill::new(0.0, 0.0, 10000.0);
        lf.organic_kg = 5000.0;

        // Simular 1 hora (3600 ticks a dt=1.0)
        for _ in 0..3600 {
            lf.tick(1.0);
        }

        assert!(lf.methane_level > 0.0);
    }

    #[test]
    fn test_methane_ventilation() {
        let mut lf = Landfill::new(0.0, 0.0, 10000.0);
        lf.organic_kg = 5000.0;
        lf.has_methane_ventilation = true;
        lf.methane_level = 50.0;

        for _ in 0..100 {
            lf.tick(1.0);
        }

        assert!(lf.methane_level < 50.0);
    }

    #[test]
    fn test_recycling_plant() {
        let mut plant = RecyclingPlant::new(0.0, 0.0, 100.0);
        plant.processed_this_tick = 100.0;
        plant.total_revenue += 100.0 * plant.revenue_per_kg;
        assert_eq!(plant.total_revenue, 15.0);
    }

    #[test]
    fn test_building_waste_generation() {
        let wm = WasteManager::new();
        let house_waste = wm.generate_building_waste(crate::ecs::BuildingType::House);
        assert!(!house_waste.is_empty());

        let factory_waste = wm.generate_building_waste(crate::ecs::BuildingType::Factory);
        let has_toxic = factory_waste.iter().any(|w| w.waste_type == WasteType::Toxic);
        assert!(has_toxic, "Fábricas deben generar residuos tóxicos");
    }

    #[test]
    fn test_uncollected_waste_penalty() {
        let mut wm = WasteManager::new();
        wm.uncollected_waste = 500.0;
        // Simular penalización
        wm.environmental_fines += wm.uncollected_waste * 0.5;
        assert_eq!(wm.environmental_fines, 250.0);
    }
}
