use crate::mvp_ast::{Decl, SourceLoc};
use crate::mvp_parse;
use std::fs;
use std::path::{Path, PathBuf};

pub fn load(entry: &Path) -> Result<Vec<Decl>, String> {
    load_file(entry, &mut Vec::new(), None)
}

fn load_file(
    path: &Path,
    stack: &mut Vec<PathBuf>,
    include_loc: Option<&SourceLoc>,
) -> Result<Vec<Decl>, String> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        if let Some(loc) = include_loc {
            format!(
                "{}:{}:{}: cannot read include `{}`: {}",
                loc.path.display(),
                loc.line,
                loc.column,
                path.display(),
                error
            )
        } else {
            format!("{}:1:1: cannot read file: {}", path.display(), error)
        }
    })?;
    if let Some(index) = stack.iter().position(|item| item == &canonical) {
        let chain = stack[index..]
            .iter()
            .chain(std::iter::once(&canonical))
            .map(|item| item.display().to_string())
            .collect::<Vec<_>>()
            .join(" -> ");
        return Err(if let Some(loc) = include_loc {
            format!(
                "{}:{}:{}: include cycle detected: {}",
                loc.path.display(),
                loc.line,
                loc.column,
                chain
            )
        } else {
            format!("{}:1:1: include cycle detected: {chain}", path.display())
        });
    }

    let source = fs::read_to_string(&canonical).map_err(|error| {
        if let Some(loc) = include_loc {
            format!(
                "{}:{}:{}: cannot read include `{}`: {}",
                loc.path.display(),
                loc.line,
                loc.column,
                path.display(),
                error
            )
        } else {
            format!("{}:1:1: cannot read file: {}", canonical.display(), error)
        }
    })?;
    let declarations = mvp_parse::parse(&source, &canonical)?;

    stack.push(canonical.clone());
    let mut expanded = Vec::new();
    for declaration in declarations {
        match declaration {
            Decl::Include { path: child, loc } => {
                let child_path = PathBuf::from(&child);
                let resolved = if child_path.is_absolute() {
                    child_path
                } else {
                    canonical
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(child_path)
                };
                expanded.extend(load_file(&resolved, stack, Some(&loc))?);
            }
            other => expanded.push(other),
        }
    }
    stack.pop();
    Ok(expanded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mvp_ast::Decl;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn expands_nested_includes_in_place_and_detects_cycles() {
        let root = std::env::temp_dir().join(format!(
            "wira-mvp-loader-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(
            root.join("main.wira"),
            "project \"Main\"\ninclude \"nested/one.wira\"\nsupply After \"after\" {}\n",
        )
        .unwrap();
        fs::write(
            root.join("nested/one.wira"),
            "supply Before \"before\" {}\ninclude \"two.wira\"\n",
        )
        .unwrap();
        fs::write(root.join("nested/two.wira"), "motor M \"motor\" {}\n").unwrap();

        let declarations = load(&root.join("main.wira")).unwrap();
        let tags = declarations
            .iter()
            .filter_map(|decl| match decl {
                Decl::Device(device) => Some(device.tag.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(tags, ["Before", "M", "After"]);

        fs::write(root.join("nested/two.wira"), "include \"one.wira\"\n").unwrap();
        let error = load(&root.join("main.wira")).unwrap_err();
        assert!(error.contains("include cycle detected"), "{error}");
        assert!(error.contains("two.wira"), "{error}");

        fs::remove_dir_all(root).unwrap();
    }
}
