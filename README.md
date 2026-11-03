# ./trace

A terminal roguelike. You are jacked in through a chain of proxies, and a trace is walking toward you one hop at a time. Each turn you get 3 cycles to move through a file tree, siphon chunks, decrypt locks, wipe logs, and bounce or burn proxies. The sysadmin's next card is always face up, and the ▼ marker shows exactly where the trace will land. Jack out before it reaches `@you`.

This tree replaces the old Mastermind rules core. The slice you can play is layer 1: a 7-step practice breach with `mentor@localhost`, two shops, breach 2, and the boss **THE PARENT**. Clearing it ends on a layer-cleared screen.

## Run

```bash
cargo run --release
```

The binary is `trace`. A seed can be forced:

```bash
cargo run --release -- --seed TRAC-7F3A-C0DE
```

`cargo test` and `cargo clippy --all-targets -- -D warnings` cover the rules, a headless L1 balance sim, and rendered screens at 154×44 and 120×40.

There is no audio device required. If ALSA cannot open a playback device, music and effects stay silent and the game continues. Volumes, the colourblind palettes, and the mouse toggle live on the Options screen and are written to `~/.config/trace` (or `$TRACE_HOME` if that is set).

## Keys

| Key | Action |
| --- | --- |
| `j` / `k` | Move the cursor |
| `l` or enter | `cd` in (1 cycle) |
| `h` | `cd ..` (1 cycle) |
| `s` | Siphon one chunk (1 cycle) |
| `d` | Decrypt a locked folder (1 cycle) |
| `w` | Wipe a LOG, the trace steps back (1 cycle) |
| `b` | Bounce: add a proxy (2 cycles) |
| `x` | Burn a proxy, the trace jumps back 2 (2 cycles) |
| space | End the turn |
| `q` | Jack out and bank bits × amp |
| `!` then `1`/`2` | Run a one-use script from `~/bin` |
| tab | Inspect the focused card |
| `?` | Key reference |

Arrows work as well. In the shop, `1`–`6` buy, `r` rerolls, `v` sells, and enter leaves. The mouse hovers a card to inspect it and clicks a folder to `cd`.

Score is bits × amp. A traced jack-out keeps half. Missing the breach target ends the run. The tutorial cannot fail, and it can be skipped with escape.
