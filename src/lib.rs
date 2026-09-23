//! Wira electrical schematic compiler.

pub mod mvp_ast;
pub mod mvp_layout;
pub mod mvp_loader;
pub mod mvp_model;
pub mod mvp_parse;
pub mod mvp_pdf;

use std::path::Path;

pub fn compile(entry: &Path) -> Result<mvp_model::Project, String> {
    let declarations = mvp_loader::load(entry)?;
    mvp_model::lower(&declarations)
}

pub fn build(entry: &Path) -> Result<(mvp_model::Project, Vec<u8>), String> {
    let project = compile(entry)?;
    let pages = mvp_layout::layout(&project);
    let pdf_pages: Vec<_> = pages
        .iter()
        .map(|page| {
            page.shapes
                .iter()
                .map(|shape| match shape {
                    mvp_layout::Shape::Line(x1, y1, x2, y2) => mvp_pdf::Op::Line {
                        x1: *x1,
                        y1: *y1,
                        x2: *x2,
                        y2: *y2,
                    },
                    mvp_layout::Shape::Rect(x, y, w, h) => mvp_pdf::Op::Rect {
                        x: *x,
                        y: *y,
                        w: *w,
                        h: *h,
                    },
                    mvp_layout::Shape::Circle(x, y, r) => mvp_pdf::Op::Circle {
                        x: *x,
                        y: *y,
                        r: *r,
                    },
                    mvp_layout::Shape::Text(x, y, size, text) => mvp_pdf::Op::Text {
                        x: *x,
                        y: *y,
                        size: *size,
                        text: text.clone(),
                    },
                })
                .collect()
        })
        .collect();
    let bytes = mvp_pdf::render(&pdf_pages)?;
    Ok((project, bytes))
}
