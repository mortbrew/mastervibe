//! ASCII art for the 21 default-rig tools.
//!
//! Rows are the picture from the tool card sheet, matched by catalog name.
//! Each row is 20 columns wide.

/// Three picture rows for `name`, or nothing when the tool has no card art.
pub fn art_lines(name: &str) -> Option<[&'static str; 3]> {
    Some(match name.to_ascii_uppercase().as_str() {
        r#"PACKET SNIFFER"# => [
            r#"       ((·))        "#,
            r#"       ─┼─╪─        "#,
            r#"        ╱ ╲         "#,
        ],
        r#"/VAR/LOG"# => [
            r#"      ┌──────┐      "#,
            r#"      │>>log│       "#,
            r#"      └──────┘      "#,
        ],
        r#"KEYLOGGER"# => [
            r#"       ┌┬┬┬┬┐       "#,
            r#"       ├┼┼┼┼┤       "#,
            r#"       └┴┴┴┴┘       "#,
        ],
        r#"SHOULDER SURF"# => [
            r#"        ◔ ◔         "#,
            r#"         ‿          "#,
            r#"       ▕[■]▏        "#,
        ],
        r#"ZERO-DAY"# => [
            r#"         ╱╲         "#,
            r#"        ╱00╲        "#,
            r#"       ╱DAY ╲       "#,
        ],
        r#"OVERCLOCK"# => [
            r#"       ╭─╮          "#,
            r#"       │╲│»»        "#,
            r#"       ╰─╯          "#,
        ],
        r#"SPEEDRUN.SH"# => [
            r#"         ▸▸▸        "#,
            r#"        ./run       "#,
            r#"         ▸▸▸        "#,
        ],
        r#"NULL ROUTE"# => [
            r#"        ●─╳         "#,
            r#"           ╳        "#,
            r#"        ∅           "#,
        ],
        r#"BRUTE FORCE"# => [
            r#"       ▓▓▓▓▓▓       "#,
            r#"       ▓░▓░▓░       "#,
            r#"       ░▓░▓░▓       "#,
        ],
        r#"SLOW BURN"# => [
            r#"         ░          "#,
            r#"        ▒░▒         "#,
            r#"        ▓▓▓         "#,
        ],
        r#"SHADOW"# => [
            r#"        ●           "#,
            r#"         ▓          "#,
            r#"          ░         "#,
        ],
        r#"PRECISION"# => [
            r#"         ┼          "#,
            r#"       ─(●)─        "#,
            r#"         ┼          "#,
        ],
        r#"DAISY CHAIN"# => [
            r#"       ●─●─●─       "#,
            r#"       ─●─●─●       "#,
            r#"                    "#,
        ],
        r#"BULLSEYE"# => [
            r#"        ╭───╮       "#,
            r#"        │◎ │        "#,
            r#"        ╰───╯       "#,
        ],
        r#"ASCII ART"# => [
            r#"        _/\_        "#,
            r#"       (o_o)        "#,
            r#"        / \         "#,
        ],
        r#"REPEATER"# => [
            r#"        ↻ X         "#,
            r#"        X X         "#,
            r#"        X X         "#,
        ],
        r#"HIGH VALUE"# => [
            r#"         ▲          "#,
            r#"        ▲▲▲         "#,
            r#"        $$$         "#,
        ],
        r#"CRYPTO MINER"# => [
            r#"         T          "#,
            r#"        ╱B╲         "#,
            r#"        ▔▔▔         "#,
        ],
        r#"COLD WALLET"# => [
            r#"        ┌──┐        "#,
            r#"        │❄$│        "#,
            r#"        └──┘        "#,
        ],
        r#"DARK WEB"# => [
            r#"      ▓░▓░          "#,
            r#"      ░▓.onion      "#,
            r#"      ▓░▓░          "#,
        ],
        r#"HONEYPOT"# => [
            r#"        ╭──╮        "#,
            r#"        │≋≋│        "#,
            r#"        ╰──╯        "#,
        ],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::art_lines;
    use mastervibe_core::embedded_tools;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn every_default_tool_has_20_column_art() {
        for tool in embedded_tools().tools() {
            let lines =
                art_lines(&tool.name).unwrap_or_else(|| panic!("missing art for {}", tool.name));
            for line in lines {
                assert_eq!(UnicodeWidthStr::width(line), 20, "{} {line:?}", tool.name);
            }
        }
        assert!(art_lines("not a tool").is_none());
    }
}
