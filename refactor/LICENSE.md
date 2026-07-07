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

### 1.6 Política de Uso Aceptable y Contenido Prohibido
Queda estrictamente prohibida la creación, publicación, distribución o alojamiento de cualquier modificación, mod, fork o herramienta que contenga, facilite o promueva elementos ilegales, maliciosos o poco éticos. Específicamente, se prohíbe:

### 1.6.1 Código Malicioso y Malware
    Cualquier software que ejecute funciones ocultas o dañinas en el equipo del usuario, incluyendo de forma enunciativa pero no limitativa: troyanos, virus, ransomware, mineros de criptomonedas no autorizados (crypto-jackers), exploit-kits o cualquier tipo de vulnerabilidad inyectada a propósito.

### 1.6.2 Contenido Ilegal y Violación de Propiedad Intelectual
    Mods o forks que utilicen activos (modelos 3D, texturas, música, código, marcas registradas) que pertenezcan a terceros sin su consentimiento explícito (por ejemplo, meter marcas reales de automóviles o personajes de otras franquicias de videojuegos sin licencia). Asi mismo, se prohíbe cualquier contenido que facilite actividades delictivas en el mundo real.

### 1.6.3 Contenido Ofensivo, Abusivo y Hostigamiento
    Contenido que promueva el discurso de odio, la discriminación, el racismo, la xenofobia, o la violencia contra personas o colectivos. Se prohíbe estrictamente el material pornográfico, gore explícito o representaciones visuales extremas no aptas para el público general en el apartado general, el material pornográfico, gore explícito o representaciones visuales extremas no aptas para el público general deben ir en la seccion de mayores de edad.

### 1.6.4 Prácticas No Éticas y Trampas (Cheats)
    Mods destinados a sabotear la experiencia de juego de otros usuarios, alterar de forma maliciosa las clasificaciones oficiales, o atacar la infraestructura de los servidores de Rycimmu mediante denegación de servicio (DoS/DDoS) o inyección de exploits de red.

### 1.6.5 Publicidad Engañosa y Spam:
    Mods que funcionen como vehículos para inyectar publicidad no deseada, adware, enlaces de afiliados engañosos, o que alteren el juego base únicamente para redirigir al usuario a sitios web externos con fines de lucro encubierto.

La violación de cualquiera de estos subpuntos resultará en la eliminación inmediata del proyecto de la tienda oficial, el banneo permanente del desarrollador en el ecosistema de Rycimmu y, si la gravedad del caso lo amerita, la notificación a las autoridades legales competentes.

---

## 2. Reglas de Publicación, Mantenimiento y Documentación

### 2.1 Presencia Obligatoria en la Tienda
Cualquier mod, bifurcación (fork) o modificación debe estar obligatoriamente publicado en la tienda oficial.

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
Queda estrictamente prohibido alterar los binarios del juego, inyectar código en tiempo de ejecución o manipular la memoria, cualquier mod debe ser a traves de un modloader.

### 3.3 Forks sin Soporte de Mods
Si un fork decide no dar soporte a la tienda oficial de mods, la licencia lo obliga a eliminar por completo cualquier capacidad de cargar mods en su código.

### 3.4 Retrocompatibilidad Absoluta Obligatoria
Cualquier API o componente intermedio desarrollado por un mod o fork debe mantener compatibilidad del 100% hacia atrás con todas sus versiones públicas anteriores para evitar romper las dependencias de otros creadores.


### 3.5 Mods y forks: redistribucion
Los mods y forks deben incluir todos los archivos binarios correspondientes para la ejecución, tanto necesarios como opcionales. No se puede simplemente dejar solo el código fuente y esperar a que el usuario lo compile. Además de que la descarga debe ser directa en la misma plataforma, no puede haber redirección a plataformas externas.

---

## 4. Protección de Red, Servidores

### 4.2 Derivación por Protocolo de Red
Cualquier software ajeno (sea cliente o servidor) que implemente, imite o utilice el mismo protocolo de red del juego para comunicarse con el servidor oficial o actuar como tal, se considera legalmente una obra derivada. Esto obliga a que cualquier réplica escrita desde cero (por ejemplo, por una multinacional) deba adoptar obligatoriamente esta misma licencia restrictiva.

### 4.3 Obligación de Servidor Abierto (Efecto AGPL)
Si el software modificado se ejecuta en un servidor para ofrecer juego en red, el operador está obligado a publicar y poner a disposición de los usuarios el código fuente exacto y los binarios de la versión que está corriendo en ese momento.

---

## 5. Monetización Restringida

### 5.1 Solo Donaciones Voluntarias
Queda prohibido el lucro directo (vender mods, usar pasarelas de pago o muros de pago/paywalls). Los creadores solo pueden financiarse mediante aportaciones voluntarias ajenas al software las cuales no pueden desbloquear contenido.

---

## 6. Moderación y Garantías

### 6.1 Derecho de Admisión y Remoción Unilateral
El equipo de Rycimmu tiene la facultad de borrar de su tienda de forma inmediata y sin previo aviso cualquier proyecto.

### 6.2 Exclusión de Responsabilidad (As Is)
El juego y sus herramientas se entregan "tal cual". El equipo de Rycimmu no se hace responsable si sus parches rompen la compatibilidad con los mods, ni si un mod de terceros causa daños en el equipo o servidor de un usuario.

---

## 7. Acuerdo de Cesión y Transferencia de Derechos de Autor (CLA)

Al enviar cualquier contribución (incluyendo, pero no limitado a, código fuente, binarios, documentación, correcciones o sugerencias) a este repositorio a través de Pull Requests, commits o cualquier otro medio, usted (el "Colaborador") acepta transferir de forma total, exclusiva, irrevocable, mundial y perpetua todos los derechos de autor, propiedad intelectual y derechos conexos de dicha contribución al Autor Original del proyecto (Rycimmu Development Team).

El Autor Original se reserva el derecho absoluto de sublicenciar, modificar, comercializar, cerrar o cambiar los términos de la licencia del software base en el futuro sin necesidad de consentimiento adicional ni compensación económica hacia el Colaborador.

El Colaborador garantiza que es el autor legítimo del código aportado y que este no infringe derechos de terceros. El Colaborador acepta que su contribución sea irrevocable una vez fusionada en la rama principal del proyecto, y que no podrá retirar dicha contribución ni exigir su eliminación del código base en el futuro.

### 7.1 Proceso de Firma del CLA

Para que cualquier Pull Request sea fusionado, el Colaborador DEBERÁ firmar explícitamente este CLA mediante el bot designado (CLA Assistant). El bot verificará automáticamente la firma y bloqueará cualquier fusión sin el CLA firmado. No se aceptarán contribuciones sin la firma digital explícita del CLA; el silencio o la inacción no constituyen aceptación.

---

## 8. Atribución e Inspiración

Este proyecto fue inspirado por Citybound de Anselm Eickhoff (https://github.com/citybound/citybound), un simulador urbano pionero. Rycimmu es una implementación completamente independiente, reescrita desde cero en Rust puro con arquitectura ECS, renderizado nativo y sistemas de simulación originales. No comparte código ni dependencias con el proyecto original, por lo tanto no es obra derivada ni debe heredar su licencia.