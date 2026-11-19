//! Shared colours for the viewer, resource heatmap, and examples.
//!
//! General styling uses Catppuccin Mocha. Change the named roles here to try
//! different colours throughout the app, including [`SELECTED_MARKER_OUTLINE`].

use bevy::prelude::*;

/// Catppuccin flavour supplying the general styling colours.
pub const UI_COLORS: catppuccin::FlavorColors = catppuccin::PALETTE.mocha.colors;

/// Background behind the resource field.
pub const BACKGROUND: Color = from_catppuccin(UI_COLORS.base);
/// Background of the run header.
pub const HEADER_BACKGROUND: Color = from_catppuccin(UI_COLORS.surface0);
/// Background of the vertex inspector.
pub const INSPECTOR_BACKGROUND: Color = from_catppuccin(UI_COLORS.mantle);
/// Headings and primary information.
pub const TEXT_PRIMARY: Color = from_catppuccin(UI_COLORS.text);
/// Labels and secondary information.
pub const TEXT_SECONDARY: Color = from_catppuccin(UI_COLORS.subtext1);
/// Interior of each vertex marker.
pub const MARKER_FILL: Color = from_catppuccin(UI_COLORS.rosewater);
/// Outline of an unselected vertex marker.
pub const MARKER_OUTLINE: Color = from_catppuccin(UI_COLORS.crust);
/// Outline of the selected vertex marker.
pub const SELECTED_MARKER_OUTLINE: Color = from_catppuccin(UI_COLORS.overlay0);

/// Colour at the bottom of the resource scale.
pub const HEATMAP_LOW: Color = Color::linear_rgb(35.0 / 255.0, 55.0 / 255.0, 140.0 / 255.0);
/// Colour at the midpoint of the resource scale.
pub const HEATMAP_MID: Color = Color::linear_rgb(37.0 / 255.0, 173.0 / 255.0, 154.0 / 255.0);
/// Colour at the top of the resource scale.
pub const HEATMAP_HIGH: Color = Color::linear_rgb(253.0 / 255.0, 220.0 / 255.0, 70.0 / 255.0);
/// Heatmap annotation colour on light cells.
pub const HEATMAP_TEXT_DARK: Color = Color::BLACK;
/// Heatmap annotation colour on dark cells.
pub const HEATMAP_TEXT_LIGHT: Color = Color::WHITE;

// Match the Catppuccin crate's Bevy conversion in constant expressions.
const fn from_catppuccin(color: catppuccin::Color) -> Color {
    Color::hsl(color.hsl.h as f32, color.hsl.s as f32, color.hsl.l as f32)
}

/// Returns the resource colour at a normalized value clamped to `[0, 1]`.
pub fn heatmap_color(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);

    if t < 0.5 {
        HEATMAP_LOW.mix(&HEATMAP_MID, t * 2.0)
    } else {
        HEATMAP_MID.mix(&HEATMAP_HIGH, (t - 0.5) * 2.0)
    }
}

/// Chooses a light or dark heatmap annotation colour from the cell's luminance.
pub fn contrasting_text_color(background: Color) -> Color {
    let rgb = background.to_linear();
    let luminance = 0.2126 * rgb.red + 0.7152 * rgb.green + 0.0722 * rgb.blue;

    if luminance > 0.179 {
        HEATMAP_TEXT_DARK
    } else {
        HEATMAP_TEXT_LIGHT
    }
}
