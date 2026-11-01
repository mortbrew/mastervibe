//! Screens. Drawing only: every number comes from the core or from [`crate::format`].

mod board;
mod over;
mod overlay;
mod payout;
mod scoring;
mod shop;
mod theme;
mod title;

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use ratatui::Terminal;

use crate::app::{App, Screen};

use self::theme::screen;

/// Draw `app` into `frame`.
pub fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    frame.render_widget(Block::default().style(screen()), area);
    if area.width < 80 || area.height < 24 {
        too_small(frame, area);
    } else {
        match app.screen {
            Screen::Title => title::draw(frame, area, app),
            Screen::Board => board::draw(frame, area, app),
            Screen::Scoring => scoring::draw(frame, area, app),
            Screen::Payout => payout::draw(frame, area, app),
            Screen::Shop => shop::draw(frame, area, app),
            Screen::GameOver => over::draw(frame, area, app),
        }
    }
    if app.dead_confirm {
        overlay::dead(frame, area, app);
    }
    if app.quit_confirm {
        overlay::quit(frame, area);
    }
    if app.help {
        overlay::help(frame, area);
    }
}

/// Render one frame to plain text. Tests and the screenshot pass use this.
pub fn render_screen(app: &App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test backend opens");
    terminal
        .draw(|frame| draw(frame, app))
        .expect("frame draws");
    buffer_text(terminal.backend().buffer())
}

pub(crate) fn shell(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(theme::green())
        .title(format!(" {title} "))
        .style(screen())
}

pub(crate) fn frame_shell(frame: &mut Frame, area: Rect, title: &str) -> Rect {
    let block = shell(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    inner
}

pub(crate) fn footer(hints: &[(&str, &str)]) -> Line<'static> {
    let mut spans = Vec::new();
    for (index, (key, action)) in hints.iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled("  ", theme::dim()));
        }
        spans.push(Span::styled(format!("[{key}]"), theme::amber()));
        spans.push(Span::styled(format!(" {action}"), theme::green()));
    }
    Line::from(spans)
}

pub(crate) fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

fn too_small(frame: &mut Frame, area: Rect) {
    let message = format!(
        "mastervibes needs at least 80x24. this terminal is {}x{}.",
        area.width, area.height
    );
    frame.render_widget(Paragraph::new(message).style(theme::amber()), area);
}

fn buffer_text(buf: &ratatui::buffer::Buffer) -> String {
    let area = buf.area;
    let mut out = String::new();
    for y in area.y..area.y.saturating_add(area.height) {
        for x in area.x..area.x.saturating_add(area.width) {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}
