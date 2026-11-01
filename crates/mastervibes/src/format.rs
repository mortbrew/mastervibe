//! Player-facing numbers, pegs, and the signature preview.
//!
//! Score math stays in the core. This module only formats what a probe would
//! pay before pegs, and walks a finished [`ScoreBreakdown`] into lines the
//! scoring screen can reveal one at a time.

use mastervibes_core::{
    embedded_catalog, embedded_tools, Amp, Code, ScoreBreakdown, ScoreLineKind, Symbol, GLYPHS,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// One revealed row of a probe's bits × amp pipeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreStep {
    /// What produced the change.
    pub label: String,
    /// Bits this row added. A clamp row stores the magnitude, not a delta.
    pub bits_delta: i32,
    /// Amp this row added.
    pub amp_delta: Amp,
    /// Bits after this row.
    pub run_bits: i32,
    /// Amp after this row.
    pub run_amp: Amp,
    /// This row is the clamp that pins a negative total at zero.
    pub clamp: bool,
}

/// What the probe being typed would pay before any pegs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    /// Every slot has a symbol.
    pub complete: bool,
    /// Signature name, once the probe is complete.
    pub signature: Option<String>,
    /// Signature bits. Zero when an absent symbol wipes the probe.
    pub bits: i32,
    /// Base amp plus the signature bonus. The base alone when the probe is wiped.
    pub amp: Amp,
    /// `bits × amp` in thousandths, before pegs and tools.
    pub before_pegs: i64,
    /// The probe contains a symbol already shown to be absent.
    pub wiped: bool,
}

/// Thousandths of a point as a player would read them. `5800000` is `5,800`.
pub fn format_points(milli: i64) -> String {
    let sign = if milli < 0 { "-" } else { "" };
    let milli = milli.unsigned_abs();
    let whole = milli / 1000;
    let frac = milli % 1000;
    let digits = group_thousands(whole);
    if frac == 0 {
        format!("{sign}{digits}")
    } else if frac % 100 == 0 {
        format!("{sign}{digits}.{}", frac / 100)
    } else if frac % 10 == 0 {
        format!("{sign}{digits}.{:02}", frac / 10)
    } else {
        format!("{sign}{digits}.{frac:03}")
    }
}

/// Glyph for a charset index. Unknown indexes read as `?`.
pub fn glyph(index: u8) -> char {
    GLYPHS.chars().nth(usize::from(index)).unwrap_or('?')
}

/// Symbols joined by spaces: `0 1 2 3`.
pub fn format_symbols(indices: &[u8]) -> String {
    indices
        .iter()
        .map(|index| glyph(*index).to_string())
        .collect::<Vec<_>>()
        .join(" ")
}

/// `*` is a hit, `+` is a near. Empty feedback is `no pegs`.
pub fn format_pegs(hits: u8, nears: u8) -> String {
    if hits == 0 && nears == 0 {
        return "no pegs".to_string();
    }
    let mut out = String::new();
    for _ in 0..hits {
        out.push('*');
    }
    if hits > 0 && nears > 0 {
        out.push(' ');
    }
    for _ in 0..nears {
        out.push('+');
    }
    out
}

/// ASCII bar of `width` columns. `current / target` fills `#`, the rest `-`.
pub fn bar(current: i64, target: i64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let fill = if target <= 0 || current <= 0 {
        0
    } else {
        let ratio = (current as f64 / target as f64).clamp(0.0, 1.0);
        (ratio * width as f64).round() as usize
    };
    let fill = fill.min(width);
    format!("[{}{}]", "#".repeat(fill), "-".repeat(width - fill))
}

/// Keep `text` inside `width` columns. A clipped string ends in `..`.
pub fn fit(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.to_string();
    }
    if width <= 2 {
        return ".".repeat(width);
    }
    let mut out = String::new();
    for ch in text.chars() {
        let next = out.width() + ch.width().unwrap_or(0);
        if next > width - 2 {
            break;
        }
        out.push(ch);
    }
    out.push_str("..");
    out
}

/// Pad `text` on the left so it sits in the middle of `width` columns.
pub fn center(text: &str, width: usize) -> String {
    let columns = UnicodeWidthStr::width(text);
    if columns >= width {
        return fit(text, width);
    }
    let pad = (width - columns) / 2;
    format!("{}{text}", " ".repeat(pad))
}

/// Signature and the bits × amp it pays before pegs.
///
/// An absent symbol wipes that preview to zero. Pegs and tools are not
/// guessed here; the core scores them when the probe is submitted.
pub fn preview(draft: &[Option<u8>], dead: &[Symbol]) -> Preview {
    let Some(indices) = draft.iter().copied().collect::<Option<Vec<_>>>() else {
        return Preview {
            complete: false,
            signature: None,
            bits: 0,
            amp: Amp::ZERO,
            before_pegs: 0,
            wiped: false,
        };
    };
    let Ok(code) = Code::try_from_indices(&indices) else {
        return Preview {
            complete: false,
            signature: None,
            bits: 0,
            amp: Amp::ZERO,
            before_pegs: 0,
            wiped: false,
        };
    };
    let catalog = embedded_catalog();
    let signature = catalog.detect(&code);
    let dead_mask = dead_mask(dead);
    let wiped = indices.iter().any(|index| dead_mask & (1 << index) != 0);
    if wiped {
        return Preview {
            complete: true,
            signature: Some(signature.name.clone()),
            bits: 0,
            amp: catalog.scoring().base_amp,
            before_pegs: 0,
            wiped: true,
        };
    }
    let amp = catalog.scoring().base_amp.saturating_add(signature.amp);
    let before_pegs = i64::from(signature.bits).saturating_mul(i64::from(amp.milli()));
    Preview {
        complete: true,
        signature: Some(signature.name.clone()),
        bits: signature.bits,
        amp,
        before_pegs,
        wiped: false,
    }
}

/// Walk `score` in pipeline order.
///
/// The second value is the base amp the probe started from. The last step's
/// running totals match [`ScoreBreakdown::bits`] and [`ScoreBreakdown::amp`].
pub fn build_steps(score: &ScoreBreakdown) -> (Vec<ScoreStep>, Amp) {
    let mut bits = 0i32;
    let mut amp = Amp::ZERO;
    let mut base = Amp::ONE;
    let mut steps = Vec::new();
    for line in &score.lines {
        match &line.kind {
            ScoreLineKind::Base => {
                amp = amp.saturating_add(line.amp);
                base = line.amp;
            }
            ScoreLineKind::Clamp => {
                if line.bits != 0 {
                    bits = 0;
                }
                if line.amp.milli() != 0 {
                    amp = Amp::ZERO;
                }
                steps.push(ScoreStep {
                    label: "clamp".to_string(),
                    bits_delta: line.bits,
                    amp_delta: line.amp,
                    run_bits: bits,
                    run_amp: amp,
                    clamp: true,
                });
            }
            other => {
                bits = bits.saturating_add(line.bits);
                amp = amp.saturating_add(line.amp);
                steps.push(ScoreStep {
                    label: step_label(other, &score.signature_name),
                    bits_delta: line.bits,
                    amp_delta: line.amp,
                    run_bits: bits,
                    run_amp: amp,
                    clamp: false,
                });
            }
        }
    }
    (steps, base)
}

fn step_label(kind: &ScoreLineKind, signature: &str) -> String {
    match kind {
        ScoreLineKind::Base => "base".to_string(),
        ScoreLineKind::Signature => signature.to_string(),
        ScoreLineKind::Hits { count } => format!("{count} {}", plural(*count, "hit", "hits")),
        ScoreLineKind::Nears { count } => format!("{count} {}", plural(*count, "near", "nears")),
        ScoreLineKind::Neighbours { .. } => "adjacent hits".to_string(),
        ScoreLineKind::Dead => "absent symbol".to_string(),
        ScoreLineKind::Tool { id } => embedded_tools()
            .get(id)
            .map(|tool| tool.name.clone())
            .unwrap_or_else(|| id.clone()),
        ScoreLineKind::Clamp => "clamp".to_string(),
    }
}

fn plural(count: u8, one: &str, many: &str) -> &'static str {
    // The labels are static words. Map the borrowed choice back to a static.
    if count == 1 {
        match one {
            "hit" => "hit",
            "near" => "near",
            _ => "hit",
        }
    } else {
        match many {
            "hits" => "hits",
            "nears" => "nears",
            _ => "hits",
        }
    }
}

fn dead_mask(dead: &[Symbol]) -> u8 {
    dead.iter()
        .fold(0u8, |mask, symbol| mask | (1 << symbol.index()))
}

fn group_thousands(value: u64) -> String {
    let digits = value.to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{build_steps, format_pegs, format_points, preview};
    use mastervibes_core::{
        score_probe, score_probe_with_dead, Amp, Charset, Code, ScoreHook, Symbol, ToolMod,
    };

    fn code(text: &str) -> Code {
        Code::parse(Charset::full(), text).unwrap()
    }

    #[test]
    fn points_drop_trailing_zeros_and_group_thousands() {
        assert_eq!(format_points(0), "0");
        assert_eq!(format_points(5_800_000), "5,800");
        assert_eq!(format_points(117_500), "117.5");
        assert_eq!(format_points(1_250), "1.25");
        assert_eq!(format_points(-2_000), "-2");
    }

    #[test]
    fn pegs_use_star_for_hit_and_plus_for_near() {
        assert_eq!(format_pegs(0, 0), "no pegs");
        assert_eq!(format_pegs(2, 0), "**");
        assert_eq!(format_pegs(0, 1), "+");
        assert_eq!(format_pegs(2, 1), "** +");
    }

    #[test]
    fn preview_is_the_signature_before_pegs() {
        let pair = preview(&[Some(0), Some(0), Some(1), Some(2)], &[]);
        assert!(pair.complete);
        assert_eq!(pair.signature.as_deref(), Some("Pair"));
        assert_eq!(pair.bits, 15);
        assert_eq!(pair.amp, Amp::from_milli(2_000));
        assert_eq!(pair.before_pegs, 30_000);
        assert!(!pair.wiped);

        // 0132 is not strictly ascending, so it stays All Unique.
        let unique = preview(&[Some(0), Some(1), Some(3), Some(2)], &[]);
        assert_eq!(unique.signature.as_deref(), Some("All Unique"));
        assert_eq!(unique.before_pegs, 10_000);

        let partial = preview(&[Some(0), None, Some(2), Some(3)], &[]);
        assert!(!partial.complete);
        assert!(partial.signature.is_none());
    }

    #[test]
    fn an_absent_symbol_wipes_the_preview() {
        let dead = Symbol::new(0).unwrap();
        let wiped = preview(&[Some(0), Some(1), Some(3), Some(2)], &[dead]);
        assert!(wiped.wiped);
        assert_eq!(wiped.signature.as_deref(), Some("All Unique"));
        assert_eq!(wiped.before_pegs, 0);
        assert_eq!(wiped.bits, 0);
    }

    #[test]
    fn steps_land_on_the_scored_totals() {
        let catalog = mastervibes_core::embedded_catalog();
        let score = score_probe(
            &code("0012"),
            mastervibes_core::Pegs {
                hits: 2,
                nears: 1,
                exact_mask: 0b0011,
            },
            catalog,
            None,
            &[],
        );
        let (steps, base) = build_steps(&score);
        assert_eq!(base, Amp::ONE);
        let last = steps.last().expect("pair with pegs has steps");
        assert_eq!(last.run_bits, score.bits);
        assert_eq!(last.run_amp, score.amp);
        assert!(steps.iter().any(|step| step.label == "Pair"));
        assert!(steps.iter().any(|step| step.label == "2 hits"));
        assert!(steps.iter().any(|step| step.label == "1 near"));
    }

    #[test]
    fn a_wiped_probe_steps_through_absent_symbol() {
        let catalog = mastervibes_core::embedded_catalog();
        let score = score_probe_with_dead(
            &code("0000"),
            mastervibes_core::Pegs::counts(0, 0),
            catalog,
            None,
            &[],
            0b1,
        );
        let (steps, _) = build_steps(&score);
        assert!(steps.iter().any(|step| step.label == "absent symbol"));
        assert_eq!(score.total_milli, 0);
        let last = steps.last().unwrap();
        assert_eq!(last.run_bits, score.bits);
        assert_eq!(last.run_amp, score.amp);
    }

    #[test]
    fn clamp_steps_match_a_negative_tool() {
        let catalog = mastervibes_core::embedded_catalog();
        let sink = ToolMod {
            id: "sink".to_string(),
            bits: -10_000,
            amp: Amp::from_milli(-10_000),
        };
        let hooks: [&dyn ScoreHook; 1] = [&sink];
        let score = score_probe(
            &code("0132"),
            mastervibes_core::Pegs::counts(0, 0),
            catalog,
            None,
            &hooks,
        );
        let (steps, _) = build_steps(&score);
        let last = steps.last().unwrap();
        assert_eq!(last.run_bits, 0);
        assert_eq!(last.run_amp, Amp::ZERO);
        assert!(steps.iter().any(|step| step.clamp));
    }
}
