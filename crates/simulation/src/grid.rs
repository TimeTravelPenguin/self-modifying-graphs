use bevy::prelude::*;

/// A lattice site, with columns increasing rightwards and rows upwards.
#[derive(Component, Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct GridCoord {
    pub column: u32,
    pub row: u32,
}

impl GridCoord {
    pub const fn new(column: u32, row: u32) -> Self {
        Self { column, row }
    }
}

/// Converts logical lattice coordinates into positions around the world origin.
#[derive(Resource, Clone, Copy, Debug)]
pub struct GridLayout {
    columns: u32,
    rows: u32,
    /// The distance between the centers of adjacent tiles.
    pitch: f32,
    /// The side length of each square tile.
    tile_side: f32,
    corner_radius: f32,
}

impl GridLayout {
    pub fn new(columns: u32, rows: u32, pitch: f32, tile_side: f32, corner_radius: f32) -> Self {
        assert!(columns > 0 && rows > 0, "grid dimensions must be nonzero");
        assert!(
            pitch.is_finite() && pitch > 0.0,
            "pitch must be positive and finite"
        );
        assert!(
            tile_side.is_finite() && tile_side > 0.0 && tile_side <= pitch,
            "tile side must be positive, finite, and no larger than the pitch"
        );

        assert!(
            corner_radius.is_finite() && corner_radius >= 0.0 && corner_radius <= tile_side / 2.0,
            "corner radius must be finite and between zero and half the tile side"
        );

        Self {
            columns,
            rows,
            pitch,
            tile_side,
            corner_radius,
        }
    }

    pub const fn columns(&self) -> u32 {
        self.columns
    }

    pub const fn rows(&self) -> u32 {
        self.rows
    }

    pub const fn tile_side(&self) -> f32 {
        self.tile_side
    }

    pub const fn corner_radius(&self) -> f32 {
        self.corner_radius
    }

    /// All tile, vertex, and edge positions should use this same mapping.
    pub fn world_position(&self, coord: GridCoord) -> Vec2 {
        let center_column = (self.columns as f32 - 1.0) / 2.0;
        let center_row = (self.rows as f32 - 1.0) / 2.0;

        Vec2::new(
            (coord.column as f32 - center_column) * self.pitch,
            (coord.row as f32 - center_row) * self.pitch,
        )
    }

    /// Includes the visible half-tile extending beyond each outermost site.
    pub fn bounds_size(&self) -> Vec2 {
        Vec2::new(
            (self.columns as f32 - 1.0) * self.pitch + self.tile_side,
            (self.rows as f32 - 1.0) * self.pitch + self.tile_side,
        )
    }

    /// Returns a row-major index, rejecting coordinates outside this lattice.
    pub fn index(&self, coord: GridCoord) -> Option<usize> {
        if coord.column >= self.columns || coord.row >= self.rows {
            return None;
        }

        Some(coord.row as usize * self.columns as usize + coord.column as usize)
    }
}

impl Default for GridLayout {
    fn default() -> Self {
        Self::new(17, 13, 50.0, 44.0, 8.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn odd_grid_has_a_site_at_the_origin_and_symmetric_bounds() {
        let layout = GridLayout::new(3, 5, 10.0, 8.0, 2.0);
        let bottom_left = layout.world_position(GridCoord::new(0, 0));
        let top_right = layout.world_position(GridCoord::new(2, 4));

        assert_eq!(layout.world_position(GridCoord::new(1, 2)), Vec2::ZERO);
        assert_eq!(bottom_left + top_right, Vec2::ZERO);
        assert_eq!(layout.bounds_size(), Vec2::new(28.0, 48.0));
    }

    #[test]
    fn even_grid_straddles_the_origin() {
        let layout = GridLayout::new(4, 2, 10.0, 8.0, 2.0);

        assert_eq!(
            layout.world_position(GridCoord::new(1, 0)),
            Vec2::new(-5.0, -5.0)
        );
        assert_eq!(
            layout.world_position(GridCoord::new(2, 1)),
            Vec2::new(5.0, 5.0)
        );
        assert_eq!(layout.bounds_size(), Vec2::new(38.0, 18.0));
    }

    #[test]
    fn neighbours_are_one_pitch_apart() {
        let layout = GridLayout::default();
        let origin = layout.world_position(GridCoord::new(8, 6));

        assert_eq!(
            layout.world_position(GridCoord::new(9, 6)) - origin,
            Vec2::new(50.0, 0.0)
        );
        assert_eq!(
            layout.world_position(GridCoord::new(8, 7)) - origin,
            Vec2::new(0.0, 50.0)
        );
    }

    #[test]
    fn single_site_is_centered_and_bounds_include_its_tile() {
        let layout = GridLayout::new(1, 1, 50.0, 44.0, 8.0);

        assert_eq!(layout.world_position(GridCoord::new(0, 0)), Vec2::ZERO);
        assert_eq!(layout.bounds_size(), Vec2::splat(44.0));
        assert_eq!(layout.index(GridCoord::new(0, 0)), Some(0));
    }

    #[test]
    fn indices_are_row_major_and_reject_outside_sites() {
        let layout = GridLayout::new(3, 2, 50.0, 44.0, 8.0);

        assert_eq!(layout.index(GridCoord::new(2, 0)), Some(2));
        assert_eq!(layout.index(GridCoord::new(0, 1)), Some(3));
        assert_eq!(layout.index(GridCoord::new(2, 1)), Some(5));
        assert_eq!(layout.index(GridCoord::new(3, 0)), None);
        assert_eq!(layout.index(GridCoord::new(0, 2)), None);
    }
}
