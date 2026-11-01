//! Consistent-code helper and a small deterministic solver.
//!
//! [`count_consistent`] is the question later tools want to ask: how many
//! secrets still fit every probe so far? [`probes_to_solve`] plays a fixed
//! strategy (always the lexicographically smallest code still in that set)
//! and reports how many probes it needs. The fairness check uses that number
//! as a cheap worst-case bound.

use thiserror::Error;

use crate::code::{Code, MAX_SLOTS, MAX_SYMBOLS};
use crate::pegs::{score_slots, Pegs};

/// Largest space the helper will scan. `8^6`, the layer 7–8 secret space.
pub const SOLVER_MAX_SPACE: u64 = 262_144;

/// One previous probe and the pegs it produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observation {
    /// The probe that was played.
    pub probe: Code,
    /// Pegs the secret returned for that probe.
    pub pegs: Pegs,
}

/// The helper refused the request.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SolverError {
    /// Slot count was outside `1..=MAX_SLOTS`.
    #[error("solver slot count {0} is outside 1..={MAX_SLOTS}")]
    Length(usize),
    /// Charset size was outside `1..=MAX_SYMBOLS`.
    #[error("solver symbol count {0} is outside 1..={MAX_SYMBOLS}")]
    Symbols(usize),
    /// `symbols ^ slots` is above [`SOLVER_MAX_SPACE`].
    #[error("search space {0} exceeds the solver limit {SOLVER_MAX_SPACE}")]
    Space(u64),
    /// A feedback probe was not the same length as the secret.
    #[error("feedback probe does not match the slot count")]
    FeedbackLength,
    /// A code used a symbol outside the charset being searched.
    #[error("code uses a symbol outside the charset")]
    SymbolRange,
}

/// How many codes of this shape still produce every observation.
///
/// An empty `feedback` slice counts the whole space (`symbols ^ slots`).
pub fn count_consistent(
    slots: usize,
    symbols: usize,
    feedback: &[Observation],
) -> Result<u64, SolverError> {
    let total = space(slots, symbols)?;
    validate_feedback(slots, symbols, feedback)?;
    let mut digits = [0u8; MAX_SLOTS];
    let mut count = 0u64;
    for _ in 0..total {
        if fits(&digits[..slots], feedback) {
            count += 1;
        }
        bump(&mut digits, slots, symbols);
    }
    Ok(count)
}

/// Whether `candidate` would have produced every observation if it were the secret.
pub fn is_consistent(candidate: &Code, feedback: &[Observation]) -> Result<bool, SolverError> {
    for observation in feedback {
        if observation.probe.len() != candidate.len() {
            return Err(SolverError::FeedbackLength);
        }
        if score_slots(candidate.as_slice(), observation.probe.as_slice()) != observation.pegs {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Probes the built-in solver needs to crack `secret`.
///
/// The solver guesses the lexicographically smallest code that is still
/// consistent with the pegs so far (last slot varies fastest). `symbols` is
/// the charset size; the secret's own length is the slot count.
///
/// Returns `Ok(None)` when the secret is not cracked within `limit` probes.
pub fn probes_to_solve(
    secret: &Code,
    symbols: usize,
    limit: u32,
) -> Result<Option<u32>, SolverError> {
    let slots = secret.len();
    let _total = space(slots, symbols)?;
    if !secret.within_charset(symbols) {
        return Err(SolverError::SymbolRange);
    }
    if limit == 0 {
        return Ok(None);
    }

    let mut remaining = enumerate(slots, symbols);
    for step in 1..=limit {
        if remaining.is_empty() {
            return Ok(None);
        }
        let guess = remaining[0];
        if &guess.sym[..slots] == secret.as_slice() {
            return Ok(Some(step));
        }
        let peg = score_slots(secret.as_slice(), &guess.sym[..slots]);
        let mut write = 0;
        for read in 0..remaining.len() {
            let candidate = remaining[read];
            if score_slots(&candidate.sym[..slots], &guess.sym[..slots]) == peg {
                remaining[write] = candidate;
                write += 1;
            }
        }
        remaining.truncate(write);
    }
    Ok(None)
}

#[derive(Clone, Copy)]
struct Candidate {
    sym: [u8; MAX_SLOTS],
}

fn enumerate(slots: usize, symbols: usize) -> Vec<Candidate> {
    let total = symbols.pow(u32::try_from(slots).unwrap_or(0));
    let mut digits = [0u8; MAX_SLOTS];
    let mut out = Vec::with_capacity(total);
    for _ in 0..total {
        out.push(Candidate { sym: digits });
        bump(&mut digits, slots, symbols);
    }
    out
}

fn fits(candidate: &[u8], feedback: &[Observation]) -> bool {
    feedback
        .iter()
        .all(|observation| score_slots(candidate, observation.probe.as_slice()) == observation.pegs)
}

fn bump(digits: &mut [u8], slots: usize, symbols: usize) {
    for slot in (0..slots).rev() {
        digits[slot] = digits[slot].wrapping_add(1);
        if usize::from(digits[slot]) < symbols {
            return;
        }
        digits[slot] = 0;
    }
}

fn space(slots: usize, symbols: usize) -> Result<u64, SolverError> {
    if !(1..=MAX_SLOTS).contains(&slots) {
        return Err(SolverError::Length(slots));
    }
    if !(1..=MAX_SYMBOLS).contains(&symbols) {
        return Err(SolverError::Symbols(symbols));
    }
    let size = u64::from(u8::try_from(symbols).unwrap_or(u8::MAX))
        .checked_pow(u32::try_from(slots).unwrap_or(u32::MAX))
        .ok_or(SolverError::Space(u64::MAX))?;
    if size > SOLVER_MAX_SPACE {
        return Err(SolverError::Space(size));
    }
    Ok(size)
}

fn validate_feedback(
    slots: usize,
    symbols: usize,
    feedback: &[Observation],
) -> Result<(), SolverError> {
    for observation in feedback {
        if observation.probe.len() != slots {
            return Err(SolverError::FeedbackLength);
        }
        if !observation.probe.within_charset(symbols) {
            return Err(SolverError::SymbolRange);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{count_consistent, is_consistent, probes_to_solve, Observation, SolverError};
    use crate::code::{Charset, Code};
    use crate::pegs::{score_pegs, Pegs};

    fn code(text: &str) -> Code {
        Code::parse(Charset::full(), text).unwrap()
    }

    #[test]
    fn lex_solver_matches_known_depths_for_4x6() {
        let cases = [
            ("0000", 1u32),
            ("0001", 2),
            ("2222", 3),
            ("0123", 4),
            ("5432", 8),
            ("4543", 9),
            ("5554", 9),
        ];
        for (secret, probes) in cases {
            let secret = code(secret);
            assert_eq!(probes_to_solve(&secret, 6, 10).unwrap(), Some(probes));
            assert_eq!(probes_to_solve(&secret, 6, probes - 1).unwrap(), None);
        }
        assert_eq!(probes_to_solve(&code("0123"), 6, 0).unwrap(), None);
    }

    #[test]
    fn empty_feedback_is_the_whole_space_and_a_solved_probe_leaves_one() {
        assert_eq!(count_consistent(4, 6, &[]).unwrap(), 1296);
        assert_eq!(count_consistent(2, 2, &[]).unwrap(), 4);
        let probe = code("0123");
        let pegs = score_pegs(&probe, &probe).unwrap();
        let left = count_consistent(4, 6, &[Observation { probe, pegs }]).unwrap();
        assert_eq!(left, 1);

        let impossible = Observation {
            probe: code("0123"),
            pegs: Pegs { hits: 3, nears: 1 },
        };
        assert_eq!(count_consistent(4, 6, &[impossible]).unwrap(), 0);
    }

    #[test]
    fn honest_feedback_keeps_the_secret_and_matches_a_brute_filter() {
        let secret = code("0123");
        let probes = ["0000", "0011", "0123"];
        let feedback: Vec<Observation> = probes
            .into_iter()
            .map(|probe| {
                let probe = code(probe);
                Observation {
                    pegs: score_pegs(&secret, &probe).unwrap(),
                    probe,
                }
            })
            .collect();
        assert!(is_consistent(&secret, &feedback).unwrap());
        assert!(!is_consistent(&code("0124"), &feedback).unwrap());

        let mut brute = 0u64;
        for id in 0..6u32.pow(4) {
            let candidate = code(&unpack(id, 4, 6));
            if is_consistent(&candidate, &feedback).unwrap() {
                brute += 1;
            }
        }
        assert_eq!(count_consistent(4, 6, &feedback).unwrap(), brute);
        assert!(brute >= 1);
    }

    #[test]
    fn rejects_a_space_or_a_code_the_helper_cannot_search() {
        assert_eq!(count_consistent(0, 6, &[]), Err(SolverError::Length(0)));
        assert_eq!(count_consistent(4, 0, &[]), Err(SolverError::Symbols(0)));
        assert!(matches!(
            probes_to_solve(&code("0126"), 6, 5),
            Err(SolverError::SymbolRange)
        ));
        let err = count_consistent(
            4,
            6,
            &[Observation {
                probe: code("01234"),
                pegs: Pegs { hits: 0, nears: 0 },
            }],
        );
        assert_eq!(err, Err(SolverError::FeedbackLength));
    }

    fn unpack(mut id: u32, slots: usize, symbols: u32) -> String {
        let mut digits = vec![0u8; slots];
        for slot in (0..slots).rev() {
            digits[slot] = u8::try_from(id % symbols).unwrap();
            id /= symbols;
        }
        digits
            .into_iter()
            .map(|digit| char::from(b'0' + digit))
            .collect()
    }
}
