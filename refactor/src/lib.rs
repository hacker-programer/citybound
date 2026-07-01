// Citybound Native - Biblioteca central
//
// Re-exporta todos los módulos públicos para uso en main.rs y tests.
//
// Arquitectura:
// - ecs: Entity Component System (hecs)
// - sim: Sistemas de simulación (tiempo, tráfico, economía, suelo)
// - render: Renderizado software al framebuffer
// - luts: Look-up tables trigonométricas precalculadas
// - object_pool: Pool de entidades preasignadas
// - bump_alloc: Bump allocator por frame
// - input: Manejo de input con debounce

pub mod ecs;
pub mod sim;
pub mod render;
pub mod luts;
pub mod object_pool;
pub mod bump_alloc;
pub mod input;
