//! Visual identity, following the tui-tools family (tui-kit): Tokyo Night by
//! default, the active Omarchy theme when one is present, `$TUI_THEME` to point
//! at any Omarchy-style `colors.toml`, and `NO_COLOR` / `--no-color` to drop
//! every color while keeping layout and bold.

use ratatui::style::{Color, Modifier, Style};
use std::{collections::HashMap, path::PathBuf};

/// Raw colors. Mirrors tui-kit's `theme.Palette`.
#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub name: String,
    pub foreground: Color,
    pub muted: Color,
    /// Titles and focused borders.
    pub accent: Color,
    pub selection: Color,
    pub green: Color,
    pub red: Color,
    pub yellow: Color,
    pub blue: Color,
    pub magenta: Color,
    pub cyan: Color,
}

impl Palette {
    pub fn tokyo_night() -> Self {
        Self {
            name: "tokyo-night".into(),
            foreground: hex("#c0caf5").unwrap(),
            muted: hex("#565f89").unwrap(),
            accent: hex("#7aa2f7").unwrap(),
            selection: hex("#33467c").unwrap(),
            green: hex("#9ece6a").unwrap(),
            red: hex("#f7768e").unwrap(),
            yellow: hex("#e0af68").unwrap(),
            blue: hex("#7aa2f7").unwrap(),
            magenta: hex("#bb9af7").unwrap(),
            cyan: hex("#7dcfff").unwrap(),
        }
    }

    /// Overlay the keys of an Omarchy `colors.toml` onto `base`. Unknown or
    /// malformed values keep the base color: an exotic theme degrades, it
    /// never takes the tool down.
    fn from_keys(name: String, keys: &HashMap<String, String>, base: Palette) -> Self {
        let pick = |names: &[&str], fallback: Color| {
            names
                .iter()
                .find_map(|n| keys.get(*n).and_then(|v| hex(v)))
                .unwrap_or(fallback)
        };
        Self {
            name,
            foreground: pick(&["bright_foreground", "foreground"], base.foreground),
            muted: pick(&["dark_foreground", "muted"], base.muted),
            accent: pick(&["accent", "blue"], base.accent),
            selection: pick(&["selection"], base.selection),
            green: pick(&["green"], base.green),
            red: pick(&["red"], base.red),
            yellow: pick(&["yellow"], base.yellow),
            blue: pick(&["blue"], base.blue),
            magenta: pick(&["bright_magenta", "magenta"], base.magenta),
            cyan: pick(&["bright_cyan", "cyan"], base.cyan),
        }
    }
}

/// Palette plus the shared styles. Mirrors tui-kit's `theme.Theme`.
#[derive(Debug, Clone)]
pub struct Theme {
    pub palette: Palette,
    pub no_color: bool,
    /// Non-fatal problem resolving the palette (e.g. a bad `$TUI_THEME`),
    /// surfaced in the status line.
    pub warning: Option<String>,
}

impl Theme {
    pub fn resolve(no_color_flag: bool) -> Self {
        let (palette, warning) = resolve_palette();
        let no_color = no_color_flag || std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
        Self {
            palette,
            no_color,
            warning,
        }
    }

    fn fg(&self, c: Color) -> Style {
        if self.no_color {
            Style::default()
        } else {
            Style::default().fg(c)
        }
    }

    pub fn base(&self) -> Style {
        self.fg(self.palette.foreground)
    }
    pub fn muted(&self) -> Style {
        self.fg(self.palette.muted)
    }
    pub fn accent(&self) -> Style {
        self.fg(self.palette.accent)
    }
    pub fn title(&self) -> Style {
        self.accent().add_modifier(Modifier::BOLD)
    }
    pub fn table_head(&self) -> Style {
        self.fg(self.palette.magenta).add_modifier(Modifier::BOLD)
    }
    pub fn key(&self) -> Style {
        self.fg(self.palette.yellow).add_modifier(Modifier::BOLD)
    }
    pub fn ok(&self) -> Style {
        self.fg(self.palette.green)
    }
    pub fn warn(&self) -> Style {
        self.fg(self.palette.yellow)
    }
    /// Panel borders: rounded, muted.
    pub fn border(&self) -> Style {
        self.muted()
    }
    /// Dialog borders: rounded, accent.
    pub fn dialog_border(&self) -> Style {
        self.accent()
    }

    /// Series color by name, so charts and table agree.
    pub fn series(&self, c: Color) -> Style {
        self.fg(c)
    }
}

/// Where Omarchy keeps the active theme. Newer releases moved it from
/// `~/.config` to `~/.local/state`; check both.
const OMARCHY_COLORS: [&str; 2] = [
    ".local/state/omarchy/current/theme/colors.toml",
    ".config/omarchy/current/theme/colors.toml",
];

/// `$TUI_THEME` → active Omarchy theme → Tokyo Night. Never fails.
fn resolve_palette() -> (Palette, Option<String>) {
    if let Some(custom) = std::env::var_os("TUI_THEME").filter(|v| !v.is_empty()) {
        let path = expand_home(&custom.to_string_lossy());
        return match load_palette_file(&path) {
            Ok(p) => (p, None),
            Err(e) => (Palette::tokyo_night(), Some(e)),
        };
    }
    if let Some(home) = std::env::var_os("HOME") {
        for rel in OMARCHY_COLORS {
            if let Ok(p) = load_palette_file(&PathBuf::from(&home).join(rel)) {
                return (p, None);
            }
        }
    }
    (Palette::tokyo_night(), None)
}

fn load_palette_file(path: &std::path::Path) -> Result<Palette, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("theme: open {}: {e}", path.display()))?;
    let keys = parse_kv(&text);
    if keys.is_empty() {
        return Err(format!(
            "theme: {} has no usable color keys",
            path.display()
        ));
    }
    // The theme directory name is the most useful label we have.
    let name = path
        .parent()
        .and_then(|d| d.file_name())
        .map(|d| d.to_string_lossy().into_owned())
        .filter(|d| d != "theme")
        .map(|d| format!("omarchy:{d}"))
        .unwrap_or_else(|| "omarchy".into());
    Ok(Palette::from_keys(name, &keys, Palette::tokyo_night()))
}

/// Flat `key = "value"` parser — all an Omarchy colors.toml needs, so no TOML
/// crate. Comments, sections and anything unparseable are skipped.
fn parse_kv(text: &str) -> HashMap<String, String> {
    text.lines()
        .filter_map(|line| {
            let (k, v) = line.split_once('=')?;
            let k = k.trim();
            if k.starts_with('#') || k.starts_with('[') {
                return None;
            }
            let v = v.trim();
            // Hex colors contain '#', so a quoted value is read up to its
            // closing quote; only a bare value has a trailing comment cut.
            let v = match v.chars().next() {
                Some(q @ ('"' | '\'')) => v[1..].split(q).next().unwrap_or(""),
                _ => v.split('#').next().unwrap_or("").trim(),
            };
            Some((k.to_string(), v.to_string()))
        })
        .filter(|(k, v)| !k.is_empty() && !v.is_empty())
        .collect()
}

fn expand_home(p: &str) -> PathBuf {
    match (p.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(p),
    }
}

/// `#rgb`, `#rrggbb` or `#rrggbbaa` (alpha ignored).
fn hex(s: &str) -> Option<Color> {
    let h = s.strip_prefix('#')?;
    let byte = |i: usize| u8::from_str_radix(h.get(i..i + 2)?, 16).ok();
    match h.len() {
        3 => {
            let n = |i: usize| {
                u8::from_str_radix(h.get(i..i + 1)?, 16)
                    .ok()
                    .map(|v| v * 17)
            };
            Some(Color::Rgb(n(0)?, n(1)?, n(2)?))
        }
        6 | 8 => Some(Color::Rgb(byte(0)?, byte(2)?, byte(4)?)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_forms() {
        assert_eq!(hex("#7aa2f7"), Some(Color::Rgb(0x7a, 0xa2, 0xf7)));
        assert_eq!(hex("#fff"), Some(Color::Rgb(255, 255, 255)));
        assert_eq!(hex("#7aa2f7ff"), Some(Color::Rgb(0x7a, 0xa2, 0xf7)));
        assert_eq!(hex("7aa2f7"), None);
        assert_eq!(hex("#zzzzzz"), None);
    }

    #[test]
    fn omarchy_keys_overlay_defaults() {
        let keys = parse_kv(
            "mode = \"dark\"\naccent = \"#112233\"\nred = \"bogus\"\n[section]\n\nbright_cyan = '#0db9d7'\n",
        );
        let p = Palette::from_keys("omarchy:x".into(), &keys, Palette::tokyo_night());
        assert_eq!(p.accent, Color::Rgb(0x11, 0x22, 0x33));
        assert_eq!(p.cyan, Color::Rgb(0x0d, 0xb9, 0xd7));
        // Malformed value keeps the default.
        assert_eq!(p.red, Palette::tokyo_night().red);
    }
}
