//! Input, saves, and the screen machine. The rules live in `trace_core`.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use serde::{Deserialize, Serialize};
use trace_core::{Act, Campaign, End, Phase};

use crate::audio::{
    Audio, SFX_ALARM, SFX_BLIP, SFX_BOUNCE, SFX_BURN, SFX_CHORD, SFX_CLICK, SFX_COIN, SFX_DENY,
    SFX_STATIC, SFX_TICK, SFX_WIPE, ST_CALM, ST_CLOSE, ST_MID, ST_SHOP, ST_STING, ST_TITLE,
    ST_TRACED, ST_WARN,
};
use crate::ui::theme::PaletteId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Boot,
    Title,
    Options,
    NewRun,
    Play,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    Continue,
    Quit,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub palette: PaletteId,
    pub scanlines: bool,
    pub shake: u8,
    pub speed: u8,
    pub music: u8,
    pub sfx: u8,
    pub adaptive: bool,
    pub mouse: bool,
    pub hover: bool,
    pub confirm_buys: bool,
    pub vim: bool,
    pub math: bool,
    pub tips: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            palette: PaletteId::Default,
            scanlines: false,
            shake: 1,
            speed: 2,
            music: 80,
            sfx: 60,
            adaptive: true,
            mouse: true,
            hover: true,
            confirm_buys: true,
            vim: true,
            math: true,
            tips: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    pub kind: HitKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitKind {
    Menu(usize),
    Opt(usize),
    Tree(usize),
    Tool(usize),
    Script(usize),
    Offer(usize),
    Level(usize),
}

pub struct App {
    pub campaign: Campaign,
    pub screen: Screen,
    pub config: Config,
    pub status: String,
    pub help: bool,
    pub tut_step: u8,
    pub tut_on: bool,
    pub bounced: bool,
    pub saw_games: bool,
    pub menu: usize,
    pub opt: usize,
    pub shop_i: usize,
    pub sell: bool,
    pub sell_i: usize,
    pub armed: Option<usize>,
    pub script_arm: bool,
    pub tool_i: usize,
    pub pinned: bool,
    pub hover_tool: Option<usize>,
    pub hover_offer: Option<usize>,
    pub payout_open: bool,
    pub boot_ms: u64,
    pub elapsed: u64,
    pub blink: bool,
    pub tear: u32,
    pub shake: i32,
    pub reset_at: Option<u64>,
    pub confirm_new: bool,
    pub pending_seed: u64,
    pub home: PathBuf,
    pub audio: Audio,
    pub cols: u16,
    pub rows: u16,
    pub hits: Vec<Hit>,
    pub alarm_on: bool,
    pub level_i: usize,
    pub has_save: bool,
    pub quit: bool,
}

impl App {
    pub fn boot(home: PathBuf, audio: Audio) -> Self {
        let mut app = Self::fresh(home, audio);
        app.config = app.read_config();
        if let Some(saved) = app.read_campaign() {
            app.pending_seed = saved.seed;
            app.has_save = true;
            app.campaign = saved;
            app.menu = 0;
        }
        app.audio.set_levels(app.config.music, app.config.sfx);
        app.audio.set_adaptive(app.config.adaptive);
        app
    }

    pub fn fresh(home: PathBuf, audio: Audio) -> Self {
        Self {
            campaign: Campaign::new(1),
            screen: Screen::Boot,
            config: Config::default(),
            status: String::new(),
            help: false,
            tut_step: 1,
            tut_on: false,
            bounced: false,
            saw_games: false,
            menu: 1,
            opt: 0,
            shop_i: 0,
            sell: false,
            sell_i: 0,
            armed: None,
            script_arm: false,
            tool_i: 0,
            pinned: false,
            hover_tool: None,
            hover_offer: None,
            payout_open: false,
            boot_ms: 0,
            elapsed: 0,
            blink: true,
            tear: 0,
            shake: 0,
            reset_at: None,
            confirm_new: false,
            pending_seed: 0x7F3A_C0DE,
            home,
            audio,
            cols: 80,
            rows: 24,
            hits: Vec::new(),
            alarm_on: false,
            level_i: 0,
            has_save: false,
            quit: false,
        }
    }

    pub fn tick(&mut self, ms: u64) {
        self.elapsed = self.elapsed.saturating_add(ms);
        self.blink = (self.elapsed / 350) % 2 == 0;
        if self.screen == Screen::Boot {
            self.boot_ms = self.boot_ms.saturating_add(ms);
            if self.boot_ms > self.scaled(1500) {
                self.screen = Screen::Title;
            }
        }
        if self.tear > 0 {
            self.tear = self.tear.saturating_sub(ms as u32);
        }
        if self.shake > 0 {
            self.shake -= 1;
        }
        if self.opt == 13 {
            if let Some(start) = self.reset_at {
                let left = self
                    .scaled(3000)
                    .saturating_sub(self.elapsed.saturating_sub(start));
                if left == 0 {
                    self.delete_save();
                    self.reset_at = None;
                    self.status = "save wiped".into();
                } else {
                    self.status =
                        format!("wiping save in {}s — move away to cancel", left / 1000 + 1);
                }
            }
        }
        self.watch_alarm();
        self.sync_audio();
    }

    pub fn scaled(&self, ms: u64) -> u64 {
        let speed = [0.5, 1.0, 1.5, 2.0][self.config.speed as usize % 4];
        (ms as f32 / speed).max(1.0) as u64
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Flow {
        if key.kind == crossterm::event::KeyEventKind::Release {
            return Flow::Continue;
        }
        if self.help {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')
            ) {
                self.help = false;
            }
            return Flow::Continue;
        }
        if self.payout_open {
            if matches!(key.code, KeyCode::Enter | KeyCode::Char(' ')) {
                self.payout_open = false;
                self.audio.sfx(SFX_CLICK);
            }
            return Flow::Continue;
        }
        match self.screen {
            Screen::Boot => {
                self.screen = Screen::Title;
                self.audio.sfx(SFX_CLICK);
            }
            Screen::Title => return self.key_title(key),
            Screen::Options => self.key_options(key),
            Screen::NewRun => self.key_new(key),
            Screen::Play => self.key_play(key),
        }
        Flow::Continue
    }

    pub fn handle_mouse(&mut self, ev: MouseEvent) {
        if !self.config.mouse || self.help {
            return;
        }
        match ev.kind {
            MouseEventKind::Moved => self.hover_at(ev.column, ev.row),
            MouseEventKind::Down(MouseButton::Left) => {
                self.pinned = false;
                self.click_at(ev.column, ev.row);
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if let Some(HitKind::Tool(i)) = self.hit_at(ev.column, ev.row) {
                    self.tool_i = i;
                    self.pinned = true;
                    self.hover_tool = Some(i);
                }
            }
            _ => {}
        }
    }

    pub fn press(&mut self, ch: char) -> Flow {
        self.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
    }

    pub fn press_code(&mut self, code: KeyCode) -> Flow {
        self.handle_key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    pub fn save_blurb(&self) -> String {
        match self.campaign.phase {
            Phase::Tutorial => "L1 tutorial".into(),
            Phase::Breach => format!(
                "L1 B{} · ${}",
                self.campaign.breach_index + 1,
                self.campaign.cash
            ),
            Phase::Shop => format!("shop · ${}", self.campaign.cash),
            Phase::LevelUp => "level-up".into(),
            Phase::Cleared => format!("L1 cleared · ${}", self.campaign.cash),
            Phase::Over => "run over".into(),
        }
    }

    fn key_title(&mut self, key: KeyEvent) -> Flow {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => return Flow::Quit,
            KeyCode::Char('j') | KeyCode::Down => self.move_menu(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_menu(-1),
            KeyCode::Enter | KeyCode::Char('l') => {
                if self.menu == 6 {
                    return Flow::Quit;
                }
                self.activate_menu();
            }
            KeyCode::Char('?') => self.help = true,
            _ => {}
        }
        Flow::Continue
    }

    fn move_menu(&mut self, dir: i32) {
        let n = 7i32;
        for _ in 0..n {
            self.menu = (self.menu as i32 + dir).rem_euclid(n) as usize;
            if self.menu != 0 || self.has_save {
                break;
            }
        }
        self.audio.sfx(SFX_CLICK);
    }

    fn activate_menu(&mut self) {
        self.audio.sfx(SFX_CLICK);
        match self.menu {
            0 if self.has_save => self.continue_save(),
            0 => self.status = "no save".into(),
            1 => {
                self.confirm_new = self.has_save;
                self.screen = Screen::NewRun;
            }
            2..=4 => self.status = "sealed in this build".into(),
            5 => self.screen = Screen::Options,
            6 => {
                // quit handled by returning from a dedicated path
                self.status = "q logs out".into();
            }
            _ => {}
        }
    }

    fn key_options(&mut self, key: KeyEvent) {
        let n = 14i32;
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.reset_at = None;
                self.write_config();
                self.screen = Screen::Title;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.reset_at = None;
                self.opt = (self.opt as i32 + 1).rem_euclid(n) as usize;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.reset_at = None;
                self.opt = (self.opt as i32 - 1).rem_euclid(n) as usize;
            }
            KeyCode::Char('h') | KeyCode::Left => self.tweak_opt(-1),
            KeyCode::Char('l') | KeyCode::Right => self.tweak_opt(1),
            KeyCode::Enter | KeyCode::Char(' ') => self.toggle_opt(),
            _ => {}
        }
    }

    fn tweak_opt(&mut self, dir: i32) {
        match self.opt {
            0 => self.config.palette = self.config.palette.shift(dir),
            2 => {
                self.config.shake = (self.config.shake as i32 + dir).rem_euclid(3) as u8;
            }
            3 => {
                self.config.speed = (self.config.speed as i32 + dir).rem_euclid(4) as u8;
            }
            4 => self.config.music = step_vol(self.config.music, dir),
            5 => self.config.sfx = step_vol(self.config.sfx, dir),
            _ => self.toggle_opt(),
        }
        self.audio.set_levels(self.config.music, self.config.sfx);
        self.audio.set_adaptive(self.config.adaptive);
        self.write_config();
    }

    fn toggle_opt(&mut self) {
        match self.opt {
            0 => self.config.palette = self.config.palette.shift(1),
            1 => self.config.scanlines = !self.config.scanlines,
            2 => self.config.shake = (self.config.shake + 1) % 3,
            3 => self.config.speed = (self.config.speed + 1) % 4,
            6 => self.config.adaptive = !self.config.adaptive,
            7 => self.config.mouse = !self.config.mouse,
            8 => self.config.hover = !self.config.hover,
            9 => self.config.confirm_buys = !self.config.confirm_buys,
            10 => self.config.vim = !self.config.vim,
            11 => self.config.math = !self.config.math,
            12 => self.config.tips = !self.config.tips,
            13 => {
                if self.reset_at.is_none() {
                    self.reset_at = Some(self.elapsed);
                }
            }
            _ => {}
        }
        self.audio.set_levels(self.config.music, self.config.sfx);
        self.audio.set_adaptive(self.config.adaptive);
        self.write_config();
    }

    fn key_new(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                self.confirm_new = false;
                self.screen = Screen::Title;
            }
            KeyCode::Char('s') => {
                self.pending_seed = self.pending_seed.wrapping_mul(0x9E37).wrapping_add(1);
                self.confirm_new = false;
                self.audio.sfx(SFX_TICK);
            }
            KeyCode::Enter => {
                if self.has_save && !self.confirm_new {
                    self.confirm_new = true;
                    return;
                }
                self.begin_run();
            }
            _ => {}
        }
    }

    fn key_play(&mut self, key: KeyEvent) {
        match self.campaign.phase {
            Phase::Shop => self.key_shop(key),
            Phase::LevelUp => self.key_level(key),
            Phase::Cleared => {
                if matches!(key.code, KeyCode::Enter | KeyCode::Char('q') | KeyCode::Esc) {
                    self.screen = Screen::Title;
                }
            }
            Phase::Over => match key.code {
                KeyCode::Char('q') | KeyCode::Esc => self.screen = Screen::Title,
                KeyCode::Char('n') => {
                    self.pending_seed = entropy();
                    self.confirm_new = false;
                    self.screen = Screen::NewRun;
                }
                KeyCode::Char('r') => {
                    self.pending_seed = self.campaign.seed;
                    self.begin_run();
                }
                _ => {}
            },
            Phase::Tutorial | Phase::Breach => self.key_breach(key),
        }
    }

    fn key_breach(&mut self, key: KeyEvent) {
        if self.tut_on && self.tut_step == 6 && matches!(key.code, KeyCode::Enter) {
            self.goto_lesson(7);
            return;
        }
        if matches!(key.code, KeyCode::Esc) && self.tut_on {
            self.skip_tutorial();
            return;
        }
        if key.code == KeyCode::Char('?') {
            self.help = true;
            return;
        }
        if self.script_arm {
            self.script_arm = false;
            if let KeyCode::Char(ch) = key.code {
                if let Some(i) = ch.to_digit(10) {
                    self.apply_act(Act::Script(i as usize - 1));
                }
            }
            return;
        }
        if key.code == KeyCode::Char('!') {
            self.script_arm = true;
            self.status = "script: 1 or 2".into();
            return;
        }
        if key.code == KeyCode::Tab {
            if !self.campaign.tools.is_empty() {
                self.tool_i = (self.tool_i + 1) % self.campaign.tools.len();
                self.pinned = true;
            }
            return;
        }
        if key.code == KeyCode::BackTab {
            if !self.campaign.tools.is_empty() {
                self.tool_i =
                    (self.tool_i + self.campaign.tools.len() - 1) % self.campaign.tools.len();
                self.pinned = true;
            }
            return;
        }
        let act = match key.code {
            KeyCode::Char('j') | KeyCode::Down => Some(Act::Down),
            KeyCode::Char('k') | KeyCode::Up => Some(Act::Up),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => Some(Act::In),
            KeyCode::Char('h') | KeyCode::Left => Some(Act::Out),
            KeyCode::Char('s') => Some(Act::Siphon),
            KeyCode::Char('d') => Some(Act::Decrypt),
            KeyCode::Char('w') => Some(Act::Wipe),
            KeyCode::Char('b') => Some(Act::Bounce),
            KeyCode::Char('x') => Some(Act::Burn),
            KeyCode::Char('q') => Some(Act::Jack),
            KeyCode::Char(' ') => Some(Act::EndTurn),
            _ => None,
        };
        if let Some(act) = act {
            self.apply_act(act);
        }
    }

    fn key_shop(&mut self, key: KeyEvent) {
        if self.sell {
            match key.code {
                KeyCode::Esc | KeyCode::Char('v') => self.sell = false,
                KeyCode::Char('j') | KeyCode::Down => {
                    if !self.campaign.tools.is_empty() {
                        self.sell_i = (self.sell_i + 1) % self.campaign.tools.len();
                    }
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    if !self.campaign.tools.is_empty() {
                        self.sell_i = (self.sell_i + self.campaign.tools.len() - 1)
                            % self.campaign.tools.len();
                    }
                }
                KeyCode::Enter => {
                    let before = self.campaign.cash;
                    match self.campaign.sell(self.sell_i) {
                        Ok(msg) => {
                            self.status = msg;
                            self.audio.sfx(SFX_COIN);
                            if self.sell_i >= self.campaign.tools.len() && self.sell_i > 0 {
                                self.sell_i -= 1;
                            }
                            if self.campaign.cash != before {
                                self.write_save();
                            }
                        }
                        Err(err) => {
                            self.status = err;
                            self.audio.sfx(SFX_DENY);
                        }
                    }
                }
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Char('v') => {
                self.sell = !self.campaign.tools.is_empty();
                self.sell_i = 0;
            }
            KeyCode::Char('r') => match self.campaign.reroll() {
                Ok(msg) => {
                    self.status = msg;
                    self.audio.sfx(SFX_TICK);
                    self.write_save();
                }
                Err(err) => {
                    self.status = err;
                    self.audio.sfx(SFX_DENY);
                }
            },
            KeyCode::Enter => {
                let before = self.campaign.phase;
                self.campaign.leave_shop();
                self.note_phase(before);
                self.audio.sfx(SFX_CLICK);
                self.status = "jacking in".into();
            }
            KeyCode::Tab => {
                let n = self
                    .campaign
                    .shop
                    .as_ref()
                    .map(|s| s.offers.len())
                    .unwrap_or(1);
                if n > 0 {
                    self.shop_i = (self.shop_i + 1) % n;
                }
            }
            KeyCode::Char(ch) if ('1'..='6').contains(&ch) => {
                if let Some(i) = ch.to_digit(10) {
                    self.buy_offer(i as usize - 1);
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => self.status = "enter leaves the shop".into(),
            _ => {}
        }
    }

    fn key_level(&mut self, key: KeyEvent) {
        let n = self.campaign.level_choices.len();
        if n == 0 {
            self.campaign.phase = Phase::Cleared;
            self.write_save();
            return;
        }
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.level_i = (self.level_i + 1) % n,
            KeyCode::Char('k') | KeyCode::Up => self.level_i = (self.level_i + n - 1) % n,
            KeyCode::Enter => self.take_level(self.level_i),
            KeyCode::Char(ch) if ('1'..='3').contains(&ch) => {
                if let Some(i) = ch.to_digit(10) {
                    self.take_level(i as usize - 1);
                }
            }
            _ => {}
        }
    }

    fn take_level(&mut self, index: usize) {
        let Some(kind) = self.campaign.level_choices.get(index).copied() else {
            return;
        };
        match self.campaign.pick_level(kind) {
            Ok(msg) => {
                self.status = msg;
                self.audio.sfx(SFX_CHORD);
                self.write_save();
            }
            Err(err) => self.status = err,
        }
    }

    fn buy_offer(&mut self, index: usize) {
        if self.config.confirm_buys && self.armed != Some(index) {
            self.armed = Some(index);
            self.shop_i = index;
            self.status = "again to buy".into();
            self.audio.sfx(SFX_TICK);
            return;
        }
        self.armed = None;
        match self.campaign.buy(index) {
            Ok(msg) => {
                self.status = msg;
                self.audio.sfx(SFX_COIN);
                self.write_save();
            }
            Err(err) => {
                self.status = err;
                self.audio.sfx(SFX_DENY);
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn debug_act(&mut self, act: Act) {
        self.apply_act(act);
    }

    fn apply_act(&mut self, act: Act) {
        if let Some(why) = self.tutorial_block(act) {
            self.status = why.into();
            self.audio.sfx(SFX_DENY);
            return;
        }
        let before = self.campaign.phase;
        let outcome = self.campaign.act(act);
        self.status = outcome.message.clone();
        if outcome.ok {
            self.sfx_for(act);
            self.after_tutorial(act);
        } else {
            self.audio.sfx(SFX_DENY);
        }
        if matches!(outcome.end, End::Jacked | End::Traced) {
            self.open_payout(outcome.end == End::Traced);
        }
        if self.campaign.phase != Phase::Tutorial {
            self.tut_on = false;
        }
        self.note_phase(before);
        self.sync_audio();
    }

    fn open_payout(&mut self, traced: bool) {
        self.payout_open = true;
        self.tut_on = false;
        if traced {
            self.tear = self.scaled(1100) as u32;
            self.shake = if self.config.shake == 0 { 0 } else { 12 };
            self.audio.sfx(SFX_STATIC);
            self.audio.set_state(ST_TRACED);
        } else {
            self.audio.sfx(SFX_CHORD);
            self.audio.set_state(ST_STING);
        }
    }

    fn tutorial_block(&self, act: Act) -> Option<&'static str> {
        if !self.tut_on {
            return None;
        }
        let allowed = match self.tut_step {
            1 => matches!(act, Act::Up | Act::Down | Act::In | Act::Out),
            2 => matches!(
                act,
                Act::Up | Act::Down | Act::In | Act::Out | Act::Siphon | Act::EndTurn
            ),
            3 => matches!(act, Act::Up | Act::Down | Act::Wipe | Act::EndTurn),
            4 => matches!(
                act,
                Act::Up | Act::Down | Act::In | Act::Out | Act::EndTurn | Act::Siphon
            ),
            5 => matches!(
                act,
                Act::Up
                    | Act::Down
                    | Act::In
                    | Act::Out
                    | Act::Decrypt
                    | Act::Siphon
                    | Act::EndTurn
            ),
            6 => false,
            _ => match act {
                Act::Bounce | Act::Burn | Act::Up | Act::Down | Act::EndTurn => true,
                Act::Jack => self.bounced,
                _ => false,
            },
        };
        if allowed {
            None
        } else {
            Some(match self.tut_step {
                1 => "cd in with l",
                2 => "siphon with s",
                3 => "wipe the LOG with w",
                4 => "space ends the turn",
                5 => "open games/, then the KEY",
                6 => "press enter",
                _ if !self.bounced => "bounce with b, then q",
                _ => "q jacks out",
            })
        }
    }

    fn after_tutorial(&mut self, act: Act) {
        if !self.tut_on {
            return;
        }
        let Some(breach) = self.campaign.breach.as_ref() else {
            return;
        };
        match self.tut_step {
            1 => {
                if act == Act::In && breach.pwd().ends_with("downloads") {
                    self.goto_lesson(2);
                }
            }
            2 => {
                if breach.file_banked("readme.txt") {
                    self.goto_lesson(3);
                }
            }
            3 => {
                if breach.file_wiped("chat.log") {
                    self.goto_lesson(4);
                }
            }
            4 => {
                if act == Act::EndTurn {
                    self.goto_lesson(5);
                }
            }
            5 => {
                if breach.visited_dir("games") {
                    self.saw_games = true;
                }
                if self.saw_games && breach.file_banked("id_rsa") {
                    self.goto_lesson(6);
                }
            }
            7 => {
                if act == Act::Bounce {
                    self.bounced = true;
                    self.status = "proxy up. q banks the haul.".into();
                }
            }
            _ => {}
        }
    }

    fn goto_lesson(&mut self, step: u8) {
        self.tut_step = step;
        if let Some(breach) = self.campaign.breach.as_mut() {
            breach.prepare_lesson(step);
        }
        self.audio.sfx(SFX_CLICK);
    }

    fn skip_tutorial(&mut self) {
        let before = self.campaign.phase;
        self.campaign.skip_tutorial();
        self.tut_on = false;
        self.open_payout(false);
        self.note_phase(before);
    }

    fn begin_run(&mut self) {
        self.campaign = Campaign::new(self.pending_seed);
        self.tut_on = true;
        self.tut_step = 1;
        self.bounced = false;
        self.saw_games = false;
        self.payout_open = false;
        self.confirm_new = false;
        if let Some(breach) = self.campaign.breach.as_mut() {
            breach.prepare_lesson(1);
        }
        self.screen = Screen::Play;
        self.has_save = true;
        self.write_save();
        self.audio.sfx(SFX_CHORD);
    }

    fn continue_save(&mut self) {
        let Some(saved) = self.read_campaign() else {
            self.status = "no save".into();
            self.has_save = false;
            return;
        };
        self.tut_on = saved.phase == Phase::Tutorial;
        self.tut_step = 1;
        self.campaign = saved;
        if self.tut_on {
            if let Some(breach) = self.campaign.breach.as_mut() {
                breach.prepare_lesson(1);
            }
        }
        self.screen = Screen::Play;
        self.payout_open = false;
        self.audio.sfx(SFX_CLICK);
    }

    fn sfx_for(&mut self, act: Act) {
        let id = match act {
            Act::In | Act::Out => SFX_TICK,
            Act::Siphon => SFX_BLIP,
            Act::Wipe => SFX_WIPE,
            Act::Bounce => SFX_BOUNCE,
            Act::Burn => SFX_BURN,
            Act::Decrypt => SFX_TICK,
            Act::Jack => SFX_CHORD,
            Act::Script(_) => SFX_BLIP,
            _ => SFX_CLICK,
        };
        self.audio.sfx(id);
    }

    fn watch_alarm(&mut self) {
        let danger = self
            .campaign
            .breach
            .as_ref()
            .map(|b| b.projected_hops() <= 0)
            .unwrap_or(false);
        if danger && !self.alarm_on && !self.tut_on {
            self.audio.sfx(SFX_ALARM);
        }
        self.alarm_on = danger;
    }

    fn sync_audio(&mut self) {
        self.audio.set_levels(self.config.music, self.config.sfx);
        self.audio.set_adaptive(self.config.adaptive);
        if self.tear > 0 {
            self.audio.set_state(ST_TRACED);
            return;
        }
        if self.payout_open {
            let traced = self
                .campaign
                .payout
                .as_ref()
                .map(|p| p.traced)
                .unwrap_or(false);
            self.audio
                .set_state(if traced { ST_TRACED } else { ST_STING });
            return;
        }
        let state = match self.screen {
            Screen::Boot | Screen::Title | Screen::Options | Screen::NewRun => ST_TITLE,
            Screen::Play => match self.campaign.phase {
                Phase::Shop | Phase::LevelUp => ST_SHOP,
                Phase::Cleared => ST_STING,
                Phase::Over => ST_TRACED,
                Phase::Tutorial | Phase::Breach => self
                    .campaign
                    .breach
                    .as_ref()
                    .map(|b| {
                        let hops = b.projected_hops();
                        if hops <= 0 {
                            ST_WARN
                        } else if b.hops <= 1 {
                            ST_CLOSE
                        } else if b.hops <= 3 {
                            ST_MID
                        } else {
                            ST_CALM
                        }
                    })
                    .unwrap_or(ST_CALM),
            },
        };
        self.audio.set_state(state);
    }

    fn note_phase(&mut self, before: Phase) {
        if self.campaign.phase != before {
            if self.campaign.phase == Phase::Over {
                self.delete_save();
            } else {
                self.write_save();
            }
        }
    }

    fn hover_at(&mut self, x: u16, y: u16) {
        self.hover_tool = None;
        self.hover_offer = None;
        if let Some(HitKind::Tool(i)) = self.hit_at(x, y) {
            if self.config.hover {
                self.hover_tool = Some(i);
            }
        }
        if let Some(HitKind::Offer(i)) = self.hit_at(x, y) {
            if self.config.hover {
                self.hover_offer = Some(i);
                self.shop_i = i;
            }
        }
        if let Some(HitKind::Menu(i)) = self.hit_at(x, y) {
            self.menu = i;
        }
    }

    fn click_at(&mut self, x: u16, y: u16) {
        let Some(kind) = self.hit_at(x, y) else {
            return;
        };
        match kind {
            HitKind::Menu(i) => {
                self.menu = i;
                if i == 6 {
                    self.quit = true;
                } else {
                    self.activate_menu();
                }
            }
            HitKind::Opt(i) => {
                self.opt = i;
                self.toggle_opt();
            }
            HitKind::Tree(i) => {
                if let Some(breach) = self.campaign.breach.as_mut() {
                    breach.cursor = i;
                }
                if let Some(child) = self
                    .campaign
                    .breach
                    .as_ref()
                    .and_then(|b| b.children().get(i).cloned())
                {
                    if child.dir {
                        self.apply_act(Act::In);
                    }
                }
            }
            HitKind::Tool(i) => {
                self.tool_i = i;
                self.pinned = true;
                self.hover_tool = Some(i);
            }
            HitKind::Script(i) => self.apply_act(Act::Script(i)),
            HitKind::Offer(i) => self.buy_offer(i),
            HitKind::Level(i) => self.take_level(i),
        }
    }

    fn hit_at(&self, x: u16, y: u16) -> Option<HitKind> {
        self.hits
            .iter()
            .rev()
            .find(|h| x >= h.x && x < h.x + h.w && y >= h.y && y < h.y + h.h)
            .map(|h| h.kind)
    }

    fn write_save(&mut self) {
        if self.campaign.phase == Phase::Over {
            return;
        }
        let _ = std::fs::create_dir_all(&self.home);
        if let Ok(text) = serde_json::to_string_pretty(&self.campaign.to_save()) {
            if std::fs::write(self.home.join("save.json"), text).is_ok() {
                self.has_save = true;
            }
        }
    }

    fn write_config(&self) {
        let _ = std::fs::create_dir_all(&self.home);
        if let Ok(text) = serde_json::to_string_pretty(&self.config) {
            let _ = std::fs::write(self.home.join("config.json"), text);
        }
    }

    fn read_config(&self) -> Config {
        let path = self.home.join("config.json");
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn read_campaign(&self) -> Option<Campaign> {
        let text = std::fs::read_to_string(self.home.join("save.json")).ok()?;
        let save = serde_json::from_str(&text).ok()?;
        Some(Campaign::from_save(save))
    }

    fn delete_save(&mut self) {
        let _ = std::fs::remove_file(self.home.join("save.json"));
        self.has_save = false;
        self.campaign = Campaign::new(self.pending_seed);
    }
}

fn step_vol(v: u8, dir: i32) -> u8 {
    (v as i32 + dir * 5).clamp(0, 100) as u8
}

fn entropy() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
}

pub fn config_dir() -> PathBuf {
    if let Ok(path) = std::env::var("TRACE_HOME") {
        return PathBuf::from(path);
    }
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".config").join("trace")
}
