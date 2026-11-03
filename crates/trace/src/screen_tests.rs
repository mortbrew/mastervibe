use crossterm::event::KeyCode;
use trace_core::{simulate, tool, Act, Phase, Policy, ToolId};

use crate::app::{App, Screen};
use crate::audio::Audio;
use crate::ui::render_plain;
use crate::ui::theme::PaletteId;

fn app() -> App {
    let home =
        std::env::temp_dir().join(format!("trace-test-{}-{}", std::process::id(), rand_tag()));
    let _ = std::fs::create_dir_all(&home);
    let mut app = App::fresh(home, Audio::silent());
    app.pending_seed = 42;
    app
}

fn rand_tag() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1)
}

fn text(app: &mut App, w: u16, h: u16) -> String {
    render_plain(app, w, h)
}

#[test]
fn title_continue_is_grey_without_a_save() {
    let mut app = app();
    app.screen = Screen::Title;
    let view = text(&mut app, 154, 44);
    assert!(view.contains("./trace"), "{view}");
    assert!(view.contains("NEW RUN"));
    assert!(view.contains("OPTIONS"));
    assert!(view.contains("EXIT"));
    assert!(view.contains("no save"));
    assert!(!view.to_lowercase().contains("balatro"));
    assert!(!view.to_lowercase().contains("joker"));
}

#[test]
fn options_has_colourblind_palette_and_mouse() {
    let mut app = app();
    app.screen = Screen::Options;
    app.config.palette = PaletteId::Deut;
    let view = text(&mut app, 154, 44);
    assert!(view.contains("colourblind (deut)"), "{view}");
    assert!(view.contains("mouse"));
    assert!(view.contains("music"));
    assert!(view.contains("deut"));
    let narrow = text(&mut app, 120, 40);
    assert!(narrow.contains("OPTIONS"));
}

#[test]
fn tutorial_step_three_shows_the_marker() {
    let mut app = app();
    app.screen = Screen::Title;
    app.menu = 1;
    app.press_code(KeyCode::Enter);
    app.press_code(KeyCode::Enter);
    assert_eq!(app.tut_step, 1);
    app.press('l');
    app.press('s');
    app.press('s');
    assert_eq!(app.tut_step, 3, "{}", app.status);
    let view = text(&mut app, 154, 44);
    assert!(
        view.contains("THE TRACE") || view.contains("the trace"),
        "{view}"
    );
    assert!(view.contains("chat.log"), "{view}");
    assert!(view.contains('▼'), "{view}");
    assert!(view.contains("mentor@localhost"), "{view}");
    let small = text(&mut app, 120, 40);
    assert!(small.contains("chat.log"), "{small}");
}

#[test]
fn tool_card_hides_the_effect_until_inspect() {
    let mut app = app();
    app.campaign.tools.push(ToolId::DeadDrop);
    app.campaign.skip_tutorial();
    app.campaign.leave_shop();
    app.screen = Screen::Play;
    app.tut_on = false;
    app.tool_i = 0;
    app.pinned = true;
    let view = text(&mut app, 154, 44);
    assert!(view.contains("Dead Drop"), "{view}");
    assert_eq!(
        view.matches("75%").count(),
        1,
        "effect text should live only in INSPECT\n{view}"
    );
    assert!(view.contains("PROXY CHAIN"), "{view}");
    assert!(view.contains("HAUL"), "{view}");
}

#[test]
fn tutorial_reaches_the_shop_and_l1_can_clear() {
    let mut app = app();
    app.screen = Screen::Title;
    app.menu = 1;
    app.press_code(KeyCode::Enter);
    app.press_code(KeyCode::Enter);
    let keys = ['l', 's', 's', 'w', ' ', 'l', 'h', 'j', 'd', ' ', 'l', 's'];
    for key in keys {
        app.press(key);
    }
    assert_eq!(app.tut_step, 6, "status {}", app.status);
    app.press_code(KeyCode::Enter);
    assert_eq!(app.tut_step, 7, "{}", app.status);
    app.press('b');
    assert!(app.bounced, "{}", app.status);
    app.press('q');
    assert!(app.payout_open, "{}", app.status);
    app.press_code(KeyCode::Enter);
    assert_eq!(app.campaign.phase, Phase::Shop);

    app.config.confirm_buys = false;
    buy_if_present(&mut app, ToolId::Keyring);
    buy_if_present(&mut app, ToolId::FatPipe);
    app.press_code(KeyCode::Enter);
    assert_eq!(app.campaign.phase, Phase::Breach);

    let mut guard = 0;
    while !matches!(app.campaign.phase, Phase::Cleared | Phase::Over) && guard < 800 {
        guard += 1;
        let _ = text(&mut app, 120, 40);
        if guard % 7 == 0 {
            let _ = text(&mut app, 154, 44);
        }
        if app.payout_open {
            app.press_code(KeyCode::Enter);
            continue;
        }
        match app.campaign.phase {
            Phase::Shop => {
                app.config.confirm_buys = false;
                buy_level_log(&mut app);
                buy_if_present(&mut app, ToolId::DeadDrop);
                app.press_code(KeyCode::Enter);
            }
            Phase::LevelUp => {
                app.press('1');
            }
            Phase::Breach | Phase::Tutorial => {
                let act = app
                    .campaign
                    .breach
                    .as_ref()
                    .map(trace_core::sensible)
                    .unwrap_or(Act::EndTurn);
                app.debug_act(act);
            }
            _ => break,
        }
    }
    assert_eq!(
        app.campaign.phase,
        Phase::Cleared,
        "line {}",
        app.campaign.over_line
    );
    let end = text(&mut app, 154, 44);
    assert!(
        end.contains("LAYER CLEARED") || end.contains("CLEARED"),
        "{end}"
    );
    let forbidden = ["balatro", "joker", "ante", "blind"];
    for word in forbidden {
        assert!(!end.to_lowercase().contains(word), "{word}");
    }
    let _ = tool(ToolId::FatPipe);
    let _ = simulate(1, Policy::Sensible);
}

fn buy_if_present(app: &mut App, id: ToolId) {
    let index = app.campaign.shop.as_ref().and_then(|shop| {
        shop.offers
            .iter()
            .position(|offer| matches!(offer, Some(trace_core::Offer::Tool(t)) if *t == id))
    });
    if let Some(index) = index {
        if let Some(ch) = char::from_digit((index + 1) as u32, 10) {
            app.press(ch);
        }
    }
}

fn buy_level_log(app: &mut App) {
    if app.campaign.levels.log != 1 {
        return;
    }
    let index = app.campaign.shop.as_ref().and_then(|shop| {
        shop.offers.iter().position(|offer| {
            matches!(
                offer,
                Some(trace_core::Offer::Level(trace_core::LevelKind::Log))
            )
        })
    });
    if let Some(index) = index {
        if let Some(ch) = char::from_digit((index + 1) as u32, 10) {
            app.press(ch);
        }
    }
}
