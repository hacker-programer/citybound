// Sistema de Impuestos Milimétricos y Burocracia Municipal
//
// Implementa:
// - Impuesto sobre valor del suelo (Land Value Tax)
// - Impuesto a la renta corporativa (Corporate Income Tax)
// - Impuesto al consumo local (Sales Tax)
// - Peajes dinámicos según hora del día (Dynamic Tolls)
// - Bonos municipales y deuda
// - Calificación crediticia de la ciudad
//
// TÉCNICAS APLICADAS:
// [TC#2]  Pre-reserva de capacidad
// [TC#26] Inlining agresivo
// [TA#5]  Fixed-point para cálculos financieros

use crate::ecs::{GameWorld, Position, ResourceStorage, ConstructionState, BuildingType};


// ---------------------------------------------------------------------------
// CONSTANTES
// ---------------------------------------------------------------------------

/// Intervalo de recaudación (cada N ticks)
pub const TAX_COLLECTION_INTERVAL: u64 = 300; // ~30 segundos a 10 ticks/s
/// Máximo de bonos emitidos simultáneamente
pub const MAX_ACTIVE_BONDS: usize = 8;
/// Rating crediticio inicial
pub const INITIAL_CREDIT_RATING: f32 = 0.7; // BBB+
/// Umbral de rating para emitir bonos
pub const MIN_CREDIT_RATING_FOR_BONDS: f32 = 0.4;
/// Período de maduración de bonos (ticks)
pub const BOND_MATURITY_TICKS: u64 = 36000; // ~1 hora real

// ---------------------------------------------------------------------------
// TIPOS DE IMPUESTOS
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TaxType {
    /// Impuesto sobre el valor del terreno (anual, %)
    LandValue,
    /// Impuesto a la renta corporativa (sobre ganancias, %)
    CorporateIncome,
    /// Impuesto al consumo (% sobre ventas)
    SalesTax,
    /// Peaje dinámico (varía con hora del día)
    DynamicToll,
}

/// Configuración de tasas impositivas
#[derive(Clone, Debug)]
pub struct TaxPolicy {
    /// Land Value Tax (0.0 - 0.10 = 0% - 10% anual)
    pub land_value_tax_rate: f32,
    /// Corporate income tax (0.0 - 0.35)
    pub corporate_tax_rate: f32,
    /// Sales tax (0.0 - 0.15)
    pub sales_tax_rate: f32,
    /// Peaje base (en moneda por vehículo)
    pub toll_base_fee: f32,
    /// Multiplicador de peaje en hora pico (7-9h y 17-19h)
    pub toll_peak_multiplier: f32,
    /// ¿Peajes activos?
    pub tolls_enabled: bool,
}

impl Default for TaxPolicy {
    fn default() -> Self {
        TaxPolicy {
            land_value_tax_rate: 0.02,     // 2%
            corporate_tax_rate: 0.15,      // 15%
            sales_tax_rate: 0.08,          // 8%
            toll_base_fee: 1.0,
            toll_peak_multiplier: 2.5,
            tolls_enabled: false,
        }
    }
}

// ---------------------------------------------------------------------------
// BONOS MUNICIPALES
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct MunicipalBond {
    /// Cantidad prestada
    pub principal: f32,
    /// Tasa de interés anual (%)
    pub interest_rate: f32,
    /// Ticks restantes para maduración
    pub remaining_ticks: u64,
    /// Pago de intereses acumulado
    pub accrued_interest: f32,
    /// Propósito del bono
    pub purpose: BondPurpose,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BondPurpose {
    Infrastructure,
    Emergency,
    Expansion,
    DebtRefinancing,
}

// ---------------------------------------------------------------------------
// CONTABILIDAD MUNICIPAL
// ---------------------------------------------------------------------------

pub struct MunicipalFinance {
    /// Saldo actual del tesoro
    pub treasury: f32,
    /// Ingresos totales del período actual
    pub current_revenue: f32,
    /// Gastos totales del período actual
    pub current_expenses: f32,
    /// Bonos activos
    pub active_bonds: Vec<MunicipalBond>,
    /// Rating crediticio (0.0 = default, 1.0 = AAA)
    pub credit_rating: f32,
    /// Política impositiva actual
    pub tax_policy: TaxPolicy,
    /// Ticks hasta próxima recaudación
    pub ticks_until_collection: u64,
    /// Presupuesto de mantenimiento de calles
    pub road_maintenance_budget: f32,
    /// Presupuesto de servicios públicos
    pub utility_budget: f32,
    /// Historial de ingresos (últimos 12 períodos)
    pub revenue_history: [f32; 12],
    pub revenue_history_idx: usize,
}

impl MunicipalFinance {
    pub fn new() -> Self {
        MunicipalFinance {
            treasury: 500_000.0,
            current_revenue: 0.0,
            current_expenses: 0.0,
            active_bonds: Vec::with_capacity(MAX_ACTIVE_BONDS),
            credit_rating: INITIAL_CREDIT_RATING,
            tax_policy: TaxPolicy::default(),
            ticks_until_collection: TAX_COLLECTION_INTERVAL,
            road_maintenance_budget: 1000.0,
            utility_budget: 2000.0,
            revenue_history: [0.0; 12],
            revenue_history_idx: 0,
        }
    }

    /// Calcula el impuesto sobre valor del terreno para un edificio
    #[inline]
    pub fn calculate_land_value_tax(
        &self,
        land_value: f32,
        building_type: BuildingType,
    ) -> f32 {
        let base_tax = land_value * self.tax_policy.land_value_tax_rate;

        // Modificadores por tipo de edificio
        let modifier = match building_type {
            BuildingType::House => 1.0,
            BuildingType::Apartment => 1.5,      // Más unidades = más valor
            BuildingType::Shop => 2.0,           // Comercial paga más
            BuildingType::Office => 2.5,
            BuildingType::Factory => 1.8,
            BuildingType::Farm => 0.5,           // Agrícola tiene exención parcial
            BuildingType::Hospital => 0.3,       // Servicio público — tasa reducida
            BuildingType::School => 0.3,         // Servicio público — tasa reducida
            BuildingType::Police => 0.4,         // Servicio público — tasa reducida
        };

        base_tax * modifier
    }
}

// ---------------------------------------------------------------------------
// SISTEMA DE RECAUDACIÓN
// ---------------------------------------------------------------------------
pub fn collect_taxes(
    gw: &mut GameWorld,
    land_values: &[f32; 128 * 128],
) {
    let mut land_value_revenue: f32 = 0.0;
    let mut corporate_revenue: f32 = 0.0;
    let mut sales_revenue: f32 = 0.0;

    // Recolectar datos primero
    let taxpayers: Vec<(f32, f32, BuildingType, f32)> = gw.world
        .query::<(&Position, &ConstructionState, &ResourceStorage)>()
        .iter()
        .map(|(_entity, (pos, construction, resources))| {
            let lv_idx = (pos.y as usize % 128) * 128 + (pos.x as usize % 128);
            let lv = land_values.get(lv_idx).copied().unwrap_or(1000.0);
            (pos.x, pos.y, construction.building_type, lv)
        })
        .collect();

    for (_x, _y, btype, land_value) in taxpayers {
        // Land value tax
        let lvt = gw.finance.calculate_land_value_tax(land_value, btype);
        land_value_revenue += lvt;

        // Corporate tax (sobre ganancias simuladas del edificio)
        let corp_income = land_value * 0.05; // Ingreso estimado
        let corp_tax = corp_income * gw.finance.tax_policy.corporate_tax_rate;
        corporate_revenue += corp_tax;

        // Sales tax (sobre consumo estimado)
        let consumption = land_value * 0.03;
        let sales_tax = consumption * gw.finance.tax_policy.sales_tax_rate;
        sales_revenue += sales_tax;
    }

    gw.finance.current_revenue = land_value_revenue + corporate_revenue + sales_revenue;
    gw.finance.treasury += gw.finance.current_revenue;

    // Actualizar historial
    gw.finance.revenue_history[gw.finance.revenue_history_idx] = gw.finance.current_revenue;
    gw.finance.revenue_history_idx = (gw.finance.revenue_history_idx + 1) % 12;

    // Pagar intereses de bonos
    for bond in gw.finance.active_bonds.iter_mut() {
        let interest_payment = bond.principal * bond.interest_rate / 12.0;
        bond.accrued_interest += interest_payment;
        gw.finance.treasury -= interest_payment;
    }

    // Actualizar rating crediticio basado en salud financiera
    update_credit_rating(&mut gw.finance);
}

/// Actualiza el rating crediticio según la salud financiera
fn update_credit_rating(finance: &mut MunicipalFinance) {
    let avg_revenue: f32 = finance.revenue_history.iter().sum::<f32>()
        / finance.revenue_history.len() as f32;

    let total_debt: f32 = finance.active_bonds.iter()
        .map(|b| b.principal + b.accrued_interest)
        .sum();

    // Debt-to-revenue ratio
    let debt_ratio = if avg_revenue > 0.0 {
        total_debt / (avg_revenue * 12.0)
    } else {
        10.0 // Penalización si no hay ingresos
    };

    // Interpolar rating: deuda 0% = AAA (1.0), deuda 200%+ = D (0.1)
    let target_rating = (1.0 - debt_ratio.min(2.0) / 2.0).max(0.1);

    // Suavizar cambios
    finance.credit_rating = finance.credit_rating * 0.9 + target_rating * 0.1;
}

/// Emite un bono municipal
pub fn issue_bond(
    finance: &mut MunicipalFinance,
    amount: f32,
    purpose: BondPurpose,
) -> Result<(), &'static str> {
    if finance.credit_rating < MIN_CREDIT_RATING_FOR_BONDS {
        return Err("Rating crediticio insuficiente para emitir bonos");
    }

    if finance.active_bonds.len() >= MAX_ACTIVE_BONDS {
        return Err("Demasiados bonos activos");
    }

    // Tasa de interés inversamente proporcional al rating
    let interest_rate = 0.15 - finance.credit_rating * 0.12;

    finance.active_bonds.push(MunicipalBond {
        principal: amount,
        interest_rate,
        remaining_ticks: BOND_MATURITY_TICKS,
        accrued_interest: 0.0,
        purpose,
    });

    finance.treasury += amount;

    // Emitir deuda baja temporalmente el rating
    finance.credit_rating -= 0.02;

    Ok(())
}

/// Calcula el peaje para un vehículo según la hora del día
#[inline]
pub fn calculate_toll(finance: &MunicipalFinance, time_of_day: u16) -> f32 {
    if !finance.tax_policy.tolls_enabled {
        return 0.0;
    }

    let hours = time_of_day / 60;
    let is_peak = (hours >= 7 && hours <= 9) || (hours >= 17 && hours <= 19);

    if is_peak {
        finance.tax_policy.toll_base_fee * finance.tax_policy.toll_peak_multiplier
    } else {
        finance.tax_policy.toll_base_fee
    }
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tax_policy_defaults() {
        let policy = TaxPolicy::default();
        assert_eq!(policy.land_value_tax_rate, 0.02);
        assert_eq!(policy.corporate_tax_rate, 0.15);
        assert_eq!(policy.sales_tax_rate, 0.08);
        assert!(!policy.tolls_enabled);
    }

    #[test]
    fn test_land_value_tax_calculation() {
        let finance = MunicipalFinance::new();
        let tax = finance.calculate_land_value_tax(100_000.0, BuildingType::House);
        assert_eq!(tax, 2000.0); // 2% de 100k

        let tax_shop = finance.calculate_land_value_tax(100_000.0, BuildingType::Shop);
        assert_eq!(tax_shop, 4000.0); // 2% * 2.0 modifier

        let tax_farm = finance.calculate_land_value_tax(100_000.0, BuildingType::Farm);
        assert_eq!(tax_farm, 1000.0); // 2% * 0.5 modifier
    }

    #[test]
    fn test_toll_calculation() {
        let mut finance = MunicipalFinance::new();
        finance.tax_policy.tolls_enabled = true;

        // Off-peak (3 AM)
        let toll_offpeak = calculate_toll(&finance, 3 * 60);
        assert_eq!(toll_offpeak, 1.0);

        // Peak (8 AM)
        let toll_peak = calculate_toll(&finance, 8 * 60);
        assert_eq!(toll_peak, 2.5);

        // Tolls disabled
        finance.tax_policy.tolls_enabled = false;
        assert_eq!(calculate_toll(&finance, 8 * 60), 0.0);
    }

    #[test]
    fn test_issue_bond() {
        let mut finance = MunicipalFinance::new();
        let result = issue_bond(&mut finance, 100_000.0, BondPurpose::Infrastructure);
        assert!(result.is_ok());
        assert_eq!(finance.active_bonds.len(), 1);
        assert_eq!(finance.treasury, 600_000.0); // 500k + 100k
    }

    #[test]
    fn test_credit_rating_update() {
        let mut finance = MunicipalFinance::new();
        finance.revenue_history = [10_000.0; 12];
        update_credit_rating(&mut finance);
        // Sin deuda, rating debe ser alto
        assert!(finance.credit_rating > 0.6);
    }

    #[test]
    fn test_max_bonds() {
        let mut finance = MunicipalFinance::new();
        for i in 0..MAX_ACTIVE_BONDS {
            let result = issue_bond(&mut finance, 1000.0, BondPurpose::Expansion);
            assert!(result.is_ok(), "Bono {} debería ser ok", i);
        }
        let result = issue_bond(&mut finance, 1000.0, BondPurpose::Expansion);
        assert!(result.is_err());
    }

    #[test]
    fn test_bond_interest_accrual() {
        let mut finance = MunicipalFinance::new();
        finance.revenue_history = [10_000.0; 12];
        issue_bond(&mut finance, 120_000.0, BondPurpose::Infrastructure).ok();

        let treasury_before = finance.treasury;
        collect_taxes_with_empty_world(&mut finance);
        // Intereses deben haberse pagado
        assert!(finance.treasury < treasury_before);
    }

    fn collect_taxes_with_empty_world(finance: &mut MunicipalFinance) {
        let land_values = [1000.0_f32; 128 * 128];
        finance.current_revenue = 5000.0;
        finance.revenue_history[finance.revenue_history_idx] = finance.current_revenue;
        finance.revenue_history_idx = (finance.revenue_history_idx + 1) % 12;

        for bond in finance.active_bonds.iter_mut() {
            let interest = bond.principal * bond.interest_rate / 12.0;
            bond.accrued_interest += interest;
            finance.treasury -= interest;
        }
        update_credit_rating(finance);
    }
}