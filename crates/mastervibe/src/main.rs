//! Playable terminal for mastervibe.
//!
//! The rules stay in `mastervibe_core`. This binary draws the run and sends
//! keypresses back through `Run`.

use std::io::{self, stdout, IsTerminal};
use std::time::{Duration, Instant};

use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use mastervibe::{draw, App, Flow};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

fn main() -> io::Result<()> {
    if !stdout().is_terminal() {
        eprintln!("mastervibe needs a terminal.");
        std::process::exit(1);
    }
    let mut app = App::new();
    run(&mut app)
}

fn run(app: &mut App) -> io::Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, Hide)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(out))?;
    let hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore();
        hook(info);
    }));

    let result = event_loop(&mut terminal, app);
    restore()?;
    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    let mut last = Instant::now();
    loop {
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(last).as_millis();
        last = now;
        let millis = u64::try_from(elapsed).unwrap_or(u64::MAX);
        app.tick(millis);

        terminal.draw(|frame| draw(frame, app))?;

        if event::poll(Duration::from_millis(30))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Release && app.handle_key(key) == Flow::Quit {
                    break;
                }
            }
        }
    }
    Ok(())
}

fn restore() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen, Show)?;
    Ok(())
}
