# RYCIMMU — CONTRATO DE LICENCIA DE USUARIO FINAL Y ECOSISTEMA (EULA)

> **Versión 1.0 — Source-Available con Copyleft Estricto**

Copyright (c) 2025 Rycimmu Development Team. Todos los derechos reservados.

---

## 1. Transparencia y Copyleft Obligatorio

### 1.1 Herencia de Licencia (Copyleft Estricto)
Cualquier obra derivada, modificación, fork, herramienta o mod basado en este juego debe compartir obligatoriamente esta misma licencia y todos sus términos, sin excepción.

### 1.2 Obligación de Entrega Completa
Estás obligado a compartir públicamente tanto el código fuente como los binarios finales compilados de tu juego o modificación.

### 1.3 Regla de la Tienda (Listo para Usar)
Cualquier publicación en tu tienda oficial debe incluir obligatoriamente los binarios finales listos para ejecutar. Está prohibido dejar únicamente el código fuente para que el usuario tenga que compilarlo por su cuenta.

### 1.4 Derecho de Integración
El equipo de Rycimmu conserva el derecho legal y unilateral de absorber, adoptar o integrar oficialmente al juego base cualquier idea, lógica o código de los mods creados por la comunidad.

### 1.5 Privacidad Absoluta (Anti-Datos)
Se prohíbe estrictamente que cualquier mod o fork recopile datos personales de los usuarios (correos, IPs, telemetría, datos de hardware).

---

## 2. Reglas de Publicación, Mantenimiento y Documentación

### 2.1 Presencia Obligatoria en la Tienda
Cualquier mod, bifurcación (fork) o modificación debe estar obligatoriamente publicado en tu tienda oficial.

### 2.2 Regla de Sincronización y Última Versión
Los desarrolladores pueden distribuir sus proyectos en otras plataformas externas, pero tu tienda oficial siempre debe tener la última versión. Está estrictamente prohibido que una plataforma externa esté más actualizada que tu web.

### 2.3 Documentación Exhaustiva Obligatoria
Para publicar y mantener un mod o fork, el autor debe incluir y mantener actualizada:
- **Documentación Interna**: Explicación técnica detallada del funcionamiento de su código.
- **Documentación Externa**: Un tutorial claro y accesible para el usuario final.

---

## 3. Arquitectura Técnica y Retrocompatibilidad

### 3.1 Uso Obligatorio del Mod Loader
Los mods solo pueden interactuar con el juego a través de la API y los puntos de entrada oficiales.

### 3.2 Cláusula Anti-Inyección
Queda estrictamente prohibido alterar los binarios del juego, inyectar código en tiempo de ejecución o manipular la memoria.

### 3.3 Forks sin Soporte de Mods
Si un fork decide no dar soporte a tu ecosistema/tienda oficial de mods, la licencia lo obliga a eliminar por completo cualquier capacidad de cargar mods en su código.

### 3.4 Retrocompatibilidad Absoluta Obligatoria
Cualquier API o componente intermedio desarrollado por un mod o fork debe mantener compatibilidad del 100% hacia atrás con todas sus versiones públicas anteriores para evitar romper las dependencias de otros creadores.

---

## 4. Protección de Red, Servidores e Ingeniería Inversa (Anti-Multinacionales)

### 4.1 Prohibición de Ingeniería Inversa
Se prohíbe estrictamente el uso de ingeniería inversa, descompilación, desensamblado o análisis dinámico del código y los binarios del juego.

### 4.2 Derivación por Protocolo de Red
Cualquier software ajeno (sea cliente o servidor) que implemente, imite o utilice el mismo protocolo de red del juego para comunicarse con el servidor oficial o actuar como tal, se considera legalmente una obra derivada. Esto obliga a que cualquier réplica escrita desde cero (por ejemplo, por una multinacional) deba adoptar obligatoriamente esta misma licencia restrictiva.

### 4.3 Obligación de Servidor Abierto (Efecto AGPL)
Si el software modificado se ejecuta en un servidor para ofrecer juego en red, el operador está obligado a publicar y poner a disposición de los usuarios el código fuente exacto y los binarios de la versión que está corriendo en ese momento.

---

## 5. Monetización Restringida

### 5.1 Solo Donaciones Voluntarias
Queda prohibido el lucro directo (vender mods, usar pasarelas de pago o muros de pago/paywalls). Los creadores solo pueden financiarse mediante aportaciones voluntarias ajenas al software (Patreon, Ko-fi o el sistema de propinas de tu tienda).

---

## 6. Moderación y Garantías

### 6.1 Derecho de Admisión y Remoción Unilateral
El equipo de Rycimmu tiene la facultad de borrar de tu tienda de forma inmediata y sin previo aviso cualquier proyecto que incluya malware, contenido ofensivo, viole derechos de autor de terceros o incumpla cualquiera de las reglas técnicas, de documentación o de entrega de binarios.

### 6.2 Exclusión de Responsabilidad (As Is)
El juego y sus herramientas se entregan "tal cual". El equipo de Rycimmu no se hace responsable si sus parches rompen la compatibilidad con los mods, ni si un mod de terceros causa daños en el equipo o servidor de un usuario.

---

## 7. Atribución e Inspiración

Este proyecto fue inspirado por Citybound de Anselm Eickhoff (https://github.com/citybound/citybound), un simulador urbano pionero. Rycimmu es una implementación completamente independiente, reescrita desde cero en Rust puro con arquitectura ECS, renderizado nativo y sistemas de simulación originales. No comparte código ni dependencias con el proyecto original.