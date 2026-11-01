//! Exact multipliers, stored in thousandths of an amp.
//!
//! Signature bonuses use halves (`1.5`) and hit bonuses use quarters (`0.25`).
//! Those are not all nice in binary floating point once they stack, so the
//! pipeline keeps amp as an integer number of thousandths and forms
//! `score = bits × amp` in that scale.

use std::fmt;

/// A multiplier in thousandths. `1000` is `×1`, `1500` is `×1.5`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Amp(i32);

impl Amp {
    /// `×0`.
    pub const ZERO: Self = Self(0);
    /// `×1`.
    pub const ONE: Self = Self(1000);

    /// Build an amp from thousandths. Negative values are allowed so a tool
    /// can cut the multiplier; the scoring pipeline clamps the total at zero.
    pub const fn from_milli(milli: i32) -> Self {
        Self(milli)
    }

    /// Thousandths of an amp.
    pub const fn milli(self) -> i32 {
        self.0
    }

    /// The multiplier as a decimal (`1.5` for 1500).
    pub fn as_f64(self) -> f64 {
        f64::from(self.0) / 1000.0
    }

    /// Add two amps, saturating at the `i32` bounds.
    pub const fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    /// Subtract two amps, saturating at the `i32` bounds.
    pub const fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    /// Multiply two multipliers. `1.5 × 2` is `3`, computed in thousandths.
    pub fn saturating_mul(self, other: Self) -> Self {
        let product = i64::from(self.0).saturating_mul(i64::from(other.0)) / 1000;
        Self(clamp_i64_to_i32(product))
    }
}

impl fmt::Display for Amp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        let milli = self.0.unsigned_abs();
        let whole = milli / 1000;
        let frac = milli % 1000;
        if frac == 0 {
            write!(f, "{sign}{whole}")
        } else if frac % 100 == 0 {
            write!(f, "{sign}{whole}.{}", frac / 100)
        } else if frac % 10 == 0 {
            write!(f, "{sign}{whole}.{:02}", frac / 10)
        } else {
            write!(f, "{sign}{whole}.{frac:03}")
        }
    }
}

pub(crate) fn parse_amp(value: f64) -> Result<Amp, String> {
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

fn clamp_i64_to_i32(value: i64) -> i32 {
    if value > i64::from(i32::MAX) {
        i32::MAX
    } else if value < i64::from(i32::MIN) {
        i32::MIN
    } else {
        i32::try_from(value).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::Amp;

    #[test]
    fn thousandths_round_trip_the_table_values() {
        assert_eq!(Amp::ONE.milli(), 1000);
        assert_eq!(Amp::from_milli(1500).as_f64(), 1.5);
        assert_eq!(Amp::from_milli(250).to_string(), "0.25");
        assert_eq!(Amp::from_milli(1500).to_string(), "1.5");
        assert_eq!(Amp::ONE.to_string(), "1");
        assert_eq!(Amp::from_milli(-500).to_string(), "-0.5");
    }

    #[test]
    fn multiply_keeps_halves_exact() {
        let one_and_a_half = Amp::from_milli(1500);
        let two = Amp::from_milli(2000);
        assert_eq!(one_and_a_half.saturating_mul(two), Amp::from_milli(3000));
    }
}
