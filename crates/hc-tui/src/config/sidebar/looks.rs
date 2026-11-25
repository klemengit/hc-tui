//! hc-tui only: the highlight of the space and agent rows in the sidebar.
//! `[ui.sidebar] focused_style`, `cursor_style` and `open_style`.

use serde::Deserialize;

/// The palette names a row style may use, besides `#RGB` / `#RRGGBB`.
pub const SIDEBAR_COLOR_NAMES: &[&str] = &[
    "accent",
    "panel",
    "sidebar",
    "active_row",
    "selection",
    "surface0",
    "surface1",
    "overlay0",
    "overlay1",
    "text",
    "subtext0",
    "mauve",
    "green",
    "yellow",
    "red",
    "blue",
    "teal",
    "peach",
];

/// A colour: a fixed `#RGB` / `#RRGGBB`, or a name from the active theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SidebarColor {
    Rgb(u8, u8, u8),
    Theme(String),
}

impl<'de> Deserialize<'de> for SidebarColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if let Some(hex) = value.strip_prefix('#') {
            let digits: Option<Vec<u8>> = hex
                .chars()
                .map(|ch| ch.to_digit(16).map(|digit| digit as u8))
                .collect();
            return match digits.as_deref() {
                Some([r, g, b]) => Ok(Self::Rgb(r * 17, g * 17, b * 17)),
                Some([r1, r2, g1, g2, b1, b2]) => {
                    Ok(Self::Rgb(r1 * 16 + r2, g1 * 16 + g2, b1 * 16 + b2))
                }
                _ => Err(serde::de::Error::custom(
                    "sidebar colour must be #RGB, #RRGGBB or a theme name",
                )),
            };
        }
        if SIDEBAR_COLOR_NAMES.contains(&value.as_str()) {
            Ok(Self::Theme(value))
        } else {
            Err(serde::de::Error::custom(format!(
                "unknown sidebar colour `{value}`; use #RGB, #RRGGBB or one of: {}",
                SIDEBAR_COLOR_NAMES.join(", ")
            )))
        }
    }
}

/// One row look. Every field is optional; unset fields keep the default
/// look. `marker` is one character drawn just left of the row's text.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SidebarRowStyle {
    pub fg: Option<SidebarColor>,
    pub bg: Option<SidebarColor>,
    pub bold: Option<bool>,
    pub dim: Option<bool>,
    pub reverse: Option<bool>,
    pub marker: Option<char>,
    /// Colour of the marker only; `fg` when unset.
    pub marker_fg: Option<SidebarColor>,
}
