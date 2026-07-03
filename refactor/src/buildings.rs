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
        // 24: Vertedero Municipal
        templates.push(BuildingTemplate {
            id: 24,
            name: "Vertedero Municipal",
            description: "Entierra basura. Si no tiene geomembrana, lixiviados envenenan napas. Genera metano explosivo.",
            construction_time_days: 150,
            max_occupancy: 30,
            effects: BuildingEffects {
                operating_cost_annual: 500_000.0,
                jobs_created: 30,
                land_value_multiplier: 0.05,
                air_pollution: 0.2,
                water_pollution: 0.3,
                soil_pollution: 0.8,
                happiness_effect: -0.05,
                waste_generation: -50.0,  // Absorbe residuos
                traffic_generation: 8.0,
                ..Default::default()
            },
            requires_water: false,
            requires_electricity: false,
            requires_fiber: false,
            requires_road_access: true,
            nimby_radius: 20,
            requires_nearby: None,
        });

        // 25: Incinerador de Residuos
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
            nimby_radius: 12,
            requires_nearby: Some("Water"),
        });

        BuildingCatalog { templates }
    }
    /// Obtiene un template por ID (acceso O(1))
    #[inline(always)]
    pub fn get(&self, id: u16) -> Option<&BuildingTemplate> {
        self.templates.get(id as usize)
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