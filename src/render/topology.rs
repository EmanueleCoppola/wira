use std::collections::{BTreeMap, HashMap};

use crate::mvp_model::{Endpoint, Kind, Project};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageTopology {
    pub nets: Vec<Net>,
    pub feeders: Vec<MotorFeeder>,
    pub bundles: Vec<PhaseBundle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseBundle {
    pub supply: usize,
    pub feeder_indices: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Net {
    pub id: usize,
    pub endpoints: Vec<Endpoint>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MotorFeeder {
    pub supply: usize,
    pub switch: usize,
    pub motor: usize,
    pub phases: [PhasePath; 3],
    pub protective_earth: Option<EarthPath>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhasePath {
    pub source: Endpoint,
    pub pole_input: Endpoint,
    pub pole_output: Endpoint,
    pub motor: Endpoint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarthPath {
    pub source: Endpoint,
    pub motor: Endpoint,
}

fn endpoint(device: usize, terminal: &str) -> Endpoint {
    Endpoint {
        device,
        terminal: terminal.into(),
    }
}

fn root(parents: &[usize], mut index: usize) -> usize {
    while parents[index] != index {
        index = parents[index];
    }
    index
}

/// Analyze only the connections on the selected page. Invalid page indices are empty.
pub fn analyze(project: &Project, page_index: usize) -> PageTopology {
    let Some(page) = project.pages.get(page_index) else {
        return PageTopology {
            nets: Vec::new(),
            feeders: Vec::new(),
            bundles: Vec::new(),
        };
    };

    let mut endpoints: Vec<_> = page
        .connections
        .iter()
        .flat_map(|connection| [&connection.from, &connection.to])
        .cloned()
        .collect();
    endpoints.sort_by(|a, b| (a.device, &a.terminal).cmp(&(b.device, &b.terminal)));
    endpoints.dedup();

    let indices: HashMap<_, _> = endpoints
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, endpoint)| (endpoint, index))
        .collect();
    let mut parents: Vec<_> = (0..endpoints.len()).collect();
    for connection in &page.connections {
        let a = root(&parents, indices[&connection.from]);
        let b = root(&parents, indices[&connection.to]);
        parents[a.max(b)] = a.min(b);
    }

    let mut groups: BTreeMap<usize, Vec<Endpoint>> = BTreeMap::new();
    for (index, endpoint) in endpoints.into_iter().enumerate() {
        groups
            .entry(root(&parents, index))
            .or_default()
            .push(endpoint);
    }
    let nets: Vec<_> = groups
        .into_values()
        .enumerate()
        .map(|(id, endpoints)| Net { id, endpoints })
        .collect();
    let net_ids: HashMap<_, _> = nets
        .iter()
        .flat_map(|net| {
            net.endpoints
                .iter()
                .cloned()
                .map(move |endpoint| (endpoint, net.id))
        })
        .collect();
    let same_net =
        |a: &Endpoint, b: &Endpoint| net_ids.get(a).is_some_and(|id| net_ids.get(b) == Some(id));

    let mut feeders = Vec::new();
    for (supply, source_device) in project.devices.iter().enumerate() {
        if !matches!(source_device.kind, Kind::Supply { .. }) {
            continue;
        }
        for (switch, switch_device) in project.devices.iter().enumerate() {
            let Kind::Contactor { poles, .. } = &switch_device.kind else {
                continue;
            };
            for (motor, motor_device) in project.devices.iter().enumerate() {
                if !matches!(motor_device.kind, Kind::Motor { .. }) {
                    continue;
                }
                let mut paths = Vec::new();
                let (mut source_mask, mut motor_mask) = (0u8, 0u8);
                for pole in poles {
                    let pole_input = endpoint(switch, &pole.from);
                    let pole_output = endpoint(switch, &pole.to);
                    let sources: Vec<_> = ["L1", "L2", "L3"]
                        .into_iter()
                        .enumerate()
                        .filter(|(_, terminal)| same_net(&endpoint(supply, terminal), &pole_input))
                        .collect();
                    let motors: Vec<_> = ["U", "V", "W"]
                        .into_iter()
                        .enumerate()
                        .filter(|(_, terminal)| same_net(&pole_output, &endpoint(motor, terminal)))
                        .collect();
                    if let ([(source_index, source_terminal)], [(motor_index, motor_terminal)]) =
                        (sources.as_slice(), motors.as_slice())
                    {
                        if source_index != motor_index {
                            continue;
                        }
                        source_mask |= 1 << source_index;
                        motor_mask |= 1 << motor_index;
                        paths.push(PhasePath {
                            source: endpoint(supply, source_terminal),
                            pole_input,
                            pole_output,
                            motor: endpoint(motor, motor_terminal),
                        });
                    }
                }
                if paths.len() != 3 || source_mask != 0b111 || motor_mask != 0b111 {
                    continue;
                }
                let protective_earth = {
                    let source = endpoint(supply, "PE");
                    let motor = endpoint(motor, "PE");
                    same_net(&source, &motor).then_some(EarthPath { source, motor })
                };
                feeders.push(MotorFeeder {
                    supply,
                    switch,
                    motor,
                    phases: paths.try_into().expect("exactly three phase paths"),
                    protective_earth,
                });
            }
        }
    }

    let mut by_supply: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (index, feeder) in feeders.iter().enumerate() {
        by_supply.entry(feeder.supply).or_default().push(index);
    }
    let bundles = by_supply
        .into_iter()
        .map(|(supply, feeder_indices)| PhaseBundle {
            supply,
            feeder_indices,
        })
        .collect();

    PageTopology {
        nets,
        feeders,
        bundles,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mvp_model::{Coil, Connection, Device, Page, Pole};

    fn device(tag: &str, kind: Kind) -> Device {
        Device {
            tag: tag.into(),
            description: String::new(),
            kind,
            terminals: Vec::new(),
        }
    }

    fn supply(tag: &str) -> Device {
        device(
            tag,
            Kind::Supply {
                voltage: "400V".into(),
            },
        )
    }

    fn switch(tag: &str, poles: &[(&str, &str)]) -> Device {
        device(
            tag,
            Kind::Contactor {
                coil: Coil {
                    from: "A1".into(),
                    to: "A2".into(),
                    voltage: "24V".into(),
                },
                poles: poles
                    .iter()
                    .map(|&(from, to)| Pole {
                        from: from.into(),
                        to: to.into(),
                    })
                    .collect(),
            },
        )
    }

    fn motor(tag: &str) -> Device {
        device(
            tag,
            Kind::Motor {
                voltage: "400V".into(),
                power: "1kW".into(),
            },
        )
    }

    fn connect(a: (usize, &str), b: (usize, &str)) -> Connection {
        Connection {
            from: endpoint(a.0, a.1),
            to: endpoint(b.0, b.1),
        }
    }

    fn project(devices: Vec<Device>, connections: Vec<Connection>) -> Project {
        Project {
            name: String::new(),
            devices,
            pages: vec![Page {
                name: String::new(),
                connections,
            }],
        }
    }

    #[test]
    fn nets_join_shared_endpoints_and_ignore_wire_direction_and_order() {
        let devices = vec![supply("S"), switch("K", &[("I", "O")]), motor("M")];
        let connections = vec![
            connect((1, "I"), (0, "L1")),
            connect((2, "U"), (1, "O")),
            connect((0, "L1"), (2, "V")),
        ];
        let topology = analyze(&project(devices.clone(), connections.clone()), 0);
        let mut reversed = connections;
        reversed.reverse();
        for connection in &mut reversed {
            std::mem::swap(&mut connection.from, &mut connection.to);
        }
        assert_eq!(topology, analyze(&project(devices, reversed), 0));
        assert_eq!(topology.nets.len(), 2);
        assert_eq!(topology.nets[0].id, 0);
        assert_eq!(
            topology.nets[0].endpoints,
            vec![endpoint(0, "L1"), endpoint(1, "I"), endpoint(2, "V")]
        );
        assert!(topology.feeders.is_empty());
    }

    #[test]
    fn feeder_follows_semantic_pole_order_with_arbitrary_tags_and_reversed_wires() {
        let devices = vec![
            supply("GRID"),
            switch("SW", &[("c", "d"), ("a", "b"), ("e", "f")]),
            motor("LOAD"),
        ];
        let connections = vec![
            connect((1, "a"), (0, "L1")),
            connect((2, "U"), (1, "b")),
            connect((0, "L2"), (1, "c")),
            connect((1, "d"), (2, "V")),
            connect((1, "e"), (0, "L3")),
            connect((2, "W"), (1, "f")),
            connect((2, "PE"), (0, "PE")),
        ];
        let topology = analyze(&project(devices, connections), 0);
        assert_eq!(topology.feeders.len(), 1);
        let feeder = &topology.feeders[0];
        assert_eq!((feeder.supply, feeder.switch, feeder.motor), (0, 1, 2));
        assert_eq!(
            feeder
                .phases
                .iter()
                .map(|p| p.source.terminal.as_str())
                .collect::<Vec<_>>(),
            ["L2", "L1", "L3"]
        );
        assert_eq!(
            feeder
                .phases
                .iter()
                .map(|p| p.pole_input.terminal.as_str())
                .collect::<Vec<_>>(),
            ["c", "a", "e"]
        );
        assert_eq!(
            feeder
                .phases
                .iter()
                .map(|p| p.pole_output.terminal.as_str())
                .collect::<Vec<_>>(),
            ["d", "b", "f"]
        );
        assert_eq!(
            feeder
                .phases
                .iter()
                .map(|p| p.motor.terminal.as_str())
                .collect::<Vec<_>>(),
            ["V", "U", "W"]
        );
        assert_eq!(
            feeder.protective_earth,
            Some(EarthPath {
                source: endpoint(0, "PE"),
                motor: endpoint(2, "PE")
            })
        );
    }

    #[test]
    fn finds_multiple_feeders_with_shared_or_separate_supplies() {
        let devices = vec![
            supply("A"),
            supply("B"),
            switch("X", &[("x1", "y1"), ("x2", "y2"), ("x3", "y3")]),
            switch("Y", &[("p1", "q1"), ("p2", "q2"), ("p3", "q3")]),
            switch("Z", &[("r1", "s1"), ("r2", "s2"), ("r3", "s3")]),
            motor("M"),
            motor("N"),
            motor("O"),
        ];
        let mut connections = Vec::new();
        for (source, switch, motor, inputs, outputs) in [
            (0, 2, 5, ["x1", "x2", "x3"], ["y1", "y2", "y3"]),
            (0, 3, 6, ["p1", "p2", "p3"], ["q1", "q2", "q3"]),
            (1, 4, 7, ["r1", "r2", "r3"], ["s1", "s2", "s3"]),
        ] {
            for (phase, (input, output)) in inputs.into_iter().zip(outputs).enumerate() {
                connections.push(connect(
                    (source, ["L1", "L2", "L3"][phase]),
                    (switch, input),
                ));
                connections.push(connect((switch, output), (motor, ["U", "V", "W"][phase])));
            }
        }
        let topology = analyze(&project(devices, connections), 0);
        assert_eq!(
            topology
                .feeders
                .iter()
                .map(|f| (f.supply, f.switch, f.motor))
                .collect::<Vec<_>>(),
            [(0, 2, 5), (0, 3, 6), (1, 4, 7)]
        );
        assert!(topology
            .feeders
            .iter()
            .all(|f| f.protective_earth.is_none()));
    }

    #[test]
    fn incomplete_and_wrongly_joined_phases_do_not_form_feeders() {
        let devices = vec![
            supply("S"),
            switch("K", &[("a", "b"), ("c", "d"), ("e", "f")]),
            motor("M"),
        ];
        let mut connections = vec![
            connect((0, "L1"), (1, "a")),
            connect((1, "b"), (2, "U")),
            connect((0, "L2"), (1, "c")),
            connect((1, "d"), (2, "V")),
            connect((0, "L3"), (1, "e")),
        ];
        assert!(analyze(&project(devices.clone(), connections.clone()), 0)
            .feeders
            .is_empty());
        connections.push(connect((1, "f"), (2, "U")));
        assert!(analyze(&project(devices, connections), 0)
            .feeders
            .is_empty());
    }

    #[test]
    fn page_scope_and_invalid_index() {
        let mut project = project(vec![supply("S")], vec![connect((0, "L1"), (0, "PE"))]);
        project.pages.push(Page {
            name: "empty".into(),
            connections: Vec::new(),
        });
        assert_eq!(analyze(&project, 1).nets, Vec::<Net>::new());
        assert_eq!(analyze(&project, 2).nets, Vec::<Net>::new());
    }
}
