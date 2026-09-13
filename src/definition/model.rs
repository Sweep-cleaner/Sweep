//! The cleaner definition model.
//!
//! This is the in-memory shape every front-end (native TOML, BleachBit's
//! CleanerML XML, winapp2.ini) is normalised into. Keeping one model means the
//! engine, the action providers and the CLI never need to know which format a
//! cleaner came from.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::core::error::{Error, Result};
use crate::platform;

/// Operating-system filter attached to cleaners, options, actions and values.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OsFilter(pub Option<String>);

impl OsFilter {
    /// Matches every platform.
    pub fn any() -> Self {
        Self(None)
    }

    /// Parse the `os="..."` attribute.
    pub fn new(value: &str) -> Self {
        if value.is_empty() {
            Self(None)
        } else {
            Self(Some(value.to_ascii_lowercase()))
        }
    }

    /// Does this definition apply to the running system?
    pub fn matches(&self) -> bool {
        match &self.0 {
            None => true,
            Some(filter) => platform::os_matches(filter),
        }
    }
}

/// How a cleaner decides that the target application is currently running.
#[derive(Debug, Clone)]
pub enum RunningCheck {
    /// A process with this executable name exists.
    Exe {
        /// Executable name, e.g. `firefox.exe`.
        name: String,
        /// Only count processes owned by the current user.
        same_user: bool,
        /// OS filter.
        os: OsFilter,
    },
    /// A glob whose matches indicate a live instance (e.g. a `lock` file).
    Path {
        /// Glob pattern.
        pattern: String,
        /// OS filter.
        os: OsFilter,
    },
}

/// How a `<var>`/[[var]] value is resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VarSearch {
    /// Use the text as-is.
    #[default]
    Literal,
    /// Expand as a glob and keep every match.
    Glob,
    /// Read the value from the Windows registry.
    Registry,
}

/// One candidate value of a multi-value variable.
#[derive(Debug, Clone)]
pub struct VarValue {
    /// Raw text (or glob pattern, or registry path depending on `search`).
    pub raw: String,
    /// How to resolve `raw`.
    pub search: VarSearch,
    /// OS filter.
    pub os: OsFilter,
    /// Registry value name when `search == Registry`.
    pub reg_name: Option<String>,
}

/// A named, multi-value variable such as `$$profile$$`.
#[derive(Debug, Clone)]
pub struct VarDef {
    /// Variable name as written between `$$...$$`.
    pub name: String,
    /// Candidate values, in declaration order.
    pub values: Vec<VarValue>,
}

/// Resolved variables, ready for `$$name$$` expansion.
#[derive(Debug, Default, Clone)]
pub struct VarSet {
    map: BTreeMap<String, Vec<String>>,
}

impl VarSet {
    /// Empty variable set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert (or prepend to) a variable's value list.
    ///
    /// Later declarations win, matching BleachBit's `handle_cleaner_var()`:
    /// the most specific `<value os="...">` is usually written last.
    pub fn insert(&mut self, name: impl Into<String>, values: Vec<String>) {
        let name = name.into();
        if values.is_empty() {
            return;
        }
        match self.map.get_mut(&name) {
            Some(existing) => {
                let mut merged = values;
                merged.append(existing);
                *existing = merged;
            }
            None => {
                self.map.insert(name, values);
            }
        }
    }

    /// Look up a variable.
    pub fn get(&self, name: &str) -> Option<&[String]> {
        self.map.get(name).map(|v| v.as_slice())
    }

    /// Is the set empty?
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Expand every `$$name$$` placeholder in `input`.
    ///
    /// Returns one string per combination of the variables actually referenced
    /// — BleachBit's `expand_multi_var()` semantics, including the rule that a
    /// string with no placeholder (or no *known* placeholder) is returned
    /// unchanged exactly once.
    pub fn expand(&self, input: &str) -> Vec<String> {
        if self.is_empty() || !input.contains("$$") {
            return vec![input.to_string()];
        }

        let mut used: Vec<&str> = Vec::new();
        for name in self.map.keys() {
            if input.contains(&format!("$${name}$$")) {
                used.push(name.as_str());
            }
        }

        if used.is_empty() {
            return vec![input.to_string()];
        }

        // Stable, insertion-ordered cartesian expansion.
        let mut results: Vec<String> = vec![input.to_string()];
        for name in used {
            let Some(values) = self.map.get(name) else {
                continue;
            };
            let placeholder = format!("$${name}$$");
            let mut next = Vec::with_capacity(results.len() * values.len());
            for candidate in &results {
                for value in values {
                    next.push(candidate.replace(&placeholder, value));
                }
            }
            results = next;
        }

        results
    }
}

/// How `<action search="...">` should interpret `path`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchKind {
    /// The path is a literal file (or a glob-free path); check existence.
    File,
    /// Expand as a glob, then walk the matches.
    #[default]
    Glob,
    /// Everything inside the directory, including subdirectories, but not the
    /// directory itself.
    WalkAll,
    /// Only files inside the directory (recursively), never directories.
    WalkFiles,
    /// Everything inside the directory *and* the directory itself.
    WalkTop,
    /// Deferred: the path is a root for the deep-scan phase.
    Deep,
}

impl SearchKind {
    /// Parse a `search=` attribute value.
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "" | "file" => SearchKind::File,
            "glob" => SearchKind::Glob,
            "walk.all" => SearchKind::WalkAll,
            "walk.files" => SearchKind::WalkFiles,
            "walk.top" => SearchKind::WalkTop,
            "deep" => SearchKind::Deep,
            _ => return None,
        })
    }

    /// Stable string form.
    pub fn as_str(self) -> &'static str {
        match self {
            SearchKind::File => "file",
            SearchKind::Glob => "glob",
            SearchKind::WalkAll => "walk.all",
            SearchKind::WalkFiles => "walk.files",
            SearchKind::WalkTop => "walk.top",
            SearchKind::Deep => "deep",
        }
    }
}

/// `type="f"` / `type="d"` restriction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObjectType {
    /// Regular file only.
    File,
    /// Directory only.
    Dir,
}

/// One cleaning action.
#[derive(Debug, Clone)]
pub struct ActionDef {
    /// Action command, e.g. `delete`, `sqlite.vacuum`, `apt.clean`.
    pub command: String,
    /// How to interpret `path`.
    pub search: SearchKind,
    /// The path (possibly containing `$$vars$$`, `~` and environment
    /// variables).
    pub path: String,
    /// OS filter.
    pub os: OsFilter,
    /// Regex matched against the file name.
    pub regex: Option<String>,
    /// Exclude when this regex matches the file name.
    pub nregex: Option<String>,
    /// Regex matched against the whole path.
    pub wholeregex: Option<String>,
    /// Exclude when this regex matches the whole path.
    pub nwholeregex: Option<String>,
    /// Restrict to files or directories.
    pub object_type: Option<ObjectType>,

    // ---- extensions beyond BleachBit --------------------------------------
    /// Only consider files whose mtime is older than this many days.
    pub max_age_days: Option<u32>,
    /// Only consider files newer than this many days.
    pub min_age_days: Option<u32>,
    /// Only consider files at least this big.
    pub min_size: Option<u64>,
    /// Only consider files at most this big.
    pub max_size: Option<u64>,
    /// Cap recursion depth for `walk.*` searches.
    pub max_depth: Option<usize>,
    /// Include directories in the result set even for `walk.files`.
    pub include_dirs: Option<bool>,

    // ---- command-specific parameters ---------------------------------------
    /// `ini` action: section to remove.
    pub section: Option<String>,
    /// `ini` action: parameter to remove (whole section when absent).
    pub parameter: Option<String>,
    /// `json` action: slash-separated path to the key to remove.
    pub address: Option<String>,
    /// `process` action: command line to run.
    pub cmd: Option<String>,
    /// `process` action: wait for completion.
    pub wait: bool,
    /// `winreg` action: registry value name.
    pub reg_name: Option<String>,
    /// `winreg` action: subkeys to preserve.
    pub exclude_keys: Vec<String>,
}

impl Default for ActionDef {
    fn default() -> Self {
        Self {
            command: "delete".into(),
            search: SearchKind::default(),
            path: String::new(),
            os: OsFilter::any(),
            regex: None,
            nregex: None,
            wholeregex: None,
            nwholeregex: None,
            object_type: None,
            max_age_days: None,
            min_age_days: None,
            min_size: None,
            max_size: None,
            max_depth: None,
            include_dirs: None,
            section: None,
            parameter: None,
            address: None,
            cmd: None,
            wait: true,
            reg_name: None,
            exclude_keys: Vec::new(),
        }
    }
}

/// One user-selectable cleaning option.
#[derive(Debug, Clone)]
pub struct OptionDef {
    /// Machine identifier, e.g. `cache`.
    pub id: String,
    /// Human label.
    pub label: String,
    /// Sentences shown in the UI / `--list`.
    pub description: String,
    /// Shown before the option is enabled.
    pub warning: Option<String>,
    /// Actions performed when the option runs.
    pub actions: Vec<ActionDef>,
    /// OS filter.
    pub os: OsFilter,
}

/// A complete cleaner.
#[derive(Debug, Clone)]
pub struct CleanerDef {
    /// Machine identifier, e.g. `firefox`.
    pub id: String,
    /// Human name.
    pub name: String,
    /// One-line description.
    pub description: String,
    /// OS filter for the whole cleaner.
    pub os: OsFilter,
    /// How to detect that the target is running.
    pub running: Vec<RunningCheck>,
    /// Multi-value variables.
    pub vars: Vec<VarDef>,
    /// Cleaning options.
    pub options: Vec<OptionDef>,
}

impl CleanerDef {
    /// Minimal cleaner with no options.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: String::new(),
            os: OsFilter::any(),
            running: Vec::new(),
            vars: Vec::new(),
            options: Vec::new(),
        }
    }

    /// Resolve the multi-value variables for the current platform.
    ///
    /// Performs glob expansion and registry lookups, so it must be called
    /// lazily — not at parse time — to avoid touching the filesystem for
    /// cleaners that will never run.
    pub fn resolve_vars(&self) -> VarSet {
        let mut set = VarSet::new();

        for var in &self.vars {
            let mut values: Vec<String> = Vec::new();
            for value in &var.values {
                if !value.os.matches() {
                    continue;
                }
                match value.search {
                    VarSearch::Literal => values.push(value.raw.clone()),
                    VarSearch::Glob => {
                        let expanded = crate::core::path::expand(&value.raw);
                        match crate::fsutil::walk::glob_dirs(&expanded.to_string_lossy()) {
                            Ok(matches) => {
                                for m in matches {
                                    values.push(m.to_string_lossy().into_owned());
                                }
                            }
                            Err(err) => log::debug!(
                                "glob var '{}' in cleaner '{}' failed: {}",
                                var.name,
                                self.id,
                                err
                            ),
                        }
                    }
                    VarSearch::Registry => {
                        #[cfg(windows)]
                        {
                            if let Some(found) = crate::platform::windows::reg_read_string(
                                &value.raw,
                                value.reg_name.as_deref(),
                            ) {
                                values.push(found);
                            }
                        }
                    }
                }
            }
            set.insert(var.name.clone(), values);
        }

        set
    }

    /// All options that apply to this platform.
    pub fn active_options(&self) -> impl Iterator<Item = &OptionDef> {
        self.options.iter().filter(|o| o.os.matches())
    }

    /// Look up an option by id.
    pub fn option(&self, id: &str) -> Option<&OptionDef> {
        self.options.iter().find(|o| o.id == id)
    }

    /// Is the application this cleaner targets currently running?
    ///
    /// Cleaning a live browser or mail client is the most common cause of
    /// "my history came back": the application flushes its own state on exit
    /// and undoes the cleaning. The worker refuses to run in that situation
    /// unless the user forces it.
    pub fn is_running(&self) -> bool {
        self.running.iter().any(|check| match check {
            RunningCheck::Exe {
                name,
                same_user,
                os,
            } => {
                if !os.matches() {
                    return false;
                }
                let _ = same_user; // sysinfo only reports our own processes on
                                   // Windows; on Unix we cannot filter cheaply,
                                   // so a hit is a hit.
                platform::is_process_running(name)
            }
            RunningCheck::Path { pattern, os } => {
                if !os.matches() {
                    return false;
                }
                let expanded = crate::core::path::expand(pattern);
                crate::fsutil::walk::glob_paths(&expanded.to_string_lossy())
                    .map(|matches| !matches.is_empty())
                    .unwrap_or(false)
            }
        })
    }

    /// Human-readable reason why the cleaner is considered running, if any.
    pub fn running_reason(&self) -> Option<String> {
        for check in &self.running {
            let hit = match check {
                RunningCheck::Exe { name, os, .. } => {
                    os.matches() && platform::is_process_running(name)
                }
                RunningCheck::Path { pattern, os } => {
                    os.matches()
                        && crate::fsutil::walk::glob_paths(
                            &crate::core::path::expand(pattern).to_string_lossy(),
                        )
                        .map(|m| !m.is_empty())
                        .unwrap_or(false)
                }
            };
            if hit {
                return Some(match check {
                    RunningCheck::Exe { name, .. } => format!("process '{name}' is running"),
                    RunningCheck::Path { pattern, .. } => format!("lock file '{pattern}' exists"),
                });
            }
        }
        None
    }

    /// Validate the definition and return a list of human-readable problems.
    pub fn validate(&self) -> Vec<String> {
        let mut problems = Vec::new();

        if self.id.is_empty() {
            problems.push("missing cleaner id".into());
        }
        if self.name.is_empty() {
            problems.push(format!("cleaner '{}' has no label", self.id));
        }
        let mut seen_options = std::collections::HashSet::new();
        for option in &self.options {
            if option.id.is_empty() {
                problems.push(format!("cleaner '{}' has an option without id", self.id));
            } else if !seen_options.insert(option.id.clone()) {
                problems.push(format!(
                    "cleaner '{}' has a duplicate option id '{}'",
                    self.id, option.id
                ));
            }
            if option.label.is_empty() {
                problems.push(format!("option '{}.{}' has no label", self.id, option.id));
            }
            for action in &option.actions {
                if !crate::action::provider::exists(&action.command) {
                    problems.push(format!(
                        "cleaner '{}' uses unknown action command '{}'",
                        self.id, action.command
                    ));
                }
            }
        }
        problems
    }
}

/// Where a definition came from, which decides how much we trust it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trust {
    /// Shipped with Sweep or installed into a system directory.
    Trusted,
    /// Loaded from a user-writable directory: `process` and `winreg`
    /// actions are dropped, exactly like BleachBit.
    Untrusted,
}

impl Trust {
    /// May this cleaner run an arbitrary external command?
    pub fn allows_process(self) -> bool {
        self == Trust::Trusted
    }

    /// May this cleaner edit the Windows registry?
    pub fn allows_registry(self) -> bool {
        self == Trust::Trusted
    }
}

/// A cleaner definition plus its provenance.
#[derive(Debug, Clone)]
pub struct LoadedCleaner {
    /// The definition.
    pub def: CleanerDef,
    /// Trust level.
    pub trust: Trust,
    /// Source file (for diagnostics).
    pub source: PathBuf,
}

impl LoadedCleaner {
    /// Wrap a definition with provenance.
    pub fn new(def: CleanerDef, trust: Trust, source: PathBuf) -> Self {
        Self { def, trust, source }
    }
}

/// Drop actions that an untrusted cleaner is not allowed to run.
pub fn apply_trust(mut cleaner: CleanerDef, trust: Trust) -> (CleanerDef, Vec<String>) {
    let mut ignored = Vec::new();
    if trust == Trust::Trusted {
        return (cleaner, ignored);
    }

    for option in cleaner.options.iter_mut() {
        option.actions.retain(|action| {
            // Privileged providers (process, winreg, apt.autoremove, …) are
            // only permitted for trusted sources. The decision is driven by the
            // provider registry so the blocklist can never drift out of sync.
            let allowed = if crate::action::provider::is_privileged(&action.command) {
                trust.allows_process()
            } else {
                true
            };
            if !allowed {
                ignored.push(action.command.clone());
            }
            allowed
        });
    }

    (cleaner, ignored)
}

/// Turn an action's raw path string into candidate [`PathBuf`]s.
///
/// Variable placeholders and environment variables are both expanded here, so
/// this is the one place where a declared path becomes a real location — which
/// makes it the right place to enforce that the location is **absolute**.
///
/// A relative path would be resolved against the process working directory, so
/// the very same cleaner would delete different things depending on where
/// `sweep` happened to be started, and an imported `winapp2.ini` could smuggle
/// one in deliberately. Such paths are dropped instead of being followed.
///
/// A path that still contains an unresolved `$$name$$` placeholder is dropped
/// quietly: it only means the variable matched nothing on this machine, so the
/// cleaner simply does not apply here.
pub fn expand_action_paths(action: &ActionDef, vars: &VarSet) -> Vec<PathBuf> {
    let strict = crate::action::provider::filesystem_path(&action.command);
    let mut out = Vec::new();

    for raw in vars.expand(&action.path) {
        let expanded = crate::core::path::expand(&raw);
        // A path is "rooted" (safe to act on) when it cannot be mistaken for
        // something relative to the process working directory. `is_absolute()`
        // is platform-specific — `/etc/x` is absolute on Linux but not on
        // Windows — so we also accept the other platform's root syntax via
        // `looks_absolute`, which understands drive letters and leading slashes
        // everywhere. A genuinely relative path fails both and is dropped.
        let rooted = expanded.as_os_str().is_empty()
            || expanded.is_absolute()
            || crate::core::path::looks_absolute(&expanded.to_string_lossy());
        if !strict || rooted {
            out.push(expanded);
            continue;
        }
        if raw.contains("$$") {
            log::debug!(
                "skipping '{}': variable in '{raw}' matched nothing on this system",
                expanded.display()
            );
        } else {
            log::warn!(
                "ignoring relative path '{raw}' (resolved to '{}'): \
                 cleaner paths must be absolute",
                expanded.display()
            );
        }
    }

    out
}

/// Read a definition, dispatching on the file extension.
pub fn parse_file(path: &std::path::Path, trust: Trust) -> Result<CleanerDef> {
    let text = std::fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());

    match path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
        .as_str()
    {
        "toml" => crate::definition::native::parse_native(&text, &name),
        "xml" => crate::definition::cleanerml::parse_cleanerml(&text, &name, trust),
        "ini" => Err(Error::definition(
            name,
            "winapp2.ini files must be loaded with load_winapp2()",
        )),
        other => Err(Error::definition(
            name,
            format!("unsupported definition format '.{other}'"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varset_cartesian() {
        let mut set = VarSet::new();
        set.insert("profile", vec!["a".into(), "b".into()]);
        set.insert("kind", vec!["cache".into(), "log".into()]);

        let out = set.expand("$$profile$$/$$kind$$");
        assert_eq!(out.len(), 4);
        assert!(out.contains(&"a/cache".to_string()));
        assert!(out.contains(&"b/log".to_string()));

        // No placeholder -> returned once, unchanged.
        assert_eq!(set.expand("/tmp/foo"), vec!["/tmp/foo".to_string()]);
    }

    /// Build an action with a single filesystem command/path.
    fn action(command: &str, path: &str) -> ActionDef {
        ActionDef {
            command: command.into(),
            path: path.into(),
            ..ActionDef::default()
        }
    }

    // An absolute path is always accepted, on every platform.
    #[test]
    fn expand_keeps_absolute_filesystem_paths() {
        let vars = VarSet::new();
        let out = expand_action_paths(&action("delete", "/etc/x"), &vars);
        assert_eq!(out, vec![PathBuf::from("/etc/x")]);
    }

    // A relative path would be resolved against the process CWD, so it must be
    // dropped — this is the guard that keeps a cleaner from deleting different
    // files depending on where `sweep` was started.
    #[test]
    fn expand_drops_relative_filesystem_paths() {
        let vars = VarSet::new();
        assert!(expand_action_paths(&action("delete", "tmp/x"), &vars).is_empty());
        assert!(expand_action_paths(&action("shred", "../x"), &vars).is_empty());
    }

    // `winreg` interprets its path as a registry key (`HKCU\Software\…`), not as
    // a location, so the absolute-only rule must not apply to it.
    #[test]
    fn expand_keeps_registry_keys() {
        let vars = VarSet::new();
        let out = expand_action_paths(&action("winreg", "HKCU\\Software\\Sweep"), &vars);
        assert_eq!(out, vec![PathBuf::from("HKCU\\Software\\Sweep")]);
    }

    // An unresolved `$$var$$` means the variable matched nothing on this machine;
    // the cleaner simply does not apply, so the fragment is dropped quietly.
    #[test]
    fn expand_drops_unresolved_placeholders() {
        let vars = VarSet::new();
        assert!(expand_action_paths(&action("delete", "$$profile$$/cache"), &vars).is_empty());
    }

    #[test]
    fn searchkind_round_trips() {
        for kind in [
            SearchKind::File,
            SearchKind::Glob,
            SearchKind::WalkAll,
            SearchKind::WalkFiles,
            SearchKind::WalkTop,
            SearchKind::Deep,
        ] {
            assert_eq!(SearchKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(SearchKind::parse(""), Some(SearchKind::File));
        assert_eq!(SearchKind::parse("nonsense"), None);
    }

    #[test]
    fn osfilter_matches_platform() {
        assert!(OsFilter::any().matches());
        // `OsFilter::new` lower-cases so lookups are case-insensitive.
        let filtered = OsFilter::new("LINUX");
        assert_eq!(filtered.matches(), cfg!(target_os = "linux"));
    }
}
