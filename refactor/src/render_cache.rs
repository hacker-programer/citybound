// RenderCache v0.17 — Pre-sort estático con sprites reales [FASE 9]
//
// Ahora usa las categorías del TextureAtlas para asignar sprites
// reales a edificios, terreno y vehículos.
//
// NOVEDADES v0.17:
// - rebuild_from_world_with_atlas: asigna sprites reales usando el atlas
// - building_sprite_for: lookup de sprite por BuildingType en el atlas
// - terrain_sprite_for: lookup de sprite de terreno en el atlas
// - vehicle_sprite: sprite de coche aleatorio del atlas

use crate::ecs::{BuildingType, ZoneType, Position, Renderable, ZoneComponent, ConstructionState, TrafficCar};
use crate::ecs::{BuildingType, ZoneType, Position, Renderable, ZoneComponent, ConstructionState, TrafficCar};
use crate::texture_atlas::{TextureAtlas, BuildingTileStyle};
use crate::render::{
    COLOR_ZONE_RESIDENTIAL, COLOR_ZONE_COMMERCIAL, COLOR_ZONE_INDUSTRIAL,
    COLOR_ZONE_AGRICULTURAL, COLOR_ZONE_ROAD, COLOR_ZONE_PARK,
    COLOR_BUILDING_HOUSE, COLOR_BUILDING_APARTMENT, COLOR_BUILDING_SHOP,
    COLOR_BUILDING_OFFICE, COLOR_BUILDING_FACTORY, COLOR_BUILDING_FARM,
    COLOR_BUILDING_HOSPITAL, COLOR_BUILDING_SCHOOL, COLOR_BUILDING_POLICE,
};
pub const NUM_RENDER_LAYERS: usize = 6;

#[derive(Copy, Clone, Debug)]
pub struct RenderCacheEntry {
    pub world_x: f32,
    pub world_y: f32,
    pub shape_type: u8,
    pub color: u32,
    pub size_x: f32,
    pub layer: u8,
    pub sprite_index: u16,
}

impl RenderCacheEntry {
    #[inline(always)]
    pub fn new(x: f32, y: f32, shape: u8, color: u32, size_x: f32, layer: u8) -> Self {
        RenderCacheEntry { world_x: x, world_y: y, shape_type: shape, color, size_x, layer, sprite_index: 0 }
    }
    #[inline(always)]
    pub fn new_sprite(x: f32, y: f32, sprite_idx: u16, size_x: f32, layer: u8) -> Self {
        RenderCacheEntry { world_x: x, world_y: y, shape_type: 0, color: 0, size_x, layer, sprite_index: sprite_idx }
    }
}

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

    /// [FASE 9]: Reconstruye el cache asignando sprites reales del atlas
    pub fn rebuild_from_world_with_atlas(&mut self, world: &hecs::World, atlas: &TextureAtlas) {
        self.clear();

        // Contador para variar sprites
        let mut sprite_offset: usize = 0;

        // Zonas (capa 1) — usan colores planos con alpha para indicar zonificación
        for (_entity, (pos, zone)) in world.query::<(&Position, &ZoneComponent)>().iter() {
            if zone.density > 0 {
                self.push(RenderCacheEntry {
                    world_x: pos.x, world_y: pos.y,
                    shape_type: 0,
                    color: zone_color(zone.zone_type),
                    size_x: 1.0,
                    layer: LAYER_ZONES,
                    sprite_index: 0, // Las zonas usan rectángulos de color
                });
            }
        }

        // Edificios (capa 2-3) — usan sprites reales del atlas
        for (_entity, (pos, renderable)) in world.query::<(&Position, &Renderable)>().iter() {
            if renderable.layer == 2 || renderable.layer == 3 {
                // Si ya tiene sprite_index, usarlo; si no, intentar buscar en el atlas
                let si = if renderable.sprite_index > 0 {
                    renderable.sprite_index
                } else {
                    // Buscar por color/categoría
                    let category = guess_building_category(renderable.color);
                    let idx = atlas.categories.building_sprite(category) as u16;
                    if idx > 0 { idx } else { 0 }
                };

                self.push(RenderCacheEntry {
                    world_x: pos.x, world_y: pos.y,
                    shape_type: renderable.shape_type,
                    color: renderable.color,
                    size_x: renderable.size_x,
                    layer: renderable.layer as u8,
                    sprite_index: si,
                });
            }
        }

        // Construcciones en progreso (capa 3)
        for (_entity, (pos, cs)) in world.query::<(&Position, &ConstructionState)>().iter() {
            if cs.progress < 1.0 && cs.progress > 0.0 {
                let style = building_type_to_style(cs.building_type);
                let si = atlas.categories.building_sprite(style) as u16;
                self.push(RenderCacheEntry {
                    world_x: pos.x, world_y: pos.y,
                    shape_type: 0,
                    color: 0x88_FF_FF_00,
                    size_x: 3.0 * cs.progress,
                    layer: LAYER_CONSTRUCTION,
                    sprite_index: if si > 0 { si } else { 0 },
                });
            }
        }

        // Tráfico (capa 4) — vehículos
        for (_entity, (pos, renderable, _car)) in world.query::<(&Position, &Renderable, &TrafficCar)>().iter() {
            // Usar sprite de vehículo si hay disponible
            let vi = if atlas.categories.vehicles.is_empty() {
                0u16
            } else {
                sprite_offset += 1;
                atlas.categories.vehicles[sprite_offset % atlas.categories.vehicles.len()] as u16
            };

            self.push(RenderCacheEntry {
                world_x: pos.x, world_y: pos.y,
                shape_type: renderable.shape_type,
                color: renderable.color,
                size_x: renderable.size_x,
                layer: LAYER_TRAFFIC,
                sprite_index: vi,
            });
        }

        self.dirty = false;
    }

    /// Compatibilidad con API antigua (sin atlas)
    pub fn rebuild_from_world(&mut self, world: &hecs::World) {
        let dummy_atlas = TextureAtlas::new();
        self.rebuild_from_world_with_atlas(world, &dummy_atlas);
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
// MAPEO DE BUILDINGTYPE → BUILDINGTILESTYLE
// ---------------------------------------------------------------------------

#[inline(always)]
pub fn building_type_to_style(btype: BuildingType) -> BuildingTileStyle {
    match btype {
        BuildingType::House => BuildingTileStyle::House,
        BuildingType::Apartment => BuildingTileStyle::Apartment,
        BuildingType::Shop => BuildingTileStyle::Shop,
        BuildingType::Office => BuildingTileStyle::Office,
        BuildingType::Factory => BuildingTileStyle::Factory,
        BuildingType::Farm => BuildingTileStyle::Farm,
        BuildingType::Hospital => BuildingTileStyle::Hospital,
        BuildingType::School => BuildingTileStyle::School,
        BuildingType::Police => BuildingTileStyle::Police,
    }
}

/// Adivina la categoría de edificio por su color legacy
fn guess_building_category(color: u32) -> BuildingTileStyle {
    let r = (color >> 16) & 0xFF;
    let g = (color >> 8) & 0xFF;
    let b = color & 0xFF;

    if r > 200 && g < 150 && b < 100 { return BuildingTileStyle::House; }      // Naranja/marrón = casa
    if r > 160 && g > 160 && b > 160 { return BuildingTileStyle::Apartment; }   // Gris = apartamento
    if b > 200 && r < 150 { return BuildingTileStyle::Shop; }                   // Azul = tienda
    if g > 180 && r > 180 && b < 120 { return BuildingTileStyle::School; }      // Amarillo = escuela
    if r > 200 && g < 120 && b < 120 { return BuildingTileStyle::Hospital; }    // Rojo = hospital
    if b > 200 && r < 100 && g < 150 { return BuildingTileStyle::Police; }      // Azul = policía
    if r < 120 && g < 120 && b < 120 { return BuildingTileStyle::Factory; }     // Oscuro = fábrica
    if g > 150 && r > 100 && b < 100 { return BuildingTileStyle::Farm; }        // Verde = granja
    BuildingTileStyle::Generic
}

// ---------------------------------------------------------------------------
// COLORES DE ZONA Y EDIFICIO (legacy)
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
        BuildingType::Hospital => 0xFF_F4_81_81,
        BuildingType::School => 0xFF_FF_D5_4F,
        BuildingType::Police => 0xFF_42_45_E8,
    }
}

#[inline(always)]
pub fn building_sprite(_btype: BuildingType) -> u16 {
    // Obsoleto: ahora se usa el atlas directamente
    0
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

#[inline(always)]
pub fn zone_sprite(_ztype: ZoneType) -> u16 { 0 }

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

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
    fn test_building_type_to_style() {
        assert_eq!(building_type_to_style(BuildingType::House), BuildingTileStyle::House);
        assert_eq!(building_type_to_style(BuildingType::Police), BuildingTileStyle::Police);
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
}
