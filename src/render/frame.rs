use super::{
    drawing::{DrawCommand, TextAlign, PAGE_H_MM, PAGE_W_MM},
    geometry::Point,
};
fn p(x: f32, y: f32) -> Point {
    Point::new(x, y)
}
fn line(a: Point, b: Point, width: f32) -> DrawCommand {
    DrawCommand::Line {
        from: a,
        to: b,
        width,
    }
}
fn text(x: f32, y: f32, size: f32, value: impl Into<String>, align: TextAlign) -> DrawCommand {
    DrawCommand::Text {
        at: p(x, y),
        size,
        text: value.into(),
        align,
    }
}
pub fn draw(project: &str, title: &str, index: usize, total: usize) -> Vec<DrawCommand> {
    let mut c = vec![
        DrawCommand::Rect {
            origin: p(8.0, 8.0),
            size: p(PAGE_W_MM - 16.0, PAGE_H_MM - 16.0),
            width: 0.35,
        },
        DrawCommand::Rect {
            origin: p(14.0, 14.0),
            size: p(PAGE_W_MM - 28.0, 165.0),
            width: 0.25,
        },
    ];
    for i in 0..8 {
        let x = 14.0 + (i as f32 + 0.5) * 269.0 / 8.0;
        c.push(text(x, 12.5, 2.0, (i + 1).to_string(), TextAlign::Center));
        c.push(text(x, 184.0, 2.0, (i + 1).to_string(), TextAlign::Center));
    }
    for i in 0..4 {
        let y = 14.0 + (i as f32 + 0.5) * 165.0 / 4.0;
        let ch = (b'A' + i as u8) as char;
        c.push(text(11.0, y, 2.0, ch.to_string(), TextAlign::Center));
        c.push(text(286.0, y, 2.0, ch.to_string(), TextAlign::Center));
    }
    c.push(line(p(14.0, 179.0), p(283.0, 179.0), 0.35));
    c.push(line(p(14.0, 189.0), p(283.0, 189.0), 0.35));
    c.push(line(p(164.0, 189.0), p(164.0, 202.0), 0.25));
    c.push(line(p(240.0, 189.0), p(240.0, 202.0), 0.25));
    c.push(text(17.0, 195.0, 3.1, project, TextAlign::Left));
    c.push(text(
        17.0,
        199.5,
        2.0,
        "Wira electrical schematic",
        TextAlign::Left,
    ));
    c.push(text(167.0, 196.0, 2.8, title, TextAlign::Left));
    c.push(text(
        243.0,
        196.0,
        2.8,
        format!("{} / {}", index + 1, total),
        TextAlign::Left,
    ));
    c
}
