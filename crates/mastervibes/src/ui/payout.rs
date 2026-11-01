//! Breach total and the dollars it paid.

use mastervibes_core::{Amp, BreachRole, BreachStatus, Phase, RunState};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::format::{bar, format_points};

use super::theme::{self, screen};
use super::{footer, frame_shell};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let inner = frame_shell(frame, area, "mastervibes");
    let Some(state) = app.state() else {
        return;
    };
    let [head, cols, progress, foot] = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(6),
        Constraint::Length(4),
        Constraint::Length(1),
    ])
    .areas(inner);

    draw_head(frame, head, &state);
    let [score, money] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(cols);
    draw_score(frame, score, &state);
    draw_money(frame, money, &state);
    draw_progress(frame, progress, &state);
    frame.render_widget(
        Paragraph::new(footer(&[
            ("q", "quit"),
            ("?", "help"),
            ("enter", "continue"),
        ]))
        .style(screen()),
        foot,
    );
}

fn draw_head(frame: &mut Frame, area: Rect, state: &RunState) {
    let cracked = state
        .breach
        .as_ref()
        .map(|breach| breach.status == BreachStatus::Cracked)
        .unwrap_or(false);
    let title = if cracked { "CRACKED" } else { "NOT CRACKED" };
    let style = if cracked {
        theme::green_bold()
    } else {
        theme::amber_bold()
    };
    let used = state
        .breach
        .as_ref()
        .map(|breach| breach.probes_used)
        .unwrap_or(0);
    let budget = state.probe_budget;
    let lines = vec![
        Line::from(Span::styled(title, style)),
        Line::from(Span::styled(
            format!(
                "{}  ·  layer {}  ·  probe {used} of {budget}  (par {})",
                state.role, state.layer, state.par
            ),
            theme::green(),
        )),
    ];
    frame.render_widget(Paragraph::new(lines).style(screen()), area);
}

fn draw_score(frame: &mut Frame, area: Rect, state: &RunState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::green())
        .title(" score ")
        .style(screen());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let breach = state.breach.as_ref();
    let probes = breach.map(|row| row.probe_score_milli).unwrap_or(0);
    let settled = breach.and_then(|row| row.settled_milli).unwrap_or(0);
    let factor = breach.and_then(|row| row.multiplier_milli).unwrap_or(1000);
    let cracked = breach
        .map(|row| row.status == BreachStatus::Cracked)
        .unwrap_or(false);
    let factor_amp = Amp::from_milli(factor);
    let factor_label = if cracked {
        format!("crack ×{factor_amp}")
    } else {
        format!("kept ×{factor_amp}")
    };
    let lines = vec![
        money_gap("probes", &format_points(probes)),
        Line::from(Span::styled(factor_label, theme::amber())),
        Line::from(""),
        Line::from(vec![
            Span::styled("BREACH  ", theme::green_bold()),
            Span::styled(format_points(settled), theme::amber_bold()),
        ]),
    ];
    frame.render_widget(Paragraph::new(lines).style(screen()), inner);
}

fn draw_money(frame: &mut Frame, area: Rect, state: &RunState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::amber())
        .title(" $ ")
        .style(screen());
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(payout) = state.last_payout else {
        frame.render_widget(Paragraph::new("no payout").style(screen()), inner);
        return;
    };
    let mut lines = vec![dollar_line("breach", payout.base)];
    if payout.under_par > 0 {
        lines.push(dollar_line("under par", payout.under_par));
    }
    if payout.tools > 0 {
        lines.push(dollar_line("tools", payout.tools));
    }
    if payout.overkill > 0 {
        lines.push(dollar_line("extra target", payout.overkill));
    }
    lines.push(dollar_line("interest", payout.interest));
    let before = state.money.saturating_sub(payout.total);
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        format!("${before} + ${} → ${}", payout.total, state.money),
        theme::amber_bold(),
    )));
    frame.render_widget(Paragraph::new(lines).style(screen()), inner);
}

fn draw_progress(frame: &mut Frame, area: Rect, state: &RunState) {
    let width = area.width as usize;
    let score = state.layer_score_milli;
    let lines = vec![
        Line::from(Span::styled(
            format!(
                "LAYER {}   {} / {}",
                state.layer,
                format_points(score),
                format_points(state.target_milli)
            ),
            theme::green(),
        )),
        Line::from(Span::styled(
            bar(score, state.target_milli, width.saturating_sub(2)),
            theme::green(),
        )),
        Line::from(Span::styled(next_label(state), theme::amber())),
    ];
    frame.render_widget(Paragraph::new(lines).style(screen()), area);
}

fn next_label(state: &RunState) -> String {
    match state.phase {
        Phase::Won => "run clear".to_string(),
        Phase::Lost => "run over".to_string(),
        Phase::Shop => {
            if state.role == BreachRole::Root {
                format!("next: layer {} Perimeter", state.layer + 1)
            } else {
                let next = match state.role {
                    BreachRole::Perimeter => "Internal",
                    BreachRole::Internal => "Root",
                    BreachRole::Root => "Perimeter",
                };
                format!("next: {next}")
            }
        }
        Phase::Breach => "breach still open".to_string(),
    }
}

fn dollar_line(label: &str, amount: u32) -> Line<'static> {
    Line::from(Span::styled(
        format!("{label:<14}${amount}"),
        theme::green(),
    ))
}

fn money_gap(label: &str, amount: &str) -> Line<'static> {
    Line::from(Span::styled(format!("{label:<14}{amount}"), theme::green()))
}
