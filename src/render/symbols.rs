use super::geometry::{Bounds, GridPoint, Point};
use crate::mvp_model::{Device, Kind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SymbolId {
    PowerSupply,
    Plc,
    ContactorCoil,
    ContactorContact,
    Motor,
    ThreePhaseSupply,
    ProtectiveEarth,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection {
    Left,
    Right,
    Up,
    Down,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolPort {
    pub role: String,
    pub anchor: GridPoint,
    pub direction: PortDirection,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyRole {
    DeviceTag,
    Description,
    Rating,
    FunctionText,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PropertyAnchor {
    pub role: PropertyRole,
    pub at: Point,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphicPrimitive {
    Line(Point, Point),
    Rect(Bounds),
    Circle(Point, f32),
}
#[derive(Debug, Clone)]
pub struct SymbolDefinition {
    pub id: SymbolId,
    pub bounds: Bounds,
    pub graphics: Vec<GraphicPrimitive>,
    pub ports: Vec<SymbolPort>,
    pub property_anchors: Vec<PropertyAnchor>,
}
fn port(role: &str, col: i32, row: i32, direction: PortDirection) -> SymbolPort {
    SymbolPort {
        role: role.into(),
        anchor: GridPoint::new(col, row),
        direction,
    }
}
fn anchor(role: PropertyRole, x: f32, y: f32) -> PropertyAnchor {
    PropertyAnchor {
        role,
        at: Point::new(x, y),
    }
}
fn line(a: (f32, f32), b: (f32, f32)) -> GraphicPrimitive {
    GraphicPrimitive::Line(Point::new(a.0, a.1), Point::new(b.0, b.1))
}
fn rect(b: Bounds) -> GraphicPrimitive {
    GraphicPrimitive::Rect(b)
}
fn circle(x: f32, y: f32, r: f32) -> GraphicPrimitive {
    GraphicPrimitive::Circle(Point::new(x, y), r)
}
pub fn id_for(device: &Device, coil: bool) -> SymbolId {
    match device.kind {
        Kind::PowerSupply { .. } => SymbolId::PowerSupply,
        Kind::Plc { .. } => SymbolId::Plc,
        Kind::Contactor { .. } if coil => SymbolId::ContactorCoil,
        Kind::Contactor { .. } => SymbolId::ContactorContact,
        Kind::Motor { .. } => SymbolId::Motor,
        Kind::Supply { .. } => SymbolId::ThreePhaseSupply,
    }
}
pub fn definition(id: SymbolId) -> SymbolDefinition {
    use PortDirection::*;
    use PropertyRole::*;
    use SymbolId::*;
    let (bounds, graphics, ports, property_anchors) = match id {
        PowerSupply => (
            Bounds::new(-12.0, -8.0, 12.0, 8.0),
            vec![
                rect(Bounds::new(-12.0, -8.0, 12.0, 8.0)),
                line((-7.0, 3.0), (7.0, -3.0)),
            ],
            vec![port("+", 3, -1, Right), port("-", 3, 1, Right)],
            vec![anchor(DeviceTag, 0.0, -11.0), anchor(Rating, 0.0, 13.0)],
        ),
        Plc => (
            Bounds::new(-16.0, -12.0, 16.0, 12.0),
            vec![
                rect(Bounds::new(-16.0, -12.0, 16.0, 12.0)),
                line((-16.0, -5.0), (16.0, -5.0)),
            ],
            vec![
                port("L+", -4, -1, Left),
                port("M", -4, 1, Left),
                port("Q0.0", 4, 0, Right),
            ],
            vec![anchor(DeviceTag, 0.0, -8.0), anchor(Description, 0.0, 9.0)],
        ),
        ContactorCoil => (
            Bounds::new(-12.0, -5.0, 12.0, 5.0),
            vec![
                rect(Bounds::new(-8.0, -5.0, 8.0, 5.0)),
                line((-12.0, 0.0), (-8.0, 0.0)),
                line((8.0, 0.0), (12.0, 0.0)),
                line((-3.0, -3.0), (3.0, 3.0)),
                line((-3.0, 3.0), (3.0, -3.0)),
            ],
            vec![port("A1", -3, 0, Left), port("A2", 3, 0, Right)],
            vec![anchor(DeviceTag, 0.0, -9.0), anchor(Rating, 0.0, 10.0)],
        ),
        ContactorContact => (
            Bounds::new(-6.0, -12.0, 6.0, 12.0),
            vec![
                line((0.0, -12.0), (0.0, -4.0)),
                line((0.0, -4.0), (5.0, 3.0)),
                line((0.0, 5.0), (0.0, 12.0)),
            ],
            vec![port("in", 0, -3, Up), port("out", 0, 3, Down)],
            vec![anchor(DeviceTag, 0.0, -15.0)],
        ),
        Motor => (
            Bounds::new(-48.0, -28.0, 53.0, 15.0),
            vec![
                circle(0.0, 0.0, 13.0),
                line((-48.0, -28.0), (-48.0, -19.0)),
                line((-48.0, -19.0), (-10.0, -8.0)),
                line((0.0, -28.0), (0.0, -13.0)),
                line((48.0, -28.0), (48.0, -19.0)),
                line((48.0, -19.0), (10.0, -8.0)),
                line((13.0, 0.0), (48.0, 0.0)),
                line((48.0, 0.0), (48.0, 5.0)),
                line((43.0, 5.0), (53.0, 5.0)),
                line((45.0, 7.0), (51.0, 7.0)),
                line((47.0, 9.0), (49.0, 9.0)),
            ],
            vec![
                port("U", -12, -7, Up),
                port("V", 0, -7, Up),
                port("W", 12, -7, Up),
                port("PE", 12, 0, Right),
            ],
            vec![anchor(DeviceTag, 0.0, 20.0), anchor(Rating, 0.0, 25.0)],
        ),
        ThreePhaseSupply => (
            Bounds::new(-48.0, -5.0, 96.0, 5.0),
            vec![
                line((-48.0, -4.0), (-48.0, 0.0)),
                line((0.0, -4.0), (0.0, 0.0)),
                line((48.0, -4.0), (48.0, 0.0)),
                line((96.0, -4.0), (96.0, 0.0)),
            ],
            vec![
                port("L1", -12, 0, Down),
                port("L2", 0, 0, Down),
                port("L3", 12, 0, Down),
                port("PE", 24, 0, Down),
            ],
            vec![anchor(DeviceTag, 0.0, -10.0), anchor(Rating, 0.0, -6.0)],
        ),
        ProtectiveEarth => (
            Bounds::new(-5.0, -4.0, 5.0, 4.0),
            vec![
                line((0.0, -4.0), (0.0, 0.0)),
                line((-5.0, 0.0), (5.0, 0.0)),
                line((-3.0, 2.0), (3.0, 2.0)),
                line((-1.0, 4.0), (1.0, 4.0)),
            ],
            vec![port("PE", 0, -1, Up)],
            vec![],
        ),
    };
    SymbolDefinition {
        id,
        bounds,
        graphics,
        ports,
        property_anchors,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn built_in_symbols_have_unique_ports_and_valid_bounds() {
        for id in [
            SymbolId::PowerSupply,
            SymbolId::Plc,
            SymbolId::ContactorCoil,
            SymbolId::ContactorContact,
            SymbolId::Motor,
            SymbolId::ThreePhaseSupply,
            SymbolId::ProtectiveEarth,
        ] {
            let def = definition(id);
            assert!(def.bounds.left < def.bounds.right && def.bounds.top < def.bounds.bottom);
            assert!(!def.graphics.is_empty());
            let mut roles = HashSet::new();
            for port in &def.ports {
                assert!(roles.insert(&port.role), "duplicate port in {id:?}");
                let p = port.anchor.mm();
                assert!(p.x >= def.bounds.left && p.x <= def.bounds.right);
                assert!(p.y >= def.bounds.top && p.y <= def.bounds.bottom);
            }
            if id != SymbolId::ProtectiveEarth {
                assert!(def
                    .property_anchors
                    .iter()
                    .any(|a| a.role == PropertyRole::DeviceTag));
            }
        }
    }
}
