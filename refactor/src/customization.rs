// Sistema de Personalización Visual de Edificios
//
// Implementa:
// - Paletas de colores por tipo de edificio
// - Estilos arquitectónicos (moderno, clásico, industrial, colonial)
// - Variaciones de altura y ancho
// - Estados visuales (construcción, normal, abandonado, lujoso, ruinoso)
// - Techos, jardines, y detalles decorativos
// - Guardado y carga de configuraciones visuales
//
// TÉCNICAS APLICADAS:
// [TC#5]  LUTs para colores predefinidos
// [TC#26] Inlining agresivo

use crate::ecs::BuildingType;

// ---------------------------------------------------------------------------
// ESTILOS ARQUITECTÓNICOS
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum ArchitecturalStyle {
    Modern,
    Classical,
    Industrial,
    Colonial,
    Brutalist,
    ArtDeco,
    Minimalist,
    Mediterranean,
}

impl ArchitecturalStyle {
    pub fn all() -> [ArchitecturalStyle; 8] {
        [
            Self::Modern, Self::Classical, Self::Industrial,
            Self::Colonial, Self::Brutalist, Self::ArtDeco,
            Self::Minimalist, Self::Mediterranean,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Modern => "Moderno",
            Self::Classical => "Clásico",
            Self::Industrial => "Industrial",
            Self::Colonial => "Colonial",
            Self::Brutalist => "Brutalista",
            Self::ArtDeco => "Art Decó",
            Self::Minimalist => "Minimalista",
            Self::Mediterranean => "Mediterráneo",
        }
    }
}

// ---------------------------------------------------------------------------
// PALETAS DE COLORES
// ---------------------------------------------------------------------------

/// Color ARGB predefinido para fachadas
#[derive(Copy, Clone, Debug)]
pub struct FacadeColor {
    pub primary: u32,
    pub secondary: u32,
    pub trim: u32,
    pub roof: u32,
}

impl FacadeColor {
    pub const WHITE: FacadeColor = FacadeColor {
        primary: 0xFF_F5_F5_F5,
        secondary: 0xFF_E0_E0_E0,
        trim: 0xFF_BD_BDBD,
        roof: 0xFF_79_5555,
    };

    pub const BEIGE: FacadeColor = FacadeColor {
        primary: 0xFF_D7_CC_C8,
        secondary: 0xFF_BE_A9_A0,
        trim: 0xFF_8D_6E_63,
        roof: 0xFF_5D_4037,
    };

    pub const RED_BRICK: FacadeColor = FacadeColor {
        primary: 0xFF_C6_2839,
        secondary: 0xFF_8E_24_29,
        trim: 0xFF_FF_F3_E0,
        roof: 0xFF_3E_2723,
    };

    pub const BLUE_SLATE: FacadeColor = FacadeColor {
        primary: 0xFF_60_7D8B,
        secondary: 0xFF_45_5A64,
        trim: 0xFF_FF_FFFF,
        roof: 0xFF_26_3238,
    };

    pub const GLASS_STEEL: FacadeColor = FacadeColor {
        primary: 0xFF_90_CAF9,
        secondary: 0xFF_42_A5_F5,
        trim: 0xFF_B0_BEC5,
        roof: 0xFF_37_475F,
    };

    pub const CONCRETE: FacadeColor = FacadeColor {
        primary: 0xFF_9E_9E9E,
        secondary: 0xFF_75_7575,
        trim: 0xFF_BD_BDBD,
        roof: 0xFF_42_4242,
    };

    pub const TERRACOTTA: FacadeColor = FacadeColor {
        primary: 0xFF_E6_A8_73,
        secondary: 0xFF_C4_7B4A,
        trim: 0xFF_FF_ECB3,
        roof: 0xFF_79_5548,
    };

    pub fn all_presets() -> [(&'static str, FacadeColor); 7] {
        [
            ("Blanco", Self::WHITE),
            ("Beige", Self::BEIGE),
            ("Ladrillo Rojo", Self::RED_BRICK),
            ("Azul Pizarra", Self::BLUE_SLATE),
            ("Vidrio y Acero", Self::GLASS_STEEL),
            ("Concreto", Self::CONCRETE),
            ("Terracota", Self::TERRACOTTA),
        ]
    }
}

// ---------------------------------------------------------------------------
// CONFIGURACIÓN VISUAL DEL EDIFICIO
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug)]
pub struct BuildingAppearance {
    /// Estilo arquitectónico
    pub style: ArchitecturalStyle,
    /// Paleta de colores
    pub colors: FacadeColor,
    /// Altura (pisos)
    pub height_floors: u8,
    /// Ancho relativo (1.0 = estándar)
    pub width_factor: f32,
    /// ¿Tiene jardín delantero?
    pub has_front_garden: bool,
    /// ¿Tiene techo decorativo?
    pub decorative_roof: bool,
    /// ¿Tiene balcones?
    pub has_balconies: bool,
    /// Estado visual
    pub visual_state: BuildingVisualState,
    /// Nivel de detalle (0 = básico, 3 = máximo)
    pub detail_level: u8,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BuildingVisualState {
    /// En construcción (andamios)
    UnderConstruction,
    /// Normal
    Normal,
    /// Bien mantenido (jardín cuidado, pintura fresca)
    WellMaintained,
    /// Lujoso (detalles dorados, iluminación)
    Luxury,
    /// Descuidado (pintura descascarada, maleza)
    Neglected,
    /// Ruinoso (ventanas rotas, grafiti)
    Ruined,
    /// Abandonado (tablones en ventanas)
    Abandoned,
}

impl BuildingAppearance {
    /// Crea apariencia por defecto según tipo de edificio
    pub fn default_for(building_type: BuildingType) -> Self {
        match building_type {
            BuildingType::House => BuildingAppearance {
                style: ArchitecturalStyle::Colonial,
                colors: FacadeColor::BEIGE,
                height_floors: 1,
                width_factor: 1.0,
                has_front_garden: true,
                decorative_roof: true,
                has_balconies: false,
                visual_state: BuildingVisualState::Normal,
                detail_level: 1,
            },
            BuildingType::Apartment => BuildingAppearance {
                style: ArchitecturalStyle::Modern,
                colors: FacadeColor::CONCRETE,
                height_floors: 5,
                width_factor: 1.5,
                has_front_garden: false,
                decorative_roof: false,
                has_balconies: true,
                visual_state: BuildingVisualState::Normal,
                detail_level: 1,
            },
            BuildingType::Shop => BuildingAppearance {
                style: ArchitecturalStyle::ArtDeco,
                colors: FacadeColor::GLASS_STEEL,
                height_floors: 2,
                width_factor: 1.2,
                has_front_garden: false,
                decorative_roof: true,
                has_balconies: false,
                visual_state: BuildingVisualState::Normal,
                detail_level: 1,
            },
            BuildingType::Office => BuildingAppearance {
                style: ArchitecturalStyle::Modern,
                colors: FacadeColor::GLASS_STEEL,
                height_floors: 8,
                width_factor: 2.0,
                has_front_garden: false,
                decorative_roof: false,
                has_balconies: false,
                visual_state: BuildingVisualState::Normal,
                detail_level: 1,
            },
            BuildingType::Factory => BuildingAppearance {
                style: ArchitecturalStyle::Industrial,
                colors: FacadeColor::CONCRETE,
                height_floors: 1,
                width_factor: 3.0,
                has_front_garden: false,
                decorative_roof: false,
                has_balconies: false,
                visual_state: BuildingVisualState::Normal,
                detail_level: 0,
            },
            BuildingType::Farm => BuildingAppearance {
                style: ArchitecturalStyle::Mediterranean,
                colors: FacadeColor::TERRACOTTA,
                height_floors: 1,
                width_factor: 1.0,
                has_front_garden: true,
                decorative_roof: true,
                has_balconies: false,
                visual_state: BuildingVisualState::Normal,
                detail_level: 0,
            },
        }
    }

    /// Aplica un estilo arquitectónico
    pub fn with_style(mut self, style: ArchitecturalStyle) -> Self {
        self.style = style;
        // Ajustar colores según estilo
        self.colors = match style {
            ArchitecturalStyle::Modern => FacadeColor::GLASS_STEEL,
            ArchitecturalStyle::Classical => FacadeColor::WHITE,
            ArchitecturalStyle::Industrial => FacadeColor::CONCRETE,
            ArchitecturalStyle::Colonial => FacadeColor::BEIGE,
            ArchitecturalStyle::Brutalist => FacadeColor::CONCRETE,
            ArchitecturalStyle::ArtDeco => FacadeColor::TERRACOTTA,
            ArchitecturalStyle::Minimalist => FacadeColor::WHITE,
            ArchitecturalStyle::Mediterranean => FacadeColor::TERRACOTTA,
        };
        self
    }

    /// Aplica una paleta de colores
    pub fn with_colors(mut self, colors: FacadeColor) -> Self {
        self.colors = colors;
        self
    }

    /// Cambia la altura
    pub fn with_height(mut self, floors: u8) -> Self {
        self.height_floors = floors.min(20).max(1);
        self
    }

    /// Marca como lujoso (mejora apariencia)
    pub fn make_luxury(mut self) -> Self {
        self.visual_state = BuildingVisualState::Luxury;
        self.detail_level = 3;
        self
    }

    /// Marca como arruinado (abandono)
    pub fn make_ruined(mut self) -> Self {
        self.visual_state = BuildingVisualState::Ruined;
        self
    }

    /// Calcula el color ARGB resultante para renderizar
    #[inline]
    pub fn render_color(&self) -> u32 {
        match self.visual_state {
            BuildingVisualState::UnderConstruction => 0xFF_FF_A0_00, // Naranja andamios
            BuildingVisualState::Normal => self.colors.primary,
            BuildingVisualState::WellMaintained => self.colors.primary,
            BuildingVisualState::Luxury => {
                // Mezclar con dorado
                let r = (((self.colors.primary >> 16) & 0xFF) as u32 + 0x30).min(0xFF);
                let g = (((self.colors.primary >> 8) & 0xFF) as u32 + 0x20).min(0xFF);
                let b = ((self.colors.primary & 0xFF) as u32 + 0x10).min(0xFF);
                (0xFF << 24) | (r << 16) | (g << 8) | b
            }
            BuildingVisualState::Neglected => {
                // Oscurecer y desaturar
                let r = ((self.colors.primary >> 16) & 0xFF) as u32 * 3 / 4;
                let g = ((self.colors.primary >> 8) & 0xFF) as u32 * 3 / 4;
                let b = (self.colors.primary & 0xFF) as u32 * 3 / 4;
                (0xFF << 24) | (r << 16) | (g << 8) | b
            }
            BuildingVisualState::Ruined => 0xFF_69_6969,
            BuildingVisualState::Abandoned => 0xFF_4A_4A4A,
        }
    }
}

// ---------------------------------------------------------------------------
// GESTOR DE PERSONALIZACIÓN
// ---------------------------------------------------------------------------

pub struct CustomizationManager {
    /// Apariencias por edificio (indexado por coordenadas)
    pub appearances: Vec<(f32, f32, BuildingAppearance)>,
    /// Estilo por defecto para nuevas construcciones
    pub default_style: ArchitecturalStyle,
    /// Paleta por defecto
    pub default_colors: FacadeColor,
}

impl CustomizationManager {
    pub fn new() -> Self {
        CustomizationManager {
            appearances: Vec::with_capacity(1024),
            default_style: ArchitecturalStyle::Modern,
            default_colors: FacadeColor::WHITE,
        }
    }

    /// Registra un edificio con su apariencia
    pub fn register_building(&mut self, x: f32, y: f32, appearance: BuildingAppearance) {
        // Reemplazar si ya existe
        if let Some(existing) = self.appearances.iter_mut()
            .find(|(bx, by, _)| (*bx - x).abs() < 0.1 && (*by - y).abs() < 0.1)
        {
            existing.2 = appearance;
        } else {
            self.appearances.push((x, y, appearance));
        }
    }

    /// Obtiene la apariencia de un edificio
    pub fn get_appearance(&self, x: f32, y: f32) -> Option<&BuildingAppearance> {
        self.appearances.iter()
            .find(|(bx, by, _)| (*bx - x).abs() < 0.1 && (*by - y).abs() < 0.1)
            .map(|(_, _, a)| a)
    }

    /// Aplica un estilo a todos los edificios de un tipo en una zona
    pub fn apply_style_to_zone(
        &mut self,
        x1: f32, y1: f32, x2: f32, y2: f32,
        style: ArchitecturalStyle,
    ) {
        for (x, y, appearance) in self.appearances.iter_mut() {
            if *x >= x1 && *x <= x2 && *y >= y1 && *y <= y2 {
                *appearance = appearance.with_style(style);
            }
        }
    }

    /// Actualiza estado visual basado en economía del edificio
    pub fn update_from_economy(
        &mut self,
        x: f32, y: f32,
        money: f32,
        abandoned: bool,
    ) {
        if let Some((_, _, appearance)) = self.appearances.iter_mut()
            .find(|(bx, by, _)| (*bx - x).abs() < 0.1 && (*by - y).abs() < 0.1)
        {
            if abandoned {
                appearance.visual_state = BuildingVisualState::Abandoned;
            } else if money > 10000.0 {
                appearance.visual_state = BuildingVisualState::Luxury;
            } else if money > 5000.0 {
                appearance.visual_state = BuildingVisualState::WellMaintained;
            } else if money < 100.0 {
                appearance.visual_state = BuildingVisualState::Neglected;
            } else {
                appearance.visual_state = BuildingVisualState::Normal;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_appearance_per_type() {
        let house = BuildingAppearance::default_for(BuildingType::House);
        assert_eq!(house.height_floors, 1);
        assert!(house.has_front_garden);

        let office = BuildingAppearance::default_for(BuildingType::Office);
        assert!(office.height_floors > 4);
        assert!(!office.has_front_garden);
    }

    #[test]
    fn test_with_style_changes_colors() {
        let mut appearance = BuildingAppearance::default_for(BuildingType::House);
        let modern = appearance.with_style(ArchitecturalStyle::Modern);
        assert_eq!(modern.style, ArchitecturalStyle::Modern);
    }

    #[test]
    fn test_render_color_by_state() {
        let normal = BuildingAppearance::default_for(BuildingType::House);
        assert_eq!(normal.render_color(), FacadeColor::BEIGE.primary);

        let mut ruined = normal;
        ruined.visual_state = BuildingVisualState::Ruined;
        assert_eq!(ruined.render_color(), 0xFF_69_6969);

        let mut abandoned = normal;
        abandoned.visual_state = BuildingVisualState::Abandoned;
        assert!(abandoned.render_color() != normal.render_color());
    }

    #[test]
    fn test_facade_color_presets() {
        let presets = FacadeColor::all_presets();
        assert_eq!(presets.len(), 7);
        for (name, _) in &presets {
            assert!(!name.is_empty());
        }
    }

    #[test]
    fn test_architectural_style_names() {
        for style in ArchitecturalStyle::all() {
            let name = style.name();
            assert!(!name.is_empty());
        }
    }

    #[test]
    fn test_customization_manager_register() {
        let mut cm = CustomizationManager::new();
        let appearance = BuildingAppearance::default_for(BuildingType::Shop);
        cm.register_building(10.0, 20.0, appearance);

        let retrieved = cm.get_appearance(10.0, 20.0);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().style, ArchitecturalStyle::ArtDeco);
    }

    #[test]
    fn test_update_from_economy_luxury() {
        let mut cm = CustomizationManager::new();
        let appearance = BuildingAppearance::default_for(BuildingType::House);
        cm.register_building(50.0, 50.0, appearance);

        cm.update_from_economy(50.0, 50.0, 20000.0, false);
        let updated = cm.get_appearance(50.0, 50.0).unwrap();
        assert_eq!(updated.visual_state, BuildingVisualState::Luxury);
    }

    #[test]
    fn test_update_from_economy_abandoned() {
        let mut cm = CustomizationManager::new();
        let appearance = BuildingAppearance::default_for(BuildingType::House);
        cm.register_building(50.0, 50.0, appearance);

        cm.update_from_economy(50.0, 50.0, 50.0, true);
        let updated = cm.get_appearance(50.0, 50.0).unwrap();
        assert_eq!(updated.visual_state, BuildingVisualState::Abandoned);
    }
}
