//! Centralized theme colors for the Chambers app.
//!
//! This module provides consistent colors that match the app's dark theme
//! and can be used throughout the application.

use gpui::{rgb, rgba, Hsla};

/// App theme colors - dark theme matching the overall app design
pub struct AppColors;

#[allow(dead_code)]
impl AppColors {
    // ── Backgrounds ──────────────────────────────────────────────────────

    /// Main background color
    pub fn bg_main() -> Hsla {
        rgb(0x1a1a1a).into()
    }

    /// Header/toolbar background color
    pub fn bg_header() -> Hsla {
        rgb(0x252525).into()
    }

    /// Secondary background (slightly lighter)
    pub fn bg_secondary() -> Hsla {
        rgb(0x1e1e1e).into()
    }

    /// Hover state background
    pub fn bg_hover() -> Hsla {
        rgb(0x3a3a3a).into()
    }

    /// Active/selected state background
    pub fn bg_active() -> Hsla {
        rgb(0x2a2a2a).into()
    }

    /// Cell highlight background (subtle blue tint)
    pub fn bg_cell_selected() -> Hsla {
        rgba(0x0078d430).into()
    }

    // ── Borders ──────────────────────────────────────────────────────────

    /// Default border color
    pub fn border() -> Hsla {
        rgb(0x3a3a3a).into()
    }

    /// Subtle border color
    pub fn border_subtle() -> Hsla {
        rgb(0x2a2a2a).into()
    }

    /// Active/selected border color
    pub fn border_active() -> Hsla {
        rgb(0x0078d4).into()
    }

    // ── Text ─────────────────────────────────────────────────────────────

    /// Primary text color
    pub fn text() -> Hsla {
        rgb(0xe0e0e0).into()
    }

    /// Secondary text color
    pub fn text_secondary() -> Hsla {
        rgb(0xb0b0b0).into()
    }

    /// Muted text color
    pub fn text_muted() -> Hsla {
        rgb(0x808080).into()
    }

    /// Dim text color
    pub fn text_dim() -> Hsla {
        rgb(0x606060).into()
    }

    // ── Accent Colors ────────────────────────────────────────────────────

    /// Primary accent color (blue)
    pub fn accent() -> Hsla {
        rgb(0x0078d4).into()
    }

    /// Accent hover color
    pub fn accent_hover() -> Hsla {
        rgb(0x1a8cde).into()
    }

    /// Truncated text indicator color (light blue)
    pub fn truncated_text() -> Hsla {
        rgb(0x6eb5ff).into()
    }

    // ── Status Colors ────────────────────────────────────────────────────

    /// Error/danger color
    pub fn error() -> Hsla {
        rgb(0xf44336).into()
    }

    /// Error hover background
    pub fn error_hover_bg() -> Hsla {
        rgba(0xf4433615).into()
    }

    /// Success color
    pub fn success() -> Hsla {
        rgb(0x4caf50).into()
    }

    /// Warning color
    pub fn warning() -> Hsla {
        rgb(0xff9800).into()
    }

    // ── Menu Colors ──────────────────────────────────────────────────────

    /// Menu background
    pub fn menu_bg() -> Hsla {
        rgb(0x1f1f1f).into()
    }

    /// Menu border
    pub fn menu_border() -> Hsla {
        rgb(0x333333).into()
    }

    /// Menu item hover background
    pub fn menu_hover() -> Hsla {
        rgb(0x2a2a2a).into()
    }
}
