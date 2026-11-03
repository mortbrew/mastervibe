//! Phosphor palettes. Every signal also has a glyph, so hue is never the only cue.

use ratatui::style::{Color, Modifier, Style};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaletteId {
    Default,
    Deut,
    Prot,
    Trit,
    Mono,
}

impl PaletteId {
    pub fn all() -> [PaletteId; 5] {
        [
            Self::Default,
            Self::Deut,
            Self::Prot,
            Self::Trit,
            Self::Mono,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Deut => "colourblind (deut)",
            Self::Prot => "colourblind (prot)",
            Self::Trit => "colourblind (trit)",
            Self::Mono => "mono",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Deut => "deut",
            Self::Prot => "prot",
            Self::Trit => "trit",
            Self::Mono => "mono",
        }
    }
}

impl PaletteId {
    pub fn shift(self, dir: i32) -> Self {
        let all = Self::all();
        let i = all.iter().position(|p| *p == self).unwrap_or(0) as i32;
        let n = all.len() as i32;
        all[(i + dir).rem_euclid(n) as usize]
    }
}

#[derive(Clone, Copy)]
pub struct Pal {
    pub bg: Color,
    pub frame: Color,
    pub green: Color,
    pub dim: Color,
    pub amber: Color,
    pub danger: Color,
    pub safe: Color,
    pub data: Color,
    pub key: Color,
    pub cred: Color,
    pub log: Color,
    pub text: Color,
    pub rare: Color,
    pub uncommon: Color,
    pub common: Color,
    pub mentor: Color,
    pub hover: Color,
    pub select_bg: Color,
}

pub fn palette(id: PaletteId) -> Pal {
    match id {
        PaletteId::Default => Pal {
            bg: Color::Rgb(0, 0, 0),
            frame: Color::Rgb(40, 210, 90),
            green: Color::Rgb(90, 255, 130),
            dim: Color::Rgb(50, 100, 65),
            amber: Color::Rgb(230, 190, 70),
            danger: Color::Rgb(255, 70, 55),
            safe: Color::Rgb(80, 190, 255),
            data: Color::Rgb(70, 235, 230),
            key: Color::Rgb(255, 220, 70),
            cred: Color::Rgb(255, 170, 40),
            log: Color::Rgb(230, 80, 210),
            text: Color::Rgb(210, 230, 210),
            rare: Color::Rgb(220, 90, 230),
            uncommon: Color::Rgb(70, 210, 230),
            common: Color::Rgb(200, 215, 200),
            mentor: Color::Rgb(230, 200, 70),
            hover: Color::Rgb(90, 230, 230),
            select_bg: Color::Rgb(30, 50, 20),
        },
        PaletteId::Deut => Pal {
            bg: Color::Rgb(0, 0, 0),
            frame: Color::Rgb(170, 200, 230),
            green: Color::Rgb(120, 180, 255),
            dim: Color::Rgb(90, 100, 120),
            amber: Color::Rgb(255, 190, 40),
            danger: Color::Rgb(255, 140, 30),
            safe: Color::Rgb(70, 130, 255),
            data: Color::Rgb(40, 220, 220),
            key: Color::Rgb(255, 220, 40),
            cred: Color::Rgb(255, 180, 40),
            log: Color::Rgb(230, 60, 180),
            text: Color::Rgb(220, 220, 220),
            rare: Color::Rgb(230, 70, 190),
            uncommon: Color::Rgb(40, 200, 220),
            common: Color::Rgb(210, 210, 210),
            mentor: Color::Rgb(255, 190, 50),
            hover: Color::Rgb(80, 210, 230),
            select_bg: Color::Rgb(20, 30, 50),
        },
        PaletteId::Prot => Pal {
            bg: Color::Rgb(0, 0, 0),
            frame: Color::Rgb(180, 190, 230),
            green: Color::Rgb(255, 170, 50),
            dim: Color::Rgb(110, 100, 90),
            amber: Color::Rgb(255, 210, 80),
            danger: Color::Rgb(80, 130, 255),
            safe: Color::Rgb(255, 160, 40),
            data: Color::Rgb(240, 220, 80),
            key: Color::Rgb(245, 245, 245),
            cred: Color::Rgb(255, 190, 60),
            log: Color::Rgb(255, 120, 180),
            text: Color::Rgb(230, 225, 210),
            rare: Color::Rgb(255, 130, 190),
            uncommon: Color::Rgb(240, 210, 90),
            common: Color::Rgb(220, 215, 200),
            mentor: Color::Rgb(255, 200, 70),
            hover: Color::Rgb(255, 210, 90),
            select_bg: Color::Rgb(40, 30, 10),
        },
        PaletteId::Trit => Pal {
            bg: Color::Rgb(0, 0, 0),
            frame: Color::Rgb(160, 220, 210),
            green: Color::Rgb(80, 220, 210),
            dim: Color::Rgb(80, 110, 110),
            amber: Color::Rgb(255, 80, 140),
            danger: Color::Rgb(230, 60, 160),
            safe: Color::Rgb(40, 210, 200),
            data: Color::Rgb(240, 220, 70),
            key: Color::Rgb(245, 245, 245),
            cred: Color::Rgb(255, 140, 60),
            log: Color::Rgb(90, 140, 255),
            text: Color::Rgb(220, 230, 225),
            rare: Color::Rgb(230, 70, 170),
            uncommon: Color::Rgb(240, 210, 80),
            common: Color::Rgb(210, 220, 215),
            mentor: Color::Rgb(255, 90, 150),
            hover: Color::Rgb(80, 220, 210),
            select_bg: Color::Rgb(20, 40, 40),
        },
        PaletteId::Mono => Pal {
            bg: Color::Rgb(0, 0, 0),
            frame: Color::Gray,
            green: Color::White,
            dim: Color::DarkGray,
            amber: Color::White,
            danger: Color::White,
            safe: Color::Gray,
            data: Color::White,
            key: Color::White,
            cred: Color::Gray,
            log: Color::White,
            text: Color::Gray,
            rare: Color::White,
            uncommon: Color::Gray,
            common: Color::DarkGray,
            mentor: Color::White,
            hover: Color::White,
            select_bg: Color::DarkGray,
        },
    }
}

impl Pal {
    pub fn fg(self, color: Color) -> Style {
        Style::default().fg(color).bg(self.bg)
    }

    pub fn bold(self, color: Color) -> Style {
        self.fg(color).add_modifier(Modifier::BOLD)
    }
}
