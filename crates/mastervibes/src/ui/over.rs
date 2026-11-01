//! Win or loss. The seed and the run's stats.

use mastervibes_core::{Phase, RunState};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::format::{fit, format_points};

use super::theme::{self, screen};
use super::{footer, frame_shell};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let inner = frame_shell(frame, area, "mastervibes");
    let Some(state) = app.state() else {
        return;
    };
    let [head, body, foot] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(1),
    ])
    .areas(inner);
    draw_head(frame, head, &state);
    draw_body(frame, body, app, &state);
    frame.render_widget(
        Paragraph::new(footer(&[
            ("q", "quit"),
            ("?", "help"),
            ("n", "new run"),
            ("r", "retry seed"),
        ]))
        .style(screen()),
        foot,
    );
}

fn draw_head(frame: &mut Frame, area: Rect, state: &RunState) {
    let won = state.phase == Phase::Won || state.end.as_ref().map(|end| end.won).unwrap_or(false);
    let title = if won { "RUN CLEAR" } else { "RUN OVER" };
    let style = if won {
        theme::green_bold()
    } else {
        theme::red()
    };
    let seed = mastervibes_core::RunSeed::new(state.key, 1)
        .map(|seed| seed.to_string())
        .unwrap_or_default();
    let lines = vec![
        Line::from(Span::styled(title, style)),
        Line::from(vec![
            Span::styled("seed ", theme::dim()),
            Span::styled(seed, theme::amber_bold()),
        ]),
        Line::from(Span::styled(
            "layer 1 of this seed replays the run",
            theme::dim(),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).style(screen()), area);
}

fn draw_body(frame: &mut Frame, area: Rect, app: &App, state: &RunState) {
    let [stats, tools] =
        Layout::horizontal([Constraint::Percentage(60), Constraint::Percentage(40)]).areas(area);
    draw_stats(frame, stats, app, state);
    draw_tools(frame, tools, state);
}

fn draw_stats(frame: &mut Frame, area: Rect, app: &App, state: &RunState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::green())
        .title(" run ")
        .style(screen());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let end = state.end.as_ref();
    let score = end
        .map(|row| row.layer_score_milli)
        .unwrap_or(state.layer_score_milli);
    let target = end
        .map(|row| row.target_milli)
        .unwrap_or(state.target_milli);
    let gap = target.saturating_sub(score);
    let verdict = if gap > 0 {
        format!("missed by {}", format_points(gap))
    } else {
        format!(
            "over target by {}",
            format_points(score.saturating_sub(target))
        )
    };
    let best_probe = if app.stats.best_probe_milli < 0 {
        "-".to_string()
    } else if app.stats.best_probe_sig.is_empty() {
        format_points(app.stats.best_probe_milli)
    } else {
        format!(
            "{} ({})",
            format_points(app.stats.best_probe_milli),
            app.stats.best_probe_sig
        )
    };
    let best_breach = if app.stats.best_breach_milli < 0 {
        "-".to_string()
    } else {
        format_points(app.stats.best_breach_milli)
    };
    let most = app
        .most_played()
        .map(|(name, count)| format!("{name} ({count})"))
        .unwrap_or_else(|| "—".to_string());
    let lines = vec![
        row("reached", &format!("layer {} {}", state.layer, state.role)),
        row(
            "layer score",
            &format!("{} / {}", format_points(score), format_points(target)),
        ),
        Line::from(Span::styled(verdict, theme::amber())),
        Line::from(""),
        row("best probe", &best_probe),
        row("best breach", &best_breach),
        row("probes", &app.stats.probes.to_string()),
        row("payouts", &format!("${}", app.stats.payouts)),
        row("held", &format!("${}", state.money)),
        row("tools bought", &app.stats.tools_bought.to_string()),
        row("most played", &most),
        row("time", &app.elapsed_text()),
    ];
    frame.render_widget(Paragraph::new(lines).style(screen()), inner);
}

fn draw_tools(frame: &mut Frame, area: Rect, state: &RunState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::amber())
        .title(" tools ")
        .style(screen());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let width = inner.width as usize;
    let mut lines = Vec::new();
    if state.tools.is_empty() {
        lines.push(Line::from(Span::styled("no tools", theme::dim())));
    }
    for tool in &state.tools {
        lines.push(Line::from(Span::styled(
            fit(&tool.name, width),
            theme::amber(),
        )));
        lines.push(Line::from(Span::styled(
            fit(
                &format!("{} · {}", tool.archetype, theme::rarity_word(tool.rarity)),
                width,
            ),
            theme::dim(),
        )));
    }
    frame.render_widget(Paragraph::new(lines).style(screen()), inner);
}

fn row(label: &str, value: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<14}"), theme::dim()),
        Span::styled(value.to_string(), theme::green()),
    ])
}
