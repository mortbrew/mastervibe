//! Bits × amp pipeline.
//!
//! Order, always:
//!
//! 1. Start from the catalog's base amp.
//! 2. Add the winning signature's bits and amp bonus.
//! 3. Add hit and near contributions from the catalog rates.
//! 4. Run tool hooks, each of which may read the probe and change the running
//!    totals (add, replace, or multiply).
//! 5. Clamp bits and amp at zero.
//!
//! `total_milli` is `bits × amp_milli`, so a score of 62.5 is stored as
//! `62500`. Divide by 1000 for the player-facing number.

use crate::amp::Amp;
use crate::catalog::{Catalog, Signature};
use crate::code::Code;
use crate::layer::LayerSpec;
use crate::pegs::Pegs;

/// What a tool sees when it adjusts a probe's score.
#[derive(Clone, Copy, Debug)]
pub struct ScoreContext<'a> {
    /// The probe that was just played.
    pub probe: &'a Code,
    /// Pegs that probe earned.
    pub pegs: Pegs,
    /// Signature that won detection.
    pub signature: &'a Signature,
    /// Layer the breach belongs to, when the caller knows it.
    pub layer: Option<LayerSpec>,
    /// Catalog the rates came from.
    pub catalog: &'a Catalog,
}

/// Running totals handed to a hook. Hooks may add to these or overwrite them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunningScore {
    /// Bits so far.
    pub bits: i32,
    /// Amp so far, in thousandths.
    pub amp: Amp,
}

/// Something that can change bits and amp after the base, signature, and pegs.
///
/// Tools are not in this crate yet. The trait is the slot they plug into:
/// implement it, and pass the hooks to [`score_probe`] in the order they
/// should fire.
pub trait ScoreHook {
    /// Stable id recorded on the score line.
    fn id(&self) -> &str;

    /// Adjust `running`. Called after signature and peg contributions.
    fn apply(&self, ctx: &ScoreContext<'_>, running: &mut RunningScore);
}

/// A flat bits / amp bonus. Enough for tests and for tools whose whole effect
/// is "add these numbers".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolMod {
    /// Tool id stored on the score line.
    pub id: String,
    /// Bits to add. Negative values are a penalty.
    pub bits: i32,
    /// Amp to add. Negative values are a penalty.
    pub amp: Amp,
}

impl ScoreHook for ToolMod {
    fn id(&self) -> &str {
        &self.id
    }

    fn apply(&self, _ctx: &ScoreContext<'_>, running: &mut RunningScore) {
        running.bits = running.bits.saturating_add(self.bits);
        running.amp = running.amp.saturating_add(self.amp);
    }
}

/// One step in the pipeline, as a net change from the previous step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreLine {
    /// Which step produced the change.
    pub kind: ScoreLineKind,
    /// Net bits added by this step.
    pub bits: i32,
    /// Net amp added by this step.
    pub amp: Amp,
}

/// Where a [`ScoreLine`] came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScoreLineKind {
    /// The catalog's base amp.
    Base,
    /// The winning signature's base bits and amp bonus.
    Signature,
    /// `count` hits, at the catalog's per-hit rate.
    Hits {
        /// How many hits the probe scored.
        count: u8,
    },
    /// `count` nears, at the catalog's per-near rate.
    Nears {
        /// How many nears the probe scored.
        count: u8,
    },
    /// A tool hook. `bits` / `amp` on the line are the net change it caused.
    Tool {
        /// [`ScoreHook::id`] of the hook.
        id: String,
    },
    /// Bits or amp was brought back up to zero.
    Clamp,
}

/// The full score for one probe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreBreakdown {
    /// Id of the signature that won.
    pub signature_id: String,
    /// Player-facing signature name.
    pub signature_name: String,
    /// Pipeline steps, in the order they were applied.
    pub lines: Vec<ScoreLine>,
    /// Final bits, after hooks and the clamp.
    pub bits: i32,
    /// Final amp, after hooks and the clamp.
    pub amp: Amp,
    /// `bits × amp` in thousandths of a point.
    pub total_milli: i64,
}

impl ScoreBreakdown {
    /// Player-facing score. `62500` thousandths is `62.5`.
    pub fn points(&self) -> f64 {
        self.total_milli as f64 / 1000.0
    }
}

/// Score `probe` given the pegs it already earned.
///
/// `hooks` run in order after the signature and peg lines. Pass `&[]` when
/// nothing is modifying the probe.
pub fn score_probe(
    probe: &Code,
    pegs: Pegs,
    catalog: &Catalog,
    layer: Option<LayerSpec>,
    hooks: &[&dyn ScoreHook],
) -> ScoreBreakdown {
    let signature = catalog.detect(probe);
    let rules = catalog.scoring();
    let ctx = ScoreContext {
        probe,
        pegs,
        signature,
        layer,
        catalog,
    };

    let mut running = RunningScore {
        bits: 0,
        amp: Amp::ZERO,
    };
    let mut lines = Vec::new();

    push(
        &mut lines,
        &mut running,
        ScoreLineKind::Base,
        0,
        rules.base_amp,
    );
    push(
        &mut lines,
        &mut running,
        ScoreLineKind::Signature,
        signature.bits,
        signature.amp,
    );
    if pegs.hits > 0 {
        push(
            &mut lines,
            &mut running,
            ScoreLineKind::Hits { count: pegs.hits },
            rules.hit_bits.saturating_mul(i32::from(pegs.hits)),
            Amp::from_milli(rules.hit_amp.milli().saturating_mul(i32::from(pegs.hits))),
        );
    }
    if pegs.nears > 0 {
        push(
            &mut lines,
            &mut running,
            ScoreLineKind::Nears { count: pegs.nears },
            rules.near_bits.saturating_mul(i32::from(pegs.nears)),
            Amp::from_milli(rules.near_amp.milli().saturating_mul(i32::from(pegs.nears))),
        );
    }

    for hook in hooks {
        let before = running;
        hook.apply(&ctx, &mut running);
        lines.push(ScoreLine {
            kind: ScoreLineKind::Tool {
                id: hook.id().to_string(),
            },
            bits: running.bits.saturating_sub(before.bits),
            amp: running.amp.saturating_sub(before.amp),
        });
    }

    if running.bits < 0 {
        let bits = running.bits.saturating_neg();
        running.bits = 0;
        lines.push(ScoreLine {
            kind: ScoreLineKind::Clamp,
            bits,
            amp: Amp::ZERO,
        });
    }
    if running.amp.milli() < 0 {
        let amp = Amp::from_milli(running.amp.milli().saturating_neg());
        running.amp = Amp::ZERO;
        lines.push(ScoreLine {
            kind: ScoreLineKind::Clamp,
            bits: 0,
            amp,
        });
    }

    let total_milli = i64::from(running.bits).saturating_mul(i64::from(running.amp.milli()));
    ScoreBreakdown {
        signature_id: signature.id.clone(),
        signature_name: signature.name.clone(),
        lines,
        bits: running.bits,
        amp: running.amp,
        total_milli,
    }
}

fn push(
    lines: &mut Vec<ScoreLine>,
    running: &mut RunningScore,
    kind: ScoreLineKind,
    bits: i32,
    amp: Amp,
) {
    running.bits = running.bits.saturating_add(bits);
    running.amp = running.amp.saturating_add(amp);
    lines.push(ScoreLine { kind, bits, amp });
}

#[cfg(test)]
mod tests {
    use super::{score_probe, RunningScore, ScoreContext, ScoreHook, ScoreLineKind, ToolMod};
    use crate::amp::Amp;
    use crate::catalog::{embedded_catalog, Catalog};
    use crate::code::{Charset, Code};
    use crate::pegs::Pegs;

    fn code(text: &str) -> Code {
        Code::parse(Charset::full(), text).unwrap()
    }

    #[test]
    fn all_unique_whiff_is_base_bits_times_base_amp() {
        let catalog = embedded_catalog();
        let score = score_probe(
            &code("0132"),
            Pegs { hits: 0, nears: 0 },
            catalog,
            None,
            &[],
        );
        assert_eq!(score.signature_id, "all_unique");
        assert_eq!(score.bits, 10);
        assert_eq!(score.amp, Amp::ONE);
        assert_eq!(score.total_milli, 10_000);
        assert!((score.points() - 10.0).abs() < 1e-9);
    }

    #[test]
    fn pair_with_pegs_adds_hit_and_near_rates() {
        let catalog = embedded_catalog();
        // bits = 15 + 5×2 + 2×1 = 27
        // amp  = 1 + 1 + 0.25×2 = 2.5
        // score = 67.5
        let score = score_probe(
            &code("0012"),
            Pegs { hits: 2, nears: 1 },
            catalog,
            None,
            &[],
        );
        assert_eq!(score.signature_id, "pair");
        assert_eq!(score.bits, 27);
        assert_eq!(score.amp, Amp::from_milli(2500));
        assert_eq!(score.total_milli, 67_500);
        let summed_bits: i32 = score.lines.iter().map(|line| line.bits).sum();
        let summed_amp: i32 = score.lines.iter().map(|line| line.amp.milli()).sum();
        assert_eq!(summed_bits, score.bits);
        assert_eq!(summed_amp, score.amp.milli());
    }

    #[test]
    fn hooks_can_read_the_signature_and_a_flat_mod_stacks() {
        struct PairBonus;
        impl ScoreHook for PairBonus {
            fn id(&self) -> &str {
                "pair-bonus"
            }
            fn apply(&self, ctx: &ScoreContext<'_>, running: &mut RunningScore) {
                if ctx.signature.id == "pair" {
                    running.bits += 7;
                    running.amp = running.amp.saturating_mul(Amp::from_milli(2000));
                }
            }
        }

        let catalog = embedded_catalog();
        let spike = ToolMod {
            id: "spike".to_string(),
            bits: 3,
            amp: Amp::from_milli(500),
        };
        let bonus = PairBonus;
        let hooks: [&dyn ScoreHook; 2] = [&bonus, &spike];
        let score = score_probe(
            &code("0012"),
            Pegs { hits: 0, nears: 0 },
            catalog,
            None,
            &hooks,
        );
        // signature: 15 bits, amp 2.0
        // pair bonus: +7 bits, amp ×2 → 4.0
        // spike: +3 bits, +0.5 amp → 25 bits, amp 4.5
        assert_eq!(score.bits, 25);
        assert_eq!(score.amp, Amp::from_milli(4500));
        assert_eq!(score.total_milli, 112_500);
        assert!(score.lines.iter().any(|line| matches!(
            &line.kind,
            ScoreLineKind::Tool { id } if id == "pair-bonus"
        )));
    }

    #[test]
    fn rates_come_from_the_catalog_not_from_constants() {
        let mut text = include_str!("../data/signatures.toml").to_string();
        text = text.replace("bits = 15", "bits = 100");
        text = text.replace("hit_bits = 5", "hit_bits = 1");
        text = text.replace("near_amp = 0.0", "near_amp = 0.5");
        let catalog = Catalog::from_toml(&text).unwrap();
        let score = score_probe(
            &code("0012"),
            Pegs { hits: 1, nears: 1 },
            &catalog,
            None,
            &[],
        );
        // bits = 100 + 1 hit + 2 near = 103
        // amp = 1 + 1 + 0.25 hit + 0.5 near = 2.75
        assert_eq!(score.bits, 103);
        assert_eq!(score.amp, Amp::from_milli(2750));
    }

    #[test]
    fn negative_hooks_clamp_at_zero() {
        let catalog = embedded_catalog();
        let sink = ToolMod {
            id: "sink".to_string(),
            bits: -10_000,
            amp: Amp::from_milli(-10_000),
        };
        let hooks: [&dyn ScoreHook; 1] = [&sink];
        let score = score_probe(
            &code("0132"),
            Pegs { hits: 0, nears: 0 },
            catalog,
            None,
            &hooks,
        );
        assert_eq!(score.bits, 0);
        assert_eq!(score.amp, Amp::ZERO);
        assert_eq!(score.total_milli, 0);
        assert!(score
            .lines
            .iter()
            .any(|line| line.kind == ScoreLineKind::Clamp));
    }
}
