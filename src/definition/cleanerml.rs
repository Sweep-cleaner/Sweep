//! Importer for BleachBit's CleanerML (XML) cleaner definitions.
//!
//! This is what lets Sweep read all 100+ cleaners shipped with BleachBit
//! (`/usr/share/bleachbit/cleaners/*.xml`) without maintaining a second copy.
//!
//! Two security properties are preserved from the Python implementation:
//!
//! * a `<!DOCTYPE>` with an internal subset is rejected (entity expansion /
//!   billion-laughs attacks);
//! * `process` and `winreg` actions are dropped for cleaners that were loaded
//!   from a user-writable directory.

use std::collections::HashMap;

use quick_xml::events::attributes::Attributes;
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::core::error::{Error, Result};
use crate::definition::model::{
    ActionDef, CleanerDef, ObjectType, OptionDef, OsFilter, RunningCheck, SearchKind, Trust,
    VarDef, VarSearch, VarValue,
};

// Which actions an untrusted cleaner may use is decided by the provider
// registry's `privileged` flag (see `crate::action::provider::is_privileged`),
// so there is no separate hard-coded list to keep in sync.

/// Reject a DTD with an internal subset (XXE / entity-expansion guard).
///
/// quick-xml does not expand external entities, so an external-only DOCTYPE is
/// harmless; BleachBit rejects any internal subset, and so do we.
fn reject_internal_dtd(text: &str, file: &str) -> Result<()> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().check_end_names = false;
    reader.config_mut().trim_text(false);

    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::DocType(doc)) => {
                let raw = String::from_utf8_lossy(doc.as_ref()).to_string();
                if raw.contains('[') {
                    return Err(Error::definition(
                        file,
                        "DTD with an internal subset is not allowed",
                    ));
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => {
                return Err(Error::definition(file, err.to_string()));
            }
            _ => {}
        }
        buffer.clear();
    }
    Ok(())
}

/// Collect `key="value"` pairs.
fn attrs_to_map(attrs: Attributes<'_>) -> Result<HashMap<String, String>> {
    let mut map = HashMap::new();
    for attribute in attrs {
        let attribute = attribute.map_err(|err| Error::msg(err.to_string()))?;
        let key = String::from_utf8_lossy(attribute.key.as_ref()).to_string();
        let value = attribute
            .unescape_value()
            .map_err(|err| Error::msg(err.to_string()))?
            .into_owned();
        map.insert(key, value);
    }
    Ok(map)
}

fn attr(map: &HashMap<String, String>, key: &str) -> String {
    map.get(key).cloned().unwrap_or_default()
}

fn optional_attr(map: &HashMap<String, String>, key: &str) -> Option<String> {
    map.get(key)
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn bool_attr(map: &HashMap<String, String>, key: &str) -> bool {
    matches!(
        attr(map, key).to_lowercase().as_str(),
        "true" | "yes" | "1" | "t" | "y"
    )
}

struct Parser<'a> {
    file: &'a str,
    cleaner: CleanerDef,
    trust: Trust,
    ignored: Vec<String>,
    /// Current `<option>` while parsing.
    current_option: Option<OptionDef>,
    /// Text content of the element being read.
    text: String,
}

impl<'a> Parser<'a> {
    fn new(file: &'a str, trust: Trust) -> Self {
        Self {
            file,
            cleaner: CleanerDef::new(String::new(), String::new()),
            trust,
            ignored: Vec::new(),
            current_option: None,
            text: String::new(),
        }
    }

    fn err(&self, reason: impl Into<String>) -> Error {
        Error::definition(self.file, reason)
    }

    fn take_text(&mut self) -> String {
        std::mem::take(&mut self.text).trim().to_string()
    }

    fn parse(&mut self, text: &str) -> Result<()> {
        let mut reader = Reader::from_str(text);
        reader.config_mut().trim_text(false);

        let mut buffer = Vec::new();
        let mut path: Vec<String> = Vec::new();

        loop {
            match reader.read_event_into(&mut buffer) {
                Ok(Event::Start(element)) => {
                    let name = String::from_utf8_lossy(element.name().as_ref()).to_string();
                    let attrs = attrs_to_map(element.attributes())?;
                    self.text.clear();
                    path.push(name.clone());
                    self.start(&name, &attrs, &path)?;
                }
                Ok(Event::Empty(element)) => {
                    let name = String::from_utf8_lossy(element.name().as_ref()).to_string();
                    let attrs = attrs_to_map(element.attributes())?;
                    self.text.clear();
                    path.push(name.clone());
                    self.start(&name, &attrs, &path)?;
                    self.end(&name, &path)?;
                    path.pop();
                }
                Ok(Event::End(element)) => {
                    let name = String::from_utf8_lossy(element.name().as_ref()).to_string();
                    self.end(&name, &path)?;
                    path.pop();
                    self.text.clear();
                }
                Ok(Event::Text(node)) => {
                    let decoded = node
                        .unescape()
                        .map_err(|err| self.err(err.to_string()))?
                        .into_owned();
                    self.text.push_str(&decoded);
                }
                Ok(Event::CData(node)) => {
                    self.text.push_str(&String::from_utf8_lossy(node.as_ref()));
                }
                Ok(Event::Eof) => break,
                Ok(_) => {}
                Err(err) => return Err(self.err(err.to_string())),
            }
            buffer.clear();
        }

        Ok(())
    }

    fn start(
        &mut self,
        name: &str,
        attrs: &HashMap<String, String>,
        path: &[String],
    ) -> Result<()> {
        match name {
            "cleaner" => {
                self.cleaner.id = attr(attrs, "id");
                self.cleaner.os = OsFilter::new(&attr(attrs, "os"));
            }
            "option"
                if matches!(
                    path.get(path.len().wrapping_sub(2)).map(|s| s.as_str()),
                    Some("cleaner")
                ) =>
            {
                self.current_option = Some(OptionDef {
                    id: attr(attrs, "id"),
                    label: String::new(),
                    description: String::new(),
                    warning: None,
                    actions: Vec::new(),
                    os: OsFilter::new(&attr(attrs, "os")),
                });
            }
            "var" => {
                // handled in end() once we know the name
            }
            "action" => {
                self.handle_action(attrs)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_action(&mut self, attrs: &HashMap<String, String>) -> Result<()> {
        let command = attr(attrs, "command");
        let os = OsFilter::new(&attr(attrs, "os"));
        if !os.matches() {
            return Ok(());
        }
        if self.trust != Trust::Trusted && crate::action::provider::is_privileged(&command) {
            self.ignored.push(command);
            return Ok(());
        }

        let search = SearchKind::parse(&attr(attrs, "search")).unwrap_or(SearchKind::File);
        let object_type = match attr(attrs, "type").as_str() {
            "f" => Some(ObjectType::File),
            "d" => Some(ObjectType::Dir),
            _ => None,
        };

        let action = ActionDef {
            command,
            search,
            path: attr(attrs, "path"),
            os: OsFilter::any(),
            regex: optional_attr(attrs, "regex"),
            nregex: optional_attr(attrs, "nregex"),
            wholeregex: optional_attr(attrs, "wholeregex"),
            nwholeregex: optional_attr(attrs, "nwholeregex"),
            object_type,
            section: optional_attr(attrs, "section"),
            parameter: optional_attr(attrs, "parameter"),
            address: optional_attr(attrs, "address"),
            cmd: optional_attr(attrs, "cmd"),
            wait: !matches!(
                attr(attrs, "wait").to_lowercase().chars().next(),
                Some('f' | 'n')
            ),
            reg_name: optional_attr(attrs, "name"),
            ..ActionDef::default()
        };

        match self.current_option.as_mut() {
            Some(option) => option.actions.push(action),
            None => {
                return Err(self.err("<action> outside of <option>"));
            }
        }
        Ok(())
    }

    fn end(&mut self, name: &str, path: &[String]) -> Result<()> {
        let parent = path
            .get(path.len().wrapping_sub(2))
            .map(|s| s.as_str())
            .unwrap_or("");

        match (parent, name) {
            ("cleaner", "label") => self.cleaner.name = self.take_text(),
            ("cleaner", "description") => self.cleaner.description = self.take_text(),
            ("option", "label") => {
                let text = self.take_text();
                if let Some(option) = self.current_option.as_mut() {
                    option.label = text;
                }
            }
            ("option", "description") => {
                let text = self.take_text();
                if let Some(option) = self.current_option.as_mut() {
                    option.description = text;
                }
            }
            ("option", "warning") => {
                let text = self.take_text();
                if let Some(option) = self.current_option.as_mut() {
                    option.warning = Some(text);
                }
            }
            ("cleaner", "option") => {
                if let Some(option) = self.current_option.take() {
                    if !option.label.is_empty() || !option.actions.is_empty() {
                        self.cleaner.options.push(option);
                    }
                }
            }
            _ => {}
        }

        let _ = parent;
        Ok(())
    }
}

/// Parse one CleanerML document.
pub fn parse_cleanerml(text: &str, file: &str, trust: Trust) -> Result<CleanerDef> {
    reject_internal_dtd(text, file)?;

    let mut parser = Parser::new(file, trust);
    parser.parse(text)?;

    // A second, simpler pass collects <var> blocks, which need sibling
    // attributes and are easier to read with a targeted scan.
    parser.cleaner.vars = parse_vars(text, file)?;
    parser.cleaner.running = parse_running(text, file)?;

    if parser.cleaner.id.is_empty() {
        return Err(Error::definition(file, "missing <cleaner id=\"...\">"));
    }

    if !parser.ignored.is_empty() {
        parser.ignored.sort();
        parser.ignored.dedup();
        log::warn!(
            "ignoring {} action(s) from untrusted cleaner {}: {}",
            parser.ignored.len(),
            file,
            parser.ignored.join(", ")
        );
    }

    Ok(parser.cleaner)
}

/// Second pass: `<var name="x"><value os="...">text</value></var>`.
fn parse_vars(text: &str, file: &str) -> Result<Vec<VarDef>> {
    let mut vars: Vec<VarDef> = Vec::new();
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);

    let mut buffer = Vec::new();
    let mut in_var = false;
    let mut current_name = String::new();
    let mut pending_value: Option<(VarSearch, Option<String>, String, OsFilter)> = None;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) | Ok(Event::Empty(element)) => {
                let name = String::from_utf8_lossy(element.name().as_ref()).to_string();
                let attrs = attrs_to_map(element.attributes())?;
                match name.as_str() {
                    "var" => {
                        in_var = true;
                        current_name = attr(&attrs, "name");
                    }
                    "value" if in_var => {
                        let search = match attr(&attrs, "search").as_str() {
                            "glob" => VarSearch::Glob,
                            "winreg" => VarSearch::Registry,
                            _ => VarSearch::Literal,
                        };
                        pending_value = Some((
                            search,
                            optional_attr(&attrs, "name"),
                            String::new(),
                            OsFilter::new(&attr(&attrs, "os")),
                        ));
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(node)) => {
                if let Some((_, _, ref mut content, _)) = pending_value {
                    if let Ok(decoded) = node.unescape() {
                        content.push_str(&decoded);
                    }
                }
            }
            Ok(Event::End(element)) => {
                let name = String::from_utf8_lossy(element.name().as_ref()).to_string();
                match name.as_str() {
                    "var" => {
                        in_var = false;
                        current_name.clear();
                    }
                    "value" if in_var => {
                        if let Some((search, reg_name, content, os)) = pending_value.take() {
                            if let Some(var) = vars.iter_mut().find(|v| v.name == current_name) {
                                var.values.push(VarValue {
                                    raw: content.trim().to_string(),
                                    search,
                                    os,
                                    reg_name,
                                });
                            } else {
                                vars.push(VarDef {
                                    name: current_name.clone(),
                                    values: vec![VarValue {
                                        raw: content.trim().to_string(),
                                        search,
                                        os,
                                        reg_name,
                                    }],
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(Error::definition(file, err.to_string())),
            _ => {}
        }
        buffer.clear();
    }

    Ok(vars)
}

/// Second pass: `<running type="exe" os="linux" same_user="true">firefox</running>`.
fn parse_running(text: &str, file: &str) -> Result<Vec<RunningCheck>> {
    let mut checks = Vec::new();
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(true);

    let mut buffer = Vec::new();
    let mut pending: Option<(String, String, bool, OsFilter, String)> = None;

    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(element)) | Ok(Event::Empty(element)) => {
                let name = String::from_utf8_lossy(element.name().as_ref()).to_string();
                if name == "running" {
                    let attrs = attrs_to_map(element.attributes())?;
                    pending = Some((
                        attr(&attrs, "type"),
                        String::new(),
                        bool_attr(&attrs, "same_user"),
                        OsFilter::new(&attr(&attrs, "os")),
                        String::new(),
                    ));
                }
            }
            Ok(Event::Text(node)) => {
                if let Some((_, ref mut content, _, _, _)) = pending {
                    if let Ok(decoded) = node.unescape() {
                        content.push_str(&decoded);
                    }
                }
            }
            Ok(Event::End(element)) => {
                let name = String::from_utf8_lossy(element.name().as_ref()).to_string();
                if name == "running" {
                    if let Some((kind, content, same_user, os, _)) = pending.take() {
                        let value = content.trim().to_string();
                        if value.is_empty() {
                            continue;
                        }
                        match kind.as_str() {
                            "exe" => checks.push(RunningCheck::Exe {
                                name: value,
                                same_user,
                                os,
                            }),
                            "pathname" | "path" => {
                                checks.push(RunningCheck::Path { pattern: value, os })
                            }
                            other => {
                                return Err(Error::definition(
                                    file,
                                    format!("unknown <running type=\"{other}\">"),
                                ))
                            }
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(Error::definition(file, err.to_string())),
            _ => {}
        }
        buffer.clear();
    }

    Ok(checks)
}

/// Parse a single CleanerML file, tolerating a directory-wide read failure.
pub fn load_cleanerml_dir_entry(path: &std::path::Path, trust: Trust) -> Vec<Result<CleanerDef>> {
    match std::fs::read_to_string(path) {
        Ok(text) => vec![parse_cleanerml(&text, &path.display().to_string(), trust)],
        Err(err) => vec![Err(Error::io(path, err))],
    }
}
