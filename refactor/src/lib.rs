// Citybound Native - Biblioteca central v0.10.0 [FASE 7]
//
// Re-exporta todos los módulos públicos para uso en main.rs y tests.
//
// Arquitectura:
// - ecs: Entity Component System (hecs)
// - sim: Sistemas de simulación (tiempo, tráfico, economía, suelo)
// - render: Renderizado software al framebuffer (con SIMD) + RenderCache
// - render_cache: Pre-sort estático de entidades por capa
// - luts: Look-up tables trigonométricas precalculadas
// - object_pool: Pool de entidades preasignadas
// - bump_alloc: Bump allocator por frame
// - input: Manejo de input con debounce
// - terrain: Mapa de terreno con ruido Perlin pre-generado
// - quadtree: Quadtree espacial
// - simd_render: Framebuffer SIMD autovectorizado [SSE2]
// - rng_pool: RNG pre-generado [TC#22]
// - flow_field: Flow fields para pathfinding O(1) [TA#7]
// - bitboard: Bitboards para colisiones en grilla [TI#6]
// - audio: Audio procedural con cpal [FASE 7]
// - traffic_lanes: Tráfico con carriles A/B Street [#361]
// - interactive: Herramienta de diseño urbano [#392]
// - supply_chain: Cadena de suministro física [M#1]
// - land_value: Valor del suelo y gentrificación [M#2]
// - utilities: Propagación de agua/electricidad [M#3]
// - road_wear: Desgaste de infraestructura [M#4]
// - labor_market: Mercado laboral [M#5]
// - tax_system: Impuestos milimétricos y bonos [M#6]
// - parking: Estacionamiento físico y HOA [M#7]
// - waste_mgmt: Clasificación de basura [M#8]
// - customization: Personalización visual de edificios [M#9]
// - politics: NIMBY, sindicatos, elecciones [M#10]
// - climate: Ciclo día/noche con color grading [FASE 7]
// - persistence: Save/Load con bincode [FASE 7]

pub mod ecs;
pub mod sim;
pub mod render;
pub mod render_cache;
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
pub mod supply_chain;
pub mod land_value;
pub mod utilities;
pub mod road_wear;
pub mod labor_market;
pub mod tax_system;
pub mod parking;
pub mod waste_mgmt;
pub mod customization;
pub mod politics;
pub mod climate;
pub mod persistence;
pub mod pedestrian; // 🆕 Social Force Model — peatones, cruces, multitudes
