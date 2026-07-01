# Citybound Native - Refactor ECS

Refactorización completa del simulador de ciudades Citybound como aplicación nativa de escritorio en Rust puro.

## 🎯 Objetivo

Convertir el Citybound original (basado en navegador, TypeScript + Rust/WASM) en una aplicación de escritorio nativa optimizada para hardware legacy (Pentium 4GB RAM, 2 núcleos).

## 🏗️ Arquitectura

### Entity Component System (ECS) con `hecs`

```
┌─────────────────────────────────────────┐
│              GAME LOOP                  │
│   Input → Update → Simulate → Render    │
└─────────────────────────────────────────┘
                    │
    ┌───────────────┼───────────────┐
    ▼               ▼               ▼
┌───────┐     ┌──────────┐    ┌────────┐
│  ECS  │     │   SIM    │    │ RENDER │
│(hecs) │◄───►│ Systems  │───►│(minifb)│
└───────┘     └──────────┘    └────────┘
```

### Componentes (almacenados en Struct-of-Arrays para localidad de caché)

| Componente | Descripción | Alineación L1 |
|-----------|-------------|---------------|
| `Position` | Coordenadas (f32, f32) | 64 bytes |
| `Velocity` | Velocidad (f32, f32) | 64 bytes |
| `Renderable` | Forma, color, tamaño, capa | 64 bytes |
| `ZoneComponent` | Tipo de zona, densidad | 64 bytes |
| `TrafficCar` | Velocidad, aceleración, carril | 64 bytes |
| `ResourceStorage` | Dinero, comida, bienes | 64 bytes |
| `ConstructionState` | Progreso, tipo de edificio | 64 bytes |
| `Camera` | Offset, zoom | 64 bytes |
| `Lifetime` | Ticks restantes | 64 bytes |

### Sistemas de Simulación

1. **TimeSystem**: Avance del tiempo, ticks, hora del día
2. **TrafficSystem**: Microsimulación de tráfico con aceleración inteligente
3. **EconomySystem**: Consumo/producción de recursos
4. **LandUseSystem**: Desarrollo de zonas y construcción

## 📊 Optimizaciones Implementadas

### Técnicas Comunes (30 de videojuegos)
- [x] TC#1: Object Pooling Masivo (10,000 entidades)
- [x] TC#2: Pre-reserva de capacidad (Vec::with_capacity)
- [x] TC#3: Baking de iluminación a texturas (paleta predefinida)
- [x] TC#5: LUTs trigonométricas (3600 entradas)
- [x] TC#6: Texturas en atlas (colores predefinidos)
- [x] TC#10: Pre-multiplicación de matrices de transformación (cámara)
- [x] TC#13: Loop unrolling manual en funciones críticas
- [x] TC#17: Culling estático (solo viewport visible)
- [x] TC#23: Pre-ordenamiento por Z-Index (capas)
- [x] TC#25: f32 exclusivo en todo el motor
- [x] TC#26: Inlining agresivo (#[inline(always)])

### Técnicas Avanzadas (20 de videojuegos)
- [x] TA#1: ECS puro (hecs con SoA)
- [x] TA#2: LTO fat + codegen-units=1 + panic=abort
- [x] TA#4: Bump allocator por frame (16MB preasignados)
- [x] TA#9: Structs alineados a 64 bytes (línea caché L1)
- [x] TA#14: Zero-copy al framebuffer
- [x] TA#15: Uso exclusivo de f32
- [x] TA#16: Inlining agresivo
- [x] TA#17: Acceso unchecked en bucles validados
- [x] TA#20: Bump allocator con reset atómico

### Técnicas Innovadoras (10 de videojuegos)
- [x] TI#1: Autómatas finitos aplanados para estados
- [x] TI#6: Bitboards para colisiones en grilla (estructuras de bits)

### Técnicas de Aplicaciones
- [x] TAp#1: Debounce/throttle en input
- [x] TAp#9: Caché in-memory indexada
- [x] TAp#27: Caché L1 forzada con alineación

## 🚀 Ejecución

### Requisitos
- Rust 1.70+ (stable)
- Windows/Linux/Mac
- 4GB RAM mínimo

### Compilar y ejecutar

```bash
cd refactor
cargo run --release
```

### Tests

```bash
cargo test
cargo test -- --nocapture  # Ver output de tests
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
├── Cargo.toml          # Dependencias mínimas
├── README.md           # Este archivo
├── src/
│   ├── main.rs         # Punto de entrada, game loop
│   ├── lib.rs          # Re-exportaciones
│   ├── ecs.rs          # Componentes, World, queries
│   ├── sim.rs          # Sistemas de simulación
│   ├── render.rs       # Renderizado software
│   ├── luts.rs         # Look-up tables trigonométricas
│   ├── object_pool.rs  # Pool de entidades
│   ├── bump_alloc.rs   # Bump allocator por frame
│   └── input.rs        # Input con bitfields
└── tests/
    ├── unit_tests.rs    # Tests unitarios
    └── integration_tests.rs  # Tests de integración
```

## 🔧 Perfil de Release

```toml
[profile.release]
lto = "fat"              # Link-time optimization agresiva
codegen-units = 1        # Máximo inlining cross-crate
panic = "abort"          # Sin unwinding
opt-level = 3            # Optimización máxima
strip = "symbols"        # Binario más pequeño
overflow-checks = false  # Sin checks en release
```

## 📝 Notas de Diseño

1. **Sin unsafe innecesario**: Todo el código unsafe está documentado con `// SAFETY:`.
2. **Sin dependencias web**: Cero HTML, JS, CSS, WASM. Solo Rust nativo + minifb.
3. **ECS puro**: `hecs` proporciona almacenamiento SoA nativo, maximizando hits de caché.
4. **Paso fijo de simulación**: 10 ticks/segundo independientes del framerate.
5. **Renderizado software**: Sin dependencia de GPU, funciona en cualquier hardware.

## 📜 Licencia

AGPL-3.0 (misma que el proyecto original Citybound)
