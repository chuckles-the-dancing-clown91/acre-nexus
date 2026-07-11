//! **Investor / syndication** distribution-waterfall engine (issue #13, Beyond-GA
//! vertical). Pure, deterministic money math — no I/O — so it unit-tests like
//! [`crate::underwriting`]. The `syndication` module's routes persist the inputs
//! (commitments, capital calls) and the outputs (distribution lines); this file
//! owns the arithmetic.
//!
//! ## The waterfall
//! A distribution of `total_cents` to a legal entity's investors flows through
//! three tiers, in order, each fully paid before the next:
//!
//! 1. **Return of capital** — pay down each investor's *unreturned contributed
//!    capital*, pro-rata by that unreturned balance.
//! 2. **Preferred return** — a hurdle paid pro-rata to the *preferred owed*,
//!    computed as a simple one-period rate (`pref_rate_bps`) on contributed
//!    capital. (Deliberately simple/deterministic — a real accrual compounds over
//!    time; documented as a known simplification.)
//! 3. **Profit split (carried interest)** — of what remains, the GP takes
//!    `carry_bps` as carry; the rest is split among all investors pro-rata by
//!    contributed capital. With no GP on the cap table, no carry is taken and the
//!    whole remainder goes to the investors.
//!
//! Every tier splits an exact integer-cent pool via [`allocate`] (largest
//! remainder), so **the allocations always sum to `total_cents`** — no cent is
//! created or lost.

use uuid::Uuid;

/// 100% in basis points.
const BPS: i128 = 10_000;

/// One investor's position going into a distribution.
#[derive(Clone, Debug)]
pub struct Stake {
    pub commitment_id: Uuid,
    pub owner_id: Uuid,
    /// Total capital the investor has actually funded (their basis).
    pub contributed_cents: i64,
    /// Contributed capital not yet returned by prior distributions.
    pub unreturned_cents: i64,
    /// True for the general partner / manager — the party that earns carry.
    pub is_gp: bool,
}

/// Waterfall tunables for one distribution.
#[derive(Clone, Copy, Debug)]
pub struct WaterfallParams {
    /// Simple one-period preferred-return rate on contributed capital (bps).
    pub pref_rate_bps: i32,
    /// GP carried interest on the profit tier (bps).
    pub carry_bps: i32,
}

/// One investor's slice of a distribution, broken out by tier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Allocation {
    pub commitment_id: Uuid,
    pub owner_id: Uuid,
    pub return_of_capital_cents: i64,
    pub preferred_cents: i64,
    /// The investor's pro-rata share of the post-carry profit tier.
    pub profit_cents: i64,
    /// Carried interest earned (nonzero only for the GP).
    pub carry_cents: i64,
}

impl Allocation {
    /// Everything this investor receives from the distribution.
    pub fn total_cents(&self) -> i64 {
        self.return_of_capital_cents + self.preferred_cents + self.profit_cents + self.carry_cents
    }
}

/// Distribute `pool` cents across `weights` proportionally and **exactly** (the
/// result sums to `pool` for any `pool >= 0`), using the largest-remainder
/// method. When every weight is zero but `pool > 0`, the pool is spread as
/// evenly as possible so no cents are lost.
fn allocate(pool: i64, weights: &[i64]) -> Vec<i64> {
    let n = weights.len();
    if n == 0 || pool <= 0 {
        return vec![0; n];
    }
    let w: Vec<i128> = weights.iter().map(|&x| (x.max(0)) as i128).collect();
    let total: i128 = w.iter().sum();
    let pool = pool as i128;

    // Degenerate: no weight to go by — spread evenly, largest-remainder style.
    if total == 0 {
        let base = pool / n as i128;
        let extra = (pool - base * n as i128) as usize;
        return (0..n)
            .map(|i| (base + if i < extra { 1 } else { 0 }) as i64)
            .collect();
    }

    let mut shares = vec![0i64; n];
    // (index, fractional remainder) for handing out the leftover cents.
    let mut rema: Vec<(usize, i128)> = Vec::with_capacity(n);
    let mut assigned: i128 = 0;
    for (i, &wi) in w.iter().enumerate() {
        let exact = pool * wi;
        let floor = exact / total;
        shares[i] = floor as i64;
        rema.push((i, exact - floor * total));
        assigned += floor;
    }
    // leftover < n by construction; give one extra cent to the largest remainders.
    let leftover = (pool - assigned) as usize;
    rema.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    for &(idx, _) in rema.iter().take(leftover) {
        shares[idx] += 1;
    }
    shares
}

/// Run the three-tier waterfall. Returns one [`Allocation`] per stake, in the
/// same order. The allocations always sum to `total_cents.max(0)`.
pub fn run_waterfall(
    total_cents: i64,
    stakes: &[Stake],
    params: &WaterfallParams,
) -> Vec<Allocation> {
    let n = stakes.len();
    let mut roc = vec![0i64; n];
    let mut pref = vec![0i64; n];
    let mut profit = vec![0i64; n];
    let mut carry = vec![0i64; n];
    let mut remaining = total_cents.max(0);

    // Tier 1 — return of capital, pro-rata by unreturned balance.
    let unret: Vec<i64> = stakes.iter().map(|s| s.unreturned_cents.max(0)).collect();
    let roc_pool = remaining.min(unret.iter().sum());
    if roc_pool > 0 {
        roc = allocate(roc_pool, &unret);
        remaining -= roc_pool;
    }

    // Tier 2 — preferred return: simple one-period rate on contributed capital.
    let pref_owed: Vec<i64> = stakes
        .iter()
        .map(|s| {
            (s.contributed_cents.max(0) as i128 * params.pref_rate_bps.max(0) as i128 / BPS) as i64
        })
        .collect();
    let pref_pool = remaining.min(pref_owed.iter().sum());
    if pref_pool > 0 {
        pref = allocate(pref_pool, &pref_owed);
        remaining -= pref_pool;
    }

    // Tier 3 — profit split: carry to the GP, the rest by contributed capital.
    if remaining > 0 {
        let has_gp = stakes.iter().any(|s| s.is_gp);
        let carry_total = if has_gp {
            (remaining as i128 * params.carry_bps.clamp(0, 10_000) as i128 / BPS) as i64
        } else {
            0
        };
        if carry_total > 0 {
            // Split carry among GP stakes, pro-rata by contributed (equal if none).
            let gp_contrib: Vec<i64> = stakes
                .iter()
                .map(|s| {
                    if s.is_gp {
                        s.contributed_cents.max(0)
                    } else {
                        0
                    }
                })
                .collect();
            let gp_weights: Vec<i64> = if gp_contrib.iter().sum::<i64>() > 0 {
                gp_contrib
            } else {
                stakes.iter().map(|s| i64::from(s.is_gp)).collect()
            };
            carry = allocate(carry_total, &gp_weights);
        }
        let lp_profit = remaining - carry_total;
        let contrib: Vec<i64> = stakes.iter().map(|s| s.contributed_cents.max(0)).collect();
        profit = allocate(lp_profit, &contrib);
    }

    (0..n)
        .map(|i| Allocation {
            commitment_id: stakes[i].commitment_id,
            owner_id: stakes[i].owner_id,
            return_of_capital_cents: roc[i],
            preferred_cents: pref[i],
            profit_cents: profit[i],
            carry_cents: carry[i],
        })
        .collect()
}

/// Split a capital call of `total_cents` across commitments pro-rata by their
/// committed capital, exact to the cent.
pub fn split_capital_call(total_cents: i64, committed: &[i64]) -> Vec<i64> {
    allocate(total_cents.max(0), committed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stake(contrib: i64, unret: i64, gp: bool) -> Stake {
        Stake {
            commitment_id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            contributed_cents: contrib,
            unreturned_cents: unret,
            is_gp: gp,
        }
    }

    fn sum(a: &[Allocation]) -> i64 {
        a.iter().map(|x| x.total_cents()).sum()
    }

    #[test]
    fn allocate_is_exact_and_pro_rata() {
        // 100 across 1:3 -> 25 / 75.
        assert_eq!(allocate(100, &[100, 300]), vec![25, 75]);
        // Indivisible: 10 across three equal weights -> 4/3/3 (largest remainder).
        let a = allocate(10, &[1, 1, 1]);
        assert_eq!(a.iter().sum::<i64>(), 10);
        assert_eq!(a, vec![4, 3, 3]);
        // Zero weights but positive pool -> spread evenly, nothing lost.
        let z = allocate(7, &[0, 0, 0]);
        assert_eq!(z.iter().sum::<i64>(), 7);
        // Nonpositive pool -> all zeros.
        assert_eq!(allocate(0, &[1, 2]), vec![0, 0]);
    }

    #[test]
    fn return_of_capital_only_when_distribution_is_small() {
        // Two LPs, each with 1,000,000 unreturned; distribute 600,000 total.
        let stakes = vec![
            stake(1_000_000, 1_000_000, false),
            stake(1_000_000, 1_000_000, false),
        ];
        let p = WaterfallParams {
            pref_rate_bps: 0,
            carry_bps: 2000,
        };
        let a = run_waterfall(600_000, &stakes, &p);
        // Entirely return of capital, split 50/50, nothing reaches profit.
        assert_eq!(a[0].return_of_capital_cents, 300_000);
        assert_eq!(a[1].return_of_capital_cents, 300_000);
        assert_eq!(a[0].profit_cents + a[0].carry_cents, 0);
        assert_eq!(sum(&a), 600_000);
    }

    #[test]
    fn full_waterfall_with_pref_and_carry() {
        // GP + one LP, each contributed 1,000,000, fully returned already
        // (unreturned = 0), 10% pref, 20% carry. Distribute 500,000.
        let gp = stake(1_000_000, 0, true);
        let lp = stake(1_000_000, 0, false);
        let stakes = vec![gp, lp];
        let p = WaterfallParams {
            pref_rate_bps: 1000, // 10%
            carry_bps: 2000,     // 20%
        };
        let a = run_waterfall(500_000, &stakes, &p);
        // Pref owed = 10% of 1,000,000 each = 100,000 each = 200,000 total.
        assert_eq!(a[0].preferred_cents, 100_000);
        assert_eq!(a[1].preferred_cents, 100_000);
        // Remaining 300,000 profit: 20% carry = 60,000 to GP; 240,000 split
        // 50/50 by contributed capital = 120,000 each.
        assert_eq!(a[0].carry_cents, 60_000);
        assert_eq!(a[1].carry_cents, 0);
        assert_eq!(a[0].profit_cents, 120_000);
        assert_eq!(a[1].profit_cents, 120_000);
        // GP nets 100k pref + 120k profit + 60k carry = 280k; LP nets 220k.
        assert_eq!(a[0].total_cents(), 280_000);
        assert_eq!(a[1].total_cents(), 220_000);
        assert_eq!(sum(&a), 500_000);
    }

    #[test]
    fn no_gp_means_no_carry() {
        let stakes = vec![stake(1_000_000, 0, false), stake(3_000_000, 0, false)];
        let p = WaterfallParams {
            pref_rate_bps: 0,
            carry_bps: 2000,
        };
        let a = run_waterfall(400_000, &stakes, &p);
        // No carry: full 400,000 split 1:3 by contributed capital.
        assert!(a.iter().all(|x| x.carry_cents == 0));
        assert_eq!(a[0].profit_cents, 100_000);
        assert_eq!(a[1].profit_cents, 300_000);
        assert_eq!(sum(&a), 400_000);
    }

    #[test]
    fn conserves_every_cent_on_awkward_numbers() {
        let stakes = vec![
            stake(333_333, 111_111, true),
            stake(666_667, 222_222, false),
            stake(1, 1, false),
        ];
        let p = WaterfallParams {
            pref_rate_bps: 733,
            carry_bps: 1875,
        };
        for total in [1i64, 7, 99, 100_001, 1_234_567] {
            let a = run_waterfall(total, &stakes, &p);
            assert_eq!(sum(&a), total, "distribution {total} must be conserved");
            assert!(a.iter().all(|x| x.total_cents() >= 0));
        }
    }

    #[test]
    fn capital_call_splits_pro_rata() {
        // Call 1,000,000 across commitments 250k / 750k -> 250k / 750k.
        assert_eq!(
            split_capital_call(1_000_000, &[250_000, 750_000]),
            vec![250_000, 750_000]
        );
        // Exact to the cent on an indivisible call.
        let s = split_capital_call(100, &[1, 1, 1]);
        assert_eq!(s.iter().sum::<i64>(), 100);
    }
}
