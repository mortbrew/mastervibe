//! Mastermind pegs.
//!
//! A **hit** is the right symbol in the right slot. A **near** is the right
//! symbol in the wrong slot. Each secret symbol is matched at most once, and
//! hits are taken before nears, so duplicate probes cannot invent extra pegs.

use std::hash::{Hash, Hasher};

use thiserror::Error;

use crate::code::{Code, MAX_SYMBOLS};

/// Feedback for one probe.
///
/// Equality and hashing use the counts only. `exact_mask` says which slots
/// were exact, which neighbour amp needs, but a player is told the counts.
#[derive(Clone, Copy, Debug)]
pub struct Pegs {
    /// Right symbol, right slot. An exact.
    pub hits: u8,
    /// Right symbol, wrong slot.
    pub nears: u8,
    /// Bit `i` is set when slot `i` is an exact.
    pub exact_mask: u8,
}

impl PartialEq for Pegs {
    fn eq(&self, other: &Self) -> bool {
        self.hits == other.hits && self.nears == other.nears
    }
}

impl Eq for Pegs {}

impl Hash for Pegs {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hits.hash(state);
        self.nears.hash(state);
    }
}

impl Pegs {
    /// Peg counts with no exact slots recorded.
    ///
    /// Neighbour amp stays at zero until a mask is filled in. [`score_pegs`]
    /// sets the mask.
    pub fn counts(hits: u8, nears: u8) -> Self {
        Self {
            hits,
            nears,
            exact_mask: 0,
        }
    }

    /// Every slot is an exact, which is how a breach is cracked.
    pub fn all_hits(self, slots: usize) -> bool {
        usize::from(self.hits) == slots && self.nears == 0
    }

    /// Whether slot `slot` is an exact.
    pub fn exact_at(self, slot: usize) -> bool {
        slot < 8 && (self.exact_mask & (1 << slot)) != 0
    }

    /// Pairs of side-by-side exacts. Three exacts in a row are two pairs.
    pub fn adjacent_exact_pairs(self) -> u8 {
        let mut pairs = 0u8;
        for slot in 0..7 {
            if self.exact_at(slot) && self.exact_at(slot + 1) {
                pairs += 1;
            }
        }
        pairs
    }
}

impl std::fmt::Display for Pegs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "hits={} nears={}", self.hits, self.nears)
    }
}

/// [`score_pegs`] could not compare the two codes.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PegError {
    /// Secret and probe must be the same length.
    #[error("secret has {secret} slots and the probe has {probe}")]
    LengthMismatch {
        /// Slots in the secret.
        secret: usize,
        /// Slots in the probe.
        probe: usize,
    },
}

/// Score `probe` as a guess of `secret`.
///
/// The result is symmetric: scoring either code against the other yields the
/// same pegs. A symbol that already scored a hit is not also counted as a near.
pub fn score_pegs(secret: &Code, probe: &Code) -> Result<Pegs, PegError> {
    if secret.len() != probe.len() {
        return Err(PegError::LengthMismatch {
            secret: secret.len(),
            probe: probe.len(),
        });
    }
    Ok(score_slots(secret.as_slice(), probe.as_slice()))
}

/// Score two equal-length index slices. Callers guarantee the lengths match.
pub(crate) fn score_slots(secret: &[u8], probe: &[u8]) -> Pegs {
    debug_assert_eq!(secret.len(), probe.len());
    let slots = secret.len();
    let mut hits = 0u8;
    let mut exact_mask = 0u8;
    let mut secret_counts = [0u8; MAX_SYMBOLS];
    let mut probe_counts = [0u8; MAX_SYMBOLS];
    for (slot, (&secret_symbol, &probe_symbol)) in secret.iter().zip(probe.iter()).enumerate() {
        if secret_symbol == probe_symbol {
            hits += 1;
            exact_mask |= 1 << slot;
        } else {
            secret_counts[usize::from(secret_symbol)] += 1;
            probe_counts[usize::from(probe_symbol)] += 1;
        }
    }
    let mut nears = 0u8;
    for symbol in 0..MAX_SYMBOLS {
        nears += secret_counts[symbol].min(probe_counts[symbol]);
    }
    debug_assert!(usize::from(hits) + usize::from(nears) <= slots);
    debug_assert!(slots < 2 || hits != (slots as u8) - 1 || nears != 1);
    Pegs {
        hits,
        nears,
        exact_mask,
    }
}

#[cfg(test)]
mod tests {
    use super::{score_pegs, score_slots, PegError, Pegs};
    use crate::code::{Charset, Code};

    fn code(text: &str) -> Code {
        Code::parse(Charset::full(), text).unwrap()
    }

    #[test]
    fn named_duplicate_cases() {
        let cases = [
            ("1123", "1123", 4, 0),
            ("1123", "1111", 2, 0),
            ("1111", "1122", 2, 0),
            ("1112", "2221", 0, 2),
            ("1122", "2211", 0, 4),
            ("1122", "1234", 1, 1),
            ("0012", "0103", 1, 2),
            ("0123", "0000", 1, 0),
            ("0123", "3210", 0, 4),
            ("0123", "4567", 0, 0),
            ("1221", "1122", 2, 2),
        ];
        for (secret, probe, hits, nears) in cases {
            let pegs = score_pegs(&code(secret), &code(probe)).unwrap();
            assert_eq!(pegs.hits, hits, "{secret} vs {probe} hits");
            assert_eq!(pegs.nears, nears, "{secret} vs {probe} nears");
        }
    }

    #[test]
    fn length_mismatch_is_an_error() {
        let err = score_pegs(&code("0123"), &code("01234")).unwrap_err();
        assert_eq!(
            err,
            PegError::LengthMismatch {
                secret: 4,
                probe: 5
            }
        );
    }

    #[test]
    fn adjacent_exacts_count_overlapping_pairs() {
        let pegs = score_pegs(&code("0123"), &code("0123")).unwrap();
        assert_eq!(pegs.exact_mask, 0b1111);
        assert_eq!(pegs.adjacent_exact_pairs(), 3);
        let split = score_pegs(&code("0123"), &code("0132")).unwrap();
        // slots 0 and 1 match; 2 and 3 are swapped
        assert_eq!(split.hits, 2);
        assert_eq!(split.exact_mask, 0b0011);
        assert_eq!(split.adjacent_exact_pairs(), 1);
        assert_eq!(Pegs::counts(2, 1).adjacent_exact_pairs(), 0);
    }

    #[test]
    fn reference_marker_algorithm_agrees_on_every_4x6_pair() {
        let symbols = 6u8;
        let slots = 4usize;
        let total = usize::from(symbols).pow(slots as u32);
        for secret_id in 0..total {
            let secret = unpack(secret_id, slots, symbols);
            for probe_id in 0..total {
                let probe = unpack(probe_id, slots, symbols);
                let pegs = score_slots(&secret, &probe);
                let (hits, nears) = reference(&secret, &probe);
                assert_eq!((pegs.hits, pegs.nears), (hits, nears));
                let flipped = score_slots(&probe, &secret);
                assert_eq!(pegs, flipped);
                assert_eq!(pegs.exact_mask, flipped.exact_mask);
                let impossible = pegs.hits == (slots as u8) - 1 && pegs.nears == 1;
                assert!(!impossible);
                assert!(usize::from(pegs.hits) + usize::from(pegs.nears) <= slots);
            }
        }
    }

    fn unpack(mut id: usize, slots: usize, symbols: u8) -> [u8; 4] {
        let mut digits = [0u8; 4];
        for slot in (0..slots).rev() {
            digits[slot] = u8::try_from(id % usize::from(symbols)).unwrap();
            id /= usize::from(symbols);
        }
        digits
    }

    /// Independent scorer: mark hits, then greedily mark nears.
    fn reference(secret: &[u8], probe: &[u8]) -> (u8, u8) {
        let slots = secret.len();
        let mut used_secret = [false; 6];
        let mut used_probe = [false; 6];
        let mut hits = 0u8;
        for slot in 0..slots {
            if secret[slot] == probe[slot] {
                hits += 1;
                used_secret[slot] = true;
                used_probe[slot] = true;
            }
        }
        let mut nears = 0u8;
        for probe_slot in 0..slots {
            if used_probe[probe_slot] {
                continue;
            }
            for secret_slot in 0..slots {
                if !used_secret[secret_slot] && secret[secret_slot] == probe[probe_slot] {
                    used_secret[secret_slot] = true;
                    nears += 1;
                    break;
                }
            }
        }
        (hits, nears)
    }
}
