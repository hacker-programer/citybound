// RenderCache v0.10 — Pre-sort estático de entidades por capa [FASE 7]
//
// En vez de ordenar/sortear entidades cada frame, las entidades se
// insertan en el bucket de capa correcto al crearse.
// El render solo itera los buckets en orden (0..=5), sin sort.
//
// FASE 7: 
// - rebuild_from_world para sincronizar con ECS
// - Colores para nuevos edificios (Hospital, Escuela, Policía)
// - Integración completa con pipeline de render

use crate::ecs::{BuildingType, ZoneType, Position, Renderable, ZoneComponent};

/// Capas de renderizado (orden back-to-front)
pub const LAYER_TERRAIN: u8 = 0;
pub const LAYER_ZONES: u8 = 1;
pub const LAYER_BUILDINGS: u8 = 2;
pub const LAYER_CONSTRUCTION: u8 = 3;
pub const LAYER_TRAFFIC: u8 = 4;
pub const LAYER_UI: u8 = 5;
pub const NUM_RENDER_LAYERS: usize = 6;

/// Entrada en el cache de render — datos mínimos para dibujar
/// Entrada en el cache de render — datos mínimos para dibujar
#[derive(Copy, Clone, Debug)]
pub struct RenderCacheEntry {
    pub world_x: f32,
    pub world_y: f32,
    pub shape_type: u8,
    pub color: u32,
    pub size_x: f32,
    pub layer: u8,
}

impl RenderCacheEntry {
    #[inline(always)]
    pub fn new(x: f32, y: f32, shape: u8, color: u32, size_x: f32, layer: u8) -> Self {
        RenderCacheEntry { world_x: x, world_y: y, shape_type: shape, color, size_x, layer }
    }
}
/// Cache de render con buckets pre-ordenados por capa.
pub struct RenderCache {
    pub buckets: [Vec<RenderCacheEntry>; NUM_RENDER_LAYERS],
    pub dirty: bool,
}
impl RenderCache {
    pub fn new() -> Self {
        let buckets: [Vec<RenderCacheEntry>; NUM_RENDER_LAYERS] = 
            std::array::from_fn(|_| Vec::with_capacity(4096));
        RenderCache { buckets, dirty: true }
    }

    pub fn clear(&mut self) {
        for bucket in self.buckets.iter_mut() {
            bucket.clear();
        }
    }

    #[inline(always)]
    pub fn push(&mut self, entry: RenderCacheEntry) {
        let layer = entry.layer.min(NUM_RENDER_LAYERS as u8 - 1) as usize;
        unsafe {
            self.buckets.get_unchecked_mut(layer).push(entry);
        }
    }

    pub fn total_entries(&self) -> usize {
        self.buckets.iter().map(|b| b.len()).sum()
    }

    /// [FASE 7]: Reconstruye el cache desde el mundo ECS
    pub fn rebuild_from_world(&mut self, world: &hecs::World) {
        self.clear();

        // Zonas (capa 1)
        for (_entity, (pos, zone)) in world.query::<(&Position, &ZoneComponent)>().iter() {
            if zone.density > 0 {
                self.push(RenderCacheEntry::new(
                    pos.x, pos.y, 0, // rect (shape_type 0)
                    zone_color(zone.zone_type),
                    1.0, LAYER_ZONES,
                ));
            }
        }

        // Edificios y construcciones (capa 2-3)
        for (_entity, (pos, renderable)) in world.query::<(&Position, &Renderable)>().iter() {
            if renderable.layer >= 2 && renderable.layer <= 3 {
                self.push(RenderCacheEntry::new(
                    pos.x, pos.y, renderable.shape_type,
                    renderable.color, renderable.size_x, renderable.layer as u8,
                ));
            }
        }

        // Tráfico (capa 4)
        for (_entity, (pos, renderable)) in world.query::<(&Position, &Renderable)>().iter() {
            if renderable.layer >= 4 {
                self.push(RenderCacheEntry::new(
                    pos.x, pos.y, renderable.shape_type,
                    renderable.color, renderable.size_x, renderable.layer as u8,
                ));
            }
        }
        self.dirty = false;
    }

    #[inline]
    pub fn iter_layers(&self) -> RenderCacheIter<'_> {
        RenderCacheIter {
            cache: self,
            current_layer: 0,
            current_idx: 0,
        }
    }
}
pub struct RenderCacheIter<'a> {
    cache: &'a RenderCache,
    current_layer: usize,
    current_idx: usize,
}

impl<'a> Iterator for RenderCacheIter<'a> {
    type Item = &'a RenderCacheEntry;

    fn next(&mut self) -> Option<&'a RenderCacheEntry> {
        loop {
            if self.current_layer >= NUM_RENDER_LAYERS {
                return None;
            }
            let bucket = &self.cache.buckets[self.current_layer];
            if self.current_idx < bucket.len() {
                let entry = &bucket[self.current_idx];
                self.current_idx += 1;
                return Some(entry);
            }
            self.current_layer += 1;
            self.current_idx = 0;
        }
    }
}

// ---------------------------------------------------------------------------
// COLORES POR TIPO (centralizados para consistencia)
// ---------------------------------------------------------------------------

#[inline(always)]
pub fn building_color(btype: BuildingType) -> u32 {
    match btype {
        BuildingType::House => 0xFF_C4_7B_4A,
        BuildingType::Apartment => 0xFF_B0_BEC5,
        BuildingType::Shop => 0xFF_26_C6_DA,
        BuildingType::Office => 0xFF_78_90_9C,
        BuildingType::Factory => 0xFF_8D_6E_63,
        BuildingType::Farm => 0xFF_8B_C3_4A,
        // [FASE 7]: Edificios públicos
        BuildingType::Hospital => 0xFF_F4_81_81,   // Rojo claro (cruz roja)
        BuildingType::School => 0xFF_FF_D5_4F,     // Amarillo
        BuildingType::Police => 0xFF_42_45_E8,     // Azul policía
    }
}

#[inline(always)]
pub fn zone_color(ztype: ZoneType) -> u32 {
    match ztype {
        ZoneType::Residential => 0x44_66_BB_6A,
        ZoneType::Commercial => 0x44_42_A5_F5,
        ZoneType::Industrial => 0x44_EF_5350,
        ZoneType::Agricultural => 0x44_9C_CC_65,
        ZoneType::Road => 0x44_55_55_55,
        ZoneType::Park => 0x44_4C_AF_50,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs;
    use crate::object_pool::EntityPool;

    #[test]
    fn test_render_cache_push_and_iter() {
        let mut cache = RenderCache::new();

        cache.push(RenderCacheEntry::new(10.0, 10.0, 0, 0xFF_FF_00_00, 2.0, LAYER_TRAFFIC));
        cache.push(RenderCacheEntry::new(20.0, 20.0, 1, 0xFF_00_FF_00, 3.0, LAYER_BUILDINGS));
        cache.push(RenderCacheEntry::new(30.0, 30.0, 1, 0xFF_00_00_FF, 1.0, LAYER_ZONES));

        let all: Vec<&RenderCacheEntry> = cache.iter_layers().collect();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].layer, LAYER_ZONES);
        assert_eq!(all[1].layer, LAYER_BUILDINGS);
        assert_eq!(all[2].layer, LAYER_TRAFFIC);
    }

    #[test]
    fn test_render_cache_clear() {
        let mut cache = RenderCache::new();
        cache.push(RenderCacheEntry::new(0.0, 0.0, 0, 0, 1.0, 0));
        assert_eq!(cache.total_entries(), 1);
        cache.clear();
        assert_eq!(cache.total_entries(), 0);
    }

    #[test]
    fn test_render_cache_capacity() {
        let mut cache = RenderCache::new();
        for i in 0..1000 {
            cache.push(RenderCacheEntry::new(i as f32, 0.0, 0, 0xFFFFFFFF, 1.0, (i % 6) as u8));
        }
        assert_eq!(cache.total_entries(), 1000);
    }

    #[test]
    fn test_layer_clamping() {
        let mut cache = RenderCache::new();
        cache.push(RenderCacheEntry::new(0.0, 0.0, 0, 0, 1.0, 255));
        let all: Vec<&RenderCacheEntry> = cache.iter_layers().collect();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].layer, NUM_RENDER_LAYERS as u8 - 1);
    }

    #[test]
    fn test_building_colors() {
        let types = [
            BuildingType::House, BuildingType::Apartment, BuildingType::Shop,
            BuildingType::Office, BuildingType::Factory, BuildingType::Farm,
            BuildingType::Hospital, BuildingType::School, BuildingType::Police,
        ];
        for t in &types {
            let c = building_color(*t);
            assert_eq!((c >> 24) & 0xFF, 0xFF, "Alpha must be 0xFF for {:?}", t);
        }
    }

    #[test]
    fn test_zone_colors() {
        let types = [
            ZoneType::Residential, ZoneType::Commercial, ZoneType::Industrial,
            ZoneType::Agricultural, ZoneType::Road, ZoneType::Park,
        ];
        for t in &types {
            let c = zone_color(*t);
            assert_eq!((c >> 24) & 0xFF, 0x44, "Alpha must be 0x44 for zones");
        }
    }

    #[test]
    fn test_rebuild_from_world() {
        crate::luts::init_trig_luts();
        crate::rng_pool::init_rng_pool(42);
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);
        let mut cache = RenderCache::new();
        cache.rebuild_from_world(&gw.world);
        assert!(cache.total_entries() > 0, "Cache must be populated from world");
        assert!(!cache.dirty);
    }

    #[test]
    fn test_new_building_colors_distinct() {
        let hospital = building_color(BuildingType::Hospital);
        let school = building_color(BuildingType::School);
        let police = building_color(BuildingType::Police);
        assert_ne!(hospital, school);
        assert_ne!(school, police);
        assert_ne!(hospital, police);
    }
}
