// Herramienta de Diseño Urbano Interactivo
//
// ISSUE #392: Looking for collaborators for a version of citybound to
// provide citizen urban design tool
//
// Implementa un modo interactivo donde el usuario puede:
// - Pintar zonas (residencial, comercial, industrial, agrícola)
// - Colocar edificios con un clic
// - Ver preview en tiempo real antes de confirmar
// - Deshacer/rehacer acciones
// - Ajustar tamaño de pincel
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad
// [TC#10] Transformación de cámara inversa para screen→world
// [TC#26] Inlining agresivo en hot paths
// [TA#17] Acceso unchecked en bucles validados

use crate::ecs::{
    GameWorld, Position, Renderable, ZoneComponent, ZoneType,
    ConstructionState, BuildingType, ResourceStorage, Camera,
};
use crate::input::{InputState, GameKey, MouseButton};
use std::collections::VecDeque;

// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Tamaño máximo del historial de undo
pub const MAX_UNDO_HISTORY: usize = 64;
/// Tamaño de pincel por defecto (en celdas)
pub const DEFAULT_BRUSH_SIZE: i32 = 3;
/// Tamaño máximo de pincel
pub const MAX_BRUSH_SIZE: i32 = 20;
/// Tamaño mínimo de pincel
pub const MIN_BRUSH_SIZE: i32 = 1;
/// Colores ARGB para preview
pub const PREVIEW_COLOR_ZONE: u32 = 0x66_FF_FF_00;
pub const PREVIEW_COLOR_BUILDING: u32 = 0x88_00_FF_00;
pub const PREVIEW_COLOR_GHOST: u32 = 0x44_FF_FF_FF;
pub const HIGHLIGHT_COLOR: u32 = 0x88_FF_FF_00;

// ---------------------------------------------------------------------------
// TIPOS DE ACCIÓN (para undo/redo)
// ---------------------------------------------------------------------------

/// Una acción de diseño que se puede deshacer
#[derive(Clone, Debug)]
pub enum DesignAction {
    /// Colocar un edificio en (x, y) de tipo BuildingType
    PlaceBuilding {
        x: f32,
        y: f32,
        building_type: BuildingType,
        entity_id: Option<u64>,
    },
    /// Pintar zona en un rectángulo
    PaintZone {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        zone_type: ZoneType,
        density: u8,
        /// Entidades creadas (IDs para poder eliminarlas en undo)
        entity_ids: Vec<u64>,
    },
    /// Eliminar edificio(s) en una posición
    RemoveBuilding {
        x: f32,
        y: f32,
        /// Datos del edificio eliminado para restaurar
        building_type: BuildingType,
        money: f32,
        food: f32,
        goods: f32,
    },
    /// Limpiar zona (eliminar zona)
    ClearZone {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        previous_zones: Vec<(f32, f32, ZoneType, u8)>,
    },
}

// ---------------------------------------------------------------------------
// ESTADO DE LA HERRAMIENTA DE DISEÑO
// ---------------------------------------------------------------------------

/// Modo actual de la herramienta
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DesignMode {
    /// Modo normal (sin herramienta activa)
    None,
    /// Pintando zonas
    PaintZone,
    /// Colocando edificios
    PlaceBuilding,
    /// Inspeccionando (click para ver info)
    Inspect,
}

/// Estado completo de la herramienta de diseño
pub struct DesignTool {
    /// Modo actual
    pub mode: DesignMode,
    /// ¿Está activa la herramienta?
    pub active: bool,
    /// Tipo de zona actual para pintar
    pub current_zone: ZoneType,
    /// Tipo de edificio actual para colocar
    pub current_building: BuildingType,
    /// Densidad de zona a pintar
    pub brush_density: u8,
    /// Tamaño del pincel (radio en celdas)
    pub brush_size: i32,
    /// Posición del mouse en coordenadas de mundo
    pub mouse_world_x: f32,
    pub mouse_world_y: f32,
    /// ¿Está el mouse sobre la ventana?
    pub mouse_in_window: bool,
    /// Posición de inicio del drag (para zonas)
    pub drag_start_x: Option<f32>,
    pub drag_start_y: Option<f32>,
    /// ¿Está arrastrando actualmente?
    pub is_dragging: bool,
    /// ¿Botón de mouse presionado?
    pub mouse_down: bool,
    /// Preview fantasma activo
    pub show_ghost: bool,
    /// Historial de acciones para undo
    pub undo_stack: VecDeque<DesignAction>,
    /// Historial para redo
    pub redo_stack: VecDeque<DesignAction>,
    /// Contador de IDs para entidades creadas
    pub entity_id_counter: u64,
}

impl DesignTool {
    /// Crea una nueva herramienta de diseño
    pub fn new() -> Self {
        DesignTool {
            mode: DesignMode::None,
            active: false,
            current_zone: ZoneType::Residential,
            current_building: BuildingType::House,
            brush_density: 2,
            brush_size: DEFAULT_BRUSH_SIZE,
            mouse_world_x: 0.0,
            mouse_world_y: 0.0,
            mouse_in_window: false,
            drag_start_x: None,
            drag_start_y: None,
            is_dragging: false,
            mouse_down: false,
            show_ghost: true,
            undo_stack: VecDeque::with_capacity(MAX_UNDO_HISTORY),
            redo_stack: VecDeque::with_capacity(MAX_UNDO_HISTORY),
            entity_id_counter: 0,
        }
    }

    /// Activa/desactiva la herramienta
    pub fn toggle(&mut self) {
        self.active = !self.active;
        if self.active && self.mode == DesignMode::None {
            self.mode = DesignMode::PaintZone;
        }
        if !self.active {
            self.mode = DesignMode::None;
            self.drag_start_x = None;
            self.drag_start_y = None;
            self.is_dragging = false;
        }
    }

    /// Cambia al modo de pintar zonas
    pub fn set_paint_mode(&mut self) {
        self.mode = DesignMode::PaintZone;
        self.active = true;
    }

    /// Cambia al modo de colocar edificios
    pub fn set_building_mode(&mut self) {
        self.mode = DesignMode::PlaceBuilding;
        self.active = true;
    }

    /// Cambia al modo de inspección
    pub fn set_inspect_mode(&mut self) {
        self.mode = DesignMode::Inspect;
        self.active = true;
    }

    /// Cicla el tipo de zona (1-6)
    pub fn cycle_zone(&mut self) {
        self.current_zone = match self.current_zone {
            ZoneType::Residential => ZoneType::Commercial,
            ZoneType::Commercial => ZoneType::Industrial,
            ZoneType::Industrial => ZoneType::Agricultural,
            ZoneType::Agricultural => ZoneType::Road,
            ZoneType::Road => ZoneType::Park,
            ZoneType::Park => ZoneType::Residential,
        };
    }

    /// Cicla el tipo de edificio
    pub fn cycle_building(&mut self) {
        self.current_building = match self.current_building {
            BuildingType::House => BuildingType::Apartment,
            BuildingType::Apartment => BuildingType::Shop,
            BuildingType::Shop => BuildingType::Office,
            BuildingType::Office => BuildingType::Factory,
            BuildingType::Factory => BuildingType::Farm,
            BuildingType::Farm => BuildingType::Hospital,
            BuildingType::Hospital => BuildingType::School,
            BuildingType::School => BuildingType::Police,
            BuildingType::Police => BuildingType::House,
        };
    }

    /// Aumenta el tamaño del pincel
    pub fn increase_brush(&mut self) {
        self.brush_size = (self.brush_size + 1).min(MAX_BRUSH_SIZE);
    }

    /// Reduce el tamaño del pincel
    pub fn decrease_brush(&mut self) {
        self.brush_size = (self.brush_size - 1).max(MIN_BRUSH_SIZE);
    }

    /// Deshacer última acción
    /// Deshacer última acción
    pub fn undo(&mut self, game_world: &mut GameWorld) {
        if let Some(action) = self.undo_stack.pop_back() {
            match &action {
                DesignAction::PlaceBuilding { x, y, entity_id, .. } => {
                    // Eliminar la entidad del edificio
                    if let Some(_id) = entity_id {
                        // En hecs no podemos buscar por un u64 arbitrario,
                        // así que eliminamos por posición
                        let mut to_remove = Vec::new();
                        for (entity, (pos, _construction)) in game_world.world
                            .query::<(&Position, &ConstructionState)>()
                            .iter()
                        {
                            if (pos.x - *x).abs() < 0.5 && (pos.y - *y).abs() < 0.5 {
                                to_remove.push(entity);
                            }
                        }
                        for entity in to_remove {
                            // Limpiar bitboard
                            game_world.bitgrid.clear(0, *x, *y);
                            let _ = game_world.world.despawn(entity);
                        }
                    }
                }
                DesignAction::PaintZone { x1, y1, x2, y2, entity_ids, .. } => {
                    // Eliminar entidades de zona creadas
                    for _eid in entity_ids {
                        // Similar: eliminar por posición en el rango
                    }
                    let mut to_remove = Vec::new();
                    for (entity, (pos, _zone)) in game_world.world
                        .query::<(&Position, &ZoneComponent)>()
                        .iter()
                    {
                        if pos.x >= *x1 && pos.x <= *x2 && pos.y >= *y1 && pos.y <= *y2 {
                            to_remove.push(entity);
                        }
                    }
                    for entity in to_remove {
                        let _ = game_world.world.despawn(entity);
                    }
                }
                DesignAction::RemoveBuilding { x, y, building_type, money, food, goods } => {
                    // Restaurar edificio eliminado
                    game_world.world.spawn((
                        Position::new(*x, *y),
                        Renderable::rect(building_color(*building_type), 3.0, 3),
                        ConstructionState { progress: 1.0, building_type: *building_type },
                        ResourceStorage { money: *money, food: *food, goods: *goods },
                    ));
                    game_world.bitgrid.set(0, *x, *y);
                }
                DesignAction::ClearZone { x1: _, y1: _, x2: _, y2: _, previous_zones } => {

                    // Restaurar zonas anteriores
                    for (zx, zy, ztype, density) in previous_zones {
                        game_world.world.spawn((
                            Position::new(*zx, *zy),
                            Renderable::rect(zone_color(*ztype), 1.0, 1),
                            ZoneComponent { zone_type: *ztype, density: *density },
                        ));
                    }
                }
            }
            self.redo_stack.push_back(action);
        }
    }

    /// Rehacer última acción deshecha
    pub fn redo(&mut self, game_world: &mut GameWorld) {
        if let Some(action) = self.redo_stack.pop_back() {
            match &action {
                DesignAction::PlaceBuilding { x, y, building_type, .. } => {
                    game_world.world.spawn((
                        Position::new(*x, *y),
                        Renderable::rect(building_color(*building_type), 3.0, 3),
                        ConstructionState { progress: 1.0, building_type: *building_type },
                        ResourceStorage { money: 1000.0, food: 100.0, goods: 50.0 },
                    ));
                    game_world.bitgrid.set(0, *x, *y);
                }
                DesignAction::PaintZone { x1, y1, x2, y2, zone_type, density, .. } => {
                    for dx in 0..=(*x2 - *x1) as i32 {
                        for dy in 0..=(*y2 - *y1) as i32 {
                            game_world.world.spawn((
                                Position::new(x1 + dx as f32, y1 + dy as f32),
                                Renderable::rect(zone_color(*zone_type), 1.0, 1),
                                ZoneComponent { zone_type: *zone_type, density: *density },
                            ));
                        }
                    }
                }
                DesignAction::RemoveBuilding { .. } => {
                    // Ya fue eliminado en undo, no hacemos nada adicional
                }
                DesignAction::ClearZone { .. } => {
                    // Ya fue limpiado en undo, no hacemos nada adicional
                }
            }
            self.undo_stack.push_back(action);
        }
    }

    /// Agrega una acción al historial de undo
    /// Agrega una acción al historial de undo
    fn push_action(&mut self, action: DesignAction) {
        if self.undo_stack.len() >= MAX_UNDO_HISTORY {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(action);
        self.redo_stack.clear();
        self.entity_id_counter += 1;
    }

    /// Ejecuta una acción de colocación de edificio
    pub fn execute_place_building(&mut self, gw: &mut GameWorld, x: f32, y: f32) {
        let btype = self.current_building;

        // Verificar que no hay obstáculo en la posición
        if gw.bitgrid.is_obstacle(x, y) {
            return; // Ya hay algo allí
        }

        gw.world.spawn((
            Position::new(x, y),
            Renderable::rect(building_color(btype), 3.0, 3),
            ConstructionState { progress: 1.0, building_type: btype },
            ResourceStorage { money: 1000.0, food: 100.0, goods: 50.0 },
        ));

        gw.bitgrid.set(0, x, y);

        self.push_action(DesignAction::PlaceBuilding {
            x,
            y,
            building_type: btype,
            entity_id: Some(self.entity_id_counter),
        });
    }

    /// Ejecuta una acción de pintar zona
    pub fn execute_paint_zone(&mut self, gw: &mut GameWorld,
                               x1: f32, y1: f32, x2: f32, y2: f32) {
        let ztype = self.current_zone;
        let density = self.brush_density;

        let min_x = x1.min(x2);
        let max_x = x1.max(x2);
        let min_y = y1.min(y2);
        let max_y = y1.max(y2);

        let mut entity_ids = Vec::with_capacity(
            ((max_x - min_x + 1.0) * (max_y - min_y + 1.0)) as usize
        );

        for dx in 0..=(max_x - min_x) as i32 {
            for dy in 0..=(max_y - min_y) as i32 {
                let wx = min_x + dx as f32;
                let wy = min_y + dy as f32;

                if wx >= 0.0 && wx < gw.grid_size as f32
                    && wy >= 0.0 && wy < gw.grid_size as f32
                {
                    gw.world.spawn((
                        Position::new(wx, wy),
                        Renderable::rect(zone_color(ztype), 1.0, 1),
                        ZoneComponent { zone_type: ztype, density },
                    ));
                    entity_ids.push(self.entity_id_counter);
                    self.entity_id_counter += 1;
                }
            }
        }

        self.push_action(DesignAction::PaintZone {
            x1: min_x,
            y1: min_y,
            x2: max_x,
            y2: max_y,
            zone_type: ztype,
            density,
            entity_ids,
        });
    }
    /// Ejecuta una acción de eliminar edificio
    pub fn execute_remove_building(&mut self, gw: &mut GameWorld, x: f32, y: f32) {
        let mut removed = None;

        for (entity, (pos, construction, resources)) in gw.world
            .query::<(&Position, &ConstructionState, &ResourceStorage)>()
            .iter()
        {
            if (pos.x - x).abs() < 0.5 && (pos.y - y).abs() < 0.5 {
                removed = Some((
                    entity,
                    construction.building_type,
                    resources.money,
                    resources.food,
                    resources.goods,
                ));
                break;
            }
        }
        if let Some((entity, btype, money, food, goods)) = removed {
            gw.bitgrid.clear(0, x, y);
            let _ = gw.world.despawn(entity);

            self.push_action(DesignAction::RemoveBuilding {
                x,
                y,
                building_type: btype,
                money,
                food,
                goods,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// PROCESAMIENTO DE INPUT
// ---------------------------------------------------------------------------

/// Procesa input para la herramienta de diseño
pub fn process_design_input(
    tool: &mut DesignTool,
    gw: &mut GameWorld,
    input: &InputState,
    window_width: u32,
    window_height: u32,
) {
    if input.is_key_pressed(GameKey::Tab) {
        tool.toggle();
    }

    if !tool.active {
        return;
    }

    // Cambiar modo con teclas
    if input.is_key_pressed(GameKey::Key1) {
        if input.is_key_down(GameKey::Shift) {
            tool.cycle_building();
        } else {
            tool.set_paint_mode();
            tool.cycle_zone();
        }
    }
    if input.is_key_pressed(GameKey::Key2) {
        tool.set_building_mode();
    } else {
        tool.set_paint_mode();
        tool.current_zone = ZoneType::Commercial;
    }
    if input.is_key_pressed(GameKey::Key3) {
        tool.set_paint_mode();
        tool.current_zone = ZoneType::Industrial;
    }
    if input.is_key_pressed(GameKey::Key4) {
        tool.set_paint_mode();
        tool.current_zone = ZoneType::Agricultural;
    }
    if input.is_key_pressed(GameKey::Key5) {
        tool.set_paint_mode();
        tool.current_zone = ZoneType::Road;
    }
    if input.is_key_pressed(GameKey::Key6) {
        tool.set_paint_mode();
        tool.current_zone = ZoneType::Park;
    }

    // Modo building con B
    if input.is_key_pressed(GameKey::B) {
        tool.set_building_mode();
    }
    // Modo inspección con I
    if input.is_key_pressed(GameKey::I) {
        tool.set_inspect_mode();
    }

    // Tamaño de pincel
    if input.is_key_pressed(GameKey::BracketLeft) {
        tool.decrease_brush();
    }
    if input.is_key_pressed(GameKey::BracketRight) {
        tool.increase_brush();
    }

    // Undo/Redo
    if input.is_key_pressed(GameKey::Z) && input.is_key_down(GameKey::Control) {
        if input.is_key_down(GameKey::Shift) {
            tool.redo(gw);
        } else {
            tool.undo(gw);
        }
    }

    // Obtener posición del mouse en coordenadas de mundo
    let mouse_screen_x = input.mouse_x;
    let mouse_screen_y = input.mouse_y;

    tool.mouse_in_window = mouse_screen_x >= 0.0
        && mouse_screen_x < window_width as f32
        && mouse_screen_y >= 0.0
        && mouse_screen_y < window_height as f32;

    // Convertir screen→world usando cámara
    let (cam_ox, cam_oy, cam_zoom) = get_camera_params(gw);
    let scale = 4.0 * cam_zoom; // CELL_SIZE * zoom
    let offset_x = (window_width as f32 / 2.0) - cam_ox * scale;
    let offset_y = (window_height as f32 / 2.0) - cam_oy * scale;

    tool.mouse_world_x = (mouse_screen_x - offset_x) / scale;
    tool.mouse_world_y = (mouse_screen_y - offset_y) / scale;

    // Snapping a grid para construcción
    let snapped_x = tool.mouse_world_x.floor() + 0.5;
    let snapped_y = tool.mouse_world_y.floor() + 0.5;

    // Procesar clicks según modo
    let mouse_left = input.is_mouse_pressed(MouseButton::Left);
    let mouse_right = input.is_mouse_pressed(MouseButton::Right);
    let mouse_left_down = input.is_mouse_down(MouseButton::Left);

    match tool.mode {
        DesignMode::PaintZone => {
            if mouse_left && tool.mouse_in_window {
                // Iniciar drag de zona
                tool.drag_start_x = Some(snapped_x);
                tool.drag_start_y = Some(snapped_y);
                tool.is_dragging = true;
                tool.mouse_down = true;
            }

            if mouse_left_down && tool.is_dragging {
                // Continuar drag (se aplica al soltar)
            }

            if !mouse_left_down && tool.is_dragging {
                // Finalizar drag: pintar zona
                if let (Some(sx), Some(sy)) = (tool.drag_start_x, tool.drag_start_y) {
                    tool.execute_paint_zone(gw, sx, sy, snapped_x, snapped_y);
                }
                tool.is_dragging = false;
                tool.mouse_down = false;
                tool.drag_start_x = None;
                tool.drag_start_y = None;
            }

            // Click derecho: pintar con pincel
            if mouse_right && tool.mouse_in_window {
                let brush = tool.brush_size;
                let half = brush / 2;
                tool.execute_paint_zone(
                    gw,
                    snapped_x - half as f32,
                    snapped_y - half as f32,
                    snapped_x + half as f32,
                    snapped_y + half as f32,
                );
            }

            // Eliminar zona con Shift+Click
            if mouse_right && input.is_key_down(GameKey::Shift) && tool.mouse_in_window {
                // Limpiar zona en el área del pincel
                let brush = tool.brush_size;
                let half = brush / 2;
                let mut previous = Vec::new();

                for (_entity, (pos, zone)) in gw.world

                for (entity, (pos, zone)) in gw.world
                    .query::<(&Position, &ZoneComponent)>()
                    .iter()
                {
                    if pos.x >= snapped_x - half as f32
                        && pos.x <= snapped_x + half as f32
                        && pos.y >= snapped_y - half as f32
                        && pos.y <= snapped_y + half as f32
                    {
                        previous.push((pos.x, pos.y, zone.zone_type, zone.density));
                    }
                }

                // Eliminar entidades en el rango
                let mut to_remove = Vec::new();
                for (entity, (pos, _zone)) in gw.world
                    .query::<(&Position, &ZoneComponent)>()
                    .iter()
                {
                    if pos.x >= snapped_x - half as f32
                        && pos.x <= snapped_x + half as f32
                        && pos.y >= snapped_y - half as f32
                        && pos.y <= snapped_y + half as f32
                    {
                        to_remove.push(entity);
                    }
                }
                for entity in to_remove {
                    let _ = gw.world.despawn(entity);
                }

                tool.push_action(DesignAction::ClearZone {
                    x1: snapped_x - half as f32,
                    y1: snapped_y - half as f32,
                    x2: snapped_x + half as f32,
                    y2: snapped_y + half as f32,
                    previous_zones: previous,
                });
            }
        }

        DesignMode::PlaceBuilding => {
            if mouse_left && tool.mouse_in_window {
                tool.execute_place_building(gw, snapped_x, snapped_y);
            }
            if mouse_right && tool.mouse_in_window {
                tool.execute_remove_building(gw, snapped_x, snapped_y);
            }
        }

        DesignMode::Inspect => {
            if mouse_left && tool.mouse_in_window {
                // Mostrar información de la celda
                let _info = inspect_cell(gw, snapped_x, snapped_y);
            }
        }

        DesignMode::None => {}
    }
}

// ---------------------------------------------------------------------------
// RENDERIZADO DE PREVIEW
// ---------------------------------------------------------------------------

/// Renderiza el preview fantasma y la UI de la herramienta de diseño
pub fn render_design_overlay(
    tool: &DesignTool,
    framebuffer: &mut [u32],
    width: usize,
    height: usize,
    gw: &GameWorld,
) {
    if !tool.active || !tool.mouse_in_window {
        return;
    }

    let (cam_ox, cam_oy, cam_zoom) = get_camera_params(gw);
    let scale = 4.0 * cam_zoom;
    let offset_x = (width as f32 / 2.0) - cam_ox * scale;
    let offset_y = (height as f32 / 2.0) - cam_oy * scale;

    let snapped_x = tool.mouse_world_x.floor() + 0.5;
    let snapped_y = tool.mouse_world_y.floor() + 0.5;

    let sx = (snapped_x * scale + offset_x) as i32;
    let sy = (snapped_y * scale + offset_y) as i32;

    match tool.mode {
        DesignMode::PaintZone => {
            // Preview del área de zona
            let half = tool.brush_size / 2;
            let px1 = ((snapped_x - half as f32) * scale + offset_x) as i32;
            let py1 = ((snapped_y - half as f32) * scale + offset_y) as i32;
            let psize = (tool.brush_size as f32 * scale) as i32;

            // Rectángulo de preview
            draw_rect_dashed(framebuffer, width, height, px1, py1, psize, psize, PREVIEW_COLOR_ZONE);

            // Si está arrastrando, dibujar el área completa
            if tool.is_dragging {
                if let (Some(dx), Some(dy)) = (tool.drag_start_x, tool.drag_start_y) {
                    let dsx = (dx * scale + offset_x) as i32;
                    let dsy = (dy * scale + offset_y) as i32;
                    let dw = (sx - dsx).abs();
                    let dh = (sy - dsy).abs();
                    draw_rect_dashed(
                        framebuffer, width, height,
                        dsx.min(sx), dsy.min(sy), dw, dh,
                        PREVIEW_COLOR_ZONE,
                    );
                }
            }
        }

        DesignMode::PlaceBuilding => {
            // Preview del edificio fantasma
            let bsize = (3.0 * scale) as i32;
            let bcolor = building_color(tool.current_building);
            unsafe {
                crate::simd_render::fill_rect_alpha_simd(
                    framebuffer, width, height,
                    sx - bsize / 2, sy - bsize / 2, bsize, bsize,
                    (bcolor & 0x00_FF_FF_FF) | 0x88_00_00_00, // Semitransparente
                );
            }
        }

        DesignMode::Inspect => {
            // Cursor de inspección (cruz)
            let cross_size = 5;
            unsafe {
                crate::simd_render::fill_rect_simd(
                    framebuffer, width, height,
                    sx - cross_size, sy, cross_size * 2, 1,
                    HIGHLIGHT_COLOR,
                );
                crate::simd_render::fill_rect_simd(
                    framebuffer, width, height,
                    sx, sy - cross_size, 1, cross_size * 2,
                    HIGHLIGHT_COLOR,
                );
            }
        }

        DesignMode::None => {}
    }

    // UI: barra de estado inferior
    let ui_text = format!(
        "Modo: {} | Zona: {:?} | Edificio: {:?} | Pincel: {} | [Tab] salir | [Z] undo",
        match tool.mode {
            DesignMode::None => "Off",
            DesignMode::PaintZone => "Pintar",
            DesignMode::PlaceBuilding => "Construir",
            DesignMode::Inspect => "Inspeccionar",
        },
        tool.current_zone,
        tool.current_building,
        tool.brush_size,
    );

    let ui_y = height as i32 - 20;
    unsafe {
        crate::simd_render::fill_rect_alpha_simd(
            framebuffer, width, height,
            0, ui_y, width as i32, 20,
            0xCC_00_00_00,
        );
    }
    draw_text_simple(framebuffer, width, 8, ui_y + 3, &ui_text, 0xFF_FF_FF_FF);
}

// ---------------------------------------------------------------------------
// UTILIDADES
// ---------------------------------------------------------------------------

#[inline(always)]
fn get_camera_params(gw: &GameWorld) -> (f32, f32, f32) {
    for (_entity, (camera,)) in gw.world.query::<(&Camera,)>().iter() {
        return (camera.offset_x, camera.offset_y, camera.zoom);
    }
    (64.0, 64.0, 1.0)
}

#[inline(always)]
fn building_color(btype: BuildingType) -> u32 {
    match btype {
        BuildingType::House => 0xFF_C4_7B_4A,
        BuildingType::Apartment => 0xFF_B0_BEC5,
        BuildingType::Shop => 0xFF_26_C6_DA,
        BuildingType::Office => 0xFF_78_90_9C,
        BuildingType::Factory => 0xFF_8D_6E_63,
        BuildingType::Farm => 0xFF_8B_C3_4A,
        BuildingType::Hospital => 0xFF_F4_43_36,  // Rojo médico
        BuildingType::School => 0xFF_FF_C1_07,    // Amarillo institucional
        BuildingType::Police => 0xFF_21_21_21,     // Azul policial oscuro
    }
}

#[inline(always)]
fn zone_color(ztype: ZoneType) -> u32 {
    match ztype {
        ZoneType::Residential => 0x44_66_BB_6A,
        ZoneType::Commercial => 0x44_42_A5_F5,
        ZoneType::Industrial => 0x44_EF_5350,
        ZoneType::Agricultural => 0x44_9C_CC_65,
        ZoneType::Road => 0x44_55_55_55,
        ZoneType::Park => 0x44_4C_AF_50,
    }
}

/// Inspecciona una celda y retorna información
fn inspect_cell(gw: &GameWorld, x: f32, y: f32) -> String {
    let mut info = format!("Celda ({:.0}, {:.0}): ", x.floor(), y.floor());

    // Buscar zona
    for (_entity, (pos, zone)) in gw.world.query::<(&Position, &ZoneComponent)>().iter() {
        if (pos.x - x).abs() < 0.5 && (pos.y - y).abs() < 0.5 && zone.density > 0 {
            info.push_str(&format!("Zona {:?} (densidad {})", zone.zone_type, zone.density));
        }
    }

    // Buscar edificio
    for (_entity, (pos, construction)) in gw.world
        .query::<(&Position, &ConstructionState)>()
        .iter()
    {
        if (pos.x - x).abs() < 1.5 && (pos.y - y).abs() < 1.5 {
            info.push_str(&format!(" | Edificio {:?} ({:.0}%)",
                construction.building_type, construction.progress * 100.0));
        }
    }

    info
}

/// Dibuja un rectángulo con línea discontinua
fn draw_rect_dashed(fb: &mut [u32], w: usize, h: usize,
                    x: i32, y: i32, rw: i32, rh: i32, color: u32) {
    let dash_len = 4;
    let gap_len = 4;

    // Borde superior
    let mut px = x;
    let mut drawing = true;
    while px < x + rw {
        let seg_end = if drawing { (px + dash_len).min(x + rw) } else { (px + gap_len).min(x + rw) };
        if drawing {
            unsafe {
                crate::simd_render::fill_rect_simd(fb, w, h, px, y, seg_end - px, 1, color);
                crate::simd_render::fill_rect_simd(fb, w, h, px, y + rh - 1, seg_end - px, 1, color);
            }
        }
        px = seg_end;
        drawing = !drawing;
    }

    // Bordes laterales
    let mut py = y;
    drawing = true;
    while py < y + rh {
        let seg_end = if drawing { (py + dash_len).min(y + rh) } else { (py + gap_len).min(y + rh) };
        if drawing {
            unsafe {
                crate::simd_render::fill_rect_simd(fb, w, h, x, py, 1, seg_end - py, color);
                crate::simd_render::fill_rect_simd(fb, w, h, x + rw - 1, py, 1, seg_end - py, color);
            }
        }
        py = seg_end;
        drawing = !drawing;
    }
}

/// Dibuja texto simple (caracteres ASCII básicos)
fn draw_text_simple(fb: &mut [u32], fb_w: usize, x: i32, y: i32, text: &str, color: u32) {
    let mut cx = x;
    for ch in text.chars() {
        if ch.is_ascii() {
            // Dibujar carácter simple como punto (placeholder)
            if ch != ' ' {
                unsafe {
                    if cx >= 0 && cx < fb_w as i32 && y >= 0 && y < 600 {
                        crate::simd_render::fill_rect_simd(fb, fb_w, 600, cx, y, 5, 7, color);
                    }
                }
            }
            cx += 6;
        }
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ecs;
    use crate::object_pool::EntityPool;

    fn setup_tool_and_world() -> (DesignTool, GameWorld) {
        let tool = DesignTool::new();
        let mut pool = EntityPool::new(1000);
        let gw = ecs::create_world(&mut pool);
        (tool, gw)
    }

    #[test]
    fn test_design_tool_creation() {
        let tool = DesignTool::new();
        assert!(!tool.active);
        assert_eq!(tool.mode, DesignMode::None);
        assert_eq!(tool.brush_size, DEFAULT_BRUSH_SIZE);
        assert_eq!(tool.current_zone, ZoneType::Residential);
        assert_eq!(tool.current_building, BuildingType::House);
        assert!(tool.undo_stack.is_empty());
        assert!(tool.redo_stack.is_empty());
    }

    #[test]
    fn test_toggle_tool() {
        let mut tool = DesignTool::new();
        tool.toggle();
        assert!(tool.active);
        assert_eq!(tool.mode, DesignMode::PaintZone);

        tool.toggle();
        assert!(!tool.active);
        assert_eq!(tool.mode, DesignMode::None);
    }

    #[test]
    fn test_cycle_zone() {
        let mut tool = DesignTool::new();
        assert_eq!(tool.current_zone, ZoneType::Residential);
        tool.cycle_zone();
        assert_eq!(tool.current_zone, ZoneType::Commercial);
        tool.cycle_zone();
        assert_eq!(tool.current_zone, ZoneType::Industrial);
        tool.cycle_zone();
        assert_eq!(tool.current_zone, ZoneType::Agricultural);
        tool.cycle_zone();
        assert_eq!(tool.current_zone, ZoneType::Road);
        tool.cycle_zone();
        assert_eq!(tool.current_zone, ZoneType::Park);
        tool.cycle_zone();
        assert_eq!(tool.current_zone, ZoneType::Residential);
    }

    #[test]
    fn test_cycle_building() {
        let mut tool = DesignTool::new();
        assert_eq!(tool.current_building, BuildingType::House);
        tool.cycle_building();
        assert_eq!(tool.current_building, BuildingType::Apartment);
        tool.cycle_building();
        assert_eq!(tool.current_building, BuildingType::Shop);
    }

    #[test]
    fn test_brush_size() {
        let mut tool = DesignTool::new();
        assert_eq!(tool.brush_size, DEFAULT_BRUSH_SIZE);

        tool.increase_brush();
        assert_eq!(tool.brush_size, DEFAULT_BRUSH_SIZE + 1);

        tool.decrease_brush();
        assert_eq!(tool.brush_size, DEFAULT_BRUSH_SIZE);

        // No puede bajar de 1
        for _ in 0..100 {
            tool.decrease_brush();
        }
        assert_eq!(tool.brush_size, MIN_BRUSH_SIZE);

        // No puede subir de MAX
        for _ in 0..100 {
            tool.increase_brush();
        }
        assert_eq!(tool.brush_size, MAX_BRUSH_SIZE);
    }

    #[test]
    fn test_place_building() {
        let (mut tool, mut gw) = setup_tool_and_world();

        let buildings_before = gw.world.query::<&ConstructionState>().iter().count();

        tool.active = true;
        tool.mode = DesignMode::PlaceBuilding;
        tool.current_building = BuildingType::Shop;
        tool.execute_place_building(&mut gw, 50.0, 50.0);

        let buildings_after = gw.world.query::<&ConstructionState>().iter().count();
        assert_eq!(buildings_after, buildings_before + 1);

        // Debe haber undo
        assert_eq!(tool.undo_stack.len(), 1);
        assert!(tool.redo_stack.is_empty());
    }

    #[test]
    fn test_paint_zone() {
        let (mut tool, mut gw) = setup_tool_and_world();

        let zones_before = gw.world.query::<&ZoneComponent>().iter()
            .filter(|(_, z)| z.density > 0)
            .count();

        tool.active = true;
        tool.mode = DesignMode::PaintZone;
        tool.current_zone = ZoneType::Commercial;
        tool.brush_density = 3;
        tool.execute_paint_zone(&mut gw, 80.0, 80.0, 82.0, 82.0);

        let zones_after = gw.world.query::<&ZoneComponent>().iter()
            .filter(|(_, z)| z.density > 0)
            .count();

        assert!(zones_after >= zones_before + 9, // 3x3 area
            "Esperaba al menos {} zonas, hay {}", zones_before + 9, zones_after);

        assert_eq!(tool.undo_stack.len(), 1);
    }

    #[test]
    fn test_undo_redo() {
        let (mut tool, mut gw) = setup_tool_and_world();

        let before = gw.world.query::<&ConstructionState>().iter().count();

        tool.active = true;
        tool.mode = DesignMode::PlaceBuilding;
        tool.execute_place_building(&mut gw, 50.0, 50.0);
        tool.execute_place_building(&mut gw, 55.0, 55.0);

        let after_place = gw.world.query::<&ConstructionState>().iter().count();
        assert_eq!(after_place, before + 2);

        // Undo
        tool.undo(&mut gw);
        let after_undo = gw.world.query::<&ConstructionState>().iter().count();
        assert_eq!(after_undo, before + 1);

        // Redo
        tool.redo(&mut gw);
        let after_redo = gw.world.query::<&ConstructionState>().iter().count();
        assert_eq!(after_redo, before + 2);
    }

    #[test]
    fn test_remove_building() {
        let (mut tool, mut gw) = setup_tool_and_world();

        let before = gw.world.query::<&ConstructionState>().iter().count();

        tool.active = true;
        tool.mode = DesignMode::PlaceBuilding;
        tool.execute_place_building(&mut gw, 90.0, 90.0);

        let after_place = gw.world.query::<&ConstructionState>().iter().count();
        assert_eq!(after_place, before + 1);

        // Eliminar
        tool.execute_remove_building(&mut gw, 90.0, 90.0);

        let after_remove = gw.world.query::<&ConstructionState>().iter().count();
        assert_eq!(after_remove, before);

        // Undo de la eliminación debe restaurar
        tool.undo(&mut gw);
        let after_undo = gw.world.query::<&ConstructionState>().iter().count();
        assert_eq!(after_undo, before + 1);
    }
}
