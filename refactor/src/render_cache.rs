// RenderCache v0.18 — Pre-sort estático con sprites reales [REFACTOR VISUAL]
//
// Usa las categorías del TextureAtlas para asignar sprites reales a edificios,
// terreno y vehículos. Si no hay sprites, el renderizador dibuja formas
// arquitectónicas reconocibles (casas con tejado, fábricas con chimeneas, etc.)
//
// PALETA v0.18: Tonos tierra muted, sin colores saturados.

use crate::ecs::{BuildingType, ZoneType, Position, Renderable, ZoneComponent, ConstructionState, TrafficCar};
use crate::texture_atlas::{TextureAtlas, BuildingTileStyle};

pub const LAYER_TERRAIN: u8 = 0;
pub const LAYER_ZONES: u8 = 1;
pub const LAYER_BUILDINGS: u8 = 2;
pub const LAYER_CONSTRUCTION: u8 = 3;
pub const LAYER_TRAFFIC: u8 = 4;
pub const LAYER_UI: u8 = 5;
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

    /// Reconstruye el cache asignando sprites reales del atlas
    pub fn rebuild_from_world_with_atlas(&mut self, world: &hecs::World, atlas: &TextureAtlas) {
        self.clear();

        let mut sprite_offset: usize = 0;

        // ---- Zonas (capa 1) — colores planos con alpha ----
        for (_entity, (pos, zone)) in world.query::<(&Position, &ZoneComponent)>().iter() {
            if zone.density > 0 {
                self.push(RenderCacheEntry {
                    world_x: pos.x,
                    world_y: pos.y,
                    shape_type: 0,
                    color: zone_color(zone.zone_type),
                    size_x: 4.0,
                    layer: LAYER_ZONES,
                    sprite_index: 0,
                });
            }
        }

        // ---- Edificios completos (capa 2) — sprites del atlas ----
        {
            let mut building_entries: Vec<RenderCacheEntry> = Vec::with_capacity(512);

            for (_entity, (pos, cs)) in world.query::<(&Position, &ConstructionState)>().iter() {
                if cs.progress >= 1.0 {
                    let style = building_type_to_style(cs.building_type);
                    let si = atlas.categories.building_sprite(style) as u16;
                    let color = building_color(cs.building_type);
                    building_entries.push(RenderCacheEntry {
                        world_x: pos.x,
                        world_y: pos.y,
                        shape_type: 0,
                        color,
                        size_x: 4.0,
                        layer: LAYER_BUILDINGS,
                        sprite_index: si,
                    });
                }
            }

            // Fallback: edificios con Renderable pero sin ConstructionState
            for (_entity, (pos, renderable)) in world.query::<(&Position, &Renderable)>().iter() {
                if renderable.layer == 2 || renderable.layer == 3 {
                    if world.query_one::<&ConstructionState>(_entity).is_ok() {
                        continue;
                    }
                    let style = guess_building_category(renderable.color);
                    let si = atlas.categories.building_sprite(style) as u16;
                    building_entries.push(RenderCacheEntry {
                        world_x: pos.x,
                        world_y: pos.y,
                        shape_type: renderable.shape_type,
                        color: renderable.color,
                        size_x: renderable.size_x,
                        layer: renderable.layer as u8,
                        sprite_index: si,
                    });
                }
            }

            for entry in building_entries {
                self.push(entry);
            }
        }

        // ---- Construcciones en progreso (capa 3) ----
        for (_entity, (pos, cs)) in world.query::<(&Position, &ConstructionState)>().iter() {
            if cs.progress < 1.0 && cs.progress > 0.0 {
                let style = building_type_to_style(cs.building_type);
                let si = atlas.categories.building_sprite(style) as u16;
                self.push(RenderCacheEntry {
                    world_x: pos.x,
                    world_y: pos.y,
                    shape_type: 0,
                    color: 0xFF_C8_B8_5C, // amarillo construcción
                    size_x: 3.0 * cs.progress,
                    layer: LAYER_CONSTRUCTION,
                    sprite_index: if si > 0 { si } else { 0 },
                });
            }
        }

        // ---- Tráfico (capa 4) — vehículos con sprites ----
        for (_entity, (pos, renderable, _car)) in world.query::<(&Position, &Renderable, &TrafficCar)>().iter() {
            let vi = if atlas.categories.vehicles.is_empty() {
                0u16
            } else {
                sprite_offset += 1;
                atlas.categories.vehicles[sprite_offset % atlas.categories.vehicles.len()] as u16
            };

            self.push(RenderCacheEntry {
                world_x: pos.x,
                world_y: pos.y,
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

/// Adivina la categoría de edificio por su color
fn guess_building_category(color: u32) -> BuildingTileStyle {
    let r = (color >> 16) & 0xFF;
    let g = (color >> 8) & 0xFF;
    let b = color & 0xFF;

    // Nueva paleta muted
    if is_near(r, g, b, 0xC4, 0x8E, 0x6A, 30) { return BuildingTileStyle::House; }
    if is_near(r, g, b, 0xA8, 0xA8, 0xB0, 25) { return BuildingTileStyle::Apartment; }
    if is_near(r, g, b, 0x5C, 0xA0, 0xB8, 30) { return BuildingTileStyle::Shop; }
    if is_near(r, g, b, 0x8A, 0x9B, 0xA8, 25) { return BuildingTileStyle::Office; }
    if is_near(r, g, b, 0x8A, 0x7A, 0x6E, 25) { return BuildingTileStyle::Factory; }
    if is_near(r, g, b, 0x8C, 0xA8, 0x6A, 30) { return BuildingTileStyle::Farm; }
    if is_near(r, g, b, 0xE8, 0xE8, 0xF0, 30) { return BuildingTileStyle::Hospital; }
    if is_near(r, g, b, 0xE8, 0xD8, 0x8C, 30) { return BuildingTileStyle::School; }
    if is_near(r, g, b, 0x5C, 0x70, 0xC4, 30) { return BuildingTileStyle::Police; }

    // Fallback a legacy colors
    if r > 200 && g < 150 && b < 100 { return BuildingTileStyle::House; }
    if r > 160 && g > 160 && b > 160 { return BuildingTileStyle::Apartment; }
    if b > 200 && r < 150 { return BuildingTileStyle::Shop; }
    if g > 180 && r > 180 && b < 120 { return BuildingTileStyle::School; }
    if r > 200 && g < 120 && b < 120 { return BuildingTileStyle::Hospital; }
    if b > 200 && r < 100 && g < 150 { return BuildingTileStyle::Police; }
    if r < 120 && g < 120 && b < 120 { return BuildingTileStyle::Factory; }
    if g > 150 && r > 100 && b < 100 { return BuildingTileStyle::Farm; }

    BuildingTileStyle::Generic
}

#[inline(always)]
fn is_near(r: u32, g: u32, b: u32, tr: u32, tg: u32, tb: u32, tol: u32) -> bool {
    (r as i32 - tr as i32).abs() < tol as i32
        && (g as i32 - tg as i32).abs() < tol as i32
        && (b as i32 - tb as i32).abs() < tol as i32
}

// ---------------------------------------------------------------------------
// COLORES DE ZONA Y EDIFICIO (paleta v0.18 muted)
// ---------------------------------------------------------------------------

#[inline(always)]
pub fn building_color(btype: BuildingType) -> u32 {
    match btype {
        BuildingType::House     => 0xFF_C4_8E_6A,  // terracota suave
        BuildingType::Apartment => 0xFF_A8_A8_B0,  // gris medio
        BuildingType::Shop      => 0xFF_5C_A0_B8,  // azul comercio
        BuildingType::Office    => 0xFF_8A_9B_A8,  // gris azulado
        BuildingType::Factory   => 0xFF_8A_7A_6E,  // marrón industrial
        BuildingType::Farm      => 0xFF_8C_A8_6A,  // verde rural
        BuildingType::Hospital  => 0xFF_E8_E8_F0,  // blanco hospital
        BuildingType::School    => 0xFF_E8_D8_8C,  // amarillo educativo
        BuildingType::Police    => 0xFF_5C_70_C4,  // azul policial
    }
}

#[inline(always)]
pub fn building_sprite(_btype: BuildingType) -> u16 { 0 }

#[inline(always)]
pub fn zone_color(ztype: ZoneType) -> u32 {
    match ztype {
        ZoneType::Residential  => 0x88_7B_A0_5C,  // verde apagado
        ZoneType::Commercial   => 0x88_5C_8F_A0,  // azul apagado
        ZoneType::Industrial   => 0x88_A0_6C_5C,  // rojo apagado
        ZoneType::Agricultural => 0x88_8F_A0_5C,  // amarillo apagado
        ZoneType::Road         => 0x88_88_88_88,  // gris
        ZoneType::Park         => 0x88_5C_A0_6C,  // verde menta
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
