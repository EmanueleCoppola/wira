use std::collections::{HashMap, HashSet};

use crate::mvp_ast::{Decl, DeviceKind, Field, SourceLoc, TerminalRef};

#[derive(Debug, Clone)]
pub struct Project {
    pub name: String,
    pub devices: Vec<Device>,
    pub pages: Vec<Page>,
}

#[derive(Debug, Clone)]
pub struct Device {
    pub tag: String,
    pub description: String,
    pub kind: Kind,
    pub terminals: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum Kind {
    PowerSupply {
        input: String,
        output: String,
    },
    Supply {
        voltage: String,
    },
    Plc {
        supply: String,
        outputs: Vec<(String, String)>,
    },
    Contactor {
        coil: Coil,
        poles: Vec<Pole>,
    },
    Motor {
        voltage: String,
        power: String,
    },
}

#[derive(Debug, Clone)]
pub struct Coil {
    pub from: String,
    pub to: String,
    pub voltage: String,
}

#[derive(Debug, Clone)]
pub struct Pole {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone)]
pub struct Page {
    pub name: String,
    pub connections: Vec<Connection>,
}

#[derive(Debug, Clone)]
pub struct Connection {
    pub from: Endpoint,
    pub to: Endpoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Endpoint {
    pub device: usize,
    pub terminal: String,
}

pub fn lower(decls: &[Decl]) -> Result<Project, String> {
    let mut name = None;
    let mut devices = Vec::new();
    let mut tags = HashMap::new();
    let mut page_decls = Vec::new();
    for decl in decls {
        match decl {
            Decl::Project { name: n, loc } => {
                if name.replace(n.clone()).is_some() {
                    return Err(err(loc, "duplicate project declaration"));
                }
            }
            Decl::Device(d) => {
                if tags.contains_key(&d.tag) {
                    return Err(err(&d.loc, &format!("duplicate device tag `{}`", d.tag)));
                }
                let device = lower_device(d)?;
                tags.insert(d.tag.clone(), devices.len());
                devices.push(device);
            }
            Decl::Page(p) => page_decls.push(p),
            Decl::Include { .. } => return Err("unexpanded include declaration".into()),
        }
    }
    let name = name.ok_or("project declaration is missing")?;
    if page_decls.is_empty() {
        return Err("at least one page declaration is required".into());
    }
    let mut pages = Vec::new();
    let mut page_names = HashSet::new();
    for p in page_decls {
        if !page_names.insert(p.name.clone()) {
            return Err(err(&p.loc, &format!("duplicate page `{}`", p.name)));
        }
        let mut connections = Vec::new();
        for c in &p.connections {
            connections.push(Connection {
                from: resolve(&c.from, &devices, &tags)?,
                to: resolve(&c.to, &devices, &tags)?,
            });
        }
        pages.push(Page {
            name: p.name.clone(),
            connections,
        });
    }
    Ok(Project {
        name,
        devices,
        pages,
    })
}

fn err(loc: &SourceLoc, message: &str) -> String {
    format!(
        "{}:{}:{}: {message}",
        loc.path.display(),
        loc.line,
        loc.column
    )
}

fn value(fields: &[Field], key: &str, loc: &SourceLoc) -> Result<String, String> {
    let values: Vec<_> = fields
        .iter()
        .filter_map(|f| match f {
            Field::KeyValue { key: k, value, .. } if k == key => Some(value.clone()),
            _ => None,
        })
        .collect();
    match values.as_slice() {
        [v] => Ok(v.clone()),
        [] => Err(err(loc, &format!("missing `{key}` field"))),
        _ => Err(err(loc, &format!("duplicate `{key}` field"))),
    }
}

fn lower_device(d: &crate::mvp_ast::DeviceDecl) -> Result<Device, String> {
    for field in &d.fields {
        let (valid, loc) = match field {
            Field::KeyValue { key, loc, .. } => {
                let keys: &[&str] = match d.kind {
                    DeviceKind::PowerSupply => &["input", "output"],
                    DeviceKind::Supply => &["voltage", "phases"],
                    DeviceKind::Plc => &["supply"],
                    DeviceKind::Motor => &["voltage", "phases", "power"],
                    DeviceKind::Contactor => &[],
                };
                (keys.contains(&key.as_str()), loc)
            }
            Field::PlcOutput { loc, .. } => (d.kind == DeviceKind::Plc, loc),
            Field::Coil { loc, .. } | Field::Pole { loc, .. } => {
                (d.kind == DeviceKind::Contactor, loc)
            }
        };
        if !valid {
            return Err(err(loc, "field is not valid for this device type"));
        }
    }
    let (kind, terminals) = match d.kind {
        DeviceKind::PowerSupply => {
            let input = value(&d.fields, "input", &d.loc)?;
            let output = value(&d.fields, "output", &d.loc)?;
            (
                Kind::PowerSupply { input, output },
                vec!["L", "N", "PE", "+", "-"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
            )
        }
        DeviceKind::Supply => {
            let voltage = value(&d.fields, "voltage", &d.loc)?;
            if value(&d.fields, "phases", &d.loc)? != "3" {
                return Err(err(&d.loc, "only three-phase supplies are supported"));
            }
            (
                Kind::Supply { voltage },
                vec!["L1", "L2", "L3", "PE"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
            )
        }
        DeviceKind::Plc => {
            let supply = value(&d.fields, "supply", &d.loc)?;
            if supply != "24VDC" {
                return Err(err(&d.loc, "only 24VDC PLC supplies are supported"));
            }
            let mut terminals = vec!["L+".into(), "M".into()];
            let mut outputs = Vec::new();
            for f in &d.fields {
                if let Field::PlcOutput {
                    name,
                    description,
                    loc,
                } = f
                {
                    if terminals.contains(name) {
                        return Err(err(loc, &format!("duplicate terminal `{name}`")));
                    }
                    terminals.push(name.clone());
                    outputs.push((name.clone(), description.clone()));
                }
            }
            (Kind::Plc { supply, outputs }, terminals)
        }
        DeviceKind::Contactor => {
            let mut coil = None;
            let mut poles = Vec::new();
            let mut terminals = Vec::new();
            for f in &d.fields {
                match f {
                    Field::Coil {
                        from,
                        to,
                        voltage,
                        loc,
                    } => {
                        if coil.is_some() {
                            return Err(err(loc, "duplicate coil definition"));
                        }
                        add_pair(&mut terminals, from, to, loc)?;
                        coil = Some(Coil {
                            from: from.clone(),
                            to: to.clone(),
                            voltage: voltage.clone(),
                        });
                    }
                    Field::Pole { from, to, loc } => {
                        add_pair(&mut terminals, from, to, loc)?;
                        poles.push(Pole {
                            from: from.clone(),
                            to: to.clone(),
                        });
                    }
                    _ => return Err(err(&d.loc, "invalid contactor field")),
                }
            }
            let coil = coil.ok_or_else(|| err(&d.loc, "contactor needs a coil"))?;
            if poles.is_empty() {
                return Err(err(&d.loc, "contactor needs at least one pole"));
            }
            (Kind::Contactor { coil, poles }, terminals)
        }
        DeviceKind::Motor => {
            let voltage = value(&d.fields, "voltage", &d.loc)?;
            if value(&d.fields, "phases", &d.loc)? != "3" {
                return Err(err(&d.loc, "only three-phase motors are supported"));
            }
            let power = value(&d.fields, "power", &d.loc)?;
            (
                Kind::Motor { voltage, power },
                vec!["U", "V", "W", "PE"]
                    .into_iter()
                    .map(String::from)
                    .collect(),
            )
        }
    };
    Ok(Device {
        tag: d.tag.clone(),
        description: d.description.clone(),
        kind,
        terminals,
    })
}

fn add_pair(terminals: &mut Vec<String>, a: &str, b: &str, loc: &SourceLoc) -> Result<(), String> {
    if a == b || terminals.iter().any(|t| t == a || t == b) {
        return Err(err(
            loc,
            &format!("duplicate contactor terminal in `{a} -> {b}`"),
        ));
    }
    terminals.push(a.into());
    terminals.push(b.into());
    Ok(())
}

fn resolve(
    reference: &TerminalRef,
    devices: &[Device],
    tags: &HashMap<String, usize>,
) -> Result<Endpoint, String> {
    let Some(&device) = tags.get(&reference.device) else {
        return Err(err(
            &reference.loc,
            &format!("unknown device `{}`", reference.device),
        ));
    };
    if !devices[device].terminals.contains(&reference.terminal) {
        return Err(err(
            &reference.loc,
            &format!(
                "unknown terminal `{}.{}`; available: {}",
                reference.device,
                reference.terminal,
                devices[device].terminals.join(", ")
            ),
        ));
    }
    Ok(Endpoint {
        device,
        terminal: reference.terminal.clone(),
    })
}
