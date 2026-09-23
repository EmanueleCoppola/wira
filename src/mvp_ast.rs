use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLoc {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decl {
    Project { name: String, loc: SourceLoc },
    Include { path: String, loc: SourceLoc },
    Device(DeviceDecl),
    Page(PageDecl),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceDecl {
    pub kind: DeviceKind,
    pub tag: String,
    pub description: String,
    pub fields: Vec<Field>,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceKind {
    PowerSupply,
    Supply,
    Plc,
    Contactor,
    Motor,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Field {
    KeyValue {
        key: String,
        value: String,
        loc: SourceLoc,
    },
    PlcOutput {
        name: String,
        description: String,
        loc: SourceLoc,
    },
    Coil {
        from: String,
        to: String,
        voltage: String,
        loc: SourceLoc,
    },
    Pole {
        from: String,
        to: String,
        loc: SourceLoc,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageDecl {
    pub name: String,
    pub connections: Vec<ConnectionDecl>,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDecl {
    pub from: TerminalRef,
    pub to: TerminalRef,
    pub loc: SourceLoc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRef {
    pub device: String,
    pub terminal: String,
    pub loc: SourceLoc,
}
