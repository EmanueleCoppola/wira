//! Vector PDF backend. Page layout is expressed as simple drawing operations.
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

pub const PAGE_W: f32 = 841.89;
pub const PAGE_H: f32 = 595.28;

#[derive(Debug, Clone)]
pub enum Op {
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
    Circle {
        x: f32,
        y: f32,
        r: f32,
    },
    Text {
        x: f32,
        y: f32,
        size: f32,
        text: String,
    },
}

pub fn render(pages: &[Vec<Op>]) -> Result<Vec<u8>, String> {
    if pages.is_empty() {
        return Err("project has no pages".into());
    }
    let mut pdf = Pdf::new();
    let catalog = Ref::new(1);
    let tree = Ref::new(2);
    let font = Ref::new(3);
    let font_name = Name(b"F1");
    pdf.catalog(catalog).pages(tree);
    let page_refs: Vec<_> = (0..pages.len())
        .map(|i| Ref::new(4 + i as i32 * 2))
        .collect();
    pdf.pages(tree)
        .kids(page_refs.iter().copied())
        .count(pages.len() as i32);
    pdf.type1_font(font).base_font(Name(b"Helvetica"));
    for (i, ops) in pages.iter().enumerate() {
        let page_id = page_refs[i];
        let content_id = Ref::new(page_id.get() + 1);
        let mut page = pdf.page(page_id);
        page.media_box(Rect::new(0.0, 0.0, PAGE_W, PAGE_H));
        page.parent(tree).contents(content_id);
        page.resources().fonts().pair(font_name, font);
        page.finish();
        let mut content = Content::new();
        content.set_line_width(1.0);
        for op in ops {
            match op {
                Op::Line { x1, y1, x2, y2 } => {
                    check(&[*x1, *y1, *x2, *y2])?;
                    content.move_to(*x1, *y1).line_to(*x2, *y2).stroke();
                }
                Op::Rect { x, y, w, h } => {
                    check(&[*x, *y, *w, *h])?;
                    content.rect(*x, *y, *w, *h).stroke();
                }
                Op::Circle { x, y, r } => {
                    check(&[*x, *y, *r])?;
                    let k = *r * 0.5522848;
                    content
                        .move_to(*x + *r, *y)
                        .cubic_to(*x + *r, *y + k, *x + k, *y + *r, *x, *y + *r)
                        .cubic_to(*x - k, *y + *r, *x - *r, *y + k, *x - *r, *y)
                        .cubic_to(*x - *r, *y - k, *x - k, *y - *r, *x, *y - *r)
                        .cubic_to(*x + k, *y - *r, *x + *r, *y - k, *x + *r, *y)
                        .stroke();
                }
                Op::Text { x, y, size, text } => {
                    check(&[*x, *y, *size])?;
                    let ascii = ascii(text);
                    content
                        .begin_text()
                        .set_font(font_name, *size)
                        .set_text_matrix([1.0, 0.0, 0.0, 1.0, *x, *y])
                        .show(Str(ascii.as_bytes()))
                        .end_text();
                }
            }
        }
        pdf.stream(content_id, &content.finish());
    }
    Ok(pdf.finish())
}

fn check(values: &[f32]) -> Result<(), String> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err("PDF coordinate is not finite".into())
    }
}
fn ascii(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => {
                'a'
            }
            'È' | 'É' | 'Ê' | 'Ë' | 'è' | 'é' | 'ê' | 'ë' => 'e',
            'Ö' | 'Ó' | 'Ò' | 'Ô' | 'ö' | 'ó' | 'ò' | 'ô' => 'o',
            'Ü' | 'Ú' | 'Ù' | 'Û' | 'ü' | 'ú' | 'ù' | 'û' => 'u',
            'ç' | 'Ç' => 'c',
            c if c.is_ascii_graphic() || c == ' ' => c,
            _ => '?',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writes_two_pages() {
        let bytes = render(&[
            vec![Op::Text {
                x: 30.0,
                y: 30.0,
                size: 12.0,
                text: "One".into(),
            }],
            vec![Op::Line {
                x1: 1.0,
                y1: 2.0,
                x2: 3.0,
                y2: 4.0,
            }],
        ])
        .unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        assert_eq!(
            String::from_utf8_lossy(&bytes)
                .matches("/Type /Page\n")
                .count(),
            2
        );
    }
}
