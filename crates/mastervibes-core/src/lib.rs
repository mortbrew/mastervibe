//! Rules core for mastervibes, a code-breaking roguelike.
//!
//! A **probe** is a guess. **Pegs** are the feedback: a **hit** is the right
//! symbol in the right slot, a **near** is the right symbol in the wrong slot.
//! The **charset** is the symbol set. A **signature** is the pattern of a
//! probe. **Bits** are additive points, **amp** is the multiplier, and
//! `score = bits × amp`. A **layer** is one stage of an eight-layer run, and a
//! **breach** is the round played against that layer's secret.
//!
//! This crate is pure logic. It does not read the terminal.
//!
//! ```
//! use mastervibes_core::{generate_secret, RunSeed};
//!
//! let seed = RunSeed::parse("MV-7F3A-9C21-L4")?;
//! let secret = generate_secret(seed);
//! assert!(secret.fair);
//! assert_eq!(secret.spec.layer(), 4);
//! assert_eq!(seed.to_string(), "MV-7F3A-9C21-L4");
//! # Ok::<(), mastervibes_core::SeedError>(())
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod pattern;

pub mod amp;
pub mod breach;
pub mod catalog;
pub mod code;
pub mod fairness;
pub mod generate;
pub mod layer;
pub mod pegs;
pub mod scoring;
pub mod seed;
pub mod solver;

pub use amp::Amp;
pub use breach::{Breach, BreachError, BreachStatus, Turn};
pub use catalog::{embedded_catalog, Catalog, CatalogError, ScoringRules, Signature};
pub use code::{Charset, Code, CodeError, Symbol, GLYPHS, MAX_SLOTS, MAX_SYMBOLS};
pub use fairness::{is_fair, unfair_reason, UnfairReason};
pub use generate::{
    generate_from_draws, generate_secret, plan_run, GeneratedCode, RunPlan, MAX_FAIR_ATTEMPTS,
};
pub use layer::{LayerSpec, LAYER_COUNT};
pub use pegs::{score_pegs, PegError, Pegs};
pub use scoring::{
    score_probe, RunningScore, ScoreBreakdown, ScoreContext, ScoreHook, ScoreLine, ScoreLineKind,
    ToolMod,
};
pub use seed::{RunSeed, SeedError};
pub use solver::{
    count_consistent, is_consistent, probes_to_solve, Observation, SolverError, SOLVER_MAX_SPACE,
};
