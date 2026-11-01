//! Help, quit, and the absent-symbol warning.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;

use super::centered;
use super::theme::{self, screen};

pub fn help(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled("PEGS", theme::amber_bold())),
        Line::from("A hit is the right symbol in the right slot."),
        Line::from("A near is the right symbol in the wrong slot."),
        Line::from("The row shows how many of each. * is a hit, + is a near."),
        Line::from(""),
        Line::from(Span::styled("SIGNATURE", theme::amber_bold())),
        Line::from("A full probe forms one signature: the shape that"),
        Line::from("pays (pair, palindrome, triple, mono, and the rest)."),
        Line::from(""),
        Line::from(Span::styled("SCORING", theme::amber_bold())),
        Line::from("Score is bits x amp. Hits and nears add bits, then"),
        Line::from("tools. An absent symbol wipes the probe: it scores 0."),
        Line::from("A cracked breach is multiplied by (1 + 0.3 per probe"),
        Line::from("left), capped at x5."),
        Line::from(""),
        Line::from(Span::styled("any key closes", theme::dim())),
    ];
    let width = area.width.saturating_sub(2);
    let height = area.height.saturating_sub(2).min(22);
    box_over(frame, area, " help ", lines, width, height);
}

pub fn quit(frame: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from("quit mastervibe?"),
        Line::from(""),
        Line::from(vec![
            Span::styled("[y]", theme::amber()),
            Span::raw(" quit    "),
            Span::styled("[n]", theme::amber()),
            Span::raw(" stay"),
        ]),
    ];
    box_over(frame, area, " quit ", lines, 42, 6);
}

pub fn dead(frame: &mut Frame, area: Rect, app: &App) {
    let warning = app
        .absent_warning()
        .unwrap_or_else(|| "this probe scores 0.".to_string());
    let lines = vec![
        Line::from(Span::styled(warning, theme::amber())),
        Line::from(""),
        Line::from(vec![
            Span::styled("[y]", theme::amber()),
            Span::raw(" submit    "),
            Span::styled("[n]", theme::amber()),
            Span::raw(" edit"),
        ]),
    ];
    let width = area.width.saturating_sub(4);
    box_over(frame, area, " absent symbol ", lines, width, 8);
}

fn box_over(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    lines: Vec<Line<'_>>,
    width: u16,
    height: u16,
) {
    let rect = centered(area, width, height);
    frame.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::amber())
        .title(title)
        .style(screen());
    let inner = block.inner(rect);
    frame.render_widget(block, rect);
    frame.render_widget(
        Paragraph::new(lines)
            .style(screen())
            .wrap(Wrap { trim: true }),
        inner,
    );
}
