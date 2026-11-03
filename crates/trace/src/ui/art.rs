//! Portrait art. The card shows the picture and the name. The effect lives in INSPECT.

use trace_core::Arch;

pub const STEALTH: &[&str] = &[
    "    .-^^^-.    ",
    "   /  ___  \\   ",
    "  |  (o o)  |  ",
    "  |   \\_/   |  ",
    "   \\  ___  /   ",
    "  __|/|_|\\|__  ",
    "  ░░ ghost ░░  ",
];

pub const GREED: &[&str] = &[
    "   _________   ",
    "  [ $ $ $ $ ]  ",
    "  |█▓▒DATA▒▓█| ",
    "  |█▓▒░░░▒▓█|  ",
    "   \\_______/   ",
    "    ║ ║ ║ ║    ",
    "   +bits++++   ",
];

pub const PROXY: &[&str] = &[
    " @══╗          ",
    "    ╚═p1══╗    ",
    "          ║    ",
    "    ╔═p2══╝    ",
    "    ║          ",
    "    ╚═p3═══▶◆  ",
    "  bounce·burn  ",
];

pub const DATA: &[&str] = &[
    " ┌───┐ ┌───┐   ",
    " │KEY│ │CRD│   ",
    " │ ◆ │ │ $ │   ",
    " └───┘ └───┘   ",
    "   ┌────────┐  ",
    "   │ ZERO ∅ │  ",
    "   └────────┘  ",
];

pub const CYCLE: &[&str] = &[
    "    .-----.    ",
    "   /  12   \\   ",
    "  | 9  ●  3 |  ",
    "  |   /     |  ",
    "   \\   6   /   ",
    "    `-----´    ",
    "   ● ● ● »     ",
];

pub const CHAOS: &[&str] = &[
    "  ▓▒░▒▓▒░▒▓▒  ",
    "  ░ ?¿ ?¿ ? ░  ",
    "  ▒ ┌┐ ╳ ┌┐ ▒  ",
    "  ▓ ?¿ ?¿ ? ▓  ",
    "  ░▒▓▒░▒▓▒░▒  ",
    "   segfault    ",
    "   0x?? 0x??   ",
];

pub const XAMP: &[&str] = &[
    "   \\   │   /   ",
    "    \\  │  /    ",
    "  ═══ ×AMP ═══ ",
    "    /  │  \\    ",
    "   /   │   \\   ",
    "               ",
    "  bits × amp   ",
];

pub const LEVEL: &[&str] = &[
    "     ▲▲▲▲     ",
    "    ▲▲▲▲▲▲    ",
    "      ║║      ",
    "      ║║      ",
    "     verb     ",
    "  Lv  → next  ",
    "               ",
];

pub const SCRIPT: &[&str] = &[
    "  ┌──────────┐",
    "  │ $ ./▒▒▒  │",
    "  │ > ███▒░  │",
    "  │ > done.  │",
    "  └──────────┘",
];

pub fn for_arch(arch: Arch) -> &'static [&'static str] {
    match arch {
        Arch::Stealth => STEALTH,
        Arch::Greed => GREED,
        Arch::Proxy => PROXY,
        Arch::Data => DATA,
        Arch::Cycle => CYCLE,
        Arch::Chaos => CHAOS,
        Arch::XAmp => XAMP,
    }
}

pub const LOGO: &[&str] = &[
    "      ████████╗██████╗  █████╗  ██████╗███████╗",
    "      ╚══██╔══╝██╔══██╗██╔══██╗██╔════╝██╔════╝",
    "./      ██║   ██████╔╝███████║██║     █████╗  ",
    "        ██║   ██╔══██╗██╔══██║██║     ██╔══╝  ",
    "        ██║   ██║  ██║██║  ██║╚██████╗███████╗",
    "        ╚═╝   ╚═╝  ╚═╝╚═╝  ╚═╝ ╚═════╝╚══════╝",
];
