// Citybound Native - Biblioteca central
//
// Re-exporta todos los módulos públicos para uso en main.rs y tests.
//
// Arquitectura:
// - ecs: Entity Component System (hecs)
// - sim: Sistemas de simulación (tiempo, tráfico, economía, suelo)
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
