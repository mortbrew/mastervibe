//! Rules for ./trace. The terminal never has to know how a score is made.

mod bot;
mod breach;
mod catalog;
mod run;
mod score;
mod shop;
mod tree;

pub use bot::{sensible, simulate, Policy, Report};
pub use breach::{Act, Boss, Breach, ChildView, End, IntentView, Loadout, Outcome, Row, Tone};
pub use catalog::{
    fmt_amp, has_tool, interest_for, parse_seed, script, seed_label, tool, Arch, Intent, Kind,
    LevelKind, Levels, Rarity, ScriptId, ToolId, Verbs, B2_TARGET, B3_TARGET, BIN_SLOTS, DECK_SIZE,
    JACK_PAY, SCRIPTS, START_CASH, TOOLS, TOOL_SLOTS,
};
pub use run::{Campaign, Payout, Phase, Save};
pub use score::{apply_trace, compute, Score};
pub use shop::{Offer, Shop};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deck_is_twenty_and_wipe_pushes_the_trace() {
        let mut breach = Breach::practice(&[], Levels::default());
        assert_eq!(breach.intent_view().total, 20);
        breach.prepare_lesson(3);
        let before = breach.hops;
        let outcome = breach.act(Act::Wipe);
        assert!(outcome.ok, "{}", outcome.message);
        assert!(breach.hops > before);
        assert!(breach.file_wiped("chat.log"));
    }

    #[test]
    fn idle_projection_is_one_hop_closer() {
        let breach = Breach::live(
            1,
            7,
            &Loadout {
                tools: vec![],
                levels: Levels::default(),
                scripts: vec![],
                verbs: Verbs::default(),
                strace: false,
            },
        );
        if breach.intent.kind == catalog_idle() && breach.boss == Boss::None && breach.turn < 4 {
            let view = breach.intent_view();
            if !view.title.contains("TRACE") {
                assert_eq!(breach.projected_hops(), breach.hops as i32 - 1);
            }
        }
    }

    fn catalog_idle() -> crate::catalog::IntentKind {
        crate::catalog::IntentKind::Idle
    }

    #[test]
    fn key_amp_and_trace_keep_half() {
        let score = compute(score::ScoreIn {
            tools: &[],
            levels: Levels::default(),
            bits: 30,
            keys: 1,
            zeros: 0,
            types: 0,
            proxies: 3,
            near_miss: false,
            empty_cycles: false,
            turn: 4,
        });
        assert_eq!(score.amp_milli, 2000);
        assert_eq!(score.score, 60);
        assert_eq!(apply_trace(score.score, &[]), 30);
        assert_eq!(apply_trace(100, &[ToolId::DeadDrop]), 75);
    }

    #[test]
    fn near_miss_adds_one_amp() {
        let score = compute(score::ScoreIn {
            tools: &[],
            levels: Levels::default(),
            bits: 40,
            keys: 0,
            zeros: 0,
            types: 0,
            proxies: 3,
            near_miss: true,
            empty_cycles: false,
            turn: 5,
        });
        assert_eq!(score.amp_milli, 2000);
        assert_eq!(score.score, 80);
    }

    #[test]
    fn interest_caps_at_five() {
        assert_eq!(interest_for(7), 1);
        assert_eq!(interest_for(40), 5);
    }

    #[test]
    fn shop_has_six_offers_and_an_off_build_slot() {
        let owned = vec![ToolId::FatPipe, ToolId::Compressor];
        let shop = shop::roll(3, 0, 0, &owned, Levels::default(), Verbs::default());
        assert!(shop.offers.len() >= 5, "{}", shop.offers.len());
        assert!(shop.lean.contains("greed"), "{}", shop.lean);
        let tools: Vec<_> = shop
            .offers
            .iter()
            .filter_map(|o| match o {
                Some(Offer::Tool(id)) => Some(*id),
                _ => None,
            })
            .collect();
        assert!(
            tools.iter().any(|id| tool(*id).arch != Arch::Greed),
            "expected an off-build offer"
        );
    }

    #[test]
    fn save_roundtrip_restarts_the_breach() {
        let mut campaign = Campaign::new(99);
        let _ = campaign.act(Act::Jack);
        assert_eq!(campaign.phase, Phase::Shop);
        assert_eq!(campaign.cash, 8);
        let save = campaign.to_save();
        let loaded = Campaign::from_save(save);
        assert_eq!(loaded.phase, Phase::Shop);
        assert_eq!(loaded.cash, 8);
    }

    #[test]
    fn tutorial_can_bank_the_readme() {
        let mut breach = Breach::practice(&[], Levels::default());
        breach.prepare_lesson(1);
        assert!(breach.act(Act::In).ok);
        breach.prepare_lesson(2);
        assert!(breach.act(Act::Siphon).ok);
        assert!(breach.act(Act::Siphon).ok);
        assert!(breach.file_banked("readme.txt"));
        assert_eq!(breach.bits, 30);
    }

    #[test]
    fn parent_logs_are_in_the_projection() {
        let breach = Breach::live(
            2,
            11,
            &Loadout {
                tools: vec![],
                levels: Levels::default(),
                scripts: vec![],
                verbs: Verbs::default(),
                strace: false,
            },
        );
        assert_eq!(breach.boss, Boss::Parent);
        assert!(breach.unwiped_logs() >= 3);
        let mut later = Breach::live(
            2,
            11,
            &Loadout {
                tools: vec![],
                levels: Levels::default(),
                scripts: vec![],
                verbs: Verbs::default(),
                strace: true,
            },
        );
        later.turn = 4;
        let view = later.intent_view();
        assert!(view.parent >= 3, "{}", view.detail);
        assert!(view.detail.contains("PARENT"));
    }

    #[test]
    fn sensible_clears_l1_and_greedy_gets_traced() {
        let mut wins = 0;
        let mut greedy_traced = 0;
        let mut greedy_wins = 0;
        let n = 24u32;
        for seed in 1..=n {
            let sensible = simulate(u64::from(seed), Policy::Sensible);
            if sensible.cleared {
                wins += 1;
            }
            let greedy = simulate(u64::from(seed), Policy::Greedy);
            if greedy.traced > 0 {
                greedy_traced += 1;
            }
            if greedy.cleared {
                greedy_wins += 1;
            }
        }
        assert!(wins * 100 / n >= 60, "sensible cleared {wins}/{n}");
        assert!(
            greedy_traced * 100 / n >= 50,
            "greedy traced {greedy_traced}/{n}"
        );
        assert!(
            greedy_wins < wins,
            "greedy wins {greedy_wins} should trail sensible {wins}"
        );
    }
}
