# Citybound Native v0.15.0 — Documentación Técnica Exhaustiva

> **"Un simulador de ciudades ultra-realista donde cada detalle importa."**
>
> Citybound Native es un simulador de construcción de ciudades realista escrito en Rust puro,
> con renderizado por software (framebuffer), GPU backend adaptativo, arquitectura ECS,
> 150+ tipos de edificios distópicos, y 35+ sistemas de simulación avanzada con
> fundamento matemático documentado.
>
> **Última compilación limpia:** ✅ 0 errores, 0 warnings (cargo check 2025-06-25)
> **Versión:** 0.15.0 | **Rust:** 1.96.0 stable | **Edición:** 2021
> **Plataformas:** Windows, Android, macOS, Linux
```
┌──────────────────────────────────────────────────────────────────────┐
│                        MAIN GAME LOOP                               │
│  main.rs: init → load → loop(tick, render, input) → save → exit    │
└──────────────────────────────┬───────────────────────────────────────┘
                               │
            ┌──────────────────┼──────────────────┐
            ▼                  ▼                  ▼
    ┌───────────┐     ┌───────────────┐   ┌──────────────┐
    │  INPUT    │     │  SIMULACIÓN   │   │   RENDER     │
    │ input.rs  │     │   sim.rs      │   │ render.rs    │
    │ ecs.rs    │     │ (tick loop)   │   │ render_cache │
    └─────┬─────┘     └───────┬───────┘   │ simd_render  │
          │                   │           │ luts.rs      │
          ▼                   │           └──────────────┘
    ┌───────────┐             │
    │interactive│             │
    │  design   │             │
    └───────────┘             │
                              │
    ┌─────────────────────────┼─────────────────────────┐
    │                         ▼                         │
    │              SISTEMAS DE SIMULACIÓN               │
    │  (ejecutados por sim.rs en orden cada tick)       │
    └───────────────────────────────────────────────────┘
                              │
     ┌────────────────────────┼────────────────────────┐
     │                        │                        │
     ▼                        ▼                        ▼
┌──────────┐          ┌──────────────┐          ┌──────────┐
│ ECONOMÍA │          │ MOVILIDAD    │          │ ENTORNO  │
├──────────┤          ├──────────────┤          ├──────────┤
│tax_system│◄────────►│traffic_lanes │◄────────►│climate   │
│finance   │          │flow_field    │          │terrain   │
│land_value│          │parking       │          │utilities │
│labor_mrkt│          │pedestrian    │          │waste_mgmt│
│supply_ch │          │lane_manager  │          │pollution │
└────┬─────┘          └──────┬───────┘          └────┬─────┘
     │                       │                       │
     └───────────────────────┼───────────────────────┘
                             │
                             ▼
                    ┌────────────────┐
                    │    SOCIEDAD    │
                    ├────────────────┤
                    │politics.rs     │
                    │ - NIMBY        │
                    │ - sindicatos   │
                    │ - elecciones   │
                    │ - vetos/recall │
                    │customization   │
                    │persistence     │
                    └────────────────┘
```

### Matriz de Interacción entre Sistemas

```
                TAX LND ROAD UTIL LAB SUP PARK WAST POL CLIM PED FLOW TRAF
TAX (impuestos)  ●   ↑   ·    ·   ↑   ↑   ·    ·   ↑   ·    ·   ·    ·
LND (valor suelo) ↓   ●   ·    ·   ·   ·   ↑    ↓   ↑   ·    ·   ·    ↓
ROAD (desgaste)   ·   ·   ●    ·   ·   ·   ·    ·   ·   ·    ·   ·    ↑
UTIL (servicios)  ·   ·   ·    ●   ·   ·   ·    ↑   ·   ·    ·   ·    ·
LAB (laboral)     ↑   ·   ·    ·   ●   ·   ·    ·   ·   ·    ·   ↓    ·
SUP (supply ch)   ·   ·   ·    ·   ·   ●   ·    ·   ·   ·    ·   ·    ↑
PARK (parking)    ·   ·   ·    ·   ·   ·   ●    ·   ↑   ·    ·   ·    ↑
WAST (basura)     ·   ·   ·    ↓   ·   ·   ·    ●   ↑   ·    ·   ·    ·
POL (política)    ↑   ·   ·    ·   ·   ·   ↑    ·   ●   ·    ·   ·    ·
CLIM (clima)      ·   ·   ·    ·   ·   ·   ·    ·   ·   ●    ·   ·    ·
PED (peatones)    ·   ·   ·    ·   ·   ·   ·    ·   ·   ·    ●   ↑    ·
FLOW (flow field) ·   ·   ·    ·   ·   ·   ·    ·   ·   ·    ↓   ●    →
TRAF (tráfico)    ·   ·   →    ·   ·   →   →    ·   ·   ·    ·   ·    ●

Simbología:
  ● = sistema consigo mismo (difusión, evolución temporal)
  ↑ = afecta/se ve afectado por
  ↓ = es afectado por
  → = fluye hacia (dependencia de datos)
  · = sin interacción directa
```

---

## 📐 MODELOS MATEMÁTICOS

### 1. Impuestos y Finanzas Municipales (`tax_system.rs`)

#### 1.1 Recaudación Total (por período T = 300 ticks)

```
R_total = Σ R_land + Σ R_corp + Σ R_sales + Σ R_tolls
```

#### 1.2 Impuesto sobre el Valor del Suelo (Land Value Tax)

Para cada parcela `i` con valor del suelo `V_i`:

```
R_land = Σ (τ_land · V_i · A_i / 100)
```

Donde:
- `τ_land` ∈ [0.0, 0.15]: tasa del impuesto (típico 0.5%–2%)
- `V_i`: valor del suelo (de `LandValueHeatmap.values[i]`)
- `A_i`: área de la parcela en celdas

#### 1.3 Impuesto a la Renta Corporativa

Para cada corporación `j` con ingresos `I_j` y costos `C_j`:

```
G_j = max(0, I_j − C_j)               // Ganancia neta
R_corp = Σ (τ_corp · G_j)
```

Donde `τ_corp` ∈ [0.0, 0.35] (típico 15%–28%).

#### 1.4 Impuesto al Consumo Local (Sales Tax)

```
R_sales = τ_sales · Σ (consumo_total_edificios_comerciales)
```

Donde `τ_sales` ∈ [0.0, 0.15].

#### 1.5 Peajes Dinámicos (Dynamic Tolls)

Los peajes varían con la hora del día:

```
τ_toll(h) = τ_base · f(h)

f(h) = {
  0.30  si 0 ≤ h < 6     (madrugada, bajo tráfico)
  1.00  si 6 ≤ h < 9     (hora pico AM)
  0.70  si 9 ≤ h < 17    (horas laborales)
  1.20  si 17 ≤ h < 20   (hora pico PM)
  0.50  si 20 ≤ h < 24   (noche)
}
```

#### 1.6 Bonos Municipales y Calificación Crediticia

Emisión de bono de valor `B` con interés `r` y madurez `M` ticks:

```
Pago_interés_período = B · r / (M / TAX_COLLECTION_INTERVAL)
```

La calificación crediticia se modela como:

```
credit_rating(t+1) = clamp(
  credit_rating(t) + α·(superávit_relativo) − β·(deuda/PBI) − γ·(desempleo),
  0.0, 1.0
)
```

Donde:
- `α = 0.02`: sensibilidad al superávit
- `β = 0.05`: penalización por deuda alta (>60% PBI cae rating)
- `γ = 0.03`: penalización por desempleo
- Umbrales: AAA (>0.85), BBB (>0.60), BB (>0.40), B (>0.25), CCC (>0.10), D

---

### 2. Valor del Suelo y Gentrificación (`land_value.rs`)

#### 2.1 Ecuación de Difusión (Heat Equation Discretizada)

El valor del suelo sigue una ecuación de difusión 2D con fuentes/sumideros:

```
∂V/∂t = D · ∇²V + S(x,y) − δ · V + β_park · P(x,y) − β_poll · C(x,y) − β_ind · I(x,y)
```

Discretización (Forward Euler, grilla 128×128):

```
V[i,j]^(t+1) = V[i,j]^t + D·Δt · [V[i+1,j] + V[i-1,j] + V[i,j+1] + V[i,j-1] − 4·V[i,j]]^t
              + S[i,j]·Δt − δ·V[i,j]^t·Δt
              + β_park·P[i,j]·Δt − β_poll·C[i,j]·Δt − β_ind·I[i,j]·Δt
```

Parámetros:
- `D = 0.15` (DIFFUSION_RATE): coeficiente de difusión
- `δ = 0.001`: decaimiento natural
- `β_park = 0.05` (PARK_VALUE_BOOST): bonus por parque cercano
- `β_poll = 0.03` (POLLUTION_VALUE_PENALTY): penalización por contaminación
- `β_ind = 0.02` (INDUSTRIAL_VALUE_PENALTY): penalización industrial
- `S(x,y)`: fuentes (edificios de alto valor, proximidad a servicios)

#### 2.2 Gentrificación

Un residente es desplazado si:

```
V_celda > GENTRIFICATION_THRESHOLD · income_residente
```

Donde `GENTRIFICATION_THRESHOLD = 1.5`.

El desplazamiento produce:
1. El residente migra a la periferia (celda con menor V y contaminación presente)
2. El edificio original se reemplaza por uno de mayor densidad/valor
3. Aumenta la desigualdad espacial (índice Gini de valores del suelo)

#### 2.3 Contaminación (Pollution Heatmap)

La contaminación sigue la misma ecuación de difusión con fuente industrial:

```
∂C/∂t = D_poll · ∇²C + Q_factories(x,y) − λ_decay · C
```

- `D_poll = 0.10`: difusión de contaminantes
- `λ_decay = 0.001` (POLLUTION_DECAY): decaimiento natural
- `Q_factories`: emisión de fábricas (proporcional a producción)

---

### 3. Desgaste de Infraestructura (`road_wear.rs`)

#### 3.1 Modelo de Desgaste Acumulativo

Para cada celda de la grilla vial (128×128):

```
W[x,y]^(t+1) = W[x,y]^t + Σ(w_car · n_cars + w_truck · n_trucks) − r_natural · Δt
```

Donde:
- `w_car = 0.001`: desgaste por coche normal
- `w_truck = 0.01`: desgaste por camión (10× más dañino)
- `r_natural = 0.0001` (NATURAL_REPAIR): reparación natural por tick
- `W ∈ [0, MAX_WEAR=100]`: nivel de desgaste (0=perfecto, 100=destruido)

#### 3.2 Efecto en Velocidad del Tráfico

```
speed_factor(W) = 1.0 − (W − DAMAGE_THRESHOLD) / (MAX_WEAR − DAMAGE_THRESHOLD) · MAX_SPEED_PENALTY
```

Donde:
- `DAMAGE_THRESHOLD = 30.0`: punto donde empieza a afectar
- `MAX_SPEED_PENALTY = 0.6`: reducción máxima (60% más lento)
- `speed_factor ∈ [0.4, 1.0]`

El flow field local se multiplica por `speed_factor` para reducir la velocidad efectiva.

---

### 4. Propagación de Utilidades (`utilities.rs`)

#### 4.1 Modelo de Presión/Voltaje por Distancia Manhattan

Para una grilla 32×32 (cada celda = 4×4 del mundo):

```
P[x,y] = P_source − PRESSURE_LOSS_PER_CELL · d_manhattan(source, [x,y])
```

Donde:
- `P_source = 100.0` (MAX_PRESSURE)
- `PRESSURE_LOSS_PER_CELL = 5.0`
- `d_manhattan = |x − sx| + |y − sy|`

#### 4.2 Condición de Funcionamiento

Un edificio en `(x,y)` funciona si:

```
P[x/4, y/4] ≥ MIN_PRESSURE_THRESHOLD
```

Donde `MIN_PRESSURE_THRESHOLD = 20.0`.

Si no se alcanza el umbral:
- Sin agua: `ResourceStorage.food` se reduce (-10% por tick)
- Sin electricidad: producción cae al 50%

---

### 5. Mercado Laboral (`labor_market.rs`)

#### 5.1 Matching Trabajador-Empleo

Un trabajador desempleado `w` puede aceptar empleo en empresa `e` si:

```
d_euclidean(w.pos, e.pos) ≤ MAX_COMMUTE_DISTANCE
```

Y se asigna al empleador más cercano con vacantes. El matching es voraz (greedy).

#### 5.2 Despido y Abandono

- Si un trabajador pasa `MAX_JOB_SEARCH_TICKS = 200` desempleado → probabilidad de abandono
- Si un empleador pasa `MAX_UNSTAFFED_TICKS = 400` sin trabajadores → probabilidad de cierre `ABANDONMENT_CHANCE = 0.002` por tick

#### 5.3 Commute

Cada trabajador empleado verifica diariamente que su workplace sigue existiendo y accesible. Si el edificio colapsa o es demolido, el trabajador queda desempleado.

---

### 6. Cadena de Suministro (`supply_chain.rs`)

#### 6.1 Producción y Consumo

```
Producción_fábrica = FACTORY_PRODUCTION = 5.0 unidades/tick
Consumo_tienda = SHOP_CONSUMPTION = 1.0 unidades/tick
```

#### 6.2 Transporte en Camión

Un camión de carga transporta `CARGO_CAPACITY = 20.0` unidades a velocidad `CARGO_TRUCK_SPEED = 6.0`. Sigue flow fields hacia el destino.

#### 6.3 Quiebra

Si una tienda pasa `BANKRUPTCY_TICKS = 300` sin recibir suministro:

```
P(quiebra | sin_stock_ticks > BANKRUPTCY_TICKS) = 0.01 por tick adicional
```

El edificio queda marcado como `AbandonedBuilding` y deja de generar ingresos.

---

### 7. Gestión de Residuos (`waste_mgmt.rs`)

#### 7.1 Clasificación

Cada edificio genera residuos según su tipo:

| Tipo edificio | Orgánico | Reciclable | Tóxico | General |
|---------------|----------|------------|--------|---------|
| House         | 0.5      | 0.3        | 0.05   | 0.2     |
| Shop          | 0.3      | 0.5        | 0.05   | 0.3     |
| Factory       | 0.1      | 0.3        | 0.4    | 0.3     |
| Farm          | 2.0      | 0.2        | 0.1    | 0.1     |
| Hospital      | 0.4      | 0.3        | 0.5    | 0.5     |

Unidades: kg/tick.

#### 7.2 Acumulación de Metano en Vertederos

```
CH4(t+1) = CH4(t) + k_org · organic_kg − k_vent · ventilation_factor
```

Donde:
- `k_org = 0.01`: tasa de generación de metano por kg orgánico
- `k_vent = 0.05`: tasa de ventilación (si tiene sistema)
- `ventilation_factor = 1.0` si `has_methane_ventilation`, sino `0.0`

Si `CH4 > 80.0`: peligro de explosión. Probabilidad de explosión:

```
P(explosion) = (CH4 − 80) / 100 · 0.001 por tick
```

#### 7.3 Contaminación de Napas Freáticas

```
GW_contamination(t+1) = GW_contamination(t) + k_tox · toxic_kg · (1 − geomembrane_factor)
```

Donde:
- `geomembrane_factor = 1.0` si tiene geomembrana, `0.0` si no
- `k_tox = 0.005`

Si `GW_contamination > 50.0`: el agua de la zona no es potable, causa enfermedades.

---

### 8. Política y Sociología (`politics.rs`)

#### 8.1 Aprobación Global

```
approval(t+1) = approval(t) + Δ_services − Δ_taxes − Δ_nimby − Δ_gentrification + Δ_unions
```

Donde cada delta se calcula como:

```
Δ_services = α_s · (calidad_servicios − umbral)
Δ_taxes = −α_t · max(0, tasa_efectiva − 0.25)
Δ_nimby = −α_n · Σ(unwanted_facilities_cercanas) / total_ciudadanos
Δ_gentrification = −α_g · fracción_desplazados
Δ_unions = −α_u · huelgas_activas
```

#### 8.2 Veto del Concejo

Si `approval < VETO_THRESHOLD = 0.35` en ≥ 5/9 distritos:
- Presupuestos bloqueados (no se pueden modificar tasas)
- Obras públicas suspendidas

#### 8.3 Destitución (Recall)

Si `approval < RECALL_THRESHOLD = 0.15` en ≥ 7/9 distritos:
- Game over político: la simulación termina

#### 8.4 NIMBY

Para una instalación no deseada F en posición (x_F, y_F):

```
molestia_ciudadano = Σ nuisance_radius(F) / distancia(ciudadano, F)²
```

Si `molestia > umbral_nimby`: el ciudadano se une a protestas, reduce aprobación, y bloquea calles cercanas.

---

### 9. Tráfico y Flow Fields (`flow_field.rs`, `traffic_lanes.rs`)

#### 9.1 Flow Field

Para cada celda de la grilla (64×64):

```
F[x,y] = vector_unitario_hacia_destino_más_cercano
```

Combinación de flow fields:

```
F_combined = (1 − w_congestion) · F_base + w_congestion · F_alternate
```

Donde `w_congestion = lane.congestion` (0 a 1).

#### 9.2 Intelligent Driver Model (IDM)

Para un vehículo con velocidad `v`, distancia al líder `s`, y velocidad de aproximación `Δv`:

```
dv/dt = a · [1 − (v/v0)^δ − (s*(v,Δv)/s)²]

donde s*(v,Δv) = s0 + v·T + v·Δv/(2√(a·b))
```

Parámetros:
- `a = max_accel`: aceleración máxima
- `b = comfort_decel`: deceleración confortable
- `v0 = desired_speed`: velocidad deseada
- `s0 = min_gap`: distancia mínima
- `T = time_headway`: tiempo de reacción
- `δ = 4`: exponente de aceleración

#### 9.3 MOBIL Lane Changes

Un vehículo cambia de carril si:

```
Δa_ego + p_mobil · Δa_others > a_threshold
```

Donde:
- `Δa_ego`: cambio en aceleración del vehículo
- `Δa_others`: cambio en aceleración de vehículos afectados
- `p_mobil = 0.5`: politeness factor
- `a_threshold = 0.2`: umbral de cambio

---

### 10. Ciclo Día/Noche (`climate.rs`)

#### 10.1 Función de Color Grading

```
(r_mul, g_mul, b_mul) = f(time_fraction)

f(h) = {
  (1.00, 0.85+0.15t, 0.70+0.30t)  si 5≤h<7   (amanecer, t=(h-5)/2)
  (1.00, 1.00, 1.00)               si 7≤h<18  (día pleno)
  (1.00, 0.70+0.30(1-t), 0.50+0.50(1-t)) si 18≤h<20 (atardecer)
  (0.25, 0.25+0.10t, 0.40+0.30t)   si h≥20 o h<5 (noche)
}
```

---

### 11. Estacionamiento (`parking.rs`)

#### 11.1 Modelo de Búsqueda

```
P(encuentra_parking) = {
  1.0  si available_spots > 0 en edificio destino
  0.8  si hay calle con espacio < 20m
  0.3  si solo hay calle lejana
  0.0  si todo lleno → circling_cars++
}
```

#### 11.2 Factor de Congestión por Parking

```
congestion_factor = min(circling_cars · 0.01, 0.8)
```

Reduce la velocidad del tráfico en el radio de influencia.

#### 11.3 Asociación de Vecinos (HOA)

Las zonas HOA aplican reglas adicionales:

```
violación = {
  true si (no_street_parking Y estacionó_en_calle)
  true si (overnight_restriction Y hora ∈ [21:00, 06:00] Y estacionó_fuera_de_garaje)
  true si (visitor_permit_required Y es_visitante Y sin_permiso)
  true si (!allow_commercial_vehicles Y es_vehículo_comercial)
  true si (sidewalk_parking_prohibited Y estacionó_en_vereda)
}
```

Multas:
```
multa = violation_fine · (1 + reincidencia · 0.5)
```

Juicios:
- Si un ciudadano acumula >3 multas impagas → demanda legal
- Costo del juicio para la ciudad: `legal_cost = 500 + complexity · 200`
- Si la HOA pierde el juicio → se disuelve temporalmente (180 ticks)

---

### 12. Peatones y Flujo Peatonal (`pedestrian.rs`) 🆕

#### 12.1 Modelo de Fuerza Social (Social Force Model)

Cada peatón `p` está sujeto a fuerzas:

```
F_p = F_destino + Σ F_rep_peatones + F_rep_obstáculos + F_atracción + F_aleatoria
```

**Fuerza hacia el destino:**

```
F_destino = (1/τ) · (v0 · e_destino − v_actual)
```

Donde:
- `τ = 0.5s`: tiempo de relajación
- `v0 = 1.34 m/s`: velocidad deseada (≈ 4.8 km/h, caminata normal)
- `e_destino`: vector unitario hacia el destino

**Fuerza de repulsión entre peatones:**

```
F_rep_ij = A · exp[(r_ij − d_ij)/B] · n_ij · λ + k · g(r_ij − d_ij) · n_ij
```

Donde:
- `A = 2.0`: intensidad de repulsión
- `B = 0.3m`: rango de interacción
- `r_ij = r_i + r_j`: suma de radios corporales (≈ 0.4m)
- `d_ij`: distancia entre peatones
- `λ = 0.7`: anisotropía (peatones detrás = menos influencia)
- `k = 120 kg/s²`: constante de compresión
- `g(x) = max(0, x)`: función rampa

#### 12.2 Densidad de Multitud y Velocidad

La velocidad efectiva se reduce con la densidad:

```
v_eff(ρ) = v0 · max(0.1, 1 − ρ/ρ_max)
```

Donde:
- `ρ`: densidad local (peatones/m²)
- `ρ_max = 5.4`: densidad de atasco
- En condiciones normales: `v_eff ≈ 1.3 m/s`
- En hora pico: `v_eff ≈ 0.6 m/s`

#### 12.3 Cruce de Calles

Un peatón decide cruzar si:

```
gap > T_cruce · v_vehículo
```

Donde:
- `gap`: espacio libre en la calle
- `T_cruce = ancho_calle / v0_peatón`: tiempo necesario para cruzar
- `v_vehículo`: velocidad del vehículo más cercano

Semáforos peatonales:
```
P(cruzar | semáforo) = {
  1.0  si luz_peatonal = verde
  0.2  si luz_peatonal = roja (imprudencia)
  0.5  si no hay semáforo
}
```

#### 12.4 Generación de Viajes Peatonales

```
viajes_desde_edificio(t, tipo) = {
  house → shop:    0.15 · n_residentes · factor_hora(h)
  house → work:    0.30 · n_residentes · factor_hora(h)
  house → park:    0.05 · n_residentes · factor_hora(h)
  shop → shop:     0.08 · n_visitantes · factor_hora(h)
  office → shop:   0.12 · n_trabajadores · factor_hora(h) (almuerzo)
  hospital → *:    0.20 · n_pacientes
  school → *:      0.25 · n_estudiantes (salida)
}

factor_hora(h) = {
  0.05  si 0≤h<5    (noche)
  0.30  si 5≤h<7    (temprano)
  1.00  si 7≤h<9    (hora pico AM)
  0.40  si 9≤h<12   (mañana)
  0.60  si 12≤h<14  (almuerzo)
  0.40  si 14≤h<17  (tarde)
  1.00  si 17≤h<19  (hora pico PM)
  0.30  si 19≤h<22  (noche temprano)
  0.10  si 22≤h<24  (noche)
}
```

#### 12.5 Efecto en la Ciudad

- **Comercio**: más peatones = más ventas en tiendas (+0.1% por peatón/hora)
- **Tráfico**: los peatones en cruces reducen velocidad vehicular (factor 0.85 si cruce activo)
- **Atropellos**: si `gap < 2.0m` y peatón cruza, probabilidad de accidente
- **Percepción de seguridad**: calles con peatones = -5% criminalidad local

---

## 📁 ÍNDICE DE ARCHIVOS

| Archivo | Líneas | Descripción |
|---------|--------|-------------|
| `Cargo.toml` | ~70 | Definición del crate, dependencias, perfiles |
| `lib.rs` | ~52 | Re-exporta todos los módulos |
| `main.rs` | ~310 | Game loop, ventana, time warp |
| `ecs.rs` | ~475 | ECS, GameWorld, componentes |
| `sim.rs` | ~300 | Tick principal de simulación |
| `render.rs` | ~670 | Renderizado software |
| `render_cache.rs` | ~150 | Pre-sort por capa |
| `simd_render.rs` | ~200 | SIMD autovectorizado |
| `luts.rs` | ~80 | LUTs trigonométricas |
| `object_pool.rs` | ~60 | Pool de entidades |
| `bump_alloc.rs` | ~40 | Bump allocator |
| `input.rs` | ~120 | Input con debounce |
| `terrain.rs` | ~120 | Ruido Perlin pre-generado |
| `quadtree.rs` | ~586 | BVH balanceado |
| `rng_pool.rs` | ~150 | RNG determinista |
| `flow_field.rs` | ~180 | Flow fields O(1) |
| `bitboard.rs` | ~60 | Bitboards para colisiones |
| `audio.rs` | ~200 | Audio procedural cpal |
| `traffic_lanes.rs` | ~350 | Tráfico con carriles IDM+MOBIL |
| `pedestrian.rs` | ~300 | Peatones: social force model, cruces, multitudes 🆕 |
| `interactive.rs` | ~1083 | Herramienta de diseño |
| `supply_chain.rs` | ~200 | Cadena de suministro |
| `land_value.rs` | ~150 | Valor del suelo |
| `utilities.rs` | ~120 | Agua/electricidad |
| `road_wear.rs` | ~80 | Desgaste vial |
| `labor_market.rs` | ~100 | Mercado laboral |
| `tax_system.rs` | ~250 | Impuestos y bonos |
| `parking.rs` | ~320 | Estacionamiento + HOA + vereda |
| `waste_mgmt.rs` | ~85 | Gestión de basura |
| `customization.rs` | ~260 | Personalización visual |
| `politics.rs` | ~150 | NIMBY, sindicatos, elecciones |
| `climate.rs` | ~120 | Ciclo día/noche |
| `persistence.rs` | ~317 | Save/Load bincode |

---

## 🔧 ARQUITECTURA

```
main.rs (Game Loop con Time Warp)
  ├── Ventana minifb (800×600, Scale X2)
  ├── Doble buffer (swap de punteros, sin memcpy)
  ├── Time Warp: 1×, 2×, 4×, 8× (teclas +/-)
  │
  ├── Fase de carga:
  │   ├── luts::init_trig_luts()
  │   ├── rng_pool::init_rng_pool(42)
  │   ├── bump_alloc::init_frame_allocator()
  │   ├── audio::AudioPlayer::init()
  │   ├── object_pool::EntityPool::new(1000)
  │   ├── ecs::create_world()
  │   ├── sim::init_simulation()
  │   └── persistence::load_game("save.dat")
  │
  ├── Cache warming
  └── Bucle principal:
      ├── bump_alloc::reset_frame()
      ├── input_state.update(&window)
      ├── Guardar/Cargar (F5/F9)
      ├── interactive::process_design_input()
      ├── ecs::process_input() (WASD)
      ├── sim::tick() × speed_multiplier
      ├── Sistemas periódicos (tax, parking, waste, politics, wear, pedestrian)
      ├── spatial_grid.rebuild()
      ├── render_cache.rebuild_from_world()
      ├── render::render_world_cached()
      ├── climate::apply_day_night_overlay()
      ├── interactive::render_design_overlay()
      ├── render::render_stats_panel()
      └── Swap buffers + window.update()
```

---

## 🏗️ COMPONENTES ECS

| Componente | Campos | Align |
|------------|--------|-------|
| `Position` | `x: f32, y: f32` | 64B |
| `Velocity` | `dx: f32, dy: f32` | 64B |
| `Renderable` | `shape_type: u8, color: u32, size: f32, layer: u8` | 64B |
| `TrafficCar` | `speed, max_speed, acceleration, lane_position, lane_id` | 64B |
| `Pedestrian` | `dest_x, dest_y, speed, stress, state` 🆕 | 64B |
| `ResourceStorage` | `money: f32, food: f32, goods: f32` | 64B |
| `ConstructionState` | `progress: f32, building_type: BuildingType` | 64B |
| `ZoneComponent` | `zone_type: ZoneType, density: u8` | 64B |
| `Lifetime` | `remaining: f32` | 64B |
| `Camera` | `x, y, zoom, speed` | 64B |

---

## ⚡ OPTIMIZACIONES (30 Técnicas Comunes + 20 Avanzadas + 10 Vanguardia)

Ver sección de técnicas en el código fuente de cada módulo (etiquetas `[TC#N]`, `[TA#N]`, `[TI#N]`).

---

## 📊 PERFILES DE COMPILACIÓN

```toml
[profile.dev]
opt-level = 1
debuginfo = 1

[profile.release]
opt-level = 3
lto = "fat"
panic = "abort"
overflow-checks = false
codegen-units = 1
```

---

## 🔬 REFERENCIAS

- **IDM Traffic Model**: Treiber, M., Hennecke, A., Helbing, D. (2000). "Congested traffic states in empirical observations and microscopic simulations". *Physical Review E*, 62(2), 1805.
- **MOBIL Lane Changes**: Kesting, A., Treiber, M., Helbing, D. (2007). "General lane-changing model MOBIL for car-following models". *Transportation Research Record*, 1999(1), 86-94.
- **Social Force Model (Pedestrians)**: Helbing, D., Molnár, P. (1995). "Social force model for pedestrian dynamics". *Physical Review E*, 51(5), 4282. 🆕
- **Land Value Tax**: George, H. (1879). *Progress and Poverty*. NY: D. Appleton & Co.
- **Gentrification Dynamics**: Lees, L., Slater, T., Wyly, E. (2008). *Gentrification*. Routledge.
- **Urban Heat Island**: Oke, T.R. (1982). "The energetic basis of the urban heat island". *Quarterly Journal of the Royal Meteorological Society*, 108(455), 1-24.
- **Flow Fields**: Dijkstra, E.W. (1959). "A note on two problems in connexion with graphs". *Numerische Mathematik*, 1, 269-271.
- **NIMBY Politics**: Dear, M. (1992). "Understanding and overcoming the NIMBY syndrome". *Journal of the American Planning Association*, 58(3), 288-300.
- **A/B Street Traffic**: https://github.com/a-b-street/abstreet — modelo de tráfico basado en carriles con IDM.
- **Citybound Original**: https://github.com/citybound/citybound — proyecto original de Anselm Eickhoff.