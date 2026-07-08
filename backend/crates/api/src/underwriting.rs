//! **Investment underwriting** (roadmap Phase 7, issue #41).
//!
//! Pure, deterministic real-estate deal math: given a set of [`Assumptions`]
//! (purchase / rehab / rent / financing / multi-year projection knobs), compute
//! the metrics every investor underwrites on — **cap rate**, **cash-on-cash
//! return**, **DSCR**, and a multi-year **IRR** — plus a rent-growth
//! **sensitivity** band so the operator can see the IRR range rather than a
//! single point estimate.
//!
//! No I/O, no database — the [`Deal`](entity::deal::Model) route layer feeds a
//! deal's stored assumptions in and formats the [`Underwriting`] out. Money is
//! integer cents; rates are basis points (`bps`, 1% = 100 bps). Ratios that are
//! not money (cap rate, DSCR, IRR) are returned in bps / as `f64` so the edge
//! can format them without re-deriving.

/// Underwriting inputs. Every field has a sensible default so a half-filled
/// deal still underwrites; [`Assumptions::purchase_price_cents`] falls back
/// through offer → asking at the call site.
#[derive(Clone, Copy, Debug)]
pub struct Assumptions {
    pub purchase_price_cents: i64,
    pub rehab_cents: i64,
    pub closing_costs_cents: i64,
    /// After-repair value; used for the exit basis when no exit cap is given.
    pub arv_cents: i64,
    pub monthly_rent_cents: i64,
    pub monthly_expenses_cents: i64,
    pub vacancy_bps: i32,
    pub down_payment_bps: i32,
    pub interest_rate_bps: i32,
    pub loan_term_years: i32,
    pub rent_growth_bps: i32,
    pub appreciation_bps: i32,
    pub exit_cap_rate_bps: i32,
    pub selling_costs_bps: i32,
    pub hold_years: i32,
}

impl Default for Assumptions {
    fn default() -> Self {
        // Neutral, opinion-light defaults for the knobs an investor rarely fills
        // in on a first pass. Prices/rents default to 0 (nothing to underwrite).
        Assumptions {
            purchase_price_cents: 0,
            rehab_cents: 0,
            closing_costs_cents: 0,
            arv_cents: 0,
            monthly_rent_cents: 0,
            monthly_expenses_cents: 0,
            vacancy_bps: 500,       // 5% vacancy
            down_payment_bps: 2000, // 20% down
            interest_rate_bps: 700, // 7.00% APR
            loan_term_years: 30,
            rent_growth_bps: 300,   // 3% / yr
            appreciation_bps: 300,  // 3% / yr
            exit_cap_rate_bps: 0,   // 0 => value off appreciation, not a cap
            selling_costs_bps: 700, // 7% cost of sale
            hold_years: 5,
        }
    }
}

/// One point on the rent-growth sensitivity band.
#[derive(Clone, Copy, Debug)]
pub struct SensitivityPoint {
    pub rent_growth_bps: i32,
    /// IRR at this rent-growth assumption, or `None` if it doesn't converge.
    pub irr_bps: Option<i32>,
}

/// Computed underwriting output.
#[derive(Clone, Debug)]
pub struct Underwriting {
    pub purchase_price_cents: i64,
    /// All-in cost basis: purchase + rehab + closing.
    pub total_project_cost_cents: i64,
    pub loan_amount_cents: i64,
    pub down_payment_cents: i64,
    /// Cash out of pocket: down payment + closing + rehab.
    pub total_cash_invested_cents: i64,
    pub monthly_debt_service_cents: i64,
    pub annual_debt_service_cents: i64,
    pub gross_rent_annual_cents: i64,
    pub vacancy_loss_cents: i64,
    pub effective_gross_income_cents: i64,
    pub operating_expenses_annual_cents: i64,
    pub noi_annual_cents: i64,
    pub annual_cash_flow_cents: i64,
    /// Going-in cap rate on the all-in cost basis (bps).
    pub cap_rate_bps: i32,
    /// Cash-on-cash return (bps of cash invested).
    pub cash_on_cash_bps: i32,
    /// Debt-service coverage ratio (NOI ÷ debt service); `f64`, e.g. `1.25`.
    pub dscr: f64,
    /// Projected value at sale (year `hold_years`).
    pub exit_value_cents: i64,
    /// Loan principal still outstanding at sale.
    pub loan_balance_at_exit_cents: i64,
    /// Net sale proceeds = exit value − selling costs − loan payoff.
    pub net_sale_proceeds_cents: i64,
    /// Levered IRR over the hold, or `None` if it doesn't converge.
    pub irr_bps: Option<i32>,
    /// Total profit over the hold: Σ cash flow + net sale proceeds − cash in.
    pub total_profit_cents: i64,
    /// Rent-growth sensitivity band (−2pp, −1pp, base, +1pp, +2pp).
    pub sensitivity: Vec<SensitivityPoint>,
}

/// `x * bps / 10_000`, rounded to nearest cent, computed in `i128` to avoid
/// overflow on large portfolios.
fn apply_bps(cents: i64, bps: i32) -> i64 {
    let prod = cents as i128 * bps as i128;
    // Round half away from zero.
    let rounded = if prod >= 0 {
        (prod + 5_000) / 10_000
    } else {
        (prod - 5_000) / 10_000
    };
    rounded as i64
}

/// Fixed-rate fully-amortising monthly payment for `principal` at `annual_bps`
/// over `term_years`. Zero-interest amortises linearly; a zero term/principal
/// yields a zero payment.
fn monthly_payment_cents(principal_cents: i64, annual_bps: i32, term_years: i32) -> i64 {
    if principal_cents <= 0 || term_years <= 0 {
        return 0;
    }
    let n = (term_years * 12) as f64;
    let r = annual_bps as f64 / 10_000.0 / 12.0;
    let p = principal_cents as f64;
    let pay = if r.abs() < 1e-12 {
        p / n
    } else {
        p * r / (1.0 - (1.0 + r).powf(-n))
    };
    pay.round() as i64
}

/// Remaining loan principal after `elapsed_years` of amortisation.
fn loan_balance_after(
    principal_cents: i64,
    annual_bps: i32,
    term_years: i32,
    elapsed_years: i32,
) -> i64 {
    if principal_cents <= 0 || term_years <= 0 || elapsed_years <= 0 {
        return principal_cents.max(0);
    }
    if elapsed_years >= term_years {
        return 0;
    }
    let r = annual_bps as f64 / 10_000.0 / 12.0;
    let pay = monthly_payment_cents(principal_cents, annual_bps, term_years) as f64;
    let mut bal = principal_cents as f64;
    for _ in 0..(elapsed_years * 12) {
        let interest = bal * r;
        bal = (bal + interest - pay).max(0.0);
    }
    bal.round() as i64
}

/// Internal rate of return of a cash-flow stream (index = period, in years),
/// via bisection on the discount rate. Returns `None` when there's no sign
/// change in NPV over the search window (e.g. an all-negative stream).
fn irr_bps(cash_flows: &[f64]) -> Option<i32> {
    let npv = |rate: f64| -> f64 {
        cash_flows
            .iter()
            .enumerate()
            .map(|(t, cf)| cf / (1.0 + rate).powi(t as i32))
            .sum()
    };
    let (mut lo, mut hi) = (-0.9999_f64, 10.0_f64);
    let (mut f_lo, f_hi) = (npv(lo), npv(hi));
    if f_lo.is_nan() || f_hi.is_nan() || f_lo * f_hi > 0.0 {
        return None;
    }
    for _ in 0..200 {
        let mid = (lo + hi) / 2.0;
        let f_mid = npv(mid);
        if f_mid.abs() < 1e-6 {
            return Some((mid * 10_000.0).round() as i32);
        }
        if f_lo * f_mid < 0.0 {
            hi = mid;
        } else {
            lo = mid;
            f_lo = f_mid;
        }
    }
    Some(((lo + hi) / 2.0 * 10_000.0).round() as i32)
}

/// The levered annual cash flows plus the terminal sale, as `f64` cents, for a
/// given rent-growth assumption. Year 0 is the cash invested (negative); years
/// `1..=hold` are operating cash flow, with the sale added in the final year.
fn cash_flow_stream(a: &Assumptions, base: &Underwriting, rent_growth_bps: i32) -> Vec<f64> {
    let hold = a.hold_years.clamp(1, 40);
    let g = rent_growth_bps as f64 / 10_000.0;
    let mut stream = Vec::with_capacity(hold as usize + 1);
    stream.push(-(base.total_cash_invested_cents as f64));

    // NOI grows with rent growth (income and expenses both scale), debt service
    // is fixed. The exit value is priced off the year-after NOI and the exit
    // cap (or appreciation when no cap is set).
    for year in 1..=hold {
        let factor = (1.0 + g).powi(year - 1);
        let noi = base.noi_annual_cents as f64 * factor;
        let mut cf = noi - base.annual_debt_service_cents as f64;
        if year == hold {
            cf += base.net_sale_proceeds_cents as f64;
        }
        stream.push(cf);
    }
    stream
}

/// Run the full underwrite.
pub fn underwrite(a: &Assumptions) -> Underwriting {
    let purchase = a.purchase_price_cents.max(0);
    let rehab = a.rehab_cents.max(0);
    let closing = a.closing_costs_cents.max(0);
    let total_project_cost = purchase + rehab + closing;

    let down_payment = apply_bps(purchase, a.down_payment_bps.clamp(0, 10_000));
    let loan_amount = (purchase - down_payment).max(0);
    let total_cash_invested = down_payment + closing + rehab;

    let monthly_debt = monthly_payment_cents(loan_amount, a.interest_rate_bps, a.loan_term_years);
    let annual_debt = monthly_debt * 12;

    let gross_rent_annual = a.monthly_rent_cents.max(0) * 12;
    let vacancy_loss = apply_bps(gross_rent_annual, a.vacancy_bps.clamp(0, 10_000));
    let egi = gross_rent_annual - vacancy_loss;
    let opex_annual = a.monthly_expenses_cents.max(0) * 12;
    let noi = egi - opex_annual;
    let annual_cash_flow = noi - annual_debt;

    let cap_rate_bps = if total_project_cost > 0 {
        ((noi as i128 * 10_000) / total_project_cost as i128) as i32
    } else {
        0
    };
    let cash_on_cash_bps = if total_cash_invested > 0 {
        ((annual_cash_flow as i128 * 10_000) / total_cash_invested as i128) as i32
    } else {
        0
    };
    let dscr = if annual_debt > 0 {
        noi as f64 / annual_debt as f64
    } else {
        0.0
    };

    let hold = a.hold_years.clamp(1, 40);

    // Exit value: prefer the income approach (final-year NOI ÷ exit cap); fall
    // back to appreciating the ARV (or purchase) when no exit cap is given.
    let g = a.rent_growth_bps as f64 / 10_000.0;
    let noi_at_exit = noi as f64 * (1.0 + g).powi(hold);
    let exit_value = if a.exit_cap_rate_bps > 0 {
        (noi_at_exit / (a.exit_cap_rate_bps as f64 / 10_000.0)).round() as i64
    } else {
        let basis = if a.arv_cents > 0 {
            a.arv_cents
        } else {
            purchase
        } as f64;
        (basis * (1.0 + a.appreciation_bps as f64 / 10_000.0).powi(hold)).round() as i64
    };
    let exit_value = exit_value.max(0);

    let loan_balance_at_exit =
        loan_balance_after(loan_amount, a.interest_rate_bps, a.loan_term_years, hold);
    let selling_costs = apply_bps(exit_value, a.selling_costs_bps.clamp(0, 10_000));
    let net_sale_proceeds = exit_value - selling_costs - loan_balance_at_exit;

    // Build a provisional result so the cash-flow stream can reference the
    // computed operating figures, then fill in IRR + profit + sensitivity.
    let mut result = Underwriting {
        purchase_price_cents: purchase,
        total_project_cost_cents: total_project_cost,
        loan_amount_cents: loan_amount,
        down_payment_cents: down_payment,
        total_cash_invested_cents: total_cash_invested,
        monthly_debt_service_cents: monthly_debt,
        annual_debt_service_cents: annual_debt,
        gross_rent_annual_cents: gross_rent_annual,
        vacancy_loss_cents: vacancy_loss,
        effective_gross_income_cents: egi,
        operating_expenses_annual_cents: opex_annual,
        noi_annual_cents: noi,
        annual_cash_flow_cents: annual_cash_flow,
        cap_rate_bps,
        cash_on_cash_bps,
        dscr,
        exit_value_cents: exit_value,
        loan_balance_at_exit_cents: loan_balance_at_exit,
        net_sale_proceeds_cents: net_sale_proceeds,
        irr_bps: None,
        total_profit_cents: 0,
        sensitivity: Vec::new(),
    };

    let stream = cash_flow_stream(a, &result, a.rent_growth_bps);
    result.irr_bps = if total_cash_invested > 0 {
        irr_bps(&stream)
    } else {
        None
    };
    // Σ stream nets the year-0 cash outflow against every later inflow, so the
    // sum *is* the net profit over the hold.
    result.total_profit_cents = stream.iter().sum::<f64>().round() as i64;

    // Rent-growth sensitivity: −2pp … +2pp around the base assumption.
    result.sensitivity = [-200, -100, 0, 100, 200]
        .iter()
        .map(|delta| {
            let g_bps = (a.rent_growth_bps + delta).max(0);
            let s = cash_flow_stream(a, &result, g_bps);
            SensitivityPoint {
                rent_growth_bps: g_bps,
                irr_bps: if total_cash_invested > 0 {
                    irr_bps(&s)
                } else {
                    None
                },
            }
        })
        .collect();

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rental_case() -> Assumptions {
        Assumptions {
            purchase_price_cents: 20_000_000, // $200,000
            rehab_cents: 0,
            closing_costs_cents: 600_000, // $6,000
            arv_cents: 20_000_000,
            monthly_rent_cents: 200_000,    // $2,000
            monthly_expenses_cents: 60_000, // $600
            vacancy_bps: 500,
            down_payment_bps: 2000,
            interest_rate_bps: 700,
            loan_term_years: 30,
            rent_growth_bps: 300,
            appreciation_bps: 300,
            exit_cap_rate_bps: 0,
            selling_costs_bps: 700,
            hold_years: 5,
        }
    }

    #[test]
    fn amortized_payment_matches_formula() {
        // $160k @ 7% / 30yr ≈ $1064.48/mo.
        let pay = monthly_payment_cents(16_000_000, 700, 30);
        assert!((pay - 106_448).abs() <= 50, "payment was {pay}");
    }

    #[test]
    fn zero_interest_amortizes_linearly() {
        let pay = monthly_payment_cents(12_000_000, 0, 10);
        assert_eq!(pay, 12_000_000 / 120);
    }

    #[test]
    fn core_metrics_are_sane() {
        let u = underwrite(&rental_case());
        // 20% of $200k down, + $6k closing.
        assert_eq!(u.down_payment_cents, 4_000_000);
        assert_eq!(u.loan_amount_cents, 16_000_000);
        assert_eq!(u.total_cash_invested_cents, 4_600_000);
        // NOI = (24,000 gross − 5% vacancy) − 7,200 opex = 22,800 − 7,200 = 15,600.
        assert_eq!(u.gross_rent_annual_cents, 2_400_000);
        assert_eq!(u.vacancy_loss_cents, 120_000);
        assert_eq!(u.noi_annual_cents, 1_560_000);
        // Cap rate on cost basis $206k ≈ 7.57%.
        assert!((u.cap_rate_bps - 757).abs() <= 5, "cap {}", u.cap_rate_bps);
        // DSCR = NOI / annual debt service, > 1 for this deal.
        assert!(u.dscr > 1.0 && u.dscr < 2.0, "dscr {}", u.dscr);
        // Positive, converging IRR.
        assert!(u.irr_bps.unwrap() > 0, "irr {:?}", u.irr_bps);
        assert_eq!(u.sensitivity.len(), 5);
        // Higher rent growth => higher IRR (monotone across the band).
        let irrs: Vec<i32> = u.sensitivity.iter().filter_map(|s| s.irr_bps).collect();
        assert_eq!(irrs.len(), 5);
        for w in irrs.windows(2) {
            assert!(w[1] >= w[0], "sensitivity not monotone: {irrs:?}");
        }
    }

    #[test]
    fn irr_of_known_stream() {
        // −1000 then +600, +600: IRR ≈ 13.07%.
        let irr = irr_bps(&[-1000.0, 600.0, 600.0]).unwrap();
        assert!((irr - 1307).abs() <= 20, "irr {irr}");
    }

    #[test]
    fn all_cash_deal_has_no_debt_service() {
        let mut a = rental_case();
        a.down_payment_bps = 10_000; // 100% down
        let u = underwrite(&a);
        assert_eq!(u.loan_amount_cents, 0);
        assert_eq!(u.annual_debt_service_cents, 0);
        assert_eq!(u.dscr, 0.0);
        assert_eq!(u.annual_cash_flow_cents, u.noi_annual_cents);
    }

    #[test]
    fn empty_deal_does_not_panic() {
        let u = underwrite(&Assumptions::default());
        assert_eq!(u.noi_annual_cents, 0);
        assert_eq!(u.irr_bps, None);
        assert_eq!(u.cap_rate_bps, 0);
    }

    #[test]
    fn exit_cap_drives_value() {
        let mut a = rental_case();
        a.exit_cap_rate_bps = 600; // 6% exit cap
        let u = underwrite(&a);
        // Value ≈ year-5 NOI / 6%. Positive and well above the loan payoff.
        assert!(u.exit_value_cents > 0);
        assert!(u.net_sale_proceeds_cents > 0);
    }
}
