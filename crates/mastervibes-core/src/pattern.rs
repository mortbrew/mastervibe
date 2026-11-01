//! Signature detectors.
//!
//! Rewards and priorities live in the TOML catalog. This module only answers
//! "does this probe have this shape?"
//!
//! Frequency patterns partition every probe:
//!
//! | id | shape |
//! | --- | --- |
//! | `mono` | every slot is the same symbol |
//! | `quad` | some symbol fills at least four slots, and at least one slot differs |
//! | `full_lock` | a triple and a pair (leftover slots are singletons) |
//! | `triple` | the heaviest symbol appears three times, and there is no pair (this includes two triples) |
//! | `two_pair` | at least two distinct pairs, and nothing heavier |
//! | `pair` | exactly one pair, everything else unique |
//! | `all_unique` | no repeated symbol |
//!
//! Shape patterns can match on top of a frequency pattern:
//!
//! * `ascending` — each next slot's symbol index is strictly greater
//! * `palindrome` — the probe reads the same forwards and backwards
//!
//! The catalog picks the matching row with the highest `bits × (1 + amp)`,
//! so a probe still has one signature. Priority only breaks a tie.

use crate::code::{Code, MAX_SYMBOLS};

/// Every detector id the loader is required to supply a row for.
pub(crate) const KNOWN_IDS: [&str; 9] = [
    "all_unique",
    "pair",
    "two_pair",
    "triple",
    "full_lock",
    "quad",
    "mono",
    "ascending",
    "palindrome",
];

pub(crate) fn is_known_id(id: &str) -> bool {
    KNOWN_IDS.contains(&id)
}

pub(crate) fn matches(id: &str, probe: &Code) -> bool {
    match id {
        "all_unique" => frequency(probe) == Frequency::AllUnique,
        "pair" => frequency(probe) == Frequency::Pair,
        "two_pair" => frequency(probe) == Frequency::TwoPair,
        "triple" => frequency(probe) == Frequency::Triple,
        "full_lock" => frequency(probe) == Frequency::FullLock,
        "quad" => frequency(probe) == Frequency::Quad,
        "mono" => frequency(probe) == Frequency::Mono,
        "ascending" => is_ascending(probe),
        "palindrome" => is_palindrome(probe),
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Frequency {
    AllUnique,
    Pair,
    TwoPair,
    Triple,
    FullLock,
    Quad,
    Mono,
}

fn frequency(probe: &Code) -> Frequency {
    let mut counts = [0u8; MAX_SYMBOLS];
    for &symbol in probe.as_slice() {
        counts[usize::from(symbol)] += 1;
    }
    let mut max = 0u8;
    let mut pairs = 0u8;
    for count in counts {
        if count > max {
            max = count;
        }
        if count == 2 {
            pairs += 1;
        }
    }
    let slots = u8::try_from(probe.len()).unwrap_or(u8::MAX);
    if max == slots {
        Frequency::Mono
    } else if max >= 4 {
        Frequency::Quad
    } else if max == 3 && pairs >= 1 {
        Frequency::FullLock
    } else if max == 3 {
        Frequency::Triple
    } else if pairs >= 2 {
        Frequency::TwoPair
    } else if pairs == 1 {
        Frequency::Pair
    } else {
        Frequency::AllUnique
    }
}

fn is_ascending(probe: &Code) -> bool {
    probe.as_slice().windows(2).all(|pair| pair[0] < pair[1])
}

fn is_palindrome(probe: &Code) -> bool {
    let slots = probe.as_slice();
    let len = slots.len();
    (0..len / 2).all(|index| slots[index] == slots[len - 1 - index])
}

#[cfg(test)]
mod tests {
    use super::{frequency, matches, Frequency};
    use crate::code::{Charset, Code};

    fn code(text: &str) -> Code {
        Code::parse(Charset::full(), text).unwrap()
    }

    #[test]
    fn frequency_patterns_match_the_documented_shapes() {
        assert_eq!(frequency(&code("0132")), Frequency::AllUnique);
        assert_eq!(frequency(&code("0012")), Frequency::Pair);
        assert_eq!(frequency(&code("0011")), Frequency::TwoPair);
        assert_eq!(frequency(&code("001122")), Frequency::TwoPair);
        assert_eq!(frequency(&code("0001")), Frequency::Triple);
        assert_eq!(frequency(&code("000111")), Frequency::Triple);
        assert_eq!(frequency(&code("00011")), Frequency::FullLock);
        assert_eq!(frequency(&code("00111")), Frequency::FullLock);
        assert_eq!(frequency(&code("00012")), Frequency::Triple);
        assert_eq!(frequency(&code("00001")), Frequency::Quad);
        assert_eq!(frequency(&code("000011")), Frequency::Quad);
        assert_eq!(frequency(&code("000001")), Frequency::Quad);
        assert_eq!(frequency(&code("0000")), Frequency::Mono);
        assert_eq!(frequency(&code("00000")), Frequency::Mono);
        assert_eq!(frequency(&code("000000")), Frequency::Mono);
    }

    #[test]
    fn shapes_are_orthogonal_to_frequency() {
        assert!(matches("ascending", &code("0123")));
        assert!(matches("all_unique", &code("0123")));
        assert!(!matches("ascending", &code("0132")));
        assert!(!matches("ascending", &code("0012")));
        assert!(matches("palindrome", &code("0110")));
        assert!(matches("two_pair", &code("0110")));
        assert!(matches("palindrome", &code("0000")));
        assert!(matches("mono", &code("0000")));
        assert!(!matches("palindrome", &code("0011")));
        assert!(matches("palindrome", &code("012210")));
        assert!(!matches("ascending", &code("012210")));
    }
}
