//! Shared window geometry for the field, inspector, and pointer input.

use bevy::prelude::*;

const SIDEBAR_BREAKPOINT: f32 = 900.0;
const SIDEBAR_WIDTH: f32 = 240.0;
const STACKED_INSPECTOR_HEIGHT: f32 = 144.0;
const HEADER_HEIGHT: f32 = 60.0;
const LEGEND_HEIGHT: f32 = 48.0;
const FIELD_PADDING: f32 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ViewerLayout {
    window_size: Vec2,
    pub(super) field: Rect,
    pub(super) inspector: Rect,
    pub(super) legend: Rect,
    pub(super) header: Rect,
    pub(super) compact: bool,
}

impl ViewerLayout {
    /// Lays out the viewer in logical pixels from the window's top-left corner.
    pub(super) fn new(window_size: Vec2) -> Self {
        let compact = window_size.x < SIDEBAR_BREAKPOINT;
        let header_bottom = HEADER_HEIGHT.min(window_size.y);
        let inspector_top = if compact {
            (window_size.y - STACKED_INSPECTOR_HEIGHT).max(header_bottom)
        } else {
            header_bottom
        };

        let field_right = if compact {
            window_size.x
        } else {
            window_size.x - SIDEBAR_WIDTH
        };

        let field_and_legend_bottom = if compact {
            inspector_top
        } else {
            window_size.y
        };

        let legend_top = (field_and_legend_bottom - LEGEND_HEIGHT).max(header_bottom);
        let header = Rect::new(0.0, 0.0, window_size.x, header_bottom);
        let field = Rect::new(0.0, header_bottom, field_right, legend_top);
        let legend = Rect::new(0.0, legend_top, field_right, field_and_legend_bottom);
        let inspector = Rect::from_corners(
            Vec2::new(if compact { 0.0 } else { field_right }, inspector_top),
            window_size,
        );

        Self {
            window_size,
            field,
            inspector,
            legend,
            header,
            compact,
        }
    }

    /// Fits a centered field into its pane with padding for protruding markers.
    pub(super) fn fit_field(self, field_size: Vec2) -> Option<FieldView> {
        let field_pane_size = self.field.size();

        if field_pane_size.min_element() <= 0.0 {
            return None;
        }

        let window_center = self.window_size * 0.5;
        let padded_size = field_size + Vec2::splat(2.0 * FIELD_PADDING);
        let scale = (padded_size / field_pane_size).max_element();
        let camera_offset = screen_difference_to_world(window_center - self.field.center()) * scale;

        Some(FieldView {
            window_center,
            field: self.field,
            scale,
            camera_offset,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct FieldView {
    window_center: Vec2,
    field: Rect,
    /// World units per logical pixel for the orthographic projection.
    pub(super) scale: f32,
    /// Camera translation in the XY plane.
    pub(super) camera_offset: Vec2,
}

impl FieldView {
    /// Converts a cursor inside the field pane to a world position.
    pub(super) fn screen_to_world(self, cursor: Vec2) -> Option<Vec2> {
        if !cursor.cmpge(self.field.min).all() || !cursor.cmplt(self.field.max).all() {
            return None;
        }

        let world_offset = screen_difference_to_world(cursor - self.window_center) * self.scale;
        Some(self.camera_offset + world_offset)
    }
}

fn screen_difference_to_world(offset: Vec2) -> Vec2 {
    Vec2::new(offset.x, -offset.y)
}

#[cfg(test)]
mod tests {
    use bevy::window::WindowResolution;

    use super::*;

    #[test]
    fn reserves_the_sidebar_at_default_width_and_the_lower_panel_at_minimum_width() {
        let wide = ViewerLayout::new(Vec2::new(1040.0, 820.0));
        assert!(!wide.compact);
        assert_eq!(wide.header, Rect::new(0.0, 0.0, 1040.0, 60.0));
        assert_eq!(wide.field, Rect::new(0.0, 60.0, 800.0, 772.0));
        assert_eq!(wide.legend, Rect::new(0.0, 772.0, 800.0, 820.0));
        assert_eq!(wide.inspector, Rect::new(800.0, 60.0, 1040.0, 820.0));

        let narrow = ViewerLayout::new(Vec2::new(760.0, 740.0));
        assert!(narrow.compact);
        assert_eq!(narrow.field, Rect::new(0.0, 60.0, 760.0, 548.0));
        assert_eq!(narrow.legend, Rect::new(0.0, 548.0, 760.0, 596.0));
        assert_eq!(narrow.inspector, Rect::new(0.0, 596.0, 760.0, 740.0));
        assert!(ViewerLayout::new(Vec2::new(899.0, 740.0)).compact);
        assert!(!ViewerLayout::new(Vec2::new(900.0, 740.0)).compact);
    }

    #[test]
    fn centers_and_fits_the_padded_grid_and_only_maps_clicks_in_the_field() {
        let grid_size = Vec2::new(600.0, 420.0);

        for window_size in [Vec2::new(1040.0, 820.0), Vec2::new(760.0, 740.0)] {
            let layout = ViewerLayout::new(window_size);
            let view = layout.fit_field(grid_size).unwrap();
            let field_center = layout.field.center();
            let screen_half_size = (grid_size * 0.5 + Vec2::splat(FIELD_PADDING)) / view.scale;

            assert!(
                view.screen_to_world(field_center)
                    .unwrap()
                    .abs_diff_eq(Vec2::ZERO, 1e-5)
            );
            assert!(
                (field_center - screen_half_size)
                    .cmpge(layout.field.min - Vec2::splat(1e-4))
                    .all()
            );
            assert!(
                (field_center + screen_half_size)
                    .cmple(layout.field.max + Vec2::splat(1e-4))
                    .all()
            );
            assert!(
                view.screen_to_world(field_center + Vec2::new(10.0, -10.0))
                    .unwrap()
                    .abs_diff_eq(Vec2::splat(10.0 * view.scale), 1e-5)
            );
            assert!(view.screen_to_world(layout.header.center()).is_none());
            assert!(view.screen_to_world(layout.legend.center()).is_none());
            assert!(view.screen_to_world(layout.inspector.center()).is_none());
            assert!(view.screen_to_world(layout.field.max).is_none());
        }
    }

    #[test]
    fn layout_and_cursor_mapping_use_logical_pixels() {
        let standard = WindowResolution::new(760, 740);
        let high_dpi = WindowResolution::new(1520, 1480).with_scale_factor_override(2.0);
        let standard_layout = ViewerLayout::new(standard.size());
        let high_dpi_layout = ViewerLayout::new(high_dpi.size());
        let grid_size = Vec2::splat(600.0);

        assert_eq!(standard_layout, high_dpi_layout);
        assert_eq!(
            standard_layout.fit_field(grid_size),
            high_dpi_layout.fit_field(grid_size)
        );
        assert!(ViewerLayout::new(Vec2::ZERO).fit_field(grid_size).is_none());
    }
}
