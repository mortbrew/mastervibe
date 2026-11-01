//! A headless player.
//!
//! It guesses the lexicographically smallest code still consistent with the
//! pegs, the revealed slots, and the symbols already shown to be absent. In
//! the shop it buys the offered tool it ranks highest, selling a worse one
//! when the rig is full, and rerolls once when nothing was affordable.
//!
//! The ranking is a fixed list, not a search. It is there so a run can be won
//! and lost from the seed alone.

use crate::code::Code;
use crate::pegs::score_pegs;
use crate::rules::BreachRole;
use crate::run::{Phase, Run};

/// What one full play of a seed did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimReport {
    /// Layer 8's Root breach met its target.
    pub won: bool,
    /// Layers whose Root breach met the target.
    pub layers_cleared: u8,
    /// Score of each finished layer, in thousandths, including a layer that
    /// ended the run.
    pub layer_scores_milli: Vec<i64>,
    /// Dollars left at the end.
    pub money: u32,
}

/// Play `key` to the end.
pub fn play(key: u32) -> SimReport {
    let mut run = Run::start(key);
    let mut scores = Vec::new();
    loop {
        let phase = run.state().phase;
        match phase {
            Phase::Won => {
                if let Some(end) = run.state().end {
                    scores.push(end.layer_score_milli);
                }
                return report(true, scores, run.state().money);
            }
            Phase::Lost => {
                if let Some(end) = run.state().end {
                    scores.push(end.layer_score_milli);
                }
                let cleared = u8::try_from(scores.len().saturating_sub(1)).unwrap_or(0);
                return SimReport {
                    won: false,
                    layers_cleared: cleared,
                    layer_scores_milli: scores,
                    money: run.state().money,
                };
            }
            Phase::Shop => {
                let on_root = run.state().role == BreachRole::Root;
                let layer_score = run.state().layer_score_milli;
                shop(&mut run);
                if on_root {
                    scores.push(layer_score);
                }
                run.advance().expect("the shop can be left");
            }
            Phase::Breach => {
                let guess = guess(&run.state());
                run.submit_probe(&guess)
                    .expect("the bot's guess fits the breach");
            }
        }
    }
}

fn report(won: bool, scores: Vec<i64>, money: u32) -> SimReport {
    let layers_cleared = u8::try_from(scores.len()).unwrap_or(0);
    SimReport {
        won,
        layers_cleared,
        layer_scores_milli: scores,
        money,
    }
}

fn shop(run: &mut Run) {
    let mut bought = false;
    for _ in 0..6 {
        let state = run.state();
        let Some(shop) = state.shop else {
            return;
        };
        let best = shop
            .offers
            .iter()
            .enumerate()
            .max_by_key(|(_, offer)| priority(&offer.id));
        let Some((index, offer)) = best else {
            break;
        };
        let rank = priority(&offer.id);
        let price = offer.price;
        if state.tools.len() < state.rig_slots && state.money >= price {
            run.buy(index).expect("the offer is still for sale");
            bought = true;
            continue;
        }
        if state.tools.len() == state.rig_slots {
            if let Some((slot, owned)) = state
                .tools
                .iter()
                .enumerate()
                .min_by_key(|(_, tool)| priority(&tool.id))
            {
                if priority(&owned.id) < rank && state.money + shop.sell_values[slot] >= price {
                    run.sell(slot).expect("the slot is occupied");
                    run.buy(index).expect("selling frees a slot");
                    bought = true;
                    continue;
                }
            }
        }
        break;
    }
    if !bought {
        let state = run.state();
        let afford_reroll = state
            .shop
            .as_ref()
            .map(|shop| state.money >= shop.reroll_cost && state.tools.len() < state.rig_slots)
            .unwrap_or(false);
        if afford_reroll {
            run.reroll().expect("the reroll was affordable");
            if let Some(shop) = run.state().shop {
                if let Some((index, _offer)) = shop
                    .offers
                    .iter()
                    .enumerate()
                    .filter(|(_, offer)| run.state().money >= offer.price)
                    .max_by_key(|(_, offer)| priority(&offer.id))
                {
                    if run.state().tools.len() < run.state().rig_slots {
                        run.buy(index).ok();
                    }
                }
            }
        }
    }
}

fn guess(state: &crate::run::RunState) -> Code {
    let breach = state.breach.as_ref().expect("a breach is open");
    let mut dead = 0u8;
    for symbol in &breach.dead {
        dead |= 1 << symbol.index();
    }
    let revealed: Vec<Option<u8>> = breach
        .revealed
        .iter()
        .map(|slot| slot.map(|symbol| symbol.index()))
        .collect();
    let mut digits = vec![0u8; state.slots];
    let total = state.symbols.pow(u32::try_from(state.slots).unwrap_or(0));
    for _ in 0..total {
        if legal(&digits, &revealed, dead, breach) {
            return Code::try_from_indices(&digits).expect("digits are in range");
        }
        bump(&mut digits, state.symbols);
    }
    Code::try_from_indices(&vec![0u8; state.slots]).expect("a zero code fits")
}

fn legal(
    digits: &[u8],
    revealed: &[Option<u8>],
    dead: u8,
    breach: &crate::run::BreachState,
) -> bool {
    for (slot, reveal) in revealed.iter().enumerate() {
        if let Some(symbol) = reveal {
            if digits[slot] != *symbol {
                return false;
            }
        }
    }
    if digits.iter().any(|symbol| dead & (1 << symbol) != 0) {
        return false;
    }
    let guess = Code::try_from_indices(digits).expect("digits are in range");
    for turn in &breach.history {
        let pegs = score_pegs(&guess, &turn.probe).expect("lengths match");
        if pegs.hits != turn.pegs.hits || pegs.nears != turn.pegs.nears {
            return false;
        }
    }
    true
}

fn bump(digits: &mut [u8], symbols: usize) {
    for slot in (0..digits.len()).rev() {
        digits[slot] = digits[slot].wrapping_add(1);
        if usize::from(digits[slot]) < symbols {
            return;
        }
        digits[slot] = 0;
    }
}

fn priority(id: &str) -> i32 {
    match id {
        "overclock" => 100,
        "zero_day" => 90,
        "precision" => 80,
        "daisy_chain" => 76,
        "shadow" => 72,
        "shoulder_surf" => 70,
        "packet_sniffer" => 68,
        "var_log" => 64,
        "honeypot" => 60,
        "speedrun" => 58,
        "bullseye" => 50,
        "repeater" => 46,
        "brute_force" => 44,
        "slow_burn" => 40,
        "ascii_art" => 36,
        "null_route" => 32,
        "high_value" => 28,
        "keylogger" => 22,
        "crypto_miner" => 18,
        "cold_wallet" => 16,
        "dark_web" => 12,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::play;

    /// Seeds 0..48. Targets in `run.toml` are tuned so this is 15 wins, 31%.
    #[test]
    fn lex_bot_wins_about_a_third_of_the_sample() {
        let mut wins = 0u32;
        let mut first = false;
        let mut fifth = false;
        for key in 0..48u32 {
            let won = play(key).won;
            if won {
                wins += 1;
            }
            if key == 0 {
                first = won;
            }
            if key == 5 {
                fifth = won;
            }
        }
        assert_eq!(wins, 15, "win rate {wins}/48");
        assert!(fifth);
        assert!(!first);
    }
}
