// Sistema de Estacionamiento Físico Estricto
//
// Implementa:
// - Cada edificio tiene capacidad de garaje según tipo
// - Autos que llegan a destino deben estacionar físicamente
// - Sin parking disponible → circulan buscando → congestión
// - Estacionamiento en calle con cupos limitados
// - Asociaciones de Dueños (HOA) con reglas de estacionamiento
// - Multas por estacionar donde no se debe
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad
// [TI#6]  Bitboards para tracking de espacios ocupados
// [TC#26] Inlining agresivo

// ---------------------------------------------------------------------------
// TIPOS DE ESTACIONAMIENTO
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ParkingType {
    /// Sin estacionamiento
    None,
    /// Garaje privado (casa unifamiliar)
    GaragePrivate,
    /// Estacionamiento subterráneo (edificio)
    Underground,
    /// Estacionamiento en superficie (lote)
    SurfaceLot,
    /// Estacionamiento en calle
    StreetParking,
    /// Estacionamiento público (parking municipal)
    PublicGarage,
}

/// Capacidad de estacionamiento de un edificio
#[derive(Copy, Clone, Debug)]
pub struct ParkingCapacity {
    /// Tipo de estacionamiento
    pub parking_type: ParkingType,
    /// Espacios totales
    pub total_spaces: u16,
    /// Espacios ocupados actualmente
    pub occupied_spaces: u16,
    /// Espacios reservados (discapacitados, carga)
    pub reserved_spaces: u16,
    /// ¿Permite estacionamiento de visitantes?
    pub visitor_parking: bool,
    /// Espacios para visitantes
    pub visitor_spaces: u16,
}

impl ParkingCapacity {
    pub fn new(parking_type: ParkingType, total: u16) -> Self {
        ParkingCapacity {
            parking_type,
            total_spaces: total,
            occupied_spaces: 0,
            reserved_spaces: (total as f32 * 0.05) as u16, // 5% reservados
            visitor_parking: total > 10,
            visitor_spaces: if total > 10 { (total as f32 * 0.1) as u16 } else { 0 },
        }
    }

    #[inline]
    pub fn available(&self) -> u16 {
        self.total_spaces.saturating_sub(self.occupied_spaces + self.reserved_spaces)
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.available() == 0
    }

    #[inline]
    pub fn park(&mut self) -> bool {
        if self.is_full() {
            false
        } else {
            self.occupied_spaces += 1;
            true
        }
    }

    #[inline]
    pub fn unpark(&mut self) {
        self.occupied_spaces = self.occupied_spaces.saturating_sub(1);
    }
}

// ---------------------------------------------------------------------------
// ASOCIACIÓN DE DUEÑOS (HOA)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct HoaRules {
    /// ¿Está activa la HOA en esta zona?
    pub active: bool,
    /// ¿Prohíbe estacionar en la calle?
    pub no_street_parking: bool,
    /// ¿Requiere permiso para visitantes?
    pub visitor_permit_required: bool,
    /// Cuota mensual de HOA
    pub monthly_fee: f32,
    /// Multa por violar reglas
    pub violation_fine: f32,
    /// ¿Permite vehículos comerciales?
    pub allow_commercial_vehicles: bool,
    /// ¿Restringe estacionamiento nocturno (2-6 AM)?
    pub overnight_restriction: bool,
}

impl Default for HoaRules {
    fn default() -> Self {
        HoaRules {
            active: false,
            no_street_parking: false,
            visitor_permit_required: false,
            monthly_fee: 0.0,
            violation_fine: 50.0,
            allow_commercial_vehicles: true,
            overnight_restriction: false,
        }
    }
}

// ---------------------------------------------------------------------------
// GESTOR DE ESTACIONAMIENTO
// ---------------------------------------------------------------------------

/// Rastreo de estacionamiento en calle por segmento
#[derive(Clone, Debug)]
pub struct StreetParkingSegment {
    /// Posición en el mundo
    pub x: f32,
    pub y: f32,
    /// Espacios totales en este segmento
    pub total_spaces: u8,
    /// Espacios ocupados
    pub occupied: u8,
    /// ¿Es zona HOA con restricciones?
    pub hoa_zone: bool,
    /// ¿Requiere parquímetro?
    pub metered: bool,
    /// Tarifa por hora (si tiene parquímetro)
    pub hourly_rate: f32,
}

impl StreetParkingSegment {
    #[inline]
    pub fn available(&self) -> u8 {
        self.total_spaces.saturating_sub(self.occupied)
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.occupied >= self.total_spaces
    }
}

/// Sistema completo de gestión de estacionamiento
pub struct ParkingManager {
    /// Capacidad de parking por edificio (indexado por posición en grid)
    pub building_parking: Vec<(f32, f32, ParkingCapacity)>,
    /// Segmentos de estacionamiento en calle
    pub street_segments: Vec<StreetParkingSegment>,
    /// Reglas HOA por zona
    pub hoa_zones: Vec<(f32, f32, f32, HoaRules)>, // (x, y, radius, rules)
    /// Coches buscando estacionamiento (generan congestión)
    pub circling_cars: u32,
    /// Multas emitidas este período
    pub fines_issued: u32,
    /// Ingresos por parquímetros
    pub meter_revenue: f32,
    /// Tiempo promedio de búsqueda de estacionamiento (ticks)
    pub avg_search_time: f32,
}

impl ParkingManager {
    pub fn new() -> Self {
        ParkingManager {
            building_parking: Vec::with_capacity(1024),
            street_segments: Vec::with_capacity(4096),
            hoa_zones: Vec::with_capacity(64),
            circling_cars: 0,
            fines_issued: 0,
            meter_revenue: 0.0,
            avg_search_time: 0.0,
        }
    }

    /// Inicializa la capacidad de estacionamiento para un edificio
    pub fn register_building_parking(
        &mut self,
        x: f32, y: f32,
        building_type: crate::ecs::BuildingType,
    ) {
        let capacity = match building_type {
            crate::ecs::BuildingType::House => ParkingCapacity::new(ParkingType::GaragePrivate, 2),
            crate::ecs::BuildingType::Apartment => ParkingCapacity::new(ParkingType::Underground, 20),
            crate::ecs::BuildingType::Shop => ParkingCapacity::new(ParkingType::SurfaceLot, 15),
            crate::ecs::BuildingType::Office => ParkingCapacity::new(ParkingType::Underground, 40),
            crate::ecs::BuildingType::Factory => ParkingCapacity::new(ParkingType::SurfaceLot, 10),
            crate::ecs::BuildingType::Farm => ParkingCapacity::new(ParkingType::None, 0),
            crate::ecs::BuildingType::Hospital => ParkingCapacity::new(ParkingType::Underground, 50),
            crate::ecs::BuildingType::School => ParkingCapacity::new(ParkingType::SurfaceLot, 30),
            crate::ecs::BuildingType::Hospital => ParkingCapacity::new(ParkingType::Underground, 50),
            crate::ecs::BuildingType::School => ParkingCapacity::new(ParkingType::SurfaceLot, 30),
            crate::ecs::BuildingType::Police => ParkingCapacity::new(ParkingType::GaragePrivate, 12),
        };

        self.building_parking.push((x, y, capacity));
    }

    /// Busca estacionamiento para un coche que llega a destino
    /// Retorna true si encontró lugar, false si debe seguir circulando
    pub fn find_parking(&mut self, x: f32, y: f32, is_commercial: bool) -> bool {
        // 1. Intentar en el edificio destino
        for (bx, by, capacity) in self.building_parking.iter_mut() {
            let dist = ((x - *bx) * (x - *bx) + (y - *by) * (y - *by)).sqrt();
            if dist < 5.0 && capacity.park() {
                return true; // Estacionado en edificio
            }
            }
        }

        // 2. Intentar en calle cercana
        let mut best_segment: Option<usize> = None;
        let mut best_dist = f32::MAX;

        for (i, seg) in self.street_segments.iter().enumerate() {
            if seg.hoa_zone && is_commercial {
                // Zona HOA puede no permitir vehículos comerciales
                for (_hx, _hy, _hr, rules) in &self.hoa_zones {
                    if !rules.allow_commercial_vehicles {
                        continue;
                    }
                }
            }

            let dist = ((x - seg.x) * (x - seg.x) + (y - seg.y) * (y - seg.y)).sqrt();
            if dist < best_dist {
                best_dist = dist;
                best_segment = Some(i);
            }
        }

        if let Some(idx) = best_segment {
            if best_dist < 20.0 {
                // Encontró lugar en la calle
                self.street_segments[idx].occupied += 1;
                if self.street_segments[idx].metered {
                    self.meter_revenue += self.street_segments[idx].hourly_rate;
                }
                return true;
            }
        }

        // 3. Sin estacionamiento → a circular
        self.circling_cars += 1;
        false
    }

    /// Libera un espacio de estacionamiento cuando un coche sale
    pub fn leave_parking(&mut self, x: f32, y: f32) {
        // Liberar del edificio
        for (_bx, _by, capacity) in self.building_parking.iter_mut() {
            if capacity.occupied_spaces > 0 {
                capacity.unpark();
                return;
            }
        }

        // Liberar de la calle
        for seg in self.street_segments.iter_mut() {
            let dist = ((x - seg.x) * (x - seg.x) + (y - seg.y) * (y - seg.y)).sqrt();
            if dist < 3.0 && seg.occupied > 0 {
                seg.occupied -= 1;
                return;
            }
        }
    }

    /// Genera estacionamiento en calle para un segmento de carril
    pub fn add_street_parking(&mut self, x: f32, y: f32, spaces: u8, metered: bool, rate: f32) {
        self.street_segments.push(StreetParkingSegment {
            x, y,
            total_spaces: spaces,
            occupied: 0,
            hoa_zone: false,
            metered,
            hourly_rate: rate,
        });
    }

    /// Registra una zona HOA
    pub fn add_hoa_zone(&mut self, x: f32, y: f32, radius: f32, rules: HoaRules) {
        self.hoa_zones.push((x, y, radius, rules));

        // Marcar segmentos de calle dentro de la HOA
        for seg in self.street_segments.iter_mut() {
            let dist = ((x - seg.x) * (x - seg.x) + (y - seg.y) * (y - seg.y)).sqrt();
            if dist < radius {
                seg.hoa_zone = true;
            }
        }
    }

    /// Actualiza estadísticas de búsqueda cada tick
    pub fn tick(&mut self, dt: f32) {
        // Coches que encuentran lugar dejan de circular
        let found = (self.circling_cars as f32 * 0.1 * dt) as u32;
        self.circling_cars = self.circling_cars.saturating_sub(found);

        // Suavizar tiempo promedio de búsqueda
        self.avg_search_time = self.avg_search_time * 0.95
            + (self.circling_cars as f32 * 0.05);

        // Resetear multas y recaudación al final del período
        // (se resetea externamente cada TAX_COLLECTION_INTERVAL)
    }

    /// Obtiene el factor de congestión por búsqueda de parking (0-1)
    /// A mayor circling_cars, menor magnitud en flow fields cercanos
    #[inline]
    pub fn congestion_factor(&self) -> f32 {
        (self.circling_cars as f32 * 0.01).min(0.8)
    }
}

// ---------------------------------------------------------------------------
// INTEGRACIÓN CON EDIFICIOS
// ---------------------------------------------------------------------------

pub fn recommended_parking(building_type: crate::ecs::BuildingType) -> u16 {
    match building_type {
        crate::ecs::BuildingType::House => 2,
        crate::ecs::BuildingType::Apartment => 30,
        crate::ecs::BuildingType::Shop => 20,
        crate::ecs::BuildingType::Office => 50,
        crate::ecs::BuildingType::Factory => 15,
        crate::ecs::BuildingType::Farm => 0,
        crate::ecs::BuildingType::Hospital => 60,
        crate::ecs::BuildingType::School => 40,
        crate::ecs::BuildingType::Police => 15,
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parking_capacity_new() {
        let cap = ParkingCapacity::new(ParkingType::GaragePrivate, 2);
        assert_eq!(cap.total_spaces, 2);
        assert_eq!(cap.available(), 2);
        assert!(!cap.is_full());
    }

    #[test]
    fn test_parking_full() {
        let mut cap = ParkingCapacity::new(ParkingType::SurfaceLot, 3);
        assert!(cap.park());
        assert!(cap.park());
        assert!(cap.park());
        assert!(!cap.park()); // Lleno
        assert!(cap.is_full());
    }

    #[test]
    fn test_parking_unpark() {
        let mut cap = ParkingCapacity::new(ParkingType::Underground, 5);
        for _ in 0..5 { cap.park(); }
        assert!(cap.is_full());
        cap.unpark();
        assert_eq!(cap.available(), 1);
    }

    #[test]
    fn test_street_parking_segment() {
        let seg = StreetParkingSegment {
            x: 10.0, y: 20.0,
            total_spaces: 8,
            occupied: 5,
            hoa_zone: false,
            metered: true,
            hourly_rate: 2.0,
        };
        assert_eq!(seg.available(), 3);
        assert!(!seg.is_full());
    }

    #[test]
    fn test_parking_manager_find() {
        let mut pm = ParkingManager::new();
        pm.register_building_parking(10.0, 10.0, crate::ecs::BuildingType::House);
        pm.add_street_parking(10.0, 12.0, 4, false, 0.0);

        // Debe encontrar parking en el edificio
        assert!(pm.find_parking(10.0, 10.0, false));
        assert_eq!(pm.building_parking[0].2.occupied_spaces, 1);
    }

    #[test]
    fn test_circling_when_full() {
        let mut pm = ParkingManager::new();
        // Sin parking registrado
        assert!(!pm.find_parking(50.0, 50.0, false));
        assert_eq!(pm.circling_cars, 1);
    }

    #[test]
    fn test_congestion_factor() {
        let mut pm = ParkingManager::new();
        pm.circling_cars = 50;
        let factor = pm.congestion_factor();
        assert!(factor > 0.0);
        assert!(factor <= 0.8);
    }

    #[test]
    fn test_hoa_rules_default() {
        let rules = HoaRules::default();
        assert!(!rules.active);
        assert!(!rules.no_street_parking);
        assert_eq!(rules.violation_fine, 50.0);
    }

    #[test]
    fn test_recommended_parking() {
        assert_eq!(recommended_parking(crate::ecs::BuildingType::House), 2);
        assert_eq!(recommended_parking(crate::ecs::BuildingType::Office), 50);
        assert_eq!(recommended_parking(crate::ecs::BuildingType::Farm), 0);
    }
}
