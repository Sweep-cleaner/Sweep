//! Native (TOML) cleaner definitions.
//!
//! BleachBit stores cleaners as XML because that is what was convenient in
//! 2008. Sweep's own format is TOML: it is smaller, needs no entity
//! escaping for Windows paths full of backslashes, comments are cheap, and
//! `serde` gives us a typed schema — a typo in a cleaner is a parse error at
//! load time instead of a silently ignored attribute at clean time.
//!
//! ```toml
//! id = "firefox"
//! name = "Firefox"
//! description = "Web browser"
//!
//! [[running]]
//! type = "exe"
//! value = "firefox"
//! os = "unix"
//!
//! [[var]]
//! name = "profile"
//! [[var.value]]
//! os = "unix"
//! search = "glob"
//! value = "~/.mozilla/firefox/*"
//!
//! [[option]]
//! id = "cache"
//! label = "Cache"
//! description = "Delete the cache"
//!
//! [[option.action]]
//! command = "delete"
//! search = "walk.all"
//! path = "~/.cache/mozilla/"
//! ```

use serde::Deserialize;

use crate::core::error::{Error, Result};
use crate::definition::model::{
    ActionDef, CleanerDef, ObjectType, OptionDef, OsFilter, RunningCheck, SearchKind, VarDef,
    VarSearch, VarValue,
};

/// Size strings like `"10MB"` are accepted by serde through this wrapper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct ByteSize(Option<u64>);

impl<'de> Deserialize<'de> for ByteSize {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = toml::Value::deserialize(deserializer)?;
        match raw {
            toml::Value::Integer(n) if n >= 0 => Ok(ByteSize(Some(n as u64))),
            toml::Value::String(text) => crate::fsutil::size::human_to_bytes(&text)
                .map(|n| ByteSize(Some(n)))
                .map_err(serde::de::Error::custom),
            other => Err(serde::de::Error::custom(format!(
                "expected an integer or a size string, got {other:?}"
            ))),
        }
    }
}

#[derive(Debug, Deserialize)]
struct NativeDef {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    os: String,
    #[serde(default)]
    running: Vec<RunningToml>,
    #[serde(default)]
    var: Vec<VarToml>,
    #[serde(default)]
    option: Vec<OptionToml>,
}

#[derive(Debug, Deserialize)]
struct RunningToml {
    #[serde(rename = "type")]
    kind: String,
    value: String,
    #[serde(default)]
    os: String,
    #[serde(default)]
    same_user: bool,
}

#[derive(Debug, Deserialize)]
struct VarToml {
    name: String,
    #[serde(default)]
    value: Vec<VarValueToml>,
}

#[derive(Debug, Deserialize)]
struct VarValueToml {
    #[serde(default)]
    value: String,
    #[serde(default)]
    search: String,
    #[serde(default)]
    os: String,
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct OptionToml {
    id: String,
    label: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    warning: Option<String>,
    #[serde(default)]
    os: String,
    #[serde(default)]
    action: Vec<ActionToml>,
}

#[derive(Debug, Deserialize)]
struct ActionToml {
    command: String,
    #[serde(default)]
    search: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    os: String,
    #[serde(default)]
    regex: Option<String>,
    #[serde(default)]
    nregex: Option<String>,
    #[serde(default)]
    wholeregex: Option<String>,
    #[serde(default)]
    nwholeregex: Option<String>,
    #[serde(default)]
    #[serde(rename = "type")]
    object_type: Option<String>,

    #[serde(default)]
    max_age_days: Option<u32>,
    #[serde(default)]
    min_age_days: Option<u32>,
    #[serde(default)]
    min_size: Option<ByteSize>,
    #[serde(default)]
    max_size: Option<ByteSize>,
    #[serde(default)]
    max_depth: Option<usize>,
    #[serde(default)]
    include_dirs: Option<bool>,

    #[serde(default)]
    section: Option<String>,
    #[serde(default)]
    parameter: Option<String>,
    #[serde(default)]
    address: Option<String>,
    #[serde(default)]
    cmd: Option<String>,
    #[serde(default = "default_true")]
    wait: bool,
    #[serde(default)]
    reg_name: Option<String>,
    #[serde(default)]
    exclude_keys: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl ActionToml {
    fn into_action(self, file: &str) -> Result<ActionDef> {
        let search = if self.search.is_empty() {
            // Sensible default: a path with a wildcard is a glob, otherwise it
            // is a literal file.
            if crate::fsutil::walk::has_glob(&self.path) {
                SearchKind::Glob
            } else {
                SearchKind::File
            }
        } else {
            SearchKind::parse(&self.search).ok_or_else(|| {
                Error::definition(file, format!("invalid search='{}'", self.search))
            })?
        };

        let object_type = match self.object_type.as_deref() {
            Some("f") | Some("file") => Some(ObjectType::File),
            Some("d") | Some("dir") | Some("directory") => Some(ObjectType::Dir),
            None => None,
            Some(other) => {
                return Err(Error::definition(
                    file,
                    format!("invalid type='{other}' (expected 'f' or 'd')"),
                ))
            }
        };

        Ok(ActionDef {
            command: self.command,
            search,
            path: self.path,
            os: OsFilter::new(&self.os),
            regex: nonempty(self.regex),
            nregex: nonempty(self.nregex),
            wholeregex: nonempty(self.wholeregex),
            nwholeregex: nonempty(self.nwholeregex),
            object_type,
            max_age_days: self.max_age_days,
            min_age_days: self.min_age_days,
            min_size: self.min_size.and_then(|b| b.0),
            max_size: self.max_size.and_then(|b| b.0),
            max_depth: self.max_depth,
            include_dirs: self.include_dirs,
            section: self.section,
            parameter: self.parameter,
            address: self.address,
            cmd: self.cmd,
            wait: self.wait,
            reg_name: nonempty(self.reg_name),
            exclude_keys: self.exclude_keys,
        })
    }
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

/// Parse a native TOML cleaner definition.
pub fn parse_native(text: &str, file: &str) -> Result<CleanerDef> {
    let raw: NativeDef =
        toml::from_str(text).map_err(|err| Error::definition(file, err.to_string()))?;

    let mut running = Vec::new();
    for entry in raw.running {
        match entry.kind.as_str() {
            "exe" => running.push(RunningCheck::Exe {
                name: entry.value,
                same_user: entry.same_user,
                os: OsFilter::new(&entry.os),
            }),
            "path" | "pathname" => running.push(RunningCheck::Path {
                pattern: entry.value,
                os: OsFilter::new(&entry.os),
            }),
            other => {
                return Err(Error::definition(
                    file,
                    format!("unknown running type '{other}' (expected 'exe' or 'path')"),
                ))
            }
        }
    }

    let mut vars = Vec::new();
    for var in raw.var {
        let mut values = Vec::new();
        for value in var.value {
            let search = match value.search.as_str() {
                "" | "literal" => VarSearch::Literal,
                "glob" => VarSearch::Glob,
                "winreg" | "registry" => VarSearch::Registry,
                other => {
                    return Err(Error::definition(
                        file,
                        format!("unknown var search '{other}'"),
                    ))
                }
            };
            values.push(VarValue {
                raw: value.value,
                search,
                os: OsFilter::new(&value.os),
                reg_name: nonempty(Some(value.name)),
            });
        }
        vars.push(VarDef {
            name: var.name,
            values,
        });
    }

    let mut options = Vec::new();
    for option in raw.option {
        let mut actions = Vec::new();
        for action in option.action {
            actions.push(action.into_action(file)?);
        }
        options.push(OptionDef {
            id: option.id,
            label: option.label,
            description: option.description,
            warning: option.warning,
            actions,
            os: OsFilter::new(&option.os),
        });
    }

    Ok(CleanerDef {
        id: raw.id,
        name: raw.name,
        description: raw.description,
        os: OsFilter::new(&raw.os),
        running,
        vars,
        options,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
id = "demo"
name = "Demo"
description = "A demo cleaner"

[[var]]
name = "profile"
[[var.value]]
os = "unix"
search = "glob"
value = "~/.config/demo/*"

[[option]]
id = "cache"
label = "Cache"
description = "Delete the cache"

[[option.action]]
command = "delete"
search = "walk.all"
path = "$$profile$$/cache/"

[[option.action]]
command = "delete"
search = "glob"
path = "$$profile$$/*.log"
max_age_days = 7
min_size = "1MB"
"#;

    #[test]
    fn parses_native_definition() {
        let def = parse_native(SAMPLE, "demo.toml").unwrap();
        assert_eq!(def.id, "demo");
        assert_eq!(def.vars.len(), 1);
        let option = def.option("cache").unwrap();
        assert_eq!(option.actions.len(), 2);
        assert_eq!(option.actions[1].max_age_days, Some(7));
        assert_eq!(option.actions[1].min_size, Some(1_000_000));
        assert_eq!(option.actions[0].search, SearchKind::WalkAll);
    }
}
