//! One breach: the round played against a single secret.
//!
//! The terminal layer will call [`Breach::probe`] once per guess. This type
//! owns the pegs, the signature score, the budget, and when the round is over.
//! It does not print anything. `secret` is visible to the caller so the rules
//! can be tested; the UI must not show it.

use thiserror::Error;

use crate::catalog::Catalog;
use crate::code::Code;
use crate::layer::LayerSpec;
use crate::pegs::{score_pegs, Pegs};
use crate::scoring::{score_probe, ScoreBreakdown, ScoreHook};
use crate::solver::{count_consistent, is_consistent, Observation, SolverError};

/// Where a breach stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BreachStatus {
    /// Probes remain and the secret is not yet cracked.
    Open,
    /// A probe scored a hit in every slot.
    Cracked,
    /// The probe budget was spent without cracking the secret.
    Failed,
}

/// One probe and everything the rules derived from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Turn {
    /// The probe the player submitted.
    pub probe: Code,
    /// Hits and nears against the secret.
    pub pegs: Pegs,
    /// Signature score, including any tool hooks.
    pub score: ScoreBreakdown,
}

/// A round against one secret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Breach {
    spec: LayerSpec,
    secret: Code,
    history: Vec<Turn>,
    status: BreachStatus,
}

impl Breach {
    /// Start a breach. `secret` must already fit `spec` (generation does this).
    pub fn new(spec: LayerSpec, secret: Code) -> Result<Self, BreachError> {
        if secret.len() != spec.slots() || !secret.within_charset(spec.symbols()) {
            return Err(BreachError::Secret);
        }
        let status = if spec.probe_budget() == 0 {
            BreachStatus::Failed
        } else {
            BreachStatus::Open
        };
        Ok(Self {
            spec,
            secret,
            history: Vec::new(),
            status,
        })
    }

    /// Layer rules for this round.
    pub fn spec(&self) -> LayerSpec {
        self.spec
    }

    /// The secret. The terminal UI must not render this.
    pub fn secret(&self) -> &Code {
        &self.secret
    }

    /// Round status.
    pub fn status(&self) -> BreachStatus {
        self.status
    }

    /// Whether the player can still probe.
    pub fn is_open(&self) -> bool {
        self.status == BreachStatus::Open
    }

    /// Probes already played.
    pub fn probes_used(&self) -> usize {
        self.history.len()
    }

    /// Probe budget still unused. Cracking early leaves the remainder unused.
    pub fn probes_left(&self) -> usize {
        usize::from(self.spec.probe_budget()).saturating_sub(self.history.len())
    }

    /// Probes played so far, oldest first.
    pub fn history(&self) -> &[Turn] {
        &self.history
    }

    /// Play `probe`. `hooks` are the tool effects active for this probe.
    pub fn probe(
        &mut self,
        probe: &Code,
        catalog: &Catalog,
        hooks: &[&dyn ScoreHook],
    ) -> Result<&Turn, BreachError> {
        if self.status != BreachStatus::Open {
            return Err(BreachError::Closed);
        }
        if probe.len() != self.spec.slots() {
            return Err(BreachError::Length {
                got: probe.len(),
                expected: self.spec.slots(),
            });
        }
        if !probe.within_charset(self.spec.symbols()) {
            return Err(BreachError::Charset);
        }
        let pegs = score_pegs(&self.secret, probe).expect("lengths were checked");
        let score = score_probe(probe, pegs, catalog, Some(self.spec), hooks);
        self.history.push(Turn {
            probe: *probe,
            pegs,
            score,
        });
        if pegs.all_hits(self.spec.slots()) {
            self.status = BreachStatus::Cracked;
        } else if self.history.len() >= usize::from(self.spec.probe_budget()) {
            self.status = BreachStatus::Failed;
        }
        Ok(self.history.last().expect("the probe was just recorded"))
    }

    /// Secrets still consistent with every probe in this breach.
    pub fn consistent_count(&self) -> Result<u64, SolverError> {
        count_consistent(self.spec.slots(), self.spec.symbols(), &self.observations())
    }

    /// Whether `candidate` still fits the feedback so far.
    pub fn is_consistent(&self, candidate: &Code) -> Result<bool, SolverError> {
        is_consistent(candidate, &self.observations())
    }

    fn observations(&self) -> Vec<Observation> {
        self.history
            .iter()
            .map(|turn| Observation {
                probe: turn.probe,
                pegs: turn.pegs,
            })
            .collect()
    }
}

/// A probe was refused, or the secret did not fit the layer.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BreachError {
    /// The secret's length or charset does not match the layer.
    #[error("secret does not fit this layer")]
    Secret,
    /// The breach is cracked or out of probes.
    #[error("breach is already over")]
    Closed,
    /// The probe had the wrong number of slots.
    #[error("probe has {got} slots, this layer has {expected}")]
    Length {
        /// Slots in the probe.
        got: usize,
        /// Slots the layer requires.
        expected: usize,
    },
    /// The probe used a symbol this layer does not have.
    #[error("probe uses a symbol outside this layer's charset")]
    Charset,
}

#[cfg(test)]
mod tests {
    use super::{Breach, BreachError, BreachStatus};
    use crate::amp::Amp;
    use crate::catalog::embedded_catalog;
    use crate::code::{Charset, Code};
    use crate::layer::LayerSpec;
    use crate::scoring::{ScoreHook, ToolMod};

    fn code(symbols: usize, text: &str) -> Code {
        Code::parse(Charset::new(symbols).unwrap(), text).unwrap()
    }

    #[test]
    fn cracking_on_the_last_probe_wins_and_a_miss_fails() {
        let spec = LayerSpec::for_layer(1).unwrap().with_probe_budget(2);
        let mut breach = Breach::new(spec, code(6, "0123")).unwrap();
        let catalog = embedded_catalog();
        assert_eq!(breach.consistent_count().unwrap(), 1296);
        assert_eq!(breach.probes_left(), 2);

        let turn = breach.probe(&code(6, "0012"), catalog, &[]).unwrap();
        assert_eq!(turn.score.signature_id, "pair");
        assert!(turn.pegs.hits < 4);
        assert!(breach.is_open());
        assert!(breach.consistent_count().unwrap() < 1296);
        assert!(breach.is_consistent(breach.secret()).unwrap());

        let cracking = breach.probe(&code(6, "0123"), catalog, &[]).unwrap();
        assert!(cracking.pegs.all_hits(4));
        assert_eq!(breach.status(), BreachStatus::Cracked);
        assert_eq!(breach.consistent_count().unwrap(), 1);
        assert_eq!(
            breach.probe(&code(6, "0123"), catalog, &[]),
            Err(BreachError::Closed)
        );
    }

    #[test]
    fn spending_the_budget_without_a_hit_in_every_slot_fails() {
        let spec = LayerSpec::for_layer(1).unwrap().with_probe_budget(1);
        let mut breach = Breach::new(spec, code(6, "0123")).unwrap();
        breach
            .probe(&code(6, "0000"), embedded_catalog(), &[])
            .unwrap();
        assert_eq!(breach.status(), BreachStatus::Failed);
        assert_eq!(breach.probes_left(), 0);
    }

    #[test]
    fn probe_shape_is_checked_and_hooks_reach_the_score() {
        let spec = LayerSpec::for_layer(1).unwrap();
        assert_eq!(
            Breach::new(spec, code(6, "01234")),
            Err(BreachError::Secret)
        );
        let mut breach = Breach::new(spec, code(6, "0123")).unwrap();
        let catalog = embedded_catalog();
        assert_eq!(
            breach.probe(&code(6, "01234"), catalog, &[]).unwrap_err(),
            BreachError::Length {
                got: 5,
                expected: 4
            }
        );
        assert_eq!(
            breach.probe(&code(8, "0126"), catalog, &[]).unwrap_err(),
            BreachError::Charset
        );
        let spike = ToolMod {
            id: "spike".to_string(),
            bits: 4,
            amp: Amp::ZERO,
        };
        let hooks: [&dyn ScoreHook; 1] = [&spike];
        // 0132 against 0123 is two hits and two nears, plus the +4 bit tool.
        // bits = 10 signature + 5×2 hits + 2×2 nears + 4 tool = 28.
        let turn = breach.probe(&code(6, "0132"), catalog, &hooks).unwrap();
        assert_eq!(turn.pegs.hits, 2);
        assert_eq!(turn.pegs.nears, 2);
        assert_eq!(turn.score.signature_id, "all_unique");
        assert_eq!(turn.score.bits, 28);
    }

    #[test]
    fn a_zero_budget_starts_failed() {
        let spec = LayerSpec::for_layer(1).unwrap().with_probe_budget(0);
        let breach = Breach::new(spec, code(6, "0123")).unwrap();
        assert_eq!(breach.status(), BreachStatus::Failed);
    }
}
