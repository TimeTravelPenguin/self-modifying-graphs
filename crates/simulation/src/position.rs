/// A zero-based coordinate measured in lattice cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    /// Column index.
    pub x: usize,
    /// Row index.
    pub y: usize,
}

impl Position {
    /// Creates a position from its column and row indices.
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

impl From<(usize, usize)> for Position {
    fn from((x, y): (usize, usize)) -> Self {
        Self { x, y }
    }
}
