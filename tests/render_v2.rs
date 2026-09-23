use std::path::PathBuf;
use wira::{
    compile,
    mvp_model::Endpoint,
    render::{
        self,
        drawing::{DrawCommand, Function, PAGE_H_MM, PAGE_W_MM},
        geometry::{Bounds, GridPoint, Point, GRID_MM},
        placement::PHASE_PITCH_COLS,
        symbols::{definition, PortDirection, SymbolId},
    },
};

fn example() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/motor-starter/main.wira")
}

fn device(project: &wira::mvp_model::Project, tag: &str) -> usize {
    project
        .devices
        .iter()
        .position(|device| device.tag == tag)
        .unwrap()
}

fn symbol_bounds(instance: &render::drawing::SymbolInstance) -> Bounds {
    definition(instance.symbol)
        .bounds
        .translated(instance.origin.mm())
}

fn route_contains(route: &render::drawing::WireRoute, point: GridPoint) -> bool {
    route.points.windows(2).any(|segment| {
        let [a, b] = [segment[0], segment[1]];
        if a.col == b.col {
            point.col == a.col && point.row >= a.row.min(b.row) && point.row <= a.row.max(b.row)
        } else {
            point.row == a.row && point.col >= a.col.min(b.col) && point.col <= a.col.max(b.col)
        }
    })
}

fn motor_feeder_project() -> wira::mvp_model::Project {
    let mut project = compile(&example()).unwrap();
    let original = [
        device(&project, "AC400"),
        device(&project, "KM1"),
        device(&project, "M1"),
    ];
    let connections = project.pages[1]
        .connections
        .iter()
        .filter(|connection| {
            original.contains(&connection.from.device) && original.contains(&connection.to.device)
        })
        .cloned()
        .collect::<Vec<_>>();
    let copies = original.map(|index| project.devices[index].clone());
    let mut remap = [0usize; 3];
    for (slot, mut copy) in copies.into_iter().enumerate() {
        let index = project.devices.len();
        copy.tag.push_str("_2");
        project.devices.push(copy);
        remap[slot] = index;
    }
    let duplicate = connections
        .into_iter()
        .map(|mut connection| {
            connection.from.device = remap[original
                .iter()
                .position(|&i| i == connection.from.device)
                .unwrap()];
            connection.to.device = remap[original
                .iter()
                .position(|&i| i == connection.to.device)
                .unwrap()];
            connection
        })
        .collect::<Vec<_>>();
    project.pages[1].connections.extend(duplicate);
    project
}

#[test]
fn symbols_match_devices_and_expose_terminal_ports() {
    let project = compile(&example()).unwrap();
    let pages = render::layout(&project);

    let expected = [
        (
            "PSU1",
            0,
            Function::Main(device(&project, "PSU1")),
            SymbolId::PowerSupply,
        ),
        (
            "PLC1",
            0,
            Function::Main(device(&project, "PLC1")),
            SymbolId::Plc,
        ),
        (
            "KM1",
            0,
            Function::Coil(device(&project, "KM1")),
            SymbolId::ContactorCoil,
        ),
        (
            "AC400",
            1,
            Function::Main(device(&project, "AC400")),
            SymbolId::ThreePhaseSupply,
        ),
        (
            "M1",
            1,
            Function::Main(device(&project, "M1")),
            SymbolId::Motor,
        ),
    ];
    for (tag, page_index, function, symbol) in expected {
        let instance = pages[page_index]
            .symbols
            .iter()
            .find(|instance| {
                instance.device == device(&project, tag) && instance.function == function
            })
            .unwrap_or_else(|| panic!("missing {tag} symbol on page {page_index}"));
        assert_eq!(instance.symbol, symbol);
        assert_eq!(definition(instance.symbol).id, symbol);
    }

    let motor = definition(SymbolId::Motor);
    assert_eq!(
        motor
            .ports
            .iter()
            .map(|port| port.role.as_str())
            .collect::<Vec<_>>(),
        ["U", "V", "W", "PE"]
    );
    assert!(motor
        .ports
        .iter()
        .all(|port| port.direction == PortDirection::Up || port.role == "PE"));
    let coil = definition(SymbolId::ContactorCoil);
    assert_eq!(
        coil.ports
            .iter()
            .map(|port| port.role.as_str())
            .collect::<Vec<_>>(),
        ["A1", "A2"]
    );
}

#[test]
fn placement_uses_expected_grid_origins_and_zones() {
    let project = compile(&example()).unwrap();
    let pages = render::layout(&project);

    let expected = [
        (
            0,
            "PSU1",
            Function::Main(device(&project, "PSU1")),
            GridPoint::new(10, 22),
            "B1",
        ),
        (
            0,
            "PLC1",
            Function::Main(device(&project, "PLC1")),
            GridPoint::new(31, 22),
            "B4",
        ),
        (
            0,
            "KM1",
            Function::Coil(device(&project, "KM1")),
            GridPoint::new(53, 22),
            "B6",
        ),
    ];
    for (page_index, tag, function, origin, zone) in expected {
        assert!(
            pages[page_index].symbols.iter().any(|instance| {
                instance.device == device(&project, tag)
                    && instance.function == function
                    && instance.origin == origin
                    && instance.page_index == page_index
                    && instance.zone == zone
            }),
            "missing {tag} at {origin:?} in zone {zone}"
        );
    }
    assert_eq!(pages[0].symbols.len(), 3);
    assert_eq!(pages[1].symbols.len(), 5);

    let power = &pages[1];
    let supply = device(&project, "AC400");
    let motor = device(&project, "M1");
    let phase_cols = power
        .symbols
        .iter()
        .filter(|symbol| symbol.symbol == SymbolId::ContactorContact)
        .map(|symbol| symbol.origin.col)
        .collect::<Vec<_>>();
    assert_eq!(phase_cols.len(), 3);
    assert_eq!(phase_cols[1] - phase_cols[0], PHASE_PITCH_COLS);
    assert_eq!(phase_cols[2] - phase_cols[1], PHASE_PITCH_COLS);
    assert_eq!((phase_cols[1] - phase_cols[0]) * GRID_MM as i32, 8);
    let supply_instance = power.symbols.iter().find(|s| s.device == supply).unwrap();
    let motor_instance = power.symbols.iter().find(|s| s.device == motor).unwrap();
    assert_eq!(supply_instance.origin.col, motor_instance.origin.col);
    assert_eq!(phase_cols[1], supply_instance.origin.col);
    assert_eq!(phase_cols[1], motor_instance.origin.col);

    for page in &pages {
        for (i, a) in page.symbols.iter().enumerate() {
            for b in &page.symbols[i + 1..] {
                if a.symbol == SymbolId::ContactorContact
                    && b.symbol == SymbolId::ContactorContact
                    && a.feeder_index == b.feeder_index
                {
                    continue;
                }
                assert!(
                    !symbol_bounds(a).intersects(symbol_bounds(b)),
                    "symbols {} and {} overlap on {}",
                    project.devices[a.device].tag,
                    project.devices[b.device].tag,
                    page.title
                );
            }
        }
    }
}

#[test]
fn routes_preserve_connections_and_join_symbol_ports_on_the_grid() {
    let project = compile(&example()).unwrap();
    let pages = render::layout(&project);

    for (page_index, page) in project.pages.iter().enumerate() {
        assert_eq!(pages[page_index].wires.len(), page.connections.len());
        for (connection, wire) in page.connections.iter().zip(&pages[page_index].wires) {
            assert_eq!(wire.from, connection.from);
            assert_eq!(wire.to, connection.to);
            assert_eq!(
                wire.points.first(),
                render::placement::port(&pages[page_index].symbols, &project, &connection.from)
                    .as_ref()
            );
            assert_eq!(
                wire.points.last(),
                render::placement::port(&pages[page_index].symbols, &project, &connection.to)
                    .as_ref()
            );
            assert!(wire.points.len() >= 2);
            assert!(
                wire.points
                    .windows(2)
                    .all(|pair| { pair[0].col == pair[1].col || pair[0].row == pair[1].row }),
                "route contains a diagonal segment: {:?}",
                wire.points
            );
        }
    }

    let control = &pages[0];
    let plc = device(&project, "PLC1");
    let km = device(&project, "KM1");
    assert!(control.wires.iter().any(|wire| {
        wire.from
            == Endpoint {
                device: plc,
                terminal: "Q0.0".into(),
            }
            && wire.to
                == Endpoint {
                    device: km,
                    terminal: "A1".into(),
                }
            && wire.points == [GridPoint::new(35, 22), GridPoint::new(50, 22)]
    }));
}

#[test]
fn power_phases_reach_distinct_contactor_contacts() {
    let project = compile(&example()).unwrap();
    let power = &render::layout(&project)[1];
    let supply = device(&project, "AC400");
    let contactor = device(&project, "KM1");
    let phases = [("L1", "1/L1", 0), ("L2", "3/L2", 1), ("L3", "5/L3", 2)];
    let mut origins = std::collections::BTreeSet::new();

    for (source_terminal, target_terminal, pole_index) in phases {
        let wire = power
            .wires
            .iter()
            .find(|wire| {
                wire.from
                    == Endpoint {
                        device: supply,
                        terminal: source_terminal.into(),
                    }
            })
            .unwrap();
        assert_eq!(wire.to.device, contactor);
        assert_eq!(wire.to.terminal, target_terminal);

        let symbol = power
            .symbols
            .iter()
            .find(|symbol| symbol.device == contactor && symbol.pole_index == Some(pole_index))
            .unwrap();
        assert_eq!(symbol.symbol, SymbolId::ContactorContact);
        assert!(origins.insert(symbol.origin));
        let motor_port = render::placement::port(
            &power.symbols,
            &project,
            &Endpoint {
                device: device(&project, "M1"),
                terminal: ["U", "V", "W"][pole_index].into(),
            },
        )
        .unwrap();
        assert_eq!(wire.points.last().unwrap().col, motor_port.col);
        assert_eq!(wire.points.first().unwrap().col, symbol.origin.col);
    }
    assert_eq!(origins.len(), 3);
}

#[test]
fn topology_layout_recognizes_compact_feeder_with_shared_zero_volt_net() {
    let project = compile(&example()).unwrap();
    let power = &render::layout(&project)[1];
    assert_eq!(power.topology.feeders.len(), 1);
    assert_eq!(power.topology.bundles.len(), 1);
    assert_eq!(power.topology.bundles[0].feeder_indices, [0]);
    let feeder = &power.topology.feeders[0];
    assert_eq!(feeder.supply, device(&project, "AC400"));
    assert_eq!(feeder.switch, device(&project, "KM1"));
    assert_eq!(feeder.motor, device(&project, "M1"));

    let control_topology = render::topology::analyze(&project, 0);
    assert!(control_topology.nets.iter().any(|net| {
        net.endpoints.contains(&Endpoint {
            device: device(&project, "PSU1"),
            terminal: "-".into(),
        }) && net.endpoints.contains(&Endpoint {
            device: device(&project, "PLC1"),
            terminal: "M".into(),
        }) && net.endpoints.contains(&Endpoint {
            device: device(&project, "KM1"),
            terminal: "A2".into(),
        })
    }));

    let contacts = power
        .symbols
        .iter()
        .filter(|s| s.symbol == SymbolId::ContactorContact)
        .collect::<Vec<_>>();
    assert_eq!(contacts.len(), 3);
    assert_eq!(
        contacts[1].origin.col - contacts[0].origin.col,
        PHASE_PITCH_COLS
    );
    assert_eq!(
        contacts[2].origin.col - contacts[1].origin.col,
        PHASE_PITCH_COLS
    );
    let group = &power.pole_groups[0];
    assert_eq!(power.pole_groups.len(), 1);
    assert_eq!(group.members.len(), 3);
    assert_eq!(group.device, feeder.switch);
    assert_eq!(
        group.tag_anchor,
        Point::new(group.bounds.left - 3.0, contacts[0].origin.mm().y + 1.0)
    );

    let earth = power
        .wires
        .iter()
        .find(|wire| wire.from.terminal == "PE" && wire.to.terminal == "PE")
        .unwrap();
    assert_eq!(earth.points.len(), 3);
    assert!(earth
        .points
        .windows(2)
        .all(|s| s[0].col == s[1].col || s[0].row == s[1].row));
    assert_eq!(earth.points[1].col, earth.points[0].col);
    assert_eq!(earth.points[1].row, earth.points[2].row);
    assert!(
        earth
            .points
            .windows(2)
            .map(|s| (s[0].col - s[1].col).abs() + (s[0].row - s[1].row).abs())
            .sum::<i32>()
            <= 20
    );

    for (i, a) in power.symbols.iter().enumerate() {
        for b in &power.symbols[i + 1..] {
            if a.symbol == SymbolId::ContactorContact && b.symbol == SymbolId::ContactorContact {
                continue;
            }
            assert!(!symbol_bounds(a).intersects(symbol_bounds(b)));
        }
    }
    for wire in &power.wires {
        assert!(wire
            .points
            .windows(2)
            .all(|s| s[0].col == s[1].col || s[0].row == s[1].row));
    }
    for symbol in &power.symbols {
        for port in definition(symbol.symbol).ports {
            let at = symbol.origin.offset(port.anchor).mm();
            assert!(!power.commands.iter().any(|command| matches!(command,
                DrawCommand::Circle { center, .. } if *center == at
            )));
        }
    }
    assert_eq!(power, &render::layout(&project)[1]);
}

#[test]
fn renamed_tags_and_reordered_reversed_connections_keep_feeder_layout() {
    let mut project = compile(&example()).unwrap();
    for tag in ["AC400", "KM1", "M1"] {
        let index = device(&project, tag);
        project.devices[index].tag = format!("renamed_{tag}");
    }
    project.pages[1].connections.reverse();
    for connection in &mut project.pages[1].connections {
        std::mem::swap(&mut connection.from, &mut connection.to);
    }
    let power = &render::layout(&project)[1];
    assert_eq!(power.topology.feeders.len(), 1);
    assert_eq!(power.topology.bundles.len(), 1);
    let feeder = &power.topology.feeders[0];
    assert_eq!(project.devices[feeder.supply].tag, "renamed_AC400");
    assert_eq!(project.devices[feeder.switch].tag, "renamed_KM1");
    assert_eq!(project.devices[feeder.motor].tag, "renamed_M1");
    assert_eq!(
        power
            .symbols
            .iter()
            .filter(|s| s.symbol == SymbolId::ContactorContact)
            .count(),
        3
    );
    assert_eq!(power.pole_groups.len(), 1);
    let original = render::layout(&compile(&example()).unwrap());
    let original_earth = original[1]
        .wires
        .iter()
        .find(|wire| wire.from.terminal == "PE" && wire.to.terminal == "PE")
        .unwrap();
    let reversed_earth = power
        .wires
        .iter()
        .find(|wire| wire.from.terminal == "PE" && wire.to.terminal == "PE")
        .unwrap();
    assert_eq!(
        reversed_earth.points,
        original_earth
            .points
            .iter()
            .rev()
            .copied()
            .collect::<Vec<_>>()
    );
}

#[test]
fn two_motor_feeders_tile_side_by_side() {
    let project = motor_feeder_project();
    let power = &render::layout(&project)[1];
    assert_eq!(power.topology.feeders.len(), 2);
    assert_eq!(power.topology.bundles.len(), 2);
    assert!(power
        .topology
        .bundles
        .iter()
        .all(|bundle| bundle.feeder_indices.len() == 1));
    assert_eq!(power.pole_groups.len(), 2);
    let centers = power
        .topology
        .feeders
        .iter()
        .map(|feeder| {
            power
                .symbols
                .iter()
                .find(|s| s.device == feeder.motor)
                .unwrap()
                .origin
                .col
        })
        .collect::<Vec<_>>();
    assert_eq!(centers[1] - centers[0], 16);
    for feeder in &power.topology.feeders {
        let poles = power
            .symbols
            .iter()
            .filter(|s| {
                s.feeder_index
                    == power
                        .topology
                        .feeders
                        .iter()
                        .position(|f| f.motor == feeder.motor)
                    && s.symbol == SymbolId::ContactorContact
            })
            .collect::<Vec<_>>();
        assert_eq!(poles.len(), 3);
        assert_eq!(poles[1].origin.col - poles[0].origin.col, PHASE_PITCH_COLS);
        assert_eq!(poles[2].origin.col - poles[1].origin.col, PHASE_PITCH_COLS);
    }
    for (i, a) in power.symbols.iter().enumerate() {
        for b in &power.symbols[i + 1..] {
            if a.symbol == SymbolId::ContactorContact
                && b.symbol == SymbolId::ContactorContact
                && a.feeder_index == b.feeder_index
            {
                continue;
            }
            assert!(!symbol_bounds(a).intersects(symbol_bounds(b)));
        }
    }
}

#[test]
fn shared_supply_forms_one_bundle_with_two_feeder_branches() {
    let mut project = motor_feeder_project();
    let first_supply = device(&project, "AC400");
    let second_supply = device(&project, "AC400_2");
    for connection in &mut project.pages[1].connections {
        if connection.from.device == second_supply {
            connection.from.device = first_supply;
        }
        if connection.to.device == second_supply {
            connection.to.device = first_supply;
        }
    }
    let power = &render::layout(&project)[1];
    assert_eq!(power.topology.feeders.len(), 2);
    assert_eq!(power.topology.bundles.len(), 1);
    assert_eq!(power.topology.bundles[0].feeder_indices, [0, 1]);
    assert_eq!(power.wires.len(), project.pages[1].connections.len());
    let sources = power
        .symbols
        .iter()
        .filter(|symbol| {
            symbol.device == first_supply && symbol.symbol == SymbolId::ThreePhaseSupply
        })
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 2);
    assert_ne!(sources[0].origin, sources[1].origin);
}

#[test]
fn junctions_mark_shared_nodes_only_and_ports_have_no_terminal_circles() {
    let project = compile(&example()).unwrap();
    let pages = render::layout(&project);
    let control = &pages[0];

    let expected_junctions = [Point::new(96.0, 128.0)];
    assert_eq!(control.junctions, expected_junctions);
    for junction in &control.junctions {
        let point = GridPoint::new((junction.x / 4.0) as i32, (junction.y / 4.0) as i32);
        assert!(
            control
                .wires
                .iter()
                .filter(|wire| route_contains(wire, point))
                .count()
                >= 2
        );
    }

    for page in &pages {
        let port_points = page.symbols.iter().flat_map(|symbol| {
            definition(symbol.symbol)
                .ports
                .into_iter()
                .map(move |port| symbol.origin.offset(port.anchor).mm())
        });
        for port in port_points {
            assert!(
                !page.commands.iter().any(|command| matches!(command,
                    DrawCommand::Circle { center, .. } if *center == port
                )),
                "terminal marker drawn at ({}, {}) on {}",
                port.x,
                port.y,
                page.title
            );
        }
    }
}

#[test]
fn every_sheet_has_the_mm_frame_and_its_title_block() {
    let project = compile(&example()).unwrap();
    let pages = render::layout(&project);
    for (index, page) in pages.iter().enumerate() {
        assert_eq!(page.title, project.pages[index].name);
        assert_eq!(
            page.commands[0],
            DrawCommand::Rect {
                origin: render::geometry::Point::new(8.0, 8.0),
                size: render::geometry::Point::new(PAGE_W_MM - 16.0, PAGE_H_MM - 16.0),
                width: 0.35,
            }
        );
        assert_eq!(
            page.commands[1],
            DrawCommand::Rect {
                origin: render::geometry::Point::new(14.0, 14.0),
                size: render::geometry::Point::new(PAGE_W_MM - 28.0, 165.0),
                width: 0.25,
            }
        );
        assert!(page.commands.iter().any(|command| matches!(command,
            DrawCommand::Text { text, .. } if text == &project.name
        )));
        assert!(page.commands.iter().any(|command| matches!(command,
            DrawCommand::Text { text, .. } if text == &format!("{} / {}", index + 1, pages.len())
        )));
    }
}

#[test]
fn control_sheet_snapshot_is_stable() {
    let project = compile(&example()).unwrap();
    let pages = render::layout(&project);
    assert_eq!(
        pages[0].snapshot(&project),
        concat!(
            "Page Control\n",
            "Symbol PSU1 Main(0) PowerSupply @ (10,22) zone B1\n",
            "Symbol PLC1 Main(2) Plc @ (31,22) zone B4\n",
            "Symbol KM1 Coil(3) ContactorCoil @ (53,22) zone B6\n",
            "Wire PSU1.+ -> PLC1.L+ (13,21) (27,21)\n",
            "Wire PSU1.- -> PLC1.M (13,23) (15,23) (15,32) (24,32) (24,23) (27,23)\n",
            "Wire PLC1.Q0.0 -> KM1.A1 (35,22) (50,22)\n",
            "Wire KM1.A2 -> PSU1.- (56,22) (58,22) (58,32) (15,32) (15,23) (13,23)\n",
        )
    );
}

#[test]
fn power_sheet_snapshot_is_stable() {
    let project = compile(&example()).unwrap();
    let pages = render::layout(&project);
    assert_eq!(
        pages[1].snapshot(&project),
        concat!(
            "Page Power\n",
            "MotorFeeder AC400 -> KM1 -> M1 pitch 8mm\n",
            "PoleGroup KM1 feeder 0 bounds (134,80)-(162,104)\n",
            "Symbol AC400 Main(1) ThreePhaseSupply @ (37,15) zone B5\n",
            "Symbol KM1 Poles(3) ContactorContact @ (35,23) zone B4\n",
            "Symbol KM1 Poles(3) ContactorContact @ (37,23) zone B5\n",
            "Symbol KM1 Poles(3) ContactorContact @ (39,23) zone B5\n",
            "Symbol M1 Main(4) Motor @ (37,34) zone C5\n",
            "Wire AC400.L1 -> KM1.1/L1 (35,15) (35,20)\n",
            "Wire AC400.L2 -> KM1.3/L2 (37,15) (37,20)\n",
            "Wire AC400.L3 -> KM1.5/L3 (39,15) (39,20)\n",
            "Wire KM1.2/T1 -> M1.U (35,26) (35,31)\n",
            "Wire KM1.4/T2 -> M1.V (37,26) (37,31)\n",
            "Wire KM1.6/T3 -> M1.W (39,26) (39,31)\n",
            "Wire AC400.PE -> M1.PE (42,15) (42,34) (41,34)\n",
        )
    );
}

#[test]
fn too_many_feeders_report_page_bounds_instead_of_clipping() {
    let mut source =
        String::from("project \"Five Feeders\"\nsupply S \"source\" { voltage 400VAC phases 3 }\n");
    for n in 0..5 {
        source.push_str(&format!(
            "contactor K{n} \"switch\" {{ coil A1 -> A2 {{ voltage 24VDC }} pole 1/L1 -> 2/T1 pole 3/L2 -> 4/T2 pole 5/L3 -> 6/T3 }}\n\
             motor M{n} \"motor\" {{ voltage 400VAC phases 3 power 1kW }}\n"
        ));
    }
    source.push_str("page \"Power\" {\n");
    for n in 0..5 {
        for (supply, input, output, motor) in [
            ("L1", "1/L1", "2/T1", "U"),
            ("L2", "3/L2", "4/T2", "V"),
            ("L3", "5/L3", "6/T3", "W"),
        ] {
            source.push_str(&format!(
                "connect S.{supply} -> K{n}.{input}\nconnect K{n}.{output} -> M{n}.{motor}\n"
            ));
        }
    }
    source.push_str("}\n");
    let path = std::env::temp_dir().join(format!("wira-five-feeders-{}.wira", std::process::id()));
    std::fs::write(&path, source).unwrap();
    let result = wira::build(&path);
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        result.unwrap_err(),
        "motor feeder layout exceeds the A4 schematic area"
    );
}
