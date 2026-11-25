//! The configurable highlight of space and agent rows (`[ui.sidebar]
//! focused_style`, `cursor_style`, `open_style`). Unset fields leave the
//! theme's own highlight alone, so the default config draws what it always
//! did.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::palette::Palette;
use crate::config::{SidebarColor, SidebarRowStyle};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RowLook {
    fg: Option<Color>,
    bg: Option<Color>,
    bold: Option<bool>,
    dim: Option<bool>,
    reverse: Option<bool>,
    marker: Option<char>,
    marker_fg: Option<Color>,
}

fn color(value: &Option<SidebarColor>, palette: &Palette) -> Option<Color> {
    match value.as_ref()? {
        SidebarColor::Rgb(r, g, b) => Some(Color::Rgb(*r, *g, *b)),
        SidebarColor::Theme(name) => palette.named(name),
    }
}

impl RowLook {
    fn resolve(style: &SidebarRowStyle, palette: &Palette) -> Self {
        Self {
            fg: color(&style.fg, palette),
            bg: color(&style.bg, palette),
            bold: style.bold,
            dim: style.dim,
            reverse: style.reverse,
            marker: style.marker,
            marker_fg: color(&style.marker_fg, palette),
        }
    }

    /// `self` with every field `upper` sets replaced by it.
    fn over(self, upper: Self) -> Self {
        Self {
            fg: upper.fg.or(self.fg),
            bg: upper.bg.or(self.bg),
            bold: upper.bold.or(self.bold),
            dim: upper.dim.or(self.dim),
            reverse: upper.reverse.or(self.reverse),
            marker: upper.marker.or(self.marker),
            marker_fg: upper.marker_fg.or(self.marker_fg),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// Paint the look over the `rect` rows; the marker goes in `marker_at`.
    fn paint(&self, buffer: &mut Buffer, rect: Rect, marker_at: (u16, u16)) {
        let mut style = Style::default();
        if let Some(bg) = self.bg {
            style = style.bg(bg);
        }
        if let Some(fg) = self.fg {
            style = style.fg(fg);
        }
        for (flag, modifier) in [
            (self.bold, Modifier::BOLD),
            (self.dim, Modifier::DIM),
            (self.reverse, Modifier::REVERSED),
        ] {
            match flag {
                Some(true) => style = style.add_modifier(modifier),
                Some(false) => style = style.remove_modifier(modifier),
                None => {}
            }
        }
        buffer.set_style(rect, style);
        if let Some(marker) = self.marker
            && marker_at.0 < rect.right()
            && marker_at.1 < rect.bottom()
        {
            let cell = &mut buffer[marker_at];
            cell.set_char(marker);
            if let Some(fg) = self.marker_fg.or(self.fg) {
                cell.set_fg(fg);
            }
        }
    }
}

/// The three looks, resolved against the active palette.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct SidebarLooks {
    focused: RowLook,
    cursor: RowLook,
    open: RowLook,
}

impl SidebarLooks {
    /// `styles` are `focused_style`, `cursor_style` and `open_style`.
    pub(crate) fn new(styles: &[SidebarRowStyle; 3], palette: &Palette) -> Self {
        Self {
            focused: RowLook::resolve(&styles[0], palette),
            cursor: RowLook::resolve(&styles[1], palette),
            open: RowLook::resolve(&styles[2], palette),
        }
    }

    /// A marker is configured somewhere: space rows keep one cell of
    /// padding for it, on every row alike.
    pub(crate) fn reserves_marker(&self) -> bool {
        [self.focused, self.cursor, self.open]
            .iter()
            .any(|look| look.marker.is_some())
    }

    /// Paint the look of a row that is `open` (a space the cursor is
    /// inside), `focused` (where the user is) and/or under the `cursor`.
    /// Where two apply, the later field wins: open, focused, cursor.
    pub(crate) fn paint(
        &self,
        buffer: &mut Buffer,
        rect: Rect,
        marker_at: (u16, u16),
        open: bool,
        focused: bool,
        cursor: bool,
    ) {
        let mut look = RowLook::default();
        if open {
            look = look.over(self.open);
        }
        if focused {
            look = look.over(self.focused);
        }
        if cursor {
            look = look.over(self.cursor);
        }
        if !look.is_empty() {
            look.paint(buffer, rect, marker_at);
        }
    }
}
