//! ./trace — a terminal roguelike about a trace walking your proxy chain.

use std::io::{self, stdout, IsTerminal};
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::event::DisableMouseCapture;
use crossterm::event::EnableMouseCapture;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use trace::app::{config_dir, App, Flow};
use trace::audio::Audio;
use trace::ui::draw;

fn main() -> io::Result<()> {
    let mut args = std::env::args().skip(1);
    if let Some(flag) = args.next() {
        match flag.as_str() {
            "--help" | "-h" => {
                println!("./trace\n\n  cargo run --release\n  trace --seed TRAC-7F3A-C0DE\n  trace --sim");
                return Ok(());
            }
            "--sim" => {
                sim();
                return Ok(());
            }
            "--dump" => {
                let dir = args.next().unwrap_or_else(|| "shots".into());
                dump_shots(&dir);
                return Ok(());
            }
            "--seed" => {
                let seed = args.next().and_then(|s| trace_core::parse_seed(&s));
                if !stdout().is_terminal() {
                    eprintln!("trace needs a terminal.");
                    std::process::exit(1);
                }
                let mut app = App::boot(config_dir(), Audio::start());
                if let Some(seed) = seed {
                    app.pending_seed = seed;
                }
                app.screen = trace::app::Screen::NewRun;
                return run(app);
            }
            other => {
                eprintln!("unknown flag {other}. try --help");
                std::process::exit(1);
            }
        }
    }
    if !stdout().is_terminal() {
        eprintln!("trace needs a terminal. try: cargo run --release");
        std::process::exit(1);
    }
    run(App::boot(config_dir(), Audio::start()))
}

fn dump_shots(dir: &str) {
    use std::fs;
    use trace::app::Screen;
    use trace::ui::capture;
    use trace::ui::theme::PaletteId;
    use trace_core::{LevelKind, Offer, Phase, ScriptId, ToolId};

    let root = std::path::Path::new(dir);
    let _ = fs::create_dir_all(root);
    let home = std::env::temp_dir().join("trace-dump");
    let _ = fs::create_dir_all(&home);

    let mut title = App::fresh(home.clone(), Audio::silent());
    title.screen = Screen::Title;
    write_shot(&root.join("title.json"), &capture(&mut title, 154, 44));

    let mut options = App::fresh(home.clone(), Audio::silent());
    options.screen = Screen::Options;
    options.config.palette = PaletteId::Deut;
    write_shot(&root.join("options.json"), &capture(&mut options, 154, 44));

    let mut tutorial = App::fresh(home.clone(), Audio::silent());
    tutorial.pending_seed = 42;
    tutorial.screen = Screen::Title;
    tutorial.menu = 1;
    tutorial.press_code(crossterm::event::KeyCode::Enter);
    tutorial.press_code(crossterm::event::KeyCode::Enter);
    for key in ['l', 's', 's'] {
        tutorial.press(key);
    }
    write_shot(
        &root.join("tutorial.json"),
        &capture(&mut tutorial, 154, 44),
    );

    let mut breach = App::fresh(home.clone(), Audio::silent());
    breach.campaign.tools = vec![
        ToolId::FatPipe,
        ToolId::Keyring,
        ToolId::DeadDrop,
        ToolId::TorPlus,
        ToolId::RootShell,
    ];
    breach.campaign.scripts = vec![ScriptId::ZeroDay, ScriptId::Strace];
    breach.campaign.skip_tutorial();
    breach.campaign.leave_shop();
    breach.campaign.cash = 7;
    breach.campaign.scripts = vec![ScriptId::ZeroDay, ScriptId::Strace];
    breach.screen = Screen::Play;
    breach.tut_on = false;
    breach.hover_tool = Some(2);
    breach.pinned = false;
    breach.config.hover = true;
    write_shot(&root.join("breach.json"), &capture(&mut breach, 154, 44));

    let mut shop = App::fresh(home.clone(), Audio::silent());
    shop.campaign.tools = vec![ToolId::FatPipe, ToolId::Keyring, ToolId::DeadDrop];
    shop.campaign.scripts = vec![ScriptId::Rm];
    shop.campaign.skip_tutorial();
    shop.campaign.cash = 11;
    if let Some(market) = shop.campaign.shop.as_mut() {
        market.offers = vec![
            Some(Offer::Tool(ToolId::Compressor)),
            Some(Offer::Tool(ToolId::Keychain)),
            Some(Offer::Tool(ToolId::Burner)),
            Some(Offer::Tool(ToolId::Overclock)),
            Some(Offer::Level(LevelKind::Siphon)),
            Some(Offer::Script(ScriptId::Grep)),
        ];
        market.lean = "your build leans data".into();
    }
    shop.screen = Screen::Play;
    shop.shop_i = 1;
    shop.hover_offer = Some(1);
    write_shot(&root.join("shop.json"), &capture(&mut shop, 154, 44));

    let mut cleared = App::fresh(home, Audio::silent());
    cleared.campaign.phase = Phase::Cleared;
    cleared.campaign.breach = None;
    cleared.campaign.scores = vec![48, 186, 274];
    cleared.campaign.cash = 19;
    cleared.campaign.tools = vec![ToolId::Keyring, ToolId::FatPipe, ToolId::DeadDrop];
    cleared.screen = Screen::Play;
    write_shot(&root.join("cleared.json"), &capture(&mut cleared, 154, 44));
    println!("wrote shots to {dir}");
}

fn write_shot(path: &std::path::Path, shot: &trace::ui::Shot) {
    let file = std::fs::File::create(path).expect("shot");
    serde_json::to_writer(
        file,
        &serde_json::json!({
            "w": shot.width,
            "h": shot.height,
            "cells": shot.cells.iter().map(|c| serde_json::json!({
                "c": c.ch,
                "fg": [c.fg.0, c.fg.1, c.fg.2],
                "bg": [c.bg.0, c.bg.1, c.bg.2],
            })).collect::<Vec<_>>(),
        }),
    )
    .expect("json");
}

fn sim() {
    use trace_core::{simulate, Policy};
    let mut wins = 0;
    let mut greedy_traced = 0;
    let n = 24u32;
    for seed in 1..=n {
        if simulate(u64::from(seed), Policy::Sensible).cleared {
            wins += 1;
        }
        if simulate(u64::from(seed), Policy::Greedy).traced > 0 {
            greedy_traced += 1;
        }
    }
    println!("sensible clears {wins}/{n}");
    println!("greedy traced {greedy_traced}/{n}");
}

fn run(mut app: App) -> io::Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, EnableMouseCapture, Hide)?;
    let mut terminal = ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(out))?;
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore();
        hook(info);
    }));
    let result = loop {
        let start = Instant::now();
        terminal.draw(|frame| draw(frame, &mut app))?;
        if event::poll(Duration::from_millis(30))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind != KeyEventKind::Release && app.handle_key(key) == Flow::Quit {
                        break Ok(());
                    }
                }
                Event::Mouse(mouse) => app.handle_mouse(mouse),
                _ => {}
            }
        }
        if app.quit {
            break Ok(());
        }
        let spent = start.elapsed().as_millis() as u64;
        app.tick(spent.max(16));
    };
    restore()?;
    result
}

fn restore() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), DisableMouseCapture, LeaveAlternateScreen, Show)?;
    Ok(())
}
