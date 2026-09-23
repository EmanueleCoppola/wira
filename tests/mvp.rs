use std::{fs, path::PathBuf, process::Command};
use wira::{
    compile,
    mvp_model::Kind,
    render::{
        self,
        drawing::{DrawCommand, Function},
    },
};

fn example() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/motor-starter/main.wira")
}

#[test]
fn example_semantics_and_layout() {
    let project = compile(&example()).unwrap();
    assert_eq!(project.name, "Motor Starter");
    assert_eq!(
        project
            .pages
            .iter()
            .map(|p| p.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Control", "Power"]
    );
    assert_eq!(project.pages[0].connections.len(), 4);
    assert_eq!(project.pages[1].connections.len(), 7);
    let km = project.devices.iter().position(|d| d.tag == "KM1").unwrap();
    let contactor = &project.devices[km];
    let Kind::Contactor { coil, poles } = &contactor.kind else {
        panic!("not a contactor")
    };
    assert_eq!(
        (&coil.from, &coil.to),
        (&"A1".to_string(), &"A2".to_string())
    );
    assert_eq!(poles.len(), 3);
    assert_eq!(poles[1].from, "3/L2");
    assert_eq!(poles[1].to, "4/T2");
    assert_eq!(
        project.pages[1].connections.len(),
        7,
        "poles must not become external wires"
    );
    let pages = render::layout(&project);
    assert!(pages[0]
        .symbols
        .iter()
        .any(|s| s.function == Function::Coil(km)));
    assert!(!pages[0]
        .symbols
        .iter()
        .any(|s| s.function == Function::Poles(km)));
    assert!(pages[1]
        .symbols
        .iter()
        .any(|s| s.function == Function::Poles(km)));
    assert!(!pages[1]
        .symbols
        .iter()
        .any(|s| s.function == Function::Coil(km)));
}

#[test]
fn pdf_build_is_valid_and_deterministic() {
    let (_, a) = wira::build(&example()).unwrap();
    let (_, b) = wira::build(&example()).unwrap();
    assert_eq!(a, b);
    assert!(a.starts_with(b"%PDF-"));
    assert!(a.len() > 2000);
    let text = String::from_utf8_lossy(&a);
    assert_eq!(text.matches("/Type /Page\n").count(), 2);
    let pages = render::layout(&compile(&example()).unwrap());
    for expected in ["Motor Starter", "Control", "Power", "PLC1", "KM1", "M1"] {
        assert!(
            pages
                .iter()
                .flat_map(|p| &p.commands)
                .any(|c| matches!(c, DrawCommand::Text { text, .. } if text == expected)),
            "missing {expected}"
        );
    }
}

#[test]
fn cli_build_and_errors() {
    let dir = std::env::temp_dir().join(format!("wira-mvp-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let output = dir.join("example.pdf");
    let result = Command::new(env!("CARGO_BIN_EXE_wira"))
        .args([
            "build",
            example().to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(String::from_utf8_lossy(&result.stdout).contains("2 pages"));
    assert!(fs::metadata(&output).unwrap().len() > 2000);
    fs::write(
        dir.join("bad.wira"),
        "project \"Bad\"\npage \"P\" { connect UNKNOWN.X -> UNKNOWN.Y }",
    )
    .unwrap();
    let bad = Command::new(env!("CARGO_BIN_EXE_wira"))
        .args([
            "build",
            dir.join("bad.wira").to_str().unwrap(),
            "--output",
            output.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("unknown device"));
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn implicit_terminals_and_reference_errors() {
    let project = compile(&example()).unwrap();
    let terminals = |tag: &str| {
        project
            .devices
            .iter()
            .find(|d| d.tag == tag)
            .unwrap()
            .terminals
            .clone()
    };
    assert_eq!(terminals("PSU1"), ["L", "N", "PE", "+", "-"]);
    assert_eq!(terminals("AC400"), ["L1", "L2", "L3", "PE"]);
    assert_eq!(terminals("PLC1"), ["L+", "M", "Q0.0"]);
    assert_eq!(
        terminals("KM1"),
        ["A1", "A2", "1/L1", "2/T1", "3/L2", "4/T2", "5/L3", "6/T3"]
    );
    assert_eq!(terminals("M1"), ["U", "V", "W", "PE"]);
    let dir = std::env::temp_dir().join(format!("wira-errors-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let entry = dir.join("main.wira");
    let cases=[
        ("project \"P\"\npower_supply D \"d\" {\ninput 230VAC\noutput 24VDC\n}\npower_supply D \"d\" {\ninput 230VAC\noutput 24VDC\n}\npage \"P\" {}", "duplicate device tag"),
        ("project \"P\"\nplc D \"d\" { supply 24VDC }\npage \"P\" { connect D.UNKNOWN -> D.M }", "unknown terminal"),
        ("project \"P\"\ncontactor K \"k\" { coil A1 -> A2 { voltage 24VDC } pole A1 -> T1 }\npage \"P\" {}", "duplicate contactor terminal"),
    ];
    for (source, expected) in cases {
        fs::write(&entry, source).unwrap();
        let error = compile(&entry).unwrap_err();
        assert!(error.contains(expected), "expected {expected} in {error}");
        assert!(error.contains("main.wira:"));
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn nested_includes_resolve_forward_references() {
    let dir = std::env::temp_dir().join(format!("wira-includes-{}", std::process::id()));
    fs::create_dir_all(dir.join("nested")).unwrap();
    fs::write(
        dir.join("main.wira"),
        "project \"Nested\"\ninclude \"nested/pages.wira\"\ninclude \"devices.wira\"",
    )
    .unwrap();
    fs::write(
        dir.join("nested/pages.wira"),
        "page \"One\" {\nconnect A.L1 -> B.U\n}",
    )
    .unwrap();
    fs::write(dir.join("devices.wira"),"supply A \"source\" {\nvoltage 400VAC\nphases 3\n}\nmotor B \"load\" {\nvoltage 400VAC\nphases 3\npower 2.2kW\n}").unwrap();
    let project = compile(&dir.join("main.wira")).unwrap();
    assert_eq!(project.pages[0].connections.len(), 1);
    fs::write(dir.join("nested/pages.wira"), "include \"missing.wira\"").unwrap();
    let error = compile(&dir.join("main.wira")).unwrap_err();
    assert!(error.contains("cannot read include"), "{error}");
    assert!(error.contains("pages.wira:"), "{error}");
    fs::remove_dir_all(dir).unwrap();
}
