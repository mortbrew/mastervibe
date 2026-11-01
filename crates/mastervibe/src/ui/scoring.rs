//! Bits × amp, one row at a time. Any key skips. It moves on by itself.

use mastervibe_core::Amp;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::App;
use crate::format::{fit, format_pegs, format_points, ScoreStep};

use super::theme::{self, screen};
use super::{footer, frame_shell};

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let inner = frame_shell(frame, area, "mastervibe");
    let Some(moment) = app.moment.as_ref() else {
        frame.render_widget(Paragraph::new("scoring").style(screen()), inner);
        return;
    };
    let [head, body, foot] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(4),
        Constraint::Length(1),
    ])
    .areas(inner);

    let pegs = format_pegs(moment.hits, moment.nears);
    let header = vec![
        Line::from(Span::styled(
            format!("SCORING   {}", moment.signature),
            theme::amber_bold(),
        )),
        Line::from(vec![
            Span::styled(moment.probe.clone(), theme::green_bold()),
            Span::styled(format!("   {pegs}"), theme::green()),
        ]),
        Line::from(Span::styled("* hit    + near", theme::dim())),
    ];
    frame.render_widget(Paragraph::new(header).style(screen()), head);

    let shown = moment.visible();
    let width = body.width as usize;
    let mut lines = Vec::new();
    for step in moment.steps.iter().take(shown) {
        lines.push(step_line(step, width));
    }
    lines.push(Line::from(""));
    let (bits, amp, total) = running(moment, shown);
    lines.push(Line::from(Span::styled(
        format!("{bits} bits  ×  amp {amp}"),
        theme::amber_bold(),
    )));
    lines.push(Line::from(Span::styled(
        format!("= {}", format_points(total)),
        theme::green_bold(),
    )));
    frame.render_widget(Paragraph::new(lines).style(screen()), body);

    let action = if moment.finished() {
        "continue"
    } else {
        "skip"
    };
    frame.render_widget(
        Paragraph::new(footer(&[("q", "quit"), ("?", "help"), ("any", action)])).style(screen()),
        foot,
    );
}

fn step_line(step: &ScoreStep, width: usize) -> Line<'static> {
    let detail = if step.clamp {
        "back to 0".to_string()
    } else {
        let mut parts = Vec::new();
        if step.bits_delta != 0 {
            parts.push(format!("{:+} bits", step.bits_delta));
        }
        if step.amp_delta.milli() != 0 {
            if step.amp_delta.milli() > 0 {
                parts.push(format!("+{} amp", step.amp_delta));
            } else {
                parts.push(format!("{} amp", step.amp_delta));
            }
        }
        parts.join("  ")
    };
    let text = if detail.is_empty() {
        step.label.clone()
    } else {
        format!("{}   {detail}", step.label)
    };
    let amp_only = step.amp_delta.milli() != 0 && step.bits_delta == 0;
    let style = if step.label == "absent symbol" || step.clamp || amp_only {
        theme::amber()
    } else {
        theme::green()
    };
    Line::from(Span::styled(fit(&text, width), style))
}

fn running(moment: &crate::app::ScoreMoment, shown: usize) -> (i32, Amp, i64) {
    if shown >= moment.steps.len() && !moment.steps.is_empty() {
        return (moment.final_bits, moment.final_amp, moment.final_milli);
    }
    if let Some(step) = moment.steps.get(shown.saturating_sub(1)) {
        let total = i64::from(step.run_bits).saturating_mul(i64::from(step.run_amp.milli()));
        return (step.run_bits, step.run_amp, total);
    }
    (0, moment.base_amp, 0)
}
