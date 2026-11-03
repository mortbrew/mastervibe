//! Headless policies used to tune the L1 slice.

use crate::breach::{Act, Boss, Breach, ChildView};
use crate::catalog::{tool, IntentKind, LevelKind, ToolId};
use crate::run::{Campaign, Phase};
use crate::shop::Offer;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Policy {
    Sensible,
    Greedy,
}

#[derive(Clone, Debug)]
pub struct Report {
    pub cleared: bool,
    pub traced: u32,
    pub scores: Vec<u32>,
    pub phase: Phase,
    pub cash: u32,
    pub tools: usize,
}

pub fn simulate(seed: u64, policy: Policy) -> Report {
    let mut campaign = Campaign::new(seed);
    campaign.skip_tutorial();
    let mut guard = 0;
    while guard < 700 {
        guard += 1;
        match campaign.phase {
            Phase::Shop => match shop_act(&campaign, policy) {
                ShopAct::Buy(i) => {
                    if campaign.buy(i).is_err() {
                        campaign.leave_shop();
                    }
                }
                ShopAct::Leave => campaign.leave_shop(),
            },
            Phase::Breach | Phase::Tutorial => {
                let Some(breach) = campaign.breach.as_ref() else {
                    break;
                };
                let act = match policy {
                    Policy::Sensible => sensible(breach),
                    Policy::Greedy => greedy(breach),
                };
                let outcome = campaign.act(act);
                if !outcome.ok {
                    let _ = campaign.act(Act::EndTurn);
                }
            }
            Phase::LevelUp => {
                if let Some(kind) = campaign.level_choices.first().copied() {
                    let _ = campaign.pick_level(kind);
                } else {
                    campaign.phase = Phase::Cleared;
                }
            }
            Phase::Cleared | Phase::Over => break,
        }
    }
    Report {
        cleared: campaign.phase == Phase::Cleared,
        traced: campaign.traced,
        scores: campaign.scores.clone(),
        phase: campaign.phase,
        cash: campaign.cash,
        tools: campaign.tools.len(),
    }
}

enum ShopAct {
    Buy(usize),
    Leave,
}

fn shop_act(campaign: &Campaign, policy: Policy) -> ShopAct {
    let Some(shop) = campaign.shop.as_ref() else {
        return ShopAct::Leave;
    };
    match policy {
        Policy::Sensible => {
            let next_is_boss = campaign.breach_index == 1;
            if next_is_boss && campaign.levels.log == 1 {
                if let Some(i) = find_level(shop, LevelKind::Log, campaign) {
                    return ShopAct::Buy(i);
                }
            }
            for id in [
                ToolId::Keyring,
                ToolId::FatPipe,
                ToolId::DeadDrop,
                ToolId::TorPlus,
            ] {
                if !campaign.tools.contains(&id) {
                    if let Some(i) = find_tool(shop, id, campaign) {
                        return ShopAct::Buy(i);
                    }
                }
            }
            ShopAct::Leave
        }
        Policy::Greedy => {
            let mut best: Option<(usize, u32)> = None;
            for (i, offer) in shop.offers.iter().enumerate() {
                if let Some(Offer::Tool(id)) = offer {
                    let price = tool(*id).price;
                    if price <= campaign.cash
                        && campaign.tools.len() < 5
                        && !campaign.tools.contains(id)
                        && best.map(|(_, p)| price > p).unwrap_or(true)
                    {
                        best = Some((i, price));
                    }
                }
            }
            if let Some((i, _)) = best {
                ShopAct::Buy(i)
            } else {
                ShopAct::Leave
            }
        }
    }
}

fn find_tool(shop: &crate::shop::Shop, id: ToolId, campaign: &Campaign) -> Option<usize> {
    let price = tool(id).price;
    if campaign.cash < price || campaign.tools.len() >= 5 {
        return None;
    }
    shop.offers
        .iter()
        .position(|offer| matches!(offer, Some(Offer::Tool(t)) if *t == id))
}

fn find_level(shop: &crate::shop::Shop, kind: LevelKind, campaign: &Campaign) -> Option<usize> {
    let price = campaign.levels.cost(kind)?;
    if campaign.cash < price {
        return None;
    }
    shop.offers
        .iter()
        .position(|offer| matches!(offer, Some(Offer::Level(k)) if *k == kind))
}

pub fn sensible(breach: &Breach) -> Act {
    let kids = breach.children();
    if kids.is_empty() {
        return if breach.at_root() {
            Act::EndTurn
        } else if breach.cycles > 0 {
            Act::Out
        } else {
            Act::EndTurn
        };
    }
    let proj = breach.projected_hops();
    let score = breach.jack_preview().score;
    let target = breach.target.unwrap_or(0);
    let passing = target > 0 && score >= target;

    if passing && proj <= 1 {
        return Act::Jack;
    }
    let pressure = breach.boss == Boss::Parent && breach.turn >= 4 && breach.unwiped_logs() > 0;
    if proj <= 0 || (pressure && proj <= 2) {
        if let Some(act) = defend(breach, &kids) {
            return act;
        }
        if passing || proj <= 0 {
            return Act::Jack;
        }
    }
    if breach.boss == Boss::Parent && breach.turn < 4 && breach.unwiped_logs() > 0 {
        if let Some(act) = clear_logs(breach, &kids) {
            return act;
        }
    }
    if passing && (proj <= 2 || breach.turn >= 6) {
        return Act::Jack;
    }
    if let Some(act) = loot(breach, &kids, proj) {
        return act;
    }
    if !breach.at_root() {
        return if breach.cycles > 0 {
            Act::Out
        } else {
            Act::EndTurn
        };
    }
    if passing {
        Act::Jack
    } else {
        Act::EndTurn
    }
}

pub fn greedy(breach: &Breach) -> Act {
    let kids = breach.children();
    if breach.cycles == 0 {
        return Act::EndTurn;
    }
    let mut best: Option<(u8, usize)> = None;
    for (i, child) in kids.iter().enumerate() {
        let rank = greedy_rank(child);
        if let Some(rank) = rank {
            if best.map(|(r, _)| rank < r).unwrap_or(true) {
                best = Some((rank, i));
            }
        }
    }
    let Some((_, idx)) = best else {
        return if breach.at_root() {
            Act::EndTurn
        } else {
            Act::Out
        };
    };
    if breach.cursor != idx {
        return toward(breach.cursor, idx);
    }
    act_on(breach, &kids[idx])
}

fn greedy_rank(child: &ChildView) -> Option<u8> {
    if child.gone {
        return None;
    }
    if let Some(kind) = child.kind {
        if child.remaining > 0 && kind != crate::catalog::Kind::Log {
            return Some(0);
        }
        None
    } else if child.dir {
        Some(if child.locked > 0 { 2 } else { 1 })
    } else {
        None
    }
}

fn defend(breach: &Breach, kids: &[ChildView]) -> Option<Act> {
    if let Some(i) = kids.iter().position(|c| c.wipeable) {
        if breach.cursor != i {
            return Some(toward(breach.cursor, i));
        }
        return Some(if breach.cycles >= 1 {
            Act::Wipe
        } else {
            Act::EndTurn
        });
    }
    if breach.projected_hops() <= 0 && breach.can_burn() {
        return Some(Act::Burn);
    }
    if breach.projected_hops() <= 0 && breach.cycles >= breach.bounce_cost() && breach.proxies < 6 {
        return Some(Act::Bounce);
    }
    if breach.at_root() {
        if let Some(i) = kids
            .iter()
            .position(|c| c.dir && !c.visited && c.depth <= 1)
        {
            if breach.cursor != i {
                return Some(toward(breach.cursor, i));
            }
            return Some(act_on(breach, &kids[i]));
        }
    } else if breach.cycles > 0 {
        return Some(Act::Out);
    }
    None
}

fn clear_logs(breach: &Breach, kids: &[ChildView]) -> Option<Act> {
    if let Some(i) = kids.iter().position(|c| c.wipeable) {
        if breach.cursor != i {
            return Some(toward(breach.cursor, i));
        }
        return Some(if breach.cycles >= 1 {
            Act::Wipe
        } else {
            Act::EndTurn
        });
    }
    if let Some(i) = kids
        .iter()
        .position(|c| c.dir && !c.visited && c.depth <= 1 && c.locked == 0)
    {
        if breach.cursor != i {
            return Some(toward(breach.cursor, i));
        }
        return Some(act_on(breach, &kids[i]));
    }
    if !breach.at_root() && breach.cycles > 0 {
        return Some(Act::Out);
    }
    None
}

fn loot(breach: &Breach, kids: &[ChildView], proj: i32) -> Option<Act> {
    let mut best: Option<(u8, usize)> = None;
    for (i, child) in kids.iter().enumerate() {
        if let Some(rank) = loot_rank(child, proj) {
            if best.map(|(r, _)| rank < r).unwrap_or(true) {
                best = Some((rank, i));
            }
        }
    }
    let (_, idx) = best?;
    if breach.cursor != idx {
        return Some(toward(breach.cursor, idx));
    }
    Some(act_on(breach, &kids[idx]))
}

fn loot_rank(child: &ChildView, proj: i32) -> Option<u8> {
    if child.gone {
        return None;
    }
    if let Some(kind) = child.kind {
        if child.remaining == 0 {
            return None;
        }
        return match kind {
            crate::catalog::Kind::Key => Some(0),
            crate::catalog::Kind::Data => Some(1),
            crate::catalog::Kind::Cred => Some(2),
            crate::catalog::Kind::Zero if proj >= 2 => Some(3),
            crate::catalog::Kind::Log => None,
            _ => None,
        };
    }
    if child.dir && !child.visited {
        if child.depth >= 3 && proj < 3 {
            return None;
        }
        return Some(4);
    }
    if child.dir && child.locked > 0 && child.depth <= 2 {
        return Some(4);
    }
    None
}

fn act_on(breach: &Breach, child: &ChildView) -> Act {
    if child.dir {
        if child.locked > 0 {
            return if breach.cycles >= 1 {
                Act::Decrypt
            } else {
                Act::EndTurn
            };
        }
        return if breach.cycles >= 1 {
            Act::In
        } else {
            Act::EndTurn
        };
    }
    if child.wipeable {
        return if breach.cycles >= 1 {
            Act::Wipe
        } else {
            Act::EndTurn
        };
    }
    let cost = if breach.intent.kind == IntentKind::Throttle {
        2
    } else {
        1
    };
    if child.remaining > 0 && breach.cycles >= cost {
        Act::Siphon
    } else {
        Act::EndTurn
    }
}

fn toward(cursor: usize, idx: usize) -> Act {
    if cursor < idx {
        Act::Down
    } else {
        Act::Up
    }
}
