// Citybound Native - Biblioteca central v0.7.0
//
// Re-exporta todos los módulos públicos para uso en main.rs y tests.
//
// Arquitectura:
// - ecs: Entity Component System (hecs)
// - sim: Sistemas de simulación
// - render: Renderizado software al framebuffer (con SIMD)
// - luts: Look-up tables trigonométricas precalculadas
// - object_pool: Pool de entidades preasignadas
// - bump_alloc: Bump allocator por frame
// - input: Manejo de input con debounce
// - terrain: Mapa de terreno con ruido Perlin pre-generado
// - quadtree: Quadtree espacial para consultas O(log N)
// - simd_render: Funciones de framebuffer aceleradas con SIMD
// - rng_pool: Pool de RNG pre-generado [TC#22]
// - flow_field: Flow fields para pathfinding O(1) [TA#7]
// - bitboard: Bitboards para colisiones en grilla [TI#6]
// - audio: Sistema de audio procedural [TC#6]
// - traffic_lanes: Tráfico con carriles A/B Street [#361]
// - interactive: Herramienta de diseño urbano [#392]
// - supply_chain: Cadenas de suministro con camiones físicos [M#1]
// - land_value: Valor del suelo, contaminación, gentrificación [M#2]
// - utilities: Propagación de agua y electricidad [M#3]
// - road_wear: Desgaste de infraestructura y baches [M#4]
// - labor_market: Mercado laboral con commutes reales [M#5]

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
