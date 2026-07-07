# Rycimmu v0.18.0 — Simulador Urbano Realista

Simulador de ciudades nativo en Rust puro con renderizado por software, aceleración GPU adaptativa, sprites PNG, arquitectura ECS, y simulación económica avanzada. Optimizado para funcionar en hardware legacy (Pentium 4 GB RAM, 2 núcleos) y escalar dinámicamente en hardware moderno.

> **Inspirado por** Citybound de Anselm Eickhoff — una reimplementación independiente desde cero.

## 🎯 Visión

Rycimmu es un simulador de construcción de ciudades donde cada detalle importa: desde el tráfico microscópico por carriles hasta cadenas de suministro físicas, mercados laborales, sistemas políticos y valor del suelo. Todo renderizado de forma nativa sin dependencia de navegador.

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

## 🚀 Ejecución

```bash
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

## 📜 Licencia

**RYCIMMU EULA v1.0** — Source-Available con Copyleft Estricto.

Ver [LICENSE.md](LICENSE.md) para el texto completo.

---

*Inspirado por Citybound de Anselm Eickhoff. Rycimmu es una implementación 100% independiente escrita desde cero.*