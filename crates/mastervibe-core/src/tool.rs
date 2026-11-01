//! Tools: TOML definitions and a small set of effect kinds.
//!
//! Kinds are `score`, `crack`, `info`, `budget`, and `economy`. The run loop
//! decides when each one fires. Score effects become [`crate::ScoreHook`]s
//! and run in rig order. ×amp effects multiply together up to the run table's
//! cap; the hook driver keeps that product.

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use serde::Deserialize;
use thiserror::Error;

use crate::amp::{parse_amp, Amp};
use crate::code::{Code, MAX_SYMBOLS};
use crate::pattern;
use crate::pegs::Pegs;
use crate::scoring::{RunningScore, ScoreContext, ScoreHook};

/// Where a tool sits in the catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Archetype {
    /// Information about the secret.
    Recon,
    /// Rewards cracking quickly.
    Speed,
    /// Rewards spending the probe budget.
    Grind,
    /// Rewards exacts and nears.
    Pegs,
    /// Rewards symbol shapes and values.
    Symbols,
    /// Changes money, interest, or the shop.
    Economy,
    /// A rule that does not sit in one of the other boxes.
    Wild,
}

impl Archetype {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "recon" => Self::Recon,
            "speed" => Self::Speed,
            "grind" => Self::Grind,
            "pegs" => Self::Pegs,
            "symbols" => Self::Symbols,
            "economy" => Self::Economy,
            "wild" => Self::Wild,
            _ => return None,
        })
    }
}

impl std::fmt::Display for Archetype {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Recon => "Recon",
            Self::Speed => "Speed",
            Self::Grind => "Grind",
            Self::Pegs => "Pegs",
            Self::Symbols => "Symbols",
            Self::Economy => "Economy",
            Self::Wild => "Wild",
        })
    }
}

/// Catalog rarity. `C` / `U` / `R` / `L`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Rarity {
    /// Common.
    Common,
    /// Uncommon.
    Uncommon,
    /// Rare.
    Rare,
    /// Legendary.
    Legendary,
}

impl Rarity {
    fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "C" => Self::Common,
            "U" => Self::Uncommon,
            "R" => Self::Rare,
            "L" => Self::Legendary,
            _ => return None,
        })
    }

    pub(crate) fn weight_index(self) -> usize {
        match self {
            Self::Common => 0,
            Self::Uncommon => 1,
            Self::Rare => 2,
            Self::Legendary => 3,
        }
    }
}

impl std::fmt::Display for Rarity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Common => "C",
            Self::Uncommon => "U",
            Self::Rare => "R",
            Self::Legendary => "L",
        })
    }
}

/// One tool, loaded from TOML.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tool {
    /// Stable id.
    pub id: String,
    /// Catalog name.
    pub name: String,
    /// Archetype.
    pub archetype: Archetype,
    /// Rarity letter.
    pub rarity: Rarity,
    /// Shop price in dollars.
    pub price: u32,
    /// One-line effect, for a UI.
    pub summary: String,
    pub(crate) effects: Vec<Effect>,
}

impl Tool {
    pub(crate) fn extra_probes(&self) -> u8 {
        self.effects
            .iter()
            .map(|effect| match effect {
                Effect::ExtraProbes(count) => *count,
                _ => 0,
            })
            .fold(0u8, u8::saturating_add)
    }

    pub(crate) fn crack_mul(&self) -> Option<Amp> {
        self.effects.iter().find_map(|effect| match effect {
            Effect::CrackMul(amp) => Some(*amp),
            _ => None,
        })
    }

    pub(crate) fn has_score(&self) -> bool {
        self.effects
            .iter()
            .any(|effect| matches!(effect, Effect::Score(_)))
    }

    pub(crate) fn infos(&self) -> impl Iterator<Item = &InfoEffect> {
        self.effects.iter().filter_map(|effect| match effect {
            Effect::Info(info) => Some(info),
            _ => None,
        })
    }

    pub(crate) fn economies(&self) -> impl Iterator<Item = &EconomyEffect> {
        self.effects.iter().filter_map(|effect| match effect {
            Effect::Economy(economy) => Some(economy),
            _ => None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    Score(ScoreEffect),
    CrackMul(Amp),
    Info(InfoEffect),
    ExtraProbes(u8),
    Economy(EconomyEffect),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ScoreEffect {
    op: ScoreOp,
    when: When,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScoreOp {
    AddBits(i32),
    AddAmp(Amp),
    MulAmp(Amp),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum When {
    PerNear,
    MinExacts(u8),
    OnProbe(u8),
    CrackingPerUnderPar,
    FirstBlank,
    AfterPar,
    PerEdgeExact,
    PerAdjacentExact,
    Ascending,
    PerRepeat,
    PerValue { min: u8, max: u8 },
    Honeypot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InfoEffect {
    RevealRandom(u8),
    /// 1-based slot.
    RevealSlot(u8),
    NameAbsent {
        after_probe: u8,
        count: u8,
    },
    ShowRepeat,
    Honeypot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EconomyEffect {
    PayUnderPar { dollars: u32, cap: u32 },
    InterestCap(u32),
    RerollDiscount(u32),
}

/// The loaded tool list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolSet {
    tools: Vec<Tool>,
    by_id: HashMap<String, usize>,
}

impl ToolSet {
    /// Parse a tool list. Ids are unique and every tool has an effect.
    pub fn from_toml(text: &str) -> Result<Self, ToolError> {
        let raw: RawFile = toml::from_str(text).map_err(|err| ToolError::Toml(err.to_string()))?;
        if raw.tool.is_empty() {
            return Err(ToolError::Invalid("no tools".to_string()));
        }
        let mut seen = HashSet::new();
        let mut tools = Vec::with_capacity(raw.tool.len());
        for row in raw.tool {
            if row.id.is_empty() || !seen.insert(row.id.clone()) {
                return Err(ToolError::Invalid(format!(
                    "bad or duplicate id {}",
                    row.id
                )));
            }
            if row.name.trim().is_empty() || row.summary.trim().is_empty() {
                return Err(ToolError::Invalid(format!(
                    "{} needs a name and summary",
                    row.id
                )));
            }
            if row.price == 0 {
                return Err(ToolError::Invalid(format!("{} has no price", row.id)));
            }
            let archetype = Archetype::parse(&row.archetype)
                .ok_or_else(|| ToolError::Invalid(format!("{} has unknown archetype", row.id)))?;
            let rarity = Rarity::parse(&row.rarity)
                .ok_or_else(|| ToolError::Invalid(format!("{} has unknown rarity", row.id)))?;
            if row.effect.is_empty() {
                return Err(ToolError::Invalid(format!("{} has no effect", row.id)));
            }
            let mut effects = Vec::with_capacity(row.effect.len());
            for effect in row.effect {
                effects.push(parse_effect(&row.id, effect)?);
            }
            tools.push(Tool {
                id: row.id,
                name: row.name,
                archetype,
                rarity,
                price: row.price,
                summary: row.summary,
                effects,
            });
        }
        let by_id = tools
            .iter()
            .enumerate()
            .map(|(index, tool)| (tool.id.clone(), index))
            .collect();
        Ok(Self { tools, by_id })
    }

    /// Tools in catalog order.
    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    /// Look up a tool by id.
    pub fn get(&self, id: &str) -> Option<&Tool> {
        self.by_id.get(id).map(|&index| &self.tools[index])
    }
}

/// The tool list compiled into the crate.
pub fn embedded_tools() -> &'static ToolSet {
    static TOOLS: OnceLock<ToolSet> = OnceLock::new();
    TOOLS.get_or_init(|| {
        ToolSet::from_toml(include_str!("../data/tools.toml"))
            .expect("embedded tools.toml is valid")
    })
}

/// One owned tool, as a score hook. Built fresh for each probe.
pub(crate) struct RigHook<'a> {
    tool: &'a Tool,
    probe_number: u32,
    honeypot: Option<u8>,
    honeypot_armed: &'a Cell<bool>,
    blank_armed: &'a Cell<bool>,
    mul_product: &'a Cell<i32>,
    mul_cap_milli: i32,
}

impl<'a> RigHook<'a> {
    pub(crate) fn new(
        tool: &'a Tool,
        probe_number: u32,
        honeypot: Option<u8>,
        honeypot_armed: &'a Cell<bool>,
        blank_armed: &'a Cell<bool>,
        mul_product: &'a Cell<i32>,
        mul_cap_milli: i32,
    ) -> Self {
        Self {
            tool,
            probe_number,
            honeypot,
            honeypot_armed,
            blank_armed,
            mul_product,
            mul_cap_milli,
        }
    }
}

impl ScoreHook for RigHook<'_> {
    fn id(&self) -> &str {
        &self.tool.id
    }

    fn apply(&self, ctx: &ScoreContext<'_>, running: &mut RunningScore) {
        for effect in &self.tool.effects {
            let Effect::Score(score) = effect else {
                continue;
            };
            let times = self.times(ctx, &score.when);
            if times <= 0 {
                continue;
            }
            match score.op {
                ScoreOp::AddBits(bits) => {
                    running.bits = running.bits.saturating_add(bits.saturating_mul(times));
                }
                ScoreOp::AddAmp(amp) => {
                    let milli = amp.milli().saturating_mul(times);
                    running.amp = running.amp.saturating_add(Amp::from_milli(milli));
                }
                ScoreOp::MulAmp(amp) => self.apply_mul(running, amp),
            }
        }
    }
}

impl RigHook<'_> {
    fn times(&self, ctx: &ScoreContext<'_>, when: &When) -> i32 {
        let slots = ctx
            .layer
            .map(|layer| layer.slots())
            .unwrap_or(ctx.probe.len());
        let par = u32::try_from(slots).unwrap_or(u32::MAX);
        let cracking = ctx.pegs.all_hits(slots);
        match when {
            When::PerNear => i32::from(ctx.pegs.nears),
            When::MinExacts(min) => i32::from(ctx.pegs.hits >= *min),
            When::OnProbe(probe) => i32::from(self.probe_number == u32::from(*probe)),
            When::CrackingPerUnderPar => {
                if cracking {
                    i32::try_from(par.saturating_sub(self.probe_number)).unwrap_or(0)
                } else {
                    0
                }
            }
            When::FirstBlank => {
                if ctx.pegs.hits == 0 && ctx.pegs.nears == 0 && self.blank_armed.get() {
                    self.blank_armed.set(false);
                    1
                } else {
                    0
                }
            }
            When::AfterPar => i32::from(self.probe_number > par),
            When::PerEdgeExact => edge_exacts(ctx.pegs, ctx.probe.len()),
            When::PerAdjacentExact => i32::from(ctx.pegs.adjacent_exact_pairs()),
            When::Ascending => i32::from(pattern::matches("ascending", ctx.probe)),
            When::PerRepeat => extra_copies(ctx.probe),
            When::PerValue { min, max } => count_values(ctx.probe, *min, *max),
            When::Honeypot => self.honeypot_times(ctx.probe),
        }
    }

    fn honeypot_times(&self, probe: &Code) -> i32 {
        let Some(symbol) = self.honeypot else {
            return 0;
        };
        if self.honeypot_armed.get() && probe.as_slice().contains(&symbol) {
            self.honeypot_armed.set(false);
            1
        } else {
            0
        }
    }

    fn apply_mul(&self, running: &mut RunningScore, factor: Amp) {
        let current = self.mul_product.get();
        let cap = self.mul_cap_milli;
        if current <= 0 || current >= cap || factor.milli() <= 0 {
            return;
        }
        let uncapped = i64::from(current).saturating_mul(i64::from(factor.milli())) / 1000;
        if uncapped > i64::from(cap) {
            let scaled = Amp::from_milli(
                i32::try_from(
                    i64::from(running.amp.milli()).saturating_mul(i64::from(cap))
                        / i64::from(current),
                )
                .unwrap_or(i32::MAX),
            );
            running.amp = scaled;
            self.mul_product.set(cap);
        } else {
            running.amp = running.amp.saturating_mul(factor);
            let product = i32::try_from(uncapped).unwrap_or(cap).min(cap);
            self.mul_product.set(product);
        }
    }
}

fn edge_exacts(pegs: Pegs, len: usize) -> i32 {
    if len == 0 {
        return 0;
    }
    let mut count = 0;
    if pegs.exact_at(0) {
        count += 1;
    }
    if len > 1 && pegs.exact_at(len - 1) {
        count += 1;
    }
    count
}

fn extra_copies(probe: &Code) -> i32 {
    let mut counts = [0u8; MAX_SYMBOLS];
    for &symbol in probe.as_slice() {
        counts[usize::from(symbol)] = counts[usize::from(symbol)].saturating_add(1);
    }
    counts
        .into_iter()
        .map(|count| i32::from(count.saturating_sub(1)))
        .sum()
}

fn count_values(probe: &Code, min: u8, max: u8) -> i32 {
    probe
        .iter()
        .filter(|symbol| {
            let value = symbol.index().saturating_add(1);
            value >= min && value <= max
        })
        .count() as i32
}

fn tool_err(id: &str, reason: impl std::fmt::Display) -> ToolError {
    ToolError::Invalid(format!("{id}: {reason}"))
}

fn parse_effect(id: &str, raw: RawEffect) -> Result<Effect, ToolError> {
    let bad = |reason: String| tool_err(id, reason);
    match raw.kind.as_str() {
        "score" => {
            let when = parse_when(id, &raw)?;
            let op = match (raw.add_bits, raw.add_amp, raw.mul_amp) {
                (Some(bits), None, None) => ScoreOp::AddBits(bits),
                (None, Some(amp), None) => {
                    ScoreOp::AddAmp(parse_amp(amp).map_err(|reason| tool_err(id, reason))?)
                }
                (None, None, Some(amp)) => {
                    let amp = parse_amp(amp).map_err(|reason| tool_err(id, reason))?;
                    if amp.milli() <= 0 {
                        return Err(bad("mul_amp must be > 0".to_string()));
                    }
                    ScoreOp::MulAmp(amp)
                }
                _ => {
                    return Err(bad(
                        "a score effect needs exactly one of add_bits, add_amp, mul_amp"
                            .to_string(),
                    ))
                }
            };
            Ok(Effect::Score(ScoreEffect { op, when }))
        }
        "crack" => {
            if raw.when.as_deref() != Some("at_or_under_par") {
                return Err(bad("crack effects support at_or_under_par".to_string()));
            }
            let mul = raw
                .mul
                .ok_or_else(|| bad("crack effect needs mul".to_string()))?;
            let amp = parse_amp(mul).map_err(|reason| tool_err(id, reason))?;
            if amp.milli() <= 0 {
                return Err(bad("crack mul must be > 0".to_string()));
            }
            Ok(Effect::CrackMul(amp))
        }
        "info" => Ok(Effect::Info(parse_info(id, &raw)?)),
        "budget" => {
            let extra = raw
                .extra_probes
                .ok_or_else(|| bad("budget effect needs extra_probes".to_string()))?;
            if extra == 0 {
                return Err(bad("extra_probes must be > 0".to_string()));
            }
            Ok(Effect::ExtraProbes(extra))
        }
        "economy" => Ok(Effect::Economy(parse_economy(id, &raw)?)),
        other => Err(bad(format!("unknown effect kind {other}"))),
    }
}

fn parse_when(id: &str, raw: &RawEffect) -> Result<When, ToolError> {
    let bad = |reason: String| ToolError::Invalid(format!("{id}: {reason}"));
    let when = raw
        .when
        .as_deref()
        .ok_or_else(|| bad("score effect needs when".to_string()))?;
    Ok(match when {
        "per_near" => When::PerNear,
        "min_exacts" => When::MinExacts(
            raw.exacts
                .ok_or_else(|| bad("min_exacts needs exacts".to_string()))?,
        ),
        "probe" => When::OnProbe(
            raw.probe
                .ok_or_else(|| bad("probe timing needs probe".to_string()))?,
        ),
        "cracking_per_under_par" => When::CrackingPerUnderPar,
        "first_blank" => When::FirstBlank,
        "after_par" => When::AfterPar,
        "per_edge_exact" => When::PerEdgeExact,
        "per_adjacent_exact" => When::PerAdjacentExact,
        "ascending" => When::Ascending,
        "per_repeat" => When::PerRepeat,
        "per_value" => When::PerValue {
            min: raw
                .min_value
                .ok_or_else(|| bad("per_value needs min_value".to_string()))?,
            max: raw
                .max_value
                .ok_or_else(|| bad("per_value needs max_value".to_string()))?,
        },
        "honeypot" => When::Honeypot,
        other => return Err(bad(format!("unknown score timing {other}"))),
    })
}

fn parse_info(id: &str, raw: &RawEffect) -> Result<InfoEffect, ToolError> {
    let bad = |reason: String| ToolError::Invalid(format!("{id}: {reason}"));
    let action = raw
        .action
        .as_deref()
        .ok_or_else(|| bad("info effect needs action".to_string()))?;
    Ok(match action {
        "reveal_random" => InfoEffect::RevealRandom(
            raw.count
                .filter(|count| *count > 0)
                .ok_or_else(|| bad("reveal_random needs count".to_string()))?,
        ),
        "reveal_slot" => {
            let slot = raw
                .slot
                .filter(|slot| *slot > 0)
                .ok_or_else(|| bad("reveal_slot is 1-based".to_string()))?;
            InfoEffect::RevealSlot(slot)
        }
        "name_absent" => InfoEffect::NameAbsent {
            after_probe: raw
                .after_probe
                .filter(|probe| *probe > 0)
                .ok_or_else(|| bad("name_absent needs after_probe".to_string()))?,
            count: raw
                .count
                .filter(|count| *count > 0)
                .ok_or_else(|| bad("name_absent needs count".to_string()))?,
        },
        "show_repeat" => InfoEffect::ShowRepeat,
        "honeypot" => InfoEffect::Honeypot,
        other => return Err(bad(format!("unknown info action {other}"))),
    })
}

fn parse_economy(id: &str, raw: &RawEffect) -> Result<EconomyEffect, ToolError> {
    let bad = |reason: String| ToolError::Invalid(format!("{id}: {reason}"));
    let action = raw
        .action
        .as_deref()
        .ok_or_else(|| bad("economy effect needs action".to_string()))?;
    Ok(match action {
        "pay_under_par" => EconomyEffect::PayUnderPar {
            dollars: raw
                .dollars
                .ok_or_else(|| bad("pay_under_par needs dollars".to_string()))?,
            cap: raw
                .cap
                .ok_or_else(|| bad("pay_under_par needs cap".to_string()))?,
        },
        "interest_cap" => EconomyEffect::InterestCap(
            raw.extra
                .ok_or_else(|| bad("interest_cap needs extra".to_string()))?,
        ),
        "reroll_discount" => EconomyEffect::RerollDiscount(
            raw.dollars
                .ok_or_else(|| bad("reroll_discount needs dollars".to_string()))?,
        ),
        other => return Err(bad(format!("unknown economy action {other}"))),
    })
}

/// The tool table could not be loaded.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ToolError {
    /// `toml` rejected the document.
    #[error("could not parse tools: {0}")]
    Toml(String),
    /// A tool or effect the run loop cannot use.
    #[error("tool table is invalid: {0}")]
    Invalid(String),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFile {
    tool: Vec<RawTool>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTool {
    id: String,
    name: String,
    archetype: String,
    rarity: String,
    price: u32,
    summary: String,
    effect: Vec<RawEffect>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawEffect {
    kind: String,
    when: Option<String>,
    action: Option<String>,
    add_bits: Option<i32>,
    add_amp: Option<f64>,
    mul_amp: Option<f64>,
    mul: Option<f64>,
    exacts: Option<u8>,
    probe: Option<u8>,
    count: Option<u8>,
    slot: Option<u8>,
    after_probe: Option<u8>,
    min_value: Option<u8>,
    max_value: Option<u8>,
    extra_probes: Option<u8>,
    dollars: Option<u32>,
    cap: Option<u32>,
    extra: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::{embedded_tools, Archetype, ToolSet};

    #[test]
    fn embedded_tools_cover_every_archetype() {
        let tools = embedded_tools();
        assert!(tools.tools().len() >= 20);
        assert!(tools.tools().len() <= 24);
        for archetype in [
            Archetype::Recon,
            Archetype::Speed,
            Archetype::Grind,
            Archetype::Pegs,
            Archetype::Symbols,
            Archetype::Economy,
            Archetype::Wild,
        ] {
            assert!(
                tools.tools().iter().any(|tool| tool.archetype == archetype),
                "missing {archetype}"
            );
        }
        let honeypot = tools.get("honeypot").unwrap();
        assert_eq!(honeypot.name, "Honeypot");
        assert_eq!(honeypot.price, 6);
        assert_eq!(tools.get("speedrun").unwrap().name, "Speedrun.sh");
        assert_eq!(tools.get("var_log").unwrap().name, "/var/log");
        assert!(ToolSet::from_toml("tool = []").is_err() || ToolSet::from_toml("").is_err());
    }
}
