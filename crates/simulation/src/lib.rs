pub mod simulation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

impl Position {
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}

impl From<(usize, usize)> for Position {
    fn from(tuple: (usize, usize)) -> Self {
        Self {
            x: tuple.0,
            y: tuple.1,
        }
    }
}
