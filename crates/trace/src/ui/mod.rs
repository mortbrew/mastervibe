//! Screens. Layout follows the portrait mockups and shrinks down to about 120×40.

mod art;
pub mod theme;

use std::cmp::min;

use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph, Wrap};
use ratatui::Frame;
use trace_core::{
    fmt_amp, script, tool, Arch, LevelKind, Offer, Phase, Rarity, ToolId, B2_TARGET, B3_TARGET,
};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Hit, HitKind, Screen};
use art::{for_arch, LEVEL, LOGO, SCRIPT};
use theme::{palette, Pal};

pub use theme::PaletteId as Palette;

pub fn draw(frame: &mut Frame, app: &mut App) {
    app.cols = frame.area().width;
    app.rows = frame.area().height;
    app.hits.clear();
    let area = frame.area();
    let pal = palette(app.config.palette);
    frame.render_widget(Block::default().style(pal.fg(pal.green)), area);
    if area.width < 80 || area.height < 24 {
        let msg = Paragraph::new("resize the terminal — ./trace wants about 120×40")
            .style(pal.bold(pal.amber))
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(msg, area);
        return;
    }
    if app.help {
        help(frame, area, pal);
    } else {
        match app.screen {
            Screen::Boot => boot(frame, area, app, pal),
            Screen::Title => title(frame, area, app, pal),
            Screen::Options => options(frame, area, app, pal),
            Screen::NewRun => new_run(frame, area, app, pal),
            Screen::Play => play(frame, area, app, pal),
        }
    }
    polish(frame, app, pal);
}

fn boot(frame: &mut Frame, area: Rect, app: &App, pal: Pal) {
    shell(frame, area, pal, "./trace · boot", "any key skips");
    let inner = inset(area);
    let lines = boot_lines(app);
    let shown = boot_shown(app);
    let mut text = Vec::new();
    text.push(Line::from(Span::styled(" boot", pal.dim_style())));
    for (i, (tag, msg)) in lines.iter().enumerate() {
        if i < shown {
            let color = if *tag == "!!" { pal.danger } else { pal.green };
            text.push(Line::from(vec![
                Span::styled(format!(" [ {tag} ] "), pal.bold(color)),
                Span::styled(msg.clone(), pal.fg(pal.text)),
            ]));
        }
    }
    if shown > lines.len() {
        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            " kid@nullbox:~$ ./trace █",
            pal.bold(pal.green),
        )));
    }
    let box_w = min(52, inner.width.saturating_sub(2));
    let rect = Rect {
        x: inner.x + 2,
        y: inner.y + 1,
        width: box_w,
        height: min(12, inner.height.saturating_sub(2)),
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(pal.fg(pal.dim))
        .style(pal.fg(pal.text));
    frame.render_widget(
        Paragraph::new(text).block(block).wrap(Wrap { trim: false }),
        rect,
    );
}

pub fn boot_lines(app: &App) -> Vec<(&'static str, String)> {
    let save = if app.has_save {
        format!("save found: {}", app.save_blurb())
    } else {
        "no save".to_string()
    };
    vec![
        ("OK", "mounting /dev/kid".into()),
        ("OK", "proxy chain: 3 nodes".into()),
        ("OK", "intent deck: 20 cards".into()),
        ("!!", "sysadmin is awake".into()),
        ("OK", save),
    ]
}

pub fn boot_shown(app: &App) -> usize {
    let step = app.scaled(280);
    (app.boot_ms / step).min(8) as usize
}

fn title(frame: &mut Frame, area: Rect, app: &mut App, pal: Pal) {
    let seed = trace_core::seed_label(app.pending_seed);
    shell(
        frame,
        area,
        pal,
        "./trace · title",
        &format!("v0.2 · {seed}"),
    );
    let inner = inset(area);
    let boot_w = min(46, inner.width / 3);
    let boot_h = min(12, inner.height.saturating_sub(8));
    let boot_rect = Rect {
        x: inner.x + 1,
        y: inner.y + 1,
        width: boot_w,
        height: boot_h,
    };
    let mut blog = vec![Line::from(Span::styled(" boot", pal.dim_style()))];
    for (tag, msg) in boot_lines(app) {
        let color = if tag == "!!" { pal.danger } else { pal.green };
        blog.push(Line::from(vec![
            Span::styled(format!(" [ {tag} ] "), pal.bold(color)),
            Span::styled(msg, pal.fg(pal.text)),
        ]));
    }
    blog.push(Line::from(""));
    blog.push(Line::from(Span::styled(
        " kid@nullbox:~$ ./trace █",
        pal.bold(pal.green),
    )));
    frame.render_widget(
        Paragraph::new(blog).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(pal.fg(pal.dim)),
        ),
        boot_rect,
    );

    let logo_x = inner.x + boot_w + 4;
    let logo_w = inner.width.saturating_sub(boot_w + 6);
    for (i, line) in LOGO.iter().enumerate() {
        let y = inner.y + 1 + i as u16;
        if y < inner.y + inner.height && logo_w > 8 {
            put(frame, logo_x, y, line, pal.bold(pal.green), logo_w);
        }
    }

    let items = menu_items(app);
    let menu_y = inner.y + 8;
    for (i, (name, detail, enabled)) in items.iter().enumerate() {
        let y = menu_y + i as u16;
        if y + 1 >= inner.bottom() {
            break;
        }
        let selected = i == app.menu;
        let (fg, bg) = if selected {
            (pal.bg, pal.amber)
        } else if *enabled {
            (pal.green, pal.bg)
        } else {
            (pal.dim, pal.bg)
        };
        let marker = if selected { "►" } else { " " };
        let label = format!(" {marker} {name:<14} {detail}");
        put(
            frame,
            inner.x + boot_w + 4,
            y,
            &label,
            Style::default().fg(fg).bg(bg).add_modifier(Modifier::BOLD),
            logo_w,
        );
        app.hits.push(Hit {
            x: inner.x + boot_w + 4,
            y,
            w: logo_w,
            h: 1,
            kind: HitKind::Menu(i),
        });
    }

    let tip = tips(app.elapsed);
    let tip_y = inner.bottom().saturating_sub(3);
    let tip_rect = Rect {
        x: inner.x + 1,
        y: tip_y,
        width: inner.width.saturating_sub(2),
        height: 1,
    };
    put(
        frame,
        tip_rect.x,
        tip_rect.y,
        &format!(" TIP  {tip}"),
        pal.fg(pal.dim),
        tip_rect.width,
    );
    keybar(
        frame,
        inner,
        pal,
        &[
            ("j/k", "move"),
            ("enter", "select"),
            ("mouse", "hover · click"),
            ("q", "exit"),
        ],
    );
}

pub fn menu_items(app: &App) -> Vec<(String, String, bool)> {
    let continue_detail = if app.has_save {
        app.save_blurb()
    } else {
        "no save".into()
    };
    vec![
        ("CONTINUE".into(), continue_detail, app.has_save),
        ("NEW RUN".into(), "default rig · sudo 0 · seed".into(), true),
        ("DAILY BREACH".into(), "sealed this build".into(), true),
        ("CTFs".into(), "sealed this build".into(), true),
        ("/var/log".into(), "sealed this build".into(), true),
        ("OPTIONS".into(), "palette · volume · mouse".into(), true),
        ("EXIT".into(), "logout".into(), true),
    ]
}

fn tips(elapsed: u64) -> &'static str {
    const TIPS: &[&str] = &[
        "the ▼ marker never lies. if it sits on @you, press q.",
        "wipe a LOG and the trace takes a step back.",
        "jack out is free. staying one turn too long is how you get traced.",
        "autosave writes at every shop and at the start of every breach.",
        "amp multiplies the bits. a KEY is worth more than another chunk early.",
        "greedy play fills the haul and still loses it. read the marker.",
    ];
    TIPS[((elapsed / 4000) as usize) % TIPS.len()]
}

fn options(frame: &mut Frame, area: Rect, app: &mut App, pal: Pal) {
    shell(
        frame,
        area,
        pal,
        "./trace · OPTIONS",
        "saved to ~/.config/trace",
    );
    let inner = inset(area);
    let rows = option_rows(app);
    let left_w = min(70, inner.width.saturating_sub(42));
    for (i, (name, value)) in rows.iter().enumerate() {
        let y = inner.y + 1 + i as u16;
        if y + 2 >= inner.bottom() {
            break;
        }
        let selected = i == app.opt;
        let style = if selected {
            Style::default()
                .fg(pal.bg)
                .bg(pal.amber)
                .add_modifier(Modifier::BOLD)
        } else {
            pal.fg(pal.text)
        };
        let line = format!("  {name:<18} {value}");
        put(frame, inner.x + 1, y, &line, style, left_w);
        app.hits.push(Hit {
            x: inner.x + 1,
            y,
            w: left_w,
            h: 1,
            kind: HitKind::Opt(i),
        });
    }
    let preview = Rect {
        x: inner.x + left_w + 2,
        y: inner.y + 1,
        width: inner.width.saturating_sub(left_w + 4),
        height: inner.height.saturating_sub(4),
    };
    if preview.width > 16 && preview.height > 8 {
        let title = format!("PREVIEW · {}", app.config.palette.label());
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(pal.fg(pal.frame))
            .title(Span::styled(format!(" {title} "), pal.bold(pal.frame)));
        frame.render_widget(block, preview);
        let samples = [
            ("trace / danger", "orange  ◆", pal.danger),
            ("safe / looted", "blue    ✓", pal.safe),
            ("DATA", "cyan", pal.data),
            ("KEY", "yellow  ◆", pal.key),
            ("CRED", "amber   $", pal.cred),
            ("LOG", "magenta ▲", pal.log),
        ];
        for (i, (name, note, color)) in samples.iter().enumerate() {
            let y = preview.y + 2 + i as u16;
            put(
                frame,
                preview.x + 2,
                y,
                &format!("{name:<16} {note}"),
                pal.bold(*color),
                preview.width.saturating_sub(4),
            );
        }
        let note_y = preview.y + 10;
        put(
            frame,
            preview.x + 2,
            note_y,
            "every colour also has a glyph, so nothing",
            pal.fg(pal.dim),
            preview.width.saturating_sub(4),
        );
        put(
            frame,
            preview.x + 2,
            note_y + 1,
            "relies on hue alone",
            pal.fg(pal.dim),
            preview.width.saturating_sub(4),
        );
        put(
            frame,
            preview.x + 2,
            note_y + 3,
            "palettes: default · deut · prot · trit · mono",
            pal.fg(pal.dim),
            preview.width.saturating_sub(4),
        );
    }
    keybar(
        frame,
        inner,
        pal,
        &[
            ("j/k", "move"),
            ("h/l", "change"),
            ("enter", "toggle"),
            ("esc", "back"),
            ("mouse", "click"),
        ],
    );
}

pub fn option_rows(app: &App) -> Vec<(String, String)> {
    let speeds = ["0.5x", "1x", "1.5x", "2x"];
    let speed = speeds
        .get(app.config.speed as usize)
        .copied()
        .unwrap_or("1x");
    let shake = match app.config.shake {
        0 => "off",
        1 => "low",
        _ => "high",
    };
    vec![
        ("palette".into(), app.config.palette.label().into()),
        ("crt scanlines".into(), on_off(app.config.scanlines).into()),
        ("screen shake".into(), shake.into()),
        ("animation speed".into(), speed.into()),
        ("music".into(), format!("{}", app.config.music)),
        ("sfx".into(), format!("{}", app.config.sfx)),
        ("adaptive music".into(), on_off(app.config.adaptive).into()),
        ("mouse".into(), on_off(app.config.mouse).into()),
        ("hover inspects".into(), on_off(app.config.hover).into()),
        (
            "confirm buys".into(),
            if app.config.confirm_buys {
                "double-click"
            } else {
                "single click"
            }
            .into(),
        ),
        (
            "keys".into(),
            if app.config.vim {
                "vim (hjkl) / arrows"
            } else {
                "arrows / hjkl"
            }
            .into(),
        ),
        ("show ▼ math".into(), on_off(app.config.math).into()),
        ("tutorial tips".into(), on_off(app.config.tips).into()),
        ("reset save".into(), "hold enter 3s".into()),
    ]
}

fn on_off(v: bool) -> &'static str {
    if v {
        "on"
    } else {
        "off"
    }
}

fn new_run(frame: &mut Frame, area: Rect, app: &App, pal: Pal) {
    shell(frame, area, pal, "./trace · NEW RUN", "L1 home lan");
    let inner = inset(area);
    let seed = trace_core::seed_label(app.pending_seed);
    let lines = [
        "rig        default",
        "sudo       0",
        "layer      L1 home lan · tutorial, then THE PARENT",
    ];
    for (i, line) in lines.iter().enumerate() {
        put(
            frame,
            inner.x + 4,
            inner.y + 2 + i as u16,
            line,
            pal.fg(pal.text),
            inner.width.saturating_sub(6),
        );
    }
    put(
        frame,
        inner.x + 4,
        inner.y + 6,
        &format!("seed       {seed}"),
        pal.bold(pal.amber),
        inner.width.saturating_sub(6),
    );
    put(
        frame,
        inner.x + 4,
        inner.y + 8,
        "this build plays the L1 slice on the default rig.",
        pal.fg(pal.dim),
        inner.width.saturating_sub(6),
    );
    let confirm = if app.confirm_new {
        "a save is already on disk. enter again to overwrite it."
    } else {
        "enter starts the tutorial. s rolls a new seed."
    };
    put(
        frame,
        inner.x + 4,
        inner.y + 10,
        confirm,
        pal.bold(if app.confirm_new {
            pal.danger
        } else {
            pal.green
        }),
        inner.width.saturating_sub(6),
    );
    keybar(
        frame,
        inner,
        pal,
        &[("s", "new seed"), ("enter", "start"), ("esc", "back")],
    );
}

fn play(frame: &mut Frame, area: Rect, app: &mut App, pal: Pal) {
    let phase = app.campaign.phase;
    let right = match phase {
        Phase::Shop => format!("${} · shop", app.campaign.cash),
        Phase::LevelUp => format!("${} · level-up", app.campaign.cash),
        Phase::Cleared => "layer cleared".into(),
        Phase::Over => "run over".into(),
        _ => {
            if let Some(b) = app.campaign.breach.as_ref() {
                format!("turn {} · ${} · sudo 0", b.turn, app.campaign.cash)
            } else {
                format!("${}", app.campaign.cash)
            }
        }
    };
    let left = play_title(app);
    shell(frame, area, pal, &left, &right);
    let inner = inset(area);
    match phase {
        Phase::Shop => shop(frame, inner, app, pal),
        Phase::LevelUp => level_up(frame, inner, app, pal),
        Phase::Cleared => cleared(frame, inner, app, pal),
        Phase::Over => over(frame, inner, app, pal),
        Phase::Tutorial | Phase::Breach => breach(frame, inner, app, pal),
    }
    if app.payout_open {
        payout(frame, area, app, pal);
    }
    if !app.status.is_empty() && !app.payout_open {
        let y = area.y + area.height.saturating_sub(2);
        let width = area.width / 3;
        let msg = truncate(&app.status, width.saturating_sub(2) as usize);
        put(
            frame,
            area.x + area.width.saturating_sub(width + 1),
            y,
            &msg,
            pal.fg(pal.amber),
            width,
        );
    }
}

fn play_title(app: &App) -> String {
    if app.tut_on {
        let name = match app.tut_step {
            1 => "the tree",
            2 => "cycles",
            3 => "the trace",
            4 => "the deck",
            5 => "locked",
            6 => "jack out",
            _ => "proxies",
        };
        return format!("./trace · TUTORIAL {}/7 · {name}", app.tut_step);
    }
    match app.campaign.phase {
        Phase::Shop => "./trace · SHOP".into(),
        Phase::LevelUp => "./trace · LEVEL-UP".into(),
        Phase::Cleared => "./trace · LAYER CLEARED".into(),
        Phase::Over => "./trace · RUN OVER".into(),
        _ => {
            if let Some(b) = app.campaign.breach.as_ref() {
                format!("./trace · {} · {}", b.layer_name, b.breach_name)
            } else {
                "./trace".into()
            }
        }
    }
}

fn breach(frame: &mut Frame, area: Rect, app: &mut App, pal: Pal) {
    let Some(snapshot) = app
        .campaign
        .breach
        .as_ref()
        .map(|b| BreachSnap::from(b, app))
    else {
        return;
    };
    let layout = breach_layout(area, snapshot.show_tools);
    panel(
        frame,
        layout.chain,
        pal,
        "PROXY CHAIN",
        pal.frame,
        snapshot.chain.clone(),
    );
    if app.tut_on && app.tut_step == 3 {
        put(
            frame,
            layout.chain.x + layout.chain.width.saturating_sub(16),
            layout.chain.y + 1,
            "▲ look here",
            pal.bold(pal.amber),
            14,
        );
    }
    let intent_color = if snapshot.danger && app.blink {
        pal.danger
    } else {
        pal.amber
    };
    panel(
        frame,
        layout.intent,
        pal,
        "SYSADMIN NEXT",
        intent_color,
        snapshot.intent.clone(),
    );
    let mut files = snapshot.files.clone();
    if layout.files.height > snapshot.files.len() as u16 + 6 {
        files.push(Line::from(""));
        files.push(Line::from(Span::styled(
            " legend  ▒ hidden until you cd in (1 cycle = a draw)",
            pal.dim_style(),
        )));
        files.push(Line::from(Span::styled(
            "         [L] locked: decrypt first   ✓ looted",
            pal.dim_style(),
        )));
    }
    panel(frame, layout.files, pal, &snapshot.pwd, pal.frame, files);
    let file_top = layout.files.y + 1;
    for i in 0..snapshot.row_count {
        let y = file_top + i as u16;
        if y + 1 >= layout.files.y + layout.files.height {
            break;
        }
        app.hits.push(Hit {
            x: layout.files.x + 1,
            y,
            w: layout.files.width.saturating_sub(2),
            h: 1,
            kind: HitKind::Tree(i),
        });
    }
    panel(
        frame,
        layout.haul,
        pal,
        "HAUL",
        pal.frame,
        snapshot.haul.clone(),
    );
    panel(
        frame,
        layout.cycles,
        pal,
        "CYCLES",
        pal.frame,
        snapshot.cycles.clone(),
    );
    if layout.inspect.height > 2 {
        panel(
            frame,
            layout.inspect,
            pal,
            &snapshot.inspect_title,
            snapshot.inspect_color,
            snapshot.inspect.clone(),
        );
    }
    if snapshot.show_tools && layout.tools.height > 4 {
        put(
            frame,
            layout.tools.x,
            layout.tools.y,
            "TOOLS  hover = inspect · click = focus · right-click = pin",
            pal.bold(pal.frame),
            layout.tools.width,
        );
        draw_owned_tools(frame, app, pal, layout.tools);
        draw_bin(frame, app, pal, layout.bin);
    }
    if app.tut_on {
        mentor(frame, area, app, pal);
    }
    keybar(frame, area, pal, &snapshot.keys);
}

struct BreachSnap {
    chain: Vec<Line<'static>>,
    intent: Vec<Line<'static>>,
    files: Vec<Line<'static>>,
    haul: Vec<Line<'static>>,
    cycles: Vec<Line<'static>>,
    inspect: Vec<Line<'static>>,
    inspect_title: String,
    inspect_color: ratatui::style::Color,
    pwd: String,
    row_count: usize,
    show_tools: bool,
    danger: bool,
    keys: Vec<(&'static str, &'static str)>,
}

impl BreachSnap {
    fn from(breach: &trace_core::Breach, app: &App) -> Self {
        let pal = palette(app.config.palette);
        let view = breach.intent_view();
        let proj = breach.projected_hops();
        let danger = proj <= 0;
        let chain = chain_lines(breach, pal, danger && app.blink);
        let mut intent = vec![
            Line::from(Span::styled(
                view.title.clone(),
                pal.bold(if danger { pal.danger } else { pal.amber }),
            )),
            Line::from(Span::styled(view.detail.clone(), pal.fg(pal.text))),
            Line::from(""),
            Line::from(Span::styled(
                format!("intent deck {} left · of {}", view.left, view.total),
                pal.dim_style(),
            )),
        ];
        if app.config.math {
            intent.push(Line::from(Span::styled(
                view.math.clone(),
                pal.fg(pal.amber),
            )));
        }
        if breach.boss == trace_core::Boss::Parent {
            intent.push(Line::from(Span::styled(
                "THE PARENT · unwiped LOGs add hops from turn 4",
                pal.fg(pal.log),
            )));
        }
        let rows = breach.rows();
        let cursor = breach.cursor;
        let files: Vec<Line<'static>> = rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let selected = i == cursor;
                let color = tone_color(pal, row.tone);
                let bg = if selected { pal.select_bg } else { pal.bg };
                let name = format!("{:<16}", truncate(&row.name, 16));
                let label = format!("{:<8}", truncate(&row.label, 8));
                Line::from(vec![
                    Span::styled(
                        format!("{} {name}", if selected { "►" } else { " " }),
                        Style::default()
                            .fg(if selected { pal.amber } else { pal.text })
                            .bg(bg),
                    ),
                    Span::styled(
                        label,
                        Style::default()
                            .fg(color)
                            .bg(bg)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        truncate(&row.detail, 42),
                        Style::default().fg(pal.dim).bg(bg),
                    ),
                ])
            })
            .collect();
        let preview = breach.jack_preview();
        let target = breach
            .target
            .map(|t| t.to_string())
            .unwrap_or_else(|| "—".into());
        let ratio = breach
            .target
            .map(|t| (preview.score as f32 / t as f32).clamp(0.0, 1.0))
            .unwrap_or(0.0);
        let bar_w = 28;
        let fill = (ratio * bar_w as f32) as usize;
        let bar = format!("{}{}", "█".repeat(fill), "░".repeat(bar_w - fill));
        let mut haul = vec![
            Line::from(vec![
                Span::styled(format!("{} bits", preview.bits), pal.bold(pal.data)),
                Span::styled(" × ", pal.dim_style()),
                Span::styled(fmt_amp(preview.amp_milli), pal.bold(pal.rare)),
                Span::styled(" amp = ", pal.dim_style()),
                Span::styled(preview.score.to_string(), pal.bold(pal.amber)),
            ]),
            Line::from(""),
            Line::from(Span::styled(format!("target {target}"), pal.fg(pal.text))),
            Line::from(Span::styled(bar, pal.fg(pal.green))),
            Line::from(Span::styled(
                "traced = keep 50% · miss target = run over",
                pal.dim_style(),
            )),
        ];
        if preview.near_miss {
            haul.push(Line::from(Span::styled(
                "near miss · the trace is 1 hop out",
                pal.bold(pal.amber),
            )));
        }
        let dots: String = (0..3)
            .map(|i| if i < breach.cycles { "● " } else { "○ " })
            .collect();
        let cycles = vec![Line::from(vec![
            Span::styled(dots, pal.bold(pal.green)),
            Span::styled(
                format!(" {}/3 left this turn", breach.cycles.min(3)),
                pal.fg(pal.text),
            ),
        ])];
        let (inspect_title, inspect_color, inspect) = inspect_lines(app, pal);
        let show_tools = !app.campaign.tools.is_empty() || !app.campaign.scripts.is_empty();
        let keys = legal_keys(app, danger);
        Self {
            chain,
            intent,
            files,
            haul,
            cycles,
            inspect,
            inspect_title,
            inspect_color,
            pwd: format!("FILES {}", breach.pwd()),
            row_count: rows.len(),
            show_tools,
            danger,
            keys,
        }
    }
}

fn chain_lines(breach: &trace_core::Breach, pal: Pal, flash: bool) -> Vec<Line<'static>> {
    let proxies = breach.proxies.min(6);
    let mut nodes = vec!["[ADMIN]".to_string()];
    for i in 1..=proxies {
        nodes.push(format!("p{i}"));
    }
    nodes.push("@you".to_string());
    let body = nodes.join("━━━━");
    let width = body.chars().count().max(1);
    let scale = 8.max(breach.hops);
    let pos = |hops: i32| -> usize {
        let t = (hops as f32 / scale as f32).clamp(0.0, 1.0);
        ((1.0 - t) * (width.saturating_sub(1) as f32)) as usize
    };
    let mut trace_pad = " ".repeat(pos(breach.hops as i32));
    trace_pad.push_str("◆ trace");
    let proj = breach.projected_hops();
    let marker_at = if proj <= 0 {
        width.saturating_sub(1)
    } else {
        pos(proj)
    };
    let mut mark = " ".repeat(marker_at);
    mark.push('▼');
    if proj <= 0 {
        mark.push_str(" TRACED");
    } else {
        mark.push_str(" next turn lands here");
    }
    let mark_color = if proj <= 0 {
        if flash {
            pal.danger
        } else {
            pal.amber
        }
    } else {
        pal.amber
    };
    vec![
        Line::from(Span::styled(trace_pad, pal.bold(pal.danger))),
        Line::from(Span::styled(body, pal.fg(pal.text))),
        Line::from(Span::styled(mark, pal.bold(mark_color))),
        Line::from(Span::styled(
            format!(
                "trace {} hops from you · proxies {}",
                breach.hops, breach.proxies
            ),
            pal.dim_style(),
        )),
    ]
}

fn inspect_lines(app: &App, pal: Pal) -> (String, ratatui::style::Color, Vec<Line<'static>>) {
    let hover = if app.config.hover && !app.pinned {
        app.hover_tool
    } else {
        None
    };
    let index = hover.or_else(|| {
        if app.campaign.tools.is_empty() {
            None
        } else {
            Some(app.tool_i % app.campaign.tools.len())
        }
    });
    if let Some(i) = index {
        if let Some(id) = app.campaign.tools.get(i).copied() {
            return tool_inspect(id, pal, hover.is_some());
        }
    }
    if let Some(i) = app.hover_offer {
        if let Some(Some(offer)) = app.campaign.shop.as_ref().and_then(|s| s.offers.get(i)) {
            return offer_inspect(offer, &app.campaign, pal);
        }
    }
    (
        "INSPECT".into(),
        pal.frame,
        vec![
            Line::from(Span::styled("Tab or hover a card.", pal.fg(pal.text))),
            Line::from(Span::styled(
                "The picture is the tool. The words are in here.",
                pal.dim_style(),
            )),
        ],
    )
}

fn tool_inspect(
    id: ToolId,
    pal: Pal,
    hovered: bool,
) -> (String, ratatui::style::Color, Vec<Line<'static>>) {
    let def = tool(id);
    let color = rarity_color(pal, def.rarity, def.arch);
    let title = if hovered {
        format!("INSPECT · {} (hover)", def.name)
    } else {
        format!("INSPECT · {}", def.name)
    };
    let lines = vec![
        Line::from(Span::styled(
            format!("{} · {}", def.rarity.label(), def.arch.label()),
            pal.bold(color),
        )),
        Line::from(Span::styled(def.effect.to_string(), pal.fg(pal.text))),
        Line::from(Span::styled(
            format!("synergy: {}", def.synergy),
            pal.dim_style(),
        )),
        Line::from(Span::styled(format!("${}", def.price), pal.fg(pal.amber))),
    ];
    (title, color, lines)
}

fn offer_inspect(
    offer: &Offer,
    campaign: &trace_core::Campaign,
    pal: Pal,
) -> (String, ratatui::style::Color, Vec<Line<'static>>) {
    match offer {
        Offer::Tool(id) => tool_inspect(*id, pal, true),
        Offer::Level(kind) => {
            let have = campaign.levels.get(*kind);
            let cost = campaign.levels.cost(*kind).unwrap_or(0);
            (
                format!("INSPECT · {} level-up", kind.name()),
                pal.amber,
                vec![
                    Line::from(Span::styled(
                        format!("lv{have} → lv{}", have + 1),
                        pal.bold(pal.amber),
                    )),
                    Line::from(Span::styled(kind.next_text(have), pal.fg(pal.text))),
                    Line::from(Span::styled(format!("${cost}"), pal.fg(pal.amber))),
                ],
            )
        }
        Offer::Script(id) => {
            let def = script(*id);
            (
                format!("INSPECT · {}", def.name),
                pal.green,
                vec![
                    Line::from(Span::styled("1-USE · ~/bin", pal.bold(pal.green))),
                    Line::from(Span::styled(def.effect.to_string(), pal.fg(pal.text))),
                    Line::from(Span::styled(format!("${}", def.price), pal.fg(pal.amber))),
                ],
            )
        }
    }
}

fn legal_keys(app: &App, danger: bool) -> Vec<(&'static str, &'static str)> {
    if app.tut_on {
        return match app.tut_step {
            1 => vec![
                ("j/k", "move"),
                ("l", "cd in"),
                ("h", "up"),
                ("esc", "skip"),
            ],
            2 => vec![
                ("s", "siphon"),
                ("j/k", "move"),
                ("spc", "end turn"),
                ("esc", "skip"),
            ],
            3 => vec![("w", "wipe LOG"), ("j/k", "move"), ("esc", "skip")],
            4 => vec![("spc", "end turn"), ("j/k", "move"), ("esc", "skip")],
            5 => vec![
                ("l", "cd"),
                ("d", "decrypt"),
                ("s", "siphon"),
                ("h", "up"),
                ("spc", "end turn"),
                ("esc", "skip"),
            ],
            6 => vec![("enter", "next"), ("esc", "skip")],
            _ => vec![
                ("b", "bounce"),
                ("x", "burn"),
                ("q", "jack out"),
                ("esc", "skip"),
            ],
        };
    }
    let mut keys = vec![
        ("j/k", "move"),
        ("l", "cd"),
        ("h", "up"),
        ("s", "siphon"),
        ("d", "decrypt"),
        ("w", "wipe"),
        ("b", "bounce"),
        ("x", "burn"),
        ("q", if danger { "JACK" } else { "jack out" }),
        ("spc", "end turn"),
        ("!", "script"),
        ("?", "help"),
    ];
    if !app.campaign.scripts.is_empty() {
        keys.push(("mouse", "hover"));
    }
    keys
}

fn draw_owned_tools(frame: &mut Frame, app: &mut App, pal: Pal, area: Rect) {
    let n = 5u16;
    let card_w = min(20, (area.width / n).max(12));
    let y = area.y + 1;
    let h = area.height.saturating_sub(1);
    for i in 0..n {
        let x = area.x + i * card_w;
        let rect = Rect {
            x,
            y,
            width: card_w.saturating_sub(1),
            height: h,
        };
        let hover = app.hover_tool == Some(i as usize);
        let focus = app.tool_i == i as usize && i < app.campaign.tools.len() as u16;
        if let Some(id) = app.campaign.tools.get(i as usize).copied() {
            let def = tool(id);
            tool_card(
                frame,
                rect,
                pal,
                def.name,
                for_arch(def.arch),
                def.rarity,
                def.arch,
                None,
                hover,
                focus,
            );
        } else {
            empty_card(frame, rect, pal, "empty");
        }
        app.hits.push(Hit {
            x: rect.x,
            y: rect.y.saturating_sub(1),
            w: rect.width,
            h: rect.height + 1,
            kind: HitKind::Tool(i as usize),
        });
    }
}

fn draw_bin(frame: &mut Frame, app: &mut App, pal: Pal, area: Rect) {
    if area.width < 8 || area.height < 4 {
        return;
    }
    panel(
        frame,
        Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        },
        pal,
        "",
        pal.bg,
        vec![],
    );
    put(
        frame,
        area.x,
        area.y,
        &format!("~/bin  {}/2 scripts", app.campaign.scripts.len()),
        pal.bold(pal.green),
        area.width,
    );
    for i in 0..2usize {
        let y = area.y + 1 + i as u16 * 3;
        if y + 2 >= area.y + area.height {
            break;
        }
        let rect = Rect {
            x: area.x,
            y,
            width: area.width,
            height: 3,
        };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(pal.fg(pal.green));
        frame.render_widget(block, rect);
        if let Some(id) = app.campaign.scripts.get(i).copied() {
            let def = script(id);
            put(
                frame,
                rect.x + 1,
                rect.y + 1,
                &format!(" !{}  $ ./{}", i + 1, def.name),
                pal.bold(pal.green),
                rect.width.saturating_sub(2),
            );
        } else {
            put(
                frame,
                rect.x + 1,
                rect.y + 1,
                &format!(" !{}  empty", i + 1),
                pal.dim_style(),
                rect.width.saturating_sub(2),
            );
        }
        app.hits.push(Hit {
            x: rect.x,
            y: rect.y,
            w: rect.width,
            h: rect.height,
            kind: HitKind::Script(i),
        });
    }
}

fn shop(frame: &mut Frame, area: Rect, app: &mut App, pal: Pal) {
    let lean = app
        .campaign
        .shop
        .as_ref()
        .map(|s| s.lean.clone())
        .unwrap_or_else(|| "your build is still open".into());
    put(
        frame,
        area.x + 1,
        area.y,
        "BLACK MARKET",
        pal.bold(pal.frame),
        20,
    );
    put(
        frame,
        area.x + 1,
        area.y + 1,
        &format!("{lean}  ·  tools stay · level-ups stay · scripts are one use"),
        pal.dim_style(),
        area.width.saturating_sub(2),
    );
    let offers = app
        .campaign
        .shop
        .as_ref()
        .map(|s| s.offers.clone())
        .unwrap_or_default();
    let n = offers.len().max(1) as u16;
    let card_w = min(24, (area.width / n).max(14));
    let card_h = min(14, area.height.saturating_sub(16));
    for (i, offer) in offers.iter().enumerate() {
        let x = area.x + 1 + i as u16 * card_w;
        let y = area.y + 3;
        put(
            frame,
            x,
            y - 1,
            &format!(" {} ", i + 1),
            Style::default()
                .fg(pal.bg)
                .bg(pal.amber)
                .add_modifier(Modifier::BOLD),
            4,
        );
        let rect = Rect {
            x,
            y,
            width: card_w.saturating_sub(1),
            height: card_h,
        };
        let selected = app.shop_i == i;
        match offer {
            Some(Offer::Tool(id)) => {
                let def = tool(*id);
                tool_card(
                    frame,
                    rect,
                    pal,
                    def.name,
                    for_arch(def.arch),
                    def.rarity,
                    def.arch,
                    Some(def.price),
                    app.hover_offer == Some(i),
                    selected,
                );
            }
            Some(Offer::Level(kind)) => {
                level_card(frame, rect, pal, *kind, &app.campaign, selected)
            }
            Some(Offer::Script(id)) => script_card(
                frame,
                rect,
                pal,
                script(*id).name,
                script(*id).price,
                selected,
            ),
            None => empty_card(frame, rect, pal, "sold"),
        }
        app.hits.push(Hit {
            x: rect.x,
            y: rect.y,
            w: rect.width,
            h: rect.height,
            kind: HitKind::Offer(i),
        });
    }
    let lower_y = area.y + 4 + card_h;
    let lower_h = area.height.saturating_sub(card_h + 7);
    if lower_h > 4 {
        let inspect_w = min(70, area.width / 2);
        let inspect_rect = Rect {
            x: area.x + 1,
            y: lower_y,
            width: inspect_w,
            height: lower_h,
        };
        let idx = app.hover_offer.unwrap_or(app.shop_i);
        let (title, color, lines) = app
            .campaign
            .shop
            .as_ref()
            .and_then(|s| s.offers.get(idx))
            .and_then(|o| o.as_ref())
            .map(|o| offer_inspect(o, &app.campaign, pal))
            .unwrap_or_else(|| {
                (
                    "INSPECT".into(),
                    pal.frame,
                    vec![Line::from(Span::styled("sold", pal.dim_style()))],
                )
            });
        panel(frame, inspect_rect, pal, &title, color, lines);
        let owned = Rect {
            x: inspect_rect.x + inspect_rect.width + 1,
            y: lower_y,
            width: area.width.saturating_sub(inspect_w + 3),
            height: lower_h,
        };
        let mut lines = vec![Line::from(Span::styled(
            format!(
                "YOUR TOOLS {}/5   ~/bin {}/2",
                app.campaign.tools.len(),
                app.campaign.scripts.len()
            ),
            pal.bold(pal.frame),
        ))];
        for (i, id) in app.campaign.tools.iter().enumerate() {
            let mark = if app.sell && app.sell_i == i {
                "► "
            } else {
                "  "
            };
            lines.push(Line::from(Span::styled(
                format!("{mark}{}", tool(*id).name),
                pal.fg(rarity_color(pal, tool(*id).rarity, tool(*id).arch)),
            )));
        }
        if app.campaign.tools.is_empty() {
            lines.push(Line::from(Span::styled("  none yet", pal.dim_style())));
        }
        panel(frame, owned, pal, "LOADOUT", pal.frame, lines);
    }
    let cost = app
        .campaign
        .shop
        .as_ref()
        .map(|s| s.reroll_cost)
        .unwrap_or(1);
    let next = if app.campaign.breach_index == 0 {
        format!("next: L1 B2 home lan · target {B2_TARGET}")
    } else {
        format!("next: L1 B3 THE PARENT · target {B3_TARGET}")
    };
    put(
        frame,
        area.x + 1,
        area.bottom().saturating_sub(2),
        &format!(
            "${}   reroll ${cost} (+$1 each) · {next}",
            app.campaign.cash
        ),
        pal.fg(pal.text),
        area.width.saturating_sub(2),
    );
    keybar(
        frame,
        area,
        pal,
        &[
            ("1-6", "buy"),
            ("r", "reroll"),
            ("v", "sell"),
            ("tab", "inspect"),
            ("enter", "leave"),
        ],
    );
}

fn level_up(frame: &mut Frame, area: Rect, app: &mut App, pal: Pal) {
    put(
        frame,
        area.x + 2,
        area.y + 1,
        "THE PARENT is down. pick one free level-up.",
        pal.bold(pal.amber),
        area.width.saturating_sub(4),
    );
    let choices = app.campaign.level_choices.clone();
    for (i, kind) in choices.iter().enumerate() {
        let rect = Rect {
            x: area.x + 2 + i as u16 * 28,
            y: area.y + 3,
            width: 26,
            height: min(16, area.height.saturating_sub(8)),
        };
        level_card(frame, rect, pal, *kind, &app.campaign, app.level_i == i);
        let have = app.campaign.levels.get(*kind);
        put(
            frame,
            rect.x + 2,
            rect.y + rect.height.saturating_sub(3),
            &kind.next_text(have),
            pal.fg(pal.text),
            rect.width.saturating_sub(4),
        );
        app.hits.push(Hit {
            x: rect.x,
            y: rect.y,
            w: rect.width,
            h: rect.height,
            kind: HitKind::Level(i),
        });
    }
    keybar(
        frame,
        area,
        pal,
        &[("1-3", "take"), ("j/k", "move"), ("enter", "take")],
    );
}

fn cleared(frame: &mut Frame, area: Rect, app: &App, pal: Pal) {
    put(
        frame,
        area.x + 2,
        area.y + 1,
        "LAYER CLEARED",
        pal.bold(pal.green),
        20,
    );
    put(
        frame,
        area.x + 2,
        area.y + 2,
        "L1 home lan is quiet. the rest of the net is still dark in this build.",
        pal.fg(pal.text),
        area.width.saturating_sub(4),
    );
    let names = ["practice", "B2 home lan", "B3 THE PARENT"];
    for (i, score) in app.campaign.scores.iter().enumerate() {
        let name = names.get(i).copied().unwrap_or("breach");
        put(
            frame,
            area.x + 2,
            area.y + 4 + i as u16,
            &format!("{name:<18} {score}"),
            pal.fg(pal.amber),
            40,
        );
    }
    put(
        frame,
        area.x + 2,
        area.y + 8,
        &format!(
            "cash ${}   traced {}",
            app.campaign.cash, app.campaign.traced
        ),
        pal.fg(pal.text),
        40,
    );
    let tools: Vec<_> = app.campaign.tools.iter().map(|id| tool(*id).name).collect();
    put(
        frame,
        area.x + 2,
        area.y + 9,
        &format!(
            "tools  {}",
            if tools.is_empty() {
                "none".into()
            } else {
                tools.join(" · ")
            }
        ),
        pal.fg(pal.uncommon),
        area.width.saturating_sub(4),
    );
    keybar(frame, area, pal, &[("enter", "title"), ("q", "title")]);
}

fn over(frame: &mut Frame, area: Rect, app: &App, pal: Pal) {
    put(
        frame,
        area.x + 2,
        area.y + 1,
        "RUN OVER",
        pal.bold(pal.danger),
        16,
    );
    put(
        frame,
        area.x + 2,
        area.y + 3,
        &app.campaign.over_line,
        pal.fg(pal.amber),
        area.width.saturating_sub(4),
    );
    if let Some(p) = &app.campaign.payout {
        put(
            frame,
            area.x + 2,
            area.y + 5,
            &format!(
                "{} bits × {} amp = {}   kept {}",
                p.bits,
                fmt_amp(p.amp_milli),
                p.score,
                p.kept
            ),
            pal.fg(pal.text),
            area.width.saturating_sub(4),
        );
    }
    put(
        frame,
        area.x + 2,
        area.y + 7,
        "a miss ends the run. the marker showed this coming.",
        pal.dim_style(),
        area.width.saturating_sub(4),
    );
    keybar(
        frame,
        area,
        pal,
        &[("n", "new run"), ("r", "retry seed"), ("q", "title")],
    );
}

fn payout(frame: &mut Frame, area: Rect, app: &App, pal: Pal) {
    let Some(p) = &app.campaign.payout else {
        return;
    };
    let w = min(72, area.width.saturating_sub(8));
    let h = min(14, area.height.saturating_sub(6));
    let rect = Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    };
    let color = if p.traced { pal.danger } else { pal.green };
    let title = if p.traced { "TRACED" } else { "JACK OUT" };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(pal.fg(color))
        .title(Span::styled(format!(" {title} "), pal.bold(color)))
        .style(pal.fg(pal.text));
    frame.render_widget(Block::default().style(pal.fg(pal.bg)), rect);
    let target = p
        .target
        .map(|t| format!("{t}"))
        .unwrap_or_else(|| "—".into());
    let verdict = if p.cleared { "CLEARED" } else { "MISSED" };
    let lines = vec![
        Line::from(Span::styled(p.name.clone(), pal.bold(pal.amber))),
        Line::from(Span::styled(
            format!(
                "{} bits × {} amp = {}",
                p.bits,
                fmt_amp(p.amp_milli),
                p.score
            ),
            pal.fg(pal.text),
        )),
        Line::from(Span::styled(
            format!("kept {}   target {target}   {verdict}", p.kept),
            pal.bold(if p.cleared { pal.green } else { pal.danger }),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("breach +${}", p.jack_cash),
            pal.fg(pal.text),
        )),
        Line::from(Span::styled(
            format!("cred +${}  (already banked)", p.cred),
            pal.dim_style(),
        )),
        Line::from(Span::styled(
            format!(
                "interest +${}   cash now ${}",
                p.interest, app.campaign.cash
            ),
            pal.fg(pal.amber),
        )),
        Line::from(""),
        Line::from(Span::styled("enter continues", pal.dim_style())),
    ];
    frame.render_widget(Paragraph::new(lines).block(block), rect);
}

fn mentor(frame: &mut Frame, area: Rect, app: &App, pal: Pal) {
    let w = min(78, area.width.saturating_sub(8));
    let h = min(12, area.height.saturating_sub(10));
    let rect = Rect {
        x: area.x + 3,
        y: area.bottom().saturating_sub(h + 2),
        width: w,
        height: h,
    };
    let lines = mentor_lines(app.tut_step, app.bounced);
    let text: Vec<Line> = lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            let style = if i == 0 {
                pal.bold(pal.mentor)
            } else if i == 1 {
                pal.bold(pal.amber)
            } else {
                pal.fg(pal.text)
            };
            Line::from(Span::styled(line, style))
        })
        .collect();
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(pal.fg(pal.mentor))
        .style(Style::default().bg(pal.bg).fg(pal.text));
    frame.render_widget(
        Paragraph::new(text).block(block).wrap(Wrap { trim: false }),
        rect,
    );
}

fn mentor_lines(step: u8, bounced: bool) -> Vec<String> {
    match step {
        1 => vec![
            "mentor@localhost".into(),
            "NEW: moving".into(),
            "j and k move. l or enter cds in (1 cycle). h goes back up.".into(),
            "try it: cd into downloads/.".into(),
            "step 1 of 7".into(),
        ],
        2 => vec![
            "mentor@localhost".into(),
            "NEW: cycles and siphon".into(),
            "Three dots, three cycles. s takes one chunk. The file pays when the last chunk lands."
                .into(),
            "try it: siphon readme.txt until it banks.".into(),
            "step 2 of 7".into(),
        ],
        3 => vec![
            "mentor@localhost".into(),
            "NEW: THE TRACE ◆ and the ▼ marker".into(),
            "That red ◆ is the admin crawling down your proxy chain toward @you.".into(),
            "The yellow ▼ shows exactly where it will land after this turn. No surprises, ever."
                .into(),
            "If it reaches you, you are TRACED: you keep half your haul.".into(),
            "try it: press w on chat.log. watch the ▼ move one hop back.".into(),
            "words learned: cycle · siphon · trace · ▼ marker".into(),
            "step 3 of 7 · continue after you wipe".into(),
        ],
        4 => vec![
            "mentor@localhost".into(),
            "NEW: the intent deck".into(),
            "SYSADMIN NEXT is the card they play when the turn ends. 20 cards, shuffled.".into(),
            "You always see the next one. idle is +1. TRACE +2 is a surge.".into(),
            "A scan hurts only if you end the turn in that folder.".into(),
            "try it: press space. the card resolves and the next one is dealt.".into(),
            "step 4 of 7".into(),
        ],
        5 => vec![
            "mentor@localhost".into(),
            "NEW: hidden folders, locks, KEYs".into(),
            "A ▒ folder hides its files until you cd in. That costs a cycle. It is a draw.".into(),
            "[L] needs d before you can enter. A KEY banks +1 amp, and amp multiplies every bit."
                .into(),
            "try it: open games/, then decrypt .ssh/ and siphon id_rsa.".into(),
            "space ends the turn if you run out of cycles. this practice cannot trace you.".into(),
            "step 5 of 7".into(),
        ],
        6 => vec![
            "mentor@localhost".into(),
            "NEW: amp, jack out, traced".into(),
            "score = bits × amp. q banks it and costs nothing. do it while ▼ is still off @you."
                .into(),
            "The marker is red right now: ending the turn would trace you. You would keep half."
                .into(),
            "Miss a real target and the run is over. This practice has no target.".into(),
            "press enter.".into(),
            "step 6 of 7".into(),
        ],
        _ if !bounced => vec![
            "mentor@localhost".into(),
            "NEW: proxies".into(),
            "b spends 2 cycles: one more proxy, the trace one hop further, +0.5 amp past 3.".into(),
            "x spends 2: burn a proxy, the trace jumps back 2 hops.".into(),
            "try it: press b. then q to jack out and bank the haul.".into(),
            "step 7 of 7".into(),
        ],
        _ => vec![
            "mentor@localhost".into(),
            "proxy is up".into(),
            "q jacks out. the haul banks. then the shop.".into(),
            "step 7 of 7".into(),
        ],
    }
}

fn help(frame: &mut Frame, area: Rect, pal: Pal) {
    shell(frame, area, pal, "./trace · help", "esc closes");
    let inner = inset(area);
    let rows = [
        ("j / k", "move the cursor"),
        ("l / enter", "cd into the folder (1 cycle)"),
        ("h", "cd .. (1 cycle)"),
        ("s", "siphon one chunk (1, or 2 if throttled)"),
        ("d", "decrypt the locked folder (1)"),
        ("w", "wipe the LOG, trace steps back (1)"),
        ("b", "bounce: +1 proxy, +1 hop (2 cycles)"),
        ("x", "burn a proxy, trace jumps back 2 (2)"),
        ("space", "end the turn. unspent cycles are lost"),
        ("q", "jack out and bank bits × amp (free)"),
        ("! then 1-2", "run a script from ~/bin (free, one use)"),
        ("tab", "inspect the next card"),
        ("1-6 / r / v", "shop: buy, reroll, sell"),
        ("?", "this page"),
    ];
    for (i, (k, v)) in rows.iter().enumerate() {
        let y = inner.y + 1 + i as u16;
        put(
            frame,
            inner.x + 2,
            y,
            &format!("{k:<14} {v}"),
            pal.fg(pal.text),
            inner.width.saturating_sub(4),
        );
    }
    put(
        frame,
        inner.x + 2,
        inner.bottom().saturating_sub(3),
        "The trace is deterministic. ▼ is where it lands if you spend the turn as you are.",
        pal.fg(pal.amber),
        inner.width.saturating_sub(4),
    );
}

struct Zones {
    chain: Rect,
    intent: Rect,
    files: Rect,
    haul: Rect,
    cycles: Rect,
    inspect: Rect,
    tools: Rect,
    bin: Rect,
}

fn breach_layout(area: Rect, tools: bool) -> Zones {
    let tools_h = if tools {
        min(14, area.height / 3).max(8)
    } else {
        0
    };
    let key_h = 1u16;
    let top_h = min(8, area.height / 5).max(6);
    let mid_h = area
        .height
        .saturating_sub(tools_h + key_h + top_h + 1)
        .max(6);
    let intent_w = if area.width >= 110 {
        min(50, area.width / 3)
    } else {
        min(36, area.width / 3)
    };
    let left_w = area.width.saturating_sub(intent_w);
    let chain = Rect {
        x: area.x,
        y: area.y,
        width: left_w,
        height: top_h,
    };
    let intent = Rect {
        x: area.x + left_w,
        y: area.y,
        width: intent_w,
        height: top_h,
    };
    let haul_h = min(7, mid_h / 2);
    let cycle_h = 3;
    let files = Rect {
        x: area.x,
        y: area.y + top_h,
        width: left_w,
        height: mid_h,
    };
    let haul = Rect {
        x: area.x + left_w,
        y: area.y + top_h,
        width: intent_w,
        height: haul_h,
    };
    let cycles = Rect {
        x: area.x + left_w,
        y: haul.y + haul_h,
        width: intent_w,
        height: cycle_h,
    };
    let inspect = Rect {
        x: area.x + left_w,
        y: cycles.y + cycle_h,
        width: intent_w,
        height: mid_h.saturating_sub(haul_h + cycle_h),
    };
    let bin_w = if tools { min(36, area.width / 4) } else { 0 };
    let tools_rect = Rect {
        x: area.x,
        y: area.y + top_h + mid_h,
        width: area.width.saturating_sub(bin_w),
        height: tools_h,
    };
    let bin = Rect {
        x: area.x + tools_rect.width,
        y: tools_rect.y,
        width: bin_w,
        height: tools_h,
    };
    Zones {
        chain,
        intent,
        files,
        haul,
        cycles,
        inspect,
        tools: tools_rect,
        bin,
    }
}

#[allow(clippy::too_many_arguments)]
fn tool_card(
    frame: &mut Frame,
    rect: Rect,
    pal: Pal,
    name: &str,
    art: &[&str],
    rarity: Rarity,
    arch: Arch,
    price: Option<u32>,
    hover: bool,
    selected: bool,
) {
    let color = rarity_color(pal, rarity, arch);
    let border = if hover || selected { pal.hover } else { color };
    let lift = if hover && price.is_none() { 1 } else { 0 };
    let rect = Rect {
        x: rect.x,
        y: rect.y.saturating_sub(lift),
        width: rect.width,
        height: rect.height,
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(if rarity == Rarity::Rare || selected {
            BorderType::Double
        } else {
            BorderType::Rounded
        })
        .border_style(pal.fg(border));
    frame.render_widget(block, rect);
    if hover {
        put(
            frame,
            rect.x + 1,
            rect.y,
            " HOVER ",
            Style::default()
                .fg(pal.bg)
                .bg(pal.hover)
                .add_modifier(Modifier::BOLD),
            8,
        );
    }
    put(
        frame,
        rect.x + 1,
        rect.y + 1,
        &center(name, rect.width.saturating_sub(2) as usize),
        pal.bold(color),
        rect.width.saturating_sub(2),
    );
    let art_top = rect.y + 2;
    for (i, line) in art.iter().enumerate() {
        let y = art_top + i as u16;
        if y + 2 >= rect.y + rect.height {
            break;
        }
        put(
            frame,
            rect.x + 1,
            y,
            &center(line, rect.width.saturating_sub(2) as usize),
            pal.bold(color),
            rect.width.saturating_sub(2),
        );
    }
    let foot = rarity.mark();
    let y = rect.y + rect.height.saturating_sub(2);
    put(frame, rect.x + 1, y, foot, pal.fg(color), 8);
    if let Some(price) = price {
        let tag = format!("${price}");
        let x = rect.x + rect.width.saturating_sub(tag.len() as u16 + 2);
        put(frame, x, y, &tag, pal.bold(pal.amber), 6);
    }
    if selected {
        put(
            frame,
            rect.x + 1,
            rect.y + rect.height.saturating_sub(1),
            " ▲ tab ",
            Style::default().fg(pal.bg).bg(pal.amber),
            8,
        );
    }
}

fn level_card(
    frame: &mut Frame,
    rect: Rect,
    pal: Pal,
    kind: LevelKind,
    campaign: &trace_core::Campaign,
    selected: bool,
) {
    let have = campaign.levels.get(kind);
    tool_card(
        frame,
        rect,
        pal,
        "LEVEL-UP",
        LEVEL,
        Rarity::Uncommon,
        Arch::Cycle,
        campaign.levels.cost(kind),
        false,
        selected,
    );
    put(
        frame,
        rect.x + 1,
        rect.y + rect.height.saturating_sub(4),
        &center(
            &format!("{} lv{have}→{}", kind.name(), have + 1),
            rect.width.saturating_sub(2) as usize,
        ),
        pal.bold(pal.amber),
        rect.width.saturating_sub(2),
    );
}

fn script_card(frame: &mut Frame, rect: Rect, pal: Pal, name: &str, price: u32, selected: bool) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(pal.fg(pal.green))
        .border_type(if selected {
            BorderType::Double
        } else {
            BorderType::Rounded
        });
    frame.render_widget(block, rect);
    put(
        frame,
        rect.x + 1,
        rect.y + 1,
        "#!/bin/sh",
        Style::default()
            .fg(pal.bg)
            .bg(pal.green)
            .add_modifier(Modifier::BOLD),
        rect.width.saturating_sub(2),
    );
    for (i, line) in SCRIPT.iter().enumerate() {
        put(
            frame,
            rect.x + 1,
            rect.y + 3 + i as u16,
            &center(line, rect.width.saturating_sub(2) as usize),
            pal.bold(pal.green),
            rect.width.saturating_sub(2),
        );
    }
    put(
        frame,
        rect.x + 1,
        rect.y + rect.height.saturating_sub(3),
        &center(name, rect.width.saturating_sub(2) as usize),
        pal.bold(pal.green),
        rect.width.saturating_sub(2),
    );
    put(
        frame,
        rect.x + 1,
        rect.y + rect.height.saturating_sub(2),
        "1-USE",
        pal.fg(pal.green),
        6,
    );
    let tag = format!("${price}");
    put(
        frame,
        rect.x + rect.width.saturating_sub(tag.len() as u16 + 2),
        rect.y + rect.height.saturating_sub(2),
        &tag,
        pal.bold(pal.amber),
        6,
    );
}

fn empty_card(frame: &mut Frame, rect: Rect, pal: Pal, label: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(pal.fg(pal.dim));
    frame.render_widget(block, rect);
    put(
        frame,
        rect.x + 1,
        rect.y + rect.height / 2,
        &center(label, rect.width.saturating_sub(2) as usize),
        pal.dim_style(),
        rect.width.saturating_sub(2),
    );
}

fn panel(
    frame: &mut Frame,
    area: Rect,
    pal: Pal,
    title: &str,
    color: ratatui::style::Color,
    lines: Vec<Line>,
) {
    if area.width < 3 || area.height < 2 {
        return;
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color))
        .title(Span::styled(format!(" {title} "), pal.bold(color)));
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

fn keybar(frame: &mut Frame, area: Rect, pal: Pal, items: &[(&str, &str)]) {
    let y = area.y + area.height.saturating_sub(1);
    let mut x = area.x + 1;
    for (k, v) in items {
        if x + 6 >= area.x + area.width {
            break;
        }
        let key = format!(" {k} ");
        put(
            frame,
            x,
            y,
            &key,
            Style::default()
                .fg(pal.bg)
                .bg(pal.amber)
                .add_modifier(Modifier::BOLD),
            key.len() as u16,
        );
        x += key.len() as u16;
        let label = format!(" {v} ");
        put(frame, x, y, &label, pal.dim_style(), label.len() as u16 + 1);
        x += label.len() as u16 + 1;
    }
}

fn shell(frame: &mut Frame, area: Rect, pal: Pal, left: &str, right: &str) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(pal.fg(pal.frame))
        .title(Span::styled(format!(" {left} "), pal.bold(pal.green)));
    frame.render_widget(block, area);
    let right = format!(" {right} ");
    let x = area.x + area.width.saturating_sub(right.len() as u16 + 2);
    put(
        frame,
        x,
        area.y,
        &right,
        pal.bold(pal.amber),
        right.len() as u16,
    );
}

fn polish(frame: &mut Frame, app: &App, pal: Pal) {
    let buf = frame.buffer_mut();
    let area = buf.area;
    if app.config.scanlines {
        for y in (area.y..area.y + area.height).step_by(2) {
            for x in area.x..area.x + area.width {
                if let Some(cell) = buf.cell_mut((x, y)) {
                    if cell.bg == pal.bg || cell.style().bg == Some(pal.bg) {
                        cell.set_bg(ColorMix::dim(pal.bg));
                    }
                }
            }
        }
    }
    if app.tear > 0 {
        let t = app.elapsed as u16;
        for y in area.y..area.y + area.height {
            for x in area.x..area.x + area.width {
                let n = x
                    .wrapping_mul(13)
                    .wrapping_add(y.wrapping_mul(7))
                    .wrapping_add(t);
                if n % 11 == 0 {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        let ch = ['▒', '▓', '░', '/'][(n as usize) % 4];
                        cell.set_symbol(&ch.to_string());
                        cell.set_fg(pal.danger);
                    }
                }
            }
        }
    }
}

struct ColorMix;
impl ColorMix {
    fn dim(color: ratatui::style::Color) -> ratatui::style::Color {
        match color {
            ratatui::style::Color::Rgb(r, g, b) => ratatui::style::Color::Rgb(r / 5, g / 3, b / 5),
            _ => ratatui::style::Color::Rgb(0, 8, 4),
        }
    }
}

fn inset(area: Rect) -> Rect {
    Rect {
        x: area.x + 1,
        y: area.y + 1,
        width: area.width.saturating_sub(2),
        height: area.height.saturating_sub(2),
    }
}

fn put(frame: &mut Frame, x: u16, y: u16, text: &str, style: Style, max_w: u16) {
    let buf = frame.buffer_mut();
    let mut col = 0u16;
    for ch in text.chars() {
        if col >= max_w {
            break;
        }
        let width = UnicodeWidthStr::width(ch.encode_utf8(&mut [0; 4])) as u16;
        if x + col + width > buf.area.x + buf.area.width || y >= buf.area.y + buf.area.height {
            break;
        }
        if let Some(cell) = buf.cell_mut((x + col, y)) {
            cell.set_symbol(&ch.to_string());
            cell.set_style(style);
        }
        col += width.max(1);
    }
}

fn center(text: &str, width: usize) -> String {
    let w = UnicodeWidthStr::width(text);
    if w >= width {
        return text.chars().take(width).collect();
    }
    let left = (width - w) / 2;
    format!("{}{text}", " ".repeat(left))
}

fn truncate(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        text.to_string()
    } else {
        text.chars()
            .take(width.saturating_sub(1))
            .collect::<String>()
            + "…"
    }
}

fn tone_color(pal: Pal, tone: trace_core::Tone) -> ratatui::style::Color {
    use trace_core::Tone;
    match tone {
        Tone::Data => pal.data,
        Tone::Key => pal.key,
        Tone::Cred => pal.cred,
        Tone::Log => pal.log,
        Tone::Zero => pal.rare,
        Tone::Hidden => pal.log,
        Tone::Locked => pal.danger,
        Tone::Looted => pal.safe,
        Tone::Dim => pal.dim,
        Tone::Plain => pal.text,
    }
}

fn rarity_color(pal: Pal, rarity: Rarity, arch: Arch) -> ratatui::style::Color {
    if arch == Arch::Chaos {
        return pal.cred;
    }
    match rarity {
        Rarity::Common => pal.common,
        Rarity::Uncommon => pal.uncommon,
        Rarity::Rare => pal.rare,
    }
}

impl Pal {
    fn dim_style(self) -> Style {
        self.fg(self.dim)
    }
}

pub struct Shot {
    pub width: u16,
    pub height: u16,
    pub cells: Vec<ShotCell>,
}

pub struct ShotCell {
    pub ch: String,
    pub fg: (u8, u8, u8),
    pub bg: (u8, u8, u8),
}

pub fn capture(app: &mut App, w: u16, h: u16) -> Shot {
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use ratatui::Terminal;
    let backend = TestBackend::new(w, h);
    let mut term = Terminal::new(backend).expect("terminal");
    term.draw(|f| draw(f, app)).expect("draw");
    let buf = term.backend().buffer().clone();
    let mut cells = Vec::with_capacity((w as usize) * (h as usize));
    for y in 0..h {
        for x in 0..w {
            let cell = &buf[(x, y)];
            let style = cell.style();
            cells.push(ShotCell {
                ch: cell.symbol().to_string(),
                fg: rgb(style.fg.unwrap_or(Color::Rgb(180, 200, 180))),
                bg: rgb(style.bg.unwrap_or(Color::Rgb(0, 0, 0))),
            });
        }
    }
    Shot {
        width: w,
        height: h,
        cells,
    }
}

fn rgb(color: ratatui::style::Color) -> (u8, u8, u8) {
    use ratatui::style::Color;
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black | Color::Reset => (0, 0, 0),
        Color::White => (230, 235, 230),
        Color::Gray => (160, 170, 160),
        Color::DarkGray => (70, 80, 70),
        Color::Red => (220, 60, 50),
        Color::Green => (80, 220, 110),
        Color::Yellow => (230, 190, 70),
        Color::Blue => (70, 130, 255),
        Color::Magenta => (210, 80, 200),
        Color::Cyan => (70, 220, 220),
        Color::LightRed => (255, 120, 100),
        Color::LightGreen => (140, 255, 160),
        Color::LightYellow => (255, 230, 120),
        Color::LightBlue => (140, 180, 255),
        Color::LightMagenta => (240, 140, 230),
        Color::LightCyan => (140, 245, 240),
        Color::Indexed(i) => (i, i, i),
    }
}

pub fn render_plain(app: &mut App, w: u16, h: u16) -> String {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    let backend = TestBackend::new(w, h);
    let mut term = Terminal::new(backend).expect("terminal");
    term.draw(|f| draw(f, app)).expect("draw");
    let buf = term.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..h {
        for x in 0..w {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}
