use super::{
    drawing::{DrawCommand, SymbolInstance, WireRoute},
    geometry::{GridPoint, Point},
};
use crate::mvp_model::Project;
use std::collections::{BTreeMap, BTreeSet, HashMap};

type Edge = (GridPoint, GridPoint);
fn edge(a: GridPoint, b: GridPoint) -> Edge {
    if a < b {
        (a, b)
    } else {
        (b, a)
    }
}
fn route(
    points: Vec<GridPoint>,
    from: crate::mvp_model::Endpoint,
    to: crate::mvp_model::Endpoint,
) -> WireRoute {
    WireRoute { from, to, points }
}
fn compact(points: &mut Vec<GridPoint>) {
    points.dedup();
    let mut i = 1;
    while i + 1 < points.len() {
        let (a, b, c) = (points[i - 1], points[i], points[i + 1]);
        if (a.col == b.col && b.col == c.col) || (a.row == b.row && b.row == c.row) {
            points.remove(i);
        } else {
            i += 1
        }
    }
}
pub fn route_page(
    project: &Project,
    page_index: usize,
    instances: &[SymbolInstance],
    topology: &super::topology::PageTopology,
) -> Vec<WireRoute> {
    let page = &project.pages[page_index];
    let shared_nets: HashMap<_, _> = topology
        .nets
        .iter()
        .filter(|net| net.endpoints.len() >= 3)
        .flat_map(|net| {
            net.endpoints
                .iter()
                .cloned()
                .map(move |endpoint| (endpoint, net.id))
        })
        .collect();
    let rail_row = instances
        .iter()
        .map(|instance| {
            instance.origin.row
                + (super::symbols::definition(instance.symbol).bounds.bottom / 4.0).ceil() as i32
        })
        .max()
        .unwrap_or(25)
        + 7;
    page.connections
        .iter()
        .filter_map(|connection| {
            let feeder_index = topology
                .feeders
                .iter()
                .enumerate()
                .find_map(|(index, feeder)| {
                    let same_pair =
                        |a: &crate::mvp_model::Endpoint, b: &crate::mvp_model::Endpoint| {
                            (connection.from == *a && connection.to == *b)
                                || (connection.from == *b && connection.to == *a)
                        };
                    let phase = feeder.phases.iter().any(|path| {
                        same_pair(&path.source, &path.pole_input)
                            || same_pair(&path.pole_output, &path.motor)
                    });
                    let earth = feeder
                        .protective_earth
                        .as_ref()
                        .is_some_and(|path| same_pair(&path.source, &path.motor));
                    (phase || earth).then_some(index)
                });
            let a = super::placement::port_in_feeder(
                instances,
                project,
                &connection.from,
                feeder_index,
            )?;
            let b =
                super::placement::port_in_feeder(instances, project, &connection.to, feeder_index)?;
            let mut points = if let Some(index) = feeder_index {
                if let Some(earth) = &topology.feeders[index].protective_earth {
                    if connection.from == earth.source && a.col != b.col {
                        vec![a, GridPoint::new(a.col, b.row), b]
                    } else if connection.to == earth.source && a.col != b.col {
                        vec![a, GridPoint::new(b.col, a.row), b]
                    } else {
                        vec![a, b]
                    }
                } else {
                    vec![a, b]
                }
            } else if shared_nets.contains_key(&connection.from)
                && shared_nets.get(&connection.from) == shared_nets.get(&connection.to)
            {
                let tap = |endpoint: &crate::mvp_model::Endpoint, port: GridPoint| {
                    let direction = instances
                        .iter()
                        .filter(|instance| instance.device == endpoint.device)
                        .find_map(|instance| {
                            super::symbols::definition(instance.symbol)
                                .ports
                                .into_iter()
                                .find(|symbol_port| symbol_port.role == endpoint.terminal)
                                .map(|p| p.direction)
                        });
                    match direction {
                        Some(super::symbols::PortDirection::Left) => port.col - 3,
                        Some(super::symbols::PortDirection::Right) => port.col + 2,
                        _ => port.col,
                    }
                };
                let x1 = tap(&connection.from, a);
                let x2 = tap(&connection.to, b);
                vec![
                    a,
                    GridPoint::new(x1, a.row),
                    GridPoint::new(x1, rail_row),
                    GridPoint::new(x2, rail_row),
                    GridPoint::new(x2, b.row),
                    b,
                ]
            } else if a.col == b.col || a.row == b.row {
                vec![a, b]
            } else {
                let mid = (a.col + b.col) / 2;
                vec![a, GridPoint::new(mid, a.row), GridPoint::new(mid, b.row), b]
            };
            compact(&mut points);
            Some(route(
                points,
                connection.from.clone(),
                connection.to.clone(),
            ))
        })
        .collect()
}

pub fn draw_wires(wires: &[WireRoute]) -> (Vec<DrawCommand>, Vec<Point>) {
    let mut parent: Vec<usize> = (0..wires.len()).collect();
    for i in 0..wires.len() {
        for j in 0..i {
            if wires[i].from == wires[j].from
                || wires[i].from == wires[j].to
                || wires[i].to == wires[j].from
                || wires[i].to == wires[j].to
            {
                let mut a = i;
                while parent[a] != a {
                    a = parent[a]
                }
                let mut b = j;
                while parent[b] != b {
                    b = parent[b]
                }
                parent[a] = b;
            }
        }
    }
    let mut all_edges = BTreeSet::new();
    let mut net_edges = BTreeMap::<usize, BTreeSet<Edge>>::new();
    for (i, wire) in wires.iter().enumerate() {
        let mut net = i;
        while parent[net] != net {
            net = parent[net]
        }
        for pair in wire.points.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if a.col == b.col {
                for row in a.row.min(b.row)..a.row.max(b.row) {
                    let e = edge(GridPoint::new(a.col, row), GridPoint::new(a.col, row + 1));
                    all_edges.insert(e);
                    net_edges.entry(net).or_default().insert(e);
                }
            } else if a.row == b.row {
                for col in a.col.min(b.col)..a.col.max(b.col) {
                    let e = edge(GridPoint::new(col, a.row), GridPoint::new(col + 1, a.row));
                    all_edges.insert(e);
                    net_edges.entry(net).or_default().insert(e);
                }
            }
        }
    }
    let commands = all_edges
        .into_iter()
        .map(|(a, b)| DrawCommand::Line {
            from: a.mm(),
            to: b.mm(),
            width: 0.25,
        })
        .collect();
    let mut dots = BTreeSet::new();
    for edges in net_edges.values() {
        let mut degree = BTreeMap::<GridPoint, usize>::new();
        for &(a, b) in edges {
            *degree.entry(a).or_default() += 1;
            *degree.entry(b).or_default() += 1;
        }
        for (p, n) in degree {
            if n >= 3 {
                dots.insert(p);
            }
        }
    }
    (commands, dots.into_iter().map(GridPoint::mm).collect())
}
