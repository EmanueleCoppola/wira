use super::{
    drawing::{Function, PoleGroup, SymbolInstance},
    geometry::{Bounds, GridPoint, Point},
    symbols::{definition, id_for, SymbolId},
    topology::PageTopology,
};
use crate::mvp_model::{Endpoint, Kind, Page, Project};
use std::collections::HashSet;

pub const PHASE_PITCH_COLS: i32 = 2;
const FEEDER_PITCH_COLS: i32 = 16;
const POWER_CENTER_COL: i32 = 37;
const SOURCE_ROW: i32 = 15;
const CONTACT_ROW: i32 = 23;
const MOTOR_ROW: i32 = 34;

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
    feeder_index: Option<usize>,
) -> SymbolInstance {
    SymbolInstance {
        device,
        function,
        symbol,
        origin,
        page_index,
        pole_index,
        feeder_index,
        zone: zone(origin),
    }
}

pub fn place(
    project: &Project,
    page: &Page,
    page_index: usize,
    topology: &PageTopology,
) -> Vec<SymbolInstance> {
    let mut result = Vec::new();
    let count = topology.feeders.len() as i32;
    let first_center = POWER_CENTER_COL - (count - 1) * FEEDER_PITCH_COLS / 2;
    let mut placed = HashSet::new();
    for (feeder_index, feeder) in topology.feeders.iter().enumerate() {
        let center = first_center + feeder_index as i32 * FEEDER_PITCH_COLS;
        result.push(instance(
            feeder.supply,
            Function::Main(feeder.supply),
            SymbolId::ThreePhaseSupply,
            GridPoint::new(center, SOURCE_ROW),
            page_index,
            None,
            Some(feeder_index),
        ));
        for phase in &feeder.phases {
            let phase_number = ["L1", "L2", "L3"]
                .iter()
                .position(|&terminal| phase.source.terminal == terminal)
                .expect("analyzed three-phase source") as i32;
            let Kind::Contactor { poles, .. } = &project.devices[feeder.switch].kind else {
                unreachable!("analyzed contactor")
            };
            let pole_index = poles
                .iter()
                .position(|pole| pole.from == phase.pole_input.terminal)
                .expect("analyzed pole");
            result.push(instance(
                feeder.switch,
                Function::Poles(feeder.switch),
                SymbolId::ContactorContact,
                GridPoint::new(center + (phase_number - 1) * PHASE_PITCH_COLS, CONTACT_ROW),
                page_index,
                Some(pole_index),
                Some(feeder_index),
            ));
        }
        result.push(instance(
            feeder.motor,
            Function::Main(feeder.motor),
            SymbolId::Motor,
            GridPoint::new(center, MOTOR_ROW),
            page_index,
            None,
            Some(feeder_index),
        ));
        placed.extend([
            Function::Main(feeder.supply),
            Function::Poles(feeder.switch),
            Function::Main(feeder.motor),
        ]);
    }

    let functions = select(project, page);
    let control = functions.iter().any(|f| matches!(f, Function::Coil(_)))
        && functions.iter().any(
            |f| matches!(f,Function::Main(i) if matches!(project.devices[*i].kind,Kind::Plc{..})),
        );
    let mut fallback_col = if topology.feeders.is_empty() {
        10
    } else {
        10 + count * FEEDER_PITCH_COLS
    };
    for f in functions {
        if placed.contains(&f) {
            continue;
        }
        let device = match f {
            Function::Main(i) | Function::Coil(i) | Function::Poles(i) => i,
        };
        let d = &project.devices[device];
        if matches!(f, Function::Poles(_)) {
            if let Kind::Contactor { poles, .. } = &d.kind {
                for (n, _) in poles.iter().enumerate() {
                    result.push(instance(
                        device,
                        f,
                        SymbolId::ContactorContact,
                        GridPoint::new(fallback_col + n as i32 * PHASE_PITCH_COLS, CONTACT_ROW),
                        page_index,
                        Some(n),
                        None,
                    ));
                }
                fallback_col += FEEDER_PITCH_COLS;
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
        } else {
            let x = fallback_col;
            fallback_col += 15;
            GridPoint::new(x, 23)
        };
        result.push(instance(device, f, symbol, origin, page_index, None, None));
    }
    result
}

pub fn pole_groups(instances: &[SymbolInstance], topology: &PageTopology) -> Vec<PoleGroup> {
    topology
        .feeders
        .iter()
        .enumerate()
        .map(|(feeder_index, feeder)| {
            let mut members: Vec<_> = instances
                .iter()
                .enumerate()
                .filter_map(|(index, instance)| {
                    (instance.feeder_index == Some(feeder_index)
                        && instance.symbol == SymbolId::ContactorContact)
                        .then_some(index)
                })
                .collect();
            members.sort_by_key(|&index| instances[index].origin.col);
            let first = &instances[members[0]];
            let last = &instances[*members.last().unwrap()];
            let symbol_bounds = definition(SymbolId::ContactorContact).bounds;
            let left = first.origin.mm().x + symbol_bounds.left;
            let right = last.origin.mm().x + symbol_bounds.right;
            let center_y = first.origin.mm().y;
            PoleGroup {
                device: feeder.switch,
                feeder_index,
                members: members.try_into().expect("three contact instances"),
                bounds: Bounds::new(
                    left,
                    center_y + symbol_bounds.top,
                    right,
                    center_y + symbol_bounds.bottom,
                ),
                tag_anchor: Point::new(left - 3.0, center_y + 1.0),
            }
        })
        .collect()
}

pub fn port(
    instances: &[SymbolInstance],
    project: &Project,
    endpoint: &Endpoint,
) -> Option<GridPoint> {
    port_in_feeder(instances, project, endpoint, None)
}

pub fn port_in_feeder(
    instances: &[SymbolInstance],
    project: &Project,
    endpoint: &Endpoint,
    feeder_index: Option<usize>,
) -> Option<GridPoint> {
    for instance in instances.iter().filter(|i| {
        i.device == endpoint.device && (feeder_index.is_none() || i.feeder_index == feeder_index)
    }) {
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
