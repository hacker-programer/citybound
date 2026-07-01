// Citybound Native - Biblioteca central v0.7.0
//
// Re-exporta todos los módulos públicos.
//
// Arquitectura completa:
// - ecs: Entity Component System (hecs)
// - sim: Simulación (tiempo, tráfico, economía, suelo, cadenas, empleo)
// - render: Renderizado software (SIMD)
// - luts: LUTs trigonométricas
// - object_pool: Pool de entidades
// - bump_alloc: Bump allocator por frame
// - input: Input con debounce
// - terrain: Terreno Perlin
// - quadtree: Quadtree espacial
// - simd_render: SIMD autovectorizado
// - rng_pool: RNG pre-generado
// - flow_field: Flow Fields O(1)
// - bitboard: Bitboards O(1)
// - audio: Audio procedural
// - traffic_lanes: Carriles A/B Street [#361]
// - interactive: Diseño urbano [#392]
// - supply_chain: Cadenas de suministro físicas [M#1]
// - land_value: Valor del suelo y gentrificación [M#2]
// - utilities: Agua y electricidad con propagación [M#3]
// - road_wear: Desgaste de infraestructura [M#4]
// - labor_market: Mercado laboral con empleo real [M#5]

pub mod ecs;
pub mod sim;
pub mod render;
pub mod luts;
pub mod object_pool;
pub mod bump_alloc;
pub mod input;
pub mod terrain;
pub mod quadtree;
pub mod simd_render;
pub mod rng_pool;
pub mod flow_field;
pub mod bitboard;
pub mod audio;
pub mod traffic_lanes;
pub mod interactive;
pub mod supply_chain;
pub mod land_value;
pub mod utilities;
pub mod road_wear;
pub mod labor_market;
