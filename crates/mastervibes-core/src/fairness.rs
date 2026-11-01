//! Fairness guardrail for secret generation.
//!
//! A draw is rejected, and the next code is taken from the same PRNG stream,
//! when either check fails:
//!
//! * **Repeat cap.** No symbol appears more than [`LayerSpec::max_repeat`]
//!   times (2 on every stock layer). Heavy repeats make hit/near feedback
//!   ambiguous, so those secrets are treated as too hard for the probe budget.
//! * **Solver bound.** The deterministic solver in [`crate::probes_to_solve`]
//!   must crack the secret within the layer's probe budget. This is a cheap
//!   stand-in for "a careful player can finish", not a full minimax.

use crate::code::Code;
use crate::layer::LayerSpec;
use crate::solver::{probes_to_solve, SolverError};

/// Why a drawn secret was thrown back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnfairReason {
    /// Length or charset does not match the layer.
    Shape,
    /// One symbol appears more than the layer allows.
    Repeats {
        /// Highest frequency in the code.
        max: u8,
        /// The layer cap.
        limit: u8,
    },
    /// The built-in solver could not finish inside the probe budget.
    Solver {
        /// Probe budget it failed to meet.
        budget: u8,
    },
}

/// Whether `code` is an acceptable secret for `spec`.
pub fn is_fair(code: &Code, spec: LayerSpec) -> bool {
    unfair_reason(code, spec).is_none()
}

/// `None` when the code is fair. Repeat failures are reported before the
/// solver runs, so a mono secret does not pay for a search.
pub fn unfair_reason(code: &Code, spec: LayerSpec) -> Option<UnfairReason> {
    if code.len() != spec.slots() || !code.within_charset(spec.symbols()) {
        return Some(UnfairReason::Shape);
    }
    let max = code.max_frequency();
    if max > spec.max_repeat() {
        return Some(UnfairReason::Repeats {
            max,
            limit: spec.max_repeat(),
        });
    }
    if spec.probe_budget() == 0 {
        return Some(UnfairReason::Solver { budget: 0 });
    }
    match probes_to_solve(code, spec.symbols(), u32::from(spec.probe_budget())) {
        Ok(Some(_)) => None,
        Ok(None) => Some(UnfairReason::Solver {
            budget: spec.probe_budget(),
        }),
        Err(
            SolverError::SymbolRange
            | SolverError::Length(_)
            | SolverError::Symbols(_)
            | SolverError::FeedbackLength,
        ) => Some(UnfairReason::Shape),
        Err(SolverError::Space(_)) => Some(UnfairReason::Solver {
            budget: spec.probe_budget(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::{is_fair, unfair_reason, UnfairReason};
    use crate::code::{Charset, Code};
    use crate::layer::LayerSpec;

    fn code(text: &str) -> Code {
        Code::parse(Charset::full(), text).unwrap()
    }

    #[test]
    fn repeat_cap_rejects_triples_and_monos_before_the_solver() {
        let layer = LayerSpec::for_layer(1).unwrap();
        assert_eq!(
            unfair_reason(&code("0000"), layer),
            Some(UnfairReason::Repeats { max: 4, limit: 2 })
        );
        assert_eq!(
            unfair_reason(&code("0001"), layer),
            Some(UnfairReason::Repeats { max: 3, limit: 2 })
        );
        assert!(is_fair(&code("0123"), layer));
        assert!(is_fair(&code("0011"), layer));
    }

    #[test]
    fn solver_bound_rejects_a_code_past_the_budget() {
        // 4543 takes the lex solver 9 probes. Stock layer 1 allows 8, so the
        // solver bound rejects it. A budget of 10 still accepts it. Repeats
        // are in range (the symbol 4 appears twice), so this is the solver
        // check and not the repeat cap.
        let layer = LayerSpec::for_layer(1).unwrap();
        assert_eq!(
            unfair_reason(&code("4543"), layer),
            Some(UnfairReason::Solver { budget: 8 })
        );
        assert!(is_fair(&code("4543"), layer.with_probe_budget(10)));
        assert_eq!(
            unfair_reason(&code("0123"), layer.with_probe_budget(0)),
            Some(UnfairReason::Solver { budget: 0 })
        );
    }

    #[test]
    fn shape_mismatches_are_unfair() {
        let layer = LayerSpec::for_layer(1).unwrap();
        assert_eq!(
            unfair_reason(&code("01234"), layer),
            Some(UnfairReason::Shape)
        );
        assert_eq!(
            unfair_reason(&code("0126"), layer),
            Some(UnfairReason::Shape)
        );
    }
}
