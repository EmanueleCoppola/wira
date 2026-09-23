use crate::mvp_ast::{
    ConnectionDecl, Decl, DeviceDecl, DeviceKind, Field, PageDecl, SourceLoc, TerminalRef,
};
use std::path::Path;

pub fn parse(source: &str, path: &Path) -> Result<Vec<Decl>, String> {
    let tokens = lex(source, path)?;
    Parser {
        tokens,
        pos: 0,
        path,
    }
    .document()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TokenKind {
    Text(String),
    String(String),
    Punct(char),
    Arrow,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Token {
    kind: TokenKind,
    line: usize,
    column: usize,
}

impl Token {
    fn text(&self) -> Option<&str> {
        match &self.kind {
            TokenKind::Text(value) | TokenKind::String(value) => Some(value),
            _ => None,
        }
    }

    fn is(&self, value: &str) -> bool {
        self.text()
            .is_some_and(|text| text.eq_ignore_ascii_case(value))
    }

    fn is_punct(&self, value: char) -> bool {
        matches!(self.kind, TokenKind::Punct(ch) if ch == value)
    }
}

struct Cursor<'a> {
    source: &'a str,
    pos: usize,
    line: usize,
    column: usize,
}

fn lex(source: &str, path: &Path) -> Result<Vec<Token>, String> {
    let mut cursor = Cursor {
        source,
        pos: 0,
        line: 1,
        column: 1,
    };
    let mut tokens = Vec::new();

    while let Some(ch) = cursor.peek() {
        if ch.is_whitespace() {
            cursor.bump();
            continue;
        }
        if ch == '/' && cursor.peek_next() == Some('/') {
            cursor.bump();
            cursor.bump();
            while cursor.peek().is_some_and(|comment| comment != '\n') {
                cursor.bump();
            }
            continue;
        }

        let line = cursor.line;
        let column = cursor.column;
        let kind = if ch == '"' {
            cursor.bump();
            let mut value = String::new();
            loop {
                match cursor.peek() {
                    None => return Err(error(path, line, column, "unterminated string")),
                    Some('"') => {
                        cursor.bump();
                        break;
                    }
                    Some('\\') => {
                        cursor.bump();
                        let escaped = cursor
                            .bump()
                            .ok_or_else(|| error(path, line, column, "unterminated string"))?;
                        value.push(match escaped {
                            'n' => '\n',
                            'r' => '\r',
                            't' => '\t',
                            '"' => '"',
                            '\\' => '\\',
                            other => other,
                        });
                    }
                    Some(other) => {
                        value.push(other);
                        cursor.bump();
                    }
                }
            }
            TokenKind::String(value)
        } else if ch == '-' && cursor.peek_next() == Some('>') {
            cursor.bump();
            cursor.bump();
            TokenKind::Arrow
        } else if "{}:;,=().[]@".contains(ch) {
            cursor.bump();
            TokenKind::Punct(ch)
        } else {
            let mut value = String::new();
            while let Some(next) = cursor.peek() {
                if next.is_whitespace()
                    || "{}:;,=().[]@".contains(next)
                    || (next == '-' && cursor.peek_next() == Some('>'))
                    || (next == '/' && cursor.peek_next() == Some('/'))
                {
                    break;
                }
                value.push(next);
                cursor.bump();
            }
            if value.is_empty() {
                return Err(error(
                    path,
                    line,
                    column,
                    format!("unexpected character `{ch}`"),
                ));
            }
            TokenKind::Text(value)
        };
        tokens.push(Token { kind, line, column });
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        line: cursor.line,
        column: cursor.column,
    });
    Ok(tokens)
}

impl<'a> Cursor<'a> {
    fn peek(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    fn peek_next(&self) -> Option<char> {
        let mut chars = self.source[self.pos..].chars();
        chars.next();
        chars.next()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.pos += ch.len_utf8();
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(ch)
    }
}

struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    path: &'a Path,
}

impl<'a> Parser<'a> {
    fn document(mut self) -> Result<Vec<Decl>, String> {
        self.declarations(false)
    }

    fn declarations(&mut self, stop_at_brace: bool) -> Result<Vec<Decl>, String> {
        let mut declarations = Vec::new();
        while !self.at_eof() && !(stop_at_brace && self.current().is_punct('}')) {
            if self.current().is_punct(';') || self.current().is_punct(',') {
                self.bump();
                continue;
            }
            declarations.extend(self.declaration()?);
        }
        if stop_at_brace {
            self.expect_punct('}')?;
        } else if self.current().is_punct('}') {
            return self.fail("unexpected `}`");
        }
        Ok(declarations)
    }

    fn declaration(&mut self) -> Result<Vec<Decl>, String> {
        if self.current().is("project") {
            return self.project();
        }
        if self.current().is("include") {
            return Ok(vec![self.include()?]);
        }
        if self.current().is("page") {
            return Ok(vec![self.page()?]);
        }
        if device_kind(self.current().text().unwrap_or(""), None).is_some() {
            return Ok(vec![Decl::Device(self.device_body()?)]);
        }
        self.fail("expected project, include, device, or page declaration")
    }

    fn project(&mut self) -> Result<Vec<Decl>, String> {
        let token = self.bump();
        let loc = self.loc(token);
        let name = self.string("project name")?;
        Ok(vec![Decl::Project { name, loc }])
    }

    fn include(&mut self) -> Result<Decl, String> {
        let token = self.bump();
        let loc = self.loc(token);
        let path = self.string("include path")?;
        Ok(Decl::Include { path, loc })
    }

    fn device_body(&mut self) -> Result<DeviceDecl, String> {
        let kind_token = self.bump();
        let kind_name = kind_token.text().unwrap_or("").to_owned();
        let kind = device_kind(&kind_name, None).ok_or_else(|| {
            self.error_at(&kind_token, format!("unknown device kind `{kind_name}`"))
        })?;
        let tag = self.word("device tag")?;
        let description = self.string("device description")?;
        self.expect_punct('{')?;
        let loc = self.loc(kind_token);
        let mut fields = Vec::new();
        while !self.at_eof() && !self.current().is_punct('}') {
            fields.push(self.field(kind)?);
        }
        self.expect_punct('}')?;
        Ok(DeviceDecl {
            kind,
            tag,
            description,
            fields,
            loc,
        })
    }

    fn field(&mut self, kind: DeviceKind) -> Result<Field, String> {
        let start = self.current().clone();
        if kind == DeviceKind::Plc && start.is("output") {
            return self.plc_output();
        }
        if kind == DeviceKind::Contactor && start.is("coil") {
            return self.coil();
        }
        if kind == DeviceKind::Contactor && start.is("pole") {
            return self.pole();
        }

        let key = self.word("field key")?;
        let value = self.designator("field value")?;
        Ok(Field::KeyValue {
            key,
            value,
            loc: self.loc(start),
        })
    }

    fn plc_output(&mut self) -> Result<Field, String> {
        let start = self.bump();
        let name = self.designator("PLC output name")?;
        let description = self.string("PLC output description")?;
        Ok(Field::PlcOutput {
            name,
            description,
            loc: self.loc(start),
        })
    }

    fn coil(&mut self) -> Result<Field, String> {
        let start = self.bump();
        let from = self.word("coil source terminal")?;
        self.expect_arrow()?;
        let to = self.word("coil destination terminal")?;
        self.expect_punct('{')?;
        let key = self.word("coil field")?;
        if key != "voltage" {
            return self.fail("expected `voltage` in coil");
        }
        let voltage = self.word("coil voltage")?;
        self.expect_punct('}')?;
        Ok(Field::Coil {
            from,
            to,
            voltage,
            loc: self.loc(start),
        })
    }

    fn pole(&mut self) -> Result<Field, String> {
        let start = self.bump();
        let from = self.word("pole source terminal")?;
        self.expect_arrow()?;
        let to = self.word("pole destination terminal")?;
        Ok(Field::Pole {
            from,
            to,
            loc: self.loc(start),
        })
    }

    fn page(&mut self) -> Result<Decl, String> {
        let start = self.bump();
        let name = self.string("page name")?;
        self.expect_punct('{')?;
        let loc = self.loc(start);
        let mut connections = Vec::new();
        while !self.at_eof() && !self.current().is_punct('}') {
            connections.push(self.connection()?);
        }
        self.expect_punct('}')?;
        Ok(Decl::Page(PageDecl {
            name,
            connections,
            loc,
        }))
    }

    fn connection(&mut self) -> Result<ConnectionDecl, String> {
        let start = self.current().clone();
        if !self.current().is("connect") {
            return self.fail("expected `connect`");
        }
        self.bump();
        let from = self.terminal_ref()?;
        self.expect_arrow()?;
        let to = self.terminal_ref()?;
        Ok(ConnectionDecl {
            from,
            to,
            loc: self.loc(start),
        })
    }

    fn terminal_ref(&mut self) -> Result<TerminalRef, String> {
        let start = self.current().clone();
        let device = self.word("terminal device")?;
        self.expect_punct('.')?;
        let line = self.current().line;
        let mut parts = Vec::new();
        while !self.at_eof()
            && !matches!(self.current().kind, TokenKind::Arrow)
            && !self.current().is_punct('}')
            && self.current().line == line
            && !self.current().is("connect")
        {
            parts.push(self.bump());
        }
        if parts.is_empty() {
            return self.fail("expected terminal designator");
        }
        let terminal = render(&parts);
        Ok(TerminalRef {
            device,
            terminal,
            loc: self.loc(start),
        })
    }

    fn word(&mut self, what: &str) -> Result<String, String> {
        let token = self.bump();
        match token.kind {
            TokenKind::Text(text) => Ok(text),
            _ => Err(self.error_at(&token, format!("expected {what}"))),
        }
    }

    fn designator(&mut self, what: &str) -> Result<String, String> {
        let mut value = self.word(what)?;
        while self.current().is_punct('.') || self.current().is_punct(':') {
            let punctuation = self.bump();
            value.push(match punctuation.kind {
                TokenKind::Punct(ch) => ch,
                _ => unreachable!(),
            });
            value.push_str(&self.word(what)?);
        }
        Ok(value)
    }

    fn string(&mut self, what: &str) -> Result<String, String> {
        let token = self.bump();
        match token.kind {
            TokenKind::String(text) => Ok(text),
            _ => Err(self.error_at(&token, format!("expected quoted {what}"))),
        }
    }

    fn expect_arrow(&mut self) -> Result<(), String> {
        if matches!(self.current().kind, TokenKind::Arrow) {
            self.bump();
            Ok(())
        } else {
            self.fail("expected `->`")
        }
    }

    fn expect_punct(&mut self, punct: char) -> Result<(), String> {
        if self.current().is_punct(punct) {
            self.bump();
            Ok(())
        } else {
            self.fail(format!("expected `{punct}`"))
        }
    }

    fn current(&self) -> &Token {
        &self.tokens[self.pos]
    }

    fn bump(&mut self) -> Token {
        let token = self.tokens[self.pos].clone();
        self.pos += 1;
        token
    }

    fn at_eof(&self) -> bool {
        matches!(self.current().kind, TokenKind::Eof)
    }

    fn loc(&self, token: Token) -> SourceLoc {
        SourceLoc {
            path: self.path.to_path_buf(),
            line: token.line,
            column: token.column,
        }
    }

    fn error_at(&self, token: &Token, message: impl Into<String>) -> String {
        error(self.path, token.line, token.column, message)
    }

    fn fail<T>(&self, message: impl Into<String>) -> Result<T, String> {
        Err(self.error_at(self.current(), message))
    }
}

fn device_kind(name: &str, _next: Option<&str>) -> Option<DeviceKind> {
    match name {
        "power_supply" => Some(DeviceKind::PowerSupply),
        "supply" => Some(DeviceKind::Supply),
        "plc" => Some(DeviceKind::Plc),
        "contactor" => Some(DeviceKind::Contactor),
        "motor" => Some(DeviceKind::Motor),
        _ => None,
    }
}

fn render(tokens: &[Token]) -> String {
    let mut value = String::new();
    for token in tokens {
        let text = match &token.kind {
            TokenKind::Text(text) | TokenKind::String(text) => text,
            TokenKind::Punct(punct) => return render_with_punctuation(tokens, *punct, &value),
            TokenKind::Arrow => "->",
            TokenKind::Eof => "",
        };
        if !value.is_empty()
            && matches!(token.kind, TokenKind::Text(_) | TokenKind::String(_))
            && token.column > 1
        {
            value.push(' ');
        }
        value.push_str(text);
    }
    value
}

fn render_with_punctuation(tokens: &[Token], _first_punct: char, _prefix: &str) -> String {
    let mut value = String::new();
    for token in tokens {
        match &token.kind {
            TokenKind::Text(text) | TokenKind::String(text) => {
                if !value.is_empty() && token.column > 1 {
                    let previous_is_punct = value
                        .chars()
                        .last()
                        .is_some_and(|ch| "{}:;,=().[]@".contains(ch));
                    if !previous_is_punct {
                        value.push(' ');
                    }
                }
                value.push_str(text);
            }
            TokenKind::Punct(punct) => value.push(*punct),
            TokenKind::Arrow => value.push_str("->"),
            TokenKind::Eof => {}
        }
    }
    value
}

fn error(path: &Path, line: usize, column: usize, message: impl Into<String>) -> String {
    format!("{}:{}:{}: {}", path.display(), line, column, message.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mvp_ast::{Decl, DeviceKind, Field};
    use std::path::Path;

    #[test]
    fn parses_mvp_declarations_and_locations() {
        let source = r#"
            // compact MVP example
            project "Motor Starter"
            power_supply PS1 "24 V supply" {
                input 230VAC
                output 24VDC
            }
            plc PLC1 "controller" {
                supply 24VDC
                output Q0.0 "motor command"
            }
            contactor KM1 "motor contactor" {
                coil A1 -> A2 {
                    voltage 24VDC
                }
                pole 1/L1 -> 2/T1
            }
            page "Control" {
                connect PS1.+ -> PLC1.L+
                connect PLC1.Q0.0 -> KM1.A1
            }
        "#;
        let declarations = parse(source, Path::new("main.wira")).unwrap();
        assert!(matches!(&declarations[0], Decl::Project { name, .. } if name == "Motor Starter"));
        assert!(matches!(
            &declarations[1],
            Decl::Device(DeviceDecl {
                kind: DeviceKind::PowerSupply,
                ..
            })
        ));
        assert!(
            matches!(&declarations[2], Decl::Device(DeviceDecl { fields, .. }) if matches!(&fields[1], Field::PlcOutput { name, description, .. } if name == "Q0.0" && description == "motor command"))
        );
        assert!(
            matches!(&declarations[3], Decl::Device(DeviceDecl { fields, .. }) if matches!(&fields[0], Field::Coil { from, to, voltage, .. } if from == "A1" && to == "A2" && voltage == "24VDC"))
        );
        assert!(
            matches!(&declarations[4], Decl::Page(PageDecl { connections, .. }) if connections[1].from.terminal == "Q0.0")
        );
        assert_eq!(declarations[0].loc().line, 3);
    }

    #[test]
    fn supports_comments_flexible_spacing_and_punctuation() {
        let declarations = parse(
            "project \"X\" supply S \"s\" { voltage 400VAC phases 3 } page \"P\" { connect S.X1:1 -> S.Q0.0 }",
            Path::new("x.wira"),
        )
        .unwrap();
        let Decl::Page(page) = &declarations[2] else {
            panic!()
        };
        assert_eq!(page.connections[0].from.terminal, "X1:1");
        assert_eq!(page.connections[0].to.terminal, "Q0.0");
    }

    #[test]
    fn reports_path_line_and_column() {
        let error = parse("project \"X\" {", Path::new("bad.wira")).unwrap_err();
        assert!(error.starts_with("bad.wira:1:13:"), "{error}");
    }

    #[test]
    fn terminal_references_keep_real_markings() {
        let source = "page \"P\" {\nconnect KM1.4/T2 -> PLC1.Q0.0\nconnect PSU1.+ -> PSU1.-\nconnect D.A_B:1 -> D.X-Y\n}";
        let decls = parse(source, Path::new("refs.wira")).unwrap();
        let Decl::Page(page) = &decls[0] else {
            panic!()
        };
        let names: Vec<_> = page
            .connections
            .iter()
            .map(|c| {
                (
                    (c.from.device.as_str(), c.from.terminal.as_str()),
                    (c.to.device.as_str(), c.to.terminal.as_str()),
                )
            })
            .collect();
        assert_eq!(
            names,
            vec![
                (("KM1", "4/T2"), ("PLC1", "Q0.0")),
                (("PSU1", "+"), ("PSU1", "-")),
                (("D", "A_B:1"), ("D", "X-Y"))
            ]
        );
    }

    trait DeclLoc {
        fn loc(&self) -> &SourceLoc;
    }

    impl DeclLoc for Decl {
        fn loc(&self) -> &SourceLoc {
            match self {
                Decl::Project { loc, .. } | Decl::Include { loc, .. } => loc,
                Decl::Device(device) => &device.loc,
                Decl::Page(page) => &page.loc,
            }
        }
    }
}
