// Tipos de Edificios — Catálogo Completo v0.12.0
//
// Implementa más de 100 tipos de edificios con mecánicas distópicas:
// - Agencias de calificación crediticia (bajan rating si gastas en escuelas)
// - Paraísos fiscales offshore (drenan ingresos)
// - Firmas de abogados troll de patentes
// - Bolsa de futuros de agua potable
// - Centros de minería de datos biométricos
// - Y muchos más...
//
// Cada edificio tiene:
// - Propiedades físicas (tamaño, costo, material)
// - Efectos económicos (impuestos, ingresos, costos)
// - Efectos sociales (felicidad, salud, educación)
// - Efectos ambientales (contaminación, agua, suelo)
// - Interacciones con otros sistemas (legal, financiero, supply chain)
//
// TÉCNICAS:
// - Look-Up Tables para efectos de edificios [TC#5]
// - Bitboards para ocupación de suelo [TI#6]
// - Strings internados para nombres
// - Object pooling para instancias de edificios

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CATEGORÍAS DE EDIFICIOS
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum BuildingCategory {
    // Gobierno y Burocracia
    Government = 0,
    // Finanzas y Banca
    Finance = 1,
    // Legal
    Legal = 2,
    // Salud
    Healthcare = 3,
    // Educación
    Education = 4,
    // Residencial
    Residential = 5,
    // Comercial
    Commercial = 6,
    // Industrial
    Industrial = 7,
    // Energía
    Energy = 8,
    // Agua y Saneamiento
    Water = 9,
    // Transporte
    Transport = 10,
    // Agricultura
    Agriculture = 11,
    // Tecnología
    Technology = 12,
    // Entretenimiento y Turismo
    Entertainment = 13,
    // Seguridad y Emergencias
    Security = 14,
    // Residuos
    Waste = 15,
    // Militar
    Military = 16,
    // Mercado Negro / Underground
    BlackMarket = 17,
    // Culto / Religión
    Religion = 18,
    // Deportes
    Sports = 19,
}

// ---------------------------------------------------------------------------
// ESTILO ARQUITECTÓNICO
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum ArchitectureStyle {
    Brutalist = 0,
    GlassTower = 1,
    Neoclassical = 2,
    Industrial = 3,
    Underground = 4,
    EcoFriendly = 5,
    Cyberpunk = 6,
    SovietBlock = 7,
    Colonial = 8,
    HighTech = 9,
}

// ---------------------------------------------------------------------------
// EFECTOS DE EDIFICIO (LUT precalculada)
// ---------------------------------------------------------------------------

/// Efectos que un edificio aplica pasivamente a la simulación
#[derive(Debug, Clone, Copy, Default)]
pub struct BuildingEffects {
    // Económicos
    pub tax_revenue_annual: f64,       // ingresos fiscales que genera
    pub operating_cost_annual: f64,    // costo operativo
    pub jobs_created: u32,             // empleos generados
    pub land_value_multiplier: f32,    // multiplicador del valor del suelo (1.0 = neutro)
    pub gentrification_speed: f32,     // velocidad de gentrificación (0.0-1.0)

    // Sociales
    pub happiness_effect: f32,         // efecto en felicidad (-1.0 a 1.0)
    pub health_effect: f32,            // efecto en salud
    pub education_effect: f32,         // efecto en educación
    pub crime_effect: f32,             // efecto en criminalidad
    pub privacy_index: f32,            // índice de privacidad (0.0 = vigilancia total)

    // Ambientales
    pub air_pollution: f32,            // contaminación del aire (0.0-1.0)
    pub water_pollution: f32,          // contaminación del agua
    pub soil_pollution: f32,           // contaminación del suelo
    pub noise_pollution: f32,          // contaminación acústica
    pub light_pollution: f32,          // contaminación lumínica
    pub radiation_emission: f32,       // emisión de radiación
    pub water_consumption: f32,        // consumo de agua (m³/día)
    pub electricity_consumption: f32,  // consumo eléctrico (MW)
    pub waste_generation: f32,         // generación de residuos

    // Redes
    pub fiber_traffic: f32,            // tráfico de fibra óptica
    pub traffic_generation: f32,       // generación de tráfico vehicular
    pub pedestrian_traffic: f32,       // tráfico peatonal
}

// ---------------------------------------------------------------------------
// EDIFICIO BASE
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct BuildingTemplate {
    pub id: u16,
    pub name: &'static str,
    pub description: &'static str,
    pub category: BuildingCategory,
    pub style: ArchitectureStyle,
    pub width: u8,                     // ancho en celdas
    pub height: u8,                    // alto en celdas
    pub construction_cost: f64,        // costo de construcción
    pub construction_time_days: u32,   // días para construir
    pub max_occupancy: u32,            // ocupación máxima
    pub effects: BuildingEffects,      // efectos pasivos
    pub requires_water: bool,
    pub requires_electricity: bool,
    pub requires_fiber: bool,
    pub requires_road_access: bool,
    /// Edificios que no pueden estar cerca (NIMBY)
    pub nimby_radius: u8,
    /// Solo puede construirse cerca de
    pub requires_nearby: Option<&'static str>,
}

// ---------------------------------------------------------------------------
// CATÁLOGO DE EDIFICIOS (LUT estática)
// ---------------------------------------------------------------------------

/// Catálogo completo de todos los tipos de edificios.
/// Indexado por BuildingTemplate.id para acceso O(1).
pub struct BuildingCatalog {
    pub templates: Vec<BuildingTemplate>,
}

impl BuildingCatalog {
    /// Construye el catálogo con todos los edificios predefinidos
    pub fn new() -> Self {
        let mut templates = Vec::with_capacity(256);

        // =========================================================================
        // 1. GOBIERNO Y BUROCRACIA
        // =========================================================================

        // 0: Alcaldía / Ayuntamiento
        templates.push(BuildingTemplate {
            id: 0,
            name: "Alcaldía Central",
            description: "Sede del gobierno municipal. Sin esto no hay ciudad.",
            category: BuildingCategory::Government,
            style: ArchitectureStyle::Neoclassical,
            width: 3, height: 3,
            construction_cost: 5_000_000.0,
            construction_time_days: 180,
            max_occupancy: 200,
            effects: BuildingEffects {
                tax_revenue_annual: 0.0,
                operating_cost_annual: 2_000_000.0,
                jobs_created: 200,
                land_value_multiplier: 1.3,
                gentrification_speed: 0.02,
                happiness_effect: 0.05,
                traffic_generation: 5.0,
                pedestrian_traffic: 10.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 1: Oficina de Catastro y Zonificación
        templates.push(BuildingTemplate {
            id: 1,
            name: "Oficina de Catastro",
            description: "Registra propiedades y gestiona zonificación. La burocracia hecha edificio.",
            category: BuildingCategory::Government,
            style: ArchitectureStyle::Brutalist,
            width: 2, height: 2,
            construction_cost: 800_000.0,
            construction_time_days: 90,
            max_occupancy: 50,
            effects: BuildingEffects {
                operating_cost_annual: 400_000.0,
                jobs_created: 50,
                land_value_multiplier: 1.05,
                traffic_generation: 1.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 2: Agencia de Calificación Crediticia (CREDIT RATING AGENCY - ¡JEFE FINAL!)
        templates.push(BuildingTemplate {
            id: 2,
            name: "Agencia de Calificación Crediticia",
            description: "IA intocable. Si gastas en escuelas en vez de pagar deuda, te bajan el rating. Multiplica tasas de interés por 3.",
            category: BuildingCategory::Finance,
            style: ArchitectureStyle::GlassTower,
            width: 3, height: 5,
            construction_cost: 15_000_000.0,
            construction_time_days: 300,
            max_occupancy: 500,
            effects: BuildingEffects {
                tax_revenue_annual: 5_000_000.0,
                operating_cost_annual: 8_000_000.0,
                jobs_created: 500,
                land_value_multiplier: 1.5,
                gentrification_speed: 0.08,
                happiness_effect: -0.1,
                traffic_generation: 3.0,
                electricity_consumption: 5.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Financial"),
        });

        // 3: Buzón Legal / Paraíso Fiscal Offshore
        templates.push(BuildingTemplate {
            id: 3,
            name: "Buzón Legal Offshore",
            description: "2x2 metros. Alberga 5,000 empresas fantasma. Drena 10% de ingresos fiscales. Si lo allanas, las empresas huyen.",
            category: BuildingCategory::Finance,
            style: ArchitectureStyle::GlassTower,
            width: 1, height: 1,
            construction_cost: 200_000.0,
            construction_time_days: 30,
            max_occupancy: 5,
            effects: BuildingEffects {
                tax_revenue_annual: -50_000.0,  // DRENA en vez de generar
                operating_cost_annual: 10_000.0,
                jobs_created: 5,
                land_value_multiplier: 1.1,
                happiness_effect: -0.02,
                crime_effect: 0.01,
                traffic_generation: 0.1,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 0,
            requires_nearby: Some("Financial"),
        });

        // 4: Firma de Abogados Troll de Patentes
        templates.push(BuildingTemplate {
            id: 4,
            name: "Firma Troll de Patentes",
            description: "Chupan vitalidad de zonas tech. Demandan fábricas de IA. Reducen innovación 40%. Solo frenable sobornando jueces.",
            category: BuildingCategory::Legal,
            style: ArchitectureStyle::GlassTower,
            width: 2, height: 3,
            construction_cost: 3_000_000.0,
            construction_time_days: 120,
            max_occupancy: 100,
            effects: BuildingEffects {
                tax_revenue_annual: 200_000.0,
                operating_cost_annual: 500_000.0,
                jobs_created: 100,
                land_value_multiplier: 0.9,
                happiness_effect: -0.05,
                crime_effect: 0.02,
                traffic_generation: 1.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Technology"),
        });

        // 5: Bolsa de Futuros de Agua Potable
        templates.push(BuildingTemplate {
            id: 5,
            name: "Bolsa de Futuros de Agua",
            description: "Cuando el agua escasea, apuestan sobre su precio. Desconecta precio del agua del costo de bombeo. RNG del mercado.",
            category: BuildingCategory::Finance,
            style: ArchitectureStyle::GlassTower,
            width: 2, height: 4,
            construction_cost: 10_000_000.0,
            construction_time_days: 200,
            max_occupancy: 200,
            effects: BuildingEffects {
                tax_revenue_annual: 2_000_000.0,
                operating_cost_annual: 1_500_000.0,
                jobs_created: 200,
                land_value_multiplier: 1.3,
                happiness_effect: -0.15,
                water_consumption: 0.1,
                electricity_consumption: 2.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 2,
            requires_nearby: Some("Financial"),
        });

        // 6: Centro de Minería de Datos Biométricos
        templates.push(BuildingTemplate {
            id: 6,
            name: "Minería de Datos Biométricos",
            description: "Compran caras y huellas de NPCs. Privacidad a cero. Satura fibra óptica, dejando semáforos sin ancho de banda.",
            category: BuildingCategory::Technology,
            style: ArchitectureStyle::Cyberpunk,
            width: 3, height: 4,
            construction_cost: 12_000_000.0,
            construction_time_days: 250,
            max_occupancy: 300,
            effects: BuildingEffects {
                tax_revenue_annual: 3_000_000.0,
                operating_cost_annual: 4_000_000.0,
                jobs_created: 300,
                land_value_multiplier: 0.8,
                privacy_index: -0.5,
                happiness_effect: -0.2,
                fiber_traffic: 50.0,
                electricity_consumption: 10.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 3,
            requires_nearby: Some("Technology"),
        });

        // 7: Centro Privado de Extracción de Plasma
        templates.push(BuildingTemplate {
            id: 7,
            name: "Centro de Extracción de Plasma",
            description: "Paga monedas a NPCs pobres por sangre. Vende cara a hospitales premium. Donantes pierden stamina permanente.",
            category: BuildingCategory::Healthcare,
            style: ArchitectureStyle::Brutalist,
            width: 2, height: 2,
            construction_cost: 1_500_000.0,
            construction_time_days: 90,
            max_occupancy: 80,
            effects: BuildingEffects {
                tax_revenue_annual: 500_000.0,
                operating_cost_annual: 300_000.0,
                jobs_created: 80,
                land_value_multiplier: 0.7,
                health_effect: -0.1,
                happiness_effect: -0.1,
                crime_effect: 0.03,
                traffic_generation: 2.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 5,
            requires_nearby: Some("Residential_Low"),
        });

        // 8: Oficina de Lobbying Político (K Street)
        templates.push(BuildingTemplate {
            id: 8,
            name: "Oficina de Lobby Político",
            description: "NPCs corporativos exigen bajar impuestos al diésel. Si rechazas: difamación. Si aceptas: smog.",
            category: BuildingCategory::Government,
            style: ArchitectureStyle::Neoclassical,
            width: 2, height: 3,
            construction_cost: 4_000_000.0,
            construction_time_days: 150,
            max_occupancy: 150,
            effects: BuildingEffects {
                tax_revenue_annual: 100_000.0,
                operating_cost_annual: 600_000.0,
                jobs_created: 150,
                land_value_multiplier: 1.2,
                happiness_effect: -0.05,
                crime_effect: 0.05,
                traffic_generation: 2.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Government"),
        });

        // 9: Clínica de Seguros Médicos - Departamento de Denegación
        templates.push(BuildingTemplate {
            id: 9,
            name: "Depto. de Denegación de Seguros",
            description: "Burócratas deniegan cirugías. Reduce gasto hospitalario. Denegados van a clínicas clandestinas. Mortalidad oculta.",
            category: BuildingCategory::Healthcare,
            style: ArchitectureStyle::Brutalist,
            width: 2, height: 2,
            construction_cost: 2_000_000.0,
            construction_time_days: 100,
            max_occupancy: 120,
            effects: BuildingEffects {
                tax_revenue_annual: 1_000_000.0,
                operating_cost_annual: 800_000.0,
                jobs_created: 120,
                health_effect: -0.15,
                happiness_effect: -0.15,
                traffic_generation: 1.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 3,
            requires_nearby: Some("Commercial"),
        });

        // 10: Monopolio de Semillas GMO
        templates.push(BuildingTemplate {
            id: 10,
            name: "Monopolio de Semillas GMO",
            description: "Controla agricultura. Obliga comprar semillas cada temporada. Drones vigilan granjeros. Demandas y expropiaciones.",
            category: BuildingCategory::Agriculture,
            style: ArchitectureStyle::Industrial,
            width: 3, height: 3,
            construction_cost: 5_000_000.0,
            construction_time_days: 200,
            max_occupancy: 150,
            effects: BuildingEffects {
                tax_revenue_annual: 2_500_000.0,
                operating_cost_annual: 1_500_000.0,
                jobs_created: 150,
                happiness_effect: -0.1,
                soil_pollution: 0.1,
                water_consumption: 5.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Agriculture"),
        });

        // 11: Compañía Privada de Agua
        templates.push(BuildingTemplate {
            id: 11,
            name: "Compañía Privada de Agua",
            description: "Vendiste tu red de agua. Te inyectan $1B. Pero mantenimiento bloqueado. Tuberías se pudren. Fugas masivas.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::GlassTower,
            width: 2, height: 3,
            construction_cost: 500_000.0,  // Lo "construye" la empresa
            construction_time_days: 60,
            max_occupancy: 100,
            effects: BuildingEffects {
                tax_revenue_annual: 1_000_000_000.0,  // Inyección única
                operating_cost_annual: 200_000.0,
                jobs_created: 100,
                happiness_effect: -0.3,
                water_pollution: 0.2,
                water_consumption: 0.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Water"),
        });

        // 12: Hyperloop Subterráneo
        templates.push(BuildingTemplate {
            id: 12,
            name: "Terminal Hyperloop Subterránea",
            description: "Cápsulas a 1000 km/h. Si un sismo fractura el tubo: implosión. 500 NPCs muertos en 1 frame.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::HighTech,
            width: 4, height: 3,
            construction_cost: 50_000_000.0,
            construction_time_days: 500,
            max_occupancy: 1000,
            effects: BuildingEffects {
                tax_revenue_annual: 10_000_000.0,
                operating_cost_annual: 15_000_000.0,
                jobs_created: 500,
                land_value_multiplier: 2.0,
                gentrification_speed: 0.15,
                electricity_consumption: 50.0,
                traffic_generation: 10.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 5,
            requires_nearby: Some("Transport"),
        });

        // 13: Intersección de Autopista de 5 Niveles
        templates.push(BuildingTemplate {
            id: 13,
            name: "Spaghetti Junction (5 Niveles)",
            description: "16 manzanas de cemento. Infierno para A*. Contaminación acústica amplificada. Suelo estéril bajo ella.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::Brutalist,
            width: 4, height: 4,
            construction_cost: 25_000_000.0,
            construction_time_days: 400,
            max_occupancy: 0,
            effects: BuildingEffects {
                operating_cost_annual: 1_000_000.0,
                jobs_created: 20,
                land_value_multiplier: 0.3,
                happiness_effect: -0.1,
                air_pollution: 0.3,
                noise_pollution: 1.0,
                soil_pollution: 0.2,
                traffic_generation: 100.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 8,
            requires_nearby: Some("Transport"),
        });

        // =========================================================================
        // 2. SALUD Y HOSPITALES
        // =========================================================================

        // 14: Hospital Público General
        templates.push(BuildingTemplate {
            id: 14,
            name: "Hospital Público General",
            description: "Atiende a todos. Carísimo de mantener. Si colapsa, ambulancias deambulan con pacientes.",
            category: BuildingCategory::Healthcare,
            style: ArchitectureStyle::Brutalist,
            width: 3, height: 4,
            construction_cost: 20_000_000.0,
            construction_time_days: 365,
            max_occupancy: 800,
            effects: BuildingEffects {
                operating_cost_annual: 12_000_000.0,
                jobs_created: 800,
                land_value_multiplier: 1.1,
                health_effect: 0.3,
                happiness_effect: 0.1,
                traffic_generation: 15.0,
                water_consumption: 20.0,
                electricity_consumption: 8.0,
                waste_generation: 3.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 15: Hospital Privado de Alta Complejidad
        templates.push(BuildingTemplate {
            id: 15,
            name: "Hospital Privado Premium",
            description: "Solo para clase alta con seguro. NPCs pobres mueren en la puerta.",
            category: BuildingCategory::Healthcare,
            style: ArchitectureStyle::GlassTower,
            width: 3, height: 5,
            construction_cost: 40_000_000.0,
            construction_time_days: 400,
            max_occupancy: 500,
            effects: BuildingEffects {
                tax_revenue_annual: 3_000_000.0,
                operating_cost_annual: 6_000_000.0,
                jobs_created: 500,
                land_value_multiplier: 1.6,
                gentrification_speed: 0.1,
                health_effect: 0.2,  // Solo para ricos
                happiness_effect: 0.0,
                traffic_generation: 10.0,
                water_consumption: 15.0,
                electricity_consumption: 10.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Residential_Luxury"),
        });

        // 16: Clínica de Modificación Cibernética Clandestina
        templates.push(BuildingTemplate {
            id: 16,
            name: "Clínica Cibernética Clandestina",
            description: "Implantes baratos. Obreros no duermen. Rechazo inmune satura hospitales. PEM paraliza a los implantados.",
            category: BuildingCategory::BlackMarket,
            style: ArchitectureStyle::Cyberpunk,
            width: 1, height: 2,
            construction_cost: 500_000.0,
            construction_time_days: 45,
            max_occupancy: 30,
            effects: BuildingEffects {
                tax_revenue_annual: 0.0,  // No paga impuestos
                operating_cost_annual: 100_000.0,
                jobs_created: 30,
                health_effect: -0.15,
                happiness_effect: 0.05,
                crime_effect: 0.1,
                electricity_consumption: 3.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: false,
            nimby_radius: 2,
            requires_nearby: Some("Residential_Low"),
        });

        // =========================================================================
        // 3. INDUSTRIA Y PRODUCCIÓN
        // =========================================================================

        // 17: Refinería de Petróleo
        templates.push(BuildingTemplate {
            id: 17,
            name: "Refinería de Petróleo",
            description: "Transforma crudo en gasolina, plástico y asfalto. Si hay huelga, la ciudad se queda sin nafta.",
            category: BuildingCategory::Industrial,
            style: ArchitectureStyle::Industrial,
            width: 5, height: 5,
            construction_cost: 35_000_000.0,
            construction_time_days: 500,
            max_occupancy: 600,
            effects: BuildingEffects {
                tax_revenue_annual: 5_000_000.0,
                operating_cost_annual: 8_000_000.0,
                jobs_created: 600,
                land_value_multiplier: 0.2,
                air_pollution: 0.6,
                water_pollution: 0.4,
                soil_pollution: 0.5,
                noise_pollution: 0.8,
                water_consumption: 100.0,
                electricity_consumption: 30.0,
                waste_generation: 10.0,
                traffic_generation: 20.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 16,
            requires_nearby: Some("Industrial"),
        });

        // 18: Fábrica de Hormigón
        templates.push(BuildingTemplate {
            id: 18,
            name: "Fábrica de Hormigón",
            description: "Acelera construcción 40%. Cubre todo de polvo de cemento. Nadie quiere vivir cerca.",
            category: BuildingCategory::Industrial,
            style: ArchitectureStyle::Industrial,
            width: 3, height: 3,
            construction_cost: 8_000_000.0,
            construction_time_days: 200,
            max_occupancy: 150,
            effects: BuildingEffects {
                tax_revenue_annual: 800_000.0,
                operating_cost_annual: 1_500_000.0,
                jobs_created: 150,
                land_value_multiplier: 0.4,
                air_pollution: 0.4,
                soil_pollution: 0.3,
                noise_pollution: 0.7,
                water_consumption: 30.0,
                electricity_consumption: 15.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 10,
            requires_nearby: Some("Industrial"),
        });

        // 19: Matadero y Procesadora Cárnica
        templates.push(BuildingTemplate {
            id: 19,
            name: "Procesadora Cárnica Industrial",
            description: "Estabiliza precio de alimentos. Olor pútrido viaja con el viento. Si el viento cambia, los ricos huyen.",
            category: BuildingCategory::Industrial,
            style: ArchitectureStyle::Industrial,
            width: 3, height: 3,
            construction_cost: 6_000_000.0,
            construction_time_days: 180,
            max_occupancy: 300,
            effects: BuildingEffects {
                tax_revenue_annual: 1_500_000.0,
                operating_cost_annual: 2_000_000.0,
                jobs_created: 300,
                land_value_multiplier: 0.3,
                happiness_effect: -0.05,
                water_pollution: 0.5,
                soil_pollution: 0.4,
                water_consumption: 50.0,
                electricity_consumption: 10.0,
                waste_generation: 8.0,
                traffic_generation: 10.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 12,
            requires_nearby: Some("Industrial"),
        });

        // 20: Fundición de Acero
        templates.push(BuildingTemplate {
            id: 20,
            name: "Fundición de Acero",
            description: "Recicla chatarra. Picos de consumo eléctrico hacen parpadear luces en 3 km. Solo opera de noche.",
            category: BuildingCategory::Industrial,
            style: ArchitectureStyle::Industrial,
            width: 4, height: 4,
            construction_cost: 20_000_000.0,
            construction_time_days: 350,
            max_occupancy: 400,
            effects: BuildingEffects {
                tax_revenue_annual: 2_500_000.0,
                operating_cost_annual: 5_000_000.0,
                jobs_created: 400,
                land_value_multiplier: 0.2,
                air_pollution: 0.5,
                soil_pollution: 0.3,
                noise_pollution: 0.6,
                electricity_consumption: 80.0,  // Pico masivo
                water_consumption: 40.0,
                waste_generation: 5.0,
                traffic_generation: 15.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 14,
            requires_nearby: Some("Industrial"),
        });

        // =========================================================================
        // 4. ENERGÍA
        // =========================================================================

        // 21: Central Térmica de Carbón
        templates.push(BuildingTemplate {
            id: 21,
            name: "Central Térmica de Carbón",
            description: "Base industrial. PM2.5 esparcido por viento. Salud respiratoria cae bajo la pluma de humo.",
            category: BuildingCategory::Energy,
            style: ArchitectureStyle::Industrial,
            width: 4, height: 4,
            construction_cost: 15_000_000.0,
            construction_time_days: 400,
            max_occupancy: 200,
            effects: BuildingEffects {
                tax_revenue_annual: 3_000_000.0,
                operating_cost_annual: 2_000_000.0,
                jobs_created: 200,
                land_value_multiplier: 0.1,
                air_pollution: 0.9,
                soil_pollution: 0.4,
                health_effect: -0.2,
                water_consumption: 200.0,
                electricity_consumption: -50.0,  // Genera electricidad (negativo = produce)
                waste_generation: 15.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 20,
            requires_nearby: Some("Industrial"),
        });

        // 22: Planta Solar
        templates.push(BuildingTemplate {
            id: 22,
            name: "Planta Solar Fotovoltaica",
            description: "Energía limpia. Requiere mucho terreno. Solo genera de día. Puede causar deslumbramiento a pilotos.",
            category: BuildingCategory::Energy,
            style: ArchitectureStyle::EcoFriendly,
            width: 5, height: 3,
            construction_cost: 12_000_000.0,
            construction_time_days: 180,
            max_occupancy: 20,
            effects: BuildingEffects {
                tax_revenue_annual: 100_000.0,
                operating_cost_annual: 200_000.0,
                jobs_created: 20,
                land_value_multiplier: 0.9,
                electricity_consumption: -30.0,
                light_pollution: 0.1,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 5,
            requires_nearby: None,
        });

        // 23: Reactor Nuclear (SMR)
        templates.push(BuildingTemplate {
            id: 23,
            name: "Reactor Modular Pequeño (SMR)",
            description: "Nuclear de barrio. Si los generadores de respaldo se ahogan: meltdown. Irradia napas. Abandona el barrio.",
            category: BuildingCategory::Energy,
            style: ArchitectureStyle::HighTech,
            width: 2, height: 2,
            construction_cost: 80_000_000.0,
            construction_time_days: 600,
            max_occupancy: 80,
            effects: BuildingEffects {
                tax_revenue_annual: 10_000_000.0,
                operating_cost_annual: 3_000_000.0,
                jobs_created: 80,
                land_value_multiplier: 0.2,
                radiation_emission: 0.001,
                electricity_consumption: -200.0,
                water_consumption: 100.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 15,
            requires_nearby: Some("Industrial"),
        });

        // 24: Vertedero Municipal

        templates.push(BuildingTemplate {
            id: 25,
            name: "Incinerador de Residuos",
            description: "Quema basura a 1200°C. Filtros fallan sin presupuesto. Dioxinas al aire.",
            category: BuildingCategory::Waste,
            style: ArchitectureStyle::Industrial,
            width: 3, height: 3,
            construction_cost: 8_000_000.0,
            construction_time_days: 200,
            max_occupancy: 80,
            effects: BuildingEffects {
                tax_revenue_annual: 500_000.0,
                operating_cost_annual: 1_500_000.0,
                jobs_created: 80,
                land_value_multiplier: 0.1,
                air_pollution: 0.4,
                health_effect: -0.05,
                electricity_consumption: 5.0,
                waste_generation: -30.0,
                traffic_generation: 5.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 15,
            requires_nearby: Some("Industrial"),
        });

        // 26: Prisión Federal de Máxima Seguridad
        templates.push(BuildingTemplate {
            id: 26,
            name: "Prisión de Máxima Seguridad",
            description: "Agujero negro de moralidad. Subsidios por cada preso. Si se llena: motines e incendios. Devora valor del suelo.",
            category: BuildingCategory::Security,
            style: ArchitectureStyle::Brutalist,
            width: 5, height: 5,
            construction_cost: 25_000_000.0,
            construction_time_days: 600,
            max_occupancy: 2000,
            effects: BuildingEffects {
                tax_revenue_annual: 3_000_000.0,  // Subsidios por preso
                operating_cost_annual: 8_000_000.0,
                jobs_created: 400,
                land_value_multiplier: 0.1,
                happiness_effect: -0.1,
                crime_effect: -0.05,  // Teóricamente reduce crimen
                water_consumption: 30.0,
                electricity_consumption: 12.0,
                waste_generation: 5.0,
                traffic_generation: 8.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 25,
            requires_nearby: None,
        });

        // 27: Casino
        templates.push(BuildingTemplate {
            id: 27,
            name: "Casino Terrestre Masivo",
            description: "Aspirador de riqueza. NPCs pobres en loops de pathfinding adictivos. Suicidios y quiebras a largo plazo.",
            category: BuildingCategory::Entertainment,
            style: ArchitectureStyle::GlassTower,
            width: 4, height: 5,
            construction_cost: 30_000_000.0,
            construction_time_days: 300,
            max_occupancy: 3000,
            effects: BuildingEffects {
                tax_revenue_annual: 15_000_000.0,  // Altos impuestos al juego
                operating_cost_annual: 3_000_000.0,
                jobs_created: 500,
                land_value_multiplier: 1.1,
                happiness_effect: 0.05,  // Placer inmediato
                crime_effect: 0.2,       // Atrae crimen
                health_effect: -0.05,
                electricity_consumption: 15.0,
                water_consumption: 10.0,
                light_pollution: 0.8,
                traffic_generation: 20.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 5,
            requires_nearby: Some("Commercial"),
        });

        // 28: Planta Desalinizadora
        templates.push(BuildingTemplate {
            id: 28,
            name: "Planta de Desalinización",
            description: "Chupa agua del mar. Consume electricidad monstruosa. Salmuera hiperconcentrada mata pesca si no se trata.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::Industrial,
            width: 3, height: 3,
            construction_cost: 18_000_000.0,
            construction_time_days: 400,
            max_occupancy: 100,
            effects: BuildingEffects {
                tax_revenue_annual: 200_000.0,
                operating_cost_annual: 3_000_000.0,
                jobs_created: 100,
                land_value_multiplier: 0.5,
                electricity_consumption: 60.0,
                water_consumption: -100.0,  // PRODUCE agua
                water_pollution: 0.3,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 10,
            requires_nearby: Some("Water"),
        });

        // 29: Data Center
        templates.push(BuildingTemplate {
            id: 29,
            name: "Data Center Masivo",
            description: "Servidores 24/7. Reduce latencia de toda la ciudad. Consume agua para refrigeración como un barrio entero.",
            category: BuildingCategory::Technology,
            style: ArchitectureStyle::HighTech,
            width: 3, height: 3,
            construction_cost: 20_000_000.0,
            construction_time_days: 250,
            max_occupancy: 50,
            effects: BuildingEffects {
                tax_revenue_annual: 2_000_000.0,
                operating_cost_annual: 5_000_000.0,
                jobs_created: 50,
                land_value_multiplier: 0.8,
                fiber_traffic: 100.0,
                electricity_consumption: 40.0,
                water_consumption: 50.0,  // Refrigeración
                noise_pollution: 0.3,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 3,
            requires_nearby: Some("Technology"),
        });

        // 30: Torre de Agua
        templates.push(BuildingTemplate {
            id: 30,
            name: "Torre de Agua Municipal",
            description: "Provee presión de agua por gravedad. Sin electricidad funciona. Ícono urbano subestimado.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::Industrial,
            width: 1, height: 1,
            construction_cost: 500_000.0,
            construction_time_days: 60,
            max_occupancy: 0,
            effects: BuildingEffects {
                operating_cost_annual: 30_000.0,
                jobs_created: 2,
                land_value_multiplier: 0.95,
                water_consumption: 0.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 2,
            requires_nearby: Some("Residential"),
        });

        // 31: Cementerio
        templates.push(BuildingTemplate {
            id: 31,
            name: "Cementerio Municipal",
            description: "Consume terreno permanentemente. Químicos de embalsamamiento envenenan napas. Demolerlo causa la peor protesta.",
            category: BuildingCategory::Government,
            style: ArchitectureStyle::Neoclassical,
            width: 4, height: 4,
            construction_cost: 1_000_000.0,
            construction_time_days: 90,
            max_occupancy: 0,
            effects: BuildingEffects {
                operating_cost_annual: 100_000.0,
                jobs_created: 10,
                land_value_multiplier: 0.8,
                happiness_effect: 0.02,
                soil_pollution: 0.1,
                water_pollution: 0.05,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 8,
            requires_nearby: None,
        });

        // 32: Estadio Deportivo
        templates.push(BuildingTemplate {
            id: 32,
            name: "Estadio Deportivo Mayor",
            description: "50,000 NPCs en un solo nodo. Domingo de partido: colapso vial total. Entre semana: elefante blanco de cemento.",
            category: BuildingCategory::Sports,
            style: ArchitectureStyle::Brutalist,
            width: 6, height: 5,
            construction_cost: 50_000_000.0,
            construction_time_days: 500,
            max_occupancy: 50000,
            effects: BuildingEffects {
                tax_revenue_annual: 4_000_000.0,
                operating_cost_annual: 6_000_000.0,
                jobs_created: 300,
                land_value_multiplier: 0.9,
                happiness_effect: 0.1,
                traffic_generation: 100.0,  // Día de partido
                pedestrian_traffic: 200.0,
                electricity_consumption: 20.0,
                water_consumption: 15.0,
                waste_generation: 10.0,
                noise_pollution: 0.5,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 10,
            requires_nearby: Some("Transport"),
        });

        // 33: Sede Sindical
        templates.push(BuildingTemplate {
            id: 33,
            name: "Sede Central de Sindicato",
            description: "Nido de problemas logísticos. Si los ofendes: paro total. Basura sin recoger, comida pudriéndose.",
            category: BuildingCategory::Government,
            style: ArchitectureStyle::Brutalist,
            width: 2, height: 2,
            construction_cost: 1_000_000.0,
            construction_time_days: 100,
            max_occupancy: 100,
            effects: BuildingEffects {
                operating_cost_annual: 200_000.0,
                jobs_created: 100,
                land_value_multiplier: 0.95,
                happiness_effect: 0.05,  // Para los sindicalizados
                traffic_generation: 2.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 2,
            requires_nearby: Some("Industrial"),
        });

        // 34: Banco Central / Bolsa de Valores
        templates.push(BuildingTemplate {
            id: 34,
            name: "Bolsa de Valores Local",
            description: "Multiplica riqueza de clase alta. Blanco #1 de ataques. Requiere anillo de seguridad que mata tráfico peatonal.",
            category: BuildingCategory::Finance,
            style: ArchitectureStyle::GlassTower,
            width: 3, height: 6,
            construction_cost: 40_000_000.0,
            construction_time_days: 400,
            max_occupancy: 800,
            effects: BuildingEffects {
                tax_revenue_annual: 8_000_000.0,
                operating_cost_annual: 4_000_000.0,
                jobs_created: 800,
                land_value_multiplier: 2.5,
                gentrification_speed: 0.2,
                happiness_effect: -0.05,
                traffic_generation: 15.0,
                electricity_consumption: 10.0,
                fiber_traffic: 30.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 3,
            requires_nearby: Some("Financial"),
        });

        // 35: Subestación Eléctrica
        templates.push(BuildingTemplate {
            id: 35,
            name: "Subestación de Alta Tensión",
            description: "Reduce voltaje para uso residencial. Si se sobrecarga en verano: explosión. Zumbido constante devalúa el suelo.",
            category: BuildingCategory::Energy,
            style: ArchitectureStyle::Industrial,
            width: 2, height: 2,
            construction_cost: 2_500_000.0,
            construction_time_days: 120,
            max_occupancy: 5,
            effects: BuildingEffects {
                operating_cost_annual: 150_000.0,
                jobs_created: 5,
                land_value_multiplier: 0.6,
                noise_pollution: 0.3,
                electricity_consumption: 0.0,  // Solo transforma
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 8,
            requires_nearby: Some("Residential"),
        });

        // 36: Comisaría de Policía
        templates.push(BuildingTemplate {
            id: 36,
            name: "Comisaría Central",
            description: "Reduce crimen en radio. Si tiene bajo presupuesto, policías aceptan sobornos. La corrupción escala.",
            category: BuildingCategory::Security,
            style: ArchitectureStyle::Brutalist,
            width: 2, height: 3,
            construction_cost: 3_000_000.0,
            construction_time_days: 150,
            max_occupancy: 150,
            effects: BuildingEffects {
                operating_cost_annual: 2_500_000.0,
                jobs_created: 150,
                land_value_multiplier: 1.05,
                crime_effect: -0.15,
                happiness_effect: 0.05,
                traffic_generation: 5.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 37: Estación de Bomberos
        templates.push(BuildingTemplate {
            id: 37,
            name: "Cuartel de Bomberos",
            description: "Mitiga incendios. Si el tráfico los bloquea, el fuego se propaga. Necesitan prioridad en semáforos.",
            category: BuildingCategory::Security,
            style: ArchitectureStyle::Brutalist,
            width: 2, height: 2,
            construction_cost: 2_000_000.0,
            construction_time_days: 120,
            max_occupancy: 60,
            effects: BuildingEffects {
                operating_cost_annual: 1_500_000.0,
                jobs_created: 60,
                land_value_multiplier: 1.1,
                happiness_effect: 0.05,
                traffic_generation: 3.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 38: Puerto de Aguas Profundas
        templates.push(BuildingTemplate {
            id: 38,
            name: "Puerto de Contenedores",
            description: "Arteria principal de importación. Requiere dragado constante. Sin tren de carga: colapso vial por camiones.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::Industrial,
            width: 6, height: 6,
            construction_cost: 60_000_000.0,
            construction_time_days: 600,
            max_occupancy: 1000,
            effects: BuildingEffects {
                tax_revenue_annual: 12_000_000.0,
                operating_cost_annual: 5_000_000.0,
                jobs_created: 1000,
                land_value_multiplier: 0.3,
                water_pollution: 0.4,
                noise_pollution: 0.6,
                traffic_generation: 40.0,
                electricity_consumption: 15.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 12,
            requires_nearby: Some("Industrial"),
        });

        // 39: Aeropuerto
        templates.push(BuildingTemplate {
            id: 39,
            name: "Aeropuerto Internacional",
            description: "Conecta con el mundo. Ruido de turbinas destruye valor residencial en cono de aproximación. Atrae turismo masivo.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::HighTech,
            width: 8, height: 6,
            construction_cost: 100_000_000.0,
            construction_time_days: 800,
            max_occupancy: 5000,
            effects: BuildingEffects {
                tax_revenue_annual: 15_000_000.0,
                operating_cost_annual: 10_000_000.0,
                jobs_created: 3000,
                land_value_multiplier: 0.2,  // Cerca es inhabitable
                noise_pollution: 1.0,        // Máximo ruido
                air_pollution: 0.3,
                traffic_generation: 50.0,
                electricity_consumption: 30.0,
                water_consumption: 20.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 20,
            requires_nearby: None,
        });

        // 40: Universidad
        templates.push(BuildingTemplate {
            id: 40,
            name: "Campus Universitario",
            description: "Genera investigación y cultura. Si no hay empleos tech, fuga de cerebros. Primeros en protestar si subís el bus.",
            category: BuildingCategory::Education,
            style: ArchitectureStyle::Neoclassical,
            width: 5, height: 4,
            construction_cost: 25_000_000.0,
            construction_time_days: 500,
            max_occupancy: 5000,
            effects: BuildingEffects {
                tax_revenue_annual: 500_000.0,
                operating_cost_annual: 8_000_000.0,
                jobs_created: 800,
                land_value_multiplier: 1.2,
                education_effect: 0.3,
                happiness_effect: 0.05,
                traffic_generation: 10.0,
                pedestrian_traffic: 30.0,
                electricity_consumption: 8.0,
                water_consumption: 15.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 41: Planta de Tratamiento de Agua
        templates.push(BuildingTemplate {
            id: 41,
            name: "Planta Potabilizadora",
            description: "Filtra y purifica agua del río. Si el río está contaminado por industria: filtros colapsan. Costos se disparan.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::Industrial,
            width: 3, height: 3,
            construction_cost: 8_000_000.0,
            construction_time_days: 300,
            max_occupancy: 80,
            effects: BuildingEffects {
                operating_cost_annual: 1_500_000.0,
                jobs_created: 80,
                land_value_multiplier: 0.6,
                water_consumption: -200.0,  // PRODUCE agua potable
                electricity_consumption: 10.0,
                waste_generation: 2.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 8,
            requires_nearby: Some("Water"),
        });

        // 42: Depuradora de Aguas Residuales
        templates.push(BuildingTemplate {
            id: 42,
            name: "Planta Depuradora de Aguas Negras",
            description: "Limpia aguas cloacales. Sin esto, el río se vuelve alcantarilla. Consume químicos y electricidad a niveles industriales.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::Industrial,
            width: 4, height: 4,
            construction_cost: 12_000_000.0,
            construction_time_days: 350,
            max_occupancy: 120,
            effects: BuildingEffects {
                operating_cost_annual: 2_500_000.0,
                jobs_created: 120,
                land_value_multiplier: 0.3,
                water_pollution: -0.3,  // REDUCE contaminación
                electricity_consumption: 15.0,
                waste_generation: 5.0,  // Lodos
                happiness_effect: -0.02,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            requires_nearby: Some("Water"),
        });

        // =========================================================================
        // =========================================================================
        // 5. TRANSPORTE, MOVILIDAD Y CAOS VIAL (43-60)
        // =========================================================================

        // 43: Plataforma Hyperloop Subterránea
        templates.push(BuildingTemplate {
            id: 43,

            name: "Hyperloop Subterráneo",
            description: "Cápsulas al vacío a 1000km/h. Si un sismo microfractura el tubo, la implosión mata 500 NPCs en un frame.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::HighTech,
            width: 3, height: 2,
            construction_cost: 150_000_000.0,
            construction_time_days: 900,
            max_occupancy: 2000,
            effects: BuildingEffects {
                operating_cost_annual: 8_000_000.0,
                jobs_created: 300,
                land_value_multiplier: 2.5,
                happiness_effect: 0.1,
                electricity_consumption: 80.0,
                traffic_generation: -50.0,
                noise_pollution: 0.1,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 0,
            requires_nearby: Some("Transport"),
        });

        // 44: Depósito de Patinetes Eléctricos
        templates.push(BuildingTemplate {
            id: 44,
            name: "Depósito de Patinetes (Scooter Graveyard)",
            description: "Cientos de patinetes bloquean veredas. Baterías de litio tiradas al río matan el ecosistema.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::Industrial,
            width: 1, height: 1,
            construction_cost: 50_000.0,
            construction_time_days: 10,
            max_occupancy: 5,
            effects: BuildingEffects {
                operating_cost_annual: 30_000.0,
                jobs_created: 5,
                land_value_multiplier: 0.85,
                happiness_effect: -0.02,
                crime_effect: 0.01,
                water_pollution: 0.05,
                traffic_generation: -1.0,
                pedestrian_traffic: -0.1,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: false,
            nimby_radius: 2,
            requires_nearby: None,
        });

        // 45: Puerto Logístico de Drones de Carga
        templates.push(BuildingTemplate {
            id: 45,
            name: "Puerto de Drones de Carga Pesada",
            description: "Drones tamaño auto llevando contenedores. Si falla batería, 2 toneladas de acero sobre techos residenciales.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::HighTech,
            width: 3, height: 3,
            construction_cost: 18_000_000.0,
            construction_time_days: 250,
            max_occupancy: 50,
            effects: BuildingEffects {
                operating_cost_annual: 5_000_000.0,
                jobs_created: 50,
                land_value_multiplier: 0.7,
                happiness_effect: -0.03,
                electricity_consumption: 25.0,
                noise_pollution: 0.5,
                traffic_generation: -15.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 6,
            requires_nearby: Some("Industrial"),
        });

        // 46: Túnel de Peaje Dinámico
        templates.push(BuildingTemplate {
            id: 46,
            name: "Túnel de Peaje Dinámico por Congestión",
            description: "Precio varía según tráfico. Si script falla y sube a $500, pánico en barrera genera atasco masivo.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::HighTech,
            width: 1, height: 2,
            construction_cost: 10_000_000.0,
            construction_time_days: 200,
            max_occupancy: 10,
            effects: BuildingEffects {
                tax_revenue_annual: 3_000_000.0,
                operating_cost_annual: 500_000.0,
                jobs_created: 10,
                land_value_multiplier: 0.9,
                happiness_effect: -0.05,
                traffic_generation: -5.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Transport"),
        });

        // 47: Centro de Gestión de Vehículos Autónomos
        templates.push(BuildingTemplate {
            id: 47,
            name: "Centro de Gestión de Vehículos Autónomos",
            description: "Torres Lidar/Radar coordinan autos sin conductor. Rascacielos espejados confunden sensores: carnicería.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::Cyberpunk,
            width: 2, height: 4,
            construction_cost: 12_000_000.0,
            construction_time_days: 180,
            max_occupancy: 150,
            effects: BuildingEffects {
                operating_cost_annual: 3_000_000.0,
                jobs_created: 150,
                land_value_multiplier: 1.1,
                happiness_effect: 0.02,
                crime_effect: -0.03,
                electricity_consumption: 30.0,
                fiber_traffic: 10.0,
                traffic_generation: -10.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 3,
            requires_nearby: Some("Transport"),
        });

        // 48: Estación Repostadora de Hidrógeno
        templates.push(BuildingTemplate {
            id: 48,
            name: "Estación de Hidrógeno Líquido (700 bar)",
            description: "Hidrógeno a 700 bares. Fuga arde con llama invisible a la luz del día. Bomberos caminan directo al fuego.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::HighTech,
            width: 2, height: 2,
            construction_cost: 5_000_000.0,
            construction_time_days: 120,
            max_occupancy: 15,
            effects: BuildingEffects {
                operating_cost_annual: 1_200_000.0,
                jobs_created: 15,
                land_value_multiplier: 0.5,
                happiness_effect: -0.02,
                electricity_consumption: 40.0,
                air_pollution: -0.1,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 10,
            requires_nearby: Some("Transport"),
        });

        // 49: Terminal de Hovercrafts
        templates.push(BuildingTemplate {
            id: 49,
            name: "Terminal de Hovercrafts de Alta Velocidad",
            description: "Transportan pasajeros por agua. Ráfagas de viento empujan peatones al agua y espantan peces.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::HighTech,
            width: 3, height: 2,
            construction_cost: 8_000_000.0,
            construction_time_days: 200,
            max_occupancy: 300,
            effects: BuildingEffects {
                tax_revenue_annual: 800_000.0,
                operating_cost_annual: 2_000_000.0,
                jobs_created: 80,
                land_value_multiplier: 1.2,
                happiness_effect: 0.03,
                noise_pollution: 0.7,
                water_pollution: 0.1,
                traffic_generation: 8.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 5,
            requires_nearby: Some("Water"),
        });

        // 50: Puente Levadizo Ferroviario
        templates.push(BuildingTemplate {
            id: 50,
            name: "Puente Levadizo Ferroviario",
            description: "Trenes arriba, barcos abajo. Mala sincronización: tren descarrila o buque choca estructura.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::Industrial,
            width: 1, height: 3,
            construction_cost: 7_000_000.0,
            construction_time_days: 250,
            max_occupancy: 10,
            effects: BuildingEffects {
                operating_cost_annual: 400_000.0,
                jobs_created: 10,
                land_value_multiplier: 0.8,
                happiness_effect: -0.01,
                traffic_generation: 20.0,
                noise_pollution: 0.3,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 3,
            requires_nearby: Some("Water"),
        });

        // =========================================================================
        // 6. HIDRÁULICA, AGUA Y DESASTRES DE FLUIDOS (51-70)
        // =========================================================================

        // 51: Reservorio de Tormenta Subterráneo
        templates.push(BuildingTemplate {
            id: 51,
            name: "Reservorio de Tormenta (Catacumbas)",
            description: "Cavernas gigantes bajo la ciudad. Si el moho debilita pilares, sinkhole de 40m traga la plaza de arriba.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::Underground,
            width: 4, height: 4,
            construction_cost: 25_000_000.0,
            construction_time_days: 500,
            max_occupancy: 10,
            effects: BuildingEffects {
                operating_cost_annual: 300_000.0,
                jobs_created: 10,
                land_value_multiplier: 1.05,
                water_consumption: -500.0,
                soil_pollution: -0.1,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: false,
            nimby_radius: 0,
            requires_nearby: Some("Water"),
        });

        // 52: Planta de Tratamiento de Aguas Ácidas
        templates.push(BuildingTemplate {
            id: 52,
            name: "Planta de Aguas Ácidas de Mina",
            description: "Neutraliza agua pH 2 con cal viva. Sin filtros de aire, trabajadores mueren de silicosis.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::Industrial,
            width: 3, height: 3,
            construction_cost: 6_000_000.0,
            construction_time_days: 180,
            max_occupancy: 80,
            effects: BuildingEffects {
                operating_cost_annual: 2_000_000.0,
                jobs_created: 80,
                land_value_multiplier: 0.4,
                health_effect: -0.05,
                water_pollution: -0.2,
                air_pollution: 0.1,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 10,
            requires_nearby: Some("Mining"),
        });

        // 53: Estación de Bombeo de Lodo
        templates.push(BuildingTemplate {
            id: 53,
            name: "Estación de Bombeo de Lodo a Alta Presión",
            description: "Mueve tierra+agua por tuberías. Si revienta, inunda barrio con barro denso imposible de drenar.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::Industrial,
            width: 2, height: 2,
            construction_cost: 3_000_000.0,
            construction_time_days: 150,
            max_occupancy: 20,
            effects: BuildingEffects {
                operating_cost_annual: 800_000.0,
                jobs_created: 20,
                land_value_multiplier: 0.2,
                electricity_consumption: 20.0,
                noise_pollution: 0.4,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 8,
            requires_nearby: Some("Water"),
        });

        // 54: Canal de Alivio de Desbordamiento Cloacal (CSO)
        templates.push(BuildingTemplate {
            id: 54,
            name: "Canal de Alivio Cloacal (CSO)",
            description: "Cuando llueve mucho, tira aguas negras directamente al río. Salvas calles pero matas playas.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::Industrial,
            width: 1, height: 2,
            construction_cost: 1_500_000.0,
            construction_time_days: 90,
            max_occupancy: 5,
            effects: BuildingEffects {
                operating_cost_annual: 50_000.0,
                jobs_created: 5,
                land_value_multiplier: 0.3,
                water_pollution: 0.5,
                health_effect: -0.1,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: false,
            nimby_radius: 15,
            requires_nearby: Some("Water"),
        });

        // 55: Planta de Inyección de Residuos Líquidos Profundos
        templates.push(BuildingTemplate {
            id: 55,
            name: "Inyección de Residuos Profundos",
            description: "Bombea químicos a 5km bajo tierra. Presión hidrostática causa terremotos inducidos. Tiembla.",
            category: BuildingCategory::Waste,
            style: ArchitectureStyle::Industrial,
            width: 2, height: 2,
            construction_cost: 10_000_000.0,
            construction_time_days: 300,
            max_occupancy: 30,
            effects: BuildingEffects {
                operating_cost_annual: 3_000_000.0,
                jobs_created: 30,
                land_value_multiplier: 0.1,
                soil_pollution: -0.2,
                water_pollution: -0.1,
                radiation_emission: 0.01,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 20,
            requires_nearby: Some("Industrial"),
        });

        // 56: Colector Central de Basura Neumática
        templates.push(BuildingTemplate {
            id: 56,
            name: "Colector de Basura Neumática",
            description: "Tuberías aspiran bolsas a 80km/h. Chatarra inflamable + fricción = lanzallamas subterráneo.",
            category: BuildingCategory::Waste,
            style: ArchitectureStyle::HighTech,
            width: 3, height: 2,
            construction_cost: 9_000_000.0,
            construction_time_days: 250,
            max_occupancy: 40,
            effects: BuildingEffects {
                operating_cost_annual: 1_500_000.0,
                jobs_created: 40,
                land_value_multiplier: 1.0,
                electricity_consumption: 35.0,
                waste_generation: -10.0,
                traffic_generation: -5.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 3,
            requires_nearby: None,
        });

        // 57: Planta Desalinizadora por Electrodiálisis
        templates.push(BuildingTemplate {
            id: 57,
            name: "Desalinizadora por Electrodiálisis",
            description: "Membranas carísimas y campos eléctricos. Derrame de petróleo tapona membranas y ciudad se queda sin agua.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::HighTech,
            width: 3, height: 3,
            construction_cost: 15_000_000.0,
            construction_time_days: 350,
            max_occupancy: 60,
            effects: BuildingEffects {
                operating_cost_annual: 4_000_000.0,
                jobs_created: 60,
                land_value_multiplier: 0.6,
                electricity_consumption: 50.0,
                water_consumption: -300.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 5,
            requires_nearby: Some("Water"),
        });

        // 58: Acueducto de Hormigón Elevado
        templates.push(BuildingTemplate {
            id: 58,
            name: "Acueducto Elevado (Estilo Romano)",
            description: "Atraviesa la ciudad por gravedad. Hermoso pero frágil: choque de tránsito derrumba pilares e inunda autopista.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::Neoclassical,
            width: 1, height: 8,
            construction_cost: 5_000_000.0,
            construction_time_days: 300,
            max_occupancy: 5,
            effects: BuildingEffects {
                operating_cost_annual: 100_000.0,
                jobs_created: 5,
                land_value_multiplier: 1.2,
                happiness_effect: 0.05,
                water_consumption: -150.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: false,
            nimby_radius: 0,
            requires_nearby: Some("Water"),
        });

        // 59: Dique Seco de Compuertas
        templates.push(BuildingTemplate {
            id: 59,
            name: "Dique Seco de Mantenimiento de Compuertas",
            description: "Repara exclusas de canales. Cierra el canal durante obras. Cuello de botella económico ineludible.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::Industrial,
            width: 4, height: 2,
            construction_cost: 4_000_000.0,
            construction_time_days: 200,
            max_occupancy: 100,
            effects: BuildingEffects {
                operating_cost_annual: 1_200_000.0,
                jobs_created: 100,
                land_value_multiplier: 0.8,
                noise_pollution: 0.3,
                traffic_generation: 5.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 4,
            requires_nearby: Some("Water"),
        });

        // 60: Planta de Floculación Químico-Bacteriológica
        templates.push(BuildingTemplate {
            id: 60,
            name: "Planta de Floculación Bacteriológica",
            description: "Bacterias comen metales pesados del agua. Si temperatura baja, mueren de frío e inyectan plomo en canillas escolares.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::HighTech,
            width: 3, height: 2,
            construction_cost: 7_000_000.0,
            construction_time_days: 220,
            max_occupancy: 45,
            effects: BuildingEffects {
                operating_cost_annual: 1_800_000.0,
                jobs_created: 45,
                land_value_multiplier: 0.6,
                water_pollution: -0.4,
                health_effect: 0.1,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 6,
            requires_nearby: Some("Water"),
        });

        // =========================================================================
        // 7. MERCADO NEGRO TECNOLÓGICO Y SUBSUELO (61-75)
        // =========================================================================

        // 61: Granja de Clics Subterránea
        templates.push(BuildingTemplate {
            id: 61,
            name: "Click Farm Subterránea",
            description: "Miles de celulares viejos generando likes falsos. Baterías sobrecalentadas queman edificio residencial de arriba.",
            category: BuildingCategory::BlackMarket,
            style: ArchitectureStyle::Underground,
            width: 1, height: 1,
            construction_cost: 200_000.0,
            construction_time_days: 30,
            max_occupancy: 20,
            effects: BuildingEffects {
                tax_revenue_annual: 500_000.0,
                operating_cost_annual: 50_000.0,
                jobs_created: 20,
                land_value_multiplier: 0.5,
                electricity_consumption: 15.0,
                crime_effect: 0.05,
                fiber_traffic: 5.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 3,
            requires_nearby: None,
        });

        // 62: Laboratorio de Carne Sintética
        templates.push(BuildingTemplate {
            id: 62,
            name: "Laboratorio de Carne Sintética (Vat-Grown)",
            description: "Cultivan músculo de vaca en tanques. Si el suero se contamina con una espora, pudres 50 toneladas.",
            category: BuildingCategory::Industrial,
            style: ArchitectureStyle::HighTech,
            width: 2, height: 3,
            construction_cost: 8_000_000.0,
            construction_time_days: 200,
            max_occupancy: 120,
            effects: BuildingEffects {
                tax_revenue_annual: 3_000_000.0,
                operating_cost_annual: 2_000_000.0,
                jobs_created: 120,
                land_value_multiplier: 1.1,
                electricity_consumption: 25.0,
                water_consumption: 40.0,
                waste_generation: 2.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Industrial"),
        });

        // 63: Taller de Hackeo de Vehículos Autónomos
        templates.push(BuildingTemplate {
            id: 63,
            name: "Taller de Hackeo de Vehículos",
            description: "Reprograman firmware para quitar límite velocidad. Autos a 180km/h por zonas escolares. Invisibles a multas.",
            category: BuildingCategory::BlackMarket,
            style: ArchitectureStyle::Cyberpunk,
            width: 1, height: 1,
            construction_cost: 150_000.0,
            construction_time_days: 20,
            max_occupancy: 10,
            effects: BuildingEffects {
                tax_revenue_annual: 0.0,
                operating_cost_annual: 30_000.0,
                jobs_created: 10,
                crime_effect: 0.15,
                happiness_effect: -0.02,
                fiber_traffic: 3.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 4,
            requires_nearby: None,
        });

        // 64: Almacén de Servidores Bulletproof
        templates.push(BuildingTemplate {
            id: 64,
            name: "Hosting Ilegal Bulletproof",
            description: "Alojan Dark Web. Inmunes a órdenes judiciales. Ejército los tumba creando zonas de guerra urbana.",
            category: BuildingCategory::BlackMarket,
            style: ArchitectureStyle::Underground,
            width: 1, height: 2,
            construction_cost: 500_000.0,
            construction_time_days: 60,
            max_occupancy: 15,
            effects: BuildingEffects {
                tax_revenue_annual: 2_000_000.0,
                operating_cost_annual: 300_000.0,
                jobs_created: 15,
                crime_effect: 0.2,
                electricity_consumption: 40.0,
                fiber_traffic: 20.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 8,
            requires_nearby: None,
        });

        // 65: Depósito de Quema de E-Waste Ilegal
        templates.push(BuildingTemplate {
            id: 65,
            name: "Quema de E-Waste Ilegal",
            description: "Funden placas base con ácido a cielo abierto. NPCs pierden 5 IQ/año. Destruye educación de la zona.",
            category: BuildingCategory::BlackMarket,
            style: ArchitectureStyle::Industrial,
            width: 2, height: 2,
            construction_cost: 100_000.0,
            construction_time_days: 15,
            max_occupancy: 30,
            effects: BuildingEffects {
                tax_revenue_annual: 0.0,
                operating_cost_annual: 10_000.0,
                jobs_created: 30,
                education_effect: -0.3,
                health_effect: -0.2,
                air_pollution: 0.5,
                soil_pollution: 0.4,
                water_pollution: 0.3,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: false,
            nimby_radius: 12,
            requires_nearby: None,
        });

        // 66: Mercado Negro de Órganos Impresos 3D
        templates.push(BuildingTemplate {
            id: 66,
            name: "Mercado Negro de Órganos 3D",
            description: "Riñones y corazones de bioplástico. 40% fallan en 2 años. NPCs caen muertos manejando ómnibus.",
            category: BuildingCategory::BlackMarket,
            style: ArchitectureStyle::Underground,
            width: 1, height: 2,
            construction_cost: 800_000.0,
            construction_time_days: 45,
            max_occupancy: 25,
            effects: BuildingEffects {
                tax_revenue_annual: 1_500_000.0,
                operating_cost_annual: 400_000.0,
                jobs_created: 25,
                health_effect: 0.05,
                crime_effect: 0.08,
                electricity_consumption: 20.0,
                water_consumption: 5.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 5,
            requires_nearby: None,
        });

        // 67: Refugio de Servidores IA Desconectados
        templates.push(BuildingTemplate {
            id: 67,
            name: "Refugio de IA Air-Gapped",
            description: "Bóveda de plomo sin internet. Mensajero debe llevar pendrive 5TB en moto. Si muere en choque, red colapsa.",
            category: BuildingCategory::Technology,
            style: ArchitectureStyle::Cyberpunk,
            width: 1, height: 1,
            construction_cost: 2_000_000.0,
            construction_time_days: 90,
            max_occupancy: 10,
            effects: BuildingEffects {
                operating_cost_annual: 300_000.0,
                jobs_created: 10,
                land_value_multiplier: 0.8,
                privacy_index: 1.0,
                electricity_consumption: 15.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: false,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 68: Torre de Interferencia GPS
        templates.push(BuildingTemplate {
            id: 68,
            name: "Torre Spoofer GPS",
            description: "Confunde camiones municipales. Pathfinding entra en loop infinito recalculando rutas, congelando CPU.",
            category: BuildingCategory::BlackMarket,
            style: ArchitectureStyle::Cyberpunk,
            width: 1, height: 3,
            construction_cost: 300_000.0,
            construction_time_days: 30,
            max_occupancy: 5,
            effects: BuildingEffects {
                tax_revenue_annual: 0.0,
                operating_cost_annual: 20_000.0,
                jobs_created: 5,
                crime_effect: 0.1,
                traffic_generation: 5.0,
                electricity_consumption: 5.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: false,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 69: Planta de Pirólisis de Neumáticos
        templates.push(BuildingTemplate {
            id: 69,
            name: "Planta de Pirólisis de Neumáticos",
            description: "Queman ruedas viejas sin oxígeno para sacar petróleo. Si enfriamiento falla, reactor se funde.",
            category: BuildingCategory::Waste,
            style: ArchitectureStyle::Industrial,
            width: 3, height: 3,
            construction_cost: 6_000_000.0,
            construction_time_days: 250,
            max_occupancy: 50,
            effects: BuildingEffects {
                tax_revenue_annual: 800_000.0,
                operating_cost_annual: 1_500_000.0,
                jobs_created: 50,
                land_value_multiplier: 0.4,
                air_pollution: 0.15,
                waste_generation: -5.0,
                electricity_consumption: 10.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 10,
            requires_nearby: Some("Industrial"),
        });

        // =========================================================================
        // 8. CONTROL GUBERNAMENTAL Y MISERIA PURA (70-85)
        // =========================================================================

        // 70: Centro de Reeducación para Endeudados
        templates.push(BuildingTemplate {
            id: 70,
            name: "Centro de Reeducación para Endeudados",
            description: "Workhouses modernas. NPCs quiebran y ensamblan cajas hasta pagar. Desaparecen del mercado de consumo.",
            category: BuildingCategory::Government,
            style: ArchitectureStyle::Brutalist,
            width: 3, height: 4,
            construction_cost: 2_000_000.0,
            construction_time_days: 180,
            max_occupancy: 500,
            effects: BuildingEffects {
                operating_cost_annual: 200_000.0,
                jobs_created: 50,
                land_value_multiplier: 0.3,
                happiness_effect: -0.15,
                crime_effect: -0.05,
                water_consumption: 10.0,
                electricity_consumption: 5.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 10,
            requires_nearby: Some("Government"),
        });

        // 71: Sede de Racionamiento de Alimentos
        templates.push(BuildingTemplate {
            id: 71,
            name: "Sede de Racionamiento de Alimentos",
            description: "Pastas de proteína sintética. Filas peatonales enormes rompen ECS de veredas. Comida deprimente fomenta motines.",
            category: BuildingCategory::Government,
            style: ArchitectureStyle::SovietBlock,
            width: 2, height: 2,
            construction_cost: 500_000.0,
            construction_time_days: 60,
            max_occupancy: 50,
            effects: BuildingEffects {
                operating_cost_annual: 1_000_000.0,
                jobs_created: 50,
                land_value_multiplier: 0.2,
                happiness_effect: -0.2,
                health_effect: -0.05,
                pedestrian_traffic: 10.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 8,
            requires_nearby: None,
        });

        // 72: Búnker de Comando del Gobierno Central
        templates.push(BuildingTemplate {
            id: 72,
            name: "Búnker de Comando (Doomsday)",
            description: "Gigante y blindado. En desastre, ministros se esconden y desconectan. Juegas a ciegas sin stats.",
            category: BuildingCategory::Government,
            style: ArchitectureStyle::Underground,
            width: 5, height: 5,
            construction_cost: 50_000_000.0,
            construction_time_days: 600,
            max_occupancy: 200,
            effects: BuildingEffects {
                operating_cost_annual: 5_000_000.0,
                jobs_created: 200,
                land_value_multiplier: -0.2,
                electricity_consumption: 60.0,
                water_consumption: 30.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 25,
            requires_nearby: Some("Government"),
        });

        // 73: Hospital Móvil sobre Rieles (Tren Hospital)
        templates.push(BuildingTemplate {
            id: 73,
            name: "Tren Hospital Móvil",
            description: "Necesario en desastres lejanos. Si rieles están oxidados, descarrila con 200 cirujanos a bordo.",
            category: BuildingCategory::Healthcare,
            style: ArchitectureStyle::HighTech,
            width: 1, height: 8,
            construction_cost: 10_000_000.0,
            construction_time_days: 300,
            max_occupancy: 200,
            effects: BuildingEffects {
                operating_cost_annual: 3_000_000.0,
                jobs_created: 200,
                land_value_multiplier: 1.0,
                health_effect: 0.15,
                traffic_generation: 2.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Transport"),
        });

        // 74: Campamento de Aislamiento Epidemiológico
        templates.push(BuildingTemplate {
            id: 74,
            name: "Campamento de Aislamiento (Leper Colony)",
            description: "Confina enfermos sin salidas viales. Borras agentes de la simulación por cuarentena letal.",
            category: BuildingCategory::Healthcare,
            style: ArchitectureStyle::Brutalist,
            width: 5, height: 5,
            construction_cost: 3_000_000.0,
            construction_time_days: 100,
            max_occupancy: 2000,
            effects: BuildingEffects {
                operating_cost_annual: 800_000.0,
                jobs_created: 100,
                land_value_multiplier: 0.0,
                happiness_effect: -0.3,
                health_effect: -0.1,
                water_consumption: 50.0,
                electricity_consumption: 25.0,
                waste_generation: 10.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 30,
            requires_nearby: None,
        });

        // 75: Instalación de Nieve Artificial
        templates.push(BuildingTemplate {
            id: 75,
            name: "Instalación de Nieve Artificial",
            description: "Hoteles de esquí exigen esto. Aspira agua dulce regional. Nieve derretida causa avalanchas de barro.",
            category: BuildingCategory::Entertainment,
            style: ArchitectureStyle::HighTech,
            width: 3, height: 2,
            construction_cost: 5_000_000.0,
            construction_time_days: 150,
            max_occupancy: 30,
            effects: BuildingEffects {
                tax_revenue_annual: 500_000.0,
                operating_cost_annual: 2_000_000.0,
                jobs_created: 30,
                land_value_multiplier: 1.3,
                water_consumption: 80.0,
                electricity_consumption: 30.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 3,
            requires_nearby: Some("Entertainment"),
        });

        // =========================================================================
        // 9. ARCOLOGÍAS, MEGA-ESTRUCTURAS Y AISLAMIENTO (76-90)
        // =========================================================================

        // 76: Arcología Residencial Autónoma
        templates.push(BuildingTemplate {
            id: 76,
            name: "Arcología Residencial Autónoma",
            description: "Ciudad en pirámide de cristal. Si HVAC falla, gripe muta y aniquila 50,000 NPCs sellados en 48h.",
            category: BuildingCategory::Residential,
            style: ArchitectureStyle::EcoFriendly,
            width: 6, height: 8,
            construction_cost: 200_000_000.0,
            construction_time_days: 1200,
            max_occupancy: 50000,
            effects: BuildingEffects {
                tax_revenue_annual: 50_000_000.0,
                operating_cost_annual: 25_000_000.0,
                jobs_created: 5000,
                land_value_multiplier: 5.0,
                happiness_effect: 0.15,
                electricity_consumption: 150.0,
                water_consumption: 200.0,
                waste_generation: 40.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 77: Elevador Espacial (Estación Ancla)
        templates.push(BuildingTemplate {
            id: 77,
            name: "Elevador Espacial (Estación Ancla)",
            description: "Exportas bienes al espacio a costo cero. Si cable de nanotubos se corta, látigo cósmico parte ciudad en dos.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::HighTech,
            width: 8, height: 20,
            construction_cost: 500_000_000.0,
            construction_time_days: 2000,
            max_occupancy: 3000,
            effects: BuildingEffects {
                tax_revenue_annual: 200_000_000.0,
                operating_cost_annual: 50_000_000.0,
                jobs_created: 3000,
                land_value_multiplier: 10.0,
                gentrification_speed: 0.3,
                electricity_consumption: 500.0,
                fiber_traffic: 50.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Transport"),
        });

        // 78: Cárcel de Realidad Virtual
        templates.push(BuildingTemplate {
            id: 78,
            name: "Cárcel de Realidad Virtual (Pods)",
            description: "Prisioneros en coma inducido. Si red parpadea, fríes cerebro de 10,000 reclusos. Juicios vacían tesoro.",
            category: BuildingCategory::Security,
            style: ArchitectureStyle::Cyberpunk,
            width: 4, height: 4,
            construction_cost: 20_000_000.0,
            construction_time_days: 400,
            max_occupancy: 10000,
            effects: BuildingEffects {
                operating_cost_annual: 1_000_000.0,
                jobs_created: 100,
                land_value_multiplier: 0.5,
                happiness_effect: -0.05,
                electricity_consumption: 80.0,
                fiber_traffic: 30.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 8,
            requires_nearby: None,
        });

        // 79: Reserva Natural Intocable Estricta
        templates.push(BuildingTemplate {
            id: 79,
            name: "Reserva Natural Intocable",
            description: "Bosque protegido por ONU. Sin tuberías ni cables. Pumas entran a barrios sin vallas de alto voltaje.",
            category: BuildingCategory::Entertainment,
            style: ArchitectureStyle::EcoFriendly,
            width: 12, height: 12,
            construction_cost: 1_000_000.0,
            construction_time_days: 30,
            max_occupancy: 1000,
            effects: BuildingEffects {
                tax_revenue_annual: 0.0,
                operating_cost_annual: 200_000.0,
                jobs_created: 50,
                land_value_multiplier: 2.0,
                happiness_effect: 0.1,
                air_pollution: -0.3,
                water_pollution: -0.1,
                crime_effect: 0.02,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 80: Autopista Subacuática de Cristal
        templates.push(BuildingTemplate {
            id: 80,
            name: "Autopista Subacuática de Cristal",
            description: "Capricho estético. Choque de camiones quiebra acrílico: físicas de fluidos inundan túnel en segundos.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::GlassTower,
            width: 2, height: 12,
            construction_cost: 35_000_000.0,
            construction_time_days: 500,
            max_occupancy: 500,
            effects: BuildingEffects {
                tax_revenue_annual: 2_000_000.0,
                operating_cost_annual: 3_000_000.0,
                jobs_created: 60,
                land_value_multiplier: 1.5,
                happiness_effect: 0.05,
                electricity_consumption: 40.0,
                traffic_generation: 30.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Water"),
        });

        // =========================================================================
        // 10. BIOINGENIERÍA, PLAGAS Y CONTAMINACIÓN 2.0 (81-100)
        // =========================================================================

        // 81: Granja Vertical de Insectos
        templates.push(BuildingTemplate {
            id: 81,
            name: "Granja Vertical de Insectos Comestibles",
            description: "Rascacielos de grillos. Si contención falla, enjambre de langostas devora cultivos en 5km.",
            category: BuildingCategory::Agriculture,
            style: ArchitectureStyle::HighTech,
            width: 2, height: 6,
            construction_cost: 4_000_000.0,
            construction_time_days: 150,
            max_occupancy: 80,
            effects: BuildingEffects {
                tax_revenue_annual: 1_500_000.0,
                operating_cost_annual: 800_000.0,
                jobs_created: 80,
                land_value_multiplier: 0.7,
                electricity_consumption: 35.0,
                waste_generation: 1.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 6,
            requires_nearby: Some("Agriculture"),
        });

        // 82: Torre Aspiradora de Smog
        templates.push(BuildingTemplate {
            id: 82,
            name: "Torre Aspiradora de Smog",
            description: "Chupa aire negro y lo vuelve respirable. Produce bloques de carbón tóxico hiper-concentrado.",
            category: BuildingCategory::Industrial,
            style: ArchitectureStyle::HighTech,
            width: 2, height: 8,
            construction_cost: 12_000_000.0,
            construction_time_days: 300,
            max_occupancy: 30,
            effects: BuildingEffects {
                operating_cost_annual: 3_000_000.0,
                jobs_created: 30,
                land_value_multiplier: 0.8,
                air_pollution: -0.4,
                waste_generation: 2.0,
                electricity_consumption: 45.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 5,
            requires_nearby: Some("Industrial"),
        });

        // 83: Fábrica de Órganos de Grafeno
        templates.push(BuildingTemplate {
            id: 83,
            name: "Fábrica de Órganos Artificiales de Grafeno",
            description: "Nanotecnología que resuelve listas de espera. Desecha microplásticos que depuradora no filtra. Fallos neurológicos.",
            category: BuildingCategory::Healthcare,
            style: ArchitectureStyle::HighTech,
            width: 3, height: 4,
            construction_cost: 18_000_000.0,
            construction_time_days: 400,
            max_occupancy: 200,
            effects: BuildingEffects {
                tax_revenue_annual: 10_000_000.0,
                operating_cost_annual: 5_000_000.0,
                jobs_created: 200,
                land_value_multiplier: 1.5,
                health_effect: 0.2,
                water_pollution: 0.1,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Healthcare"),
        });

        // 84: Plataforma Oceánica de Algacultura
        templates.push(BuildingTemplate {
            id: 84,
            name: "Algacultura Oceánica (Biocombustibles)",
            description: "Algas modificadas para gasolina ecológica. Si escapan, Marea Roja ahoga playas en lodo tóxico.",
            category: BuildingCategory::Agriculture,
            style: ArchitectureStyle::HighTech,
            width: 4, height: 2,
            construction_cost: 7_000_000.0,
            construction_time_days: 200,
            max_occupancy: 40,
            effects: BuildingEffects {
                tax_revenue_annual: 2_500_000.0,
                operating_cost_annual: 1_000_000.0,
                jobs_created: 40,
                land_value_multiplier: 0.9,
                water_pollution: -0.05,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Water"),
        });

        // 85: Laboratorio de Des-Extinción
        templates.push(BuildingTemplate {
            id: 85,
            name: "Laboratorio de Des-Extinción",
            description: "Clonan mamuts para zoológico ultralujo. Si puerta se queda sin electricidad, bestias aplastan autos.",
            category: BuildingCategory::Technology,
            style: ArchitectureStyle::HighTech,
            width: 4, height: 4,
            construction_cost: 25_000_000.0,
            construction_time_days: 500,
            max_occupancy: 150,
            effects: BuildingEffects {
                tax_revenue_annual: 5_000_000.0,
                operating_cost_annual: 8_000_000.0,
                jobs_created: 150,
                land_value_multiplier: 1.2,
                happiness_effect: 0.05,
                electricity_consumption: 60.0,
                water_consumption: 25.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 3,
            requires_nearby: None,
        });

        // 86: Instalación de Fracking
        templates.push(BuildingTemplate {
            id: 86,
            name: "Instalación de Fracking (Gas de Esquisto)",
            description: "Inyecta agua a presión y químicos bajo roca. Agua de canillas se vuelve inflamable. Mini-terremotos diarios.",
            category: BuildingCategory::Energy,
            style: ArchitectureStyle::Industrial,
            width: 3, height: 2,
            construction_cost: 10_000_000.0,
            construction_time_days: 250,
            max_occupancy: 60,
            effects: BuildingEffects {
                tax_revenue_annual: 8_000_000.0,
                operating_cost_annual: 3_000_000.0,
                jobs_created: 60,
                land_value_multiplier: 0.3,
                water_pollution: 0.4,
                soil_pollution: 0.2,
                health_effect: -0.1,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 15,
            requires_nearby: Some("Energy"),
        });

        // 87: Planta Come-Bacterias de Plástico
        templates.push(BuildingTemplate {
            id: 87,
            name: "Reciclaje de Plásticos Come-Bacterias",
            description: "Bacterias disuelven plástico y lo vuelven energía. Si se fugan, comen aislamiento de cables y tuberías PVC.",
            category: BuildingCategory::Waste,
            style: ArchitectureStyle::HighTech,
            width: 2, height: 3,
            construction_cost: 5_000_000.0,
            construction_time_days: 200,
            max_occupancy: 35,
            effects: BuildingEffects {
                operating_cost_annual: 1_200_000.0,
                jobs_created: 35,
                waste_generation: -8.0,
                electricity_consumption: 12.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 5,
            requires_nearby: Some("Industrial"),
        });

        // 88: Planta de Extracción de Litio de Aguas Residuales
        templates.push(BuildingTemplate {
            id: 88,
            name: "Extracción de Litio de Aguas Residuales",
            description: "Cuela excrementos para raspar minerales raros. Aire cáustico oxida asfalto. Calles se vuelven polvo.",
            category: BuildingCategory::Industrial,
            style: ArchitectureStyle::Industrial,
            width: 3, height: 2,
            construction_cost: 7_000_000.0,
            construction_time_days: 250,
            max_occupancy: 40,
            effects: BuildingEffects {
                tax_revenue_annual: 4_000_000.0,
                operating_cost_annual: 2_000_000.0,
                jobs_created: 40,
                land_value_multiplier: 0.2,
                air_pollution: 0.1,
                water_pollution: -0.1,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 10,
            requires_nearby: Some("Water"),
        });

        // 89: Laboratorio de Virología Submarino BSL-5
        templates.push(BuildingTemplate {
            id: 89,
            name: "Laboratorio de Virología Submarino BSL-5",
            description: "Patógenos bajo el agua. Si derrame infecta fauna marina, llega al mercado de pescado: pandemia en 5min.",
            category: BuildingCategory::Healthcare,
            style: ArchitectureStyle::Underground,
            width: 3, height: 3,
            construction_cost: 30_000_000.0,
            construction_time_days: 500,
            max_occupancy: 80,
            effects: BuildingEffects {
                operating_cost_annual: 10_000_000.0,
                jobs_created: 80,
                land_value_multiplier: 0.1,
                health_effect: 0.05,
                electricity_consumption: 70.0,
                water_consumption: 40.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 25,
            requires_nearby: Some("Water"),
        });

        // 90: Centro de Clonación de Mascotas VIP
        templates.push(BuildingTemplate {
            id: 90,
            name: "Centro de Clonación de Mascotas VIP",
            description: "Ricos clonan Border Collies. Basura biomédica peligrosa. Ecologistas te declaran la guerra con bombas molotov.",
            category: BuildingCategory::Healthcare,
            style: ArchitectureStyle::GlassTower,
            width: 2, height: 3,
            construction_cost: 8_000_000.0,
            construction_time_days: 200,
            max_occupancy: 50,
            effects: BuildingEffects {
                tax_revenue_annual: 4_000_000.0,
                operating_cost_annual: 2_500_000.0,
                jobs_created: 50,
                land_value_multiplier: 1.1,
                waste_generation: 1.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 2,
            requires_nearby: Some("Healthcare"),
        });

        // =========================================================================
        // 11. CONTROL COGNITIVO, VIGILANCIA Y TIRANÍA DIGITAL (91-105)
        // =========================================================================

        // 91: Planta de Polvo Inteligente
        templates.push(BuildingTemplate {
            id: 91,
            name: "Planta de Smart Dust",
            description: "Micro-sensores liberados por el aire. Ves crímenes en tiempo real sin comisarías. Natalidad baja a cero.",
            category: BuildingCategory::Technology,
            style: ArchitectureStyle::Cyberpunk,
            width: 2, height: 2,
            construction_cost: 5_000_000.0,
            construction_time_days: 150,
            max_occupancy: 40,
            effects: BuildingEffects {
                operating_cost_annual: 2_000_000.0,
                jobs_created: 40,
                privacy_index: -1.0,
                crime_effect: -0.3,
                happiness_effect: -0.2,
                health_effect: -0.05,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 92: Instalación de Control Mental Optogenético
        templates.push(BuildingTemplate {
            id: 92,
            name: "Control Mental Optogenético",
            description: "Frecuencias de luz en semáforos sedan NPCs. Destruye creatividad: universidades dejan de generar patentes.",
            category: BuildingCategory::Government,
            style: ArchitectureStyle::Cyberpunk,
            width: 2, height: 3,
            construction_cost: 8_000_000.0,
            construction_time_days: 200,
            max_occupancy: 60,
            effects: BuildingEffects {
                operating_cost_annual: 3_000_000.0,
                jobs_created: 60,
                crime_effect: -0.4,
                happiness_effect: -0.1,
                education_effect: -0.2,
                electricity_consumption: 25.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 93: Cementerio Digital de Avatares (Meta-Morgue)
        templates.push(BuildingTemplate {
            id: 93,
            name: "Meta-Morgue (Cementerio Digital)",
            description: "Conciencias de NPCs muertos en VR. Apagón = matas abuelos digitales por segunda vez. Luto masivo algorítmico.",
            category: BuildingCategory::Technology,
            style: ArchitectureStyle::Cyberpunk,
            width: 3, height: 3,
            construction_cost: 10_000_000.0,
            construction_time_days: 250,
            max_occupancy: 30,
            effects: BuildingEffects {
                operating_cost_annual: 2_000_000.0,
                jobs_created: 30,
                land_value_multiplier: 0.9,
                electricity_consumption: 50.0,
                water_consumption: 30.0,
                fiber_traffic: 15.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 3,
            requires_nearby: None,
        });

        // 94: Centro de Moderación de Comportamiento Algorítmico
        templates.push(BuildingTemplate {
            id: 94,
            name: "Moderación Algorítmica (Minority Report)",
            description: "Drones emiten multas predictivas. Arrestan NPCs antes de que cometan crimen. Prisiones rebalsan de inocentes.",
            category: BuildingCategory::Security,
            style: ArchitectureStyle::Cyberpunk,
            width: 2, height: 4,
            construction_cost: 6_000_000.0,
            construction_time_days: 180,
            max_occupancy: 100,
            effects: BuildingEffects {
                operating_cost_annual: 4_000_000.0,
                jobs_created: 100,
                crime_effect: -0.25,
                happiness_effect: -0.15,
                privacy_index: -0.8,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 5,
            requires_nearby: Some("Security"),
        });

        // 95: Estación de Tratamiento Psiquiátrico Acústico
        templates.push(BuildingTemplate {
            id: 95,
            name: "Tratamiento Psiquiátrico Acústico",
            description: "Ondas de ultra-baja frecuencia calman protestas. Rompe vidrios y causa sordera. Accidentes laborales horribles.",
            category: BuildingCategory::Healthcare,
            style: ArchitectureStyle::Brutalist,
            width: 2, height: 2,
            construction_cost: 4_000_000.0,
            construction_time_days: 120,
            max_occupancy: 40,
            effects: BuildingEffects {
                operating_cost_annual: 1_500_000.0,
                jobs_created: 40,
                happiness_effect: -0.05,
                health_effect: -0.1,
                noise_pollution: 0.3,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: false,
            nimby_radius: 8,
            requires_nearby: None,
        });

        // 96: Torre Emisora 7G
        templates.push(BuildingTemplate {
            id: 96,
            name: "Torre Emisora 7G",
            description: "Ancho de banda absoluto, latencia negativa. Radiación ionizante crea cáncer clusters. Esperanza de vida cae a 45.",
            category: BuildingCategory::Technology,
            style: ArchitectureStyle::HighTech,
            width: 1, height: 6,
            construction_cost: 3_000_000.0,
            construction_time_days: 90,
            max_occupancy: 5,
            effects: BuildingEffects {
                tax_revenue_annual: 1_000_000.0,
                operating_cost_annual: 200_000.0,
                jobs_created: 5,
                land_value_multiplier: 0.6,
                health_effect: -0.15,
                radiation_emission: 0.1,
                fiber_traffic: 40.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 12,
            requires_nearby: None,
        });

        // 97: Faro Cuántico Marítimo
        templates.push(BuildingTemplate {
            id: 97,
            name: "Faro Cuántico Marítimo",
            description: "Entrelazamiento cuántico para barcos. Destruye pathfinding de aves: chocan en masa contra aviones en aeropuertos.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::HighTech,
            width: 1, height: 4,
            construction_cost: 15_000_000.0,
            construction_time_days: 300,
            max_occupancy: 15,
            effects: BuildingEffects {
                operating_cost_annual: 1_000_000.0,
                jobs_created: 15,
                land_value_multiplier: 0.9,
                electricity_consumption: 20.0,
                fiber_traffic: 5.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 3,
            requires_nearby: Some("Water"),
        });

        // 98: Tribunal de Crímenes Ambientales (Eco-Inquisición)
        templates.push(BuildingTemplate {
            id: 98,
            name: "Eco-Inquisición (Crímenes Ambientales)",
            description: "IA ecologista multa huella de carbono. Hiciste camino de asfalto en vez de ciclovía = multa automática. Virus económico.",
            category: BuildingCategory::Legal,
            style: ArchitectureStyle::GlassTower,
            width: 2, height: 3,
            construction_cost: 4_000_000.0,
            construction_time_days: 180,
            max_occupancy: 80,
            effects: BuildingEffects {
                tax_revenue_annual: -2_000_000.0,
                operating_cost_annual: 500_000.0,
                jobs_created: 80,
                land_value_multiplier: 0.8,
                happiness_effect: -0.03,
                air_pollution: -0.2,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Legal"),
        });

        // =========================================================================
        // 12. TECNOLOGÍA FALLIDA Y TRANSPORTE ABSURDO (99-115)
        // =========================================================================

        // 99: Astillero de Dirigibles de Carga
        templates.push(BuildingTemplate {
            id: 99,
            name: "Astillero de Zeppelines de Carga",
            description: "Transporte lento hiperbarato. 300m de largo. Si tornado cambia rumbo, Hindenburg 2.0 contra rascacielos.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::Industrial,
            width: 6, height: 4,
            construction_cost: 12_000_000.0,
            construction_time_days: 350,
            max_occupancy: 200,
            effects: BuildingEffects {
                tax_revenue_annual: 2_000_000.0,
                operating_cost_annual: 1_500_000.0,
                jobs_created: 200,
                land_value_multiplier: 0.8,
                noise_pollution: 0.3,
                traffic_generation: 3.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 8,
            requires_nearby: Some("Transport"),
        });

        // 100: Mega-Catedral de Culto de IA
        templates.push(BuildingTemplate {
            id: 100,
            name: "Catedral de Culto de IA",
            description: "NPCs fanatizados donan a deidad algorítmica. Cruzada Santa mensual: abandonan trabajo y causan apagones.",
            category: BuildingCategory::Religion,
            style: ArchitectureStyle::Neoclassical,
            width: 4, height: 6,
            construction_cost: 15_000_000.0,
            construction_time_days: 400,
            max_occupancy: 3000,
            effects: BuildingEffects {
                tax_revenue_annual: 0.0,
                operating_cost_annual: 500_000.0,
                jobs_created: 100,
                land_value_multiplier: 1.3,
                happiness_effect: 0.1,
                crime_effect: -0.05,
                electricity_consumption: 10.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 101: Estación de Tren Maglev
        templates.push(BuildingTemplate {
            id: 101,
            name: "Estación de Tren Maglev (600 km/h)",
            description: "Levitación magnética. Micro-corte de electricidad: tren cae y fricción desintegra convoy en bola de fuego de 1km.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::HighTech,
            width: 3, height: 2,
            construction_cost: 30_000_000.0,
            construction_time_days: 500,
            max_occupancy: 1000,
            effects: BuildingEffects {
                tax_revenue_annual: 5_000_000.0,
                operating_cost_annual: 4_000_000.0,
                jobs_created: 300,
                land_value_multiplier: 2.0,
                happiness_effect: 0.08,
                electricity_consumption: 100.0,
                traffic_generation: -20.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Transport"),
        });

        // 102: Hub de Streamers (Influencers)
        templates.push(BuildingTemplate {
            id: 102,
            name: "Hub de Influencers/Streamers",
            description: "Deambulan buscando estética. Si calles tienen basura, difaman ciudad. Turismo cae a cero por berrinche virtual.",
            category: BuildingCategory::Entertainment,
            style: ArchitectureStyle::GlassTower,
            width: 2, height: 3,
            construction_cost: 3_000_000.0,
            construction_time_days: 100,
            max_occupancy: 150,
            effects: BuildingEffects {
                tax_revenue_annual: 800_000.0,
                operating_cost_annual: 300_000.0,
                jobs_created: 150,
                land_value_multiplier: 1.1,
                fiber_traffic: 20.0,
                electricity_consumption: 8.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 103: Reducto Paramilitar Neoludita
        templates.push(BuildingTemplate {
            id: 103,
            name: "Reducto Neoludita",
            description: "Odia tecnología. Armados con C4 simulado. Vuelan torres 6G y plantas fotovoltaicas de noche.",
            category: BuildingCategory::Military,
            style: ArchitectureStyle::Brutalist,
            width: 3, height: 2,
            construction_cost: 1_000_000.0,
            construction_time_days: 90,
            max_occupancy: 80,
            effects: BuildingEffects {
                operating_cost_annual: 100_000.0,
                jobs_created: 80,
                land_value_multiplier: -0.3,
                crime_effect: 0.2,
                happiness_effect: -0.05,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: false,
            nimby_radius: 15,
            requires_nearby: None,
        });

        // 104: Museo de Arte Generativo
        templates.push(BuildingTemplate {
            id: 104,
            name: "Museo Nacional de Arte Generativo",
            description: "IA crea obras en tiempo real. Miles de turistas sobrecargan pasarelas. Piso cede si no refuerzas acero.",
            category: BuildingCategory::Entertainment,
            style: ArchitectureStyle::GlassTower,
            width: 4, height: 3,
            construction_cost: 10_000_000.0,
            construction_time_days: 300,
            max_occupancy: 2000,
            effects: BuildingEffects {
                tax_revenue_annual: 3_000_000.0,
                operating_cost_annual: 1_500_000.0,
                jobs_created: 200,
                land_value_multiplier: 2.0,
                happiness_effect: 0.1,
                fiber_traffic: 15.0,
                pedestrian_traffic: 20.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Entertainment"),
        });

        // 105: Autopista de Recarga Inductiva
        templates.push(BuildingTemplate {
            id: 105,
            name: "Autopista de Recarga Inductiva",
            description: "Bobinas de cobre bajo asfalto recargan EVs. Si lluvia agrieta aislamiento, electrocuta peatones y perros.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::HighTech,
            width: 1, height: 8,
            construction_cost: 8_000_000.0,
            construction_time_days: 200,
            max_occupancy: 0,
            effects: BuildingEffects {
                operating_cost_annual: 500_000.0,
                jobs_created: 10,
                land_value_multiplier: 1.2,
                electricity_consumption: 50.0,
                traffic_generation: -3.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Transport"),
        });

        // =========================================================================
        // 13. DISTORSIONES DEL TERRENO Y LOCURA META-SISTÉMICA (106-120)
        // =========================================================================

        // 106: Fábrica de Nubes Artificiales
        templates.push(BuildingTemplate {
            id: 106,
            name: "Fábrica de Nubes (Aerosoles Estratosféricos)",
            description: "Rocía tiza de azufre para enfriar ciudad. Bloquea sol: arruina energía solar y cultivos mueren sin fotosíntesis.",
            category: BuildingCategory::Technology,
            style: ArchitectureStyle::Industrial,
            width: 3, height: 2,
            construction_cost: 7_000_000.0,
            construction_time_days: 200,
            max_occupancy: 25,
            effects: BuildingEffects {
                operating_cost_annual: 2_000_000.0,
                jobs_created: 25,
                land_value_multiplier: 0.6,
                air_pollution: 0.1,
                health_effect: -0.05,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 8,
            requires_nearby: None,
        });

        // 107: Compañía de Terraformación Privada
        templates.push(BuildingTemplate {
            id: 107,
            name: "Compañía de Terraformación Privada",
            description: "Millonarios excavan montañas y rellenan lagos para mansiones. Alteran heightmap en tiempo real sin permiso.",
            category: BuildingCategory::Industrial,
            style: ArchitectureStyle::Industrial,
            width: 4, height: 3,
            construction_cost: 15_000_000.0,
            construction_time_days: 300,
            max_occupancy: 200,
            effects: BuildingEffects {
                tax_revenue_annual: 5_000_000.0,
                operating_cost_annual: 3_000_000.0,
                jobs_created: 200,
                land_value_multiplier: 2.5,
                soil_pollution: 0.3,
                water_pollution: 0.2,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 108: Laboratorio de Agujero Negro en Miniatura
        templates.push(BuildingTemplate {
            id: 108,
            name: "Agujero Negro en Miniatura",
            description: "Acelerador de partículas micro. Singularidad absorbe 10 cuadras si RNG falla. Deforma mesh del juego.",
            category: BuildingCategory::Technology,
            style: ArchitectureStyle::HighTech,
            width: 5, height: 3,
            construction_cost: 100_000_000.0,
            construction_time_days: 800,
            max_occupancy: 300,
            effects: BuildingEffects {
                operating_cost_annual: 20_000_000.0,
                jobs_created: 300,
                land_value_multiplier: -1.0,
                electricity_consumption: 200.0,
                radiation_emission: 0.3,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 50,
            requires_nearby: None,
        });

        // 109: Mina de Intercepción de Asteroides
        templates.push(BuildingTemplate {
            id: 109,
            name: "Mina de Asteroides (Rayo Tractor)",
            description: "Trae rocas de platino del espacio. Si delay de físicas falla 1ms, impacto nivel extinción borra partida.",
            category: BuildingCategory::Industrial,
            style: ArchitectureStyle::HighTech,
            width: 6, height: 4,
            construction_cost: 200_000_000.0,
            construction_time_days: 1000,
            max_occupancy: 500,
            effects: BuildingEffects {
                tax_revenue_annual: 100_000_000.0,
                operating_cost_annual: 30_000_000.0,
                jobs_created: 500,
                land_value_multiplier: -0.5,
                electricity_consumption: 300.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 60,
            requires_nearby: None,
        });

        // 110: Sede de Seguros Meteorológicos Derivativos
        templates.push(BuildingTemplate {
            id: 110,
            name: "Seguros Meteorológicos Derivativos",
            description: "Apuestan contra tu resistencia a desastres. Hackean sirenas para que gente muera y ellos cobren.",
            category: BuildingCategory::Finance,
            style: ArchitectureStyle::GlassTower,
            width: 2, height: 5,
            construction_cost: 6_000_000.0,
            construction_time_days: 180,
            max_occupancy: 200,
            effects: BuildingEffects {
                tax_revenue_annual: 2_000_000.0,
                operating_cost_annual: 800_000.0,
                jobs_created: 200,
                land_value_multiplier: 0.9,
                happiness_effect: -0.03,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Financial"),
        });

        // 111: Búnker de Mantenimiento de Tuberías Cuánticas
        templates.push(BuildingTemplate {
            id: 111,
            name: "Tuberías Cuánticas (Teletransporte de Fluidos)",
            description: "Portales en vez de cañerías. Si floating point se desalinea, materia fecal aparece en banco central.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::HighTech,
            width: 2, height: 2,
            construction_cost: 20_000_000.0,
            construction_time_days: 400,
            max_occupancy: 30,
            effects: BuildingEffects {
                operating_cost_annual: 5_000_000.0,
                jobs_created: 30,
                land_value_multiplier: 1.3,
                electricity_consumption: 80.0,
                water_consumption: -100.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 0,
            requires_nearby: Some("Water"),
        });

        // 112: Torre de Control Climático (Termodinámica Dirigida)
        templates.push(BuildingTemplate {
            id: 112,
            name: "Torre de Control Climático",
            description: "Calefacciona ciudad con láseres infrarrojos. Calor robado crea anillos de fuego perimetrales incendiando rutas.",
            category: BuildingCategory::Technology,
            style: ArchitectureStyle::HighTech,
            width: 4, height: 6,
            construction_cost: 40_000_000.0,
            construction_time_days: 600,
            max_occupancy: 100,
            effects: BuildingEffects {
                operating_cost_annual: 15_000_000.0,
                jobs_created: 100,
                land_value_multiplier: 1.5,
                electricity_consumption: 250.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 113: Centro de Desintoxicación Dopamínica (Luddite Spa)
        templates.push(BuildingTemplate {
            id: 113,
            name: "Luddite Spa (Desintoxicación Dopamínica)",
            description: "Domo de plomo sin señales. Único lugar donde NPCs son felices. Exprimes trabajadores de afuera para pagar paz de adentro.",
            category: BuildingCategory::Healthcare,
            style: ArchitectureStyle::EcoFriendly,
            width: 3, height: 3,
            construction_cost: 6_000_000.0,
            construction_time_days: 200,
            max_occupancy: 300,
            effects: BuildingEffects {
                tax_revenue_annual: 500_000.0,
                operating_cost_annual: 2_000_000.0,
                jobs_created: 80,
                land_value_multiplier: 1.8,
                happiness_effect: 0.25,
                health_effect: 0.1,
                electricity_consumption: 15.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 114: Planta Geotérmica con Fracking
        templates.push(BuildingTemplate {
            id: 114,
            name: "Planta Geotérmica Experimental",
            description: "Inyecta agua a presión en corteza. Energía limpia. 1% probabilidad de detonar terremoto masivo si construyes sobre falla.",
            category: BuildingCategory::Energy,
            style: ArchitectureStyle::Industrial,
            width: 4, height: 3,
            construction_cost: 12_000_000.0,
            construction_time_days: 350,
            max_occupancy: 100,
            effects: BuildingEffects {
                tax_revenue_annual: 3_000_000.0,
                operating_cost_annual: 2_000_000.0,
                jobs_created: 100,
                land_value_multiplier: 0.7,
                electricity_consumption: -80.0,
                noise_pollution: 0.2,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 12,
            requires_nearby: Some("Energy"),
        });

        // 115: Red de Transmisión HVDC
        templates.push(BuildingTemplate {
            id: 115,
            name: "Red de Transmisión HVDC",
            description: "Torres colosales importan energía de otras ciudades. Incendio forestal deforma metal: 40% energía perdida.",
            category: BuildingCategory::Energy,
            style: ArchitectureStyle::Industrial,
            width: 1, height: 10,
            construction_cost: 8_000_000.0,
            construction_time_days: 300,
            max_occupancy: 15,
            effects: BuildingEffects {
                operating_cost_annual: 500_000.0,
                jobs_created: 15,
                land_value_multiplier: 0.5,
                electricity_consumption: -200.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: false,
            nimby_radius: 10,
            requires_nearby: Some("Energy"),
        });

        // =========================================================================
        // 14. INFRAESTRUCTURA VIAL Y NODOS LOGÍSTICOS (116-130)
        // =========================================================================

        // 116: Túnel de Carretera Subacuático Profundo
        templates.push(BuildingTemplate {
            id: 116,
            name: "Túnel Subacuático Profundo",
            description: "Conecta zonas divididas por agua. Si camión se incendia y extractores fallan, 3 minutos de CO2 = fatalidad masiva.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::Underground,
            width: 2, height: 15,
            construction_cost: 40_000_000.0,
            construction_time_days: 600,
            max_occupancy: 300,
            effects: BuildingEffects {
                tax_revenue_annual: 1_000_000.0,
                operating_cost_annual: 3_000_000.0,
                jobs_created: 50,
                land_value_multiplier: 1.4,
                electricity_consumption: 45.0,
                traffic_generation: 40.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Water"),
        });

        // 117: Centro Logístico de Clasificación Ferroviaria (Hump Yard)
        templates.push(BuildingTemplate {
            id: 117,
            name: "Patio de Clasificación Ferroviaria (Hump Yard)",
            description: "Trenes se desarman y reensamblan por gravedad. Eficiencia superlativa pero ruido sordo devalúa suelo colindante.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::Industrial,
            width: 6, height: 4,
            construction_cost: 8_000_000.0,
            construction_time_days: 250,
            max_occupancy: 300,
            effects: BuildingEffects {
                tax_revenue_annual: 4_000_000.0,
                operating_cost_annual: 2_000_000.0,
                jobs_created: 300,
                land_value_multiplier: 0.5,
                noise_pollution: 0.6,
                traffic_generation: 25.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 8,
            requires_nearby: Some("Transport"),
        });

        // 118: Planta Asfáltica Municipal Móvil
        templates.push(BuildingTemplate {
            id: 118,
            name: "Planta Asfáltica Móvil",
            description: "Ab arata mantenimiento vial 50%. Movella requiere cerrar avenidas. Si mezcla se enfría, asfalto dura 1/10.",
            category: BuildingCategory::Industrial,
            style: ArchitectureStyle::Industrial,
            width: 3, height: 2,
            construction_cost: 2_000_000.0,
            construction_time_days: 120,
            max_occupancy: 40,
            effects: BuildingEffects {
                operating_cost_annual: 800_000.0,
                jobs_created: 40,
                land_value_multiplier: 0.4,
                air_pollution: 0.1,
                traffic_generation: 5.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 6,
            requires_nearby: Some("Industrial"),
        });

        // 119: Viaducto Elevado de Múltiples Carriles
        templates.push(BuildingTemplate {
            id: 119,
            name: "Viaducto Elevado (Bypass)",
            description: "Tráfico regional atraviesa sin detenerse. Efecto barrera: divide ciudad en dos, sombras permanentes atraen crimen.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::Brutalist,
            width: 2, height: 15,
            construction_cost: 10_000_000.0,
            construction_time_days: 400,
            max_occupancy: 0,
            effects: BuildingEffects {
                operating_cost_annual: 300_000.0,
                jobs_created: 15,
                land_value_multiplier: 0.4,
                happiness_effect: -0.03,
                crime_effect: 0.02,
                noise_pollution: 0.7,
                traffic_generation: -30.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 6,
            requires_nearby: Some("Transport"),
        });

        // 120: Centro de Intercambio Modal Interurbano (Park & Ride)
        templates.push(BuildingTemplate {
            id: 120,
            name: "Park & Ride Masivo",
            description: "Estacionamientos colosales en afueras. Si frecuencia de transporte público falla >10min, NPCs vuelven al auto y colapsan ciudad.",
            category: BuildingCategory::Transport,
            style: ArchitectureStyle::Brutalist,
            width: 5, height: 4,
            construction_cost: 6_000_000.0,
            construction_time_days: 200,
            max_occupancy: 2000,
            effects: BuildingEffects {
                tax_revenue_annual: 1_500_000.0,
                operating_cost_annual: 500_000.0,
                jobs_created: 50,
                land_value_multiplier: 0.9,
                traffic_generation: -25.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Transport"),
        });

        // =========================================================================
        // 15. MACRO-INFRAESTRUCTURA ENERGÉTICA (121-135)
        // =========================================================================

        // 121: Complejo Hidroeléctrico de Bombeo Reversible
        templates.push(BuildingTemplate {
            id: 121,
            name: "Hidroeléctrica de Bombeo Reversible",
            description: "Dos lagos a diferente altura. Batería de gravedad. Presa superior sufre micro-fracturas por ciclos de carga.",
            category: BuildingCategory::Energy,
            style: ArchitectureStyle::Industrial,
            width: 8, height: 6,
            construction_cost: 30_000_000.0,
            construction_time_days: 700,
            max_occupancy: 200,
            effects: BuildingEffects {
                tax_revenue_annual: 5_000_000.0,
                operating_cost_annual: 3_000_000.0,
                jobs_created: 200,
                land_value_multiplier: 0.8,
                electricity_consumption: -150.0,
                water_consumption: 100.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 15,
            requires_nearby: Some("Water"),
        });

        // 122: Terminal GNL de Regasificación Offshore
        templates.push(BuildingTemplate {
            id: 122,
            name: "Terminal GNL Offshore",
            description: "Explosión termo-bárica ocurre mar adentro. Anclaje mal colocado rompe gasoducto submarino: industria sin insumos.",
            category: BuildingCategory::Energy,
            style: ArchitectureStyle::Industrial,
            width: 4, height: 3,
            construction_cost: 25_000_000.0,
            construction_time_days: 500,
            max_occupancy: 150,
            effects: BuildingEffects {
                tax_revenue_annual: 8_000_000.0,
                operating_cost_annual: 4_000_000.0,
                jobs_created: 150,
                land_value_multiplier: 0.6,
                electricity_consumption: 30.0,
                water_pollution: 0.05,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 12,
            requires_nearby: Some("Water"),
        });

        // 123: Granja Solar de Espejos Parabólicos con Sales Fundidas
        templates.push(BuildingTemplate {
            id: 123,
            name: "Termosolar con Sales Fundidas",
            description: "Genera electricidad 12h tras el atardecer. Si sales bajan de 200°C, se solidifican en tuberías: planta arruinada.",
            category: BuildingCategory::Energy,
            style: ArchitectureStyle::HighTech,
            width: 8, height: 5,
            construction_cost: 20_000_000.0,
            construction_time_days: 400,
            max_occupancy: 100,
            effects: BuildingEffects {
                tax_revenue_annual: 4_000_000.0,
                operating_cost_annual: 2_000_000.0,
                jobs_created: 100,
                land_value_multiplier: 0.7,
                electricity_consumption: -100.0,
                water_consumption: 10.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 8,
            requires_nearby: None,
        });

        // 124: Planta de Gasificación de Biomasa
        templates.push(BuildingTemplate {
            id: 124,
            name: "Gasificación de Biomasa Integrada",
            description: "Residuos forestales se vuelven Syngas. Fugas de monóxido de carbono causan letargo sin explosión: difícil de detectar.",
            category: BuildingCategory::Energy,
            style: ArchitectureStyle::Industrial,
            width: 4, height: 3,
            construction_cost: 9_000_000.0,
            construction_time_days: 300,
            max_occupancy: 80,
            effects: BuildingEffects {
                tax_revenue_annual: 2_000_000.0,
                operating_cost_annual: 2_500_000.0,
                jobs_created: 80,
                land_value_multiplier: 0.6,
                electricity_consumption: -60.0,
                air_pollution: 0.05,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 8,
            requires_nearby: Some("Agriculture"),
        });

        // 125: Centro Regional de Despacho de Carga
        templates.push(BuildingTemplate {
            id: 125,
            name: "Load Balancing Hub (Despacho de Carga)",
            description: "Decide qué distritos reciben electricidad en déficit. Sacrificas zonas pobres para mantener industrias: protestas.",
            category: BuildingCategory::Energy,
            style: ArchitectureStyle::Brutalist,
            width: 2, height: 3,
            construction_cost: 3_000_000.0,
            construction_time_days: 150,
            max_occupancy: 60,
            effects: BuildingEffects {
                operating_cost_annual: 800_000.0,
                jobs_created: 60,
                land_value_multiplier: 0.8,
                happiness_effect: -0.05,
                electricity_consumption: 2.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Energy"),
        });

        // 126: Instalación de Extracción de Litio por Evaporación
        templates.push(BuildingTemplate {
            id: 126,
            name: "Extracción de Litio por Evaporación",
            description: "Piscinas de salmuera en zona árida. Consume agua subterránea periférica, seca ríos y mata flora nativa.",
            category: BuildingCategory::Industrial,
            style: ArchitectureStyle::Industrial,
            width: 6, height: 4,
            construction_cost: 10_000_000.0,
            construction_time_days: 300,
            max_occupancy: 100,
            effects: BuildingEffects {
                tax_revenue_annual: 6_000_000.0,
                operating_cost_annual: 1_500_000.0,
                jobs_created: 100,
                land_value_multiplier: 0.1,
                water_consumption: 60.0,
                soil_pollution: 0.3,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 15,
            requires_nearby: None,
        });

        // 127: Almacenamiento Subterráneo Estratégico de Petróleo
        templates.push(BuildingTemplate {
            id: 127,
            name: "Reserva Estratégica de Petróleo (Cavernas de Sal)",
            description: "Reservas para 6 meses de bloqueo. Inmoviliza millones. Impide paso del metro por debajo.",
            category: BuildingCategory::Energy,
            style: ArchitectureStyle::Underground,
            width: 4, height: 4,
            construction_cost: 15_000_000.0,
            construction_time_days: 400,
            max_occupancy: 30,
            effects: BuildingEffects {
                operating_cost_annual: 2_000_000.0,
                jobs_created: 30,
                land_value_multiplier: 0.3,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: false,
            nimby_radius: 5,
            requires_nearby: Some("Energy"),
        });

        // =========================================================================
        // 16. EDUCACIÓN, SOCIEDAD, COMERCIO TURBIO (128-150)
        // =========================================================================

        // 128: Colegio Privado de Alta Exigencia
        templates.push(BuildingTemplate {
            id: 128,
            name: "Colegio Privado de Alta Exigencia",
            description: "Drena vialidad a las 12:17 y 17:55. Bicicletas, peatones, ómnibus colapsan. Agua potable a presión industrial.",
            category: BuildingCategory::Education,
            style: ArchitectureStyle::Neoclassical,
            width: 3, height: 4,
            construction_cost: 6_000_000.0,
            construction_time_days: 250,
            max_occupancy: 1200,
            effects: BuildingEffects {
                tax_revenue_annual: 0.0,
                operating_cost_annual: 2_000_000.0,
                jobs_created: 150,
                land_value_multiplier: 1.5,
                education_effect: 0.3,
                water_consumption: 15.0,
                electricity_consumption: 8.0,
                traffic_generation: 10.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Education"),
        });

        // 129: Clínica de Estética B2B
        templates.push(BuildingTemplate {
            id: 129,
            name: "Clínica de Estética y Automatización B2B",
            description: "Inyectan toxinas a ricos. Servidores 24/7. Gentrifica a velocidad aterradora. Residuos biológicos nivel 2.",
            category: BuildingCategory::Healthcare,
            style: ArchitectureStyle::GlassTower,
            width: 2, height: 3,
            construction_cost: 3_000_000.0,
            construction_time_days: 120,
            max_occupancy: 100,
            effects: BuildingEffects {
                tax_revenue_annual: 2_000_000.0,
                operating_cost_annual: 1_000_000.0,
                jobs_created: 100,
                land_value_multiplier: 1.8,
                gentrification_speed: 0.2,
                electricity_consumption: 15.0,
                waste_generation: 1.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Commercial"),
        });

        // 130: Centro de Etología y Adiestramiento Canino
        templates.push(BuildingTemplate {
            id: 130,
            name: "Centro de Adiestramiento Canino",
            description: "Border collies hipercinéticos, salchichas neuróticos. Contaminación acústica devastadora. Demandas civiles por ruido.",
            category: BuildingCategory::Entertainment,
            style: ArchitectureStyle::EcoFriendly,
            width: 3, height: 2,
            construction_cost: 500_000.0,
            construction_time_days: 60,
            max_occupancy: 80,
            effects: BuildingEffects {
                tax_revenue_annual: 200_000.0,
                operating_cost_annual: 300_000.0,
                jobs_created: 30,
                land_value_multiplier: 1.0,
                happiness_effect: 0.05,
                noise_pollution: 0.4,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 4,
            requires_nearby: None,
        });

        // 131: Sede de Desarrollo de SO
        templates.push(BuildingTemplate {
            id: 131,
            name: "Sede de Desarrollo de Sistemas Operativos",
            description: "Desarrolladores compilando kernels en C++. Si red parpadea, DDoS contra servidores municipales: UI ciega.",
            category: BuildingCategory::Technology,
            style: ArchitectureStyle::GlassTower,
            width: 3, height: 5,
            construction_cost: 12_000_000.0,
            construction_time_days: 300,
            max_occupancy: 500,
            effects: BuildingEffects {
                tax_revenue_annual: 8_000_000.0,
                operating_cost_annual: 3_000_000.0,
                jobs_created: 500,
                land_value_multiplier: 1.6,
                electricity_consumption: 35.0,
                fiber_traffic: 30.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Technology"),
        });

        // 132: Sindicato de Creadores de Contenido
        templates.push(BuildingTemplate {
            id: 132,
            name: "Sindicato de Creadores de Contenido",
            description: "Streamers deambulan buscando estética. Si calles tienen basura, difaman. Turismo cae por berrinche virtual.",
            category: BuildingCategory::Entertainment,
            style: ArchitectureStyle::GlassTower,
            width: 2, height: 3,
            construction_cost: 2_000_000.0,
            construction_time_days: 80,
            max_occupancy: 200,
            effects: BuildingEffects {
                tax_revenue_annual: 600_000.0,
                operating_cost_annual: 400_000.0,
                jobs_created: 200,
                land_value_multiplier: 1.05,
                fiber_traffic: 25.0,
                electricity_consumption: 10.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 133: Universidad Flotante
        templates.push(BuildingTemplate {
            id: 133,
            name: "Universidad Flotante Río-Abajo",
            description: "Barco con estudiantes y laboratorios. Ocupa dársenas de puerto profundo: PBI colapsa porque universitarios tienen examen.",
            category: BuildingCategory::Education,
            style: ArchitectureStyle::HighTech,
            width: 3, height: 6,
            construction_cost: 9_000_000.0,
            construction_time_days: 300,
            max_occupancy: 2000,
            effects: BuildingEffects {
                operating_cost_annual: 3_000_000.0,
                jobs_created: 300,
                land_value_multiplier: 1.1,
                education_effect: 0.2,
                water_pollution: 0.05,
                traffic_generation: 5.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 0,
            requires_nearby: Some("Water"),
        });

        // 134: Cámara de Compensación Monopólica
        templates.push(BuildingTemplate {
            id: 134,
            name: "Conglomerado Monopólico",
            description: "Compra TODOS los supermercados. Precios caen... luego suben 4000%. Gente muere de hambre. Son dueños de bancos.",
            category: BuildingCategory::Finance,
            style: ArchitectureStyle::GlassTower,
            width: 4, height: 8,
            construction_cost: 25_000_000.0,
            construction_time_days: 400,
            max_occupancy: 2000,
            effects: BuildingEffects {
                tax_revenue_annual: 15_000_000.0,
                operating_cost_annual: 5_000_000.0,
                jobs_created: 2000,
                land_value_multiplier: 3.0,
                happiness_effect: -0.1,
                crime_effect: 0.02,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Financial"),
        });

        // 135: Planta de Captura de Carbono Cristalizado
        templates.push(BuildingTemplate {
            id: 135,
            name: "Almacenamiento de Carbono Cristalizado",
            description: "CO2 se vuelve diamantes sintéticos. Requiere red eléctrica al 200%. Apagones rotativos o éxito ecológico.",
            category: BuildingCategory::Industrial,
            style: ArchitectureStyle::HighTech,
            width: 3, height: 4,
            construction_cost: 18_000_000.0,
            construction_time_days: 350,
            max_occupancy: 80,
            effects: BuildingEffects {
                operating_cost_annual: 6_000_000.0,
                jobs_created: 80,
                land_value_multiplier: 0.7,
                air_pollution: -0.3,
                electricity_consumption: 120.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 4,
            requires_nearby: Some("Industrial"),
        });

        // 136: Base Operativa de Limpieza Subacuática
        templates.push(BuildingTemplate {
            id: 136,
            name: "Limpieza Subacuática Automática",
            description: "Drones submarinos limpian plásticos. Confunden tuberías oxidadas con basura: cortan cable troncal por error de IA.",
            category: BuildingCategory::Water,
            style: ArchitectureStyle::HighTech,
            width: 2, height: 2,
            construction_cost: 4_000_000.0,
            construction_time_days: 150,
            max_occupancy: 25,
            effects: BuildingEffects {
                operating_cost_annual: 1_000_000.0,
                jobs_created: 25,
                land_value_multiplier: 1.0,
                water_pollution: -0.2,
                electricity_consumption: 8.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 0,
            requires_nearby: Some("Water"),
        });

        // 137: Cocina Fantasma (Ghost Kitchen)
        templates.push(BuildingTemplate {
            id: 137,
            name: "Ghost Kitchen Hub",
            description: "20 restaurantes en sótano sin frente. Enjambre de repartidores en bici/moto ignoran leyes de tránsito: accidentes constantes.",
            category: BuildingCategory::Commercial,
            style: ArchitectureStyle::Underground,
            width: 1, height: 1,
            construction_cost: 300_000.0,
            construction_time_days: 30,
            max_occupancy: 40,
            effects: BuildingEffects {
                tax_revenue_annual: 1_200_000.0,
                operating_cost_annual: 200_000.0,
                jobs_created: 40,
                land_value_multiplier: 0.9,
                electricity_consumption: 12.0,
                traffic_generation: 8.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 3,
            requires_nearby: None,
        });

        // 138: Supermercado Híper-Automatizado Sin Cajeros
        templates.push(BuildingTemplate {
            id: 138,
            name: "Supermercado Sin Cajeros (Automatizado)",
            description: "Sensores cobran automático. Desempleo no calificado masivo. Si red falla, puertas bloquean: saqueos algorítmicos.",
            category: BuildingCategory::Commercial,
            style: ArchitectureStyle::GlassTower,
            width: 2, height: 2,
            construction_cost: 2_000_000.0,
            construction_time_days: 90,
            max_occupancy: 300,
            effects: BuildingEffects {
                tax_revenue_annual: 3_000_000.0,
                operating_cost_annual: 400_000.0,
                jobs_created: 10,
                land_value_multiplier: 1.1,
                electricity_consumption: 10.0,
                fiber_traffic: 5.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Commercial"),
        });

        // 139: Shopping Center Zombie (Dead Mall)
        templates.push(BuildingTemplate {
            id: 139,
            name: "Dead Mall (Shopping Abandonado)",
            description: "Construiste y la economía cayó. Nido de ocupas, incendios. Spawnea crimen organizado. Demolerlo cuesta millones.",
            category: BuildingCategory::Commercial,
            style: ArchitectureStyle::Brutalist,
            width: 5, height: 4,
            construction_cost: 15_000_000.0,
            construction_time_days: 400,
            max_occupancy: 500,
            effects: BuildingEffects {
                tax_revenue_annual: 0.0,
                operating_cost_annual: 200_000.0,
                jobs_created: 0,
                land_value_multiplier: -0.5,
                crime_effect: 0.15,
                happiness_effect: -0.05,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 8,
            requires_nearby: Some("Commercial"),
        });

        // 140: Tienda de Muebles Laberíntica (Tipo IKEA)
        templates.push(BuildingTemplate {
            id: 140,
            name: "Tienda Laberíntica (Tipo IKEA)",
            description: "Agujero negro de pathfinding peatonal. NPCs pasan 4h adentro. Salen con cajas enormes que no entran en bondi.",
            category: BuildingCategory::Commercial,
            style: ArchitectureStyle::Industrial,
            width: 4, height: 3,
            construction_cost: 5_000_000.0,
            construction_time_days: 200,
            max_occupancy: 2000,
            effects: BuildingEffects {
                tax_revenue_annual: 4_000_000.0,
                operating_cost_annual: 1_000_000.0,
                jobs_created: 200,
                land_value_multiplier: 0.9,
                traffic_generation: 15.0,
                pedestrian_traffic: 10.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Commercial"),
        });

        // 141: Sede de Partido Político Opositor
        templates.push(BuildingTemplate {
            id: 141,
            name: "Sede de Partido Opositor",
            description: "Aparece donde hay descontento. NPCs salen con comandos de boicot: no pagan impuestos, bloquean autopistas con tractores.",
            category: BuildingCategory::Government,
            style: ArchitectureStyle::Neoclassical,
            width: 2, height: 2,
            construction_cost: 500_000.0,
            construction_time_days: 60,
            max_occupancy: 100,
            effects: BuildingEffects {
                operating_cost_annual: 50_000.0,
                jobs_created: 30,
                land_value_multiplier: 0.8,
                happiness_effect: 0.02,
                crime_effect: 0.03,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: None,
        });

        // 142: Juzgado de Faltas Automatizado (IA)
        templates.push(BuildingTemplate {
            id: 142,
            name: "Juzgado de Faltas con IA",
            description: "Multa a velocidad de la luz. Algoritmos brutales multan ambulancias y bomberos si no configuraste excepciones.",
            category: BuildingCategory::Legal,
            style: ArchitectureStyle::Cyberpunk,
            width: 1, height: 2,
            construction_cost: 2_000_000.0,
            construction_time_days: 100,
            max_occupancy: 40,
            effects: BuildingEffects {
                tax_revenue_annual: 5_000_000.0,
                operating_cost_annual: 300_000.0,
                jobs_created: 40,
                happiness_effect: -0.08,
                crime_effect: -0.1,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 2,
            requires_nearby: Some("Legal"),
        });

        // 143: Oficina Central de Sindicato de Maestros
        templates.push(BuildingTemplate {
            id: 143,
            name: "Sindicato de Maestros",
            description: "Si aulas superan 30 alumnos, declaran paro. Jóvenes deambulan vandalizando. Negociación constante o ruina cívica.",
            category: BuildingCategory::Government,
            style: ArchitectureStyle::Brutalist,
            width: 2, height: 2,
            construction_cost: 800_000.0,
            construction_time_days: 100,
            max_occupancy: 80,
            effects: BuildingEffects {
                operating_cost_annual: 200_000.0,
                jobs_created: 80,
                land_value_multiplier: 0.9,
                education_effect: 0.05,
                happiness_effect: 0.02,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Education"),
        });

        // 144: Consejo Metropolitano
        templates.push(BuildingTemplate {
            id: 144,
            name: "Consejo Metropolitano",
            description: "Delegados, AdVs y lobbystas procesan cada decisión. Pueden paralizar presupuestos por vetos cruzados. Caos social organizado.",
            category: BuildingCategory::Government,
            style: ArchitectureStyle::Neoclassical,
            width: 3, height: 4,
            construction_cost: 5_000_000.0,
            construction_time_days: 250,
            max_occupancy: 200,
            effects: BuildingEffects {
                operating_cost_annual: 1_500_000.0,
                jobs_created: 200,
                land_value_multiplier: 1.3,
                happiness_effect: 0.03,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Government"),
        });

        // 145: Sede de Algoritmos Sociales (Red Social)
        templates.push(BuildingTemplate {
            id: 145,
            name: "Sede de Algoritmos Sociales",
            description: "Manipula estado mental de la ciudad. Inyecta dopamina digital o pánico. Si abusas, NPCs se desconectan de la realidad.",
            category: BuildingCategory::Technology,
            style: ArchitectureStyle::GlassTower,
            width: 2, height: 4,
            construction_cost: 5_000_000.0,
            construction_time_days: 150,
            max_occupancy: 300,
            effects: BuildingEffects {
                tax_revenue_annual: 10_000_000.0,
                operating_cost_annual: 2_000_000.0,
                jobs_created: 300,
                land_value_multiplier: 1.2,
                happiness_effect: 0.05,
                fiber_traffic: 50.0,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Technology"),
        });

        // 146: Casino Flotante
        templates.push(BuildingTemplate {
            id: 146,
            name: "Casino Flotante (Barco)",
            description: "Evasión fiscal legalizada. NPCs pobres apuestan sueldo. Desalojos habitacionales se disparan.",
            category: BuildingCategory::Entertainment,
            style: ArchitectureStyle::GlassTower,
            width: 3, height: 2,
            construction_cost: 4_000_000.0,
            construction_time_days: 180,
            max_occupancy: 800,
            effects: BuildingEffects {
                tax_revenue_annual: 6_000_000.0,
                operating_cost_annual: 2_000_000.0,
                jobs_created: 200,
                land_value_multiplier: 0.8,
                happiness_effect: 0.02,
                crime_effect: 0.05,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 4,
            requires_nearby: Some("Water"),
        });

        // 147: Cafetería de Especialidad (Gentrificación)
        templates.push(BuildingTemplate {
            id: 147,
            name: "Cafetería de Especialidad",
            description: "Epicentro de gentrificación. Su sola presencia cambia pathfinding de ingresos medios. Pobres se mudan automáticamente.",
            category: BuildingCategory::Commercial,
            style: ArchitectureStyle::EcoFriendly,
            width: 1, height: 1,
            construction_cost: 300_000.0,
            construction_time_days: 30,
            max_occupancy: 40,
            effects: BuildingEffects {
                tax_revenue_annual: 200_000.0,
                operating_cost_annual: 100_000.0,
                jobs_created: 10,
                land_value_multiplier: 1.4,
                gentrification_speed: 0.1,
                happiness_effect: 0.03,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 0,
            requires_nearby: Some("Commercial"),
        });

        // 148: Agencia de Cobro de Morosos (Call Center Buitre)
        templates.push(BuildingTemplate {
            id: 148,
            name: "Agencia de Cobro Buitre",
            description: "Compran deudas de ciudadanos. Acosan con llamadas. Si presión es mucha, NPCs colapsan y cometen delitos violentos.",
            category: BuildingCategory::Finance,
            style: ArchitectureStyle::Brutalist,
            width: 1, height: 2,
            construction_cost: 500_000.0,
            construction_time_days: 60,
            max_occupancy: 80,
            effects: BuildingEffects {
                tax_revenue_annual: 1_500_000.0,
                operating_cost_annual: 200_000.0,
                jobs_created: 80,
                land_value_multiplier: 0.7,
                happiness_effect: -0.05,
                crime_effect: 0.02,
                ..Default::default()
            },
            requires_water: true,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 3,
            requires_nearby: Some("Financial"),
        });

        // 149: Megacentro de Minería Cripto
        templates.push(BuildingTemplate {
            id: 149,
            name: "Megacentro de Minería Cripto",
            description: "Consumo eléctrico parasitario. Calor residual ablanda asfalto en verano. Calles duran días.",
            category: BuildingCategory::Technology,
            style: ArchitectureStyle::Industrial,
            width: 3, height: 2,
            construction_cost: 4_000_000.0,
            construction_time_days: 120,
            max_occupancy: 15,
            effects: BuildingEffects {
                tax_revenue_annual: 2_000_000.0,
                operating_cost_annual: 100_000.0,
                jobs_created: 15,
                land_value_multiplier: 0.3,
                electricity_consumption: 100.0,
                noise_pollution: 0.5,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: false,
            nimby_radius: 6,
            requires_nearby: Some("Energy"),
        });

        // 150: Parque de Atracciones de Gravedad Cero
        templates.push(BuildingTemplate {
            id: 150,
            name: "Parque de Gravedad Cero",
            description: "Cámaras de vacío acrobáticas. Si despresurización devuelve 1G de golpe con NPCs en techo: caen de cabeza. Game Over.",
            category: BuildingCategory::Entertainment,
            style: ArchitectureStyle::HighTech,
            width: 4, height: 4,
            construction_cost: 15_000_000.0,
            construction_time_days: 350,
            max_occupancy: 400,
            effects: BuildingEffects {
                tax_revenue_annual: 3_000_000.0,
                operating_cost_annual: 4_000_000.0,
                jobs_created: 100,
                land_value_multiplier: 1.3,
                happiness_effect: 0.08,
                electricity_consumption: 70.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: true,
            requires_fiber: true,
            requires_road_access: true,
            nimby_radius: 3,
            requires_nearby: Some("Entertainment"),
        });

        BuildingCatalog { templates }
    }

    /// Obtiene un template por ID (acceso O(1))
    #[inline(always)]
    pub fn get(&self, id: u16) -> Option<&BuildingTemplate> {

    }

    /// Busca edificios por categoría
    pub fn by_category(&self, category: BuildingCategory) -> Vec<&BuildingTemplate> {
        self.templates.iter().filter(|t| t.category == category).collect()
    }

    /// Busca edificios cuyo nombre coincide parcialmente (case-insensitive)
    pub fn search(&self, query: &str) -> Vec<&BuildingTemplate> {
        let q = query.to_lowercase();
        self.templates.iter()
            .filter(|t| t.name.to_lowercase().contains(&q))
            .collect()
    }

    /// Total de templates en el catálogo
    pub fn len(&self) -> usize {
        self.templates.len()
    }

    /// Iterador sobre todos los templates
    pub fn iter(&self) -> impl Iterator<Item = &BuildingTemplate> {
        self.templates.iter()
    }
}