//! Seeded secrets.
//!
//! Each layer has its own [`rand_pcg::Pcg32`] stream, derived from the run key
//! and the layer number. Sharing `MV-7F3A-9C21-L4` reproduces layer 4's secret
//! without replaying layers 1–3. Rejected draws stay on that stream, so the
//! next attempt is the next batch of slot rolls rather than a reseed.

use rand::Rng;
use rand::SeedableRng;
use rand_pcg::Pcg32;

use crate::code::{Code, MAX_SLOTS};
use crate::fairness::is_fair;
use crate::layer::{LayerSpec, LAYER_COUNT};
use crate::seed::RunSeed;

/// How many draws the guardrail will take before it keeps the last code.
pub const MAX_FAIR_ATTEMPTS: u32 = 64;

/// A secret drawn for one layer, plus how hard the guardrail had to work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeneratedCode {
    /// The secret to crack.
    pub code: Code,
    /// Draws taken, including the one that was kept. Always at least 1.
    pub attempts: u32,
    /// `false` only when [`MAX_FAIR_ATTEMPTS`] unfair codes were drawn in a row.
    pub fair: bool,
    /// Layer the code was drawn for.
    pub spec: LayerSpec,
}

/// Eight secrets, one per layer, for a single run key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RunPlan {
    /// Run key shared by every layer stream.
    pub key: u32,
    /// Index 0 is layer 1.
    pub layers: [GeneratedCode; LAYER_COUNT],
}

/// Draw the secret for `seed`'s layer.
///
/// The same seed always returns the same code. The stream is
/// `Pcg32::seed_from_u64((key << 8) | layer)`.
pub fn generate_secret(seed: RunSeed) -> GeneratedCode {
    let spec = LayerSpec::for_layer(seed.layer()).expect("RunSeed only holds layers 1..=8");
    let mut rng = Pcg32::seed_from_u64(stream_seed(seed.key(), seed.layer()));
    generate_from_draws(spec, || draw_code(&mut rng, spec))
}

/// Draw every layer of `key`. Layer *n* matches [`generate_secret`] for that layer.
pub fn plan_run(key: u32) -> RunPlan {
    let layers = std::array::from_fn(|index| {
        let layer = u8::try_from(index + 1).expect("layer index fits in u8");
        let seed = RunSeed::new(key, layer).expect("layer is in range");
        generate_secret(seed)
    });
    RunPlan { key, layers }
}

/// Fairness loop over an arbitrary draw function.
///
/// The seeded generator passes a closure that rolls slots from its PCG stream.
/// Tests pass a list. Unfair draws are discarded and the closure is called
/// again, which is what "redraw from the same stream" means.
pub fn generate_from_draws(spec: LayerSpec, mut draw: impl FnMut() -> Code) -> GeneratedCode {
    let mut last = draw();
    let mut attempts = 1u32;
    if is_fair(&last, spec) {
        return GeneratedCode {
            code: last,
            attempts,
            fair: true,
            spec,
        };
    }
    while attempts < MAX_FAIR_ATTEMPTS {
        last = draw();
        attempts += 1;
        if is_fair(&last, spec) {
            return GeneratedCode {
                code: last,
                attempts,
                fair: true,
                spec,
            };
        }
    }
    GeneratedCode {
        code: last,
        attempts,
        fair: false,
        spec,
    }
}

fn stream_seed(key: u32, layer: u8) -> u64 {
    (u64::from(key) << 8) | u64::from(layer)
}

fn draw_code(rng: &mut impl Rng, spec: LayerSpec) -> Code {
    let mut digits = [0u8; MAX_SLOTS];
    let symbols = u8::try_from(spec.symbols()).expect("symbol count fits in u8");
    for digit in digits.iter_mut().take(spec.slots()) {
        *digit = rng.gen_range(0..symbols);
    }
    Code::try_from_indices(&digits[..spec.slots()]).expect("rolled symbols are inside the charset")
}

#[cfg(test)]
mod tests {
    use super::{generate_from_draws, generate_secret, plan_run, MAX_FAIR_ATTEMPTS};
    use crate::code::{Charset, Code};
    use crate::fairness::is_fair;
    use crate::layer::LayerSpec;
    use crate::seed::RunSeed;

    fn code(text: &str) -> Code {
        Code::parse(Charset::full(), text).unwrap()
    }

    #[test]
    fn redraws_along_the_draw_sequence_and_stops_at_the_cap() {
        let spec = LayerSpec::for_layer(1).unwrap();
        let sequence = [code("0000"), code("0001"), code("0123")];
        let mut index = 0;
        let generated = generate_from_draws(spec, || {
            let next = sequence[index];
            index += 1;
            next
        });
        assert_eq!(generated.code, code("0123"));
        assert_eq!(generated.attempts, 3);
        assert!(generated.fair);
        assert_eq!(index, 3);

        let hopeless = spec.with_max_repeat(0);
        let mut draws = 0u32;
        let forced = generate_from_draws(hopeless, || {
            draws += 1;
            code("0123")
        });
        assert_eq!(draws, MAX_FAIR_ATTEMPTS);
        assert_eq!(forced.attempts, MAX_FAIR_ATTEMPTS);
        assert!(!forced.fair);
    }

    #[test]
    fn same_seed_is_pure_and_layers_do_not_share_a_stream() {
        let seed = RunSeed::parse("MV-7F3A-9C21-L4").unwrap();
        let first = generate_secret(seed);
        let second = generate_secret(seed);
        assert_eq!(first, second);
        assert!(first.fair);
        assert_eq!(first.spec.layer(), 4);
        assert_eq!(first.code.len(), 5);
        assert!(first.code.within_charset(7));
        assert!(is_fair(&first.code, first.spec));

        let layer_one = generate_secret(RunSeed::new(seed.key(), 1).unwrap());
        let layer_two = generate_secret(RunSeed::new(seed.key(), 2).unwrap());
        assert_eq!(layer_one.code.len(), 4);
        assert_ne!(
            layer_one.code, layer_two.code,
            "layer must be part of the stream seed"
        );

        let plan = plan_run(seed.key());
        assert_eq!(plan.layers[3], first);
        assert_eq!(plan.layers[0], layer_one);
        let glyphs: Vec<_> = plan
            .layers
            .iter()
            .map(|layer| layer.code.to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            glyphs,
            ["4114", "2121", "5441", "56041", "10244", "46142", "750562", "176541",]
        );
    }

    #[test]
    fn early_keys_obey_the_guardrail_and_some_of_them_redraw() {
        let mut redraws = 0u32;
        for key in 0..48u32 {
            let generated = generate_secret(RunSeed::new(key, 1).unwrap());
            assert!(generated.fair, "key {key}");
            assert!(
                generated.code.max_frequency() <= 2,
                "key {key} code {}",
                generated.code
            );
            assert!(is_fair(&generated.code, generated.spec));
            if generated.attempts > 1 {
                redraws += 1;
            }
        }
        assert!(
            redraws > 0,
            "the repeat cap should reject at least one of the first draws"
        );
    }
}
