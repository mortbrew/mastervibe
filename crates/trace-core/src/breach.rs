//! One breach: three cycles, then the sysadmin, then the trace moves.

use std::collections::HashSet;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

use crate::catalog::{
    has_tool, Intent, IntentKind, Kind, LevelKind, Levels, ScriptId, ToolId, Verbs, B2_TARGET,
    B3_TARGET, DECK_SIZE,
};
use crate::score::{self, Score};
use crate::tree::{
    count_unwiped_logs, folder_names, gen_b2, gen_b3, reveal_all, tutorial_tree, File, Folder, Node,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Up,
    Down,
    In,
    Out,
    Siphon,
    Decrypt,
    Wipe,
    Bounce,
    Burn,
    Jack,
    EndTurn,
    Script(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    None,
    Turn,
    Jacked,
    Traced,
}

#[derive(Clone, Debug)]
pub struct Outcome {
    pub ok: bool,
    pub message: String,
    pub end: End,
    pub cash: u32,
    pub double_cash: bool,
    pub arm_strace: bool,
}

impl Outcome {
    fn ok(message: impl Into<String>) -> Self {
        Self {
            ok: true,
            message: message.into(),
            end: End::None,
            cash: 0,
            double_cash: false,
            arm_strace: false,
        }
    }

    fn no(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
            end: End::None,
            cash: 0,
            double_cash: false,
            arm_strace: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Boss {
    None,
    Parent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Data,
    Key,
    Cred,
    Log,
    Zero,
    Hidden,
    Locked,
    Looted,
    Dim,
}

#[derive(Clone, Debug)]
pub struct Row {
    pub name: String,
    pub label: String,
    pub detail: String,
    pub tone: Tone,
}

#[derive(Clone, Debug)]
pub struct ChildView {
    pub name: String,
    pub dir: bool,
    pub locked: u8,
    pub visited: bool,
    pub depth: u8,
    pub kind: Option<Kind>,
    pub remaining: u8,
    pub wipeable: bool,
    pub banked: bool,
    pub gone: bool,
}

#[derive(Clone, Debug)]
pub struct IntentView {
    pub title: String,
    pub detail: String,
    pub left: usize,
    pub total: usize,
    pub math: String,
    pub danger: bool,
    pub parent: u32,
}

const T_DATA: u8 = 1;
const T_KEY: u8 = 2;
const T_CRED: u8 = 4;
const T_ZERO: u8 = 8;
const PROXY_CAP: u32 = 6;

pub struct Breach {
    pub root: Folder,
    pub cwd: Vec<usize>,
    pub cursor: usize,
    pub hops: u32,
    pub proxies: u32,
    pub cycles: u32,
    pub turn: u32,
    pub bits: u32,
    pub keys: u32,
    pub zeros: u32,
    pub types: u8,
    pub tools: Vec<ToolId>,
    pub levels: Levels,
    pub scripts: Vec<ScriptId>,
    pub verbs: Verbs,
    pub boss: Boss,
    pub target: Option<u32>,
    pub practice: bool,
    deck: Vec<Intent>,
    discard: Vec<Intent>,
    pub intent: Intent,
    deck_total: usize,
    rng: StdRng,
    visited: HashSet<String>,
    burner_used: bool,
    free_decrypt_used: bool,
    pub zero_day: u32,
    pub cred_earned: u32,
    pub layer_name: String,
    pub breach_name: String,
    ended: bool,
}

pub struct Loadout {
    pub tools: Vec<ToolId>,
    pub levels: Levels,
    pub scripts: Vec<ScriptId>,
    pub verbs: Verbs,
    pub strace: bool,
}

impl Breach {
    pub fn practice(tools: &[ToolId], levels: Levels) -> Self {
        let mut breach = Self::blank(
            tutorial_tree(),
            Boss::None,
            None,
            true,
            tools,
            levels,
            StdRng::seed_from_u64(1),
        );
        breach.hops = 5;
        breach.proxies = 2;
        breach.layer_name = "L1".into();
        breach.breach_name = "practice".into();
        breach.intent = Intent::bare(IntentKind::Idle);
        breach
    }

    pub fn live(index: u8, seed: u64, loadout: &Loadout) -> Self {
        let mut rng = StdRng::seed_from_u64(seed ^ (u64::from(index) << 16) ^ 0x71A3);
        let (tree, boss, target, name) = match index {
            2 => (
                gen_b3(&mut rng, loadout.levels),
                Boss::Parent,
                Some(B3_TARGET),
                "THE PARENT",
            ),
            _ => (
                gen_b2(&mut rng, loadout.levels),
                Boss::None,
                Some(B2_TARGET),
                "home lan",
            ),
        };
        let mut breach = Self::blank(
            tree,
            boss,
            target,
            false,
            &loadout.tools,
            loadout.levels,
            rng,
        );
        breach.scripts = loadout.scripts.clone();
        breach.verbs = loadout.verbs;
        breach.layer_name = "L1".into();
        breach.breach_name = name.into();
        if loadout.strace {
            reveal_all(&mut breach.root);
        }
        if has_tool(&breach.tools, ToolId::SpoofedMac) {
            breach.discard.push(breach.intent.clone());
            breach.intent = Intent::bare(IntentKind::Idle);
        }
        breach
    }

    fn blank(
        root: Folder,
        boss: Boss,
        target: Option<u32>,
        practice: bool,
        tools: &[ToolId],
        levels: Levels,
        mut rng: StdRng,
    ) -> Self {
        let mut deck = fresh_deck(boss);
        deck.shuffle(&mut rng);
        let deck_total = deck.len();
        let mut breach = Self {
            root,
            cwd: Vec::new(),
            cursor: 0,
            hops: 4,
            proxies: 3,
            cycles: 3,
            turn: 1,
            bits: 0,
            keys: 0,
            zeros: 0,
            types: 0,
            tools: tools.to_vec(),
            levels,
            scripts: Vec::new(),
            verbs: Verbs::default(),
            boss,
            target,
            practice,
            deck,
            discard: Vec::new(),
            intent: Intent::bare(IntentKind::Idle),
            deck_total,
            rng,
            visited: HashSet::from(["/".to_string()]),
            burner_used: false,
            free_decrypt_used: false,
            zero_day: 0,
            cred_earned: 0,
            layer_name: "L1".into(),
            breach_name: String::new(),
            ended: false,
        };
        breach.draw_intent();
        if has_tool(&breach.tools, ToolId::Overclock) {
            breach.cycles = 4;
        }
        breach
    }

    pub fn act(&mut self, act: Act) -> Outcome {
        if self.finished() {
            return Outcome::no("breach is over");
        }
        match act {
            Act::Up => self.move_cursor(-1),
            Act::Down => self.move_cursor(1),
            Act::In => self.enter(),
            Act::Out => self.leave(),
            Act::Siphon => self.siphon(),
            Act::Decrypt => self.decrypt(),
            Act::Wipe => self.wipe_selected(),
            Act::Bounce => self.bounce(),
            Act::Burn => self.burn(),
            Act::Jack => self.jack(),
            Act::EndTurn => self.end_turn(),
            Act::Script(i) => self.run_script(i),
        }
    }

    fn finished(&self) -> bool {
        self.ended
    }

    pub fn pwd(&self) -> String {
        let mut parts = vec!["/home".to_string(), self.root.name.clone()];
        let mut folder = &self.root;
        for &i in &self.cwd {
            match folder.kids.get(i) {
                Some(Node::Dir(dir)) => {
                    parts.push(dir.name.clone());
                    folder = dir;
                }
                _ => break,
            }
        }
        parts.join("/")
    }

    pub fn at_root(&self) -> bool {
        self.cwd.is_empty()
    }

    pub fn child_count(&self) -> usize {
        self.here().kids.len()
    }

    pub fn rows(&self) -> Vec<Row> {
        self.here()
            .kids
            .iter()
            .map(|node| match node {
                Node::Dir(dir) => self.dir_row(dir),
                Node::File(file) => file_row(file),
            })
            .collect()
    }

    fn dir_row(&self, dir: &Folder) -> Row {
        let n = dir.file_count();
        let count = format!("{n} file{}", if n == 1 { "" } else { "s" });
        if dir.locked > 0 {
            return Row {
                name: format!("{}/", dir.name),
                label: "DIR".into(),
                detail: format!("[L] locked · decrypt {} · {count}", dir.locked),
                tone: Tone::Locked,
            };
        }
        if !dir.revealed && !dir.visited {
            return Row {
                name: format!("{}/", dir.name),
                label: "DIR".into(),
                detail: format!("▒ hidden · depth {} · {count}", dir.depth),
                tone: Tone::Hidden,
            };
        }
        if dir.is_looted() {
            return Row {
                name: format!("{}/", dir.name),
                label: "DIR".into(),
                detail: "looted".into(),
                tone: Tone::Looted,
            };
        }
        Row {
            name: format!("{}/", dir.name),
            label: "DIR".into(),
            detail: format!("depth {} · {count} · open", dir.depth),
            tone: Tone::Plain,
        }
    }

    pub fn children(&self) -> Vec<ChildView> {
        self.here()
            .kids
            .iter()
            .map(|node| match node {
                Node::Dir(dir) => ChildView {
                    name: dir.name.clone(),
                    dir: true,
                    locked: dir.locked,
                    visited: dir.visited,
                    depth: dir.depth,
                    kind: None,
                    remaining: 0,
                    wipeable: false,
                    banked: false,
                    gone: false,
                },
                Node::File(file) => ChildView {
                    name: file.name.clone(),
                    dir: false,
                    locked: 0,
                    visited: true,
                    depth: file.depth,
                    kind: Some(file.kind),
                    remaining: file.chunks.saturating_sub(file.taken),
                    wipeable: file.kind == Kind::Log && !file.wiped && !file.gone,
                    banked: file.banked,
                    gone: file.gone,
                },
            })
            .collect()
    }

    pub fn intent_view(&self) -> IntentView {
        let parent = if self.boss == Boss::Parent && self.turn >= 4 {
            count_unwiped_logs(&self.root)
        } else {
            0
        };
        let mut detail = match self.intent.kind {
            IntentKind::Idle => "trace +1".to_string(),
            IntentKind::Surge => "the trace moves 2 hops".to_string(),
            IntentKind::Scan => format!("trace +1 · +1 more if you end in {}", self.intent.target),
            IntentKind::Ping => "trace +1 · reveals 1 hidden folder".to_string(),
            IntentKind::Reroute => format!("trace +1 · lose {}", self.intent.target),
            IntentKind::Coffee => "trace +0 · a free breather".to_string(),
            IntentKind::Lock => "trace +1 · bounce costs 3 this turn".to_string(),
            IntentKind::Throttle => "trace +1 · siphon costs 2 this turn".to_string(),
            IntentKind::Audit => format!("trace +1 · deletes 1 file in {}", self.intent.target),
            IntentKind::Lull => "trace +0 · the trace holds".to_string(),
            IntentKind::Patch => "trace +1 · relocks a folder you left".to_string(),
        };
        if parent > 0 {
            detail.push_str(&format!(" · PARENT +{parent}"));
        }
        let proj = self.projected_hops();
        let math = if proj <= 0 && self.burner_would_save() {
            format!("{} - {} = burner holds at 2", self.hops, self.move_amount())
        } else if proj <= 0 {
            format!("{} - {} = TRACED", self.hops, self.move_amount())
        } else {
            format!("{} - {} = {proj}", self.hops, self.move_amount())
        };
        IntentView {
            title: self.intent.titled(),
            detail,
            left: self.deck.len(),
            total: self.deck_total.max(DECK_SIZE),
            math,
            danger: proj <= 0,
            parent,
        }
    }

    pub fn projected_hops(&self) -> i32 {
        let raw = self.hops as i32 - self.move_amount() as i32;
        if raw <= 0 && self.burner_would_save() {
            2
        } else {
            raw
        }
    }

    pub fn burner_would_save(&self) -> bool {
        let raw = self.hops as i32 - self.move_amount() as i32;
        raw <= 0 && has_tool(&self.tools, ToolId::Burner) && !self.burner_used
    }

    pub fn move_amount(&self) -> u32 {
        let mut amount = self.intent.kind.base_move();
        if self.intent.kind == IntentKind::Scan && self.in_named(&self.intent.target) {
            amount += 1;
        }
        if self.boss == Boss::Parent && self.turn >= 4 {
            amount += count_unwiped_logs(&self.root);
        }
        amount
    }

    pub fn unwiped_logs(&self) -> u32 {
        count_unwiped_logs(&self.root)
    }

    pub fn preview(&self, near: bool) -> Score {
        score::compute(score::ScoreIn {
            tools: &self.tools,
            levels: self.levels,
            bits: self.bits,
            keys: self.keys,
            zeros: self.zeros,
            types: self.types,
            proxies: self.proxies,
            near_miss: near,
            empty_cycles: self.cycles == 0,
            turn: self.turn,
        })
    }

    pub fn jack_preview(&self) -> Score {
        self.preview(self.hops == 1)
    }

    pub fn bounce_cost(&self) -> u32 {
        let cheap = has_tool(&self.tools, ToolId::TorPlus) || self.levels.bounce >= 3;
        if self.intent.kind == IntentKind::Lock {
            if cheap {
                2
            } else {
                3
            }
        } else if cheap {
            1
        } else {
            2
        }
    }

    pub fn can_burn(&self) -> bool {
        self.proxies > 1 && self.cycles >= 2
    }

    pub fn file_banked(&self, name: &str) -> bool {
        find_file(&self.root, name)
            .map(|f| f.banked)
            .unwrap_or(false)
    }

    pub fn file_wiped(&self, name: &str) -> bool {
        find_file(&self.root, name)
            .map(|f| f.wiped)
            .unwrap_or(false)
    }

    pub fn visited_dir(&self, name: &str) -> bool {
        dir_flag(&self.root, name, |d| d.visited)
    }

    pub fn prepare_lesson(&mut self, step: u8) {
        match step {
            1 => {
                self.cwd.clear();
                self.hops = 5;
                self.proxies = 2;
                self.cycles = 3;
                self.intent = Intent::bare(IntentKind::Idle);
                self.cursor_on("downloads");
            }
            2 => {
                self.enter_named("downloads");
                self.cycles = self.cycles.max(3);
                self.cursor_on("readme.txt");
            }
            3 => {
                self.bank_named("readme.txt");
                self.cwd.clear();
                self.set_taken("homework.doc", 1);
                self.cursor_on("chat.log");
                self.cycles = 3;
                self.hops = 5;
                self.proxies = 2;
                self.intent = Intent::bare(IntentKind::Idle);
            }
            4 => {
                self.cwd.clear();
                self.cycles = 3;
                self.intent = Intent {
                    kind: IntentKind::Scan,
                    target: "downloads".into(),
                };
            }
            5 => {
                self.cwd.clear();
                self.cycles = 3;
                self.intent = Intent::bare(IntentKind::Idle);
                self.hops = self.hops.max(4);
                self.cursor_on("games");
            }
            6 => {
                self.cwd.clear();
                self.cycles = 3;
                self.hops = 1;
                self.intent = Intent::bare(IntentKind::Surge);
            }
            7 => {
                self.cycles = 3;
                self.hops = self.hops.max(4);
                self.intent = Intent::bare(IntentKind::Lull);
            }
            _ => {}
        }
    }

    fn here(&self) -> &Folder {
        walk(&self.root, &self.cwd)
    }

    fn move_cursor(&mut self, delta: i32) -> Outcome {
        let n = self.child_count();
        if n == 0 {
            return Outcome::no("empty folder");
        }
        let len = n as i32;
        self.cursor = (self.cursor as i32 + delta).rem_euclid(len) as usize;
        let name = self.here().kids[self.cursor].name().to_string();
        Outcome::ok(name)
    }

    fn enter(&mut self) -> Outcome {
        let idx = self.cursor;
        let snapshot = self.child_at(idx);
        let Some(child) = snapshot else {
            return Outcome::no("nothing there");
        };
        if !child.dir {
            return Outcome::no("not a folder");
        }
        if child.locked > 0 {
            return Outcome::no("locked — decrypt first");
        }
        let next = child_key(&self.cwd, idx);
        let cost = if has_tool(&self.tools, ToolId::PipeGrep) && self.visited.contains(&next) {
            0
        } else {
            1
        };
        if !self.spend(cost) {
            return Outcome::no(format!("need {cost} cycle"));
        }
        self.cwd.push(idx);
        self.cursor = 0;
        self.visited.insert(next);
        self.verbs.cd += 1;
        let entered = self.pwd();
        map_cwd(&mut self.root, &self.cwd, |dir| {
            dir.revealed = true;
            dir.visited = true;
        });
        Outcome::ok(format!("cd {entered}"))
    }

    fn leave(&mut self) -> Outcome {
        if self.cwd.is_empty() {
            return Outcome::no("already at the root");
        }
        let parent = parent_key(&self.cwd);
        let cost = if has_tool(&self.tools, ToolId::PipeGrep) && self.visited.contains(&parent) {
            0
        } else {
            1
        };
        if !self.spend(cost) {
            return Outcome::no(format!("need {cost} cycle"));
        }
        let left = self.cwd.pop().unwrap_or(0);
        self.cursor = left;
        self.verbs.cd += 1;
        Outcome::ok(format!("cd {}", self.pwd()))
    }

    fn siphon(&mut self) -> Outcome {
        let idx = self.cursor;
        let info = self.file_info(idx);
        let Some(info) = info else {
            return Outcome::no("nothing to siphon");
        };
        if info.kind == Kind::Log {
            return Outcome::no("a LOG is wiped, not siphoned");
        }
        if info.gone {
            return Outcome::no("file is gone");
        }
        if info.remaining == 0 {
            return Outcome::no("nothing left in that file");
        }
        let cost = if self.intent.kind == IntentKind::Throttle {
            2
        } else {
            1
        };
        if !self.spend(cost) {
            return Outcome::no(format!("need {cost} cycles"));
        }
        let mut take = if self.levels.siphon >= 2 { 2 } else { 1 };
        if has_tool(&self.tools, ToolId::Glitch) && self.rng.gen_bool(0.25) {
            take += 1;
        }
        let name = info.name.clone();
        let mut banked_now = false;
        let mut gained = 0;
        let mut cash = 0;
        map_file(&mut self.root, &self.cwd, idx, |file| {
            let room = file.chunks.saturating_sub(file.taken);
            let take = take.min(room);
            file.taken += take;
            if file.taken >= file.chunks && !file.banked {
                file.banked = true;
                banked_now = true;
            }
        });
        self.verbs.siphon += 1;
        if banked_now {
            if let Some(file) = self.file_info(idx) {
                let mut bits =
                    score::file_bits(file.kind, file.depth, file.chunks, &self.tools, self.levels);
                if self.zero_day > 0 {
                    bits *= 2;
                    self.zero_day -= 1;
                }
                gained = bits;
                self.bits += bits;
                match file.kind {
                    Kind::Key => {
                        self.keys += 1;
                        self.types |= T_KEY;
                    }
                    Kind::Data => self.types |= T_DATA,
                    Kind::Cred => {
                        self.types |= T_CRED;
                        cash = score::cred_cash(&self.tools);
                    }
                    Kind::Zero => {
                        self.zeros += 1;
                        self.types |= T_ZERO;
                    }
                    Kind::Log => {}
                }
            }
        }
        let mut message = if banked_now {
            format!("siphoned {name} · +{gained} bits")
        } else {
            format!("siphoned 1 chunk of {name}")
        };
        if cash > 0 {
            self.cred_earned += cash;
            message.push_str(&format!(" · +${cash}"));
        }
        let mut outcome = Outcome::ok(message);
        outcome.cash = cash;
        outcome
    }

    fn decrypt(&mut self) -> Outcome {
        let idx = self.cursor;
        let Some(child) = self.child_at(idx) else {
            return Outcome::no("nothing there");
        };
        if !child.dir {
            return Outcome::no("not a folder");
        }
        if child.locked == 0 {
            return Outcome::no("not locked");
        }
        let free = self.levels.decrypt >= 3 && !self.free_decrypt_used;
        let cost = if free { 0 } else { 1 };
        if !self.spend(cost) {
            return Outcome::no("need 1 cycle");
        }
        if free {
            self.free_decrypt_used = true;
        }
        let reveal = self.levels.decrypt >= 2;
        let name = child.name.clone();
        map_child_dir(&mut self.root, &self.cwd, idx, |dir| {
            dir.locked = dir.locked.saturating_sub(1);
            if reveal {
                dir.revealed = true;
            }
        });
        self.verbs.decrypt += 1;
        Outcome::ok(format!("decrypted {name}/"))
    }

    fn wipe_selected(&mut self) -> Outcome {
        let idx = self.cursor;
        let Some(info) = self.file_info(idx) else {
            return Outcome::no("nothing to wipe");
        };
        if !info.wipeable {
            return Outcome::no("not a LOG");
        }
        if !self.spend(1) {
            return Outcome::no("need 1 cycle");
        }
        let name = info.name.clone();
        map_file(&mut self.root, &self.cwd, idx, |file| file.wiped = true);
        let push = self.wipe_push();
        self.hops += push;
        self.bits += self.wipe_bits();
        self.verbs.wipe += 1;
        Outcome::ok(format!("wiped {name} · trace +{push}"))
    }

    fn wipe_push(&self) -> u32 {
        let mut push = if self.levels.log >= 2 { 2 } else { 1 };
        if has_tool(&self.tools, ToolId::GhostProcess) {
            push += 1;
        }
        push
    }

    fn wipe_bits(&self) -> u32 {
        let mut bits = 0;
        if has_tool(&self.tools, ToolId::Spool) {
            bits += 15;
        }
        if self.levels.log >= 3 {
            bits += 10;
        }
        bits
    }

    fn bounce(&mut self) -> Outcome {
        if self.proxies >= PROXY_CAP {
            return Outcome::no("proxy chain is full");
        }
        let cost = self.bounce_cost();
        if !self.spend(cost) {
            return Outcome::no(format!("need {cost} cycles"));
        }
        self.proxies += 1;
        self.hops += 1;
        self.verbs.bounce += 1;
        Outcome::ok(format!(
            "bounce · p{} · trace +1 hop",
            self.proxies.saturating_sub(1)
        ))
    }

    fn burn(&mut self) -> Outcome {
        if self.proxies <= 1 {
            return Outcome::no("last proxy — won't burn it");
        }
        if !self.spend(2) {
            return Outcome::no("need 2 cycles");
        }
        self.proxies -= 1;
        self.hops += 2;
        self.verbs.burn += 1;
        Outcome::ok("burned a proxy · trace +2 hops")
    }

    fn jack(&mut self) -> Outcome {
        let preview = self.jack_preview();
        let mut outcome = Outcome::ok(format!(
            "jack out · {} bits × {} = {}",
            preview.bits,
            crate::catalog::fmt_amp(preview.amp_milli),
            preview.score
        ));
        outcome.end = End::Jacked;
        self.ended = true;
        outcome
    }

    fn end_turn(&mut self) -> Outcome {
        self.resolve()
    }

    fn run_script(&mut self, index: usize) -> Outcome {
        let Some(id) = self.scripts.get(index).copied() else {
            return Outcome::no("empty ~/bin slot");
        };
        let name = crate::catalog::script(id).name;
        match id {
            ScriptId::ZeroDay => {
                self.zero_day += 2;
                self.scripts.remove(index);
                Outcome::ok(format!("{name} · next 2 files pay x2"))
            }
            ScriptId::Grep => {
                let mut kind = self.verbs.most_used();
                if self.levels.get(kind) >= 3 {
                    kind = first_open_verb(&self.levels);
                }
                if !self.levels.bump(kind) {
                    return Outcome::no("every verb is already maxed");
                }
                self.scripts.remove(index);
                Outcome::ok(format!(
                    "{name} · {} is now lv{}",
                    kind.name(),
                    self.levels.get(kind)
                ))
            }
            ScriptId::Wget => {
                self.scripts.remove(index);
                let mut outcome = Outcome::ok(format!("{name} · cash doubled"));
                outcome.double_cash = true;
                outcome
            }
            ScriptId::Strace => {
                reveal_all(&mut self.root);
                self.scripts.remove(index);
                let mut outcome = Outcome::ok(format!("{name} · folders revealed"));
                outcome.arm_strace = true;
                outcome
            }
            ScriptId::Rm => {
                let wiped = self.wipe_n_logs(2);
                self.scripts.remove(index);
                Outcome::ok(format!("{name} · wiped {wiped} LOGs"))
            }
        }
    }

    fn wipe_n_logs(&mut self, n: usize) -> usize {
        let paths = log_paths(&self.root, &[]);
        let push = self.wipe_push();
        let bits = self.wipe_bits();
        let mut done = 0;
        for path in paths.into_iter().take(n) {
            if wipe_at(&mut self.root, &path) {
                self.hops += push;
                self.bits += bits;
                self.verbs.wipe += 1;
                done += 1;
            }
        }
        done
    }

    fn resolve(&mut self) -> Outcome {
        let amount = self.move_amount();
        let side = self.apply_side_effects();
        let caught = self.hops <= amount;
        let mut note = if caught && has_tool(&self.tools, ToolId::Burner) && !self.burner_used {
            self.hops = 2;
            self.burner_used = true;
            "burner catches the trace at 2 hops".to_string()
        } else if caught && self.practice {
            self.hops = 2;
            "practice: the trace stops 2 hops out".to_string()
        } else if caught {
            self.hops = 0;
            self.ended = true;
            let mut outcome = Outcome::ok("TRACED");
            outcome.end = End::Traced;
            return outcome;
        } else {
            self.hops -= amount;
            format!("trace moves {amount} · {} hops left", self.hops)
        };
        if !side.is_empty() {
            note.push_str(" · ");
            note.push_str(&side);
        }
        self.discard.push(self.intent.clone());
        self.turn += 1;
        self.draw_intent();
        let carry = if self.cycles > 0 && has_tool(&self.tools, ToolId::CronJob) {
            1
        } else {
            0
        };
        self.cycles = 3 + carry;
        let mut outcome = Outcome::ok(note);
        outcome.end = End::Turn;
        outcome
    }

    fn apply_side_effects(&mut self) -> String {
        match self.intent.kind {
            IntentKind::Ping => {
                if reveal_one(&mut self.root) {
                    "ping reveals a folder".into()
                } else {
                    String::new()
                }
            }
            IntentKind::Reroute => {
                if self.proxies > 1 {
                    self.proxies -= 1;
                    "reroute drops a proxy".into()
                } else {
                    "reroute finds nothing to drop".into()
                }
            }
            IntentKind::Audit => {
                let target = self.intent.target.clone();
                if audit_named(&mut self.root, &target, &mut self.rng) {
                    format!("audit deletes a file in {target}")
                } else {
                    "audit finds nothing".into()
                }
            }
            IntentKind::Patch => {
                let current = self.here().name.clone();
                if patch_one(&mut self.root, &current) {
                    "patch relocks a folder".into()
                } else {
                    String::new()
                }
            }
            _ => String::new(),
        }
    }

    fn draw_intent(&mut self) {
        if self.deck.is_empty() {
            self.deck = fresh_deck(self.boss);
            self.deck.shuffle(&mut self.rng);
            self.deck_total = self.deck.len();
        }
        let mut intent = self
            .deck
            .pop()
            .unwrap_or_else(|| Intent::bare(IntentKind::Idle));
        if self.turn >= 6 && self.turn % 3 == 0 && intent.kind == IntentKind::Idle {
            intent.kind = IntentKind::Surge;
        }
        let mut names = Vec::new();
        folder_names(&self.root, &mut names);
        if names.is_empty() {
            names.push("home".into());
        }
        let pick = names
            .choose(&mut self.rng)
            .cloned()
            .unwrap_or_else(|| "home".into());
        match intent.kind {
            IntentKind::Scan | IntentKind::Audit | IntentKind::Patch => intent.target = pick,
            IntentKind::Reroute => {
                intent.target = format!("p{}", self.proxies.max(1));
            }
            _ => {}
        }
        self.intent = intent;
    }

    fn spend(&mut self, cost: u32) -> bool {
        if self.cycles < cost {
            return false;
        }
        self.cycles -= cost;
        true
    }

    fn in_named(&self, name: &str) -> bool {
        self.here().name == name
    }

    fn child_at(&self, idx: usize) -> Option<ChildView> {
        self.children().into_iter().nth(idx)
    }

    fn file_info(&self, idx: usize) -> Option<FileSnap> {
        match self.here().kids.get(idx) {
            Some(Node::File(file)) => Some(FileSnap::from(file)),
            _ => None,
        }
    }

    fn cursor_on(&mut self, name: &str) {
        if let Some(i) = self.here().kids.iter().position(|k| k.name() == name) {
            self.cursor = i;
        }
    }

    fn enter_named(&mut self, name: &str) {
        self.cwd.clear();
        if let Some(i) = self.root.kids.iter().position(|k| k.name() == name) {
            self.cwd.push(i);
            self.cursor = 0;
            self.visited.insert(child_key(&[], i));
            map_cwd(&mut self.root, &self.cwd, |dir| {
                dir.visited = true;
                dir.revealed = true;
            });
        }
    }

    fn bank_named(&mut self, name: &str) {
        let Some((cwd, idx)) = locate_file(&self.root, name) else {
            return;
        };
        let snap = {
            let folder = walk(&self.root, &cwd);
            match folder.kids.get(idx) {
                Some(Node::File(file)) if !file.banked => {
                    Some((file.kind, file.depth, file.chunks))
                }
                _ => None,
            }
        };
        let Some((kind, depth, chunks)) = snap else {
            return;
        };
        map_file(&mut self.root, &cwd, idx, |file| {
            file.taken = file.chunks;
            file.banked = true;
        });
        self.bits += score::file_bits(kind, depth, chunks, &self.tools, self.levels);
        match kind {
            Kind::Key => {
                self.keys += 1;
                self.types |= T_KEY;
            }
            Kind::Data => self.types |= T_DATA,
            Kind::Cred => self.types |= T_CRED,
            Kind::Zero => {
                self.zeros += 1;
                self.types |= T_ZERO;
            }
            Kind::Log => {}
        }
    }

    fn set_taken(&mut self, name: &str, taken: u8) {
        if let Some((cwd, idx)) = locate_file(&self.root, name) {
            map_file(&mut self.root, &cwd, idx, |file| {
                if !file.banked && file.chunks > taken {
                    file.taken = taken;
                }
            });
        }
    }
}

struct FileSnap {
    name: String,
    kind: Kind,
    depth: u8,
    chunks: u8,
    remaining: u8,
    wipeable: bool,
    gone: bool,
}

impl From<&File> for FileSnap {
    fn from(file: &File) -> Self {
        Self {
            name: file.name.clone(),
            kind: file.kind,
            depth: file.depth,
            chunks: file.chunks,
            remaining: file.chunks.saturating_sub(file.taken),
            wipeable: file.kind == Kind::Log && !file.wiped && !file.gone,
            gone: file.gone,
        }
    }
}

fn file_row(file: &File) -> Row {
    if file.gone {
        return Row {
            name: file.name.clone(),
            label: file.kind.label().into(),
            detail: "gone".into(),
            tone: Tone::Dim,
        };
    }
    let tone = match file.kind {
        Kind::Data => Tone::Data,
        Kind::Key => Tone::Key,
        Kind::Cred => Tone::Cred,
        Kind::Log => Tone::Log,
        Kind::Zero => Tone::Zero,
    };
    let detail = match file.kind {
        Kind::Log if file.wiped => "wiped".to_string(),
        Kind::Log => "unwiped · trace +1 if it stays".to_string(),
        Kind::Key if file.banked => "banked · +1 amp".to_string(),
        Kind::Cred if file.banked => "banked · cash".to_string(),
        Kind::Zero if file.banked => "banked · x3 amp".to_string(),
        _ if file.banked => format!("{}/{} · banked", file.chunks, file.chunks),
        _ => format!("{}/{} chunks", file.taken, file.chunks),
    };
    Row {
        name: file.name.clone(),
        label: file.kind.glyph().into(),
        detail,
        tone,
    }
}

fn fresh_deck(boss: Boss) -> Vec<Intent> {
    let mut deck = Vec::with_capacity(22);
    for _ in 0..5 {
        deck.push(Intent::bare(IntentKind::Idle));
    }
    for _ in 0..3 {
        deck.push(Intent::bare(IntentKind::Surge));
    }
    for _ in 0..4 {
        deck.push(Intent::bare(IntentKind::Scan));
    }
    deck.push(Intent::bare(IntentKind::Ping));
    deck.push(Intent::bare(IntentKind::Reroute));
    deck.push(Intent::bare(IntentKind::Coffee));
    deck.push(Intent::bare(IntentKind::Lock));
    deck.push(Intent::bare(IntentKind::Throttle));
    deck.push(Intent::bare(IntentKind::Audit));
    deck.push(Intent::bare(IntentKind::Lull));
    deck.push(Intent::bare(IntentKind::Patch));
    if boss == Boss::Parent {
        deck.push(Intent::bare(IntentKind::Surge));
        deck.push(Intent::bare(IntentKind::Lock));
    }
    debug_assert!(deck.len() == DECK_SIZE || boss == Boss::Parent);
    deck
}

fn walk<'a>(root: &'a Folder, path: &[usize]) -> &'a Folder {
    let mut folder = root;
    for &i in path {
        match &folder.kids[i] {
            Node::Dir(dir) => folder = dir,
            Node::File(_) => break,
        }
    }
    folder
}

fn map_cwd<T>(root: &mut Folder, path: &[usize], f: impl FnOnce(&mut Folder) -> T) -> T {
    fn rec<T>(folder: &mut Folder, path: &[usize], f: impl FnOnce(&mut Folder) -> T) -> T {
        if path.is_empty() {
            f(folder)
        } else {
            let i = path[0];
            match &mut folder.kids[i] {
                Node::Dir(dir) => rec(dir, &path[1..], f),
                Node::File(_) => unreachable!("cwd crossed a file"),
            }
        }
    }
    rec(root, path, f)
}

fn map_file(root: &mut Folder, cwd: &[usize], idx: usize, f: impl FnOnce(&mut File)) {
    map_cwd(root, cwd, |dir| {
        if let Some(Node::File(file)) = dir.kids.get_mut(idx) {
            f(file);
        }
    });
}

fn map_child_dir(root: &mut Folder, cwd: &[usize], idx: usize, f: impl FnOnce(&mut Folder)) {
    map_cwd(root, cwd, |dir| {
        if let Some(Node::Dir(child)) = dir.kids.get_mut(idx) {
            f(child);
        }
    });
}

fn child_key(cwd: &[usize], idx: usize) -> String {
    if cwd.is_empty() {
        format!("/{idx}")
    } else {
        format!(
            "/{}",
            cwd.iter().fold(String::new(), |mut acc, n| {
                acc.push_str(&n.to_string());
                acc.push('/');
                acc
            }) + &idx.to_string()
        )
    }
}

fn parent_key(cwd: &[usize]) -> String {
    if cwd.len() <= 1 {
        "/".into()
    } else {
        let parent = &cwd[..cwd.len() - 1];
        format!(
            "/{}",
            parent
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join("/")
        )
    }
}

fn find_file<'a>(folder: &'a Folder, name: &str) -> Option<&'a File> {
    for kid in &folder.kids {
        match kid {
            Node::File(file) if file.name == name => return Some(file),
            Node::Dir(dir) => {
                if let Some(found) = find_file(dir, name) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

fn locate_file(folder: &Folder, name: &str) -> Option<(Vec<usize>, usize)> {
    fn rec(folder: &Folder, name: &str, prefix: &mut Vec<usize>) -> Option<(Vec<usize>, usize)> {
        for (i, kid) in folder.kids.iter().enumerate() {
            match kid {
                Node::File(file) if file.name == name => return Some((prefix.clone(), i)),
                Node::Dir(dir) => {
                    prefix.push(i);
                    if let Some(found) = rec(dir, name, prefix) {
                        return Some(found);
                    }
                    prefix.pop();
                }
                _ => {}
            }
        }
        None
    }
    rec(folder, name, &mut Vec::new())
}

fn dir_flag(folder: &Folder, name: &str, pred: impl Fn(&Folder) -> bool + Copy) -> bool {
    if folder.name == name && pred(folder) {
        return true;
    }
    folder.kids.iter().any(|kid| match kid {
        Node::Dir(dir) => dir_flag(dir, name, pred),
        Node::File(_) => false,
    })
}

fn reveal_one(folder: &mut Folder) -> bool {
    for kid in &mut folder.kids {
        if let Node::Dir(dir) = kid {
            if !dir.revealed {
                dir.revealed = true;
                return true;
            }
            if reveal_one(dir) {
                return true;
            }
        }
    }
    false
}

fn audit_named(folder: &mut Folder, name: &str, rng: &mut impl Rng) -> bool {
    if folder.name == name && folder.depth > 0 {
        let choices: Vec<usize> = folder
            .kids
            .iter()
            .enumerate()
            .filter_map(|(i, kid)| match kid {
                Node::File(file) if !file.banked && !file.wiped && !file.gone => Some(i),
                _ => None,
            })
            .collect();
        if let Some(i) = choices.choose(rng).copied() {
            if let Node::File(file) = &mut folder.kids[i] {
                file.gone = true;
                return true;
            }
        }
        return false;
    }
    for kid in &mut folder.kids {
        if let Node::Dir(dir) = kid {
            if audit_named(dir, name, rng) {
                return true;
            }
        }
    }
    false
}

fn patch_one(folder: &mut Folder, current: &str) -> bool {
    for kid in &mut folder.kids {
        if let Node::Dir(dir) = kid {
            if dir.was_locked && dir.locked == 0 && dir.name != current {
                dir.locked = 1;
                return true;
            }
            if patch_one(dir, current) {
                return true;
            }
        }
    }
    false
}

fn log_paths(folder: &Folder, prefix: &[usize]) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    for (i, kid) in folder.kids.iter().enumerate() {
        match kid {
            Node::File(file) if file.kind == Kind::Log && !file.wiped && !file.gone => {
                let mut path = prefix.to_vec();
                path.push(i);
                out.push(path);
            }
            Node::Dir(dir) => {
                let mut path = prefix.to_vec();
                path.push(i);
                out.extend(log_paths(dir, &path));
            }
            _ => {}
        }
    }
    out
}

fn wipe_at(root: &mut Folder, path: &[usize]) -> bool {
    if path.is_empty() {
        return false;
    }
    let (cwd, idx) = path.split_at(path.len() - 1);
    let idx = idx[0];
    let mut ok = false;
    map_file(root, cwd, idx, |file| {
        if file.kind == Kind::Log && !file.wiped && !file.gone {
            file.wiped = true;
            ok = true;
        }
    });
    ok
}

fn first_open_verb(levels: &Levels) -> LevelKind {
    for kind in LevelKind::all() {
        if levels.get(kind) < 3 {
            return kind;
        }
    }
    LevelKind::Siphon
}
