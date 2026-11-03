//! bits × amp. Halves live as milli-units so a near miss stays exact.

use crate::catalog::{has_tool, Kind, Levels, ToolId};

#[derive(Clone, Debug)]
pub struct Score {
    pub bits: u32,
    pub amp_milli: i64,
    pub score: u32,
    pub near_miss: bool,
    pub sets: u32,
}

pub struct ScoreIn<'a> {
    pub tools: &'a [ToolId],
    pub levels: Levels,
    pub bits: u32,
    pub keys: u32,
    pub zeros: u32,
    pub types: u8,
    pub proxies: u32,
    pub near_miss: bool,
    pub empty_cycles: bool,
    pub turn: u32,
}

pub fn file_bits(kind: Kind, depth: u8, chunks: u8, tools: &[ToolId], levels: Levels) -> u32 {
    match kind {
        Kind::Data => {
            let depth = u32::from(depth.max(1));
            let mut per = 15 * depth;
            if has_tool(tools, ToolId::FatPipe) {
                per += 5;
            }
            if levels.data >= 3 {
                per += 10;
            } else if levels.data >= 2 {
                per += 5;
            }
            let mut total = per * u32::from(chunks);
            if has_tool(tools, ToolId::Compressor) && chunks >= 3 {
                total = total * 3 / 2;
            }
            total
        }
        Kind::Key => 5,
        Kind::Cred => {
            if has_tool(tools, ToolId::CredStuffer) {
                20
            } else {
                10
            }
        }
        Kind::Zero => 40,
        Kind::Log => 0,
    }
}

pub fn cred_cash(tools: &[ToolId]) -> u32 {
    if has_tool(tools, ToolId::CredStuffer) {
        3
    } else {
        2
    }
}

fn type_count(mask: u8) -> u32 {
    mask.count_ones()
}

pub fn sets_of(keys: u32) -> u32 {
    keys / 2
}

pub fn compute(input: ScoreIn<'_>) -> Score {
    let sets = sets_of(input.keys);
    let mut add: i64 = 1000;
    let key_each: i64 = if input.levels.key >= 3 { 1500 } else { 1000 };
    add += key_each * i64::from(input.keys);
    let set_bonus: i64 = if input.keys >= 3 {
        if input.levels.key >= 2 {
            3000
        } else {
            2000
        }
    } else if input.keys >= 2 {
        if input.levels.key >= 2 {
            2000
        } else {
            1000
        }
    } else {
        0
    };
    add += set_bonus;
    let over = input.proxies.saturating_sub(3);
    let per_proxy: i64 = if input.levels.bounce >= 2 { 1000 } else { 500 };
    add += per_proxy * i64::from(over);
    if has_tool(input.tools, ToolId::Keyring) {
        add += 1000 * i64::from(sets);
    }
    if has_tool(input.tools, ToolId::GrabBag) && input.empty_cycles {
        add += 1000;
    }
    if has_tool(input.tools, ToolId::HashTable) && type_count(input.types) >= 3 {
        add += 2000;
    }

    let mut mult: i64 = 1000;
    if input.zeros > 0 {
        let z: i64 = if input.levels.zero >= 3 { 4000 } else { 3000 };
        for _ in 0..input.zeros {
            mult = mult * z / 1000;
        }
    }
    if has_tool(input.tools, ToolId::RootShell) {
        mult = mult * 3 / 2;
    }
    if has_tool(input.tools, ToolId::ColdBoot) && input.turn <= 3 {
        mult *= 2;
    }
    if has_tool(input.tools, ToolId::Keychain) {
        let km: i64 = if sets >= 2 {
            2000
        } else if sets == 1 {
            1500
        } else {
            1000
        };
        mult = mult * km / 1000;
    }

    let near = input.near_miss;
    if near && has_tool(input.tools, ToolId::LastByte) {
        mult *= 2;
    } else if near {
        add += 1000;
    }

    let amp = add * mult / 1000;
    let score = (i64::from(input.bits) * amp / 1000).max(0) as u32;
    Score {
        bits: input.bits,
        amp_milli: amp,
        score,
        near_miss: near,
        sets,
    }
}

pub fn keep_ratio(tools: &[ToolId]) -> u32 {
    if has_tool(tools, ToolId::DeadDrop) {
        75
    } else {
        50
    }
}

pub fn apply_trace(score: u32, tools: &[ToolId]) -> u32 {
    score * keep_ratio(tools) / 100
}
