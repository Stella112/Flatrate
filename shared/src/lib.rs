//! Pure math shared by the Flatrate contract, the oracle and the tests.
//!
//! Units:
//! - money and prices are micro-USD (`1_000_000` = $1);
//! - claims use 6 decimals too: `1_000_000` claim units = one claim that pays at most $1;
//! - auction prices are whole cents, 1..=99, for one claim.

/// One dollar, in micro-USD. One Long plus one Short claim always pays exactly this.
pub const PAIR_MICROS: u64 = 1_000_000;
/// One cent, in micro-USD.
pub const CENT_MICROS: u64 = 10_000;

/// Payout per whole claim, in micro-USD.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Payoff {
    pub long_micros: u64,
    pub short_micros: u64,
}

/// Payout when a market is void: both sides get half.
pub const VOID_PAYOFF: Payoff = Payoff { long_micros: 500_000, short_micros: 500_000 };

/// `long = clamp((avg - floor) * 1_000_000 / (cap - floor), 0, 1_000_000)`, rounded down;
/// `short = 1_000_000 - long`. Requires `cap > floor`.
pub fn payoff(avg_micros: u64, floor_micros: u64, cap_micros: u64) -> Payoff {
    assert!(cap_micros > floor_micros, "cap must be above floor");
    let long = if avg_micros <= floor_micros {
        0
    } else if avg_micros >= cap_micros {
        PAIR_MICROS
    } else {
        let num = (avg_micros - floor_micros) as u128 * PAIR_MICROS as u128;
        (num / (cap_micros - floor_micros) as u128) as u64
    };
    Payoff { long_micros: long, short_micros: PAIR_MICROS - long }
}

/// Integer mean, rounded down. `None` for an empty slice.
pub fn mean(values: &[u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let sum: u128 = values.iter().map(|v| *v as u128).sum();
    Some((sum / values.len() as u128) as u64)
}

/// Median. For an even count, the mean of the two middle values, rounded down.
pub fn median(values: &[u64]) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    let mut v = values.to_vec();
    v.sort_unstable();
    let n = v.len();
    if n % 2 == 1 {
        Some(v[n / 2])
    } else {
        Some(((v[n / 2 - 1] as u128 + v[n / 2] as u128) / 2) as u64)
    }
}

/// fUSD micro-units paid for `balance_units` claim units at `payout_micros` per whole claim.
/// Rounded down, so Long + Short payouts never exceed the collateral.
pub fn redeem_amount(balance_units: u128, payout_micros: u64) -> u128 {
    balance_units * payout_micros as u128 / PAIR_MICROS as u128
}

/// Side of an auction bid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BidSide {
    Long,
    Short,
}

/// A revealed bid. `limit_cents` is the most the bidder pays for one claim of its side.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuctionBid {
    /// Commit order: lower = earlier commit.
    pub id: u64,
    pub side: BidSide,
    /// Whole claims wanted.
    pub qty: u64,
    pub limit_cents: u8,
    /// Escrow deposited at commit, micro-USD.
    pub escrow_micros: u128,
}

impl AuctionBid {
    /// A bid is valid if its quantity and limit are in range and its escrow covers `qty x limit`.
    pub fn is_valid(&self) -> bool {
        self.qty > 0
            && (1..=99).contains(&self.limit_cents)
            && self.qty as u128 * self.limit_cents as u128 * CENT_MICROS as u128 <= self.escrow_micros
    }
}

/// Result for one bid after clearing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fill {
    pub id: u64,
    pub side: BidSide,
    /// Whole claims filled (0 if not filled).
    pub filled_qty: u64,
    /// What the bidder pays out of escrow, micro-USD.
    pub cost_micros: u128,
    /// Escrow returned, micro-USD.
    pub refund_micros: u128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clearing {
    /// Opening price for one Long claim in cents; `None` if nothing matched.
    pub price_cents: Option<u8>,
    /// Pairs minted.
    pub matched: u64,
    /// One entry per input bid, in input order.
    pub fills: Vec<Fill>,
}

fn eligible(bid: &AuctionBid, p: u8) -> bool {
    bid.is_valid()
        && match bid.side {
            BidSide::Long => bid.limit_cents >= p,
            BidSide::Short => bid.limit_cents >= 100 - p,
        }
}

/// Uniform-price clearing of the sealed opening auction.
///
/// For each `P` in 1..=99: `L(P)` = Long qty with `limit >= P`, `S(P)` = Short qty with
/// `limit >= 100 - P`, `matched(P) = min(L, S)`. Pick the most matched; tie -> closest to 50;
/// then the lower `P`. Fill by better limit, then earlier commit; the marginal bid may be
/// filled partly. Long pays `P` cents per claim, Short pays `100 - P`; together $1 per pair.
pub fn clear_auction(bids: &[AuctionBid]) -> Clearing {
    let mut best: Option<(u8, u64)> = None;
    for p in 1u8..=99 {
        let mut l: u128 = 0;
        let mut s: u128 = 0;
        for b in bids.iter().filter(|b| eligible(b, p)) {
            match b.side {
                BidSide::Long => l += b.qty as u128,
                BidSide::Short => s += b.qty as u128,
            }
        }
        let m = l.min(s).min(u64::MAX as u128) as u64;
        if m == 0 {
            continue;
        }
        let better = match best {
            None => true,
            Some((bp, bm)) => {
                let d = (p as i16 - 50).abs();
                let bd = (bp as i16 - 50).abs();
                m > bm || (m == bm && (d < bd || (d == bd && p < bp)))
            }
        };
        if better {
            best = Some((p, m));
        }
    }

    let mut fills: Vec<Fill> = bids
        .iter()
        .map(|b| Fill {
            id: b.id,
            side: b.side,
            filled_qty: 0,
            cost_micros: 0,
            refund_micros: b.escrow_micros,
        })
        .collect();

    let Some((p, matched)) = best else {
        return Clearing { price_cents: None, matched: 0, fills };
    };

    for side in [BidSide::Long, BidSide::Short] {
        let unit_cents = match side {
            BidSide::Long => p as u128,
            BidSide::Short => 100 - p as u128,
        };
        let mut order: Vec<usize> = (0..bids.len())
            .filter(|&i| bids[i].side == side && eligible(&bids[i], p))
            .collect();
        order.sort_by(|&a, &b| {
            bids[b].limit_cents.cmp(&bids[a].limit_cents).then(bids[a].id.cmp(&bids[b].id))
        });
        let mut remaining = matched;
        for i in order {
            if remaining == 0 {
                break;
            }
            let q = bids[i].qty.min(remaining);
            remaining -= q;
            let cost = q as u128 * unit_cents * CENT_MICROS as u128;
            fills[i].filled_qty = q;
            fills[i].cost_micros = cost;
            fills[i].refund_micros = bids[i].escrow_micros - cost;
        }
    }

    Clearing { price_cents: Some(p), matched, fills }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const F: u64 = 2_400_000;
    const C: u64 = 5_600_000;

    #[test]
    fn payoff_at_floor_cap_mid() {
        assert_eq!(payoff(F, F, C), Payoff { long_micros: 0, short_micros: 1_000_000 });
        assert_eq!(payoff(C, F, C), Payoff { long_micros: 1_000_000, short_micros: 0 });
        assert_eq!(payoff(4_000_000, F, C), Payoff { long_micros: 500_000, short_micros: 500_000 });
    }

    #[test]
    fn payoff_below_floor_and_above_cap() {
        assert_eq!(payoff(1, F, C).long_micros, 0);
        assert_eq!(payoff(0, F, C).long_micros, 0);
        assert_eq!(payoff(9_000_000, F, C).long_micros, 1_000_000);
        assert_eq!(payoff(u64::MAX, F, C).long_micros, 1_000_000);
    }

    #[test]
    fn payoff_rounds_long_down() {
        // (2_400_001 - 2_400_000) * 1e6 / 3_200_000 = 0.3125 -> 0
        assert_eq!(payoff(2_400_001, F, C), Payoff { long_micros: 0, short_micros: 1_000_000 });
        // 3.98: (1_580_000 * 1e6) / 3_200_000 = 493_750 exactly
        assert_eq!(payoff(3_980_000, F, C).long_micros, 493_750);
        // 3.99: 1_590_000e6 / 3.2e6 = 496_875
        assert_eq!(payoff(3_990_000, F, C).long_micros, 496_875);
    }

    #[test]
    fn void_payoff_is_half() {
        assert_eq!(VOID_PAYOFF.long_micros + VOID_PAYOFF.short_micros, PAIR_MICROS);
        assert_eq!(VOID_PAYOFF.long_micros, 500_000);
    }

    #[test]
    #[should_panic(expected = "cap must be above floor")]
    fn payoff_rejects_bad_bounds() {
        payoff(1, 5, 5);
    }

    #[test]
    fn mean_and_median() {
        assert_eq!(mean(&[]), None);
        assert_eq!(mean(&[1, 2]), Some(1));
        assert_eq!(mean(&[3_980_000, 3_990_000, 4_000_000]), Some(3_990_000));
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[5, 1, 3]), Some(3));
        assert_eq!(median(&[4, 1, 3, 2]), Some(2));
        // the five Phase 0 sources, in micro-USD
        assert_eq!(median(&[2_560_000, 3_976_355, 3_990_000, 3_980_000, 5_087_000]), Some(3_980_000));
    }

    #[test]
    fn redeem_amount_rounds_down() {
        assert_eq!(redeem_amount(1_000_000, 493_750), 493_750);
        assert_eq!(redeem_amount(3, 333_333), 0);
        assert_eq!(redeem_amount(100_000_000, 1_000_000), 100_000_000);
    }

    fn bid(id: u64, side: BidSide, qty: u64, limit: u8) -> AuctionBid {
        AuctionBid { id, side, qty, limit_cents: limit, escrow_micros: qty as u128 * limit as u128 * 10_000 }
    }

    #[test]
    fn two_bid_match() {
        // Long up to 60c, Short up to 45c -> any P in 55..=60 matches 10; closest to 50 is 55.
        let c = clear_auction(&[bid(0, BidSide::Long, 10, 60), bid(1, BidSide::Short, 10, 45)]);
        assert_eq!(c.price_cents, Some(55));
        assert_eq!(c.matched, 10);
        assert_eq!(c.fills[0].cost_micros, 10 * 55 * 10_000);
        assert_eq!(c.fills[1].cost_micros, 10 * 45 * 10_000);
        assert_eq!(c.fills[0].refund_micros, 10 * 5 * 10_000);
        assert_eq!(c.fills[1].refund_micros, 0);
    }

    #[test]
    fn tie_break_prefers_50_then_lower() {
        // Long 70, Short 70 -> P in 30..=70 all match 5; P=50 wins.
        let c = clear_auction(&[bid(0, BidSide::Long, 5, 70), bid(1, BidSide::Short, 5, 70)]);
        assert_eq!(c.price_cents, Some(50));
        // Long 40, Short 70 -> P in 30..=40 -> closest to 50 is 40.
        let c = clear_auction(&[bid(0, BidSide::Long, 5, 40), bid(1, BidSide::Short, 5, 70)]);
        assert_eq!(c.price_cents, Some(40));
        // Long 51 / Short 51 -> P in 49..=51, 50 wins.
        let c = clear_auction(&[bid(0, BidSide::Long, 5, 51), bid(1, BidSide::Short, 51, 51)]);
        assert_eq!(c.price_cents, Some(50));
        // Long 49 qty 5 + Long 51 qty 5, Short 51 qty 10 (P>=49): P=49 -> L=10, P=51 -> L=5.
        let c = clear_auction(&[
            bid(0, BidSide::Long, 5, 49),
            bid(1, BidSide::Long, 5, 51),
            bid(2, BidSide::Short, 10, 51),
        ]);
        assert_eq!(c.price_cents, Some(49));
        assert_eq!(c.matched, 10);
    }

    #[test]
    fn best_prices_form_one_range_so_equal_distance_ties_include_50() {
        // L(P) falls and S(P) rises with P, so the set of P with the most matched is one range.
        // A tie at 45 and 55 therefore also covers 50, and 50 wins; the "lower P" rule only
        // decides between neighbours such as 49/51 when 50 itself is in range.
        let c = clear_auction(&[
            bid(0, BidSide::Long, 10, 55),
            bid(1, BidSide::Short, 10, 55),
            bid(2, BidSide::Long, 5, 45),
            bid(3, BidSide::Short, 5, 45),
        ]);
        assert_eq!(c.price_cents, Some(50));
        assert_eq!(c.matched, 10);
    }

    #[test]
    fn partial_fill_by_limit_then_commit_order() {
        // Short 7 at 50 (P>=50). Longs: id0 5@60, id1 5@55, id2 5@60.
        // At P=50: L=15, S=7 -> 7. Fill order: limit 60 (id0, id2 by commit), then 55.
        let c = clear_auction(&[
            bid(0, BidSide::Long, 5, 60),
            bid(1, BidSide::Long, 5, 55),
            bid(2, BidSide::Long, 5, 60),
            bid(3, BidSide::Short, 7, 50),
        ]);
        assert_eq!(c.price_cents, Some(50));
        assert_eq!(c.matched, 7);
        assert_eq!(c.fills[0].filled_qty, 5);
        assert_eq!(c.fills[2].filled_qty, 2);
        assert_eq!(c.fills[1].filled_qty, 0);
        assert_eq!(c.fills[1].refund_micros, 5 * 55 * 10_000);
        assert_eq!(c.fills[3].filled_qty, 7);
    }

    #[test]
    fn no_match_refunds_everything() {
        // Long 40, Short 40 -> need P<=40 and P>=60: impossible.
        let c = clear_auction(&[bid(0, BidSide::Long, 5, 40), bid(1, BidSide::Short, 5, 40)]);
        assert_eq!(c.price_cents, None);
        assert_eq!(c.matched, 0);
        assert!(c.fills.iter().all(|f| f.filled_qty == 0 && f.cost_micros == 0));
        assert_eq!(c.fills[0].refund_micros, 5 * 40 * 10_000);
        let c = clear_auction(&[]);
        assert_eq!(c.price_cents, None);
    }

    #[test]
    fn invalid_bid_is_ignored_and_refunded() {
        let mut b = bid(0, BidSide::Long, 10, 60);
        b.escrow_micros -= 1; // does not cover qty x limit
        let c = clear_auction(&[b.clone(), bid(1, BidSide::Short, 10, 45)]);
        assert_eq!(c.matched, 0);
        assert_eq!(c.fills[0].refund_micros, b.escrow_micros);
    }

    #[test]
    fn two_hundred_bids() {
        let bids: Vec<AuctionBid> = (0..200)
            .map(|i| {
                let side = if i % 2 == 0 { BidSide::Long } else { BidSide::Short };
                bid(i, side, 1 + i % 7, 1 + (i % 99) as u8)
            })
            .collect();
        let c = clear_auction(&bids);
        check_clearing(&bids, &c);
    }

    fn check_clearing(bids: &[AuctionBid], c: &Clearing) {
        assert_eq!(c.fills.len(), bids.len());
        let long: u64 = c.fills.iter().filter(|f| f.side == BidSide::Long).map(|f| f.filled_qty).sum();
        let short: u64 = c.fills.iter().filter(|f| f.side == BidSide::Short).map(|f| f.filled_qty).sum();
        assert_eq!(long, c.matched);
        assert_eq!(short, c.matched);
        let paid: u128 = c.fills.iter().map(|f| f.cost_micros).sum();
        assert_eq!(paid, c.matched as u128 * PAIR_MICROS as u128, "each pair is backed by exactly $1");
        for (b, f) in bids.iter().zip(&c.fills) {
            assert!(f.cost_micros <= b.escrow_micros, "never spend more than escrow");
            assert_eq!(f.cost_micros + f.refund_micros, b.escrow_micros);
            assert!(f.filled_qty <= b.qty);
            if f.filled_qty > 0 {
                let p = c.price_cents.unwrap();
                match b.side {
                    BidSide::Long => assert!(b.limit_cents >= p),
                    BidSide::Short => assert!(b.limit_cents >= 100 - p),
                }
            }
        }
        // No other price matches more.
        for p in 1u8..=99 {
            let l: u64 = bids.iter().filter(|b| b.side == BidSide::Long && eligible(b, p)).map(|b| b.qty).sum();
            let s: u64 = bids.iter().filter(|b| b.side == BidSide::Short && eligible(b, p)).map(|b| b.qty).sum();
            assert!(l.min(s) <= c.matched);
        }
    }

    fn arb_bid() -> impl Strategy<Value = (bool, u64, u8, u128)> {
        (any::<bool>(), 1u64..1_000, 1u8..=99, 0u128..2_000_000)
    }

    proptest! {
        #[test]
        fn prop_long_plus_short_is_one_dollar(avg in any::<u64>(), floor in 0u64..u64::MAX / 2, width in 1u64..u64::MAX / 2) {
            let p = payoff(avg, floor, floor + width);
            prop_assert_eq!(p.long_micros + p.short_micros, PAIR_MICROS);
            prop_assert!(p.long_micros <= PAIR_MICROS);
        }

        #[test]
        fn prop_payoff_monotonic(a in 0u64..20_000_000, b in 0u64..20_000_000) {
            let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
            prop_assert!(payoff(lo, F, C).long_micros <= payoff(hi, F, C).long_micros);
        }

        #[test]
        fn prop_redeem_never_exceeds_collateral(bal in 0u128..1_000_000_000_000, avg in 0u64..20_000_000) {
            let p = payoff(avg, F, C);
            prop_assert!(redeem_amount(bal, p.long_micros) + redeem_amount(bal, p.short_micros) <= bal);
        }

        #[test]
        fn prop_clearing_never_mints_more_than_escrow_covers(raw in proptest::collection::vec(arb_bid(), 0..60)) {
            let bids: Vec<AuctionBid> = raw.iter().enumerate().map(|(i, (long, qty, limit, extra))| {
                let side = if *long { BidSide::Long } else { BidSide::Short };
                let need = *qty as u128 * *limit as u128 * 10_000;
                // about 1 in 8 bids is underfunded
                let escrow = if extra % 8 == 0 { need.saturating_sub(1 + extra % 1000) } else { need + extra };
                AuctionBid { id: i as u64, side, qty: *qty, limit_cents: *limit, escrow_micros: escrow }
            }).collect();
            let c = clear_auction(&bids);
            check_clearing(&bids, &c);
        }
    }
}
