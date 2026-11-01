//! Rendered screens and a full run driven from the keyboard.

use std::fs;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use mastervibes_core::Phase;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Flow, Screen};
use crate::art::art_lines;
use crate::ui::render_screen;

fn press(app: &mut App, ch: char) -> Flow {
    app.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE))
}

fn enter(app: &mut App) -> Flow {
    app.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

fn step(app: &mut App) {
    match app.screen {
        Screen::Title => {
            enter(app);
        }
        Screen::Board => {
            if app.dead_confirm {
                press(app, 'y');
            } else if app.draft_complete() {
                enter(app);
            } else {
                press(app, '0');
            }
        }
        Screen::Scoring => app.tick(10_000),
        Screen::Payout => {
            enter(app);
        }
        Screen::Shop => {
            press(app, 'n');
        }
        Screen::GameOver => {}
    }
}

fn drive(app: &mut App) {
    for _ in 0..800 {
        if app.screen == Screen::GameOver {
            return;
        }
        let before = format!("{:?} {}", app.screen, app.status);
        step(app);
        let after = format!("{:?} {} {}", app.screen, app.status, app.draft_complete());
        if app.screen == Screen::Board && before == after && !app.dead_confirm {
            // A locked slot can swallow a '0'. Nudge the cursor and try again.
            let _ = app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE));
        }
    }
    panic!("run did not finish on {:?} ({})", app.screen, app.status);
}

fn play_until(app: &mut App, screen: Screen) {
    for _ in 0..800 {
        if app.screen == screen {
            return;
        }
        step(app);
    }
    panic!(
        "never reached {screen:?} (stuck on {:?}: {})",
        app.screen, app.status
    );
}

fn assert_width(text: &str, width: usize) {
    for (index, line) in text.lines().enumerate() {
        assert_eq!(
            UnicodeWidthStr::width(line),
            width,
            "line {index} is not {width} columns:\n{line}"
        );
    }
}

fn assert_terms(text: &str) {
    let lower = text.to_ascii_lowercase();
    for word in [
        "balatro",
        "joker",
        "blind",
        "ante",
        "poker",
        "stake",
        "consumable",
        "chips",
        "chip",
        "credits",
        "mult",
    ] {
        let padded = format!(" {word} ");
        assert!(
            !lower.contains(&padded) && !lower.contains(&format!("[{word}]")),
            "banned term {word} in:\n{text}"
        );
    }
}

fn save(name: &str, text: &str) {
    let dir = PathBuf::from("/tmp/mv-screens");
    let _ = fs::create_dir_all(&dir);
    let _ = fs::write(dir.join(name), text);
}

#[test]
fn title_fits_80x24_and_explains_the_seed() {
    let app = App::new();
    let text = render_screen(&app, 80, 24);
    assert_width(&text, 80);
    assert_terms(&text);
    assert!(text.contains("NEW RUN"), "{text}");
    assert!(text.contains("ENTER SEED"), "{text}");
    assert!(text.contains("MV-7F3A-9C21-L4"), "{text}");
    assert!(text.contains("[q]"), "{text}");
    assert!(text.contains("[?]"), "{text}");
    save("01_title.txt", &text);
}

#[test]
fn help_covers_pegs_signatures_and_scoring() {
    let mut app = App::new();
    press(&mut app, '?');
    let text = render_screen(&app, 80, 24);
    assert_width(&text, 80);
    assert_terms(&text);
    assert!(text.contains("hit"), "{text}");
    assert!(text.contains("near"), "{text}");
    assert!(
        text.contains("signature") || text.contains("SIGNATURE"),
        "{text}"
    );
    assert!(text.contains("bits"), "{text}");
    assert!(text.contains("amp"), "{text}");
    assert!(text.contains("scores 0"), "{text}");
    save("02_help.txt", &text);
}

#[test]
fn quit_asks_before_leaving() {
    let mut app = App::new();
    press(&mut app, 'q');
    let text = render_screen(&app, 80, 24);
    assert!(text.contains("quit mastervibes"), "{text}");
    assert_eq!(press(&mut app, 'n'), Flow::Continue);
    assert!(!app.quit_confirm);
    press(&mut app, 'q');
    assert_eq!(press(&mut app, 'y'), Flow::Quit);
}

#[test]
fn board_previews_the_signature_before_pegs() {
    let mut app = App::new();
    app.start_key(0x7F3A_9C21);
    for ch in ['0', '0', '1', '2'] {
        press(&mut app, ch);
    }
    let text = render_screen(&app, 80, 24);
    assert_width(&text, 80);
    assert_terms(&text);
    assert!(
        text.contains("PERIMETER") || text.contains("Perimeter"),
        "{text}"
    );
    assert!(text.contains("Pair"), "{text}");
    assert!(text.contains("before pegs"), "{text}");
    assert!(text.contains("probes left"), "{text}");
    assert!(text.contains("$4"), "{text}");
    assert!(text.contains("MV-7F3A-9C21-L1"), "{text}");
    assert!(text.contains("[q]"), "{text}");
    save("03_board.txt", &text);

    enter(&mut app);
    app.tick(220);
    assert_eq!(app.screen, Screen::Scoring);
    let scoring = render_screen(&app, 80, 24);
    assert_width(&scoring, 80);
    assert_terms(&scoring);
    assert!(scoring.contains("bits"), "{scoring}");
    assert!(scoring.contains("amp"), "{scoring}");
    assert!(
        scoring.contains('*') || scoring.contains("hit") || scoring.contains("no pegs"),
        "{scoring}"
    );
    save("04_scoring.txt", &scoring);
}

#[test]
fn shop_cards_use_the_tool_art() {
    let mut app = App::new();
    app.start_key(0x7F3A_9C21);
    play_until(&mut app, Screen::Shop);
    let text = render_screen(&app, 80, 24);
    assert_width(&text, 80);
    assert_terms(&text);
    assert!(text.contains("reroll"), "{text}");
    assert!(text.contains("[1-2]"), "{text}");
    let state = app.state().expect("run");
    let shop = state.shop.expect("shop");
    assert_eq!(shop.offers.len(), 2);
    for offer in &shop.offers {
        assert!(text.contains(&offer.name), "{}\n{text}", offer.name);
        let art = art_lines(&offer.name).expect("art");
        assert!(
            text.contains(art[0].trim())
                || text.contains(art[1].trim())
                || text.contains(art[2].trim()),
            "art for {} missing\n{text}",
            offer.name
        );
    }
    save("05_shop.txt", &text);

    // Payout was the screen before the shop. Rebuild it by finishing the next
    // breach's probes only after grabbing a payout render from a fresh run.
}

#[test]
fn payout_and_game_over_show_seed_and_stats() {
    let mut app = App::new();
    app.start_key(0x7F3A_9C21);
    play_until(&mut app, Screen::Payout);
    let payout = render_screen(&app, 80, 24);
    assert_width(&payout, 80);
    assert_terms(&payout);
    assert!(
        payout.contains("BREACH") || payout.contains("CRACKED") || payout.contains("NOT CRACKED"),
        "{payout}"
    );
    assert!(payout.contains('$'), "{payout}");
    assert!(
        payout.contains("interest") || payout.contains("under par") || payout.contains("breach"),
        "{payout}"
    );
    save("06_payout.txt", &payout);

    drive(&mut app);
    assert_eq!(app.screen, Screen::GameOver);
    let state = app.state().expect("run");
    assert!(matches!(state.phase, Phase::Won | Phase::Lost));
    let text = render_screen(&app, 120, 36);
    assert_width(&text, 120);
    assert_terms(&text);
    assert!(text.contains("MV-7F3A-9C21-L1"), "{text}");
    assert!(text.contains("probes"), "{text}");
    assert!(text.contains("best probe"), "{text}");
    assert!(text.contains("[r]"), "{text}");
    save("07_game_over.txt", &text);

    let wide = {
        let mut again = App::new();
        again.start_key(0x7F3A_9C21);
        for ch in ['0', '1', '2', '3'] {
            press(&mut again, ch);
        }
        render_screen(&again, 120, 40)
    };
    assert_width(&wide, 120);
    assert!(wide.contains("TOOLS"), "{wide}");
    save("08_board_wide.txt", &wide);
}

#[test]
fn keyboard_plays_a_full_run() {
    let mut app = App::new();
    app.start_key(0);
    drive(&mut app);
    assert_eq!(app.screen, Screen::GameOver);
    let state = app.state().expect("ended run");
    assert!(state.end.is_some());
    assert!(app.stats.probes > 0);
    let text = render_screen(&app, 80, 24);
    assert_width(&text, 80);
    assert!(text.contains("MV-0000-0000-L1"), "{text}");
    save("09_game_over_80.txt", &text);
}

#[test]
fn small_terminal_asks_for_80x24() {
    let app = App::new();
    let text = render_screen(&app, 60, 20);
    assert!(text.contains("80x24"), "{text}");
}

#[test]
fn absent_symbol_warns_before_the_probe_is_sent() {
    let mut app = App::new();
    app.start_key(0);
    // Every layer leaves at least one symbol out of the code. A mono probe of
    // that symbol comes back with no pegs, and the next one must warn.
    for digit in '0'..='5' {
        for _ in 0..2 {
            back_to_board(&mut app);
            while app.screen == Screen::Board && !app.draft_complete() {
                press(&mut app, digit);
            }
            if let Some(warning) = app.absent_warning() {
                let board = render_screen(&app, 80, 24);
                assert!(
                    board.contains("scores 0") || board.contains("honeypot"),
                    "{warning}\n{board}"
                );
                assert_grey_pool(&app);
                enter(&mut app);
                assert!(app.dead_confirm, "enter must ask before a wiped probe");
                let text = render_screen(&app, 80, 24);
                assert!(
                    text.contains("scores 0") || text.contains("honeypot"),
                    "{text}"
                );
                assert_terms(&text);
                save("10_absent.txt", &text);
                press(&mut app, 'n');
                assert!(!app.dead_confirm);
                assert_eq!(app.screen, Screen::Board);
                return;
            }
            enter(&mut app);
            app.tick(10_000);
        }
    }
    panic!("no absent symbol turned up");
}

fn back_to_board(app: &mut App) {
    for _ in 0..6 {
        match app.screen {
            Screen::Board => return,
            Screen::Scoring => app.tick(10_000),
            Screen::Payout => {
                enter(app);
            }
            Screen::Shop => {
                press(app, 'n');
            }
            Screen::GameOver | Screen::Title => panic!("run ended before an absent symbol"),
        }
    }
}

fn assert_grey_pool(app: &App) {
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use ratatui::Terminal;

    use crate::ui::draw;

    let backend = TestBackend::new(80, 24);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal.draw(|frame| draw(frame, app)).unwrap();
    let buf = terminal.backend().buffer().clone();
    let mut saw_dim = false;
    let mut saw_live = false;
    for y in 0..buf.area.height {
        let mut row = String::new();
        for x in 0..buf.area.width {
            row.push_str(buf[(x, y)].symbol());
        }
        let Some(at) = row.find("pool ") else {
            continue;
        };
        let mut col = UnicodeWidthStr::width(&row[..at]) + UnicodeWidthStr::width("pool ");
        for ch in row[at + "pool ".len()..].chars() {
            if ch.is_ascii_digit() {
                let fg = buf[(u16::try_from(col).unwrap_or(0), y)].fg;
                if fg == Color::DarkGray {
                    saw_dim = true;
                }
                if fg == Color::Green {
                    saw_live = true;
                }
            }
            col += UnicodeWidthStr::width(ch.encode_utf8(&mut [0; 4]));
        }
    }
    assert!(saw_dim, "absent symbols in the pool should be grey");
    assert!(saw_live, "live symbols in the pool should stay green");
}
