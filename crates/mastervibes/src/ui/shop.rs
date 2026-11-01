//! Two tool cards, the five slots, reroll, sell, and the next breach.

use mastervibes_core::{RunState, ToolCard};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::art::art_lines;

use super::theme::{self, screen};
use super::{footer, frame_shell};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let inner = frame_shell(frame, area, "mastervibes");
    let Some(state) = app.state() else {
        return;
    };
    let owned_h: u16 = if area.height >= 40 { 6 } else { 3 };
    let [head, cards, owned, foot] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(9),
        Constraint::Length(owned_h),
        Constraint::Length(2),
    ])
    .areas(inner);

    draw_head(frame, head, &state);
    draw_offers(frame, cards, &state);
    draw_owned(frame, owned, &state, app.sell_mode);
    draw_foot(frame, foot, app, &state);
}

fn draw_head(frame: &mut Frame, area: Rect, state: &RunState) {
    let cost = state
        .shop
        .as_ref()
        .map(|shop| shop.reroll_cost)
        .unwrap_or(0);
    let next = match state.role {
        mastervibes_core::BreachRole::Perimeter => "Internal",
        mastervibes_core::BreachRole::Internal => "Root",
        mastervibes_core::BreachRole::Root => "next layer",
    };
    let lines = vec![
        Line::from(vec![
            Span::styled(
                format!("SHOP   layer {}  after {}  ", state.layer, state.role),
                theme::green(),
            ),
            Span::styled(format!("${}", state.money), theme::amber_bold()),
        ]),
        Line::from(Span::styled(
            format!("next {next}   reroll ${cost}"),
            theme::dim(),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).style(screen()), area);
}

fn draw_offers(frame: &mut Frame, area: Rect, state: &RunState) {
    let offers = state
        .shop
        .as_ref()
        .map(|shop| shop.offers.as_slice())
        .unwrap_or(&[]);
    if offers.is_empty() {
        frame.render_widget(
            Paragraph::new("nothing for sale — reroll, or continue").style(theme::dim()),
            area,
        );
        return;
    }
    let cols = if offers.len() == 2 {
        let [left, _, right] = Layout::horizontal([
            Constraint::Min(22),
            Constraint::Length(1),
            Constraint::Min(22),
        ])
        .areas(area);
        vec![left, right]
    } else {
        let spec = vec![Constraint::Ratio(1, offers.len() as u32); offers.len()];
        Layout::horizontal(spec).split(area).to_vec()
    };
    for (index, col) in cols.iter().enumerate() {
        if let Some(tool) = offers.get(index) {
            draw_card(frame, *col, index, tool, state.money);
        }
    }
}

fn draw_card(frame: &mut Frame, area: Rect, index: usize, tool: &ToolCard, money: u32) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(theme::rarity(tool.rarity))
        .title(format!(" [{}] {} ", index + 1, tool.name))
        .style(screen());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let mut lines = Vec::new();
    if inner.width >= 20 {
        if let Some(art) = art_lines(&tool.name) {
            let pad = inner.width.saturating_sub(20) / 2;
            let prefix = " ".repeat(pad as usize);
            for row in art {
                lines.push(Line::from(Span::styled(
                    format!("{prefix}{row}"),
                    theme::green(),
                )));
            }
            lines.push(Line::from(""));
        }
    }
    lines.push(Line::from(Span::styled(
        tool.summary.clone(),
        theme::white(),
    )));
    lines.push(Line::from(""));
    let price_style = if money >= tool.price {
        theme::amber_bold()
    } else {
        theme::dim()
    };
    lines.push(Line::from(vec![
        Span::styled(
            format!("{} · {}  ", tool.archetype, theme::rarity_word(tool.rarity)),
            theme::dim(),
        ),
        Span::styled(format!("${}", tool.price), price_style),
    ]));
    frame.render_widget(
        Paragraph::new(lines)
            .style(screen())
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn draw_owned(frame: &mut Frame, area: Rect, state: &RunState, sell_mode: bool) {
    let title = if sell_mode {
        "TOOLS  —  press 1-5 to sell"
    } else {
        "TOOLS"
    };
    let style = if sell_mode {
        theme::amber()
    } else {
        theme::green()
    };
    let mut lines = vec![Line::from(Span::styled(
        format!("{title}  {}/{}", state.tools.len(), state.rig_slots),
        style,
    ))];
    let sell = state
        .shop
        .as_ref()
        .map(|shop| shop.sell_values.clone())
        .unwrap_or_default();
    let mut slots = Vec::new();
    for index in 0..state.rig_slots {
        if let Some(tool) = state.tools.get(index) {
            let price = sell.get(index).copied().unwrap_or(tool.price / 2);
            slots.push(format!("{} {} sell ${price}", index + 1, tool.name));
        } else {
            slots.push(format!("{} empty", index + 1));
        }
    }
    lines.push(Line::from(Span::styled(slots.join("   "), theme::green())));
    frame.render_widget(
        Paragraph::new(lines)
            .style(screen())
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn draw_foot(frame: &mut Frame, area: Rect, app: &App, state: &RunState) {
    let cost = state
        .shop
        .as_ref()
        .map(|shop| shop.reroll_cost)
        .unwrap_or(0);
    let reroll = format!("reroll ${cost}");
    let [keys, status] =
        Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).areas(area);
    let line = if app.sell_mode {
        footer(&[
            ("q", "quit"),
            ("?", "help"),
            ("1-5", "sell"),
            ("esc", "cancel"),
            ("n", "next breach"),
        ])
    } else {
        footer(&[
            ("q", "quit"),
            ("?", "help"),
            ("1-2", "buy"),
            ("r", reroll.as_str()),
            ("s", "sell"),
            ("n", "next breach"),
        ])
    };
    frame.render_widget(Paragraph::new(line).style(screen()), keys);
    let note = if app.status.is_empty() {
        Line::from(Span::styled("sell pays half, rounded down", theme::dim()))
    } else {
        Line::from(Span::styled(app.status.clone(), theme::amber()))
    };
    frame.render_widget(Paragraph::new(note).style(screen()), status);
}
