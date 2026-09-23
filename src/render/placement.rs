use super::{
    drawing::{Function, SymbolInstance},
    geometry::GridPoint,
    symbols::{definition, id_for, SymbolId},
};
use crate::mvp_model::{Endpoint, Kind, Page, Project};
use std::collections::HashSet;

pub fn select(project: &Project, page: &Page) -> Vec<Function> {
    let mut functions = Vec::new();
    for connection in &page.connections {
        for endpoint in [&connection.from, &connection.to] {
            let f = match &project.devices[endpoint.device].kind {
                Kind::Contactor { coil, .. }
                    if endpoint.terminal == coil.from || endpoint.terminal == coil.to =>
                {
                    Function::Coil(endpoint.device)
                }
                Kind::Contactor { .. } => Function::Poles(endpoint.device),
                _ => Function::Main(endpoint.device),
            };
            if !functions.contains(&f) {
                functions.push(f)
            }
        }
    }
    functions
}
fn zone(p: GridPoint) -> String {
    let col = ((p.col * 4 - 12).max(0) / 34 + 1).min(8);
    let row = ((p.row * 4 - 12).max(0) / 42).min(3);
    format!("{}{}", (b'A' + row as u8) as char, col)
}
fn instance(
    device: usize,
    function: Function,
    symbol: SymbolId,
    origin: GridPoint,
    page_index: usize,
    pole_index: Option<usize>,
) -> SymbolInstance {
    SymbolInstance {
        device,
        function,
        symbol,
        origin,
        page_index,
        pole_index,
        zone: zone(origin),
    }
}

pub fn place(project: &Project, page: &Page, page_index: usize) -> Vec<SymbolInstance> {
    let functions = select(project, page);
    let control = functions.iter().any(|f| matches!(f, Function::Coil(_)))
        && functions.iter().any(
            |f| matches!(f,Function::Main(i) if matches!(project.devices[*i].kind,Kind::Plc{..})),
        );
    let power = functions.iter().any(|f| matches!(f, Function::Poles(_)))
        && functions.iter().any(
            |f| matches!(f,Function::Main(i) if matches!(project.devices[*i].kind,Kind::Motor{..})),
        );
    let mut result = Vec::new();
    let mut fallback_col = 10;
    let mut seen = HashSet::new();
    for f in functions {
        let device = match f {
            Function::Main(i) | Function::Coil(i) | Function::Poles(i) => i,
        };
        if !seen.insert(f) {
            continue;
        }
        let d = &project.devices[device];
        if power && matches!(f, Function::Poles(_)) {
            if let Kind::Contactor { poles, .. } = &d.kind {
                for (n, _) in poles.iter().enumerate() {
                    let origin = GridPoint::new(25 + n as i32 * 12, 22);
                    result.push(instance(
                        device,
                        f,
                        SymbolId::ContactorContact,
                        origin,
                        page_index,
                        Some(n),
                    ));
                }
            }
            continue;
        }
        let symbol = id_for(d, matches!(f, Function::Coil(_)));
        let origin = if control {
            match symbol {
                SymbolId::PowerSupply => GridPoint::new(10, 22),
                SymbolId::Plc => GridPoint::new(31, 22),
                SymbolId::ContactorCoil => GridPoint::new(53, 22),
                _ => {
                    let x = fallback_col;
                    fallback_col += 15;
                    GridPoint::new(x, 22)
                }
            }
        } else if power {
            match symbol {
                SymbolId::ThreePhaseSupply => GridPoint::new(37, 12),
                SymbolId::Motor => GridPoint::new(37, 37),
                _ => {
                    let x = fallback_col;
                    fallback_col += 15;
                    GridPoint::new(x, 22)
                }
            }
        } else {
            let x = fallback_col;
            fallback_col += 15;
            GridPoint::new(x, 23)
        };
        result.push(instance(device, f, symbol, origin, page_index, None));
    }
    result
}

pub fn port(
    instances: &[SymbolInstance],
    project: &Project,
    endpoint: &Endpoint,
) -> Option<GridPoint> {
    for instance in instances.iter().filter(|i| i.device == endpoint.device) {
        let d = &project.devices[endpoint.device];
        let role = if instance.symbol == SymbolId::ContactorContact {
            let Kind::Contactor { poles, .. } = &d.kind else {
                continue;
            };
            let index = poles
                .iter()
                .position(|p| p.from == endpoint.terminal || p.to == endpoint.terminal)?;
            if instance.pole_index != Some(index) {
                continue;
            }
            if poles[index].from == endpoint.terminal {
                "in"
            } else {
                "out"
            }
        } else {
            &endpoint.terminal
        };
        if let Some(p) = definition(instance.symbol)
            .ports
            .into_iter()
            .find(|p| p.role == role)
        {
            return Some(instance.origin.offset(p.anchor));
        }
    }
    None
}
