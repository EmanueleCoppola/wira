pub mod drawing;
pub mod frame;
pub mod geometry;
pub mod pdf;
pub mod placement;
pub mod routing;
pub mod symbols;
pub mod topology;

use crate::mvp_model::{Kind, Project};
use drawing::{DrawCommand, PageDrawing, SymbolInstance, TextAlign};
use geometry::Point;
use symbols::{definition, GraphicPrimitive, PropertyRole, SymbolId};

pub fn layout(project: &Project) -> Vec<PageDrawing> {
    project
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| {
            let topology = topology::analyze(project, index);
            let symbols = placement::place(project, page, index, &topology);
            let pole_groups = placement::pole_groups(&symbols, &topology);
            let wires = routing::route_page(project, index, &symbols, &topology);
            let (wire_commands, junctions) = routing::draw_wires(&wires);
            let mut commands = frame::draw(&project.name, &page.name, index, project.pages.len());
            for instance in &symbols {
                draw_symbol(&mut commands, project, instance);
            }
            for group in &pole_groups {
                text(
                    &mut commands,
                    group.tag_anchor.x,
                    group.tag_anchor.y,
                    3.1,
                    &project.devices[group.device].tag,
                    TextAlign::Right,
                );
            }
            commands.extend(wire_commands);
            for p in &junctions {
                commands.push(DrawCommand::Circle {
                    center: *p,
                    radius: 0.65,
                    width: 0.25,
                    filled: true,
                });
            }
            PageDrawing {
                title: page.name.clone(),
                topology,
                commands,
                symbols,
                pole_groups,
                wires,
                junctions,
            }
        })
        .collect()
}
fn text(
    c: &mut Vec<DrawCommand>,
    x: f32,
    y: f32,
    size: f32,
    value: impl Into<String>,
    align: TextAlign,
) {
    c.push(DrawCommand::Text {
        at: Point::new(x, y),
        size,
        text: value.into(),
        align,
    });
}
fn draw_symbol(c: &mut Vec<DrawCommand>, project: &Project, i: &SymbolInstance) {
    let d = &project.devices[i.device];
    let def = definition(i.symbol);
    let o = i.origin.mm();
    let at = |p: Point| Point::new(o.x + p.x, o.y + p.y);
    for g in def.graphics {
        match g {
            GraphicPrimitive::Line(a, b) => c.push(DrawCommand::Line {
                from: at(a),
                to: at(b),
                width: 0.25,
            }),
            GraphicPrimitive::Rect(b) => c.push(DrawCommand::Rect {
                origin: at(Point::new(b.left, b.top)),
                size: Point::new(b.right - b.left, b.bottom - b.top),
                width: 0.25,
            }),
            GraphicPrimitive::Circle(p, r) => c.push(DrawCommand::Circle {
                center: at(p),
                radius: r,
                width: 0.25,
                filled: false,
            }),
        }
    }
    for prop in def.property_anchors {
        let value = match prop.role {
            PropertyRole::DeviceTag if i.symbol == SymbolId::ContactorContact => None,
            PropertyRole::DeviceTag => Some(d.tag.clone()),
            PropertyRole::Description => Some(d.description.clone()),
            PropertyRole::Rating => match &d.kind {
                Kind::PowerSupply { input, output } => Some(format!("{input} / {output}")),
                Kind::Supply { voltage } => Some(voltage.clone()),
                Kind::Contactor { coil, .. } if i.symbol == SymbolId::ContactorCoil => {
                    Some(coil.voltage.clone())
                }
                Kind::Motor { voltage, power } => Some(format!("{power}  {voltage}")),
                _ => None,
            },
            PropertyRole::FunctionText => None,
        };
        if let Some(value) = value {
            text(
                c,
                o.x + prop.at.x,
                o.y + prop.at.y,
                if prop.role == PropertyRole::DeviceTag {
                    3.1
                } else {
                    2.2
                },
                value,
                TextAlign::Center,
            );
        }
    }
    match i.symbol {
        SymbolId::Plc => {
            text(c, o.x, o.y + 1.0, 2.0, "24VDC", TextAlign::Center);
            if let Kind::Plc { outputs, .. } = &d.kind {
                if let Some((_, desc)) = outputs.first() {
                    text(c, o.x + 22.0, o.y - 3.0, 2.0, desc, TextAlign::Left);
                }
            }
        }
        SymbolId::Motor => {
            text(c, o.x, o.y + 1.0, 4.0, "M", TextAlign::Center);
            text(c, o.x, o.y + 6.0, 2.5, "3~", TextAlign::Center);
        }
        SymbolId::ContactorContact if i.feeder_index.is_none() => {
            if let Kind::Contactor { poles, .. } = &d.kind {
                if i.pole_index == Some(poles.len() / 2) {
                    text(c, o.x + 9.0, o.y, 3.1, &d.tag, TextAlign::Left);
                }
            }
        }
        _ => {}
    }
    let ports = def.ports;
    for port in ports {
        let actual = if i.symbol == SymbolId::ContactorContact {
            if let Kind::Contactor { poles, .. } = &d.kind {
                if let Some(p) = i.pole_index.and_then(|n| poles.get(n)) {
                    if port.role == "in" {
                        p.from.clone()
                    } else {
                        p.to.clone()
                    }
                } else {
                    continue;
                }
            } else {
                continue;
            }
        } else {
            port.role.clone()
        };
        let p = i.origin.offset(port.anchor).mm();
        match port.direction {
            symbols::PortDirection::Left => {
                text(c, p.x + 1.2, p.y - 1.2, 2.1, actual, TextAlign::Left)
            }
            symbols::PortDirection::Right => {
                text(c, p.x - 1.2, p.y - 1.2, 2.1, actual, TextAlign::Right)
            }
            symbols::PortDirection::Up if i.symbol == SymbolId::ContactorContact => {
                text(c, p.x - 1.2, p.y - 1.0, 1.9, actual, TextAlign::Right)
            }
            symbols::PortDirection::Down if i.symbol == SymbolId::ContactorContact => {
                text(c, p.x - 1.2, p.y + 1.8, 1.9, actual, TextAlign::Right)
            }
            symbols::PortDirection::Up | symbols::PortDirection::Down => {
                text(c, p.x + 1.2, p.y + 0.8, 1.9, actual, TextAlign::Left)
            }
        }
    }
}
