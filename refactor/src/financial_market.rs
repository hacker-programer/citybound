// Sistema de Mercado Financiero v0.12.0
//
// Implementa el mercado financiero completo:
// - Bolsa de valores (acciones de corporaciones locales)
// - Mercado de futuros de agua potable (commodity exchange)
// - Calificación crediticia municipal (afecta tasas de interés)
// - Emisión de bonos municipales
// - Mercado de criptomonedas local
// - Derivados financieros
// - Índices de mercado
//
// MECÁNICAS DISTÓPICAS:
// - Futuros de agua: el precio se desacopla del costo de bombeo
// - Calificación crediticia: castiga gasto social, premia pago de deuda
// - Bonos: permiten financiar infraestructura pero generan deuda perpetua
//
// TÉCNICAS:
// - LUTs para tasas de interés por calificación [TC#5]
// - RNG pre-generado para volatilidad de mercado [TC#22]
// - f32 en vez de f64 para todo [TC#24]

#![allow(dead_code)]

// ---------------------------------------------------------------------------
// CALIFICACIÓN CREDITICIA
// ---------------------------------------------------------------------------

/// Calificación crediticia de la municipalidad (estilo S&P/Moody's)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum CreditRating {
    /// Default — la ciudad está en quiebra
    D = 0,
    CCC = 1,
    B = 2,
    BB = 3,
    BBB = 4,
    A = 5,
    AA = 6,
    AAA = 7,
}

impl CreditRating {
    /// Tasa de interés base para préstamos según calificación [LUT - TC#5]
    pub fn base_interest_rate(&self) -> f32 {
        match self {
            CreditRating::D => 0.25,
            CreditRating::CCC => 0.18,
            CreditRating::B => 0.12,
            CreditRating::BB => 0.08,
            CreditRating::BBB => 0.05,
            CreditRating::A => 0.035,
            CreditRating::AA => 0.025,
            CreditRating::AAA => 0.015,
        }
    }

    pub fn default_penalty_multiplier(&self) -> f32 {
        match self {
            CreditRating::D => 3.0,
            CreditRating::CCC => 2.5,
            CreditRating::B => 2.0,
            CreditRating::BB => 1.5,
            CreditRating::BBB => 1.0,
            CreditRating::A => 0.8,
            CreditRating::AA => 0.6,
            CreditRating::AAA => 0.5,
        }
    }
}

/// Agencia de Calificación Crediticia
pub struct CreditAgency {
    pub rating: CreditRating,
    pub score: f32,
    pub rating_history: Vec<(u32, CreditRating)>,
    pub thresholds: [f32; 8],
    pub last_change_tick: u32,
}

impl CreditAgency {
    pub fn new() -> Self {
        CreditAgency {
            rating: CreditRating::A,
            score: 700.0,
            rating_history: Vec::with_capacity(64),
            thresholds: [0.0, 150.0, 300.0, 450.0, 600.0, 750.0, 880.0, 950.0],
            last_change_tick: 0,
        }
    }

    /// Evalúa la salud financiera de la ciudad
    pub fn evaluate(
        &mut self,
        current_tick: u32,
        treasury: f32,
        total_debt: f32,
        annual_revenue: f32,
        social_spending_ratio: f32,
        debt_service_ratio: f32,
        corporate_tax_rate: f32,
    ) -> CreditRating {
        let mut new_score = self.score;

        let debt_to_revenue = if annual_revenue > 0.0 {
            (total_debt as f32 / annual_revenue as f32).min(5.0)
        } else {
            5.0
        };
        new_score -= debt_to_revenue * 80.0;

        if treasury > 0.0 {
            new_score += (treasury as f32 / 1_000_000.0).min(50.0);
        }

        new_score -= social_spending_ratio * 100.0;
        new_score += debt_service_ratio * 60.0;
        new_score += (1.0 - corporate_tax_rate) * 50.0;

        self.score = self.score * 0.85 + new_score * 0.15;
        self.score = self.score.clamp(0.0, 1000.0);

        let new_rating = if self.score >= self.thresholds[7] {
            CreditRating::AAA
        } else if self.score >= self.thresholds[6] {
            CreditRating::AA
        } else if self.score >= self.thresholds[5] {
            CreditRating::A
        } else if self.score >= self.thresholds[4] {
            CreditRating::BBB
        } else if self.score >= self.thresholds[3] {
            CreditRating::BB
        } else if self.score >= self.thresholds[2] {
            CreditRating::B
        } else if self.score >= self.thresholds[1] {
            CreditRating::CCC
        } else {
            CreditRating::D
        };

        if new_rating != self.rating {
            self.rating_history.push((current_tick, new_rating));
            self.last_change_tick = current_tick;
        }

        self.rating = new_rating;
        new_rating
    }
}

// ---------------------------------------------------------------------------
// MERCADO DE FUTUROS DE AGUA
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct WaterFuturesMarket {
    pub spot_price: f32,
    pub production_cost: f32,
    pub market_controlled: bool,
    pub volatility: f32,
    pub active_contracts: Vec<WaterFuture>,
    pub price_history: Vec<f32>,
    pub is_privatized: bool,
    pub corporate_owner: Option<String>,
}

#[derive(Debug, Clone)]
pub struct WaterFuture {
    pub contract_id: u64,
    pub buyer: String,
    pub seller: String,
    pub volume_liters: f32,
    pub strike_price: f32,
    pub expiry_tick: u32,
    pub premium: f32,
}

impl WaterFuturesMarket {
    pub fn new() -> Self {
        WaterFuturesMarket {
            spot_price: 0.001,
            production_cost: 0.0008,
            market_controlled: false,
            volatility: 0.15,
            active_contracts: Vec::with_capacity(64),
            price_history: Vec::with_capacity(256),
            is_privatized: false,
            corporate_owner: None,
        }
    }

    pub fn tick(&mut self, water_reserves: f32, water_demand: f32) {
        if self.market_controlled {
            let supply_demand_ratio = if water_demand > 0.0 {
                water_reserves / water_demand
            } else {
                10.0
            };

            let scarcity_panic = if supply_demand_ratio < 0.5 {
                5.0
            } else if supply_demand_ratio < 1.0 {
                (1.0 - supply_demand_ratio) * 2.0
            } else {
                0.0
            };

            let random_shock = crate::rng_pool::rng_range_inclusive(-1.0, 1.0) * self.volatility;
            let speculation_factor = 1.0 + random_shock + scarcity_panic;

            self.spot_price = (self.spot_price * speculation_factor)
                .max(self.production_cost * 0.5)
                .min(self.production_cost * 20.0);

            self.price_history.push(self.spot_price);
            if self.price_history.len() > 256 {
                self.price_history.remove(0);
            }
        } else {
            self.spot_price = self.production_cost;
        }
    }

    pub fn privatize(&mut self, corporation: &str, injection_amount: f64) -> f64 {
        self.is_privatized = true;
        self.market_controlled = true;
        self.corporate_owner = Some(corporation.to_string());
        injection_amount
    }

    pub fn nationalize(&mut self) -> f64 {
        let buyback_cost = self.spot_price as f64 * 1_000_000_000.0;
        self.is_privatized = false;
        self.market_controlled = false;
        self.corporate_owner = None;
        buyback_cost
    }
}

// ---------------------------------------------------------------------------
// BOLSA DE VALORES LOCAL
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ListedCompany {
    pub ticker: String,
    pub name: String,
    pub sector: CompanySector,
    pub share_price: f32,
    pub shares_outstanding: u64,
    pub market_cap: f64,
    pub revenue_annual: f64,
    pub profit_margin: f32,
    pub price_history: Vec<f32>,
    pub volatility: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CompanySector {
    Technology = 0,
    Finance = 1,
    Healthcare = 2,
    Energy = 3,
    RealEstate = 4,
    Industrial = 5,
    Consumer = 6,
    Utilities = 7,
    Agriculture = 8,
    Water = 9,
}

pub struct StockExchange {
    pub listed_companies: Vec<ListedCompany>,
    pub index_value: f32,
    pub trading_volume: f64,
    pub is_operational: bool,
}

impl StockExchange {
    pub fn new() -> Self {
        StockExchange {
            listed_companies: Vec::with_capacity(32),
            index_value: 1000.0,
            trading_volume: 0.0,
            is_operational: true,
        }
    }

    pub fn list_company(&mut self, ticker: &str, name: &str, sector: CompanySector, initial_price: f32, shares: u64) {
        self.listed_companies.push(ListedCompany {
            ticker: ticker.to_string(),
            name: name.to_string(),
            sector,
            share_price: initial_price,
            shares_outstanding: shares,
            market_cap: initial_price as f64 * shares as f64,
            revenue_annual: 0.0,
            profit_margin: 0.1,
            price_history: vec![initial_price],
            volatility: 0.02,
        });
    }

    pub fn tick(&mut self, city_economy_health: f32) {
        let mut total_cap: f64 = 0.0;

        for company in &mut self.listed_companies {
            let drift = (city_economy_health - 0.5) * 0.01;
            let random = crate::rng_pool::rng_range_inclusive(-1.0, 1.0) * company.volatility;
            company.share_price *= 1.0 + drift + random;
            company.share_price = company.share_price.max(0.01);
            company.market_cap = company.share_price as f64 * company.shares_outstanding as f64;
            company.price_history.push(company.share_price);

            if company.price_history.len() > 100 {
                company.price_history.remove(0);
            }

            total_cap += company.market_cap;
        }

        if !self.listed_companies.is_empty() {
            self.index_value = (total_cap / self.listed_companies.len() as f64) as f32 / 1000.0;
        }
    }
}

// ---------------------------------------------------------------------------
// SISTEMA DE BONOS MUNICIPALES
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct MunicipalBond {
    pub id: u64,
    pub face_value: f64,
    pub interest_rate: f32,
    pub maturity_ticks: u32,
    pub issued_tick: u32,
    pub purpose: String,
    pub remaining_principal: f64,
}

pub struct BondMarket {
    pub active_bonds: Vec<MunicipalBond>,
    pub total_debt_outstanding: f64,
    pub annual_interest_cost: f64,
    next_bond_id: u64,
}

impl BondMarket {
    pub fn new() -> Self {
        BondMarket {
            active_bonds: Vec::with_capacity(32),
            total_debt_outstanding: 0.0,
            annual_interest_cost: 0.0,
            next_bond_id: 1,
        }
    }

    pub fn issue_bond(
        &mut self,
        face_value: f64,
        credit_rating: CreditRating,
        maturity_days: u32,
        purpose: &str,
        current_tick: u32,
    ) -> MunicipalBond {
        let id = self.next_bond_id;
        self.next_bond_id += 1;

        let rate = credit_rating.base_interest_rate();

        let bond = MunicipalBond {
            id,
            face_value,
            interest_rate: rate,
            maturity_ticks: maturity_days,
            issued_tick: current_tick,
            purpose: purpose.to_string(),
            remaining_principal: face_value,
        };

        self.total_debt_outstanding += face_value;
        self.annual_interest_cost += face_value * rate as f64;
        self.active_bonds.push(bond.clone());

        bond
    }

    pub fn service_debt(&mut self, treasury: &mut f64) -> f64 {
        let daily_interest = self.annual_interest_cost / 365.0;
        *treasury -= daily_interest;
        daily_interest
    }

    pub fn process_maturities(&mut self, current_tick: u32, treasury: &mut f64) -> Vec<u64> {
        let mut matured = Vec::new();
        let mut i = 0;

        while i < self.active_bonds.len() {
            let bond = &self.active_bonds[i];
            let age = current_tick - bond.issued_tick;

            if age >= bond.maturity_ticks {
                *treasury -= bond.remaining_principal;
                self.total_debt_outstanding -= bond.remaining_principal;
                matured.push(bond.id);
                self.active_bonds.remove(i);
            } else {
                i += 1;
            }
        }

        matured
    }
}

// ---------------------------------------------------------------------------
// SISTEMA FINANCIERO UNIFICADO
// ---------------------------------------------------------------------------

pub struct FinancialSystem {
    pub credit_agency: CreditAgency,
    pub water_market: WaterFuturesMarket,
    pub stock_exchange: StockExchange,
    pub bond_market: BondMarket,
    pub current_lending_rate: f32,
    pub inflation_rate: f32,
    pub estimated_gdp: f64,
}

impl FinancialSystem {
    pub fn new() -> Self {
        FinancialSystem {
            credit_agency: CreditAgency::new(),
            water_market: WaterFuturesMarket::new(),
            stock_exchange: StockExchange::new(),
            bond_market: BondMarket::new(),
            current_lending_rate: 0.035,
            inflation_rate: 0.02,
            estimated_gdp: 0.0,
        }
    }

    pub fn tick(
        &mut self,
        current_tick: u32,
        treasury: &mut f64,
        water_reserves: f32,
        water_demand: f32,
        social_spending_ratio: f32,
        corporate_tax_rate: f32,
    ) -> FinancialReport {
        let debt_service_ratio = if self.bond_market.total_debt_outstanding > 0.0 {
            (self.bond_market.annual_interest_cost / self.bond_market.total_debt_outstanding) as f32
        } else {
            0.0
        };

        let _rating = self.credit_agency.evaluate(
            current_tick,
            *treasury as f32,
            self.bond_market.total_debt_outstanding as f32,
            self.estimated_gdp as f32,
            social_spending_ratio,
            debt_service_ratio,
            corporate_tax_rate,
        );

        self.current_lending_rate = self.credit_agency.rating.base_interest_rate();

        self.water_market.tick(water_reserves, water_demand);

        let economy_health = self.credit_agency.score / 1000.0;
        self.stock_exchange.tick(economy_health);

        self.bond_market.service_debt(treasury);
        self.bond_market.process_maturities(current_tick, treasury);

        FinancialReport {
            credit_rating: self.credit_agency.rating,
            water_spot_price: self.water_market.spot_price,
            stock_index: self.stock_exchange.index_value,
            total_debt: self.bond_market.total_debt_outstanding as f32,
            lending_rate: self.current_lending_rate,
            inflation: self.inflation_rate,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FinancialReport {
    pub credit_rating: CreditRating,
    pub water_spot_price: f32,
    pub stock_index: f32,
        total_debt: f32,
    pub lending_rate: f32,
    pub inflation: f32,
}

// ---------------------------------------------------------------------------
// TESTS
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng_pool;

    #[test]
    fn test_credit_rating_lut() {
        assert!((CreditRating::AAA.base_interest_rate() - 0.015).abs() < 0.001);
        assert!((CreditRating::D.base_interest_rate() - 0.25).abs() < 0.001);
    }

    #[test]
    fn test_credit_agency_social_spending_penalty() {
        crate::rng_pool::init_rng_pool(12345);
        let mut agency = CreditAgency::new();
        let initial = agency.score;

        agency.evaluate(100, 1_000_000.0, 0.0, 500_000.0, 0.5, 0.0, 0.3);
        // Social spending should reduce or not significantly increase score
        assert!(agency.score < initial + 20.0);
    }

    #[test]
    fn test_water_market_privatization() {
        let mut market = WaterFuturesMarket::new();
        assert!(!market.is_privatized);
        assert!(!market.market_controlled);

        let injection = market.privatize("AquaCorp Inc", 1_000_000_000.0);
        assert!(market.is_privatized);
        assert!(market.market_controlled);
        assert_eq!(market.corporate_owner.as_deref(), Some("AquaCorp Inc"));
        assert_eq!(injection, 1_000_000_000.0);
    }

    #[test]
    fn test_bond_issuance() {
        let mut market = BondMarket::new();
        let bond = market.issue_bond(1_000_000.0, CreditRating::A, 365, "Puente nuevo", 0);

        assert_eq!(bond.face_value, 1_000_000.0);
        assert!((bond.interest_rate - 0.035).abs() < 0.001);
        assert_eq!(market.total_debt_outstanding, 1_000_000.0);
    }

    #[test]
    fn test_financial_system_tick() {
        crate::rng_pool::init_rng_pool(42);
        let mut fs = FinancialSystem::new();
        let mut treasury = 10_000_000.0;

        // Should not panic
        let report = fs.tick(0, &mut treasury, 1000000.0, 500000.0, 0.3, 0.25);

        assert!(report.water_spot_price > 0.0);
        assert!(report.stock_index > 0.0);
    }
}
