//! One run: eight layers, three breaches each, then the shop.
//!
//! A UI drives this with [`Run::start`], [`Run::state`], [`Run::submit_probe`],
//! [`Run::buy`], [`Run::sell`], [`Run::reroll`], and [`Run::advance`]. The same
//! seed and the same choices always produce the same run, including shop rolls
//! and revealed slots. [`Run::secret`] is for tests; a terminal UI must not
//! show it.
//!
//! Shop and reveal rolls use a PCG stream with the high bit set, so they never
//! step on the per-layer secret streams.

use std::cell::Cell;

use rand::Rng;
use rand::SeedableRng;
use rand_pcg::Pcg32;
use thiserror::Error;

use crate::breach::{Breach, BreachError, BreachStatus, Turn};
use crate::catalog::{embedded_catalog, Catalog};
use crate::code::{Code, Symbol};
use crate::generate::{draw_secret, layer_stream};
use crate::layer::LayerSpec;
use crate::rules::{embedded_rules, BreachRole, RunRules};
use crate::scoring::ScoreHook;
use crate::seed::RunSeed;
use crate::solver::{knowledge, Observation};
use crate::tool::{embedded_tools, EconomyEffect, InfoEffect, RigHook, Tool, ToolSet};

/// Where the run is waiting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Phase {
    /// A breach is open. Submit probes.
    Breach,
    /// The shop is open. Buy, sell, reroll, or advance.
    Shop,
    /// Layer 8's Root breach met its target.
    Won,
    /// Root ended under the layer target.
    Lost,
}

/// Why the run ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunEnd {
    /// Layer 8 cleared.
    pub won: bool,
    /// Layer the run stopped on.
    pub layer: u8,
    /// Breach the run stopped on.
    pub role: BreachRole,
    /// Sum of the three breach scores, in thousandths of a point.
    pub layer_score_milli: i64,
    /// Layer target, in thousandths of a point.
    pub target_milli: i64,
}

/// Dollars paid when a breach ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Payout {
    /// Perimeter / Internal / Root base.
    pub base: u32,
    /// `$1` per probe under par. Zero when the breach was not cracked.
    pub under_par: u32,
    /// Extra under-par pay from tools.
    pub tools: u32,
    /// Full extra targets the layer score covered. Zero except on a cleared Root.
    pub overkill: u32,
    /// Interest on the money held after the other lines.
    pub interest: u32,
    /// Sum of the lines above.
    pub total: u32,
}

/// A tool card, in the shop or in the rig.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCard {
    /// Stable id.
    pub id: String,
    /// Catalog name.
    pub name: String,
    /// Archetype.
    pub archetype: crate::tool::Archetype,
    /// Rarity letter.
    pub rarity: crate::tool::Rarity,
    /// Shop price.
    pub price: u32,
    /// One-line effect.
    pub summary: String,
}

impl ToolCard {
    fn from_tool(tool: &Tool) -> Self {
        Self {
            id: tool.id.clone(),
            name: tool.name.clone(),
            archetype: tool.archetype,
            rarity: tool.rarity,
            price: tool.price,
            summary: tool.summary.clone(),
        }
    }
}

/// The shop between breaches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShopState {
    /// Cards for sale, left to right.
    pub offers: Vec<ToolCard>,
    /// What the next reroll costs.
    pub reroll_cost: u32,
    /// Refund for each rig slot, parallel to [`RunState::tools`].
    pub sell_values: Vec<u32>,
}

/// The breach currently on screen, including one that just ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BreachState {
    /// Open, cracked, or out of probes.
    pub status: BreachStatus,
    /// Probes already played.
    pub probes_used: usize,
    /// Probes still available, including extras from tools.
    pub probes_left: usize,
    /// Sum of probe scores so far, before the crack multiplier. Thousandths.
    pub probe_score_milli: i64,
    /// Breach score after the crack multiplier or the uncracked keep. Set once
    /// the breach has ended.
    pub settled_milli: Option<i64>,
    /// Crack multiplier in thousandths (`3100` is ×3.1, `500` is the uncracked
    /// keep). Set once the breach has ended.
    pub multiplier_milli: Option<i32>,
    /// Probes, oldest first.
    pub history: Vec<Turn>,
    /// `Some` when that slot has been revealed.
    pub revealed: Vec<Option<Symbol>>,
    /// Symbols already shown to be absent. Playing one scores no signature and no pegs.
    pub dead: Vec<Symbol>,
    /// `Some` when Keylogger is in the rig: whether the secret repeats a symbol.
    pub repeats: Option<bool>,
    /// Symbol Honeypot named, if that tool is in the rig.
    pub honeypot: Option<Symbol>,
    /// Secrets still consistent with the probes, reveals, and named absents.
    pub possible: u64,
}

/// Everything a UI needs to draw the run. The secret is not in here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunState {
    /// Run key. Layer streams and the shop stream both derive from it.
    pub key: u32,
    /// What the player can do.
    pub phase: Phase,
    /// Dollars held.
    pub money: u32,
    /// 1-based layer.
    pub layer: u8,
    /// Perimeter, Internal, or Root.
    pub role: BreachRole,
    /// 1-based breach inside the layer.
    pub breach_index: u8,
    /// Breaches in a layer. Always 3.
    pub breach_count: u8,
    /// Slots in the secret.
    pub slots: usize,
    /// Charset size.
    pub symbols: usize,
    /// Par. Speed tools and under-par pay use this. It is the slot count.
    pub par: u8,
    /// Probes allowed right now, including extras from tools.
    pub probe_budget: u8,
    /// Layer budget before tools.
    pub base_budget: u8,
    /// Layer target, in thousandths of a point.
    pub target_milli: i64,
    /// Finished breach scores this layer, in thousandths. The open breach's
    /// probe sum is not included; add [`BreachState::probe_score_milli`] for a
    /// live reading, and [`BreachState::settled_milli`] once it has ended.
    pub layer_score_milli: i64,
    /// Rig capacity.
    pub rig_slots: usize,
    /// Owned tools, slot order. Score hooks fire in this order.
    pub tools: Vec<ToolCard>,
    /// Present between breaches.
    pub shop: Option<ShopState>,
    /// The breach in progress, or the one the shop is sitting behind.
    pub breach: Option<BreachState>,
    /// Payout of the breach that just ended.
    pub last_payout: Option<Payout>,
    /// Set when the run is over.
    pub end: Option<RunEnd>,
}

/// What [`Run::submit_probe`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProbeOutcome {
    /// The probe, its pegs, and its score.
    pub turn: Turn,
    /// Phase after the probe. A cracking or final probe can land in the shop,
    /// or end the run.
    pub phase: Phase,
    /// Breach score after the multiplier, once the breach has ended.
    pub breach_score_milli: Option<i64>,
    /// Finished breach scores this layer, including the one that just ended.
    pub layer_score_milli: i64,
    /// Pay for the breach, once it has ended.
    pub payout: Option<Payout>,
}

/// A run action was refused.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RunError {
    /// Buy, sell, reroll, advance, or probe was used in the wrong phase.
    #[error("that action does not fit this phase")]
    Phase,
    /// The probe was the wrong shape, or the breach is already over.
    #[error(transparent)]
    Probe(#[from] BreachError),
    /// No shop card or no such tool.
    #[error("no such tool")]
    Offer,
    /// No tool in that rig slot.
    #[error("no tool in that slot")]
    Slot,
    /// The rig already holds that tool.
    #[error("that tool is already in the rig")]
    Owned,
    /// Not enough dollars.
    #[error("not enough money")]
    Money,
    /// All five slots are full.
    #[error("the rig is full")]
    Full,
}

/// A full run. Create one with [`Run::start`].
#[derive(Debug)]
pub struct Run {
    key: u32,
    rules: RunRules,
    tools: ToolSet,
    catalog: Catalog,
    rng: Pcg32,
    layer_rng: Pcg32,
    layer_index: usize,
    breach_index: usize,
    phase: Phase,
    money: u32,
    owned: Vec<String>,
    offers: Vec<String>,
    rerolls_this_shop: u32,
    banked_milli: i64,
    breach: Option<Breach>,
    revealed: Vec<Option<u8>>,
    named_absent: u8,
    repeats: Option<bool>,
    honeypot: Option<u8>,
    honeypot_armed: Cell<bool>,
    blank_armed: Cell<bool>,
    dead_mask: u8,
    possible: u64,
    settled_milli: Option<i64>,
    multiplier_milli: Option<i32>,
    last_payout: Option<Payout>,
    end: Option<RunEnd>,
}

impl Run {
    /// Start a run from `key`. Shop rolls and secrets are determined by it.
    pub fn start(key: u32) -> Self {
        Self::new(
            key,
            embedded_rules().clone(),
            embedded_tools().clone(),
            embedded_catalog().clone(),
        )
    }

    /// Start a run on a specific rules table, tool list, and signature catalog.
    ///
    /// The embedded tables are what a normal game uses. Tests pass alternates.
    pub fn new(key: u32, rules: RunRules, tools: ToolSet, catalog: Catalog) -> Self {
        let layer_rng = layer_stream(key, 1);
        let mut run = Self {
            key,
            rules,
            tools,
            catalog,
            rng: Pcg32::seed_from_u64(shop_seed(key)),
            layer_rng,
            layer_index: 0,
            breach_index: 0,
            phase: Phase::Breach,
            money: 0,
            owned: Vec::new(),
            offers: Vec::new(),
            rerolls_this_shop: 0,
            banked_milli: 0,
            breach: None,
            revealed: Vec::new(),
            named_absent: 0,
            repeats: None,
            honeypot: None,
            honeypot_armed: Cell::new(false),
            blank_armed: Cell::new(true),
            dead_mask: 0,
            possible: 0,
            settled_milli: None,
            multiplier_milli: None,
            last_payout: None,
            end: None,
        };
        run.money = run.rules.starting_money;
        run.open_breach();
        run
    }

    /// Share string for the current layer's first breach. Later breaches of the
    /// layer continue that layer's stream.
    pub fn seed(&self) -> RunSeed {
        RunSeed::new(self.key, self.layer_number()).expect("layer is in range")
    }

    /// The secret of the current breach. Not for the terminal UI.
    pub fn secret(&self) -> Option<&Code> {
        self.breach.as_ref().map(|breach| breach.secret())
    }

    /// Snapshot for a UI.
    pub fn state(&self) -> RunState {
        let rule = self.layer_rule();
        let spec = self.spec();
        let tools: Vec<ToolCard> = self
            .owned
            .iter()
            .filter_map(|id| self.tools.get(id).map(ToolCard::from_tool))
            .collect();
        let shop = if self.phase == Phase::Shop {
            Some(ShopState {
                offers: self
                    .offers
                    .iter()
                    .filter_map(|id| self.tools.get(id).map(ToolCard::from_tool))
                    .collect(),
                reroll_cost: self.reroll_cost(),
                sell_values: tools
                    .iter()
                    .map(|tool| self.sell_value(tool.price))
                    .collect(),
            })
        } else {
            None
        };
        let breach = self.breach.as_ref().map(|breach| BreachState {
            status: breach.status(),
            probes_used: breach.probes_used(),
            probes_left: breach.probes_left(),
            probe_score_milli: probe_sum(breach),
            settled_milli: self.settled_milli,
            multiplier_milli: self.multiplier_milli,
            history: breach.history().to_vec(),
            revealed: self
                .revealed
                .iter()
                .map(|slot| slot.and_then(|index| Symbol::new(index).ok()))
                .collect(),
            dead: mask_symbols(self.dead_mask),
            repeats: self.repeats,
            honeypot: self.honeypot.and_then(|index| Symbol::new(index).ok()),
            possible: self.possible,
        });
        RunState {
            key: self.key,
            phase: self.phase,
            money: self.money,
            layer: self.layer_number(),
            role: self.role(),
            breach_index: u8::try_from(self.breach_index + 1).unwrap_or(1),
            breach_count: u8::try_from(BreachRole::ALL.len()).unwrap_or(3),
            slots: spec.slots(),
            symbols: spec.symbols(),
            par: u8::try_from(spec.slots()).unwrap_or(u8::MAX),
            probe_budget: spec.probe_budget(),
            base_budget: rule.probe_budget,
            target_milli: rule.target_milli(),
            layer_score_milli: self.banked_milli,
            rig_slots: self.rules.rig_slots,
            tools,
            shop,
            breach,
            last_payout: self.last_payout,
            end: self.end.clone(),
        }
    }

    /// Play one probe.
    pub fn submit_probe(&mut self, probe: &Code) -> Result<ProbeOutcome, RunError> {
        if self.phase != Phase::Breach {
            return Err(RunError::Phase);
        }
        let turn = self.play(probe)?;
        self.after_probe();
        let ended = self
            .breach
            .as_ref()
            .map(|breach| !breach.is_open())
            .unwrap_or(true);
        let payout = if ended { Some(self.settle()) } else { None };
        Ok(ProbeOutcome {
            turn,
            phase: self.phase,
            breach_score_milli: self.settled_milli,
            layer_score_milli: self.banked_milli,
            payout,
        })
    }

    /// Buy the shop card at `index` (0-based, left to right).
    pub fn buy(&mut self, index: usize) -> Result<(), RunError> {
        if self.phase != Phase::Shop {
            return Err(RunError::Phase);
        }
        let id = self.offers.get(index).cloned().ok_or(RunError::Offer)?;
        if self.owned.iter().any(|owned| owned == &id) {
            return Err(RunError::Owned);
        }
        let price = self.tools.get(&id).ok_or(RunError::Offer)?.price;
        if self.owned.len() >= self.rules.rig_slots {
            return Err(RunError::Full);
        }
        if self.money < price {
            return Err(RunError::Money);
        }
        self.money -= price;
        self.owned.push(id);
        self.offers.remove(index);
        Ok(())
    }

    /// Sell the rig tool at `slot` for half its price, rounded down.
    pub fn sell(&mut self, slot: usize) -> Result<(), RunError> {
        if self.phase != Phase::Shop {
            return Err(RunError::Phase);
        }
        if slot >= self.owned.len() {
            return Err(RunError::Slot);
        }
        let id = self.owned.remove(slot);
        let price = self.tools.get(&id).map(|tool| tool.price).unwrap_or(0);
        self.money = self.money.saturating_add(self.sell_value(price));
        Ok(())
    }

    /// Replace the shop cards. The first reroll costs the table's base; each
    /// further reroll in this shop costs [`RunRules::reroll_step`] more.
    /// Dark Web lowers the price.
    pub fn reroll(&mut self) -> Result<(), RunError> {
        if self.phase != Phase::Shop {
            return Err(RunError::Phase);
        }
        let cost = self.reroll_cost();
        if self.money < cost {
            return Err(RunError::Money);
        }
        self.money -= cost;
        self.rerolls_this_shop = self.rerolls_this_shop.saturating_add(1);
        self.offers = self.roll_offers();
        Ok(())
    }

    /// Leave the shop and start the next breach.
    pub fn advance(&mut self) -> Result<(), RunError> {
        if self.phase != Phase::Shop {
            return Err(RunError::Phase);
        }
        self.breach_index += 1;
        if self.breach_index >= BreachRole::ALL.len() {
            self.breach_index = 0;
            self.layer_index += 1;
            self.banked_milli = 0;
        }
        self.offers.clear();
        self.open_breach();
        Ok(())
    }

    /// Put `id` in the rig without paying. The next probe sees it. A tool that
    /// changes the breach at start (a reveal, an extra probe) applies immediately
    /// when no probe has been played yet.
    pub fn install(&mut self, id: &str) -> Result<(), RunError> {
        let tool = self.tools.get(id).ok_or(RunError::Offer)?;
        if self.owned.iter().any(|owned| owned == id) {
            return Err(RunError::Owned);
        }
        if self.owned.len() >= self.rules.rig_slots {
            return Err(RunError::Full);
        }
        let extra = tool.extra_probes();
        self.owned.push(id.to_string());
        let fresh = self
            .breach
            .as_ref()
            .map(|breach| breach.probes_used() == 0)
            .unwrap_or(false);
        if fresh {
            if extra > 0 {
                self.widen_budget(extra);
            }
            self.apply_tool_start(id);
            self.refresh_knowledge();
        }
        Ok(())
    }

    fn play(&mut self, probe: &Code) -> Result<Turn, RunError> {
        let product = Cell::new(1000);
        let cap = self.rules.tool_mul_cap.milli();
        let used = self
            .breach
            .as_ref()
            .map(|breach| breach.probes_used())
            .unwrap_or(0);
        let probe_number = u32::try_from(used + 1).unwrap_or(u32::MAX);
        let honeypot = self.honeypot;
        let dead = self.dead_mask;
        let mut breach = self.breach.take().ok_or(RunError::Phase)?;
        let mut hooks = Vec::new();
        for id in &self.owned {
            let Some(tool) = self.tools.get(id) else {
                continue;
            };
            if tool.has_score() {
                hooks.push(RigHook::new(
                    tool,
                    probe_number,
                    honeypot,
                    &self.honeypot_armed,
                    &self.blank_armed,
                    &product,
                    cap,
                ));
            }
        }
        let hook_refs: Vec<&dyn ScoreHook> = hooks.iter().map(|hook| hook as _).collect();
        let played = breach.probe_with_dead(probe, &self.catalog, &hook_refs, dead);
        let turn = played.cloned();
        self.breach = Some(breach);
        Ok(turn?)
    }

    fn after_probe(&mut self) {
        let used = self
            .breach
            .as_ref()
            .map(|breach| breach.probes_used())
            .unwrap_or(0);
        let infos: Vec<InfoEffect> = self
            .owned
            .iter()
            .filter_map(|id| self.tools.get(id))
            .flat_map(|tool| tool.infos().cloned())
            .collect();
        for info in infos {
            if let InfoEffect::NameAbsent { after_probe, count } = info {
                if used == usize::from(after_probe) {
                    self.name_absent(count);
                }
            }
        }
        self.refresh_knowledge();
    }

    fn settle(&mut self) -> Payout {
        let breach = self.breach.as_ref().expect("settling a breach");
        let cracked = breach.status() == BreachStatus::Cracked;
        let used = u32::try_from(breach.probes_used()).unwrap_or(u32::MAX);
        let probes_milli = probe_sum(breach);
        let rule = self.layer_rule().clone();
        let par = u32::from(rule.slots);
        let factor = self.crack_factor(cracked, used, rule.probe_budget, par);
        let total = probes_milli.saturating_mul(i64::from(factor)) / 1000;
        self.settled_milli = Some(total);
        self.multiplier_milli = Some(factor);
        self.banked_milli = self.banked_milli.saturating_add(total);

        let under = if cracked { par.saturating_sub(used) } else { 0 };
        let economies: Vec<EconomyEffect> = self
            .owned
            .iter()
            .filter_map(|id| self.tools.get(id))
            .flat_map(|tool| tool.economies().cloned())
            .collect();
        let mut tool_pay = 0u32;
        let mut interest_extra = 0u32;
        for economy in economies {
            match economy {
                EconomyEffect::PayUnderPar { dollars, cap } => {
                    tool_pay = tool_pay.saturating_add(dollars.saturating_mul(under).min(cap));
                }
                EconomyEffect::InterestCap(extra) => {
                    interest_extra = interest_extra.saturating_add(extra);
                }
                EconomyEffect::RerollDiscount(_) => {}
            }
        }
        let base = self.role().base_pay(&self.rules);
        let under_pay = self.rules.pay_under_par.saturating_mul(under);
        self.money = self
            .money
            .saturating_add(base)
            .saturating_add(under_pay)
            .saturating_add(tool_pay);
        let mut payout = Payout {
            base,
            under_par: under_pay,
            tools: tool_pay,
            overkill: 0,
            interest: 0,
            total: 0,
        };
        let root = self.role() == BreachRole::Root;
        let cleared = self.banked_milli >= rule.target_milli();
        if root && !cleared {
            let cap = self.rules.interest_cap.saturating_add(interest_extra);
            payout.interest = self.rules.interest(self.money, cap);
            self.money = self.money.saturating_add(payout.interest);
            payout.total = base
                .saturating_add(under_pay)
                .saturating_add(tool_pay)
                .saturating_add(payout.interest);
            self.last_payout = Some(payout);
            self.finish(false);
            return payout;
        }
        if root && cleared {
            payout.overkill = self.rules.overkill(self.banked_milli, rule.target_milli());
            self.money = self.money.saturating_add(payout.overkill);
        }
        let cap = self.rules.interest_cap.saturating_add(interest_extra);
        payout.interest = self.rules.interest(self.money, cap);
        self.money = self.money.saturating_add(payout.interest);
        payout.total = base
            .saturating_add(under_pay)
            .saturating_add(tool_pay)
            .saturating_add(payout.overkill)
            .saturating_add(payout.interest);
        self.last_payout = Some(payout);
        if root && self.layer_index + 1 == self.rules.layers.len() {
            self.finish(true);
        } else {
            self.open_shop();
        }
        payout
    }

    fn crack_factor(&self, cracked: bool, used: u32, base_budget: u8, par: u32) -> i32 {
        if !cracked {
            return self.rules.uncracked_keep.milli();
        }
        let unused = i32::try_from(u32::from(base_budget).saturating_sub(used)).unwrap_or(0);
        let mut factor = self
            .rules
            .crack_per_unused
            .milli()
            .saturating_mul(unused)
            .saturating_add(1000);
        if used <= par {
            for id in &self.owned {
                if let Some(bonus) = self.tools.get(id).and_then(|tool| tool.crack_mul()) {
                    factor = i32::try_from(
                        i64::from(factor).saturating_mul(i64::from(bonus.milli())) / 1000,
                    )
                    .unwrap_or(i32::MAX);
                }
            }
        }
        factor.min(self.rules.crack_cap.milli())
    }

    fn finish(&mut self, won: bool) {
        self.phase = if won { Phase::Won } else { Phase::Lost };
        self.end = Some(RunEnd {
            won,
            layer: self.layer_number(),
            role: self.role(),
            layer_score_milli: self.banked_milli,
            target_milli: self.layer_rule().target_milli(),
        });
    }

    fn open_shop(&mut self) {
        self.rerolls_this_shop = 0;
        self.offers = self.roll_offers();
        self.phase = Phase::Shop;
    }

    fn open_breach(&mut self) {
        let layer = self.layer_number();
        if self.breach_index == 0 {
            self.layer_rng = layer_stream(self.key, layer);
            self.banked_milli = 0;
        }
        let rule = self.layer_rule().clone();
        let spec = self
            .rules
            .layer_spec(layer)
            .expect("layer number is in the table");
        let generated = draw_secret(&mut self.layer_rng, spec);
        let extra = self.extra_probes();
        let widened = spec.with_probe_budget(rule.probe_budget.saturating_add(extra));
        let breach = Breach::new(widened, generated.code).expect("drawn secret fits its layer");
        let slots = spec.slots();
        self.breach = Some(breach);
        self.revealed = vec![None; slots];
        self.named_absent = 0;
        self.repeats = None;
        self.honeypot = None;
        self.honeypot_armed.set(false);
        self.blank_armed.set(true);
        self.dead_mask = 0;
        self.possible = 0;
        self.settled_milli = None;
        self.multiplier_milli = None;
        self.phase = Phase::Breach;
        let ids = self.owned.clone();
        for id in &ids {
            self.apply_tool_start(id);
        }
        self.refresh_knowledge();
    }

    fn apply_tool_start(&mut self, id: &str) {
        let infos: Vec<InfoEffect> = self
            .tools
            .get(id)
            .map(|tool| tool.infos().cloned().collect())
            .unwrap_or_default();
        for info in &infos {
            match info {
                InfoEffect::RevealSlot(slot) => self.reveal_fixed(*slot),
                InfoEffect::ShowRepeat => {
                    if let Some(breach) = &self.breach {
                        self.repeats = Some(breach.secret().max_frequency() > 1);
                    }
                }
                InfoEffect::Honeypot => self.plant_honeypot(),
                InfoEffect::RevealRandom(_) | InfoEffect::NameAbsent { .. } => {}
            }
        }
        // Random reveals happen after fixed ones, so a fixed slot is not spent twice.
        for info in &infos {
            if let InfoEffect::RevealRandom(count) = info {
                self.reveal_random(*count);
            }
        }
    }

    fn reveal_fixed(&mut self, slot: u8) {
        let index = usize::from(slot.saturating_sub(1));
        let Some(secret) = self.breach.as_ref().map(|breach| *breach.secret()) else {
            return;
        };
        if index >= self.revealed.len() || self.revealed[index].is_some() {
            return;
        }
        self.revealed[index] = secret.as_slice().get(index).copied();
    }

    fn reveal_random(&mut self, count: u8) {
        for _ in 0..count {
            let open: Vec<usize> = self
                .revealed
                .iter()
                .enumerate()
                .filter_map(|(index, slot)| slot.is_none().then_some(index))
                .collect();
            if open.is_empty() {
                return;
            }
            let pick = open[self.rng.gen_range(0..open.len())];
            let symbol = self
                .breach
                .as_ref()
                .and_then(|breach| breach.secret().as_slice().get(pick).copied());
            self.revealed[pick] = symbol;
        }
    }

    fn plant_honeypot(&mut self) {
        if self.honeypot.is_some() {
            return;
        }
        let Some(symbol) = self.pick_absent() else {
            return;
        };
        self.honeypot = Some(symbol);
        self.honeypot_armed.set(true);
        self.named_absent |= 1 << symbol;
    }

    fn name_absent(&mut self, count: u8) {
        for _ in 0..count {
            let Some(symbol) = self.pick_absent() else {
                return;
            };
            self.named_absent |= 1 << symbol;
        }
    }

    fn pick_absent(&mut self) -> Option<u8> {
        let breach = self.breach.as_ref()?;
        let symbols = breach.spec().symbols();
        let mut present = 0u8;
        for &symbol in breach.secret().as_slice() {
            present |= 1 << symbol;
        }
        let pool: Vec<u8> = (0..symbols)
            .map(|symbol| u8::try_from(symbol).unwrap_or(0))
            .filter(|symbol| present & (1 << symbol) == 0)
            .filter(|symbol| self.named_absent & (1 << symbol) == 0)
            .collect();
        if pool.is_empty() {
            return None;
        }
        Some(pool[self.rng.gen_range(0..pool.len())])
    }

    fn refresh_knowledge(&mut self) {
        let Some(breach) = &self.breach else {
            return;
        };
        let spec = breach.spec();
        let feedback: Vec<Observation> = breach
            .history()
            .iter()
            .map(|turn| Observation {
                probe: turn.probe,
                pegs: turn.pegs,
            })
            .collect();
        match knowledge(
            spec.slots(),
            spec.symbols(),
            &feedback,
            &self.revealed,
            self.named_absent,
        ) {
            Ok((absent, possible)) => {
                self.dead_mask = absent;
                self.possible = possible;
            }
            Err(_) => {
                self.dead_mask = self.named_absent;
                self.possible = 0;
            }
        }
    }

    fn widen_budget(&mut self, extra: u8) {
        let Some(breach) = self.breach.as_ref() else {
            return;
        };
        if breach.probes_used() != 0 {
            return;
        }
        let secret = *breach.secret();
        let spec = breach
            .spec()
            .with_probe_budget(breach.spec().probe_budget().saturating_add(extra));
        self.breach = Breach::new(spec, secret).ok();
    }

    fn roll_offers(&mut self) -> Vec<String> {
        let mut pool: Vec<usize> = self
            .tools
            .tools()
            .iter()
            .enumerate()
            .filter(|(_, tool)| !self.owned.iter().any(|id| id == &tool.id))
            .map(|(index, _)| index)
            .collect();
        let mut offers = Vec::new();
        let want = self.rules.shop_offers.min(pool.len());
        for _ in 0..want {
            let index = self.take_weighted(&mut pool);
            offers.push(self.tools.tools()[index].id.clone());
        }
        offers
    }

    fn take_weighted(&mut self, pool: &mut Vec<usize>) -> usize {
        let total: u32 = pool.iter().map(|&index| self.weight(index)).sum();
        let mut roll = self.rng.gen_range(0..total.max(1));
        let mut chosen = 0usize;
        for (pos, &index) in pool.iter().enumerate() {
            let weight = self.weight(index);
            if roll < weight {
                chosen = pos;
                break;
            }
            roll -= weight;
        }
        pool.remove(chosen)
    }

    fn weight(&self, index: usize) -> u32 {
        let rarity = self.tools.tools()[index].rarity.weight_index();
        self.rules.rarity_weight[rarity]
    }

    fn reroll_cost(&self) -> u32 {
        let discount: u32 = self
            .owned
            .iter()
            .filter_map(|id| self.tools.get(id))
            .flat_map(|tool| tool.economies())
            .map(|economy| match economy {
                EconomyEffect::RerollDiscount(dollars) => *dollars,
                _ => 0,
            })
            .fold(0, u32::saturating_add);
        self.rules
            .reroll_cost
            .saturating_add(
                self.rerolls_this_shop
                    .saturating_mul(self.rules.reroll_step),
            )
            .saturating_sub(discount)
    }

    fn sell_value(&self, price: u32) -> u32 {
        price / self.rules.sell_divisor
    }

    fn extra_probes(&self) -> u8 {
        self.owned
            .iter()
            .filter_map(|id| self.tools.get(id))
            .map(|tool| tool.extra_probes())
            .fold(0u8, u8::saturating_add)
    }

    fn layer_number(&self) -> u8 {
        self.rules.layers[self.layer_index].layer
    }

    fn layer_rule(&self) -> &crate::rules::LayerRule {
        &self.rules.layers[self.layer_index]
    }

    fn role(&self) -> BreachRole {
        BreachRole::from_index(self.breach_index).expect("breach index stays inside the layer")
    }

    fn spec(&self) -> LayerSpec {
        self.breach
            .as_ref()
            .map(|breach| breach.spec())
            .or_else(|| self.rules.layer_spec(self.layer_number()))
            .expect("layer spec")
    }
}

fn probe_sum(breach: &Breach) -> i64 {
    breach
        .history()
        .iter()
        .fold(0i64, |sum, turn| sum.saturating_add(turn.score.total_milli))
}

fn mask_symbols(mask: u8) -> Vec<Symbol> {
    (0..8)
        .filter(|symbol| mask & (1 << symbol) != 0)
        .filter_map(|symbol| Symbol::new(symbol).ok())
        .collect()
}

fn shop_seed(key: u32) -> u64 {
    // Secret streams are `(key << 8) | layer` and fit in 40 bits.
    (1u64 << 63) | (u64::from(key) << 8) | 0x5A
}

#[cfg(test)]
mod tests {
    use super::{Phase, Run};
    use crate::amp::Amp;
    use crate::code::{Charset, Code};
    use crate::scoring::score_probe;

    fn probe(symbols: usize, text: &str) -> Code {
        Code::parse(Charset::new(symbols).unwrap(), text).unwrap()
    }

    #[test]
    fn first_breach_matches_the_share_code_and_replays() {
        let run = Run::start(0x7F3A_9C21);
        let again = Run::start(0x7F3A_9C21);
        assert_eq!(run.secret().unwrap(), again.secret().unwrap());
        assert_eq!(run.state().phase, Phase::Breach);
        assert_eq!(run.state().role.to_string(), "Perimeter");
        assert_eq!(run.state().money, 4);
        assert_eq!(run.state().base_budget, 8);
        assert_eq!(run.state().rig_slots, 5);
        assert!(run.state().shop.is_none());
        assert_eq!(run.seed().to_string(), "MV-7F3A-9C21-L1");
    }

    #[test]
    fn cracking_pays_and_opens_the_shop() {
        let mut run = Run::start(3);
        let secret = *run.secret().unwrap();
        let outcome = run.submit_probe(&secret).unwrap();
        assert_eq!(outcome.phase, Phase::Shop);
        let state = run.state();
        assert_eq!(state.shop.as_ref().unwrap().offers.len(), 2);
        assert_eq!(state.shop.as_ref().unwrap().reroll_cost, 5);
        assert_eq!(state.last_payout.unwrap().base, 5);
        // Cracked on probe 1, 7 unused, ×(1 + 0.3×7) = ×3.1
        assert!(outcome.turn.score.total_milli > 0);
        assert_eq!(state.breach.as_ref().unwrap().multiplier_milli, Some(3100));
        assert!(state.money > 4);
    }

    #[test]
    fn zero_day_caps_the_doubled_crack_bonus() {
        let mut plain = Run::start(3);
        let secret = *plain.secret().unwrap();
        plain.submit_probe(&secret).unwrap();
        let mut boosted = Run::start(3);
        boosted.install("zero_day").unwrap();
        boosted.submit_probe(&secret).unwrap();
        assert_eq!(plain.state().breach.unwrap().multiplier_milli, Some(3100));
        assert_eq!(boosted.state().breach.unwrap().multiplier_milli, Some(5000));
    }

    #[test]
    fn overclock_triples_probe_one_and_the_amp_cap_holds() {
        let mut plain = Run::start(4);
        let opener = probe(6, "0012");
        let plain_turn = plain.submit_probe(&opener).unwrap().turn;
        let mut clocked = Run::start(4);
        clocked.install("overclock").unwrap();
        let clocked_turn = clocked.submit_probe(&opener).unwrap().turn;
        assert_eq!(
            clocked_turn.score.amp,
            plain_turn.score.amp.saturating_mul(Amp::from_milli(3000))
        );

        let mut both = Run::start(5);
        let secret = *both.secret().unwrap();
        let bare = score_probe(
            &secret,
            crate::pegs::score_pegs(&secret, &secret).unwrap(),
            &both.catalog,
            Some(secret_spec(&both)),
            &[],
        );
        both.install("overclock").unwrap();
        both.install("precision").unwrap();
        let turned = both.submit_probe(&secret).unwrap().turn;
        assert_eq!(
            turned.score.amp,
            bare.amp.saturating_mul(Amp::from_milli(4000))
        );
        assert_ne!(
            turned.score.amp,
            bare.amp
                .saturating_mul(Amp::from_milli(3000))
                .saturating_mul(Amp::from_milli(1750))
        );
    }

    fn secret_spec(run: &Run) -> crate::layer::LayerSpec {
        run.breach.as_ref().unwrap().spec()
    }

    #[test]
    fn daisy_chain_adds_half_an_amp_on_top_of_neighbour() {
        let mut plain = Run::start(6);
        let secret = *plain.secret().unwrap();
        let plain_turn = plain.submit_probe(&secret).unwrap().turn;
        let mut chained = Run::start(6);
        chained.install("daisy_chain").unwrap();
        let chained_turn = chained.submit_probe(&secret).unwrap().turn;
        let pairs = i32::from(plain_turn.pegs.adjacent_exact_pairs());
        assert!(pairs > 0);
        assert_eq!(
            chained_turn.score.amp.milli() - plain_turn.score.amp.milli(),
            500 * pairs
        );
    }

    #[test]
    fn honeypot_pays_thirty_bits_once_and_then_the_symbol_is_dead() {
        let mut run = Run::start(8);
        run.install("honeypot").unwrap();
        let symbol = run.state().breach.unwrap().honeypot.unwrap();
        assert!(run.state().breach.unwrap().dead.contains(&symbol));
        let text = symbol.glyph().to_string().repeat(4);
        let first = run.submit_probe(&probe(6, &text)).unwrap().turn;
        assert_eq!(first.score.bits, 30);
        assert!(first
            .score
            .lines
            .iter()
            .any(|line| matches!(line.kind, crate::scoring::ScoreLineKind::Dead)));
        let second = run.submit_probe(&probe(6, &text)).unwrap().turn;
        assert_eq!(second.score.total_milli, 0);
    }

    #[test]
    fn a_blank_probe_makes_its_symbols_dead() {
        let mut run = Run::start(9);
        let first = probe(6, "5555");
        let turn = run.submit_probe(&first).unwrap();
        if turn.turn.pegs.hits == 0 && turn.turn.pegs.nears == 0 {
            assert!(run
                .state()
                .breach
                .unwrap()
                .dead
                .iter()
                .any(|symbol| symbol.glyph() == '5'));
            let wiped = run.submit_probe(&first).unwrap().turn;
            assert_eq!(wiped.score.bits, 0);
        }
    }

    #[test]
    fn shop_reroll_steps_up_and_sell_refunds_half() {
        let mut run = Run::start(3);
        let secret = *run.secret().unwrap();
        run.submit_probe(&secret).unwrap();
        let price = run.state().shop.unwrap().offers[0].price;
        run.buy(0).unwrap();
        assert_eq!(run.state().tools.len(), 1);
        let refund = price / 2;
        let before = run.state().money;
        run.sell(0).unwrap();
        assert_eq!(run.state().money, before + refund);
        assert!(run.state().tools.is_empty());
        let first = run.state().shop.unwrap().reroll_cost;
        assert_eq!(first, 5);
        run.reroll().unwrap();
        assert_eq!(run.state().shop.unwrap().reroll_cost, 6);
        let mut rich = Run::start(3);
        rich.install("dark_web").unwrap();
        let secret = *rich.secret().unwrap();
        rich.submit_probe(&secret).unwrap();
        assert_eq!(rich.state().shop.unwrap().reroll_cost, 4);
    }

    #[test]
    fn the_same_choices_replay() {
        let play = |key| {
            let mut run = Run::start(key);
            let secret = *run.secret().unwrap();
            run.submit_probe(&secret).unwrap();
            if run.state().money >= run.state().shop.as_ref().unwrap().offers[0].price {
                run.buy(0).unwrap();
            }
            run.reroll().ok();
            run.advance().unwrap();
            run.state()
        };
        assert_eq!(play(11), play(11));
        assert_ne!(play(11).key, play(12).key);
    }

    #[test]
    fn missing_the_layer_target_ends_the_run_on_root() {
        let mut text = include_str!("../data/run.toml").to_string();
        text = text.replace("probe_budget = 8", "probe_budget = 1");
        text = text.replace("probe_budget = 7", "probe_budget = 1");
        let rules = crate::rules::RunRules::from_toml(&text).unwrap();
        let mut run = Run::new(
            1,
            rules,
            crate::tool::embedded_tools().clone(),
            crate::catalog::embedded_catalog().clone(),
        );
        for role in ["Perimeter", "Internal", "Root"] {
            assert_eq!(run.state().role.to_string(), role);
            let guess = probe(run.state().symbols, &"0".repeat(run.state().slots));
            let outcome = run.submit_probe(&guess).unwrap();
            if role != "Root" {
                assert_eq!(outcome.phase, Phase::Shop);
                // Uncracked keep is half. One wrong probe should not crack.
                assert_eq!(run.state().breach.unwrap().multiplier_milli, Some(500));
                run.advance().unwrap();
            } else {
                assert_eq!(outcome.phase, Phase::Lost);
                assert!(!run.state().end.unwrap().won);
            }
        }
    }
}
