//! Layer shape: slots, charset size, and the probe budget.
//!
//! The slot and symbol counts are fixed by the rules. The probe budget is the
//! number of probes a breach allows; it is also the cap the fairness solver
//! has to beat.

use crate::code::Charset;

/// How many layers a run contains.
pub const LAYER_COUNT: usize = 8;

/// Rules for one layer of a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerSpec {
    layer: u8,
    slots: u8,
    symbols: u8,
    probe_budget: u8,
    max_repeat: u8,
}

impl LayerSpec {
    /// Spec for `layer` in `1..=8`.
    ///
    /// | layers | slots | symbols | probe budget |
    /// | --- | --- | --- | --- |
    /// | 1–3 | 4 | 6 | 10 |
    /// | 4–6 | 5 | 7 | 12 |
    /// | 7–8 | 6 | 8 | 14 |
    ///
    /// A fair secret uses no symbol more than twice. That cap is part of the
    /// spec so a test can tighten it without inventing a second config type.
    pub fn for_layer(layer: u8) -> Option<Self> {
        let (slots, symbols, probe_budget) = match layer {
            1..=3 => (4, 6, 10),
            4..=6 => (5, 7, 12),
            7..=8 => (6, 8, 14),
            _ => return None,
        };
        Some(Self {
            layer,
            slots,
            symbols,
            probe_budget,
            max_repeat: 2,
        })
    }

    /// Layers 1 through 8, in order.
    pub fn all() -> [Self; LAYER_COUNT] {
        std::array::from_fn(|index| {
            let layer = u8::try_from(index + 1).expect("layer index fits in u8");
            Self::for_layer(layer).expect("index is a real layer")
        })
    }

    /// 1-based layer number.
    pub fn layer(self) -> u8 {
        self.layer
    }

    /// Slots in the secret.
    pub fn slots(self) -> usize {
        usize::from(self.slots)
    }

    /// Charset size.
    pub fn symbols(self) -> usize {
        usize::from(self.symbols)
    }

    /// Probes allowed in the breach.
    pub fn probe_budget(self) -> u8 {
        self.probe_budget
    }

    /// Maximum times one symbol may appear in a fair secret.
    pub fn max_repeat(self) -> u8 {
        self.max_repeat
    }

    /// Charset for this layer.
    pub fn charset(self) -> Charset {
        Charset::new(self.symbols()).expect("layer symbol count is a valid charset")
    }

    /// `symbols ^ slots`, the number of possible secrets.
    pub fn space(self) -> u64 {
        u64::from(self.symbols).pow(u32::from(self.slots))
    }

    /// Same shape, different probe budget. Used by the fairness tests and,
    /// later, by anything that wants a tighter breach.
    pub fn with_probe_budget(mut self, probe_budget: u8) -> Self {
        self.probe_budget = probe_budget;
        self
    }

    /// Same shape, different repeat cap.
    pub fn with_max_repeat(mut self, max_repeat: u8) -> Self {
        self.max_repeat = max_repeat;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::LayerSpec;

    #[test]
    fn sizes_follow_the_three_bands() {
        let cases = [
            (1, 4, 6, 10),
            (2, 4, 6, 10),
            (3, 4, 6, 10),
            (4, 5, 7, 12),
            (5, 5, 7, 12),
            (6, 5, 7, 12),
            (7, 6, 8, 14),
            (8, 6, 8, 14),
        ];
        for (layer, slots, symbols, budget) in cases {
            let spec = LayerSpec::for_layer(layer).unwrap();
            assert_eq!(spec.slots(), slots, "layer {layer}");
            assert_eq!(spec.symbols(), symbols, "layer {layer}");
            assert_eq!(spec.probe_budget(), budget, "layer {layer}");
            assert_eq!(spec.max_repeat(), 2);
            assert_eq!(spec.charset().size(), symbols);
            assert_eq!(
                spec.space(),
                u64::try_from(symbols)
                    .unwrap()
                    .pow(u32::try_from(slots).unwrap())
            );
        }
        assert!(LayerSpec::for_layer(0).is_none());
        assert!(LayerSpec::for_layer(9).is_none());
        assert_eq!(LayerSpec::all().len(), 8);
    }
}
