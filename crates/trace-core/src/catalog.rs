//! Tools, scripts, intents, and the verb level-ups for the L1 slice.
//!
//! The slice ships 20 tools (commons, uncommons, and one rare) drawn from the
//! 100-tool catalog, plus five one-use scripts.

use serde::{Deserialize, Serialize};

pub const DECK_SIZE: usize = 20;
pub const TOOL_SLOTS: usize = 5;
pub const BIN_SLOTS: usize = 2;
pub const START_CASH: u32 = 4;
pub const JACK_PAY: u32 = 3;
pub const B2_TARGET: u32 = 120;
pub const B3_TARGET: u32 = 156;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
}

impl Rarity {
    pub fn label(self) -> &'static str {
        match self {
            Self::Common => "COMMON",
            Self::Uncommon => "UNCOMMON",
            Self::Rare => "RARE",
        }
    }

    pub fn mark(self) -> &'static str {
        match self {
            Self::Common => "· COM",
            Self::Uncommon => "◇ UNC",
            Self::Rare => "◆ RAR",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Arch {
    Stealth,
    Greed,
    Proxy,
    Data,
    Cycle,
    Chaos,
    XAmp,
}

impl Arch {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stealth => "stealth",
            Self::Greed => "greed",
            Self::Proxy => "proxy",
            Self::Data => "data",
            Self::Cycle => "cycle",
            Self::Chaos => "chaos",
            Self::XAmp => "xamp",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ToolId {
    FatPipe,
    Compressor,
    DeadDrop,
    Keyring,
    CredStuffer,
    TorPlus,
    GhostProcess,
    SpoofedMac,
    CronJob,
    PipeGrep,
    Glitch,
    Spool,
    GrabBag,
    LastByte,
    Burner,
    RootShell,
    ColdBoot,
    HashTable,
    Overclock,
    Keychain,
}

pub struct ToolDef {
    pub id: ToolId,
    pub name: &'static str,
    pub rarity: Rarity,
    pub price: u32,
    pub arch: Arch,
    pub effect: &'static str,
    pub synergy: &'static str,
}

pub const TOOLS: &[ToolDef] = &[
    ToolDef {
        id: ToolId::FatPipe,
        name: "Fat Pipe",
        rarity: Rarity::Common,
        price: 4,
        arch: Arch::Greed,
        effect: "+5 bits per DATA chunk.",
        synergy: "siphon level-ups",
    },
    ToolDef {
        id: ToolId::Compressor,
        name: "Compressor",
        rarity: Rarity::Common,
        price: 5,
        arch: Arch::Greed,
        effect: "DATA files with 3 or more chunks pay +50% bits.",
        synergy: "deep files",
    },
    ToolDef {
        id: ToolId::DeadDrop,
        name: "Dead Drop",
        rarity: Rarity::Common,
        price: 4,
        arch: Arch::Stealth,
        effect: "When traced, keep 75% of the haul instead of 50%.",
        synergy: "greed tools",
    },
    ToolDef {
        id: ToolId::Keyring,
        name: "Keyring",
        rarity: Rarity::Common,
        price: 4,
        arch: Arch::Data,
        effect: "KEY files show up more often. Each KEY set gives +1 amp.",
        synergy: "KEY level-ups",
    },
    ToolDef {
        id: ToolId::CredStuffer,
        name: "Cred Stuffer",
        rarity: Rarity::Common,
        price: 4,
        arch: Arch::Data,
        effect: "CRED pays an extra $1 and +10 bits.",
        synergy: "shop economy",
    },
    ToolDef {
        id: ToolId::TorPlus,
        name: "Proxy: tor+",
        rarity: Rarity::Common,
        price: 4,
        arch: Arch::Proxy,
        effect: "bounce costs 1 cycle.",
        synergy: "proxy tools",
    },
    ToolDef {
        id: ToolId::GhostProcess,
        name: "Ghost Process",
        rarity: Rarity::Common,
        price: 4,
        arch: Arch::Stealth,
        effect: "Wiping a LOG also pulls the trace back 1 hop.",
        synergy: "LOG level-ups",
    },
    ToolDef {
        id: ToolId::SpoofedMac,
        name: "Spoofed MAC",
        rarity: Rarity::Common,
        price: 3,
        arch: Arch::Stealth,
        effect: "The first intent of each breach is idle.",
        synergy: "slow opens",
    },
    ToolDef {
        id: ToolId::CronJob,
        name: "Cron Job",
        rarity: Rarity::Common,
        price: 4,
        arch: Arch::Cycle,
        effect: "1 unspent cycle carries into the next turn.",
        synergy: "long turns",
    },
    ToolDef {
        id: ToolId::PipeGrep,
        name: "Pipe | grep",
        rarity: Rarity::Common,
        price: 4,
        arch: Arch::Cycle,
        effect: "cd into a folder you have already visited costs 0.",
        synergy: "deep trees",
    },
    ToolDef {
        id: ToolId::Glitch,
        name: "Glitch",
        rarity: Rarity::Common,
        price: 3,
        arch: Arch::Chaos,
        effect: "A siphon has a 25% chance to take 1 extra chunk.",
        synergy: "DATA builds",
    },
    ToolDef {
        id: ToolId::Spool,
        name: "Spool",
        rarity: Rarity::Uncommon,
        price: 5,
        arch: Arch::Data,
        effect: "Each LOG you wipe pays +15 bits.",
        synergy: "LOG level-ups",
    },
    ToolDef {
        id: ToolId::GrabBag,
        name: "Grab Bag",
        rarity: Rarity::Uncommon,
        price: 5,
        arch: Arch::Greed,
        effect: "Jack out with 0 cycles left for +1 amp.",
        synergy: "cycle tools",
    },
    ToolDef {
        id: ToolId::LastByte,
        name: "Last Byte",
        rarity: Rarity::Uncommon,
        price: 6,
        arch: Arch::Greed,
        effect: "A near miss multiplies amp by 2 instead of adding 1.",
        synergy: "burner",
    },
    ToolDef {
        id: ToolId::Burner,
        name: "Burner",
        rarity: Rarity::Uncommon,
        price: 6,
        arch: Arch::Stealth,
        effect: "Once per breach, a trace that would catch you resets to 2 hops.",
        synergy: "near misses",
    },
    ToolDef {
        id: ToolId::RootShell,
        name: "Root Shell",
        rarity: Rarity::Uncommon,
        price: 7,
        arch: Arch::XAmp,
        effect: "x1.5 amp.",
        synergy: "any build",
    },
    ToolDef {
        id: ToolId::ColdBoot,
        name: "Cold Boot",
        rarity: Rarity::Uncommon,
        price: 6,
        arch: Arch::XAmp,
        effect: "x2 amp if you jack out on turn 3 or earlier.",
        synergy: "fast breaches",
    },
    ToolDef {
        id: ToolId::HashTable,
        name: "Hash Table",
        rarity: Rarity::Uncommon,
        price: 6,
        arch: Arch::Data,
        effect: "Banking 3 different data types in one breach gives +2 amp.",
        synergy: "mixed haul",
    },
    ToolDef {
        id: ToolId::Overclock,
        name: "Overclock",
        rarity: Rarity::Uncommon,
        price: 6,
        arch: Arch::Cycle,
        effect: "4 cycles on the first turn of each breach.",
        synergy: "deep starts",
    },
    ToolDef {
        id: ToolId::Keychain,
        name: "Keychain.dll",
        rarity: Rarity::Rare,
        price: 9,
        arch: Arch::XAmp,
        effect: "x1.5 amp with one KEY set. x2 amp with two sets.",
        synergy: "keyring",
    },
];

pub fn tool(id: ToolId) -> &'static ToolDef {
    TOOLS.iter().find(|t| t.id == id).expect("tool id")
}

pub fn has_tool(tools: &[ToolId], id: ToolId) -> bool {
    tools.contains(&id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ScriptId {
    ZeroDay,
    Grep,
    Wget,
    Strace,
    Rm,
}

pub struct ScriptDef {
    pub id: ScriptId,
    pub name: &'static str,
    pub price: u32,
    pub effect: &'static str,
}

pub const SCRIPTS: &[ScriptDef] = &[
    ScriptDef {
        id: ScriptId::ZeroDay,
        name: "0day.sh",
        price: 4,
        effect: "The next 2 files you finish this breach pay x2 bits.",
    },
    ScriptDef {
        id: ScriptId::Grep,
        name: "grep.sh",
        price: 3,
        effect: "+1 level on your most-used verb. One use.",
    },
    ScriptDef {
        id: ScriptId::Wget,
        name: "wget.sh",
        price: 5,
        effect: "Double your $ (max +$20).",
    },
    ScriptDef {
        id: ScriptId::Strace,
        name: "strace.sh",
        price: 4,
        effect: "Folders start revealed for this breach and the next 2.",
    },
    ScriptDef {
        id: ScriptId::Rm,
        name: "rm.sh",
        price: 4,
        effect: "Wipe 2 LOGs anywhere. No cycle cost.",
    },
];

pub fn script(id: ScriptId) -> &'static ScriptDef {
    SCRIPTS.iter().find(|s| s.id == id).expect("script id")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LevelKind {
    Siphon,
    Bounce,
    Decrypt,
    Data,
    Key,
    Log,
    Zero,
}

impl LevelKind {
    pub fn all() -> [LevelKind; 7] {
        [
            Self::Siphon,
            Self::Bounce,
            Self::Decrypt,
            Self::Data,
            Self::Key,
            Self::Log,
            Self::Zero,
        ]
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Siphon => "siphon",
            Self::Bounce => "bounce",
            Self::Decrypt => "decrypt",
            Self::Data => "DATA",
            Self::Key => "KEY",
            Self::Log => "LOG",
            Self::Zero => "ZERO",
        }
    }

    pub fn next_text(self, have: u8) -> String {
        match (self, have) {
            (Self::Siphon, 1) => "takes 2 chunks per cycle".into(),
            (Self::Siphon, 2) => "+5 bits per chunk".into(),
            (Self::Bounce, 1) => "+1 amp per proxy over 3, instead of +0.5".into(),
            (Self::Bounce, 2) => "bounce costs 1 cycle".into(),
            (Self::Decrypt, 1) => "decrypt also reveals the folder".into(),
            (Self::Decrypt, 2) => "the first decrypt each breach costs 0".into(),
            (Self::Data, 1) => "+5 bits per DATA chunk".into(),
            (Self::Data, 2) => "+10 bits per DATA chunk".into(),
            (Self::Key, 1) => "a 2-KEY set gives +2 amp".into(),
            (Self::Key, 2) => "each KEY gives +1.5 amp".into(),
            (Self::Log, 1) => "wipe pulls the trace back 2 hops".into(),
            (Self::Log, 2) => "wipe also pays +10 bits".into(),
            (Self::Zero, 1) => "ZERO finishes in 3 chunks".into(),
            (Self::Zero, 2) => "ZERO multiplies amp by 4".into(),
            _ => "maxed".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Levels {
    pub siphon: u8,
    pub bounce: u8,
    pub decrypt: u8,
    pub data: u8,
    pub key: u8,
    pub log: u8,
    pub zero: u8,
}

impl Default for Levels {
    fn default() -> Self {
        Self {
            siphon: 1,
            bounce: 1,
            decrypt: 1,
            data: 1,
            key: 1,
            log: 1,
            zero: 1,
        }
    }
}

impl Levels {
    pub fn get(self, kind: LevelKind) -> u8 {
        match kind {
            LevelKind::Siphon => self.siphon,
            LevelKind::Bounce => self.bounce,
            LevelKind::Decrypt => self.decrypt,
            LevelKind::Data => self.data,
            LevelKind::Key => self.key,
            LevelKind::Log => self.log,
            LevelKind::Zero => self.zero,
        }
    }

    pub fn set(&mut self, kind: LevelKind, value: u8) {
        let slot = match kind {
            LevelKind::Siphon => &mut self.siphon,
            LevelKind::Bounce => &mut self.bounce,
            LevelKind::Decrypt => &mut self.decrypt,
            LevelKind::Data => &mut self.data,
            LevelKind::Key => &mut self.key,
            LevelKind::Log => &mut self.log,
            LevelKind::Zero => &mut self.zero,
        };
        *slot = value;
    }

    pub fn cost(self, kind: LevelKind) -> Option<u32> {
        match (kind, self.get(kind)) {
            (LevelKind::Siphon, 1) => Some(5),
            (LevelKind::Siphon, 2) => Some(8),
            (LevelKind::Bounce, 1) => Some(5),
            (LevelKind::Bounce, 2) => Some(8),
            (LevelKind::Decrypt, 1) => Some(4),
            (LevelKind::Decrypt, 2) => Some(7),
            (LevelKind::Data, 1) => Some(3),
            (LevelKind::Data, 2) => Some(6),
            (LevelKind::Key, 1) => Some(4),
            (LevelKind::Key, 2) => Some(7),
            (LevelKind::Log, 1) => Some(3),
            (LevelKind::Log, 2) => Some(6),
            (LevelKind::Zero, 1) => Some(6),
            (LevelKind::Zero, 2) => Some(10),
            _ => None,
        }
    }

    pub fn bump(&mut self, kind: LevelKind) -> bool {
        let have = self.get(kind);
        if have >= 3 {
            return false;
        }
        self.set(kind, have + 1);
        true
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Verbs {
    pub siphon: u32,
    pub wipe: u32,
    pub bounce: u32,
    pub decrypt: u32,
    pub burn: u32,
    pub cd: u32,
}

impl Verbs {
    /// The verb a `grep.sh` should raise. Ties break toward siphon.
    pub fn most_used(self) -> LevelKind {
        let ranked = [
            (self.siphon, LevelKind::Siphon),
            (self.decrypt, LevelKind::Decrypt),
            (self.bounce, LevelKind::Bounce),
            (self.wipe, LevelKind::Log),
        ];
        ranked
            .into_iter()
            .max_by_key(|(n, kind)| {
                let bias = match kind {
                    LevelKind::Siphon => 3,
                    LevelKind::Decrypt => 2,
                    LevelKind::Bounce => 1,
                    _ => 0,
                };
                (*n, bias)
            })
            .map(|(_, k)| k)
            .unwrap_or(LevelKind::Siphon)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntentKind {
    Idle,
    Surge,
    Scan,
    Ping,
    Reroute,
    Coffee,
    Lock,
    Throttle,
    Audit,
    Lull,
    Patch,
}

impl IntentKind {
    pub fn base_move(self) -> u32 {
        match self {
            Self::Surge => 2,
            Self::Coffee | Self::Lull => 0,
            _ => 1,
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Idle => "IDLE",
            Self::Surge => "TRACE +2",
            Self::Scan => "SCAN",
            Self::Ping => "PING SWEEP",
            Self::Reroute => "REROUTE",
            Self::Coffee => "COFFEE",
            Self::Lock => "LOCK PROXIES",
            Self::Throttle => "THROTTLE",
            Self::Audit => "AUDIT",
            Self::Lull => "LULL",
            Self::Patch => "PATCH",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Intent {
    pub kind: IntentKind,
    pub target: String,
}

impl Intent {
    pub fn bare(kind: IntentKind) -> Self {
        Self {
            kind,
            target: String::new(),
        }
    }

    pub fn titled(&self) -> String {
        if self.target.is_empty() {
            self.kind.title().to_string()
        } else {
            match self.kind {
                IntentKind::Scan => format!("SCAN {}", self.target),
                IntentKind::Audit => format!("AUDIT {}", self.target),
                IntentKind::Patch => format!("PATCH {}", self.target),
                IntentKind::Reroute => format!("REROUTE {}", self.target),
                _ => self.kind.title().to_string(),
            }
        }
    }
}

pub fn interest_for(cash: u32) -> u32 {
    (cash / 5).min(5)
}

pub fn fmt_amp(milli: i64) -> String {
    if milli % 1000 == 0 {
        format!("{}", milli / 1000)
    } else if milli % 100 == 0 {
        format!("{:.1}", milli as f64 / 1000.0)
    } else {
        format!("{:.2}", milli as f64 / 1000.0)
    }
}

pub fn seed_label(seed: u64) -> String {
    let n = (seed & 0xFFFF_FFFF) as u32;
    format!("TRAC-{:04X}-{:04X}", n >> 16, n & 0xFFFF)
}

pub fn parse_seed(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    if let Some(hex) = trimmed
        .strip_prefix("TRAC-")
        .or_else(|| trimmed.strip_prefix("trac-"))
    {
        let hex: String = hex.chars().filter(|c| *c != '-').collect();
        return u64::from_str_radix(&hex, 16).ok();
    }
    if let Ok(n) = trimmed.parse::<u64>() {
        return Some(n);
    }
    u64::from_str_radix(trimmed, 16).ok()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Data,
    Key,
    Cred,
    Log,
    Zero,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Data => "DATA",
            Self::Key => "KEY",
            Self::Cred => "CRED",
            Self::Log => "LOG",
            Self::Zero => "ZERO",
        }
    }

    pub fn glyph(self) -> &'static str {
        match self {
            Self::Data => "DATA",
            Self::Key => "KEY ◆",
            Self::Cred => "CRED $",
            Self::Log => "LOG ▲",
            Self::Zero => "ZERO ∅",
        }
    }
}
