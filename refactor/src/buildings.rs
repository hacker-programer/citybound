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

        // =========================================================================
        // 5. RESIDUOS Y SANEAMIENTO
        // =========================================================================

        // 24: Vertedero Municipal
        templates.push(BuildingTemplate {
            id: 24,
            name: "Vertedero Municipal",
            description: "Entierra basura. Si no tiene geomembrana, lixiviados envenenan napas. Genera metano."
     ... [Truncated por el sistema tras 3 iteraciones para ahorrar contexto]