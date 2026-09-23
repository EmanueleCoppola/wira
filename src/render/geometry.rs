#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}
impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

pub const GRID_MM: f32 = 4.0;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GridPoint {
    pub col: i32,
    pub row: i32,
}
impl GridPoint {
    pub const fn new(col: i32, row: i32) -> Self {
        Self { col, row }
    }
    pub fn mm(self) -> Point {
        Point::new(self.col as f32 * GRID_MM, self.row as f32 * GRID_MM)
    }
    pub fn offset(self, other: Self) -> Self {
        Self::new(self.col + other.col, self.row + other.row)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}
impl Bounds {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }
    pub fn translated(self, p: Point) -> Self {
        Self::new(
            self.left + p.x,
            self.top + p.y,
            self.right + p.x,
            self.bottom + p.y,
        )
    }
    pub fn intersects(self, other: Self) -> bool {
        self.left < other.right
            && self.right > other.left
            && self.top < other.bottom
            && self.bottom > other.top
    }
}
