use std::path::PathBuf;
use wira::{
    compile,
    mvp_model::Endpoint,
    render::{
        self,
        drawing::{DrawCommand, Function, PAGE_H_MM, PAGE_W_MM},
        geometry::{Bounds, GridPoint, Point},
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
        (
            1,
            "AC400",
            Function::Main(device(&project, "AC400")),
            GridPoint::new(37, 12),
            "A5",
        ),
        (
            1,
            "KM1",
            Function::Poles(device(&project, "KM1")),
            GridPoint::new(25, 22),
            "B3",
        ),
        (
            1,
            "KM1",
            Function::Poles(device(&project, "KM1")),
            GridPoint::new(37, 22),
            "B5",
        ),
        (
            1,
            "KM1",
            Function::Poles(device(&project, "KM1")),
            GridPoint::new(49, 22),
            "B6",
        ),
        (
            1,
            "M1",
            Function::Main(device(&project, "M1")),
            GridPoint::new(37, 37),
            "D5",
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

    for page in &pages {
        for (i, a) in page.symbols.iter().enumerate() {
            for b in &page.symbols[i + 1..] {
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
    }
    assert_eq!(origins.len(), 3);
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
            "Wire KM1.A2 -> PSU1.- (56,22) (60,22) (60,32) (15,32) (15,23) (13,23)\n",
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
            "Symbol AC400 Main(1) ThreePhaseSupply @ (37,12) zone A5\n",
            "Symbol KM1 Poles(3) ContactorContact @ (25,22) zone B3\n",
            "Symbol KM1 Poles(3) ContactorContact @ (37,22) zone B5\n",
            "Symbol KM1 Poles(3) ContactorContact @ (49,22) zone B6\n",
            "Symbol M1 Main(4) Motor @ (37,37) zone D5\n",
            "Wire AC400.L1 -> KM1.1/L1 (25,12) (25,19)\n",
            "Wire AC400.L2 -> KM1.3/L2 (37,12) (37,19)\n",
            "Wire AC400.L3 -> KM1.5/L3 (49,12) (49,19)\n",
            "Wire KM1.2/T1 -> M1.U (25,25) (25,30)\n",
            "Wire KM1.4/T2 -> M1.V (37,25) (37,30)\n",
            "Wire KM1.6/T3 -> M1.W (49,25) (49,30)\n",
            "Wire AC400.PE -> M1.PE (61,12) (61,37) (49,37)\n",
        )
    );
}
