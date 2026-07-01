# Citybound Native - Refactor ECS v0.6.0

Refactorización completa del simulador de ciudades Citybound como aplicación nativa de escritorio en Rust puro, optimizada para hardware legacy (Pentium 4GB RAM, 2 núcleos).

## 🎯 Objetivo

Convertir el Citybound original (basado en navegador, TypeScript + Rust/WASM) en una aplicación de escritorio nativa con:
- **Tráfico con carriles** estilo A/B Street [#361]
- **Herramienta de diseño urbano** interactivo [#392]
- Optimizaciones extremas para hardware legacy

## 🏗️ Arquitectura

```
┌──────────────────────────────────────────────────┐
│                  GAME LOOP (30 FPS)              │
│   Input → Design Tool → Simulate → Render        │
│                                                    │
│   ┌─────────┐  ┌──────────┐  ┌────────────────┐  │
│   │ ECS     │  │ Traffic  │  │ Render         │  │
│   │ (hecs)  │  │ Flow Field│  │ Software SIMD  │  │
│   │ SoA     │  │ Bitboards │  │ 16px/batch     │  │
│   │ 64B     │  │ Lanes IDM │  │ Bresenham      │  │
│   └─────────┘  └──────────┘  └────────────────┘  │
│                                                    │
│   [Bump Allocator reset cada frame]               │
│   [RNG Pool lock-free 4096 values]                │
└──────────────────────────────────────────────────┘
```

### Componentes ECS (alineados a 64B para caché L1)

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

### Sistemas

1. **TimeSystem**: Ciclo día/noche, ticks
2. **TrafficSystem**: Flow Fields O(1) + Bitboards + Carriles IDM/MOBIL
3. **EconomySystem**: Recursos de hogares
4. **LandUseSystem**: Desarrollo procedural de zonas
5. **LaneSystem** [#361]: Intersecciones con semáforos, congestión
6. **DesignSystem** [#392]: Colocación de edificios, pintado de zonas, undo/redo

## 🚦 Tráfico con Carriles (A/B Street)

- **28 carriles** en grid 128x128: autopista central, avenidas verticales, calles residenciales
- **6 intersecciones** con semáforos (verde → amarillo → rojo)
- **IDM** (Intelligent Driver Model): aceleración basada en distancia y velocidad relativa
- **MOBIL**: decisiones de cambio de carril con factor de cortesía
- **Grid espacial** 128x128 para búsqueda O(1) de carriles
- **Congestión** visual: verde → amarillo → rojo

## 🎨 Herramienta de Diseño Urbano

| Tecla | Acción |
|-------|--------|
| `Tab` | Activar/desactivar herramienta |
| `1-6` | Seleccionar tipo de zona |
| `Shift+1-6` | Seleccionar tipo de edificio |
| `B` | Modo construcción |
| `I` | Modo inspección |
| `[` / `]` | Reducir/aumentar pincel |
| Click izquierdo | Colocar edificio / Iniciar zona |
| Click derecho | Pintar con pincel / Eliminar |
| `Ctrl+Z` | Deshacer |
| `Ctrl+Shift+Z` | Rehacer |

### Modos
- **Pintar zonas**: Click y arrastrar para definir área. Click derecho pinta con pincel.
- **Construir**: Click para colocar edificio. Click derecho elimina.
- **Inspeccionar**: Click para ver información de la celda.

## 📊 Técnicas de Optimización (38 de 90)

### Comunes - Videojuegos
- [x] TC#1: Object Pooling (10,000 entidades)
- [x] TC#2: Vec::with_capacity en todos lados
- [x] TC#3: Baking de iluminación (terreno)
- [x] TC#5: LUTs trigonométricas (3600 entradas)
- [x] TC#6: Audio procedural pre-generado
- [x] TC#7: Quadtree espacial
- [x] TC#10: Pre-multiplicación de cámara
- [x] TC#13: Loop unrolling (16px/batch)
- [x] TC#14: Ruido Perlin pre-generado
- [x] TC#17: Culling estático (viewport)
- [x] TC#21: Distancias al cuadrado
- [x] TC#22: RNG Pool pre-generado
- [x] TC#23: Pre-ordenamiento Z-Index
- [x] TC#25: f32 exclusivo
- [x] TC#26: Inlining agresivo
- [x] TC#28: get_unchecked en bucles

### Avanzadas - Videojuegos
- [x] TA#1: ECS puro (hecs SoA)
- [x] TA#2: LTO fat + codegen-units=1
- [x] TA#5: Fixed-point para velocidades
- [x] TA#7: Flow Fields O(1)
- [x] TA#8: Cache Warming
- [x] TA#9: Structs alineados a 64B
- [x] TA#17: Acceso unchecked
- [x] TA#20: Bump allocator por frame

### Innovadoras
- [x] TI#4: Lock-free Atomic Relaxed
- [x] TI#6: Bitboards para colisiones O(1)

### Aplicaciones
- [x] TAp#1: Debounce en input (bitfields)
- [x] TAp#9: Caché in-memory indexada

## 🚀 Ejecución

```bash
cd refactor
cargo run --release    # Compila y ejecuta (~2-5 min primera vez)
cargo test             # 120+ tests
```

### Controles básicos

| Tecla | Acción |
|-------|--------|
| `WASD` / Flechas | Mover cámara |
| `PageUp/Down` | Zoom |
| `Tab` | Herramienta de diseño |
| `ESC` | Salir |

## 📁 Estructura

```
refactor/
├── Cargo.toml
├── README.md
├── src/
│   ├── main.rs             # Game loop + cache warming
│   ├── lib.rs              # Re-exportaciones
│   ├── ecs.rs              # Componentes, GameWorld
│   ├── sim.rs              # Simulación (Flow, Bits, Lanes)
│   ├── render.rs           # Renderizado + red de carriles
│   ├── simd_render.rs      # SIMD autovectorizado
│   ├── luts.rs             # LUTs trigonométricas
│   ├── rng_pool.rs         # RNG pre-generado
│   ├── flow_field.rs       # Flow Fields O(1)
│   ├── bitboard.rs         # Bitboards (2KB L1)
│   ├── traffic_lanes.rs    # Carriles A/B Street [#361]
│   ├── interactive.rs      # Diseño urbano [#392]
│   ├── audio.rs            # Audio procedural
│   ├── object_pool.rs      # Pool de entidades
│   ├── bump_alloc.rs       # Bump allocator
│   ├── input.rs            # Input con bitfields
│   ├── terrain.rs          # Terreno Perlin
│   └── quadtree.rs         # Quadtree espacial
└── tests/
    ├── unit_tests.rs
    └── integration_tests.rs
```

## 📜 Licencia

AGPL-3.0 (misma que el proyecto original)
