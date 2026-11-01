//! Title. New run, or a seed such as `MV-7F3A-9C21-L4`.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::App;
use crate::format::center;

use super::theme::{self, screen};
use super::{footer, frame_shell};

const LOGO: [&str; 5] = [
    r#"#   #  ###   #### ##### ##### ####  #   # ##### ####  #####  ####"#,
    r#"## ## #   # #       #   #     #   # #   #   #   #   # #     #    "#,
    r#"# # # #####  ###    #   ####  ####  #   #   #   ####  ####   ### "#,
    r#"#   # #   #     #   #   #     # #    # #    #   #   # #         #"#,
    r#"#   # #   # ####    #   ##### #  #    #   ##### ####  ##### #### "#,
];

pub fn draw(frame: &mut Frame, area: Rect, app: &App) {
    let inner = frame_shell(frame, area, "mastervibes");
    let [logo, menu, hint] = Layout::vertical([
        Constraint::Length(7),
        Constraint::Length(8),
        Constraint::Min(3),
    ])
    .areas(inner);

    draw_logo(frame, logo);
    draw_menu(frame, menu, app);
    draw_hint(frame, hint, app);
}

fn draw_logo(frame: &mut Frame, area: Rect) {
    let width = area.width as usize;
    let mut lines: Vec<Line> = LOGO
        .iter()
        .map(|row| Line::from(Span::styled(center(row, width), theme::green_bold())))
        .collect();
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        center("probe   ·   pegs   ·   bits × amp", width),
        theme::amber(),
    )));
    frame.render_widget(Paragraph::new(lines).style(screen()), area);
}

fn draw_menu(frame: &mut Frame, area: Rect, app: &App) {
    let width = area.width.min(62);
    let rect = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y,
        width,
        height: area.height.min(8),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::green())
        .style(screen());
    let inner = block.inner(rect);
    frame.render_widget(block, rect);

    let new_mark = if app.title_row == 0 { ">" } else { " " };
    let seed_mark = if app.title_row == 1 { ">" } else { " " };
    let new_style = if app.title_row == 0 {
        theme::amber_bold()
    } else {
        theme::green()
    };
    let seed_style = if app.title_row == 1 {
        theme::amber_bold()
    } else {
        theme::green()
    };
    let seed = if app.seed_input.is_empty() {
        if app.title_row == 1 {
            "_".to_string()
        } else {
            "MV-7F3A-9C21-L4".to_string()
        }
    } else {
        format!("{}_", app.seed_input)
    };
    let seed_value_style = if app.seed_input.is_empty() && app.title_row == 0 {
        theme::dim()
    } else {
        theme::amber()
    };

    let lines = vec![
        Line::from(Span::styled(
            format!("{new_mark} NEW RUN      random seed"),
            new_style,
        )),
        Line::from(vec![
            Span::styled(format!("{seed_mark} ENTER SEED   "), seed_style),
            Span::styled(seed, seed_value_style),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "a run starts at layer 1. the seed's layer is a share mark.",
            theme::dim(),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(screen())
            .wrap(Wrap { trim: false }),
        inner,
    );
}

fn draw_hint(frame: &mut Frame, area: Rect, app: &App) {
    let [keys, rest] = Layout::vertical([Constraint::Length(1), Constraint::Min(1)]).areas(area);
    frame.render_widget(
        Paragraph::new(footer(&[
            ("q", "quit"),
            ("?", "help"),
            ("up/down", "select"),
            ("enter", "confirm"),
        ]))
        .style(screen()),
        keys,
    );
    let note = if app.status.is_empty() {
        Line::from(Span::styled(
            "TIP: an absent symbol scores 0. All Unique pays the least.",
            theme::dim(),
        ))
    } else {
        Line::from(Span::styled(app.status.clone(), theme::amber()))
    };
    frame.render_widget(Paragraph::new(note).style(screen()), rest);
}
