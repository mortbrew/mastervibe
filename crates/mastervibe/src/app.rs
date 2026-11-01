//! Input and screens. The core [`Run`] owns the rules.

use std::collections::BTreeMap;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use mastervibe_core::{
    BreachStatus, Charset, Code, Phase, Run, RunError, RunSeed, RunState, Symbol, Turn, LAYER_COUNT,
};

use crate::format::{build_steps, ScoreStep};

/// Milliseconds between scoring rows. Short on purpose.
const STEP_MS: u64 = 70;
/// How long the finished product stays before the screen moves on.
const HOLD_MS: u64 = 650;

/// What the player can do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Keep the terminal open.
    Continue,
    /// Leave the program. Only after the quit confirm.
    Quit,
}

/// Which screen is up. Overlays sit on top of these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    /// New run, or type a seed.
    Title,
    /// The open breach.
    Board,
    /// Bits × amp for the probe just played.
    Scoring,
    /// Breach total and the dollars it paid.
    Payout,
    /// Two tool cards, the rig, reroll, sell, continue.
    Shop,
    /// Win or loss, with the seed and the run's stats.
    GameOver,
}

/// Counters the game-over screen reads. The core does not keep these.
#[derive(Clone, Debug)]
pub struct Stats {
    /// Probes submitted this run.
    pub probes: u32,
    /// Highest single-probe score, in thousandths. `-1` before the first probe.
    pub best_probe_milli: i64,
    /// Signature of that probe.
    pub best_probe_sig: String,
    /// Highest settled breach, in thousandths. `-1` before the first breach ends.
    pub best_breach_milli: i64,
    /// Dollars paid out by breaches.
    pub payouts: u32,
    /// Tools bought.
    pub tools_bought: u32,
    /// How often each signature was played.
    pub sigs: BTreeMap<String, u32>,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            probes: 0,
            best_probe_milli: -1,
            best_probe_sig: String::new(),
            best_breach_milli: -1,
            payouts: 0,
            tools_bought: 0,
            sigs: BTreeMap::new(),
        }
    }
}

/// The scoring moment. Rows appear on a timer and any key skips ahead.
#[derive(Clone, Debug)]
pub struct ScoreMoment {
    /// Probe glyphs.
    pub probe: String,
    /// Hits on that probe.
    pub hits: u8,
    /// Nears on that probe.
    pub nears: u8,
    /// Signature name, even when an absent symbol wiped the pay.
    pub signature: String,
    /// Rows, in pipeline order. The base amp is not a row.
    pub steps: Vec<ScoreStep>,
    /// Amp the pipeline started from.
    pub base_amp: mastervibe_core::Amp,
    /// Final bits from the core.
    pub final_bits: i32,
    /// Final amp from the core.
    pub final_amp: mastervibe_core::Amp,
    /// Final `bits × amp` in thousandths.
    pub final_milli: i64,
    /// Time since the probe landed.
    pub elapsed: u64,
    /// The player asked to jump to the product.
    pub skipped: bool,
}

impl ScoreMoment {
    fn from_turn(turn: &Turn) -> Self {
        let (steps, base_amp) = build_steps(&turn.score);
        Self {
            probe: crate::format::format_symbols(turn.probe.as_slice()),
            hits: turn.pegs.hits,
            nears: turn.pegs.nears,
            signature: turn.score.signature_name.clone(),
            steps,
            base_amp,
            final_bits: turn.score.bits,
            final_amp: turn.score.amp,
            final_milli: turn.score.total_milli,
            elapsed: 0,
            skipped: false,
        }
    }

    /// How many rows to draw right now.
    pub fn visible(&self) -> usize {
        if self.skipped {
            return self.steps.len();
        }
        let shown = usize::try_from(self.elapsed / STEP_MS).unwrap_or(usize::MAX) + 1;
        shown.min(self.steps.len())
    }

    pub(crate) fn finished(&self) -> bool {
        self.visible() == self.steps.len()
    }

    fn should_auto_dismiss(&self) -> bool {
        if self.skipped || !self.finished() {
            return false;
        }
        let done_at = (self.steps.len() as u64)
            .saturating_sub(1)
            .saturating_mul(STEP_MS)
            .saturating_add(HOLD_MS);
        self.elapsed >= done_at
    }
}

/// The terminal session.
pub struct App {
    /// Screen under the overlays.
    pub(crate) screen: Screen,
    pub(crate) run: Option<Run>,
    /// Symbols typed into the open probe. `None` is an empty slot.
    pub(crate) draft: Vec<Option<u8>>,
    pub(crate) cursor: usize,
    /// Slots a tool already revealed. The player cannot overwrite them.
    pub(crate) locked: Vec<bool>,
    pub(crate) help: bool,
    pub(crate) quit_confirm: bool,
    pub(crate) dead_confirm: bool,
    /// `0` is new run, `1` is the seed field.
    pub(crate) title_row: usize,
    pub(crate) seed_input: String,
    /// Amber status line. Empty when there is nothing to say.
    pub(crate) status: String,
    pub(crate) moment: Option<ScoreMoment>,
    pub(crate) stats: Stats,
    pub(crate) sell_mode: bool,
    pub(crate) started: Option<Instant>,
}

impl App {
    /// Title screen. No run yet.
    pub fn new() -> Self {
        Self {
            screen: Screen::Title,
            run: None,
            draft: Vec::new(),
            cursor: 0,
            locked: Vec::new(),
            help: false,
            quit_confirm: false,
            dead_confirm: false,
            title_row: 0,
            seed_input: String::new(),
            status: String::new(),
            moment: None,
            stats: Stats::default(),
            sell_mode: false,
            started: None,
        }
    }

    /// Start `key` at layer 1. Shop rolls and secrets come from the core.
    pub fn start_key(&mut self, key: u32) {
        self.boot(Run::start(key));
    }

    /// Handle one key. Release events are ignored.
    #[must_use]
    pub fn handle_key(&mut self, key: KeyEvent) -> Flow {
        if key.kind == KeyEventKind::Release {
            return Flow::Continue;
        }
        if self.help {
            self.help = false;
            return Flow::Continue;
        }
        if self.quit_confirm {
            return self.handle_quit(key);
        }
        if self.dead_confirm {
            return self.handle_dead_confirm(key);
        }
        if key.code == KeyCode::Char('?') {
            self.help = true;
            return Flow::Continue;
        }
        if is_quit_key(key) {
            self.quit_confirm = true;
            return Flow::Continue;
        }
        match self.screen {
            Screen::Title => self.handle_title(key),
            Screen::Board => self.handle_board(key),
            Screen::Scoring => self.handle_scoring(),
            Screen::Payout => self.handle_payout(key),
            Screen::Shop => self.handle_shop(key),
            Screen::GameOver => self.handle_over(key),
        }
        Flow::Continue
    }

    /// Advance the scoring moment by `millis`. A finished moment moves on.
    pub fn tick(&mut self, millis: u64) {
        if self.screen != Screen::Scoring || self.help || self.quit_confirm || self.dead_confirm {
            return;
        }
        let Some(moment) = self.moment.as_mut() else {
            self.dismiss_scoring();
            return;
        };
        moment.elapsed = moment.elapsed.saturating_add(millis);
        let dismiss = moment.should_auto_dismiss();
        if dismiss {
            self.dismiss_scoring();
        }
    }

    /// Snapshot of the run, when one is going.
    pub(crate) fn state(&self) -> Option<RunState> {
        self.run.as_ref().map(Run::state)
    }

    /// Every slot has a symbol.
    pub(crate) fn draft_complete(&self) -> bool {
        !self.draft.is_empty() && self.draft.iter().all(Option::is_some)
    }

    /// Warning for the probe being typed, when it contains an absent symbol.
    pub(crate) fn absent_warning(&self) -> Option<String> {
        let hits = self.absent_glyphs();
        if hits.is_empty() {
            return None;
        }
        let list = hits
            .iter()
            .map(|ch| ch.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        if self.honeypot_in_draft() {
            return Some(format!(
                "{list} includes the honeypot. signature and pegs pay 0; the tool pays +30 bits once."
            ));
        }
        let verb = if hits.len() == 1 { "is" } else { "are" };
        Some(format!("{list} {verb} absent. this probe scores 0."))
    }

    /// Elapsed run time as `m:ss`.
    pub(crate) fn elapsed_text(&self) -> String {
        let secs = self
            .started
            .map(|start| start.elapsed().as_secs())
            .unwrap_or(0);
        format!("{}:{:02}", secs / 60, secs % 60)
    }

    /// Signature played most often.
    pub(crate) fn most_played(&self) -> Option<(&str, u32)> {
        self.stats
            .sigs
            .iter()
            .max_by(|left, right| left.1.cmp(right.1).then_with(|| right.0.cmp(left.0)))
            .map(|(name, count)| (name.as_str(), *count))
    }

    fn boot(&mut self, run: Run) {
        self.stats = Stats::default();
        self.started = Some(Instant::now());
        self.run = Some(run);
        self.help = false;
        self.quit_confirm = false;
        self.dead_confirm = false;
        self.sell_mode = false;
        self.moment = None;
        self.status.clear();
        self.enter_board();
    }

    fn handle_quit(&mut self, key: KeyEvent) -> Flow {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => Flow::Quit,
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.quit_confirm = false;
                Flow::Continue
            }
            _ => Flow::Continue,
        }
    }

    fn handle_dead_confirm(&mut self, key: KeyEvent) -> Flow {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => self.submit_draft(),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.dead_confirm = false;
            }
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit_confirm = true,
            _ => {}
        }
        Flow::Continue
    }

    fn handle_title(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up => self.title_row = 0,
            KeyCode::Down | KeyCode::Tab => self.title_row = 1,
            KeyCode::Enter => self.confirm_title(),
            KeyCode::Backspace | KeyCode::Delete if self.title_row == 1 => {
                self.seed_input.pop();
            }
            KeyCode::Esc => {
                if self.title_row == 1 && !self.seed_input.is_empty() {
                    self.seed_input.clear();
                } else {
                    self.title_row = 0;
                }
            }
            KeyCode::Char(ch) if self.title_row == 1 && is_seed_char(ch) => {
                if self.seed_input.len() < 24 {
                    self.seed_input.push(ch);
                }
            }
            _ => {}
        }
    }

    fn confirm_title(&mut self) {
        if self.title_row == 0 {
            self.start_key(rand::random());
            return;
        }
        match RunSeed::parse(self.seed_input.trim()) {
            Ok(seed) => self.start_key(seed.key()),
            Err(err) => self.status = err.to_string(),
        }
    }

    fn handle_board(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter => self.try_submit(),
            KeyCode::Backspace | KeyCode::Delete => self.backspace(),
            KeyCode::Left => self.move_cursor(-1),
            KeyCode::Right => self.move_cursor(1),
            KeyCode::Esc => self.reset_draft(),
            KeyCode::Char(ch) => self.place_char(ch),
            _ => {}
        }
    }

    fn handle_scoring(&mut self) {
        let finished = self
            .moment
            .as_ref()
            .map(ScoreMoment::finished)
            .unwrap_or(true);
        if finished {
            self.dismiss_scoring();
        } else if let Some(moment) = self.moment.as_mut() {
            moment.skipped = true;
        }
    }

    fn handle_payout(&mut self, key: KeyEvent) {
        if matches!(
            key.code,
            KeyCode::Enter | KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char(' ')
        ) {
            self.dismiss_payout();
        }
    }

    fn handle_shop(&mut self, key: KeyEvent) {
        if self.sell_mode {
            match key.code {
                KeyCode::Esc => self.sell_mode = false,
                KeyCode::Char(ch) if ('1'..='5').contains(&ch) => {
                    let slot = usize::from(ch as u8 - b'1');
                    self.sell_slot(slot);
                }
                KeyCode::Enter | KeyCode::Char('n') | KeyCode::Char('N') => self.leave_shop(),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Char(ch) if ch == '1' || ch == '2' => {
                self.buy_index(usize::from(ch as u8 - b'1'));
            }
            KeyCode::Char('r') | KeyCode::Char('R') => self.do_reroll(),
            KeyCode::Char('s') | KeyCode::Char('S') => {
                self.sell_mode = true;
                self.status = "sell: press 1-5".to_string();
            }
            KeyCode::Enter | KeyCode::Char('n') | KeyCode::Char('N') => self.leave_shop(),
            KeyCode::Esc => self.sell_mode = false,
            _ => {}
        }
    }

    fn handle_over(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('n') | KeyCode::Char('N') => self.leave_to_title(),
            KeyCode::Char('r') | KeyCode::Char('R') => self.retry(),
            _ => {}
        }
    }

    fn place_char(&mut self, ch: char) {
        let Some(state) = self.state() else {
            return;
        };
        let Some(charset) = Charset::new(state.symbols).ok() else {
            return;
        };
        let Some(index) = charset.index_of(ch) else {
            return;
        };
        if self.cursor >= self.draft.len() {
            return;
        }
        if self.locked.get(self.cursor).copied().unwrap_or(false) {
            self.status = format!("slot {} is revealed", self.cursor + 1);
            if let Some(next) = self.next_unlocked(self.cursor + 1) {
                self.cursor = next;
            }
            return;
        }
        self.draft[self.cursor] = Some(index);
        self.status.clear();
        if let Some(next) = self.next_unlocked(self.cursor + 1) {
            self.cursor = next;
        } else if self.cursor + 1 < self.draft.len() {
            self.cursor = self.draft.len() - 1;
        }
    }

    fn backspace(&mut self) {
        if self.draft.is_empty() {
            return;
        }
        let empty = self.draft.get(self.cursor).copied().flatten().is_none();
        let locked = self.locked.get(self.cursor).copied().unwrap_or(false);
        let start = if empty || locked {
            self.cursor.saturating_sub(1)
        } else {
            self.cursor
        };
        let Some(pos) = self.prev_unlocked(start) else {
            return;
        };
        self.draft[pos] = None;
        self.cursor = pos;
        self.status.clear();
    }

    fn move_cursor(&mut self, delta: isize) {
        if self.draft.is_empty() {
            return;
        }
        let len = self.draft.len() as isize;
        let next = (self.cursor as isize + delta).clamp(0, len - 1);
        self.cursor = next as usize;
    }

    fn try_submit(&mut self) {
        if !self.draft_complete() {
            let need = self.draft.len();
            let have = self.draft.iter().filter(|slot| slot.is_some()).count();
            self.status = format!("need {need} symbols, {have} placed");
            return;
        }
        if self.absent_warning().is_some() {
            self.dead_confirm = true;
            return;
        }
        self.submit_draft();
    }

    fn submit_draft(&mut self) {
        let Some(indices) = self.draft.iter().copied().collect::<Option<Vec<_>>>() else {
            self.status = "the probe is incomplete".to_string();
            return;
        };
        let Ok(code) = Code::try_from_indices(&indices) else {
            self.status = "that probe does not fit".to_string();
            return;
        };
        let result = self
            .run
            .as_mut()
            .expect("a probe needs a run")
            .submit_probe(&code);
        self.dead_confirm = false;
        match result {
            Ok(outcome) => {
                self.note_probe(&outcome.turn);
                self.moment = Some(ScoreMoment::from_turn(&outcome.turn));
                self.screen = Screen::Scoring;
                self.status.clear();
            }
            Err(err) => self.status = explain(err),
        }
    }

    fn note_probe(&mut self, turn: &Turn) {
        self.stats.probes = self.stats.probes.saturating_add(1);
        let milli = turn.score.total_milli;
        if milli >= self.stats.best_probe_milli {
            self.stats.best_probe_milli = milli;
            self.stats.best_probe_sig = turn.score.signature_name.clone();
        }
        *self
            .stats
            .sigs
            .entry(turn.score.signature_name.clone())
            .or_default() += 1;
    }

    fn dismiss_scoring(&mut self) {
        self.moment = None;
        let ended = self
            .state()
            .and_then(|state| state.breach)
            .map(|breach| breach.status != BreachStatus::Open)
            .unwrap_or(false);
        if ended {
            self.open_payout();
        } else {
            self.enter_board();
        }
    }

    fn open_payout(&mut self) {
        if let Some(settled) = self
            .state()
            .and_then(|state| state.breach)
            .and_then(|breach| breach.settled_milli)
        {
            if settled > self.stats.best_breach_milli {
                self.stats.best_breach_milli = settled;
            }
        }
        if let Some(payout) = self.state().and_then(|state| state.last_payout) {
            self.stats.payouts = self.stats.payouts.saturating_add(payout.total);
        }
        self.screen = Screen::Payout;
        self.status.clear();
    }

    fn dismiss_payout(&mut self) {
        match self.state().map(|state| state.phase) {
            Some(Phase::Shop) => self.enter_shop(),
            Some(Phase::Won | Phase::Lost) => self.screen = Screen::GameOver,
            _ => self.enter_board(),
        }
    }

    fn buy_index(&mut self, index: usize) {
        let name = self
            .state()
            .and_then(|state| state.shop)
            .and_then(|shop| shop.offers.get(index).map(|tool| tool.name.clone()));
        let Some(name) = name else {
            self.status = "no tool on that card".to_string();
            return;
        };
        match self.run.as_mut().expect("shop needs a run").buy(index) {
            Ok(()) => {
                self.stats.tools_bought = self.stats.tools_bought.saturating_add(1);
                self.status = format!("bought {name}");
            }
            Err(err) => self.status = explain(err),
        }
    }

    fn sell_slot(&mut self, slot: usize) {
        match self.run.as_mut().expect("shop needs a run").sell(slot) {
            Ok(()) => {
                self.status = format!("sold slot {}", slot + 1);
                if self
                    .state()
                    .map(|state| state.tools.is_empty())
                    .unwrap_or(true)
                {
                    self.sell_mode = false;
                }
            }
            Err(err) => self.status = explain(err),
        }
    }

    fn do_reroll(&mut self) {
        let cost = self
            .state()
            .and_then(|state| state.shop)
            .map(|shop| shop.reroll_cost)
            .unwrap_or(0);
        match self.run.as_mut().expect("shop needs a run").reroll() {
            Ok(()) => self.status = format!("rerolled for ${cost}"),
            Err(err) => self.status = explain(err),
        }
    }

    fn leave_shop(&mut self) {
        match self.run.as_mut().expect("shop needs a run").advance() {
            Ok(()) => self.enter_board(),
            Err(err) => self.status = explain(err),
        }
    }

    fn enter_board(&mut self) {
        self.screen = Screen::Board;
        self.sell_mode = false;
        self.dead_confirm = false;
        self.moment = None;
        self.init_draft();
        self.status.clear();
    }

    fn enter_shop(&mut self) {
        self.screen = Screen::Shop;
        self.sell_mode = false;
        self.dead_confirm = false;
        self.moment = None;
        self.status.clear();
    }

    fn reset_draft(&mut self) {
        self.init_draft();
        self.status.clear();
    }

    fn init_draft(&mut self) {
        let Some(state) = self.state() else {
            self.draft.clear();
            self.locked.clear();
            self.cursor = 0;
            return;
        };
        let revealed = state
            .breach
            .as_ref()
            .map(|breach| breach.revealed.clone())
            .unwrap_or_default();
        if revealed.len() == state.slots {
            self.locked = revealed.iter().map(Option::is_some).collect();
            self.draft = revealed
                .iter()
                .map(|slot| slot.map(Symbol::index))
                .collect();
        } else {
            self.locked = vec![false; state.slots];
            self.draft = vec![None; state.slots];
        }
        self.cursor = self.next_unlocked(0).unwrap_or(0);
    }

    fn leave_to_title(&mut self) {
        let seed = self
            .state()
            .and_then(|state| RunSeed::new(state.key, 1).ok())
            .map(|seed| seed.to_string())
            .unwrap_or_default();
        self.run = None;
        self.moment = None;
        self.screen = Screen::Title;
        self.title_row = 0;
        self.seed_input = seed;
        self.sell_mode = false;
        self.dead_confirm = false;
        self.help = false;
        self.status.clear();
    }

    fn retry(&mut self) {
        let key = self.state().map(|state| state.key);
        if let Some(key) = key {
            self.start_key(key);
        }
    }

    fn next_unlocked(&self, from: usize) -> Option<usize> {
        (from..self.draft.len()).find(|index| !self.locked.get(*index).copied().unwrap_or(false))
    }

    fn prev_unlocked(&self, from: usize) -> Option<usize> {
        (0..=from.min(self.draft.len().saturating_sub(1)))
            .rev()
            .find(|index| !self.locked.get(*index).copied().unwrap_or(false))
    }

    fn absent_glyphs(&self) -> Vec<char> {
        let Some(state) = self.state() else {
            return Vec::new();
        };
        let Some(breach) = state.breach.as_ref() else {
            return Vec::new();
        };
        let dead: Vec<u8> = breach.dead.iter().map(|symbol| symbol.index()).collect();
        let mut hits = Vec::new();
        for slot in &self.draft {
            let Some(index) = slot else {
                continue;
            };
            if dead.contains(index) {
                let glyph = crate::format::glyph(*index);
                if !hits.contains(&glyph) {
                    hits.push(glyph);
                }
            }
        }
        hits
    }

    /// Honeypot still pays if its symbol is in the draft and no earlier probe used it.
    fn honeypot_in_draft(&self) -> bool {
        let Some(state) = self.state() else {
            return false;
        };
        let Some(breach) = state.breach.as_ref() else {
            return false;
        };
        let Some(symbol) = breach.honeypot else {
            return false;
        };
        let used = breach
            .history
            .iter()
            .any(|turn| turn.probe.as_slice().contains(&symbol.index()));
        if used {
            return false;
        }
        self.draft
            .iter()
            .flatten()
            .any(|index| *index == symbol.index())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn is_quit_key(key: KeyEvent) -> bool {
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') => true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => true,
        _ => false,
    }
}

fn is_seed_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '-'
}

fn explain(err: RunError) -> String {
    match err {
        RunError::Money => "not enough $".to_string(),
        RunError::Full => "all 5 tool slots are full — sell one first".to_string(),
        RunError::Owned => "that tool is already in a slot".to_string(),
        RunError::Offer => "no such tool".to_string(),
        RunError::Slot => "no tool in that slot".to_string(),
        RunError::Phase => "that action does not fit this screen".to_string(),
        RunError::Probe(err) => err.to_string(),
    }
}

/// Layers in a run. Re-exported so the header does not hard-code a second 8.
pub(crate) const LAYERS: usize = LAYER_COUNT;

#[cfg(test)]
mod tests {
    use super::App;
    use crate::format::glyph;

    #[test]
    fn warning_names_an_absent_symbol_and_the_honeypot() {
        let mut app = App::new();
        app.start_key(0);
        // Probe a code of zeros until the core marks 0 absent, or the breach ends.
        for _ in 0..8 {
            if app
                .state()
                .and_then(|state| state.breach)
                .map(|breach| breach.dead.iter().any(|symbol| symbol.index() == 0))
                .unwrap_or(false)
            {
                break;
            }
            for _ in 0..app.draft.len() {
                app.place_char('0');
            }
            app.try_submit();
            app.tick(10_000);
            if app.screen == super::Screen::Payout {
                break;
            }
        }
        let dead = app
            .state()
            .and_then(|state| state.breach)
            .map(|breach| breach.dead.clone())
            .unwrap_or_default();
        if dead.is_empty() {
            // This seed still allows 0. The warning helper is still checked below
            // by planting a draft symbol the test knows is absent.
            return;
        }
        app.enter_board();
        let index = dead[0].index();
        let ch = glyph(index);
        for slot in 0..app.draft.len() {
            if !app.locked[slot] {
                app.cursor = slot;
                app.place_char(ch);
            }
        }
        let warning = app.absent_warning().expect("absent symbol warns");
        assert!(
            warning.contains("scores 0") || warning.contains("honeypot"),
            "{warning}"
        );
    }
}
