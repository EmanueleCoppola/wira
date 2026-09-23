//! Wira electrical schematic compiler.

pub mod mvp_ast;
pub mod mvp_loader;
pub mod mvp_model;
pub mod mvp_parse;
pub mod render;

use std::path::Path;

pub fn compile(entry: &Path) -> Result<mvp_model::Project, String> {
    let declarations = mvp_loader::load(entry)?;
    mvp_model::lower(&declarations)
}

pub fn build(entry: &Path) -> Result<(mvp_model::Project, Vec<u8>), String> {
    let project = compile(entry)?;
    let pages = render::layout(&project);
    if pages
        .iter()
        .zip(&project.pages)
        .any(|(drawing, page)| drawing.wires.len() != page.connections.len())
    {
        return Err("a connection has no port in the built-in symbol library".into());
    }
    if pages.iter().flat_map(|page| &page.symbols).any(|instance| {
        if instance.feeder_index.is_none() {
            return false;
        }
        let bounds = render::symbols::definition(instance.symbol)
            .bounds
            .translated(instance.origin.mm());
        let content = render::frame::CONTENT;
        bounds.left < content.left
            || bounds.right > content.right
            || bounds.top < content.top
            || bounds.bottom > content.bottom
    }) {
        return Err("motor feeder layout exceeds the A4 schematic area".into());
    }
    let bytes = render::pdf::render(&pages)?;
    Ok((project, bytes))
}
