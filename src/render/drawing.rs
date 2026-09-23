use super::{
    geometry::{Bounds, GridPoint, Point},
    symbols::SymbolId,
    topology::PageTopology,
};
use crate::mvp_model::Endpoint;

pub const PAGE_W_MM: f32 = 297.0;
pub const PAGE_H_MM: f32 = 210.0;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}
#[derive(Debug, Clone, PartialEq)]
pub enum DrawCommand {
    Line {
        from: Point,
        to: Point,
        width: f32,
    },
    Rect {
        origin: Point,
        size: Point,
        width: f32,
    },
    Circle {
        center: Point,
        radius: f32,
        width: f32,
        filled: bool,
    },
    Text {
        at: Point,
        size: f32,
        text: String,
        align: TextAlign,
    },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Function {
    Main(usize),
    Coil(usize),
    Poles(usize),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolInstance {
    pub device: usize,
    pub function: Function,
    pub symbol: SymbolId,
    pub origin: GridPoint,
    pub page_index: usize,
    pub pole_index: Option<usize>,
    pub feeder_index: Option<usize>,
    pub zone: String,
}
#[derive(Debug, Clone, PartialEq)]
pub struct PoleGroup {
    pub device: usize,
    pub feeder_index: usize,
    pub members: [usize; 3],
    pub bounds: Bounds,
    pub tag_anchor: Point,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireRoute {
    pub from: Endpoint,
    pub to: Endpoint,
    pub points: Vec<GridPoint>,
}
#[derive(Debug, Clone, PartialEq)]
pub struct PageDrawing {
    pub title: String,
    pub topology: PageTopology,
    pub commands: Vec<DrawCommand>,
    pub symbols: Vec<SymbolInstance>,
    pub pole_groups: Vec<PoleGroup>,
    pub wires: Vec<WireRoute>,
    pub junctions: Vec<Point>,
}
impl PageDrawing {
    pub fn snapshot(&self, project: &crate::mvp_model::Project) -> String {
        let mut s = format!("Page {}\n", self.title);
        for feeder in &self.topology.feeders {
            s.push_str(&format!(
                "MotorFeeder {} -> {} -> {} pitch {}mm\n",
                project.devices[feeder.supply].tag,
                project.devices[feeder.switch].tag,
                project.devices[feeder.motor].tag,
                super::placement::PHASE_PITCH_COLS as f32 * super::geometry::GRID_MM
            ));
        }
        for group in &self.pole_groups {
            s.push_str(&format!(
                "PoleGroup {} feeder {} bounds ({:.0},{:.0})-({:.0},{:.0})\n",
                project.devices[group.device].tag,
                group.feeder_index,
                group.bounds.left,
                group.bounds.top,
                group.bounds.right,
                group.bounds.bottom
            ));
        }
        for i in &self.symbols {
            s.push_str(&format!(
                "Symbol {} {:?} {:?} @ ({},{}) zone {}\n",
                project.devices[i.device].tag,
                i.function,
                i.symbol,
                i.origin.col,
                i.origin.row,
                i.zone
            ));
        }
        for w in &self.wires {
            s.push_str(&format!(
                "Wire {}.{} -> {}.{}",
                project.devices[w.from.device].tag,
                w.from.terminal,
                project.devices[w.to.device].tag,
                w.to.terminal
            ));
            for p in &w.points {
                s.push_str(&format!(" ({},{})", p.col, p.row));
            }
            s.push('\n');
        }
        s
    }
}
