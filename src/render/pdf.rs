//! Vector PDF output for millimeter-based page drawings.
use std::collections::BTreeMap;

use pdf_writer::types::{CidFontType, FontFlags, SystemInfo, UnicodeCmap};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

use super::{
    drawing::{DrawCommand, PageDrawing, TextAlign, PAGE_H_MM, PAGE_W_MM},
    geometry::Point,
};

const FONT: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const PT_PER_MM: f32 = 72.0 / 25.4;
const FONT_NAME: Name = Name(b"NotoSans-Regular");
const RESOURCE_NAME: Name = Name(b"F1");

pub fn render(pages: &[PageDrawing]) -> Result<Vec<u8>, String> {
    if pages.is_empty() {
        return Err("project has no pages".into());
    }
    let font = TrueType::new(FONT)?;
    let mut chars = BTreeMap::<char, (u16, u16, f32)>::new();
    for page in pages {
        for command in &page.commands {
            if let DrawCommand::Text { text, .. } = command {
                for ch in text.chars() {
                    if chars.contains_key(&ch) {
                        continue;
                    }
                    let gid = font.glyph(ch).ok_or_else(|| {
                        format!("embedded font has no glyph for U+{:04X} ({ch})", ch as u32)
                    })?;
                    let cid = u16::try_from(chars.len() + 1)
                        .map_err(|_| "too many distinct text characters".to_string())?;
                    chars.insert(ch, (cid, gid, font.advance(gid)));
                }
            }
        }
    }

    let mut pdf = Pdf::new();
    let catalog = Ref::new(1);
    let tree = Ref::new(2);
    let type0 = Ref::new(3);
    let cid_font = Ref::new(4);
    let descriptor = Ref::new(5);
    let font_file = Ref::new(6);
    let cid_map = Ref::new(7);
    let unicode_map = Ref::new(8);
    pdf.catalog(catalog).pages(tree);
    let page_refs: Vec<_> = (0..pages.len())
        .map(|i| Ref::new(9 + i as i32 * 2))
        .collect();
    pdf.pages(tree)
        .kids(page_refs.iter().copied())
        .count(pages.len() as i32);

    pdf.type0_font(type0)
        .base_font(FONT_NAME)
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(cid_font)
        .to_unicode(unicode_map);
    let info = SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    };
    let mut cid = pdf.cid_font(cid_font);
    cid.subtype(CidFontType::Type2)
        .base_font(FONT_NAME)
        .system_info(info)
        .font_descriptor(descriptor)
        .cid_to_gid_map_stream(cid_map)
        .default_width(1000.0);
    if !chars.is_empty() {
        let mut by_cid: Vec<_> = chars
            .iter()
            .map(|(&ch, &(id, gid, width))| (id, gid, width, ch))
            .collect();
        by_cid.sort_by_key(|v| v.0);
        cid.widths().consecutive(1, by_cid.iter().map(|v| v.2));
        drop(cid);
        let mut map = vec![0u8; (by_cid.len() + 1) * 2];
        let mut unicode = UnicodeCmap::new(Name(b"WiraToUnicode"), info);
        for &(id, gid, _, ch) in &by_cid {
            map[id as usize * 2..id as usize * 2 + 2].copy_from_slice(&gid.to_be_bytes());
            unicode.pair(id, ch);
        }
        pdf.stream(cid_map, &map);
        pdf.cmap(unicode_map, &unicode.finish())
            .name(Name(b"WiraToUnicode"))
            .system_info(info);
    } else {
        drop(cid);
        pdf.stream(cid_map, &[0, 0]);
        pdf.cmap(
            unicode_map,
            &UnicodeCmap::<u16>::new(Name(b"WiraToUnicode"), info).finish(),
        )
        .name(Name(b"WiraToUnicode"))
        .system_info(info);
    }
    let scale = 1000.0 / font.units_per_em as f32;
    pdf.font_descriptor(descriptor)
        .name(FONT_NAME)
        .flags(FontFlags::NON_SYMBOLIC)
        .bbox(Rect::new(
            font.i16_at(font.head + 36)? as f32 * scale,
            font.i16_at(font.head + 38)? as f32 * scale,
            font.i16_at(font.head + 40)? as f32 * scale,
            font.i16_at(font.head + 42)? as f32 * scale,
        ))
        .italic_angle(0.0)
        .ascent(font.i16_at(font.hhea + 4)? as f32 * scale)
        .descent(font.i16_at(font.hhea + 6)? as f32 * scale)
        .cap_height(714.0)
        .stem_v(80.0)
        .font_file2(font_file);
    pdf.stream(font_file, FONT)
        .pair(Name(b"Length1"), FONT.len() as i32);

    for (index, page) in pages.iter().enumerate() {
        let page_id = page_refs[index];
        let content_id = Ref::new(page_id.get() + 1);
        let mut writer = pdf.page(page_id);
        writer.media_box(Rect::new(
            0.0,
            0.0,
            PAGE_W_MM * PT_PER_MM,
            PAGE_H_MM * PT_PER_MM,
        ));
        writer.parent(tree).contents(content_id);
        writer.resources().fonts().pair(RESOURCE_NAME, type0);
        writer.finish();
        let mut content = Content::new();
        content.transform([PT_PER_MM, 0.0, 0.0, -PT_PER_MM, 0.0, PAGE_H_MM * PT_PER_MM]);
        for command in &page.commands {
            draw(&mut content, command, &chars)?;
        }
        pdf.stream(content_id, &content.finish());
    }
    Ok(pdf.finish())
}

fn draw(
    content: &mut Content,
    command: &DrawCommand,
    chars: &BTreeMap<char, (u16, u16, f32)>,
) -> Result<(), String> {
    match command {
        DrawCommand::Line { from, to, width } => {
            point(*from)?;
            point(*to)?;
            positive(*width)?;
            content
                .set_line_width(*width)
                .move_to(from.x, from.y)
                .line_to(to.x, to.y)
                .stroke();
        }
        DrawCommand::Rect {
            origin,
            size,
            width,
        } => {
            point(*origin)?;
            point(*size)?;
            positive(*width)?;
            content
                .set_line_width(*width)
                .rect(origin.x, origin.y, size.x, size.y)
                .stroke();
        }
        DrawCommand::Circle {
            center,
            radius,
            width,
            filled,
        } => {
            point(*center)?;
            positive(*radius)?;
            positive(*width)?;
            let (x, y, r, k) = (center.x, center.y, *radius, *radius * 0.5522848);
            content
                .set_line_width(*width)
                .move_to(x + r, y)
                .cubic_to(x + r, y + k, x + k, y + r, x, y + r)
                .cubic_to(x - k, y + r, x - r, y + k, x - r, y)
                .cubic_to(x - r, y - k, x - k, y - r, x, y - r)
                .cubic_to(x + k, y - r, x + r, y - k, x + r, y);
            if *filled {
                content.fill_nonzero();
            } else {
                content.stroke();
            }
        }
        DrawCommand::Text {
            at,
            size,
            text,
            align,
        } => {
            point(*at)?;
            positive(*size)?;
            let mut encoded = Vec::with_capacity(text.len() * 2);
            let mut width = 0.0;
            for ch in text.chars() {
                let &(cid, _, advance) = chars
                    .get(&ch)
                    .ok_or_else(|| format!("missing encoded character {ch}"))?;
                encoded.extend_from_slice(&cid.to_be_bytes());
                width += advance * *size / 1000.0;
            }
            let x = at.x
                - match align {
                    TextAlign::Left => 0.0,
                    TextAlign::Center => width / 2.0,
                    TextAlign::Right => width,
                };
            content
                .begin_text()
                .set_font(RESOURCE_NAME, *size)
                .set_text_matrix([1.0, 0.0, 0.0, -1.0, x, at.y])
                .show(Str(&encoded))
                .end_text();
        }
    }
    Ok(())
}

fn point(p: Point) -> Result<(), String> {
    if p.x.is_finite() && p.y.is_finite() {
        Ok(())
    } else {
        Err("PDF coordinate is not finite".into())
    }
}
fn positive(v: f32) -> Result<(), String> {
    if v.is_finite() && v >= 0.0 {
        Ok(())
    } else {
        Err("PDF size or width is invalid".into())
    }
}

struct TrueType<'a> {
    data: &'a [u8],
    head: usize,
    hhea: usize,
    hmtx: usize,
    cmap: usize,
    units_per_em: u16,
    hmetrics: u16,
}
impl<'a> TrueType<'a> {
    fn new(data: &'a [u8]) -> Result<Self, String> {
        let mut font = Self {
            data,
            head: 0,
            hhea: 0,
            hmtx: 0,
            cmap: 0,
            units_per_em: 0,
            hmetrics: 0,
        };
        font.head = font.table(b"head")?;
        font.hhea = font.table(b"hhea")?;
        font.hmtx = font.table(b"hmtx")?;
        let cmap = font.table(b"cmap")?;
        let count = font.u16_at(cmap + 2)? as usize;
        font.cmap = (0..count)
            .find_map(|i| {
                let at = cmap + 4 + i * 8;
                let platform = font.u16_at(at).ok()?;
                let encoding = font.u16_at(at + 2).ok()?;
                let offset = font.u32_at(at + 4).ok()? as usize;
                ((platform, encoding) == (3, 1) && font.u16_at(cmap + offset).ok()? == 4)
                    .then_some(cmap + offset)
            })
            .ok_or("embedded font has no Unicode cmap")?;
        font.units_per_em = font.u16_at(font.head + 18)?;
        font.hmetrics = font.u16_at(font.hhea + 34)?;
        if font.units_per_em == 0 || font.hmetrics == 0 {
            return Err("invalid embedded font metrics".into());
        }
        Ok(font)
    }
    fn table(&self, tag: &[u8; 4]) -> Result<usize, String> {
        let count = self.u16_at(4)? as usize;
        for i in 0..count {
            let at = 12 + i * 16;
            if self.data.get(at..at + 4) == Some(tag) {
                return Ok(self.u32_at(at + 8)? as usize);
            }
        }
        Err(format!(
            "embedded font lacks {} table",
            String::from_utf8_lossy(tag)
        ))
    }
    fn u16_at(&self, at: usize) -> Result<u16, String> {
        let bytes: [u8; 2] = self
            .data
            .get(at..at + 2)
            .ok_or("truncated embedded font")?
            .try_into()
            .unwrap();
        Ok(u16::from_be_bytes(bytes))
    }
    fn i16_at(&self, at: usize) -> Result<i16, String> {
        Ok(self.u16_at(at)? as i16)
    }
    fn u32_at(&self, at: usize) -> Result<u32, String> {
        let bytes: [u8; 4] = self
            .data
            .get(at..at + 4)
            .ok_or("truncated embedded font")?
            .try_into()
            .unwrap();
        Ok(u32::from_be_bytes(bytes))
    }
    fn glyph(&self, ch: char) -> Option<u16> {
        let code = u16::try_from(ch as u32).ok()?;
        let n = self.u16_at(self.cmap + 6).ok()? as usize / 2;
        let ends = self.cmap + 14;
        for i in 0..n {
            if code > self.u16_at(ends + i * 2).ok()? {
                continue;
            }
            let start = self.u16_at(ends + 2 + 2 * n + i * 2).ok()?;
            if code < start {
                return None;
            }
            let delta = self.u16_at(ends + 2 + 4 * n + i * 2).ok()?;
            let offset_at = ends + 2 + 6 * n + i * 2;
            let offset = self.u16_at(offset_at).ok()? as usize;
            let gid = if offset == 0 {
                code.wrapping_add(delta)
            } else {
                let raw = self
                    .u16_at(offset_at + offset + 2 * (code - start) as usize)
                    .ok()?;
                if raw == 0 {
                    0
                } else {
                    raw.wrapping_add(delta)
                }
            };
            return (gid != 0).then_some(gid);
        }
        None
    }
    fn advance(&self, gid: u16) -> f32 {
        let metric = gid.min(self.hmetrics - 1) as usize;
        self.u16_at(self.hmtx + 4 * metric).unwrap_or(1000) as f32 * 1000.0
            / self.units_per_em as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_unicode_text_and_a4_page() {
        let page = PageDrawing {
            title: "Résumé Ω".into(),
            commands: vec![DrawCommand::Text {
                at: Point::new(20.0, 20.0),
                size: 3.0,
                text: "Résumé Ω".into(),
                align: TextAlign::Center,
            }],
            symbols: vec![],
            wires: vec![],
            junctions: vec![],
        };
        let bytes = render(&[page]).unwrap();
        assert!(bytes.starts_with(b"%PDF-"));
        assert!(bytes
            .windows(b"/FontFile2".len())
            .any(|w| w == b"/FontFile2"));
        assert!(bytes
            .windows(b"/ToUnicode".len())
            .any(|w| w == b"/ToUnicode"));
        assert!(bytes.windows(b"00E9".len()).any(|w| w == b"00E9"));
        assert!(bytes.windows(b"03A9".len()).any(|w| w == b"03A9"));
    }
}
