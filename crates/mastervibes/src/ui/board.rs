//! The breach: probe history, the probe being typed, score preview, tools.

use std::collections::HashSet;

use mastervibes_core::{BreachState, RunState, Symbol, Turn};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Modifier;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, LAYERS};
use crate::art::art_lines;
use crate::format::{bar, fit, format_pegs, format_points, format_symbols, glyph, preview};

use super::theme::{self, screen};
use super::{footer, frame_shell};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let inner = frame_shell(frame, area, "mastervibes");
    let Some(state) = app.state() else {
        frame.render_widget(Paragraph::new("no run").style(theme::amber()), inner);
        return;
    };
    // Art on the five slots only when a tool is owned and the terminal has
    // room. An empty rig stays one line so the probe list keeps the space.
    let show_art = area.height >= 36 && area.width >= 110 && !state.tools.is_empty();
    let tools_h: u16 = if show_art { 8 } else { 3 };
    let [header, body, symbols, tools, foot] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(6),
        Constraint::Length(1),
        Constraint::Length(tools_h),
        Constraint::Length(1),
    ])
    .areas(inner);

    draw_header(frame, header, &state, &app.status);
    let [history, score] =
        Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).areas(body);
    draw_history(frame, history, app, &state);
    draw_score(frame, score, app, &state);
    draw_symbols(frame, symbols, &state);
    draw_tools(frame, tools, &state);
    let place = place_key(state.symbols);
    frame.render_widget(
        Paragraph::new(footer(&[
            ("q", "quit"),
            ("?", "help"),
            (place.as_str(), "place"),
            ("bksp", "undo"),
            ("enter", "probe"),
        ]))
        .style(screen()),
        foot,
    );
}

fn draw_header(frame: &mut Frame, area: Rect, state: &RunState, status: &str) {
    let seed = mastervibes_core::RunSeed::new(state.key, state.layer)
        .map(|seed| seed.to_string())
        .unwrap_or_default();
    let left = state
        .breach
        .as_ref()
        .map(|breach| breach.probes_left)
        .unwrap_or(0);
    let line1 = Line::from(vec![
        Span::styled(format!("L{}/{}  ", state.layer, LAYERS), theme::green()),
        Span::styled(state.role.to_string(), theme::role(state.role)),
        Span::styled(
            format!(" {}/{}  ", state.breach_index, state.breach_count),
            theme::green(),
        ),
        Span::styled(format!("probes left {left}  "), theme::green_bold()),
        Span::styled(format!("${}  ", state.money), theme::amber_bold()),
        Span::styled(seed, theme::dim()),
    ]);
    let line2 = if status.is_empty() {
        Line::from(Span::styled(
            format!(
                "{} slots   {} symbols   par {}   layer {} / {}",
                state.slots,
                state.symbols,
                state.par,
                format_points(live_layer(state)),
                format_points(state.target_milli),
            ),
            theme::dim(),
        ))
    } else {
        Line::from(Span::styled(status.to_string(), theme::amber()))
    };
    frame.render_widget(Paragraph::new(vec![line1, line2]).style(screen()), area);
}

fn draw_history(frame: &mut Frame, area: Rect, app: &App, state: &RunState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::green())
        .title(" probes ")
        .style(screen());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.height == 0 {
        return;
    }
    let width = inner.width as usize;
    let header = Line::from(Span::styled(
        fit("#  probe        pegs     signature   score", width),
        theme::dim(),
    ));
    let mut rows = Vec::new();
    if let Some(breach) = state.breach.as_ref() {
        for (index, turn) in breach.history.iter().enumerate() {
            rows.push(history_line(index, turn, width));
        }
    }
    rows.push(draft_line(app, state, width));
    let room = inner.height.saturating_sub(1) as usize;
    let start = rows.len().saturating_sub(room);
    let mut lines = vec![header];
    lines.extend(rows.into_iter().skip(start));
    frame.render_widget(Paragraph::new(lines).style(screen()), inner);
}

fn history_line(index: usize, turn: &Turn, width: usize) -> Line<'static> {
    let probe = format_symbols(turn.probe.as_slice());
    let pegs = format_pegs(turn.pegs.hits, turn.pegs.nears);
    let score = format_points(turn.score.total_milli);
    let fixed = format!("{:>2} {:<11} {:<7} ", index + 1, probe, pegs);
    let score_w = UnicodeWidthStr::width(score.as_str());
    let room = width.saturating_sub(UnicodeWidthStr::width(fixed.as_str()) + score_w + 1);
    let sig = fit(&turn.score.signature_name, room);
    let text = format!("{fixed}{sig} {score}");
    let style = if turn.score.total_milli == 0 {
        theme::dim()
    } else {
        theme::green()
    };
    Line::from(Span::styled(fit(&text, width), style))
}

fn draft_line(app: &App, state: &RunState, width: usize) -> Line<'static> {
    let dead = dead_set(state.breach.as_ref());
    let mut used = UnicodeWidthStr::width("> ");
    let mut spans = vec![Span::styled("> ", theme::amber_bold())];
    if app.draft.is_empty() {
        spans.push(Span::styled("_", theme::dim()));
        used += 1;
    }
    for (index, slot) in app.draft.iter().enumerate() {
        let text = match slot {
            Some(symbol) => format!("{} ", glyph(*symbol)),
            None => "_ ".to_string(),
        };
        used += UnicodeWidthStr::width(text.as_str());
        let mut style = if app.locked.get(index).copied().unwrap_or(false) {
            theme::cyan()
        } else if slot.is_some_and(|symbol| dead.contains(&symbol)) {
            theme::dim()
        } else {
            theme::green_bold()
        };
        if index == app.cursor {
            style = style.add_modifier(Modifier::REVERSED);
        }
        spans.push(Span::styled(text, style));
    }
    if let Some(breach) = state.breach.as_ref() {
        let look = preview(&app.draft, &breach.dead);
        if look.complete {
            let tail = if look.wiped {
                format!("  {} scores 0", look.signature.unwrap_or_default())
            } else {
                format!(
                    "  {}  {} before pegs",
                    look.signature.unwrap_or_default(),
                    format_points(look.before_pegs)
                )
            };
            spans.push(Span::styled(
                fit(&tail, width.saturating_sub(used)),
                if look.wiped {
                    theme::amber()
                } else {
                    theme::green()
                },
            ));
        }
    }
    Line::from(spans)
}

fn draw_score(frame: &mut Frame, area: Rect, app: &App, state: &RunState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::amber())
        .title(" score ")
        .style(screen());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }
    let width = inner.width as usize;
    let mut lines = Vec::new();
    let breach = state.breach.as_ref();
    if let Some(warning) = app.absent_warning() {
        lines.push(Line::from(Span::styled(warning, theme::amber())));
    }
    let left = breach.map(|row| row.probes_left).unwrap_or(0);
    lines.push(Line::from(Span::styled(
        format!("probes left {left}"),
        theme::green_bold(),
    )));
    if let Some(breach) = breach {
        let look = preview(&app.draft, &breach.dead);
        if look.complete && look.wiped {
            lines.push(Line::from(Span::styled(
                look.signature.unwrap_or_default(),
                theme::dim(),
            )));
            lines.push(Line::from(Span::styled("scores 0", theme::amber_bold())));
        } else if look.complete {
            lines.push(Line::from(Span::styled(
                look.signature.unwrap_or_default(),
                theme::amber_bold(),
            )));
            lines.push(Line::from(Span::styled(
                format!("{} bits   amp {}", look.bits, look.amp),
                theme::green(),
            )));
            lines.push(Line::from(Span::styled(
                format!("{} before pegs", format_points(look.before_pegs)),
                theme::amber(),
            )));
        } else {
            let filled = app.draft.iter().filter(|slot| slot.is_some()).count();
            lines.push(Line::from(Span::styled(
                format!("signature —  {filled}/{} slots", state.slots),
                theme::dim(),
            )));
        }
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!(
                "layer {} / {}",
                format_points(live_layer(state)),
                format_points(state.target_milli)
            ),
            theme::green(),
        )));
        let bar_width = width.saturating_sub(2).max(4);
        lines.push(Line::from(Span::styled(
            bar(live_layer(state), state.target_milli, bar_width),
            theme::green(),
        )));
        lines.push(Line::from(Span::styled(
            format!("this breach {}", format_points(breach.probe_score_milli)),
            theme::dim(),
        )));
        lines.push(Line::from(Span::styled(
            fit("crack x(1+0.3*left) cap x5", width),
            theme::dim(),
        )));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled("REVEALED", theme::amber())));
        lines.push(Line::from(revealed_line(breach)));
        lines.push(Line::from(Span::styled(
            format!("absent  {}", absent_list(breach)),
            theme::dim(),
        )));
        if let Some(symbol) = breach.honeypot {
            lines.push(Line::from(Span::styled(
                format!("honeypot {}", glyph(symbol.index())),
                theme::amber(),
            )));
        }
        if let Some(repeats) = breach.repeats {
            let word = if repeats { "yes" } else { "no" };
            lines.push(Line::from(Span::styled(
                format!("repeats a symbol: {word}"),
                theme::green(),
            )));
        }
    }
    frame.render_widget(
        Paragraph::new(lines)
            .style(screen())
            .wrap(Wrap { trim: true }),
        inner,
    );
}

fn draw_symbols(frame: &mut Frame, area: Rect, state: &RunState) {
    let dead = dead_set(state.breach.as_ref());
    let mut spans = vec![
        Span::styled("* hit", theme::green_bold()),
        Span::styled("  + near", theme::amber()),
        Span::styled("   pool ", theme::dim()),
    ];
    if let Ok(charset) = mastervibes_core::Charset::new(state.symbols) {
        for (index, ch) in charset.glyphs().chars().enumerate() {
            let style = if dead.contains(&(index as u8)) {
                theme::dim()
            } else {
                theme::green_bold()
            };
            spans.push(Span::styled(format!("{ch} "), style));
        }
    }
    spans.push(Span::styled("  absent ", theme::dim()));
    spans.push(Span::styled(absent_list_state(state), theme::dim()));
    frame.render_widget(Paragraph::new(Line::from(spans)).style(screen()), area);
}

fn draw_tools(frame: &mut Frame, area: Rect, state: &RunState) {
    let [label, slots] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            format!("TOOLS {}/{}", state.tools.len(), state.rig_slots),
            theme::green(),
        )))
        .style(screen()),
        label,
    );
    if slots.height == 0 || state.rig_slots == 0 {
        return;
    }
    let spec = vec![Constraint::Ratio(1, state.rig_slots as u32); state.rig_slots];
    let cols = Layout::horizontal(spec).split(slots);
    for (index, col) in cols.iter().enumerate() {
        draw_slot(frame, *col, index, state.tools.get(index));
    }
}

fn draw_slot(
    frame: &mut Frame,
    area: Rect,
    index: usize,
    tool: Option<&mastervibes_core::ToolCard>,
) {
    let Some(tool) = tool else {
        frame.render_widget(
            Paragraph::new(vec![Line::from(Span::styled(
                format!("{} empty", index + 1),
                theme::dim(),
            ))])
            .style(screen()),
            area,
        );
        return;
    };
    let show_art = area.height >= 6 && area.width >= 22;
    let mut lines = vec![Line::from(Span::styled(
        format!("{} {}", index + 1, fit(&tool.name, area.width as usize)),
        theme::amber(),
    ))];
    if show_art {
        if let Some(art) = art_lines(&tool.name) {
            for row in art {
                lines.push(Line::from(Span::styled(row, theme::green())));
            }
        }
    }
    lines.push(Line::from(Span::styled(
        format!(
            "{} {}",
            tool.archetype,
            theme::rarity_word(tool.rarity)
                .chars()
                .next()
                .unwrap_or('?')
        ),
        theme::rarity(tool.rarity),
    )));
    frame.render_widget(Paragraph::new(lines).style(screen()), area);
}

fn revealed_line(breach: &BreachState) -> String {
    breach
        .revealed
        .iter()
        .map(|slot| match slot {
            Some(symbol) => glyph(symbol.index()).to_string(),
            None => ".".to_string(),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn absent_list(breach: &BreachState) -> String {
    let mut symbols: Vec<Symbol> = breach.dead.clone();
    if symbols.is_empty() {
        return "none".to_string();
    }
    symbols.sort();
    symbols
        .into_iter()
        .map(|symbol| glyph(symbol.index()).to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

fn absent_list_state(state: &RunState) -> String {
    state
        .breach
        .as_ref()
        .map(absent_list)
        .unwrap_or_else(|| "none".to_string())
}

fn dead_set(breach: Option<&BreachState>) -> HashSet<u8> {
    breach
        .map(|breach| breach.dead.iter().map(|symbol| symbol.index()).collect())
        .unwrap_or_default()
}

fn live_layer(state: &RunState) -> i64 {
    let extra = state
        .breach
        .as_ref()
        .filter(|breach| breach.settled_milli.is_none())
        .map(|breach| breach.probe_score_milli)
        .unwrap_or(0);
    state.layer_score_milli.saturating_add(extra)
}

fn place_key(symbols: usize) -> String {
    if symbols == 0 {
        return "0".to_string();
    }
    format!("0-{}", glyph((symbols - 1) as u8))
}
