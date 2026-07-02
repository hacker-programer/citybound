// Persistence System v0.10 [FASE 7]
//
// Save/Load del estado del juego usando bincode (binario rápido).
//
// TÉCNICAS:
// [TA#4] Zero-copy serialization — bincode es binario directo
// [TA#17] Acceso unchecked en buffers validados
//
// SaveData solo guarda los datos críticos que no se reconstruyen en init.
// Los datos de terreno, LUTs, flow fields, etc. se regeneran al cargar.

use serde::{Serialize, Deserialize};

/// Datos serializables de una partida guardada
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SaveData {
    pub version: u32,
    pub sim_tick: u64,
    pub time_of_day: u16,
    pub finance_treasury: f32,
    pub finance_tax_rate_residential: f32,
    pub finance_tax_rate_commercial: f32,
    pub finance_tax_rate_industrial: f32,
    pub politics_approval: f32,
    /// Posiciones de edificios: (x, y, building_type, money, food, goods)
    pub buildings: Vec<BuildingSaveData>,
    /// Zonas pintadas: (x, y, zone_type, density)
    pub zones: Vec<ZoneSaveData>,
    /// Carriles: datos de congestión
    pub lane_congestion: Vec<f32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BuildingSaveData {
    pub x: f32,
    pub y: f32,
    pub building_type: u8, // 0=House, 1=Apartment, 2=Shop, 3=Office, 4=Factory, 5=Farm, 6=Hospital, 7=School, 8=Police
    pub money: f32,
    pub food: f32,
    pub goods: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ZoneSaveData {
    pub x: f32,
    pub y: f32,
    pub zone_type: u8,  // 0=Res, 1=Com, 2=Ind, 3=Agri, 4=Road, 5=Park
    pub density: u8,
}

const SAVE_VERSION: u32 = 1;

impl SaveData {
    /// Crea un SaveData desde el GameWorld actual
    pub fn from_world(gw: &crate::ecs::GameWorld) -> Self {
        let buildings: Vec<BuildingSaveData> = gw.world
            .query::<(&crate::ecs::Position, &crate::ecs::ConstructionState, &crate::ecs::ResourceStorage)>()
            .iter()
            .map(|(_, (pos, cs, rs))| BuildingSaveData {
                x: pos.x,
                y: pos.y,
                building_type: building_type_to_u8(cs.building_type),
                money: rs.money,
                food: rs.food,
                goods: rs.goods,
            })
            .collect();

        let zones: Vec<ZoneSaveData> = gw.world
            .query::<(&crate::ecs::Position, &crate::ecs::ZoneComponent)>()
            .iter()
            .filter(|(_, (_, z))| z.density > 0)
            .map(|(_, (pos, z))| ZoneSaveData {
                x: pos.x,
                y: pos.y,
                zone_type: zone_type_to_u8(z.zone_type),
                density: z.density,
            })
            .collect();

        let lane_congestion: Vec<f32> = gw.lane_manager.lanes.iter()
            .map(|l| l.congestion)
            .collect();

        SaveData {
            version: SAVE_VERSION,
            sim_tick: gw.sim_tick,
            time_of_day: gw.time_of_day,
            finance_treasury: gw.finance.treasury,
            finance_tax_rate_residential: gw.finance.tax_rate_residential,
            finance_tax_rate_commercial: gw.finance.tax_rate_commercial,
            finance_tax_rate_industrial: gw.finance.tax_rate_industrial,
            politics_approval: gw.politics.global_approval,
            buildings,
            zones,
            lane_congestion,
        }
    }

    /// Restaura datos al GameWorld
    pub fn restore_to(&self, gw: &mut crate::ecs::GameWorld) {
        gw.sim_tick = self.sim_tick;
        gw.time_of_day = self.time_of_day;
        gw.finance.treasury = self.finance_treasury;
        gw.finance.tax_rate_residential = self.finance_tax_rate_residential;
        gw.finance.tax_rate_commercial = self.finance_tax_rate_commercial;
        gw.finance.tax_rate_industrial = self.finance_tax_rate_industrial;
        gw.politics.global_approval = self.politics_approval;

        // Restaurar congestión de carriles
        for (i, &cong) in self.lane_congestion.iter().enumerate() {
            if i < gw.lane_manager.lanes.len() {
                gw.lane_manager.lanes[i].congestion = cong;
            }
        }

        // Restaurar zonas
        for zone_data in &self.zones {
            let ztype = u8_to_zone_type(zone_data.zone_type);
            let color = crate::render_cache::zone_color(ztype);
            gw.world.spawn((
                crate::ecs::Position::new(zone_data.x, zone_data.y),
                crate::ecs::Renderable::rect(color, 1.0, 1),
                crate::ecs::ZoneComponent { zone_type: ztype, density: zone_data.density },
            ));
        }

        // Restaurar edificios
        for bdata in &self.buildings {
            let btype = u8_to_building_type(bdata.building_type);
            let color = crate::render_cache::building_color(btype);
            gw.world.spawn((
                crate::ecs::Position::new(bdata.x, bdata.y),
                crate::ecs::Renderable::rect(color, 3.0, 3),
                crate::ecs::ConstructionState { progress: 1.0, building_type: btype },
                crate::ecs::ResourceStorage { money: bdata.money, food: bdata.food, goods: bdata.goods },
            ));
            gw.bitgrid.set(0, bdata.x, bdata.y);
        }
    }
}

/// Guarda partida a disco
pub fn save_game(data: &SaveData, path: &str) -> Result<(), String> {
    let encoded: Vec<u8> = bincode::serialize(data)
        .map_err(|e| format!("Error al serializar: {}", e))?;
    std::fs::write(path, &encoded)
        .map_err(|e| format!("Error al escribir archivo: {}", e))?;
    Ok(())
}

/// Carga partida de disco
pub fn load_game(path: &str) -> Result<SaveData, String> {
    let data = std::fs::read(path)
        .map_err(|e| format!("Error al leer archivo: {}", e))?;
    let save: SaveData = bincode::deserialize(&data)
        .map_err(|e| format!("Error al deserializar: {}", e))?;
    if save.version != SAVE_VERSION {
        return Err(format!("Versión incompatible: {} (esperada {})", save.version, SAVE_VERSION));
    }
    Ok(save)
}

// ---------------------------------------------------------------------------
// Conversiones
// ---------------------------------------------------------------------------

#[inline]
fn building_type_to_u8(bt: crate::ecs::BuildingType) -> u8 {
    match bt {
        crate::ecs::BuildingType::House => 0,
        crate::ecs::BuildingType::Apartment => 1,
        crate::ecs::BuildingType::Shop => 2,
        crate::ecs::BuildingType::Office => 3,
        crate::ecs::BuildingType::Factory => 4,
        crate::ecs::BuildingType::Farm => 5,
        crate::ecs::BuildingType::Hospital => 6,
        crate::ecs::BuildingType::School => 7,
        crate::ecs::BuildingType::Police => 8,
    }
}

#[inline]
fn u8_to_building_type(v: u8) -> crate::ecs::BuildingType {
    match v {
        0 => crate::ecs::BuildingType::House,
        1 => crate::ecs::BuildingType::Apartment,
        2 => crate::ecs::BuildingType::Shop,
        3 => crate::ecs::BuildingType::Office,
        4 => crate::ecs::BuildingType::Factory,
        5 => crate::ecs::BuildingType::Farm,
        6 => crate::ecs::BuildingType::Hospital,
        7 => crate::ecs::BuildingType::School,
        _ => crate::ecs::BuildingType::Police,
    }
}

#[inline]
fn zone_type_to_u8(zt: crate::ecs::ZoneType) -> u8 {
    match zt {
        crate::ecs::ZoneType::Residential => 0,
        crate::ecs::ZoneType::Commercial => 1,
        crate::ecs::ZoneType::Industrial => 2,
        crate::ecs::ZoneType::Agricultural => 3,
        crate::ecs::ZoneType::Road => 4,
        crate::ecs::ZoneType::Park => 5,
    }
}

#[inline]
fn u8_to_zone_type(v: u8) -> crate::ecs::ZoneType {
    match v {
        0 => crate::ecs::ZoneType::Residential,
        1 => crate::ecs::ZoneType::Commercial,
        2 => crate::ecs::ZoneType::Industrial,
        3 => crate::ecs::ZoneType::Agricultural,
        4 => crate::ecs::ZoneType::Road,
        _ => crate::ecs::ZoneType::Park,
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
    use tempfile::NamedTempFile;

    #[test]
    fn test_save_data_from_world() {
        crate::luts::init_trig_luts();
        crate::rng_pool::init_rng_pool(42);
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);
        let save = SaveData::from_world(&gw);

        assert_eq!(save.version, SAVE_VERSION);
        assert!(save.buildings.len() >= 8); // Edificios iniciales
        assert!(save.zones.len() > 100); // Zonas iniciales
        assert!(!save.lane_congestion.is_empty());
    }

    #[test]
    fn test_save_load_roundtrip() {
        crate::luts::init_trig_luts();
        crate::rng_pool::init_rng_pool(42);
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);
        let save = SaveData::from_world(&gw);

        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_str().unwrap();
        save_game(&save, path).unwrap();

        let loaded = load_game(path).unwrap();
        assert_eq!(loaded.sim_tick, save.sim_tick);
        assert_eq!(loaded.buildings.len(), save.buildings.len());
        assert_eq!(loaded.zones.len(), save.zones.len());
    }

    #[test]
    fn test_building_type_conversion() {
        let types = [
            ecs::BuildingType::House,
            ecs::BuildingType::Apartment,
            ecs::BuildingType::Shop,
            ecs::BuildingType::Office,
            ecs::BuildingType::Factory,
            ecs::BuildingType::Farm,
            ecs::BuildingType::Hospital,
            ecs::BuildingType::School,
            ecs::BuildingType::Police,
        ];
        for &bt in &types {
            let v = building_type_to_u8(bt);
            let restored = u8_to_building_type(v);
            assert_eq!(bt, restored);
        }
    }

    #[test]
    fn test_zone_type_conversion() {
        let types = [
            ecs::ZoneType::Residential,
            ecs::ZoneType::Commercial,
            ecs::ZoneType::Industrial,
            ecs::ZoneType::Agricultural,
            ecs::ZoneType::Road,
            ecs::ZoneType::Park,
        ];
        for &zt in &types {
            let v = zone_type_to_u8(zt);
            let restored = u8_to_zone_type(v);
            assert_eq!(zt, restored);
        }
    }

    #[test]
    fn test_restore_preserves_data() {
        crate::luts::init_trig_luts();
        crate::rng_pool::init_rng_pool(42);
        let mut pool = EntityPool::new(1000);
        let mut gw = ecs::create_world(&mut pool);
        gw.sim_tick = 999;
        gw.finance.treasury = 50000.0;

        let save = SaveData::from_world(&gw);

        // Reset world
        let mut pool2 = EntityPool::new(1000);
        let mut gw2 = ecs::create_world(&mut pool2);
        save.restore_to(&mut gw2);

        assert_eq!(gw2.sim_tick, 999);
        assert_eq!(gw2.finance.treasury, 50000.0);
    }
}
