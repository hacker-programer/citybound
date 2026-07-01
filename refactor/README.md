# Citybound Native - Refactor ECS v0.5.0

Refactorización completa del simulador de ciudades Citybound como aplicación nativa de escritorio en Rust puro, optimizada para hardware legacy (Pentium 4GB RAM, 2 núcleos).

## 🎯 Objetivo

Convertir el Citybound original (basado en navegador, TypeScript + Rust/WASM) en una aplicación de escritorio nativa.

## 🏗️ Arquitectura

### Entity Component System (ECS) con `hecs`

```
┌──────────────────────────────────────────────────┐
│                  GAME LOOP                       │
│   Input → Update → Simulate → Render             │
│                                                    │
│   [Bump Allocator reset cada frame]               │
│   [RNG Pool lock-free O(1)]                       │
│   [Flow Fields O(1) pathfinding]                  │
│   [Bitboards O(1) collision]                      │
│   [SIMD autovectorized rendering]                 │
└──────────────────────────────────────────────────┘
```

### Componentes (alineados a 64B para caché L1)

| Componente | Descripción |
|-----------|-------------|
| `Position` | Coordenadas (f32, f32) |
| `Velocity` | Velocidad (f32, f32) |
| `Renderable` | Forma, color, tamaño, capa |
| `ZoneComponent` | Tipo de zona, densidad |
| `TrafficCar` | Velocidad, aceleración, carril |
| `ResourceStorage` | Dinero, comida, bienes |
| `ConstructionState` | Progreso, tipo de edificio |
| `Camera` | Offset, zoom |
| `Lifetime` | Ticks restantes |

### Sistemas de Simulación

1. **TimeSystem**: Avance del tiempo, ticks, hora del día
2. **TrafficSystem**: Microsimulación con Flow Fields O(1) + Bitboards
3. **EconomySystem**: Consumo/producción de recursos
4. **LandUseSystem**: Desarrollo de zonas (RNG pool)

## 📊 90 Técnicas de Optimización Implementadas

### Técnicas Comunes - Videojuegos (30)
- [x] TC#1: Object Pooling Masivo (10,000 entidades)
- [x] TC#2: Pre-reserva de capacidad (Vec::with_capacity)
- [x] TC#3: Baking de iluminación a texturas (terreno)
- [x] TC#4: Texturas en atlas (fill_pattern_simd)
- [x] TC#5: LUTs trigonométricas (3600 entradas)
- [x] TC#6: Audio procedural pre-generado (PCM buffers)
- [x] TC#7: Quadtree espacial
- [x] TC#8: Bincode para serialización
- [x] TC#9: Hitboxes pre-simplificadas (bitboards)
- [x] TC#10: Pre-multiplicación de matrices de cámara
- [x] TC#13: Loop unrolling manual (16px/batch)
- [x] TC#14: Ruido Perlin pre-generado
- [x] TC#17: Culling estático (viewport)
- [x] TC#21: Distancias al cuadrado
- [x] TC#22: RNG Pool pre-generado (4096 valores)
- [x] TC#23: Pre-ordenamiento por Z-Index (capas)
- [x] TC#25: f32 exclusivo
- [x] TC#26: Inlining agresivo
- [x] TC#28: get_unchecked en bucles validados

### Técnicas Avanzadas - Videojuegos (20)
- [x] TA#1: ECS puro (hecs con SoA)
- [x] TA#2: LTO fat + codegen-units=1 + panic=abort
- [x] TA#7: Flow Fields para pathfinding O(1)
- [x] TA#8: Cache Warming (L1/L2)
- [x] TA#9: Structs alineados a 64 bytes
- [x] TA#15: Uso exclusivo de f32
- [x] TA#16: Inlining agresivo
- [x] TA#17: Acceso unchecked en bucles validados
- [x] TA#20: Bump allocator por frame

### Técnicas Innovadoras - Videojuegos (10)
- [x] TI#1: Autómatas finitos aplanados
- [x] TI#4: Lock-free con Atomic Ordering::Relaxed
- [x] TI#6: Bitboards para colisiones O(1) en grilla

### Técnicas de Aplicaciones
- [x] TAp#1: Debounce/throttle en input
- [x] TAp#9: Caché in-memory indexada
- [x] TAp#27: Caché L1 forzada con alineación

## 🚀 Ejecución

### Requisitos
- Rust 1.70+ (stable)
- Windows/Linux/Mac
- 4GB RAM mínimo

```bash
cd refactor
cargo run --release
```

### Tests

```bash
cargo test
cargo test -- --nocapture
```

### Controles

| Tecla | Acción |
|-------|--------|
| WASD / Flechas | Mover cámara |
| PageUp / PageDown | Zoom in/out |
| ESC | Salir |

## 📁 Estructura de Archivos

```
refactor/
├── Cargo.toml              # Dependencias mínimas
├── README.md               # Este archivo
├── src/
│   ├── main.rs             # Game loop con cache warming
│   ├── lib.rs              # Re-exportaciones
│   ├── ecs.rs              # Componentes, GameWorld
│   ├── sim.rs              # Simulación (Flow Fields, Bitboards)
│   ├── render.rs           # Renderizado software (SIMD)
│   ├── simd_render.rs      # Framebuffer SIMD autovectorizado
│   ├── luts.rs             # LUTs trigonométricas
│   ├── rng_pool.rs         # RNG pre-generado
│   ├── flow_field.rs       # Flow Fields para pathfinding
│   ├── bitboard.rs         # Bitboards para colisiones
│   ├── audio.rs            # Audio procedural
│   ├── object_pool.rs      # Pool de entidades
│   ├── bump_alloc.rs       # Bump allocator por frame
│   ├── input.rs            # Input con bitfields
│   ├── terrain.rs          # Terreno Perlin
│   └── quadtree.rs         # Quadtree espacial
└── tests/
    ├── unit_tests.rs        # Tests unitarios
    └── integration_tests.rs # Tests de integración
```

## 🔧 Perfil de Release

```toml
[profile.release]
lto = "fat"
codegen-units = 1
panic = "abort"
opt-level = 3
strip = "symbols"
overflow-checks = false
```

## 📝 Notas de Diseño

1. **Flow Fields (TA#7)**: Cada coche consulta O(1) su celda para dirección. 100 coches = 100 lookups, no 100 * O(N log N).
2. **Bitboards (TI#6)**: Colisiones en grilla con operaciones bit a bit. 128x128 = solo 2KB en L1.
3. **SIMD Autovectorizado**: LLVM convierte los 16 stores contiguos en instrucciones SSE2 de 128 bits.
4. **RNG Pool (TC#22)**: 4096 floats pre-generados, acceso lock-free con Relaxed ordering.
5. **Cache Warming (TA#8)**: Todas las estructuras críticas se tocan durante la carga.
6. **Bump Allocator (TA#20)**: Reset atómico al inicio de cada frame, sin free() individuales.

## 📜 Licencia

AGPL-3.0 (misma que el proyecto original Citybound)
