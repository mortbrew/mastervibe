# mastervibes

A turn-based roguelike for the terminal, built on Mastermind code-breaking with a hacker theme.

You probe a hidden code. Pegs tell you which symbols hit and which are only near. The shape of your probe — a pair, a palindrome, a full lock — is its signature, and that signature is what pays. Points are bits times amp. A run is eight layers; each layer is one breach against a new code.

This tree is the v0.1.0 rules core: a library with no UI, plus a stub binary. The terminal comes in a later step.

## Layout

| Path | What it is |
| --- | --- |
| `crates/mastervibes-core` | Seeds, code generation, pegs, signatures, scoring, the consistent-code solver |
| `crates/mastervibes` | Binary stub. It links the core and prints one example secret |
| `crates/mastervibes-core/data/signatures.toml` | Signature payouts and hit/near rates |

The core crate does not depend on a terminal library. Signature numbers are loaded from TOML; the detectors that recognize a pattern stay in code.

## Rules

**Layers.** 1–3 use 4 slots and 6 symbols with 10 probes. 4–6 use 5 slots and 7 symbols with 12 probes. 7–8 use 6 slots and 8 symbols with 14 probes. The charset is the hex prefix `01234567`, cut to the layer's symbol count. Repeats are allowed in a code.

**Seeds.** A share code looks like `MV-7F3A-9C21-L4`: a 32-bit key and a layer. Each layer is its own `rand_pcg` stream, `Pcg32` seeded from `(key << 8) | layer`, so that string reproduces one secret without replaying earlier layers. The same seed always yields the same code.

**Pegs.** Standard Mastermind scoring. Hits are matched first; a symbol already used as a hit is not reused as a near, and duplicate symbols are matched one-to-one.

**Signatures.** A probe earns one signature, the matching pattern with the highest priority. The stock payouts:

| Signature | Bits | Amp bonus |
| --- | --- | --- |
| All Unique | 10 | +0 |
| Pair | 15 | +1 |
| Ascending | 20 | +1 |
| Two Pair | 25 | +1.5 |
| Palindrome | 28 | +1.5 |
| Triple | 30 | +2 |
| Full Lock | 40 | +2.5 |
| Quad | 50 | +3 |
| Mono | 60 | +4 |

Amp starts at 1, then the signature bonus, then pegs (5 bits and +0.25 amp per hit, 2 bits per near). Tools are not in this step; `ScoreHook` is the slot that will add or rewrite bits and amp later. Score is `bits × amp`, stored in thousandths so the halves stay exact.

Ascending means strictly increasing symbol indexes. A palindrome reads the same forwards and backwards and outranks Two Pair, so a four-slot `ABBA` pays as a palindrome. Two Pair is at least two pairs, so three pairs still counts. Full Lock is a triple plus a pair. Triple is a three-of-a-kind with no pair, which is also where two triples land. Quad is four or more of one symbol without filling every slot. Mono is every slot the same.

**Fairness.** A drawn secret is thrown back onto the same PRNG stream when one symbol appears more than twice, or when the built-in solver cannot crack it inside the probe budget. The solver always guesses the lexicographically smallest code still consistent with the pegs. `count_consistent` answers how many secrets still fit the feedback so far.

## Develop

```bash
cargo test
cargo clippy -- -D warnings
cargo run -p mastervibes
```

Licensed under MIT. See [LICENSE](LICENSE).
