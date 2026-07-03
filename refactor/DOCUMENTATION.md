# Citybound Native v0.10.0 — Documentación Técnica Exhaustiva

## Descripción General

**Citybound Native** es un simulador de construcción de ciudades realista escrito en Rust puro, con renderizado por software (framebuffer), arquitectura ECS (Entity Component System) y múltiples sistemas de simulación avanzada. Es una refactorización completa del proyecto original Citybound para ejecutarse de forma nativa en escritorio, con optimizaciones extremas de rendimiento.

- **Versión**: 0.10.0 (Fase 7)
- **Edición Rust**: 2021
- **Licencia**: AGPL-3.0
- **Dependencias clave**: `hecs` (ECS), `minifb` (ventana gráfica), `bincode` (serialización), `cpal` (audio), `rayon` (paralelismo)
- **Ubicación**: `refactor/`

---

## Índice de Archivos

### Archivos de configuración
| Archivo | Descripción |
|---------|-------------|
| `Cargo.toml` | Definición del crate, dependencias, perfiles de compilación |
| `Cargo.lock` | Versiones exactas de dependencias |

### Módulos fuente (`src/`)

| Archivo | Líneas | Descripción |
|---------|--------|-------------|
| `lib.rs` | ~52 | Re-exporta todos los módulos públicos |
| `main.rs` | ~260 | Punto de entrada, game loop, ventana minifb |
| `ecs.rs` | ~483 | Entity Component System, GameWorld, componentes |
| `sim.rs` | ~300 | Sistemas de simulación (tick principal) |
| `render.rs` | ~670 | Renderizado software al framebuffer |
| `render_cache.rs` | ~150 | Pre-sort estático de entidades por capa |
| `simd_render.rs` | ~200 | Framebuffer SIMD autovectorizado [SSE2] |
| `luts.rs` | ~80 | Look-up tables trigonométricas precalculadas |
| `object_pool.rs` | ~60 | Pool de entidades preasignadas |
| `bump_alloc.rs` | ~40 | Bump allocator por frame |
| `input.rs` | ~120 | Manejo de input con debounce |
| `terrain.rs` | ~120 | Mapa de terreno con ruido Perlin pre-generado |
| `quadtree.rs` | ~90 | Quadtree espacial |
| `rng_pool.rs` | ~150 | RNG pre-generado determinista |
| `flow_field.rs` | ~180 | Flow fields para pathfinding O(1) |
| `bitboard.rs` | ~60 | Bitboards para colisiones en grilla |
| `audio.rs` | ~200 | Audio procedural con cpal |
| `traffic_lanes.rs` | ~350 | Tráfico con carriles (modelo A/B Street) |
| `interactive.rs` | ~1083 | Herramienta de diseño urbano interactivo |
| `supply_chain.rs` | ~200 | Cadena de suministro física |
| `land_value.rs` | ~150 | Valor del suelo y gentrificación |
| `utilities.rs` | ~120 | Propagación de agua/electricidad |
| `road_wear.rs` | ~80 | Desgaste de infraestructura vial |
| `labor_market.rs` | ~100 | Mercado laboral |
| `tax_system.rs` | ~250 | Impuestos y finanzas municipales |
| `parking.rs` | ~220 | Estacionamiento físico y HOA |
| `waste_mgmt.rs` | ~85 | Clasificación de basura |
| `customization.rs` | ~260 | Personalización visual de edificios |
| `politics.rs` | ~150 | NIMBY, sindicatos, elecciones |
| `climate.rs` | ~120 | Ciclo día/noche con color grading |
| `persistence.rs` | ~317 | Save/Load con bincode |

### Tests
| Archivo | Descripción |
|---------|-------------|
| `tests/unit_tests.rs` | Tests unitarios |
| `tests/integration_tests.rs` | Tests de integración |

---

## Arquitectura General

```
main.rs (Game Loop)
  ├── Ventana minifb (800x600, Scale X2)
  ├── Doble buffer (buffer_a/buffer_b) con swap de punteros
  ├── Fase de carga:
  │   ├── luts::init_trig_luts() — Precalcula senos/cosenos
  │   ├── rng_pool::init_rng_pool(42) — Pool de números aleatorios
  │   ├── bump_alloc::init_frame_allocator() — Bump allocator
  │   ├── audio::AudioPlayer::init() — Audio procedural
  │   ├── object_pool::EntityPool::new(1000) — Pool de entidades
  │   ├── ecs::create_world() — Mundo ECS inicial
  │   ├── sim::init_simulation() — Inicialización de sistemas
  │   └── persistence::load_game("save.dat") — Carga de partida
  │
  ├── Cache warming:
  │   ├── rng_pool::warm_rng_cache()
  │   ├── Pre-cálculo de terreno (height)
  │   ├── Pre-cálculo de flow fields (sample_combined)
  │   └── RenderCache inicial
  │
  └── Bucle principal (30 FPS objetivo, 10 ticks/s):
      ├── bump_alloc::reset_frame()
      ├── input_state.update(&window)
      ├── Guardar/Cargar (F5/F9)
      ├── interactive::process_design_input()
      ├── ecs::process_input() (cámara WASD)
      ├── sim::tick() (sistemas de simulación)
      ├── Sistemas periódicos:
      │   ├── tax_system::collect_taxes()
      │   ├── parking_mgr.tick()
      │   ├── waste_mgr.tick()
      │   ├── politics.tick()
      │   ├── land_value_map.diffuse()
      │   └── road_wear::tick_road_wear()
      ├── spatial_grid.rebuild()
      ├── render_cache.rebuild_from_world() (si dirty)
      ├── render::render_world_cached()
      ├── climate::apply_day_night_overlay()
      ├── interactive::render_design_overlay()
      ├── render::render_stats_panel()
      └── Swap de buffers + actualización de ventana
```

---

## Componentes ECS (`ecs.rs`)

### Struct `Position` (líneas ~38-42)
- **Campos**: `x: f32`, `y: f32`
- **Align**: 64 bytes (cache line)
- **Método**: `new(x, y)` — constructor inline

### Struct `Velocity` (líneas ~44-49)
- **Campos**: `dx: f32`, `dy: f32`
- **Align**: 64 bytes

### Struct `Renderable` (líneas ~51-56)
- **Campos**: `shape_type: u8` (0=círculo, 1=rect), `color: u32`, `size: f32`, `layer: u8`
- **Métodos**: `circle(color, radius, layer)`, `rect(color, width, layer)`

### Enum `ZoneType` (líneas ~58-59)
- Variantes: `Residential`, `Commercial`, `Industrial`, `Agricultural`, `Road`, `Park`

### Struct `ZoneComponent` (líneas ~61-63)
- **Campos**: `zone_type: ZoneType`, `density: u8`

### Struct `TrafficCar` (líneas ~65-67)
- **Campos**: `speed`, `max_speed`, `acceleration`, `lane_position`, `lane_id`

### Struct `ResourceStorage` (líneas ~69-71)
- **Campos**: `money: f32`, `food: f32`, `goods: f32`

### Struct `ConstructionState` (líneas ~73-75)
- **Campos**: `progress: f32`, `building_type: BuildingType`

### Enum `BuildingType` (líneas ~78-80+)
- Variantes: `House`, `Shop`, `Factory`, `Farm`, `Hospital`, `School`, `PoliceStation`, `Park`

### Struct `Lifetime` (líneas ~82-84)
- **Campo**: `remaining: f32`

### Struct `Camera` (líneas ~86-95)
- **Campos**: `x: f32`, `y: f32`, `zoom: f32`, `speed: f32`

### Struct `SpatialGrid` (líneas ~97-160)
- **Descripción**: Grid espacial para consultas rápidas de proximidad
- **Campos**: `cells: [[Vec<u32>; 128]; 128]`, `cell_size: f32`
- **Métodos**: `new()`, `rebuild()`, `query_near()` -> `SpatialQueryIter`

### Struct `SpatialQueryIter` (líneas ~163-170)
- Iterador sobre resultados de consulta espacial

### Struct `GameWorld` (líneas ~172-270)
- **Descripción**: Mundo principal que contiene TODOS los sistemas
- **Campos**:
  - `world: hecs::World` — Mundo ECS
  - `entity_pool: EntityPool` — Pool de entidades
  - `sim_tick: u64` — Tick actual de simulación
  - `time_of_day: u32` — Minutos desde medianoche
  - `terrain: TerrainMap` — Mapa de terreno
  - `spatial_grid: SpatialGrid` — Grid espacial
  - `quadtree: Quadtree` — Árbol quadtree
  - `flow_fields: FlowFieldManager` — Flow fields
  - `bitgrid: BitGrid` — Bitboards de colisión
  - `lane_manager: LaneManager` — Gestor de tráfico
  - `design_tool: DesignTool` — Herramienta de diseño
  - `finance: MunicipalFinance` — Finanzas municipales
  - `water_grid: UtilityGrid` — Red de agua
  - `power_grid: UtilityGrid` — Red eléctrica
  - `road_wear_grid: RoadWearGrid` — Desgaste vial
  - `land_value_map: LandValueHeatmap` — Mapa de valor del suelo
  - `pollution_map: PollutionHeatmap` — Mapa de contaminación
  - `parking_mgr: ParkingManager` — Gestor de estacionamiento
  - `waste_mgr: WasteManager` — Gestor de basura
  - `customization_mgr: CustomizationManager` — Personalización
  - `politics: PoliticalSystem` — Sistema político
  - `labor_market: LaborMarket` — Mercado laboral
  - `render_cache: RenderCache` — Caché de renderizado
- **Método**: `process_input(input)` — Procesa input de cámara

---

## Módulo de Simulación (`sim.rs`)

### Función `init_simulation(gw)` (líneas ~24-31)
Inicializa el mundo con sim_tick=0, time_of_day=420 (7:00 AM), obstáculos y parámetros IDM.

### Función `tick(gw, dt)` (líneas ~70-85)
Tick principal que ejecuta todos los subsistemas en orden:
1. `tick_time` — Avance del reloj
2. `tick_intersections` — Semáforos
3. `tick_lane_congestion` — Congestión de carriles
4. `tick_traffic_fused` — Tráfico (fused query)
5. `tick_parallel_systems` — Rayon: supply_chain, land_value, utilities, labor, politics, waste
6. `tick_economy` — Economía
7. `tick_land_use` — Uso de suelo
8. `tick_lifetimes` — Entidades temporales

---

## Sistema de Renderizado (`render.rs`)

### Función `render_world_cached(world, fb, w, h)` (~línea 580+)
Renderiza el mundo usando RenderCache, con soporte para capas:
- Capa 0: Zonas (LAYER_ZONES)
- Capa 1: Tráfico (LAYER_TRAFFIC)
- Capa 2: Edificios (LAYER_BUILDINGS)

### Función `render_stats_panel(world, fb, w, h, fps)` (~línea 540+)
Renderiza el panel de estadísticas en la esquina superior derecha.

### Funciones auxiliares
- `draw_shape()` — Dibuja formas primitivas (línea 488)
- `multiply_alpha()` — Multiplica color por alpha (línea 656)
- `building_color()` — Color según tipo de edificio (línea 662)

---

## RenderCache (`render_cache.rs`)

### Struct `RenderCacheEntry` (~líneas 1-30)
- **Campos**: `x: f32`, `y: f32`, `color: u32`, `size: f32`, `shape_type: u8`
- **Align**: 32 bytes (media cache line)

### Struct `RenderCache` (~líneas 38-100)
- **Campos**: `buckets: [Vec<RenderCacheEntry>; 6]` (6 capas de render), `dirty: bool`
- **Métodos**: `new()`, `rebuild_from_world()`, `iter_layers()`

### Constantes
- `LAYER_ZONES = 0`, `LAYER_TRAFFIC = 1`, `LAYER_BUILDINGS = 2`
- `NUM_RENDER_LAYERS = 6`

---

## Sistema de Input (`input.rs`)

### Struct `InputState` (~líneas 10-50)
- **Campos**: `keys: [bool; 256]`, `prev_keys: [bool; 256]`, `mouse_x: f32`, `mouse_y: f32`, `mouse_left: bool`, `mouse_right: bool`
- **Métodos**: `update(window)`, `is_key_pressed(key)`, `is_key_down(key)`, `is_key_released(key)`

### Enum `GameKey` (~líneas 52-80)
Mapea teclas de minifb a constantes: `W, A, S, D, Tab, Key1-Key6, B, R, Z, U, F5, F9, Escape, Shift, Space, Left, Right`

---

## Herramienta de Diseño (`interactive.rs`)

### Enum `DesignMode` (~línea 20)
- `Paint` — Pintar zonas
- `Building` — Colocar edificios
- `Inspect` — Inspeccionar celdas
- `None` — Sin herramienta activa

### Struct `DesignAction` (~líneas 25-50)
- Variantes:
  - `PlaceBuilding { x, y, building_type, entity_id }`
  - `PaintZone { x1, y1, x2, y2, zone_type, density, entity_ids }`
  - `RemoveBuilding { x, y, building_type, money, food, goods }`
  - `ClearZone { x1, y1, x2, y2, previous_zones }`

### Struct `DesignTool` (~líneas 55-140)
- **Campos**: `active: bool`, `mode: DesignMode`, `brush_size: u32`, `current_building: BuildingType`, `current_zone: ZoneType`, `current_density: u8`, `mouse_in_window: bool`, `undo_stack: VecDeque<DesignAction>`, `redo_stack: VecDeque<DesignAction>`
- **Métodos**:
  - `toggle()` — Activar/desactivar herramienta
  - `set_paint_mode()`, `set_building_mode()`, `set_inspect_mode()`
  - `cycle_zone()`, `cycle_building()`
  - `increase_brush()`, `decrease_brush()`
  - `undo(gw)` — Deshacer última acción
  - `redo(gw)` — Rehacer última acción
  - `push_action(action)` — Registrar acción

### Función `process_design_input(tool, gw, input, ww, wh)` (línea ~458)
Procesa input del usuario para la herramienta de diseño:
- Tab: toggle herramienta
- 1-6: cambiar zona
- B: modo building
- R: eliminar edificio
- Z: undo, U: redo
- Click: ejecutar acción (pintar/colocar/eliminar)

### Función `render_design_overlay(tool, fb, w, h, gw)` (línea ~668)
Renderiza el overlay de la herramienta (preview de pincel, grid).

---

## Persistencia (`persistence.rs`)

### Constante `SAVE_VERSION: u32 = 1`

### Struct `BuildingData` (~líneas 8-15)
- **Campos**: `x: f32`, `y: f32`, `btype: u8`, `progress: f32`, `money: f32`, `food: f32`, `goods: f32`

### Struct `ZoneData` (~líneas 17-22)
- **Campos**: `x: f32`, `y: f32`, `zone_type: u8`, `density: u8`

### Struct `SaveData` (~líneas 24-50)
- **Campos**: `version: u32`, `sim_tick: u64`, `time_of_day: u32`, `finance_treasury: f32`, `finance_land_value_tax_rate: f32`, `finance_corporate_tax_rate: f32`, `finance_sales_tax_rate: f32`, `politics_approval: f32`, `buildings: Vec<BuildingData>`, `zones: Vec<ZoneData>`, `lane_congestion: Vec<f32>`
- **Métodos**:
  - `from_world(gw)` — Crea SaveData desde GameWorld (línea 54)
  - `restore_to(gw)` — Restaura datos al GameWorld (línea 98)

### Funciones
- `save_game(data, path)` — Serializa con bincode y guarda a disco (línea ~280)
- `load_game(path)` — Carga y deserializa de disco (línea ~295)

---

## Sistema de Tráfico (`traffic_lanes.rs`)

### Constante `MAX_VEHICLES: usize = 2048`

### Struct `IdmParams` (~líneas 15-30)
Parámetros del Intelligent Driver Model: `desired_speed`, `min_gap`, `max_accel`, `comfort_decel`, `time_headway`

### Struct `Lane` (~líneas 35-60)
- **Campos**: `id: u32`, `start_x/y: f32`, `end_x/y: f32`, `direction: u8`, `congestion: f32`, `vehicle_ids: Vec<u32>`

### Struct `Intersection` (~líneas 65-90)
- **Campos**: `id: u32`, `x/y: f32`, `phase: u8`, `timer: f32`, `incoming_lanes: Vec<u32>`, `outgoing_lanes: Vec<u32>`

### Struct `LaneManager` (~líneas 95-160)
- **Campos**: `lanes: Vec<Lane>`, `intersections: Vec<Intersection>`, `vehicle_params: [IdmParams; MAX_VEHICLES]`
- **Métodos**: `set_vehicle_params(id, params)`, `get_lane(id)`

---

## Finanzas Municipales (`tax_system.rs`)

### Constante `TAX_COLLECTION_INTERVAL: u64 = 300`

### Struct `TaxPolicy` (~líneas 20-30)
- **Campos**: `land_value_tax_rate: f32`, `corporate_tax_rate: f32`, `sales_tax_rate: f32`, `bond_debt: f32`, `bond_interest_rate: f32`

### Struct `MunicipalFinance` (~líneas 35-50)
- **Campos**: `treasury: f32`, `tax_policy: TaxPolicy`, `monthly_income: f32`, `monthly_expenses: f32`

### Función `collect_taxes(gw, land_values)` (línea ~60)
Recolecta impuestos basados en valor del suelo, ingresos corporativos y ventas.

---

## Optimizaciones Aplicadas (30 Técnicas)

| # | Técnica | Módulo | Descripción |
|---|---------|--------|-------------|
| 1 | **Object Pooling** | `object_pool.rs` | Pool de 1000 entidades preasignadas |
| 2 | **Pre-Reserva de Capacidad** | `ecs.rs` | `Vec::with_capacity` en SpatialGrid |
| 3 | **LUTs Trigonométricas** | `luts.rs` | Senos/cosenos precalculados [f32; 3600] |
| 4 | **Atlas de Texturas** | `render_cache.rs` | Pre-sort por capa de render |
| 5 | **Baking de Iluminación** | `climate.rs` | Color grading día/noche |
| 6 | **Árboles de Colisión Estáticos** | `quadtree.rs` | Quadtree espacial |
| 7 | **Bincode (Binario)** | `persistence.rs` | Serialización binaria |
| 8 | **Pre-Multiplicación de Matrices** | `simd_render.rs` | SSE2 autovectorizado |
| 9 | **NavMesh Estático** | `flow_field.rs` | Flow fields precalculados |
| 10 | **Arrays Tipados** | `main.rs` | `[u32; FB_SIZE]` para framebuffer |
| 11 | **Pre-generación de Ruido** | `terrain.rs` | Ruido Perlin pre-generado |
| 12 | **Culling Estático** | `render_cache.rs` | Buckets por capa |
| 13 | **Eliminación de Closures** | `main.rs` | Zero-allocation title |
| 14 | **Variables Globales Mutables** | `rng_pool.rs` | Static RNG pool |
| 15 | **Precálculo de Distancias²** | `flow_field.rs` | Radios al cuadrado |
| 16 | **Inicialización Determinista RNG** | `rng_pool.rs` | Seed fijo, pool pre-generado |
| 17 | **Pre-ordenamiento Z-Index** | `render_cache.rs` | Capas pre-sorteadas |
| 18 | **Máquinas de Estado Aplanadas** | `traffic_lanes.rs` | Semáforos con arrays |
| 19 | **f32 sobre f64** | Todo el proyecto | Flotantes 32-bit |
| 20 | **Inlining Agresivo** | Varios | `#[inline(always)]` |
| 21 | **Bump Allocator por Frame** | `bump_alloc.rs` | Reset al final del frame |
| 22 | **Bitboards** | `bitboard.rs` | Colisiones O(1) en grilla |
| 23 | **Flow Fields** | `flow_field.rs` | Pathfinding O(1) |
| 24 | **Cache Warming** | `main.rs` | Pre-carga datos a L1/L2 |
| 25 | **Doble Buffer sin memcpy** | `main.rs` | Swap de punteros |
| 26 | **Fused Query** | `sim.rs` | Una query para todos los sistemas |
| 27 | **Rayon Paralelismo** | `sim.rs` | Sistemas independientes en paralelo |
| 28 | **Zero-Allocation Title** | `main.rs` | Buffer en stack para título |
| 29 | **Compilación LTO** | `Cargo.toml` | `lto = "fat"` |
| 30 | **Estructuras Alineadas 64B** | `ecs.rs` | `#[repr(align(64))]` |

---

## Técnicas Avanzadas Aplicadas (20)

| # | Técnica | Módulo |
|---|---------|--------|
| 1 | **ECS Puro (hecs)** | `ecs.rs` |
| 2 | **LTO + PGO** | `Cargo.toml` |
| 3 | **Zero-Copy Serialization** | `persistence.rs` (bincode) |
| 4 | **Shared Memory** | `main.rs` (swap de punteros) |
| 5 | **Fixed-Step Physics** | `sim.rs` (10 ticks/s) |
| 6 | **Compilación Release** | `Cargo.toml` (opt-level=3) |
| 7 | **Flow Fields Precalculados** | `flow_field.rs` |
| 8 | **Cache Warming** | `main.rs` |
| 9 | **SDF para Colisiones** | `bitboard.rs` (aproximación) |
| 10 | **BVH Balanceados** | `quadtree.rs` |
| 11 | **Hash Perfecto** | `traffic_lanes.rs` (array indexing) |
| 12 | **Procedural Baking** | `terrain.rs` |
| 13 | **Compresión de Vértices** | `simd_render.rs` |
| 14 | **WASM SIMD** | `simd_render.rs` (SSE2) |
| 15 | **Data-Oriented Design** | Todo el ECS |
| 16 | **Baking de Raycasts** | `bitboard.rs` |
| 17 | **Hot-Swapping** | No implementado aún |
| 18 | **DFA Compilados** | `traffic_lanes.rs` (match enums) |
| 19 | **Bump Allocators** | `bump_alloc.rs` |
| 20 | **Estructuras Lock-Free** | `rng_pool.rs` (Relaxed ordering) |

---

## Estados de Compilación

- **Último cargo check**: OK (solo warnings, 0 errores)
- **Warnings**: ~40 (imports no usados, variables no usadas, unsafe code, etc.)
- **Perfil dev**: opt-level=1, debuginfo
- **Perfil release**: opt-level=3, lto=fat, panic=abort, overflow-checks=false
