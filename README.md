# mastervibe

A turn-based roguelike for the terminal, built on Mastermind code-breaking with a hacker theme.

You probe a hidden code. Pegs tell you which symbols are exact and which are only near. The shape of your probe — a pair, a palindrome, a full lock — is its signature, and that signature is what pays. Points are bits times amp. A run is eight layers of three breaches each. Tools from the shop change what a probe is worth and what you know before you guess.

This tree is v0.1.0: the rules core, and a terminal UI that drives it. The UI never reads the secret.

## Layout

| Path | What it is |
| --- | --- |
| `crates/mastervibe-core` | Seeds, code generation, pegs, signatures, scoring, the run loop, tools, the shop |
| `crates/mastervibe` | Terminal UI. It draws the run and calls the core |
| `crates/mastervibe-core/data/signatures.toml` | Signature payouts and peg rates |
| `crates/mastervibe-core/data/run.toml` | Layer budgets and targets, crack bonus, payouts, shop |
| `crates/mastervibe-core/data/tools.toml` | The default-rig tool list |

The core crate does not depend on a terminal library. Signature numbers are loaded from TOML; the detectors that recognize a pattern stay in code.

## Rules

**Layers.** 1–3 use 4 slots and 6 symbols with 8 probes. 4–6 use 5 slots and 7 symbols with 7 probes. 7–8 use 6 slots and 8 symbols with 7 probes. The charset is the hex prefix `01234567`, cut to the layer's symbol count. Repeats are allowed in a code.

**Seeds.** A share code looks like `MV-7F3A-9C21-L4`: a 32-bit key and a layer. Each layer is its own `rand_pcg` stream, `Pcg32` seeded from `(key << 8) | layer`, so that string reproduces one secret without replaying earlier layers. The same seed always yields the same code.

**Pegs.** Standard Mastermind scoring. Hits are matched first; a symbol already used as a hit is not reused as a near, and duplicate symbols are matched one-to-one.

**Signatures.** A probe earns one signature, the matching pattern with the highest `bits × (1 + amp)`. Priority is only the tie-break. The stock payouts:

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

Amp starts at 1, then the signature bonus, then pegs: 10 bits per exact, 4 bits per near, and +1 amp for each pair of side-by-side exacts. A symbol already shown to be absent wipes that probe's signature and pegs (the score is 0 before tools). Score is `bits × amp`, stored in thousandths so the halves stay exact. Tool ×amp effects multiply together and cap at ×4.

**Run.** Eight layers, three breaches each: Perimeter, Internal, Root. The three breach scores add up. If that total is under the layer target when Root ends, the run is over. Clearing layer 8 wins. A cracked breach scores `probe total × (1 + 0.3 per unused probe)`, capped at ×5. Extra probes from tools do not count as unused. An uncracked breach keeps half its probe score and the run continues. You start with $4. Clearing a breach pays $5 / $6 / $7, plus $1 per probe under par (par is the slot count), plus $2 for each extra time the layer score covers the target (max +$3, paid on Root). Then interest: $1 per $5 held, max $5.

**Shop.** After every breach that does not end the run: 2 tool cards, a reroll for $5 (and $1 more each further reroll in that shop), selling for half price. The rig holds 5 tools. The 21 tools are the default-rig slice of the catalog (Recon, Speed, Grind, Pegs, Symbols, Economy, Wild). Same seed, same choices, same shop.

Ascending means strictly increasing symbol indexes. A palindrome reads the same forwards and backwards and outranks Two Pair, so a four-slot `ABBA` pays as a palindrome. Two Pair is at least two pairs, so three pairs still counts. Full Lock is a triple plus a pair. Triple is a three-of-a-kind with no pair, which is also where two triples land. Quad is four or more of one symbol without filling every slot. Mono is every slot the same.

**Fairness.** A drawn secret is thrown back onto the same PRNG stream when one symbol appears more than twice, or when the built-in solver cannot crack it inside the probe budget. The solver always guesses the lexicographically smallest code still consistent with the pegs. `count_consistent` answers how many secrets still fit the feedback so far.

## Play

```bash
cargo run -p mastervibe
```

The terminal needs at least 80×24. A new run draws a random seed. Enter a seed such as `MV-7F3A-9C21-L4` to replay that key. The layer in the seed is a share mark; the run still starts at layer 1.

Keys are on the footer. `q` asks before quitting. `?` explains pegs, signatures, and scoring. Type a symbol with `0`–`7` (only the glyphs this layer has), backspace to undo, enter to probe. An absent symbol is grey, and the game asks before you submit a probe that contains one, because that probe scores 0. In the shop, `1` and `2` buy, `r` rerolls, `s` then `1`–`5` sells, and `n` starts the next breach.

## Develop

```bash
cargo test
cargo clippy -- -D warnings
cargo run -p mastervibe
```

Licensed under MIT. See [LICENSE](LICENSE).
