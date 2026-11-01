//! Shareable run seeds.
//!
//! A seed is `MV-7F3A-9C21-L4`: a `MV-` prefix, two groups of four hex digits
//! (one 32-bit key), and the layer the code is for. Formatting is uppercase.
//! Parsing accepts either case.

use std::fmt;

use thiserror::Error;

/// A share code: 32 bits of run key plus a layer in `1..=8`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RunSeed {
    key: u32,
    layer: u8,
}

impl RunSeed {
    /// Build a seed when `layer` is in range.
    pub fn new(key: u32, layer: u8) -> Result<Self, SeedError> {
        if !(1..=8).contains(&layer) {
            return Err(SeedError::Layer);
        }
        Ok(Self { key, layer })
    }

    /// Parse `MV-7F3A-9C21-L4`.
    pub fn parse(text: &str) -> Result<Self, SeedError> {
        if text.is_empty() {
            return Err(SeedError::Empty);
        }
        let mut parts = text.split('-');
        let prefix = parts.next().ok_or(SeedError::Shape)?;
        let hi = parts.next().ok_or(SeedError::Shape)?;
        let lo = parts.next().ok_or(SeedError::Shape)?;
        let layer = parts.next().ok_or(SeedError::Shape)?;
        if parts.next().is_some() {
            return Err(SeedError::Shape);
        }
        if !prefix.eq_ignore_ascii_case("MV") {
            return Err(SeedError::Prefix);
        }
        if hi.len() != 4 || lo.len() != 4 || !layer_shape(layer) {
            return Err(SeedError::Shape);
        }
        let hi = u16::from_str_radix(hi, 16).map_err(|_| SeedError::Hex)?;
        let lo = u16::from_str_radix(lo, 16).map_err(|_| SeedError::Hex)?;
        let layer_num: u8 = layer[1..].parse().map_err(|_| SeedError::Layer)?;
        Self::new((u32::from(hi) << 16) | u32::from(lo), layer_num)
    }

    /// The 32-bit run key.
    pub fn key(self) -> u32 {
        self.key
    }

    /// Layer encoded in the share string.
    pub fn layer(self) -> u8 {
        self.layer
    }
}

impl fmt::Display for RunSeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hi = u16::try_from(self.key >> 16).unwrap_or(0);
        let lo = u16::try_from(self.key & 0xFFFF).unwrap_or(0);
        write!(f, "MV-{hi:04X}-{lo:04X}-L{}", self.layer)
    }
}

fn layer_shape(layer: &str) -> bool {
    let mut chars = layer.chars();
    let Some(mark) = chars.next() else {
        return false;
    };
    let Some(digit) = chars.next() else {
        return false;
    };
    chars.next().is_none() && (mark == 'L' || mark == 'l') && digit.is_ascii_digit()
}

/// A share string that is not a mastervibes seed.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SeedError {
    /// The string was empty.
    #[error("seed is empty")]
    Empty,
    /// The first group was not `MV`.
    #[error("seed must start with MV-")]
    Prefix,
    /// The groups were not `MV-XXXX-XXXX-Ln`.
    #[error("seed must look like MV-7F3A-9C21-L4")]
    Shape,
    /// A hex group contained a non-hex digit.
    #[error("seed groups must be 4 hexadecimal digits")]
    Hex,
    /// The layer digit was outside `1..=8`.
    #[error("layer must be from 1 to 8")]
    Layer,
}

#[cfg(test)]
mod tests {
    use super::{RunSeed, SeedError};

    #[test]
    fn example_seed_round_trips() {
        let seed = RunSeed::parse("MV-7F3A-9C21-L4").unwrap();
        assert_eq!(seed.key(), 0x7F3A_9C21);
        assert_eq!(seed.layer(), 4);
        assert_eq!(seed.to_string(), "MV-7F3A-9C21-L4");
    }

    #[test]
    fn parse_accepts_lowercase_and_formats_uppercase() {
        let seed = RunSeed::parse("mv-7f3a-9c21-l4").unwrap();
        assert_eq!(seed.to_string(), "MV-7F3A-9C21-L4");
        let edges = RunSeed::parse("MV-0000-0000-L1").unwrap();
        assert_eq!(edges.key(), 0);
        assert_eq!(edges.to_string(), "MV-0000-0000-L1");
        let max = RunSeed::new(u32::MAX, 8).unwrap();
        assert_eq!(max.to_string(), "MV-FFFF-FFFF-L8");
        assert_eq!(RunSeed::parse(&max.to_string()).unwrap(), max);
    }

    #[test]
    fn parse_rejects_malformed_seeds() {
        assert_eq!(RunSeed::parse(""), Err(SeedError::Empty));
        assert_eq!(RunSeed::parse("XX-7F3A-9C21-L4"), Err(SeedError::Prefix));
        assert_eq!(RunSeed::parse("MV-7F3A-9C21"), Err(SeedError::Shape));
        assert_eq!(RunSeed::parse("MV-7F3A-9C2-L4"), Err(SeedError::Shape));
        assert_eq!(
            RunSeed::parse("MV-7F3A-9C21-L4-EXTRA"),
            Err(SeedError::Shape)
        );
        assert_eq!(RunSeed::parse("MV-7F3A-9C21-L04"), Err(SeedError::Shape));
        assert_eq!(RunSeed::parse("MV-7F3G-9C21-L4"), Err(SeedError::Hex));
        assert_eq!(RunSeed::parse("MV-7F3A-9C21-L0"), Err(SeedError::Layer));
        assert_eq!(RunSeed::parse("MV-7F3A-9C21-L9"), Err(SeedError::Layer));
        assert_eq!(RunSeed::new(1, 0), Err(SeedError::Layer));
    }
}
