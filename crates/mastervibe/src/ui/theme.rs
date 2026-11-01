//! Green and amber on black. Named ANSI colors, so a 16-colour terminal still reads.

use mastervibe_core::{BreachRole, Rarity};
use ratatui::style::{Color, Modifier, Style};

pub fn screen() -> Style {
    Style::default().fg(Color::Green).bg(Color::Black)
}

pub fn green() -> Style {
    Style::default().fg(Color::Green).bg(Color::Black)
}

pub fn green_bold() -> Style {
    green().add_modifier(Modifier::BOLD)
}

pub fn amber() -> Style {
    Style::default().fg(Color::Yellow).bg(Color::Black)
}

pub fn amber_bold() -> Style {
    amber().add_modifier(Modifier::BOLD)
}

pub fn dim() -> Style {
    Style::default().fg(Color::DarkGray).bg(Color::Black)
}

pub fn red() -> Style {
    Style::default().fg(Color::Red).bg(Color::Black)
}

pub fn cyan() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .bg(Color::Black)
        .add_modifier(Modifier::BOLD)
}

pub fn magenta() -> Style {
    Style::default().fg(Color::Magenta).bg(Color::Black)
}

pub fn white() -> Style {
    Style::default().fg(Color::White).bg(Color::Black)
}

pub fn role(role: BreachRole) -> Style {
    match role {
        BreachRole::Perimeter => green_bold(),
        BreachRole::Internal => cyan(),
        BreachRole::Root => amber_bold(),
    }
}

pub fn rarity(rarity: Rarity) -> Style {
    match rarity {
        Rarity::Common => green(),
        Rarity::Uncommon => cyan(),
        Rarity::Rare => magenta(),
        Rarity::Legendary => amber_bold(),
    }
}

pub fn rarity_word(rarity: Rarity) -> &'static str {
    match rarity {
        Rarity::Common => "Common",
        Rarity::Uncommon => "Uncommon",
        Rarity::Rare => "Rare",
        Rarity::Legendary => "Legendary",
    }
}
