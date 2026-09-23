use std::collections::HashMap;

use crate::mvp_model::{Endpoint, Kind, Page, Project};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Function {
    Main(usize),
    Coil(usize),
    Poles(usize),
}

#[derive(Debug, Clone)]
pub enum Shape {
    Line(f32, f32, f32, f32),
    Rect(f32, f32, f32, f32),
    Circle(f32, f32, f32),
    Text(f32, f32, f32, String),
}

#[derive(Debug, Clone)]
pub struct PageLayout {
    pub title: String,
    pub functions: Vec<Function>,
    pub shapes: Vec<Shape>,
    pub wire_count: usize,
}

type Point = (f32, f32);

pub fn layout(project: &Project) -> Vec<PageLayout> {
    project
        .pages
        .iter()
        .enumerate()
        .map(|(index, page)| layout_page(project, page, index))
        .collect()
}

fn function(project: &Project, endpoint: &Endpoint) -> Function {
    let device = &project.devices[endpoint.device];
    match &device.kind {
        Kind::Contactor { coil, .. }
            if endpoint.terminal == coil.from || endpoint.terminal == coil.to =>
        {
            Function::Coil(endpoint.device)
        }
        Kind::Contactor { .. } => Function::Poles(endpoint.device),
        _ => Function::Main(endpoint.device),
    }
}

fn layout_page(project: &Project, page: &Page, index: usize) -> PageLayout {
    let mut functions = Vec::new();
    for connection in &page.connections {
        for endpoint in [&connection.from, &connection.to] {
            let f = function(project, endpoint);
            if !functions.contains(&f) {
                functions.push(f);
            }
        }
    }
    let mut shapes = vec![
        Shape::Rect(24.0, 24.0, 793.89, 547.28),
        Shape::Line(24.0, 70.0, 817.89, 70.0),
        Shape::Text(40.0, 45.0, 12.0, project.name.clone()),
        Shape::Text(350.0, 45.0, 12.0, page.name.clone()),
        Shape::Text(
            740.0,
            45.0,
            10.0,
            format!("{}/{}", index + 1, project.pages.len()),
        ),
    ];
    let mut pins = HashMap::<Endpoint, Point>::new();
    let count = functions.len().max(1) as f32;
    for (i, f) in functions.iter().enumerate() {
        let x = 130.0 + (i as f32) * 580.0 / (count - 1.0).max(1.0);
        draw_function(project, f, x, &mut shapes, &mut pins);
    }
    for (i, connection) in page.connections.iter().enumerate() {
        let Some(&(x1, y1)) = pins.get(&connection.from) else {
            continue;
        };
        let Some(&(x2, y2)) = pins.get(&connection.to) else {
            continue;
        };
        let backwards = x2 < x1 - 20.0;
        if backwards {
            let via = 126.0 - (i as f32 % 3.0) * 12.0;
            line(&mut shapes, (x1, y1), (x1 + 16.0, y1));
            line(&mut shapes, (x1 + 16.0, y1), (x1 + 16.0, via));
            line(&mut shapes, (x1 + 16.0, via), (x2 + 16.0, via));
            line(&mut shapes, (x2 + 16.0, via), (x2 + 16.0, y2));
            line(&mut shapes, (x2 + 16.0, y2), (x2, y2));
        } else if (y1 - y2).abs() < 1.0 {
            line(&mut shapes, (x1, y1), (x2, y2));
        } else {
            let mid = (x1 + x2) / 2.0;
            line(&mut shapes, (x1, y1), (mid, y1));
            line(&mut shapes, (mid, y1), (mid, y2));
            line(&mut shapes, (mid, y2), (x2, y2));
        }
    }
    PageLayout {
        title: page.name.clone(),
        functions,
        shapes,
        wire_count: page.connections.len(),
    }
}

fn line(shapes: &mut Vec<Shape>, a: Point, b: Point) {
    shapes.push(Shape::Line(a.0, a.1, b.0, b.1));
}
fn label(shapes: &mut Vec<Shape>, x: f32, y: f32, size: f32, text: impl Into<String>) {
    shapes.push(Shape::Text(x, y, size, text.into()));
}
fn pin(
    pins: &mut HashMap<Endpoint, Point>,
    shapes: &mut Vec<Shape>,
    id: usize,
    term: &str,
    x: f32,
    y: f32,
    label_x: f32,
) {
    pins.insert(
        Endpoint {
            device: id,
            terminal: term.into(),
        },
        (x, y),
    );
    shapes.push(Shape::Circle(x, y, 2.5));
    label(shapes, label_x, y + 6.0, 9.0, term);
}

fn draw_function(
    project: &Project,
    f: &Function,
    x: f32,
    shapes: &mut Vec<Shape>,
    pins: &mut HashMap<Endpoint, Point>,
) {
    let id = match f {
        Function::Main(i) | Function::Coil(i) | Function::Poles(i) => *i,
    };
    let d = &project.devices[id];
    match (&d.kind, f) {
        (Kind::PowerSupply { input, output }, _) => {
            shapes.push(Shape::Rect(x - 55.0, 275.0, 110.0, 145.0));
            label(shapes, x - 31.0, 394.0, 12.0, &d.tag);
            label(
                shapes,
                x - 44.0,
                373.0,
                9.0,
                format!("{} / {}", input, output),
            );
            pin(pins, shapes, id, "+", x + 55.0, 345.0, x + 33.0);
            pin(pins, shapes, id, "-", x + 55.0, 305.0, x + 33.0);
        }
        (Kind::Plc { supply, .. }, _) => {
            shapes.push(Shape::Rect(x - 65.0, 245.0, 130.0, 190.0));
            label(shapes, x - 25.0, 409.0, 12.0, &d.tag);
            label(shapes, x - 48.0, 389.0, 9.0, &d.description);
            label(shapes, x - 29.0, 375.0, 8.0, supply);
            pin(pins, shapes, id, "L+", x - 65.0, 345.0, x - 52.0);
            pin(pins, shapes, id, "M", x - 65.0, 305.0, x - 52.0);
            if let Kind::Plc { outputs, .. } = &d.kind {
                for (j, (term, _)) in outputs.iter().enumerate() {
                    pin(
                        pins,
                        shapes,
                        id,
                        term,
                        x + 65.0,
                        345.0 - j as f32 * 30.0,
                        x + 16.0,
                    );
                }
            }
        }
        (Kind::Contactor { coil, .. }, Function::Coil(_)) => {
            label(shapes, x - 22.0, 400.0, 12.0, &d.tag);
            label(shapes, x - 20.0, 380.0, 9.0, &coil.voltage);
            shapes.push(Shape::Circle(x, 325.0, 27.0));
            label(shapes, x - 10.0, 320.0, 11.0, "K");
            line(shapes, (x - 53.0, 325.0), (x - 27.0, 325.0));
            line(shapes, (x + 27.0, 325.0), (x + 53.0, 325.0));
            pin(pins, shapes, id, &coil.from, x - 53.0, 325.0, x - 54.0);
            pin(pins, shapes, id, &coil.to, x + 53.0, 325.0, x + 35.0);
        }
        (Kind::Contactor { poles, .. }, Function::Poles(_)) => {
            label(shapes, x - 20.0, 482.0, 13.0, &d.tag);
            for (j, pole) in poles.iter().enumerate() {
                let y = 430.0 - j as f32 * 72.0;
                pin(pins, shapes, id, &pole.from, x - 70.0, y, x - 92.0);
                pin(pins, shapes, id, &pole.to, x + 70.0, y, x + 52.0);
                line(shapes, (x - 70.0, y), (x - 18.0, y));
                line(shapes, (x - 18.0, y), (x + 22.0, y + 14.0));
                line(shapes, (x + 30.0, y), (x + 70.0, y));
            }
        }
        (Kind::Motor { voltage, power }, _) => {
            shapes.push(Shape::Circle(x + 15.0, 325.0, 48.0));
            label(shapes, x + 7.0, 320.0, 18.0, "M");
            label(shapes, x - 5.0, 479.0, 12.0, &d.tag);
            label(
                shapes,
                x - 12.0,
                466.0,
                9.0,
                format!("{}  {}", power, voltage),
            );
            for (j, t) in ["U", "V", "W", "PE"].iter().enumerate() {
                let y = 430.0 - j as f32 * 72.0;
                pin(pins, shapes, id, t, x - 65.0, y, x - 52.0);
                line(shapes, (x - 65.0, y), (x - 38.0, y));
            }
        }
        (Kind::Supply { voltage }, _) => {
            label(shapes, x - 36.0, 482.0, 12.0, &d.tag);
            label(shapes, x - 37.0, 466.0, 9.0, voltage);
            for (j, t) in ["L1", "L2", "L3", "PE"].iter().enumerate() {
                let y = 430.0 - j as f32 * 72.0;
                label(shapes, x - 37.0, y + 6.0, 10.0, *t);
                line(shapes, (x - 14.0, y), (x + 40.0, y));
                pins.insert(
                    Endpoint {
                        device: id,
                        terminal: (*t).into(),
                    },
                    (x + 40.0, y),
                );
                shapes.push(Shape::Circle(x + 40.0, y, 2.5));
            }
        }
        _ => {}
    }
}
