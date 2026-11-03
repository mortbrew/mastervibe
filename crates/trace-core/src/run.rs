//! A run through the L1 slice: practice, two shops, two scored breaches, the boss.

use serde::{Deserialize, Serialize};

use crate::breach::{Act, Breach, End, Loadout, Outcome};
use crate::catalog::{
    interest_for, tool, LevelKind, Levels, ScriptId, ToolId, Verbs, BIN_SLOTS, JACK_PAY,
    START_CASH, TOOL_SLOTS,
};
use crate::score;
use crate::shop::{self, Offer, Shop};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Tutorial,
    Breach,
    Shop,
    LevelUp,
    Cleared,
    Over,
}

#[derive(Clone, Debug)]
pub struct Payout {
    pub name: String,
    pub bits: u32,
    pub amp_milli: i64,
    pub score: u32,
    pub kept: u32,
    pub traced: bool,
    pub jack_cash: u32,
    pub interest: u32,
    pub cred: u32,
    pub target: Option<u32>,
    pub cleared: bool,
}

pub struct Campaign {
    pub seed: u64,
    pub cash: u32,
    pub tools: Vec<ToolId>,
    pub scripts: Vec<ScriptId>,
    pub levels: Levels,
    pub verbs: Verbs,
    pub strace_left: u32,
    pub scores: Vec<u32>,
    pub traced: u32,
    pub breach_index: u8,
    pub phase: Phase,
    pub breach: Option<Breach>,
    pub shop: Option<Shop>,
    pub payout: Option<Payout>,
    pub level_choices: Vec<LevelKind>,
    pub shop_rerolls: u32,
    pub over_line: String,
    pub breach_strace: bool,
}

impl Campaign {
    pub fn new(seed: u64) -> Self {
        let mut campaign = Self::empty(seed);
        campaign.phase = Phase::Tutorial;
        campaign.breach = Some(Breach::practice(&[], Levels::default()));
        campaign
    }

    fn empty(seed: u64) -> Self {
        Self {
            seed,
            cash: START_CASH,
            tools: Vec::new(),
            scripts: Vec::new(),
            levels: Levels::default(),
            verbs: Verbs::default(),
            strace_left: 0,
            scores: Vec::new(),
            traced: 0,
            breach_index: 0,
            phase: Phase::Tutorial,
            breach: None,
            shop: None,
            payout: None,
            level_choices: Vec::new(),
            shop_rerolls: 0,
            over_line: String::new(),
            breach_strace: false,
        }
    }

    pub fn act(&mut self, act: Act) -> Outcome {
        let Some(breach) = self.breach.as_mut() else {
            return Outcome {
                ok: false,
                message: "no breach".into(),
                end: End::None,
                cash: 0,
                double_cash: false,
                arm_strace: false,
            };
        };
        let outcome = breach.act(act);
        if outcome.cash > 0 {
            self.cash = self.cash.saturating_add(outcome.cash);
        }
        if outcome.double_cash {
            let gain = self.cash.min(20);
            self.cash = self.cash.saturating_add(gain);
        }
        if outcome.arm_strace {
            self.strace_left = self.strace_left.max(2);
        }
        if matches!(outcome.end, End::Jacked | End::Traced) {
            let traced = outcome.end == End::Traced;
            self.finish(traced);
        }
        outcome
    }

    fn finish(&mut self, traced: bool) {
        let snapshot = {
            let Some(breach) = self.breach.as_ref() else {
                return;
            };
            let near = !traced && breach.hops == 1;
            let preview = breach.preview(near);
            let kept = if traced {
                score::apply_trace(preview.score, &breach.tools)
            } else {
                preview.score
            };
            (
                Payout {
                    name: format!("{} {}", breach.layer_name, breach.breach_name),
                    bits: preview.bits,
                    amp_milli: preview.amp_milli,
                    score: preview.score,
                    kept,
                    traced,
                    jack_cash: 0,
                    interest: 0,
                    cred: breach.cred_earned,
                    target: breach.target,
                    cleared: breach.target.map(|t| kept >= t).unwrap_or(true),
                },
                breach.verbs,
                breach.levels,
                breach.scripts.clone(),
            )
        };
        let (mut payout, verbs, levels, scripts) = snapshot;
        self.verbs = verbs;
        self.levels = levels;
        self.scripts = scripts;
        if traced {
            self.traced += 1;
        }
        self.breach = None;
        if self.breach_strace && self.strace_left > 0 {
            self.strace_left -= 1;
        }
        self.breach_strace = false;
        if !traced {
            self.cash = self.cash.saturating_add(JACK_PAY);
            payout.jack_cash = JACK_PAY;
        }
        let interest = interest_for(self.cash);
        self.cash = self.cash.saturating_add(interest);
        payout.interest = interest;
        if payout.cleared {
            self.scores.push(payout.kept);
            match self.breach_index {
                0 | 1 => {
                    self.phase = Phase::Shop;
                    self.shop_rerolls = 0;
                    self.shop = Some(self.make_shop());
                }
                _ => {
                    self.phase = Phase::LevelUp;
                    self.level_choices = level_choices(self.levels);
                    self.shop = None;
                }
            }
        } else {
            let target = payout.target.unwrap_or(0);
            let kept = payout.kept;
            self.over_line = if traced {
                format!("TRACED · kept {kept} / {target}")
            } else {
                format!("missed the target · {kept} / {target}")
            };
            self.phase = Phase::Over;
            self.shop = None;
        }
        self.payout = Some(payout);
    }

    pub fn skip_tutorial(&mut self) {
        if self.phase != Phase::Tutorial {
            return;
        }
        let _ = self.act(Act::Jack);
    }

    pub fn leave_shop(&mut self) {
        if self.phase != Phase::Shop {
            return;
        }
        self.breach_index = self.breach_index.saturating_add(1);
        if self.breach_index > 2 {
            self.phase = Phase::Cleared;
            self.shop = None;
            return;
        }
        let strace = self.strace_left > 0;
        self.breach_strace = strace;
        let loadout = Loadout {
            tools: self.tools.clone(),
            levels: self.levels,
            scripts: self.scripts.clone(),
            verbs: self.verbs,
            strace,
        };
        self.breach = Some(Breach::live(self.breach_index, self.seed, &loadout));
        self.phase = Phase::Breach;
        self.shop = None;
    }

    pub fn buy(&mut self, index: usize) -> Result<String, String> {
        let levels = self.levels;
        let offer = self
            .shop
            .as_ref()
            .and_then(|shop| shop.offers.get(index).cloned())
            .flatten()
            .ok_or_else(|| "empty offer".to_string())?;
        let price = offer.price(levels);
        if self.cash < price {
            return Err(format!("need ${price}"));
        }
        match offer {
            Offer::Tool(id) => {
                if self.tools.len() >= TOOL_SLOTS {
                    return Err("slots full — sell one (v)".into());
                }
                if self.tools.contains(&id) {
                    return Err("already installed".into());
                }
                self.cash -= price;
                self.tools.push(id);
            }
            Offer::Level(kind) => {
                if !self.levels.bump(kind) {
                    return Err("already maxed".into());
                }
                self.cash -= price;
            }
            Offer::Script(id) => {
                if self.scripts.len() >= BIN_SLOTS {
                    return Err("~/bin is full".into());
                }
                self.cash -= price;
                self.scripts.push(id);
            }
        }
        if let Some(slot) = self.shop.as_mut().and_then(|s| s.offers.get_mut(index)) {
            *slot = None;
        }
        Ok(format!("bought {}", offer.name()))
    }

    pub fn sell(&mut self, index: usize) -> Result<String, String> {
        if index >= self.tools.len() {
            return Err("no tool there".into());
        }
        let id = self.tools.remove(index);
        let refund = tool(id).price / 2;
        self.cash += refund;
        Ok(format!("sold {} · +${refund}", tool(id).name))
    }

    pub fn reroll(&mut self) -> Result<String, String> {
        let cost = self.shop.as_ref().map(|s| s.reroll_cost).unwrap_or(1);
        if self.cash < cost {
            return Err(format!("reroll needs ${cost}"));
        }
        self.cash -= cost;
        self.shop_rerolls += 1;
        self.shop = Some(self.make_shop());
        Ok(format!("rerolled · next is ${}", self.shop_rerolls + 1))
    }

    pub fn pick_level(&mut self, kind: LevelKind) -> Result<String, String> {
        if self.phase != Phase::LevelUp {
            return Err("no level-up waiting".into());
        }
        if !self.level_choices.contains(&kind) {
            return Err("not on offer".into());
        }
        if !self.levels.bump(kind) {
            return Err("already maxed".into());
        }
        self.phase = Phase::Cleared;
        self.level_choices.clear();
        Ok(format!(
            "{} is now lv{}",
            kind.name(),
            self.levels.get(kind)
        ))
    }

    fn make_shop(&self) -> Shop {
        shop::roll(
            self.seed,
            u32::from(self.breach_index),
            self.shop_rerolls,
            &self.tools,
            self.levels,
            self.verbs,
        )
    }

    pub fn to_save(&self) -> Save {
        Save {
            version: 1,
            seed: self.seed,
            cash: self.cash,
            tools: self.tools.clone(),
            scripts: self.scripts.clone(),
            levels: self.levels,
            verbs: self.verbs,
            strace_left: self.strace_left,
            scores: self.scores.clone(),
            traced: self.traced,
            breach_index: self.breach_index,
            phase: self.phase,
            shop_rerolls: self.shop_rerolls,
            over_line: self.over_line.clone(),
            breach_strace: self.breach_strace,
        }
    }

    pub fn from_save(save: Save) -> Self {
        let mut campaign = Self::empty(save.seed);
        campaign.cash = save.cash;
        campaign.tools = save.tools;
        campaign.scripts = save.scripts;
        campaign.levels = save.levels;
        campaign.verbs = save.verbs;
        campaign.strace_left = save.strace_left;
        campaign.scores = save.scores;
        campaign.traced = save.traced;
        campaign.breach_index = save.breach_index;
        campaign.phase = save.phase;
        campaign.shop_rerolls = save.shop_rerolls;
        campaign.over_line = save.over_line;
        campaign.breach_strace = save.breach_strace;
        match save.phase {
            Phase::Tutorial => {
                campaign.breach = Some(Breach::practice(&campaign.tools, campaign.levels));
            }
            Phase::Breach => {
                let strace = campaign.breach_strace || campaign.strace_left > 0;
                let loadout = Loadout {
                    tools: campaign.tools.clone(),
                    levels: campaign.levels,
                    scripts: campaign.scripts.clone(),
                    verbs: campaign.verbs,
                    strace,
                };
                campaign.breach =
                    Some(Breach::live(campaign.breach_index, campaign.seed, &loadout));
            }
            Phase::Shop => campaign.shop = Some(campaign.make_shop()),
            Phase::LevelUp => campaign.level_choices = level_choices(campaign.levels),
            Phase::Cleared | Phase::Over => {}
        }
        campaign
    }
}

fn level_choices(levels: Levels) -> Vec<LevelKind> {
    let prefer = [
        LevelKind::Log,
        LevelKind::Siphon,
        LevelKind::Key,
        LevelKind::Decrypt,
        LevelKind::Data,
        LevelKind::Bounce,
        LevelKind::Zero,
    ];
    let mut out = Vec::new();
    for kind in prefer {
        if levels.cost(kind).is_some() {
            out.push(kind);
        }
        if out.len() == 3 {
            break;
        }
    }
    out
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Save {
    pub version: u32,
    pub seed: u64,
    pub cash: u32,
    pub tools: Vec<ToolId>,
    pub scripts: Vec<ScriptId>,
    pub levels: Levels,
    pub verbs: Verbs,
    pub strace_left: u32,
    pub scores: Vec<u32>,
    pub traced: u32,
    pub breach_index: u8,
    pub phase: Phase,
    pub shop_rerolls: u32,
    pub over_line: String,
    #[serde(default)]
    pub breach_strace: bool,
}
