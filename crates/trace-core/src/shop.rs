//! The black market between breaches: tools, one level-up, one script.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

use crate::catalog::{
    script, tool, Arch, LevelKind, Levels, Rarity, ScriptId, ToolId, Verbs, TOOLS,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Offer {
    Tool(ToolId),
    Level(LevelKind),
    Script(ScriptId),
}

impl Offer {
    pub fn price(&self, levels: Levels) -> u32 {
        match self {
            Self::Tool(id) => tool(*id).price,
            Self::Level(kind) => levels.cost(*kind).unwrap_or(0),
            Self::Script(id) => script(*id).price,
        }
    }

    pub fn name(&self) -> String {
        match self {
            Self::Tool(id) => tool(*id).name.to_string(),
            Self::Level(kind) => format!("{} lv", kind.name()),
            Self::Script(id) => script(*id).name.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Shop {
    pub offers: Vec<Option<Offer>>,
    pub reroll_cost: u32,
    pub lean: String,
}

pub fn roll(
    seed: u64,
    shop_index: u32,
    rerolls: u32,
    owned: &[ToolId],
    levels: Levels,
    verbs: Verbs,
) -> Shop {
    let mut rng = StdRng::seed_from_u64(
        seed ^ 0x5A0B
            ^ u64::from(shop_index).wrapping_mul(0x100)
            ^ u64::from(rerolls).wrapping_mul(0x1_0000),
    );
    let mut pool: Vec<ToolId> = TOOLS
        .iter()
        .map(|def| def.id)
        .filter(|id| !owned.contains(id))
        .collect();
    let mut picked = Vec::new();
    for _ in 0..4 {
        if pool.is_empty() {
            break;
        }
        let rarity = roll_rarity(&mut rng);
        let choice = weighted_pick(&mut rng, &pool, owned, Some(rarity))
            .or_else(|| weighted_pick(&mut rng, &pool, owned, None));
        if let Some(id) = choice {
            pool.retain(|other| *other != id);
            picked.push(id);
        }
    }
    force_off_build(&mut picked, &mut pool, owned, &mut rng);
    let mut offers: Vec<Option<Offer>> =
        picked.into_iter().map(|id| Some(Offer::Tool(id))).collect();
    if let Some(kind) = level_offer(&mut rng, levels, verbs) {
        offers.push(Some(Offer::Level(kind)));
    }
    let scripts: Vec<ScriptId> = crate::catalog::SCRIPTS.iter().map(|s| s.id).collect();
    if let Some(id) = scripts.choose(&mut rng).copied() {
        offers.push(Some(Offer::Script(id)));
    }
    Shop {
        offers,
        reroll_cost: 1 + rerolls,
        lean: lean_label(owned),
    }
}

fn roll_rarity(rng: &mut impl Rng) -> Rarity {
    match rng.gen_range(0..100) {
        0..=4 => Rarity::Rare,
        5..=29 => Rarity::Uncommon,
        _ => Rarity::Common,
    }
}

fn weighted_pick(
    rng: &mut impl Rng,
    pool: &[ToolId],
    owned: &[ToolId],
    rarity: Option<Rarity>,
) -> Option<ToolId> {
    let candidates: Vec<ToolId> = pool
        .iter()
        .copied()
        .filter(|id| rarity.is_none_or(|r| tool(*id).rarity == r))
        .collect();
    if candidates.is_empty() {
        return None;
    }
    let weights: Vec<u32> = candidates.iter().map(|id| weight(*id, owned)).collect();
    let total: u32 = weights.iter().sum();
    if total == 0 {
        return candidates.choose(rng).copied();
    }
    let mut roll = rng.gen_range(0..total);
    for (id, w) in candidates.iter().zip(weights) {
        if roll < w {
            return Some(*id);
        }
        roll -= w;
    }
    candidates.last().copied()
}

fn weight(id: ToolId, owned: &[ToolId]) -> u32 {
    let def = tool(id);
    let mut weight = 10;
    let owned_arch = owned.iter().filter(|t| tool(**t).arch == def.arch).count();
    if owned_arch >= 2 {
        weight += 30;
    }
    weight
}

fn force_off_build(
    picked: &mut [ToolId],
    pool: &mut Vec<ToolId>,
    owned: &[ToolId],
    rng: &mut impl Rng,
) {
    let Some(lean) = dominant_arch(owned) else {
        return;
    };
    if picked.is_empty() || picked.iter().any(|id| tool(*id).arch != lean) {
        return;
    }
    let off: Vec<ToolId> = pool
        .iter()
        .copied()
        .filter(|id| tool(*id).arch != lean)
        .collect();
    if let Some(id) = off.choose(rng).copied() {
        if let Some(last) = picked.last_mut() {
            pool.retain(|other| *other != id);
            pool.push(*last);
            *last = id;
        }
    }
}

fn dominant_arch(owned: &[ToolId]) -> Option<Arch> {
    let mut best: Option<(Arch, usize)> = None;
    for id in owned {
        let arch = tool(*id).arch;
        let n = owned.iter().filter(|t| tool(**t).arch == arch).count();
        if n >= 2 && best.map(|(_, c)| n > c).unwrap_or(true) {
            best = Some((arch, n));
        }
    }
    best.map(|(arch, _)| arch)
}

pub fn lean_label(owned: &[ToolId]) -> String {
    let mut parts = Vec::new();
    for arch in [
        Arch::Stealth,
        Arch::Greed,
        Arch::Proxy,
        Arch::Data,
        Arch::Cycle,
        Arch::Chaos,
        Arch::XAmp,
    ] {
        let n = owned.iter().filter(|id| tool(**id).arch == arch).count();
        if n >= 2 {
            parts.push(arch.label());
        }
    }
    if parts.is_empty() {
        "your build is still open".to_string()
    } else {
        format!("your build leans {}", parts.join(" + "))
    }
}

fn level_offer(rng: &mut impl Rng, levels: Levels, verbs: Verbs) -> Option<LevelKind> {
    let preferred = verbs.most_used();
    if levels.cost(preferred).is_some() && rng.gen_bool(0.65) {
        return Some(preferred);
    }
    let open: Vec<LevelKind> = LevelKind::all()
        .into_iter()
        .filter(|kind| levels.cost(*kind).is_some())
        .collect();
    open.choose(rng).copied()
}
