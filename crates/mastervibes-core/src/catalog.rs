//! Signature table and scoring rates, loaded from TOML.
//!
//! The embedded table is [`embedded_catalog`]. [`Catalog::from_toml`] is the
//! same parser, so a test or a later content pass can swap the numbers without
//! changing detection.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use serde::Deserialize;
use thiserror::Error;

use crate::amp::Amp;
use crate::code::Code;
use crate::pattern::{self, KNOWN_IDS};

/// One signature row: the id the detector understands, the player-facing name,
/// and the base bits / amp bonus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Signature {
    /// Stable id (`pair`, `full_lock`, …).
    pub id: String,
    /// Glossary name (`Pair`, `Full Lock`, …).
    pub name: String,
    /// Base bits added when this signature wins.
    pub bits: i32,
    /// Amp bonus added to the pipeline's base amp.
    pub amp: Amp,
    /// Highest priority wins when several patterns match.
    pub priority: i32,
}

/// Peg rates and the amp a probe starts from. Loaded from `[scoring]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoringRules {
    /// Amp before any signature or peg bonus. The stock table uses `1`.
    pub base_amp: Amp,
    /// Bits added per hit.
    pub hit_bits: i32,
    /// Amp added per hit.
    pub hit_amp: Amp,
    /// Bits added per near.
    pub near_bits: i32,
    /// Amp added per near.
    pub near_amp: Amp,
}

/// Loaded signature table plus the scoring rates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    scoring: ScoringRules,
    signatures: Vec<Signature>,
    by_id: HashMap<String, usize>,
}

impl Catalog {
    /// Parse a signature table. Every built-in detector needs exactly one row,
    /// priorities must be unique, and unknown ids are rejected.
    pub fn from_toml(text: &str) -> Result<Self, CatalogError> {
        let raw: RawFile =
            toml::from_str(text).map_err(|err| CatalogError::Toml(err.to_string()))?;
        let scoring = parse_rules(&raw.scoring)?;
        if raw.signature.is_empty() {
            return Err(CatalogError::MissingSignature(KNOWN_IDS[0].to_string()));
        }

        let mut seen_ids = HashSet::new();
        let mut seen_priority = HashSet::new();
        let mut signatures = Vec::with_capacity(raw.signature.len());
        for row in raw.signature {
            if row.id.is_empty() || !pattern::is_known_id(&row.id) {
                return Err(CatalogError::UnknownSignature(row.id));
            }
            if !seen_ids.insert(row.id.clone()) {
                return Err(CatalogError::DuplicateSignature(row.id));
            }
            if !seen_priority.insert(row.priority) {
                return Err(CatalogError::DuplicatePriority(row.priority));
            }
            if row.name.trim().is_empty() {
                return Err(CatalogError::Invalid {
                    id: row.id,
                    reason: "name is empty".to_string(),
                });
            }
            if row.bits < 0 {
                return Err(CatalogError::Invalid {
                    id: row.id,
                    reason: "bits must be >= 0".to_string(),
                });
            }
            let amp = parse_amp(row.amp).map_err(|reason| CatalogError::Invalid {
                id: row.id.clone(),
                reason,
            })?;
            if amp.milli() < 0 {
                return Err(CatalogError::Invalid {
                    id: row.id,
                    reason: "signature amp bonus must be >= 0".to_string(),
                });
            }
            signatures.push(Signature {
                id: row.id,
                name: row.name,
                bits: row.bits,
                amp,
                priority: row.priority,
            });
        }

        for id in KNOWN_IDS {
            if !seen_ids.contains(id) {
                return Err(CatalogError::MissingSignature(id.to_string()));
            }
        }

        // Highest priority first, so detection returns on the first match.
        signatures.sort_by(|left, right| right.priority.cmp(&left.priority));
        let by_id = signatures
            .iter()
            .enumerate()
            .map(|(index, signature)| (signature.id.clone(), index))
            .collect();

        Ok(Self {
            scoring,
            signatures,
            by_id,
        })
    }

    /// Rates from `[scoring]`.
    pub fn scoring(&self) -> ScoringRules {
        self.scoring
    }

    /// Rows, highest priority first.
    pub fn signatures(&self) -> &[Signature] {
        &self.signatures
    }

    /// Look up a row by id.
    pub fn get(&self, id: &str) -> Option<&Signature> {
        self.by_id.get(id).map(|&index| &self.signatures[index])
    }

    /// The signature this probe earns.
    ///
    /// Every probe matches exactly one frequency pattern, and a loaded catalog
    /// has a row for each of them, so this always resolves.
    pub fn detect<'a>(&'a self, probe: &Code) -> &'a Signature {
        self.signatures
            .iter()
            .find(|signature| pattern::matches(&signature.id, probe))
            .expect("every probe matches a frequency signature present in the catalog")
    }
}

/// The signature table compiled into the crate.
pub fn embedded_catalog() -> &'static Catalog {
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        Catalog::from_toml(include_str!("../data/signatures.toml"))
            .expect("embedded signatures.toml is valid")
    })
}

/// The TOML table could not be turned into a catalog.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CatalogError {
    /// `toml` rejected the document.
    #[error("could not parse signature table: {0}")]
    Toml(String),
    /// An id with no detector.
    #[error("unknown signature id `{0}`")]
    UnknownSignature(String),
    /// The same id appeared twice.
    #[error("duplicate signature id `{0}`")]
    DuplicateSignature(String),
    /// Two rows claimed the same priority.
    #[error("duplicate signature priority {0}")]
    DuplicatePriority(i32),
    /// A built-in detector had no row.
    #[error("missing signature id `{0}`")]
    MissingSignature(String),
    /// A row or the scoring block had a value the pipeline cannot use.
    #[error("signature `{id}` is invalid: {reason}")]
    Invalid {
        /// Row id, or `scoring` for the rates block.
        id: String,
        /// What was wrong.
        reason: String,
    },
}

fn parse_rules(raw: &RawScoring) -> Result<ScoringRules, CatalogError> {
    let base_amp = parse_amp(raw.base_amp).map_err(|reason| CatalogError::Invalid {
        id: "scoring".to_string(),
        reason,
    })?;
    if base_amp.milli() <= 0 {
        return Err(CatalogError::Invalid {
            id: "scoring".to_string(),
            reason: "base_amp must be > 0".to_string(),
        });
    }
    if raw.hit_bits < 0 || raw.near_bits < 0 {
        return Err(CatalogError::Invalid {
            id: "scoring".to_string(),
            reason: "hit_bits and near_bits must be >= 0".to_string(),
        });
    }
    let hit_amp = non_negative_amp("hit_amp", raw.hit_amp)?;
    let near_amp = non_negative_amp("near_amp", raw.near_amp)?;
    Ok(ScoringRules {
        base_amp,
        hit_bits: raw.hit_bits,
        hit_amp,
        near_bits: raw.near_bits,
        near_amp,
    })
}

fn non_negative_amp(field: &str, value: f64) -> Result<Amp, CatalogError> {
    let amp = parse_amp(value).map_err(|reason| CatalogError::Invalid {
        id: "scoring".to_string(),
        reason: format!("{field}: {reason}"),
    })?;
    if amp.milli() < 0 {
        return Err(CatalogError::Invalid {
            id: "scoring".to_string(),
            reason: format!("{field} must be >= 0"),
        });
    }
    Ok(amp)
}

fn parse_amp(value: f64) -> Result<Amp, String> {
    if !value.is_finite() {
        return Err("amp must be finite".to_string());
    }
    if !(-1_000_000.0..=1_000_000.0).contains(&value) {
        return Err("amp is out of range".to_string());
    }
    let milli = (value * 1000.0).round();
    if (milli / 1000.0 - value).abs() > 1e-4 {
        return Err("amp must be a multiple of 0.001".to_string());
    }
    let milli = i32::try_from(milli as i64).map_err(|_| "amp is out of range".to_string())?;
    Ok(Amp::from_milli(milli))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    scoring: RawScoring,
    signature: Vec<RawSignature>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScoring {
    base_amp: f64,
    hit_bits: i32,
    hit_amp: f64,
    near_bits: i32,
    near_amp: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSignature {
    id: String,
    name: String,
    bits: i32,
    amp: f64,
    priority: i32,
}

#[cfg(test)]
mod tests {
    use super::{embedded_catalog, Catalog, CatalogError};
    use crate::amp::Amp;
    use crate::code::{Charset, Code};
    use std::collections::HashMap;

    fn code(text: &str) -> Code {
        Code::parse(Charset::full(), text).unwrap()
    }

    fn code_in(symbols: usize, text: &str) -> Code {
        Code::parse(Charset::new(symbols).unwrap(), text).unwrap()
    }

    #[test]
    fn embedded_table_matches_the_spec_and_progresses() {
        let catalog = embedded_catalog();
        let expect = [
            ("all_unique", "All Unique", 10, 0, 10),
            ("pair", "Pair", 15, 1000, 20),
            ("ascending", "Ascending", 20, 1000, 30),
            ("two_pair", "Two Pair", 25, 1500, 40),
            ("palindrome", "Palindrome", 28, 1500, 50),
            ("triple", "Triple", 30, 2000, 60),
            ("full_lock", "Full Lock", 40, 2500, 70),
            ("quad", "Quad", 50, 3000, 80),
            ("mono", "Mono", 60, 4000, 90),
        ];
        assert_eq!(catalog.signatures().len(), expect.len());
        for (id, name, bits, amp, priority) in expect {
            let row = catalog.get(id).unwrap();
            assert_eq!(row.name, name);
            assert_eq!(row.bits, bits);
            assert_eq!(row.amp, Amp::from_milli(amp));
            assert_eq!(row.priority, priority);
        }
        let rules = catalog.scoring();
        assert_eq!(rules.base_amp, Amp::ONE);
        assert_eq!(rules.hit_bits, 5);
        assert_eq!(rules.hit_amp, Amp::from_milli(250));
        assert_eq!(rules.near_bits, 2);
        assert_eq!(rules.near_amp, Amp::ZERO);

        let mut rows: Vec<_> = catalog.signatures().to_vec();
        rows.sort_by_key(|row| row.priority);
        for pair in rows.windows(2) {
            assert!(pair[0].bits <= pair[1].bits);
            assert!(pair[0].amp <= pair[1].amp);
        }
    }

    #[test]
    fn detection_uses_priority_from_the_table() {
        let catalog = embedded_catalog();
        assert_eq!(catalog.detect(&code("0132")).id, "all_unique");
        assert_eq!(catalog.detect(&code("0123")).id, "ascending");
        assert_eq!(catalog.detect(&code("0012")).id, "pair");
        assert_eq!(catalog.detect(&code("0011")).id, "two_pair");
        assert_eq!(catalog.detect(&code("0110")).id, "palindrome");
        assert_eq!(catalog.detect(&code("0001")).id, "triple");
        assert_eq!(catalog.detect(&code("00011")).id, "full_lock");
        assert_eq!(catalog.detect(&code("00001")).id, "quad");
        assert_eq!(catalog.detect(&code("000011")).id, "quad");
        assert_eq!(catalog.detect(&code("0000")).id, "mono");
        assert_eq!(catalog.detect(&code("012210")).id, "palindrome");
        assert_eq!(catalog.detect(&code("001122")).id, "two_pair");
        assert_eq!(catalog.detect(&code("000111")).id, "triple");

        // Drop Palindrome under Two Pair and ABBA stops being a palindrome payout.
        let mut text = include_str!("../data/signatures.toml").to_string();
        text = text.replace(
            "id = \"palindrome\"\nname = \"Palindrome\"\nbits = 28\namp = 1.5\npriority = 50",
            "id = \"palindrome\"\nname = \"Palindrome\"\nbits = 28\namp = 1.5\npriority = 5",
        );
        let custom = Catalog::from_toml(&text).unwrap();
        assert_eq!(catalog.detect(&code("01210")).id, "palindrome");
        assert_eq!(custom.detect(&code("0110")).id, "two_pair");
        assert_eq!(custom.detect(&code("01210")).id, "two_pair");
    }

    #[test]
    fn four_slot_labels_cover_the_space_with_the_expected_counts() {
        let catalog = embedded_catalog();
        let mut counts: HashMap<String, usize> = HashMap::new();
        for id in 0..6u32.pow(4) {
            let code = code_in(6, &unpack(id, 4, 6));
            let signature = catalog.detect(&code);
            *counts.entry(signature.id.clone()).or_default() += 1;
        }
        assert_eq!(counts["mono"], 6);
        assert_eq!(counts["ascending"], 15);
        assert_eq!(counts["palindrome"], 30);
        assert_eq!(counts["two_pair"], 60);
        assert_eq!(counts["triple"], 120);
        assert_eq!(counts["pair"], 720);
        assert_eq!(counts["all_unique"], 345);
        assert!(!counts.contains_key("full_lock"));
        assert!(!counts.contains_key("quad"));
        let total: usize = counts.values().sum();
        assert_eq!(total, 1296);
    }

    #[test]
    fn loader_rejects_a_broken_table() {
        let missing = r#"
            [scoring]
            base_amp = 1.0
            hit_bits = 5
            hit_amp = 0.25
            near_bits = 2
            near_amp = 0.0
            [[signature]]
            id = "pair"
            name = "Pair"
            bits = 15
            amp = 1.0
            priority = 20
        "#;
        assert!(matches!(
            Catalog::from_toml(missing),
            Err(CatalogError::MissingSignature(_))
        ));

        let mut unknown = include_str!("../data/signatures.toml").to_string();
        unknown.push_str(
            r#"
            [[signature]]
            id = "flush"
            name = "Flush"
            bits = 1
            amp = 0.0
            priority = 99
            "#,
        );
        assert!(matches!(
            Catalog::from_toml(&unknown),
            Err(CatalogError::UnknownSignature(_))
        ));

        let negative = include_str!("../data/signatures.toml").replace("bits = 10", "bits = -1");
        assert!(matches!(
            Catalog::from_toml(&negative),
            Err(CatalogError::Invalid { .. })
        ));

        let zero_base =
            include_str!("../data/signatures.toml").replace("base_amp = 1.0", "base_amp = 0.0");
        assert!(matches!(
            Catalog::from_toml(&zero_base),
            Err(CatalogError::Invalid { .. })
        ));

        assert!(Catalog::from_toml("not toml").is_err());
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
