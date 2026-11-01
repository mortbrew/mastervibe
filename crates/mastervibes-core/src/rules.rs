//! Run structure loaded from `data/run.toml`.
//!
//! Layer shape (slots, charset, probe budget, repeat cap) lives here too, so
//! [`crate::LayerSpec::for_layer`] and the run loop cannot drift apart.

use std::sync::OnceLock;

use serde::Deserialize;
use thiserror::Error;

use crate::amp::{parse_amp, Amp};
use crate::layer::{LayerSpec, LAYER_COUNT};

/// Breach names inside a layer, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BreachRole {
    /// First breach of the layer.
    Perimeter,
    /// Second breach of the layer.
    Internal,
    /// Third breach of the layer. The layer target is checked when it ends.
    Root,
}

impl BreachRole {
    /// The three roles, in play order.
    pub const ALL: [Self; 3] = [Self::Perimeter, Self::Internal, Self::Root];

    /// Role of breach `index` in `0..3`.
    pub fn from_index(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    /// Base payout for this breach, from the rules.
    pub fn base_pay(self, rules: &RunRules) -> u32 {
        match self {
            Self::Perimeter => rules.pay_perimeter,
            Self::Internal => rules.pay_internal,
            Self::Root => rules.pay_root,
        }
    }
}

impl std::fmt::Display for BreachRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Perimeter => "Perimeter",
            Self::Internal => "Internal",
            Self::Root => "Root",
        })
    }
}

/// One layer row from the run table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LayerRule {
    /// 1-based layer number.
    pub layer: u8,
    /// Slots in the secret.
    pub slots: u8,
    /// Charset size.
    pub symbols: u8,
    /// Probes in the breach before tools add any.
    pub probe_budget: u8,
    /// Layer target, in whole points.
    pub target: u32,
}

impl LayerRule {
    /// Target in thousandths of a point, matching the score pipeline.
    pub fn target_milli(&self) -> i64 {
        i64::from(self.target).saturating_mul(1000)
    }
}

/// Economy, shop, crack bonus, and the eight layer rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunRules {
    /// Added to 1 for each unused base-budget probe on a cracked breach.
    pub crack_per_unused: Amp,
    /// Cap on the crack multiplier.
    pub crack_cap: Amp,
    /// Fraction of probe score kept when the breach is not cracked.
    pub uncracked_keep: Amp,
    /// Cap on the product of tool ×amp effects.
    pub tool_mul_cap: Amp,
    /// Perimeter base pay.
    pub pay_perimeter: u32,
    /// Internal base pay.
    pub pay_internal: u32,
    /// Root base pay.
    pub pay_root: u32,
    /// Dollars per probe under par on a crack.
    pub pay_under_par: u32,
    /// Dollars per full extra target the layer score covers.
    pub extra_target_pay: u32,
    /// Cap on that overkill bonus.
    pub extra_target_cap: u32,
    /// Interest is `$1` per this many dollars held.
    pub interest_per: u32,
    /// Interest cap before tools.
    pub interest_cap: u32,
    /// Dollars at the start of a run.
    pub starting_money: u32,
    /// Repeat cap for a fair secret.
    pub max_repeat: u8,
    /// Tool cards offered in the shop.
    pub shop_offers: usize,
    /// Cost of the first reroll in a shop.
    pub reroll_cost: u32,
    /// Added to the reroll cost after each reroll in that shop.
    pub reroll_step: u32,
    /// Sell refund is `price / sell_divisor`.
    pub sell_divisor: u32,
    /// Tool slots in the rig.
    pub rig_slots: usize,
    /// Shop weight for Common, Uncommon, Rare, Legendary.
    pub rarity_weight: [u32; 4],
    /// Layers 1 through 8.
    pub layers: Vec<LayerRule>,
}

impl RunRules {
    /// Parse a run table.
    pub fn from_toml(text: &str) -> Result<Self, RulesError> {
        let raw: RawFile = toml::from_str(text).map_err(|err| RulesError::Toml(err.to_string()))?;
        let crack_per_unused = amp("crack.per_unused", raw.crack.per_unused)?;
        let crack_cap = amp("crack.cap", raw.crack.cap)?;
        let uncracked_keep = amp("crack.uncracked_keep", raw.crack.uncracked_keep)?;
        let tool_mul_cap = amp("amp.tool_mul_cap", raw.amp.tool_mul_cap)?;
        if crack_per_unused.milli() < 0 || crack_cap.milli() <= 0 || tool_mul_cap.milli() <= 0 {
            return Err(RulesError::Invalid(
                "crack and amp caps must be positive".to_string(),
            ));
        }
        if !(0..=1000).contains(&uncracked_keep.milli()) {
            return Err(RulesError::Invalid(
                "uncracked_keep must be from 0 to 1".to_string(),
            ));
        }
        if raw.interest.per == 0 || raw.shop.sell_divisor == 0 {
            return Err(RulesError::Invalid(
                "interest.per and sell_divisor must be > 0".to_string(),
            ));
        }
        if raw.shop.offers == 0 || raw.shop.rig_slots == 0 {
            return Err(RulesError::Invalid(
                "shop offers and rig slots must be > 0".to_string(),
            ));
        }
        if raw.fairness.max_repeat == 0 {
            return Err(RulesError::Invalid("max_repeat must be > 0".to_string()));
        }
        let weights = [
            raw.shop.weight.c,
            raw.shop.weight.u,
            raw.shop.weight.r,
            raw.shop.weight.l,
        ];
        if weights.iter().any(|weight| *weight == 0) {
            return Err(RulesError::Invalid("shop weights must be > 0".to_string()));
        }
        if raw.layer.len() != LAYER_COUNT {
            return Err(RulesError::Invalid(format!(
                "expected {LAYER_COUNT} layers, found {}",
                raw.layer.len()
            )));
        }
        let mut layers = Vec::with_capacity(LAYER_COUNT);
        for (index, row) in raw.layer.iter().enumerate() {
            let expected = u8::try_from(index + 1).expect("layer index fits");
            if row.layer != expected {
                return Err(RulesError::Invalid(format!(
                    "layer {} is numbered {}",
                    expected, row.layer
                )));
            }
            if row.slots == 0 || usize::from(row.slots) > crate::code::MAX_SLOTS {
                return Err(RulesError::Invalid(format!(
                    "layer {expected} slots are out of range"
                )));
            }
            if row.symbols == 0 || usize::from(row.symbols) > crate::code::MAX_SYMBOLS {
                return Err(RulesError::Invalid(format!(
                    "layer {expected} symbols are out of range"
                )));
            }
            if row.probe_budget == 0 || row.target == 0 {
                return Err(RulesError::Invalid(format!(
                    "layer {expected} needs a probe budget and a target"
                )));
            }
            layers.push(LayerRule {
                layer: row.layer,
                slots: row.slots,
                symbols: row.symbols,
                probe_budget: row.probe_budget,
                target: row.target,
            });
        }
        Ok(Self {
            crack_per_unused,
            crack_cap,
            uncracked_keep,
            tool_mul_cap,
            pay_perimeter: raw.payout.perimeter,
            pay_internal: raw.payout.internal,
            pay_root: raw.payout.root,
            pay_under_par: raw.payout.per_probe_under_par,
            extra_target_pay: raw.payout.extra_target,
            extra_target_cap: raw.payout.extra_target_cap,
            interest_per: raw.interest.per,
            interest_cap: raw.interest.cap,
            starting_money: raw.wallet.starting,
            max_repeat: raw.fairness.max_repeat,
            shop_offers: usize::try_from(raw.shop.offers).unwrap_or(usize::MAX),
            reroll_cost: raw.shop.reroll,
            reroll_step: raw.shop.reroll_step,
            sell_divisor: raw.shop.sell_divisor,
            rig_slots: usize::try_from(raw.shop.rig_slots).unwrap_or(usize::MAX),
            rarity_weight: weights,
            layers,
        })
    }

    /// Spec for `layer` in `1..=8`.
    pub fn layer_spec(&self, layer: u8) -> Option<LayerSpec> {
        let row = self.layers.iter().find(|row| row.layer == layer)?;
        Some(LayerSpec::from_parts(
            row.layer,
            row.slots,
            row.symbols,
            row.probe_budget,
            self.max_repeat,
        ))
    }

    /// Row for `layer` in `1..=8`.
    pub fn layer(&self, layer: u8) -> Option<&LayerRule> {
        self.layers.iter().find(|row| row.layer == layer)
    }

    /// Overkill dollars for a cleared layer. `0` when the score covers the
    /// target once. Each further full coverage pays [`Self::extra_target_pay`],
    /// up to [`Self::extra_target_cap`].
    pub fn overkill(&self, layer_milli: i64, target_milli: i64) -> u32 {
        if target_milli <= 0 || layer_milli < target_milli {
            return 0;
        }
        let times = layer_milli / target_milli;
        let extras = u32::try_from(times.saturating_sub(1)).unwrap_or(u32::MAX);
        extras
            .saturating_mul(self.extra_target_pay)
            .min(self.extra_target_cap)
    }

    /// `$1` per [`Self::interest_per`] dollars held, capped.
    pub fn interest(&self, money: u32, cap: u32) -> u32 {
        money.saturating_div(self.interest_per).min(cap)
    }
}

/// The run table compiled into the crate.
pub fn embedded_rules() -> &'static RunRules {
    static RULES: OnceLock<RunRules> = OnceLock::new();
    RULES.get_or_init(|| {
        RunRules::from_toml(include_str!("../data/run.toml")).expect("embedded run.toml is valid")
    })
}

/// The run table could not be loaded.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RulesError {
    /// `toml` rejected the document.
    #[error("could not parse run table: {0}")]
    Toml(String),
    /// A value the run loop cannot use.
    #[error("run table is invalid: {0}")]
    Invalid(String),
}

fn amp(field: &str, value: f64) -> Result<Amp, RulesError> {
    parse_amp(value).map_err(|reason| RulesError::Invalid(format!("{field}: {reason}")))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    crack: RawCrack,
    amp: RawAmp,
    payout: RawPayout,
    interest: RawInterest,
    wallet: RawWallet,
    fairness: RawFairness,
    shop: RawShop,
    layer: Vec<RawLayer>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCrack {
    per_unused: f64,
    cap: f64,
    uncracked_keep: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAmp {
    tool_mul_cap: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPayout {
    perimeter: u32,
    internal: u32,
    root: u32,
    per_probe_under_par: u32,
    extra_target: u32,
    extra_target_cap: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawInterest {
    per: u32,
    cap: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWallet {
    starting: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFairness {
    max_repeat: u8,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawShop {
    offers: u32,
    reroll: u32,
    reroll_step: u32,
    sell_divisor: u32,
    rig_slots: u32,
    weight: RawWeight,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWeight {
    #[serde(rename = "C")]
    c: u32,
    #[serde(rename = "U")]
    u: u32,
    #[serde(rename = "R")]
    r: u32,
    #[serde(rename = "L")]
    l: u32,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLayer {
    layer: u8,
    slots: u8,
    symbols: u8,
    probe_budget: u8,
    target: u32,
}

#[cfg(test)]
mod tests {
    use super::{embedded_rules, BreachRole};
    use crate::amp::Amp;

    #[test]
    fn embedded_table_matches_the_design() {
        let rules = embedded_rules();
        assert_eq!(rules.crack_per_unused, Amp::from_milli(300));
        assert_eq!(rules.crack_cap, Amp::from_milli(5000));
        assert_eq!(rules.uncracked_keep, Amp::from_milli(500));
        assert_eq!(rules.tool_mul_cap, Amp::from_milli(4000));
        assert_eq!(
            (
                rules.pay_perimeter,
                rules.pay_internal,
                rules.pay_root,
                rules.pay_under_par
            ),
            (5, 6, 7, 1)
        );
        assert_eq!((rules.extra_target_pay, rules.extra_target_cap), (2, 3));
        assert_eq!((rules.interest_per, rules.interest_cap), (5, 5));
        assert_eq!(rules.starting_money, 4);
        assert_eq!(rules.shop_offers, 2);
        assert_eq!(rules.reroll_cost, 5);
        assert_eq!(rules.reroll_step, 1);
        assert_eq!(rules.sell_divisor, 2);
        assert_eq!(rules.rig_slots, 5);
        assert_eq!(rules.layers.len(), 8);
        assert_eq!(rules.layers[0].probe_budget, 8);
        assert_eq!(rules.layers[3].probe_budget, 7);
        assert_eq!(rules.layers[7].probe_budget, 7);
        assert_eq!(BreachRole::from_index(2), Some(BreachRole::Root));
        assert_eq!(rules.overkill(2_000, 1_000), 2);
        assert_eq!(rules.overkill(3_000, 1_000), 3);
        assert_eq!(rules.overkill(10_000, 1_000), 3);
        assert_eq!(rules.overkill(1_999, 1_000), 0);
        assert_eq!(rules.interest(4, 5), 0);
        assert_eq!(rules.interest(25, 5), 5);
        assert_eq!(rules.interest(100, 5), 5);
        assert_eq!(rules.interest(35, 7), 7);
    }
}
