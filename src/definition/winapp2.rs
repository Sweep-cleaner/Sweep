//! Importer for Winapp2.ini.
//!
//! Winapp2.ini is the community-maintained cleaner database for CCleaner;
//! BleachBit can import it and so can Sweep. Roughly a thousand
//! `[Section]` blocks map onto our cleaner model as follows:
//!
//! * `LangSecRef` → cleaner id (through BleachBit's `langsecref` table);
//! * `FileKey1..N` → `delete` actions;
//! * `RegKey1..N`  → `winreg` actions (trusted cleaners only);
//! * `ExcludeKey1..N` → `nwholeregex`;
//! * `Detect` / `DetectFile` / `DetectOS` → whether the section is shown.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::error::{Error, Result};
use crate::definition::model::{
    ActionDef, CleanerDef, ObjectType, OptionDef, OsFilter, SearchKind, Trust,
};

/// The subset of BleachBit's `langsecref` → cleaner-id table we support.
///
/// Unknown references become `winapp2_<slug>` cleaners, exactly like
/// BleachBit's `section_to_cleanerid()`.
const LANGSECREF: &[(&str, &str, &str)] = &[
    ("3021", "winapp2_applications", "Applications"),
    ("3022", "winapp2_internet", "Internet"),
    ("3023", "winapp2_multimedia", "Multimedia"),
    ("3024", "winapp2_utilities", "Utilities"),
    ("3025", "winapp2_games", "Games"),
    ("3026", "winapp2_windows", "Windows"),
    ("3027", "winapp2_office", "Office"),
    ("3028", "winapp2_mozilla", "Firefox"),
    ("3029", "winapp2_thunderbird", "Thunderbird"),
    ("3030", "winapp2_google_chrome", "Google Chrome"),
    ("3031", "winapp2_opera", "Opera"),
    ("3032", "winapp2_safari", "Safari"),
    ("3033", "winapp2_registry", "Registry"),
    ("3034", "winapp2_drives", "Drives"),
    ("3035", "winapp2_fonts", "Fonts"),
];

/// `SpecialDetect` codes and what they mean.
const SPECIAL_DETECT: &[(&str, &str)] = &[
    ("DET_CHROME", "Chrome based browser"),
    ("DET_MOZILLA", "Mozilla based browser"),
    ("DET_THUNDERBIRD", "Thunderbird based mail client"),
    ("DET_OPERA", "Opera based browser"),
    ("DET_JAVA", "Java"),
    ("DET_WINDOWS", "Windows"),
];

/// A parsed INI file: section → key → value. Keys are folded to lower case,
/// section names and values are not.
type Ini = HashMap<String, HashMap<String, String>>;

/// Minimal INI reader (no external dependency, tolerant of the odd formatting
/// found in the wild: BOMs, CRLF, `;` comments, duplicate keys).
///
/// **Keys are folded to lower case.** Winapp2 writes `LangSecRef`, `DetectFile`
/// and `DetectOS`, while the importer looks them up as `langsecref`,
/// `detectfile` and `detectos`; preserving the author's capitalisation made
/// every one of those lookups miss, which silently disabled the `Detect*`
/// checks and the whole `LangSecRef` → cleaner-id table. Values keep their case
/// because they are paths and registry keys, where case can matter.
fn parse_ini(text: &str) -> Ini {
    let mut ini: Ini = HashMap::new();
    let mut current = String::new();

    for raw_line in text.lines() {
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            current = line[1..line.len() - 1].trim().to_string();
            ini.entry(current.clone()).or_default();
            continue;
        }
        if current.is_empty() {
            continue;
        }
        let (key, value) = match line.split_once('=') {
            Some((key, value)) => (key.trim().to_ascii_lowercase(), value.trim().to_string()),
            None => (line.trim().to_ascii_lowercase(), String::new()),
        };
        let entry = ini.entry(current.clone()).or_default();
        // Winapp2 uses FileKey1, FileKey2, ... so duplicates are impossible,
        // but be defensive: first occurrence wins.
        entry.entry(key).or_insert(value);
    }

    ini
}

/// Lowercase slug used to build cleaner ids.
fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('_');
            last_dash = true;
        }
    }
    out.trim_matches('_').to_string()
}

/// Translate `FILE=`/`PATH=` `ExcludeKey` entries into a negative whole-path
/// regex, mirroring BleachBit's `excludekey_to_nwholeregex()`.
fn exclude_to_nwholeregex(exclude: &str) -> Option<String> {
    let parts: Vec<&str> = exclude.split('|').collect();
    let kind = parts.first()?.to_uppercase();

    match kind.as_str() {
        "REG" => None, // registry exclusions are handled separately
        "FILE" | "PATH" => {
            let path = crate::core::path::expand(parts.get(1).unwrap_or(&""));
            let files = parts.get(2);

            let regexes: Vec<String> = std::iter::once(path.clone())
                .map(|p| match files {
                    None => fnmatch_regex(&p.to_string_lossy()),
                    Some(files) => {
                        let patterns: Vec<&str> = files.split(';').collect();
                        if patterns.len() == 1 && patterns[0] == "*.*" {
                            fnmatch_regex(&p.to_string_lossy())
                        } else if patterns.len() == 1 {
                            format!(
                                "{}\\\\{}",
                                strip_trailing_sep(&fnmatch_regex(&p.to_string_lossy())),
                                fnmatch_regex(patterns[0])
                            )
                        } else {
                            let alternatives: Vec<String> =
                                patterns.iter().map(|p| fnmatch_regex(p)).collect();
                            format!(
                                "{}\\\\({})",
                                strip_trailing_sep(&fnmatch_regex(&p.to_string_lossy())),
                                alternatives.join("|")
                            )
                        }
                    }
                })
                .collect();

            match regexes.as_slice() {
                [] => None,
                [single] => Some(single.clone()),
                many => Some(format!("({})", many.join("|"))),
            }
        }
        _ => None,
    }
}

fn strip_trailing_sep(regex: &str) -> String {
    regex
        .trim_end_matches("\\\\")
        .trim_end_matches('/')
        .to_string()
}

/// Translate one fnmatch pattern to a regex fragment.
fn fnmatch_regex(pattern: &str) -> String {
    let mut out = String::with_capacity(pattern.len() * 2);
    for ch in pattern.chars() {
        match ch {
            '*' => out.push_str(".*"),
            '?' => out.push('.'),
            '.' => out.push_str("\\."),
            '+' => out.push_str("\\+"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '[' => out.push('['),
            ']' => out.push(']'),
            '^' => out.push_str("\\^"),
            '$' => out.push_str("\\$"),
            '|' => out.push_str("\\|"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\\' | '/' => out.push_str("\\\\"),
            other => out.push(other),
        }
    }
    out
}

/// Expand the `%Foo%` variables Winapp2 relies on.
///
/// Returns **no** candidate when a variable is not defined on this machine.
/// Winapp2 is a Windows database, so on Linux every `%ProgramFiles%`-style
/// path would otherwise collapse to the empty string and leave a root-relative
/// path such as `/Foo` behind — a location nobody asked us to touch.
fn winapp_expand_vars(text: &str) -> Vec<String> {
    crate::core::path::expand_defined(text)
        .map(|path| path.to_string_lossy().into_owned())
        .into_iter()
        .collect()
}

/// Is this section active on the current machine?
fn detect(ini: &Ini, section: &str) -> bool {
    let table = match ini.get(section) {
        Some(table) => table,
        None => return false,
    };

    // DetectOS: minimum Windows version, e.g. `-5.1` means "before 5.1".
    if let Some(required) = table.get("detectos") {
        if !detect_os(required) {
            return false;
        }
    }

    let mut saw_detect = false;

    if let Some(path) = table.get("detectfile") {
        saw_detect = true;
        for candidate in winapp_expand_vars(path) {
            let path = PathBuf::from(&candidate);
            if crate::fsutil::walk::has_glob(&candidate) {
                if !crate::fsutil::walk::glob_paths(&candidate)
                    .unwrap_or_default()
                    .is_empty()
                {
                    return true;
                }
            } else if path.exists() {
                return true;
            }
        }
    }

    if let Some(key) = table.get("detect") {
        saw_detect = true;
        for candidate in winapp_expand_vars(key) {
            #[cfg(windows)]
            if crate::platform::windows::reg_key_exists(&candidate) {
                return true;
            }
            #[cfg(not(windows))]
            let _ = &candidate;
        }
    }

    if let Some(code) = table.get("specialdetect") {
        saw_detect = true;
        if special_detect(code) {
            return true;
        }
    }

    // No Detect* directive at all: the section is always active.
    !saw_detect
}

#[cfg(windows)]
fn detect_os(required: &str) -> bool {
    let (negated, version) = match required.strip_prefix('-') {
        Some(v) => (true, v),
        None => (false, required),
    };
    let current = current_windows_version();
    let matches = match version {
        "10.0" | "11.0" => current >= (10, 0),
        "6.3" => current >= (6, 3),
        "6.2" => current >= (6, 2),
        "6.1" => current >= (6, 1),
        "6.0" => current >= (6, 0),
        "5.1" => current >= (5, 1),
        _ => true,
    };
    if negated {
        !matches
    } else {
        matches
    }
}

#[cfg(not(windows))]
fn detect_os(_required: &str) -> bool {
    false
}

#[cfg(windows)]
fn current_windows_version() -> (u32, u32) {
    // `ver` prints something like "Microsoft Windows [Version 10.0.19045.4046]"
    let output = std::process::Command::new("cmd")
        .args(["/c", "ver"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();

    let digits = output
        .split_whitespace()
        .find(|part| {
            part.chars()
                .next()
                .map(|c| c.is_ascii_digit())
                .unwrap_or(false)
        })
        .unwrap_or("0.0");
    let mut parts = digits.split('.');
    let major = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    let minor = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    (major, minor)
}

fn special_detect(code: &str) -> bool {
    if let Some((_, description)) = SPECIAL_DETECT.iter().find(|(c, _)| *c == code) {
        log::debug!("SpecialDetect {code} ({description}): treated as active");
        true
    } else {
        log::debug!("unknown SpecialDetect '{code}'");
        false
    }
}

/// Build one `delete` action from a `FileKey` value.
fn filekey_actions(value: &str, excludes: &[String], file: &str) -> Result<Vec<ActionDef>> {
    let mut parts: Vec<&str> = value.split('|').collect();
    if parts.is_empty() {
        return Err(Error::definition(file, "empty FileKey"));
    }

    let dir_raw = parts.remove(0).trim();
    let filenames = if parts.is_empty() {
        String::new()
    } else {
        parts.remove(0).trim().to_string()
    };

    let mut recurse = false;
    let mut removeself = false;
    for flag in &parts {
        match flag.trim().to_uppercase().as_str() {
            "RECURSE" => recurse = true,
            "REMOVESELF" => {
                recurse = true;
                removeself = true;
            }
            other => log::debug!("unknown FileKey flag '{other}' in {file}"),
        }
    }

    let nwholeregex = match excludes {
        [] => None,
        [single] => exclude_to_nwholeregex(single).or_else(|| Some(join_excludes(excludes))),
        _ => Some(join_excludes(excludes)),
    };

    let mut actions = Vec::new();

    for dirname in winapp_expand_vars(dir_raw) {
        // A bare drive letter ("C:") needs a trailing separator before it can
        // be joined with anything.
        let dirname = if dirname.len() == 2 && dirname.ends_with(':') {
            format!("{dirname}\\")
        } else {
            dirname
        };

        // A relative FileKey would be resolved against the current working
        // directory at run time, so the same section would delete different
        // things depending on where `sweep` was started. Winapp2 entries are
        // always absolute; anything else is malformed or hostile.
        if !crate::core::path::looks_absolute(&dirname) {
            log::warn!("ignoring relative FileKey path '{dirname}' in {file}");
            continue;
        }

        for filename in filenames.split(';') {
            let filename = filename.trim();
            if filename.is_empty() {
                continue;
            }

            if recurse {
                let path = PathBuf::from(&dirname);
                if filename == "*.*" {
                    actions.push(ActionDef {
                        command: "delete".into(),
                        search: if removeself {
                            SearchKind::WalkAll
                        } else {
                            SearchKind::WalkFiles
                        },
                        path: path.to_string_lossy().into_owned(),
                        nwholeregex: nwholeregex.clone(),
                        ..ActionDef::default()
                    });
                } else {
                    actions.push(ActionDef {
                        command: "delete".into(),
                        search: SearchKind::WalkFiles,
                        path: path.to_string_lossy().into_owned(),
                        regex: Some(format!("^{}$", fnmatch_regex(filename))),
                        nwholeregex: nwholeregex.clone(),
                        ..ActionDef::default()
                    });
                }
            } else {
                let joined = PathBuf::from(&dirname).join(filename);
                let joined_text = joined.to_string_lossy().to_string();
                let search = if crate::fsutil::walk::has_glob(&joined_text) {
                    SearchKind::Glob
                } else {
                    SearchKind::File
                };
                actions.push(ActionDef {
                    command: "delete".into(),
                    search,
                    path: joined_text,
                    nwholeregex: nwholeregex.clone(),
                    ..ActionDef::default()
                });
            }
        }

        if removeself {
            let dir_text = dirname.clone();
            let search = if crate::fsutil::walk::has_glob(&dir_text) {
                SearchKind::Glob
            } else {
                SearchKind::File
            };
            actions.push(ActionDef {
                command: "delete".into(),
                search,
                path: dir_text,
                object_type: Some(ObjectType::Dir),
                ..ActionDef::default()
            });
        }
    }

    Ok(actions)
}

fn join_excludes(excludes: &[String]) -> String {
    let regexes: Vec<String> = excludes
        .iter()
        .filter_map(|e| exclude_to_nwholeregex(e))
        .collect();
    if regexes.is_empty() {
        String::new()
    } else {
        format!("({})", regexes.join("|"))
    }
}

/// Build a `winreg` action from a `RegKey` value.
fn regkey_action(value: &str, reg_excludes: &[String]) -> ActionDef {
    let mut parts = value.split('|');
    let path = parts.next().unwrap_or("").trim().to_string();
    let name = parts
        .next()
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());

    // Skip keys covered by an exclusion (exact match or prefix).
    let normalized = path.to_uppercase();
    let excluded = reg_excludes.iter().any(|exclude| {
        let exclude = exclude.to_uppercase();
        normalized == exclude || {
            let mut prefix = exclude.clone();
            prefix.push('\\');
            normalized.starts_with(&prefix)
        }
    });

    if excluded {
        log::debug!("skipping excluded registry key {path}");
    }

    ActionDef {
        command: "winreg".into(),
        search: SearchKind::File,
        path,
        reg_name: name,
        exclude_keys: reg_excludes.to_vec(),
        ..ActionDef::default()
    }
}

/// Parse a winapp2.ini file into zero or more cleaners.
pub fn parse_winapp2(text: &str, file: &str, trust: Trust) -> Result<Vec<CleanerDef>> {
    let ini = parse_ini(text);
    let mut cleaners: HashMap<String, CleanerDef> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for section in sorted_sections(&ini) {
        let table = &ini[&section];

        if let Some(required) = table.get("detectos") {
            let _ = required; // handled by detect()
        }
        if !detect(&ini, &section) {
            log::debug!("winapp2 section '{section}' is inactive on this system");
            continue;
        }

        let langsecref = table.get("langsecref").cloned().unwrap_or_default();
        let (cleaner_id, cleaner_name) =
            match LANGSECREF.iter().find(|(code, _, _)| *code == langsecref) {
                Some((_, id, name)) => ((*id).to_string(), (*name).to_string()),
                None => {
                    let name = if langsecref.is_empty() {
                        section.clone()
                    } else {
                        langsecref.clone()
                    };
                    (format!("winapp2_{}", slug(&name)), name)
                }
            };

        // Collect exclusions for this section.
        let mut excludes: Vec<String> = Vec::new();
        let mut reg_excludes: Vec<String> = Vec::new();
        for (key, value) in table.iter() {
            let upper = key.to_uppercase();
            if upper.starts_with("EXCLUDEKEY") && !value.trim().is_empty() {
                if value.trim().to_uppercase().starts_with("REG|") {
                    if let Some(rest) = value.split_once('|').map(|(_, r)| r) {
                        reg_excludes.push(rest.trim().to_string());
                    }
                } else {
                    excludes.push(value.trim().to_string());
                }
            }
        }

        let mut actions: Vec<ActionDef> = Vec::new();
        let mut keys: Vec<(&String, &String)> = table.iter().collect();
        keys.sort_by(|a, b| a.0.cmp(b.0));

        for (key, value) in keys {
            let upper = key.to_uppercase();
            if upper.starts_with("FILEKEY") {
                match filekey_actions(value, &excludes, file) {
                    Ok(mut built) => actions.append(&mut built),
                    Err(err) => log::debug!("{err}"),
                }
            } else if upper.starts_with("REGKEY") {
                if trust.allows_registry() {
                    actions.push(regkey_action(value, &reg_excludes));
                } else {
                    log::debug!(
                        "ignoring 'winreg' action from untrusted winapp2.ini section '{section}'"
                    );
                }
            }
        }

        if actions.is_empty() {
            continue;
        }

        let option = OptionDef {
            id: slug(&section),
            label: table
                .get("section")
                .cloned()
                .unwrap_or_else(|| section.clone()),
            description: table
                .get("description")
                .cloned()
                .unwrap_or_else(|| format!("Imported from {file}")),
            warning: table.get("warning").cloned(),
            actions,
            os: OsFilter::new("windows"),
        };

        match cleaners.get_mut(&cleaner_id) {
            Some(cleaner) => cleaner.options.push(option),
            None => {
                let mut cleaner = CleanerDef::new(cleaner_id.clone(), cleaner_name);
                cleaner.description = format!("Imported from {file}");
                cleaner.os = OsFilter::new("windows");
                cleaner.options.push(option);
                order.push(cleaner_id.clone());
                cleaners.insert(cleaner_id, cleaner);
            }
        }
    }

    Ok(order
        .into_iter()
        .filter_map(|id| cleaners.remove(&id))
        .collect())
}

fn sorted_sections(ini: &Ini) -> Vec<String> {
    let mut sections: Vec<String> = ini.keys().cloned().collect();
    sections.sort();
    sections
}

/// Locate winapp2.ini files in the usual places.
pub fn list_winapp_files() -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut dirs = Vec::new();
    if let Some(config) = crate::platform::config_dir() {
        dirs.push(config.join("cleaners"));
    }
    dirs.extend(crate::platform::system_cleaner_dirs());

    for dir in dirs {
        let candidate = dir.join("winapp2.ini");
        if candidate.is_file() {
            found.push(candidate);
        }
    }
    found
}

/// Load every winapp2.ini in the usual places.
pub fn load_winapp2(trust: Trust) -> Vec<Result<CleanerDef>> {
    let mut out = Vec::new();
    for file in list_winapp_files() {
        match std::fs::read_to_string(&file) {
            Ok(text) => match parse_winapp2(&text, &file.display().to_string(), trust) {
                Ok(cleaners) => {
                    for cleaner in cleaners {
                        out.push(Ok(cleaner));
                    }
                }
                Err(err) => out.push(Err(err)),
            },
            Err(err) => out.push(Err(Error::io(&file, err))),
        }
    }
    out
}

/// Convenience for the CLI: parse a specific file.
pub fn load_winapp2_file(path: &Path, trust: Trust) -> Vec<Result<CleanerDef>> {
    match std::fs::read_to_string(path) {
        Ok(text) => match parse_winapp2(&text, &path.display().to_string(), trust) {
            Ok(cleaners) => cleaners.into_iter().map(Ok).collect(),
            Err(err) => vec![Err(err)],
        },
        Err(err) => vec![Err(Error::io(path, err))],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Vec<CleanerDef> {
        parse_winapp2(text, "winapp2.ini", Trust::Untrusted).unwrap()
    }

    /// A directory that is spelled absolutely on both platforms, so the
    /// importer's behaviour can be asserted everywhere.
    const ROOTED: &str = "C:\\Logs";

    #[test]
    fn ini_keys_are_folded_to_lower_case() {
        let ini = parse_ini("\u{feff}; note\r\n[App]\r\nLangSecRef=3028\r\nDetectFile=%TEMP%\r\n");
        let table = &ini["App"];
        assert_eq!(table["langsecref"], "3028");
        assert_eq!(table["detectfile"], "%TEMP%");
        // Values keep their case.
        assert_eq!(table["detectfile"], "%TEMP%");
        assert!(!table.contains_key("LangSecRef"));
    }

    /// A real winapp2.ini writes `LangSecRef`; with the capitalisation intact
    /// the lookup missed and every section became its own `winapp2_*` cleaner.
    #[test]
    fn known_langsecref_maps_to_a_cleaner_id() {
        let cleaners = parse(&format!(
            "[Firefox Junk]\nLangSecRef=3028\nFileKey1={ROOTED}|*.log\n"
        ));
        assert_eq!(cleaners.len(), 1);
        assert_eq!(cleaners[0].id, "winapp2_mozilla");
    }

    #[test]
    fn relative_filekey_is_dropped() {
        // Resolved against the working directory at run time, so the same
        // section would delete different things from different directories.
        assert!(parse("[Relative]\nLangSecRef=3021\nFileKey1=Temp|*.log\n").is_empty());
        assert!(parse("[Relative]\nLangSecRef=3021\nFileKey1=..\\..\\Temp|*.*\n").is_empty());
    }

    /// Winapp2 is a Windows database: on a machine without its variables a
    /// path must not collapse to a root-relative fragment such as `/Logs`.
    #[test]
    fn undefined_variable_produces_no_actions() {
        let name = "SWEEP_TEST_WINA_UNSET";
        std::env::remove_var(name);
        assert!(parse(&format!(
            "[Windows Only]\nLangSecRef=3021\nFileKey1=%{name}%\\Logs|*.log\n"
        ))
        .is_empty());
    }

    #[test]
    fn absolute_filekey_survives_expansion() {
        let name = "SWEEP_TEST_WINA_SET";
        std::env::set_var(name, if cfg!(windows) { r"C:\Apps" } else { "/apps" });
        let cleaners = parse(&format!(
            "[Absolute]\nLangSecRef=3021\nFileKey1=%{name}%\\Logs|*.log\n"
        ));
        std::env::remove_var(name);

        assert_eq!(cleaners.len(), 1);
        let action = &cleaners[0].options[0].actions[0];
        assert!(
            crate::core::path::looks_absolute(&action.path),
            "'{}' is not rooted",
            action.path
        );
    }

    /// `DetectFile` only started working once keys were folded to lower case:
    /// before that a section for an application that is not installed was
    /// imported anyway.
    #[test]
    fn detect_file_decides_whether_a_section_is_active() {
        assert!(parse(&format!(
            "[Ghost]\nLangSecRef=3021\nDetectFile=C:\\no\\such\\file.exe\nFileKey1={ROOTED}|*.log\n"
        ))
        .is_empty());
    }

    #[test]
    fn slug_builds_lowercase_identifiers() {
        assert_eq!(slug("Adobe Reader 2024!"), "adobe_reader_2024");
        assert_eq!(slug("   "), "");
    }

    #[test]
    fn fnmatch_regex_escapes_regex_metacharacters() {
        assert_eq!(fnmatch_regex("a.b*c?"), "a\\.b.*c.");
        // Registry exclusions are handled separately, not as a path regex.
        assert_eq!(exclude_to_nwholeregex("REG|HKCU\\Foo"), None);
    }
}
