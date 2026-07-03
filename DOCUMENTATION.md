# Citybound — Documentación Técnica Exhaustiva del Repositorio

> **Repositorio:** `citybound`  
> **Autor original:** Anselm Eickhoff  
> **Licencia:** AGPL-3.0  
> **Rama principal:** Simulación urbana realista en Rust + TypeScript (browser UI)  
> **Refactor nativa:** `refactor/` — Motor de simulación en Rust puro con renderizado software

---

## 📁 ESTRUCTURA COMPLETA DEL REPOSITORIO

```
citybound/
├── Cargo.toml                  # Workspace root (v0.3.0) — servidor HTTP + simulación
├── README.md                   # Readme original
├── cb_simulation/              # 🧠 Motor de simulación original (Rust → WASM)
│   └── src/
│       ├── lib.rs              # Punto de entrada (1680 líneas)
│       ├── dimensions.rs       # Tipos geométricos (641 líneas)
│       ├── economy/            # Sistema económico
│       ├── environment/        # Clima, terreno
│       ├── land_use/           # Uso del suelo, zonificación
│       ├── planning/           # Planificación urbana
│       └── transport/          # Tráfico, pathfinding
├── cb_planning/                # 📐 Planificación de construcción
│   └── src/
│       ├── lib.rs              # (25263 líneas) — Lógica masiva de planificación
│       ├── construction/       # Construcción de edificios
│       └── plan_manager/       # Gestor de planes
├── cb_server/                  # 🌐 Servidor HTTP (rouille)
│   ├── main.rs                 # Punto de entrada servidor (3611 líneas)
│   ├── browser_ui_server.rs    # Servidor de assets UI (1778 líneas)
│   └── init.rs                 # Inicialización del mundo (8521 líneas)
├── cb_time/                    # ⏱️ Sistema de tiempo y actores
│   └── src/
│       ├── lib.rs              # Punto de entrada (322 líneas)
│       ├── units.rs            # Unidades temporales (6363 líneas)
│       └── actors/             # Actores temporales
├── cb_util/                    # 🛠️ Utilidades compartidas
│   └── src/
│       ├── lib.rs              # Punto de entrada (512 líneas)
│       ├── random.rs           # RNG determinista (1092 líneas)
│       ├── async_counter.rs    # Contador asíncrono (718 líneas)
│       ├── config_manager/     # Gestión de configuración
│       └── log/                # Sistema de logging
├── cb_browser_ui/              # 🖥️ Interfaz web (TypeScript + React)
│   ├── Cargo.toml              # WASM build
│   ├── src/
│   │   ├── lib.rs              # Bindings Rust→WASM (5844 líneas)
│   │   ├── citybound.tsx       # Componente principal React (10028 líneas)
│   │   ├── menu.tsx            # Menú del juego (11963 líneas)
│   │   ├── uiModes.tsx         # Modos de UI (1671 líneas)
│   │   ├── settings.js         # Configuración (9153 líneas)
│   │   ├── colors.js           # Paleta de colores (5632 líneas)
│   │   ├── renderOrder.js      # Orden de renderizado (480 líneas)
│   │   ├── toolbar.js          # Barra de herramientas (743 líneas)
│   │   ├── uuid.js             # Generador UUID (238 líneas)
│   │   ├── camera/             # Sistema de cámara
│   │   ├── debug/              # Herramientas de debug
│   │   ├── stage/              # Stage/escenario
│   │   ├── browser_utils/      # Utilidades de navegador
│   │   ├── households_browser/ # Panel de hogares
│   │   ├── land_use_browser/   # Panel de uso de suelo
│   │   ├── planning_browser/   # Panel de planificación
│   │   ├── time_browser/       # Panel de tiempo
│   │   ├── transport_browser/  # Panel de transporte
│   │   └── vegetation_browser/ # Panel de vegetación
│   ├── index.html              # HTML principal (3877 líneas)
│   ├── main.less               # Estilos (11061 líneas)
│   └── assets/                 # Iconos, fuentes, imágenes
├── modding/                    # 🧩 Reglas de arquitectura para modding
│   └── architecture_rules.yaml # (2681 líneas) — Reglas YAML para modders
├── refactor/                   # 🔥 REFACTOR NATIVO v0.10.0 (RUST PURO)
│   ├── Cargo.toml              # Dependencias: hecs, minifb, bincode, rayon, cpal
│   ├── DOCUMENTATION.md        # Documentación exhaustiva del refactor
│   ├── README.md               # Readme del refactor
│   ├── src/                    # Código fuente (34 archivos Rust)
│   │   ├── lib.rs              # Re-exporta todos los módulos (2314 líneas)
│   │   ├── main.rs             # Punto de entrada con game loop (11457 líneas)
│   │   ├── ecs.rs              # Entity Component System basado en hecs
│   │   ├── sim.rs              # Tick de simulación central
│   │   ├── render.rs           # Renderizado software al framebuffer
│   │   ├── render_cache.rs     # Pre-sort estático de entidades por capa
│   │   ├── simd_render.rs      # Framebuffer SIMD autovectorizado
│   │   ├── luts.rs             # Look-up tables trigonométricas
│   │   ├── object_pool.rs      # Pool de entidades preasignadas
│   │   ├── bump_alloc.rs       # Bump allocator por frame
│   │   ├── rng_pool.rs         # RNG pre-generado en pool
│   │   ├── input.rs            # Input con debounce
│   │   ├── terrain.rs          # Mapa de terreno con ruido Perlin
│   │   ├── quadtree.rs         # Quadtree espacial para búsquedas
│   │   ├── flow_field.rs       # Flow fields para pathfinding O(1)
│   │   ├── bitboard.rs         # Bitboards para colisiones en grilla
│   │   ├── traffic_lanes.rs    # Tráfico con carriles estilo A/B Street
│   │   ├── pedestrian.rs       # Social Force Model para peatones
│   │   ├── interactive.rs      # Herramienta de diseño urbano interactivo
│   │   ├── audio.rs            # Audio procedural con cpal
│   │   ├── climate.rs          # Ciclo día/noche con color grading
│   │   ├── supply_chain.rs     # Cadena de suministro física
│   │   ├── land_value.rs       # Valor del suelo y gentrificación
│   │   ├── utilities.rs        # Propagación de agua/electricidad
│   │   ├── road_wear.rs        # Desgaste de infraestructura vial
│   │   ├── labor_market.rs     # Mercado laboral con matching
│   │   ├── tax_system.rs       # Impuestos milimétricos y bonos
│   │   ├── parking.rs          # Estacionamiento físico y HOA
│   │   ├── waste_mgmt.rs       # Clasificación y gestión de residuos
│   │   ├── customization.rs    # Personalización visual de edificios
│   │   ├── politics.rs         # NIMBY, sindicatos, elecciones
│   │   └── persistence.rs      # Save/Load con bincode
│   ├── tests/
│   │   ├── unit_tests.rs       # Tests unitarios
│   │   └── integration_tests.rs # Tests de integración
│   └── target/                 # Build artifacts
└── repo_scripts/
    └── tooling.js              # Scripts de build tooling (4855 líneas)
```

---

## 🏗️ ARQUITECTURA DEL PROYECTO ORIGINAL (v0.3.0)

### Workspace Rust

El workspace principal (`Cargo.toml` raíz) contiene 4 crates:

| Crate | Descripción | Tipo |
|-------|-------------|------|
| `cb_simulation` | Motor de simulación (economía, transporte, uso de suelo) | lib → WASM |
| `cb_planning` | Planificación de construcción urbana | lib |
| `cb_util` | Utilidades: RNG, logging, configuración | lib |
| `cb_server` | Servidor HTTP (rouille) que sirve la UI web | bin |

El crate `cb_browser_ui` está excluido del workspace porque compila a WASM.

### Flujo de Datos Original

```
cb_server (main.rs)
  ├── Inicializa cb_simulation
  ├── Sirve cb_browser_ui (WASM + HTML/JS)
  └── API REST para comunicación cliente↔servidor
        │
        ▼
cb_browser_ui (TypeScript + Rust/WASM)
  ├── Renderizado WebGL
  ├── UI en React/Preact
  └── Comunicación con servidor vía HTTP
```

---

## 🔥 REFACTOR NATIVO v0.10.0 (carpeta `refactor/`)

### Objetivo

Versión standalone en Rust puro con renderizado software (sin navegador, sin WASM, sin JS).

### Stack Tecnológico

| Componente | Tecnología | Propósito |
|------------|-----------|-----------|
| ECS | `hecs 0.10` | Entity Component System |
| Renderizado | Framebuffer `minifb 0.27` | Ventana nativa con buffer de píxeles |
| Serialización | `bincode 1.3` + `serde` | Save/Load binario |
| Matemáticas | `libm 0.2` | Trigonometría sin std |
| RNG | `rand 0.8` (small_rng) | Aleatoriedad determinista |
| Paralelismo | `rayon 1.9` | Procesamiento multihilo |
| Audio | `cpal 0.15` | Audio procedural |
| SIMD | `std::arch::x86_64` | SSE2/AVX para renderizado |

### Sistemas Implementados en el Refactor

#### 🧠 Núcleo

| Archivo | Líneas | Estructuras Clave | Función |
|---------|--------|-------------------|---------|
| `ecs.rs` | 18065 | `GameWorld`, `SpatialGrid`, `BuildingPrototype`, `TrafficCar`, `ConstructionState` | ECS central con hecs |
| `sim.rs` | 11929 | `SimConfig`, `tick()` | Bucle principal de simulación |
| `main.rs` | 11457 | `main()`, `write_fps_title()` | Game loop a 30fps / 10 ticks/s |

#### 🎨 Renderizado

| Archivo | Líneas | Estructuras Clave | Función |
|---------|--------|-------------------|---------|
| `render.rs` | 25242 | `RenderConfig`, `render_world_cached()` | Pipeline de renderizado software |
| `render_cache.rs` | 8943 | `RenderCache`, `CachedEntity` | Pre-sort estático Z-order O(1) |
| `simd_render.rs` | 19008 | `warm_cache()`, SIMD fill | Autovectorización SSE2 |
| `luts.rs` | 4751 | `TRIG_LUT`, `init_trig_luts()` | Look-up tables seno/coseno |

#### 🚗 Transporte

| Archivo | Líneas | Estructuras Clave | Función |
|---------|--------|-------------------|---------|
| `traffic_lanes.rs` | 20376 | `LaneManager`, `Lane`, `Intersection`, IDM+MOBIL | Tráfico microscópico |
| `flow_field.rs` | 8860 | `FlowFieldGrid`, `sample_combined()` | Pathfinding O(1) |
| `parking.rs` | 15027 | `ParkingManager`, `HOA` | Estacionamiento y asociaciones |
| `pedestrian.rs` | 20328 | `Pedestrian`, Social Force Model | Peatones con fuerzas sociales |
| `road_wear.rs` | 8973 | `RoadWearGrid`, `tick_road_wear()` | Desgaste acumulativo |

#### 💰 Economía

| Archivo | Líneas | Estructuras Clave | Función |
|---------|--------|-------------------|---------|
| `tax_system.rs` | 14524 | `TaxRates`, `MunicipalBond`, `CreditRating` | Impuestos, bonos, calificación |
| `land_value.rs` | 14547 | `LandValueHeatmap`, `PollutionHeatmap` | Difusión de valor + contaminación |
| `labor_market.rs` | 10369 | `LaborMarket`, `Worker`, `Employer` | Matching trabajador-empleo |
| `supply_chain.rs` | 16966 | `CargoTruck`, `SupplyChain` | Transporte de mercancías |

#### 🏙️ Sociedad y Servicios

| Archivo | Líneas | Estructuras Clave | Función |
|---------|--------|-------------------|---------|
| `politics.rs` | 18962 | `PoliticalSystem`, `NIMBY`, `Union` | Aprobación, vetos, huelgas |
| `utilities.rs` | 9562 | `WaterGrid`, `PowerGrid` | Propagación de servicios |
| `waste_mgmt.rs` | 15938 | `WasteManager`, `Landfill` | Clasificación, metano, napas |
| `customization.rs` | 18455 | `BuildingCustomizer` | Personalización visual |
| `climate.rs` | 6919 | `day_night_cycle()` | Ciclo día/noche |
| `terrain.rs` | 10366 | `TerrainMap`, ruido Perlin | Topografía |
| `persistence.rs` | 10872 | `SaveData`, `save_game()`, `load_game()` | Serialización |

#### 🔧 Utilidades

| Archivo | Líneas | Estructuras Clave | Función |
|---------|--------|-------------------|---------|
| `object_pool.rs` | 6653 | `EntityPool` | Object pooling masivo |
| `bump_alloc.rs` | 7126 | `FrameAllocator` | Bump allocator por frame |
| `rng_pool.rs` | 6851 | `RngPool`, `warm_rng_cache()` | RNG pre-generado |
| `bitboard.rs` | 12080 | `BitBoard` | Colisiones O(1) en grilla |
| `quadtree.rs` | 19986 | `QuadTree` | Índice espacial |
| `input.rs` | 9809 | `InputState`, `GameKey` | Input con debounce |
| `audio.rs` | 9019 | `AudioPlayer` | Audio procedural |
| `interactive.rs` | 38770 | `DesignTool`, `DesignMode` | Herramienta de diseño |

### Técnicas de Optimización Aplicadas (30 Técnicas)

| # | Técnica | Archivo | Descripción |
|---|---------|---------|-------------|
| TC#1 | Object Pooling Masivo | `object_pool.rs` | 10k entidades preasignadas, sin `new` en runtime |
| TC#2 | Pre-Reserva de Capacidad | Todos los `Vec` | `Vec::with_capacity` en toda asignación |
| TC#3 | Look-Up Tables Trigonométricas | `luts.rs` | Array `[f32; 3600]` precalculado |
| TC#5 | Árboles de Colisión Estáticos | `quadtree.rs` | Quadtree construido offline |
| TC#6 | Conversión JSON→Binario | `persistence.rs` | Bincode en lugar de JSON |
| TC#8 | Pre-Multiplicación de Matrices | `render.rs` | Transformaciones horneadas |
| TC#12 | Loop Unrolling Manual | `simd_render.rs` | Macros de desenrollado |
| TC#13 | Ruido Perlin Pre-generado | `terrain.rs` | Texturas de ruido estáticas |
| TC#16 | Culling Estático (PVS-like) | `render_cache.rs` | Pre-sort por capa |
| TC#18 | Eliminación de Closures | `main.rs` | Zero-allocation título |
| TC#20 | Variables Globales Mutables | `bump_alloc.rs` | Struct global pre-asignado |
| TC#21 | Distancias al Cuadrado | `quadtree.rs` | `dist_sq` sin sqrt |
| TC#22 | RNG Pool Pre-generado | `rng_pool.rs` | Bloque de randoms iterados |
| TC#23 | Sprites Pre-ordenados | `render_cache.rs` | Z-index estático |
| TC#25 | f32 sobre f64 | Todo el motor | `f32` exclusivo |
| TC#26 | Inlining Agresivo | `#[inline(always)]` | Funciones críticas |
| TC#27 | OffscreenCanvas equivalente | `main.rs` | Doble buffer con swap de punteros |
| TC#28 | get_unchecked | `luts.rs`, `flow_field.rs` | Accesos sin bounds check |

### Técnicas Avanzadas Aplicadas (20 Técnicas)

| # | Técnica | Archivo | Descripción |
|---|---------|---------|-------------|
| TA#1 | ECS Puro | `ecs.rs` | `hecs` con Struct of Arrays |
| TA#2 | LTO + PGO | `Cargo.toml` | `lto = "fat"`, `codegen-units = 1` |
| TA#5 | Físicas Deterministas | `sim.rs` | Paso fijo a 10Hz |
| TA#7 | Flow Fields Precalculados | `flow_field.rs` | Pathfinding O(1) |
| TA#8 | Caché Caliente Artificial | `main.rs` | Warming loops |
| TA#9 | SDF para Colisiones 2D | `bitboard.rs` | Bitboards precalculados |
| TA#10 | Decimación Condicional | `render.rs` | LOD según recursos |
| TA#14 | Hash Perfecto para Assets | `ecs.rs` | `HashMap` con `hasher` rápido |

---

## 📊 ESTADO ACTUAL DEL REFACTOR

### Lo que FUNCIONA ✅
- Compilación en Windows (minifb)
- ECS con hecs funcionando
- Renderizado software al framebuffer (32-bit color)
- Flow fields para pathfinding
- Sistema de tráfico con carriles (IDM + MOBIL)
- Sistema de impuestos y finanzas
- Valor del suelo con difusión
- Ciclo día/noche
- Save/Load con bincode
- Audio procedural con cpal
- Herramienta de diseño interactivo
- Pool de entidades y bump allocator

### Lo que FALTA ❌
- Muchos sistemas son stubs parciales
- La integración completa entre sistemas no está verificada
- Tests unitarios y de integración incompletos
- No hay UI avanzada (solo estadísticas básicas)
- Los edificios del diseño del usuario no están implementados

---

## 🔮 VISIÓN: LAS 200+ ESTRUCTURAS DISTÓPICAS

El usuario ha proporcionado un documento de diseño con más de 200 edificios/estructuras organizados en estas categorías:

1. **Corporaciones Buitre y Distopía Financiera** (10 edificios)
2. **Logística Extrema y Caos Vial** (10 edificios)
3. **Hidráulica Sádica y Desastres de Fluidos** (10 edificios)
4. **Mercado Negro Tecnológico y El Subsuelo** (10 edificios)
5. **Control Gubernamental y Miseria Pura** (10 edificios)
6. **Arcologías, Mega-Estructuras y Aislamiento** (10 edificios)
7. **Bioingeniería, Plagas y Contaminación 2.0** (10 edificios)
8. **Control Cognitivo, Vigilancia y Tiranía Digital** (10 edificios)
9. **Tecnología Fallida, Cultura Decadente y Transporte Absurdo** (10 edificios)
10. **Macro-Infraestructura Energética** (10 edificios)
11. **Infraestructura Vial y Nodos Logísticos** (10 edificios)
12. **Arquitectura Civil y Sistemas de Salud** (10 edificios)
13. **Instalaciones Militares y Defensa** (10 edificios)
14. **Manejo de Crisis y Remediación Ambiental** (10 edificios)
15. **Mega-Tránsito Subterráneo y Sistemas Hídricos** (5 edificios)
16. **Macro-Desarrollo Social y Burocracia Terminal** (5 edificios)
17. **Tecnología Aeroespacial y Satélites** (5 edificios)
18. **Gestión de Residuos Definitiva** (5 edificios)
19. **Colapso, Recuperación y Meta-Narrativa** (10 edificios)

---

## 📝 NOTAS PARA EL DESARROLLO

- El refactor compila con `cargo build --release` desde la carpeta `refactor/`
- Se requiere Rust nightly para algunas features SIMD (o stable con `std::arch`)
- La ventana usa `minifb` que requiere SDK de Windows en esta plataforma
- Los tests se ejecutan con `cargo test` desde `refactor/`
