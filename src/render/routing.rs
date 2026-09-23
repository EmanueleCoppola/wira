use super::{
    drawing::{DrawCommand, SymbolInstance, WireRoute},
    geometry::{GridPoint, Point},
    placement::port,
};
use crate::mvp_model::{Kind, Project};
use std::collections::{BTreeMap, BTreeSet};

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
) -> Vec<WireRoute> {
    let page = &project.pages[page_index];
    let is_control = instances
        .iter()
        .any(|i| matches!(i.symbol, super::symbols::SymbolId::ContactorCoil));
    let is_power = instances
        .iter()
        .any(|i| matches!(i.symbol, super::symbols::SymbolId::ContactorContact));
    page.connections
        .iter()
        .filter_map(|c| {
            let a = port(instances, project, &c.from)?;
            let b = port(instances, project, &c.to)?;
            let ka = &project.devices[c.from.device].kind;
            let kb = &project.devices[c.to.device].kind;
            let mut points = if is_control {
                let is_return = |kind: &Kind, term: &str| {
                    matches!(kind, Kind::PowerSupply { .. }) && term == "-"
                        || matches!(kind, Kind::Plc { .. }) && term == "M"
                        || matches!(kind, Kind::Contactor { .. }) && term == "A2"
                };
                if is_return(ka, &c.from.terminal) && is_return(kb, &c.to.terminal) {
                    let branch = if matches!(ka, Kind::Plc { .. }) || matches!(kb, Kind::Plc { .. })
                    {
                        GridPoint::new(24, 32)
                    } else {
                        GridPoint::new(15, 32)
                    };
                    let down = |p: GridPoint| {
                        if p.col < 20 {
                            15
                        } else if p.col > 40 {
                            60
                        } else {
                            24
                        }
                    };
                    vec![
                        a,
                        GridPoint::new(down(a), a.row),
                        GridPoint::new(down(a), 32),
                        branch,
                        GridPoint::new(down(b), 32),
                        GridPoint::new(down(b), b.row),
                        b,
                    ]
                } else if matches!(ka, Kind::PowerSupply { .. }) && c.from.terminal == "+" {
                    vec![a, GridPoint::new(20, a.row), GridPoint::new(20, b.row), b]
                } else {
                    vec![
                        a,
                        GridPoint::new((a.col + b.col) / 2, a.row),
                        GridPoint::new((a.col + b.col) / 2, b.row),
                        b,
                    ]
                }
            } else if is_power {
                if a.col == b.col {
                    vec![a, b]
                } else {
                    vec![a, GridPoint::new(a.col, b.row), b]
                }
            } else if a.row == b.row || a.col == b.col {
                vec![a, b]
            } else {
                vec![a, GridPoint::new(b.col, a.row), b]
            };
            compact(&mut points);
            Some(route(points, c.from.clone(), c.to.clone()))
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
