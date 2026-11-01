//! Symbols, the charset, and a probe or secret code.
//!
//! The charset is the leading hex digits `0`–`7`. A layer uses a prefix of
//! that list (6, 7, or 8 symbols). Slots never exceed [`MAX_SLOTS`].

use std::fmt;

use thiserror::Error;

/// Longest code the rules core will hold. Layers 7–8 use all six slots.
pub const MAX_SLOTS: usize = 6;

/// Largest charset. Layers 7–8 use every symbol.
pub const MAX_SYMBOLS: usize = 8;

/// Glyphs in charset order. Index `0` is `'0'`.
pub const GLYPHS: &str = "01234567";

/// One charset symbol, ordered so ascending probes compare by index.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct Symbol(u8);

impl Symbol {
    /// Fallible constructor. `index` must be inside the full charset.
    pub fn new(index: u8) -> Result<Self, CodeError> {
        if usize::from(index) >= MAX_SYMBOLS {
            return Err(CodeError::BadIndex(index));
        }
        Ok(Self(index))
    }

    /// Position in the charset, starting at 0.
    pub const fn index(self) -> u8 {
        self.0
    }

    /// The glyph a player would read.
    pub fn glyph(self) -> char {
        char::from(GLYPHS.as_bytes()[usize::from(self.0)])
    }
}

/// The symbol set for a layer: the first `size` glyphs of [`GLYPHS`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Charset {
    size: u8,
}

impl Charset {
    /// A charset of `size` symbols. `size` must be `1..=MAX_SYMBOLS`.
    pub fn new(size: usize) -> Result<Self, CodeError> {
        if size == 0 || size > MAX_SYMBOLS {
            return Err(CodeError::BadCharset(size));
        }
        Ok(Self {
            size: u8::try_from(size).expect("size checked against MAX_SYMBOLS"),
        })
    }

    /// The full eight-symbol charset.
    pub fn full() -> Self {
        Self {
            size: u8::try_from(MAX_SYMBOLS).expect("MAX_SYMBOLS fits in u8"),
        }
    }

    /// How many symbols a probe may use.
    pub fn size(self) -> usize {
        usize::from(self.size)
    }

    /// Glyphs in play, in ascending order.
    pub fn glyphs(self) -> &'static str {
        &GLYPHS[..self.size()]
    }

    /// Map a glyph to its index when it belongs to this charset.
    pub fn index_of(self, glyph: char) -> Option<u8> {
        let index = GLYPHS.chars().position(|candidate| candidate == glyph)?;
        if index < self.size() {
            u8::try_from(index).ok()
        } else {
            None
        }
    }

    /// Whether `symbol` is inside this charset.
    pub fn contains(self, symbol: Symbol) -> bool {
        usize::from(symbol.index()) < self.size()
    }
}

/// A probe or a secret. Length is between 1 and [`MAX_SLOTS`].
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Code {
    slots: [u8; MAX_SLOTS],
    len: u8,
}

impl Code {
    /// Build a code from charset indices.
    pub fn try_from_indices(indices: &[u8]) -> Result<Self, CodeError> {
        if indices.is_empty() {
            return Err(CodeError::Empty);
        }
        if indices.len() > MAX_SLOTS {
            return Err(CodeError::TooLong);
        }
        let mut slots = [0u8; MAX_SLOTS];
        for (place, &index) in indices.iter().enumerate() {
            if usize::from(index) >= MAX_SYMBOLS {
                return Err(CodeError::BadIndex(index));
            }
            slots[place] = index;
        }
        Ok(Self {
            slots,
            len: u8::try_from(indices.len()).expect("length checked against MAX_SLOTS"),
        })
    }

    /// Parse a glyph string such as `0123` against `charset`.
    pub fn parse(charset: Charset, text: &str) -> Result<Self, CodeError> {
        if text.is_empty() {
            return Err(CodeError::Empty);
        }
        if text.chars().count() > MAX_SLOTS {
            return Err(CodeError::TooLong);
        }
        let mut slots = [0u8; MAX_SLOTS];
        let mut len = 0usize;
        for glyph in text.chars() {
            let Some(index) = charset.index_of(glyph) else {
                return Err(CodeError::BadGlyph(glyph));
            };
            slots[len] = index;
            len += 1;
        }
        Ok(Self {
            slots,
            len: u8::try_from(len).expect("length checked against MAX_SLOTS"),
        })
    }

    /// Number of slots.
    pub fn len(self) -> usize {
        usize::from(self.len)
    }

    /// Always false for a constructed code. Present so `len` has its pair.
    pub fn is_empty(self) -> bool {
        self.len == 0
    }

    /// Slot indices, length [`Self::len`].
    pub fn as_slice(&self) -> &[u8] {
        &self.slots[..self.len()]
    }

    /// Symbol in `slot`, if it exists.
    pub fn symbol(self, slot: usize) -> Option<Symbol> {
        self.as_slice().get(slot).copied().map(Symbol)
    }

    /// Symbols from the first slot to the last.
    pub fn iter(&self) -> impl Iterator<Item = Symbol> + '_ {
        self.as_slice().iter().copied().map(Symbol)
    }

    /// Highest number of times any one symbol appears.
    pub fn max_frequency(self) -> u8 {
        let mut counts = [0u8; MAX_SYMBOLS];
        for &symbol in self.as_slice() {
            counts[usize::from(symbol)] += 1;
        }
        counts.into_iter().max().unwrap_or(0)
    }

    /// Whether every slot is in `0..symbols` and the code could be played there.
    pub fn within_charset(self, symbols: usize) -> bool {
        self.as_slice()
            .iter()
            .all(|&symbol| usize::from(symbol) < symbols)
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &symbol in self.as_slice() {
            let glyph = char::from(GLYPHS.as_bytes()[usize::from(symbol)]);
            write!(f, "{glyph}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Code({self})")
    }
}

/// A code or charset that could not be built.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum CodeError {
    /// `""` is not a probe.
    #[error("code is empty")]
    Empty,
    /// Longer than [`MAX_SLOTS`].
    #[error("code is longer than {MAX_SLOTS} slots")]
    TooLong,
    /// Glyph is not in the requested charset.
    #[error("unknown symbol '{0}'")]
    BadGlyph(char),
    /// Index is outside `0..MAX_SYMBOLS`.
    #[error("symbol index {0} is outside the charset")]
    BadIndex(u8),
    /// Charset size was 0 or above [`MAX_SYMBOLS`].
    #[error("charset size {0} is outside 1..={MAX_SYMBOLS}")]
    BadCharset(usize),
}

#[cfg(test)]
mod tests {
    use super::{Charset, Code, CodeError, Symbol, GLYPHS, MAX_SYMBOLS};

    #[test]
    fn parses_glyphs_and_rejects_symbols_past_the_charset() {
        let six = Charset::new(6).unwrap();
        let code = Code::parse(six, "0125").unwrap();
        assert_eq!(code.to_string(), "0125");
        assert_eq!(code.len(), 4);
        assert_eq!(code.symbol(3).unwrap().glyph(), '5');
        assert_eq!(Code::parse(six, "0126"), Err(CodeError::BadGlyph('6')));
        assert_eq!(Code::parse(six, ""), Err(CodeError::Empty));
        assert_eq!(Code::parse(six, "0123456"), Err(CodeError::TooLong));
        assert!(Charset::new(0).is_err());
        assert!(Charset::new(MAX_SYMBOLS + 1).is_err());
        assert_eq!(Charset::full().glyphs(), GLYPHS);
    }

    #[test]
    fn symbol_order_matches_glyph_order() {
        let earlier = Symbol::new(2).unwrap();
        let later = Symbol::new(5).unwrap();
        assert!(earlier < later);
        assert_eq!(earlier.glyph(), '2');
        assert!(Symbol::new(8).is_err());
    }

    #[test]
    fn max_frequency_counts_the_heaviest_symbol() {
        let code = Code::parse(Charset::full(), "4543").unwrap();
        assert_eq!(code.max_frequency(), 2);
        let mono = Code::parse(Charset::full(), "7777").unwrap();
        assert_eq!(mono.max_frequency(), 4);
    }
}
