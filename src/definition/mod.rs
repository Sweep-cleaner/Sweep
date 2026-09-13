//! Cleaner definition loading, parsing and lookup.
//!
//! Three front-ends feed one model:
//!
//! | Format | Origin | Trust |
//! |---|---|---|
//! | `*.toml` | Sweep native, built-in or user-written | trusted / untrusted |
//! | `*.xml` | BleachBit CleanerML | untrusted in user dirs |
//! | `*.ini` | winapp2.ini | untrusted |
//!
//! Everything is normalised into [`CleanerDef`] and stored in a
//! [`CleanerRegistry`] keyed by cleaner id.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::core::error::Result;
use crate::definition::model::{CleanerDef, LoadedCleaner, Trust};

pub mod builtin;
pub mod cleanerml;
pub mod model;
pub mod native;
pub mod winapp2;

pub use model::{
    apply_trust, expand_action_paths, ActionDef, ObjectType, OptionDef, OsFilter, RunningCheck,
    SearchKind, Trust as TrustLevel, VarSet,
};

/// What to load from where. Everything is off except the built-ins.
#[derive(Debug, Clone)]
pub struct LoadOptions {
    /// Load the definitions compiled into the binary.
    pub builtin: bool,
    /// Load BleachBit's CleanerML cleaners from its standard directories.
    pub bleachbit: bool,
    /// Load and translate `winapp2.ini`.
    pub winapp2: bool,
    /// Extra directories to scan (user cleaners, drop-ins...).
    pub extra_dirs: Vec<PathBuf>,
    /// Trust level assigned to cleaners found outside the binary.
    pub external_trust: Trust,
}

impl Default for LoadOptions {
    fn default() -> Self {
        Self {
            builtin: true,
            bleachbit: false,
            winapp2: false,
            extra_dirs: Vec::new(),
            // A cleaner that lives in a user-writable directory can be
            // swapped by anything running as that user, so it must not be
            // allowed to spawn processes or edit the registry.
            external_trust: Trust::Untrusted,
        }
    }
}

impl LoadOptions {
    /// Built-ins only (the default).
    pub fn builtin_only() -> Self {
        Self::default()
    }

    /// Built-ins plus everything we can find on the system.
    pub fn everything() -> Self {
        Self {
            bleachbit: true,
            winapp2: true,
            ..Self::default()
        }
    }

    /// Add a directory to scan.
    pub fn with_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.extra_dirs.push(dir.into());
        self
    }

    /// Treat externally loaded cleaners as trusted (expert mode only).
    pub fn trusting_external(mut self) -> Self {
        self.external_trust = Trust::Trusted;
        self
    }
}

/// All known cleaners, keyed by id.
#[derive(Debug, Default)]
pub struct CleanerRegistry {
    cleaners: BTreeMap<String, LoadedCleaner>,
    /// Diagnostics collected while loading (bad definitions, conflicts...).
    warnings: Vec<String>,
}

impl CleanerRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load cleaners according to `options`, returning the registry.
    pub fn load(options: LoadOptions) -> Self {
        let mut registry = Self::new();
        registry.load_into(options);
        registry
    }

    /// Load cleaners into an existing registry.
    pub fn load_into(&mut self, options: LoadOptions) {
        if options.builtin {
            self.load_builtin();
        }
        if options.bleachbit {
            self.load_bleachbit(options.external_trust);
        }
        if options.winapp2 {
            self.load_winapp2(options.external_trust);
        }
        for dir in &options.extra_dirs {
            self.load_dir(dir, options.external_trust);
        }
    }

    /// Parse and register the definitions compiled into the binary.
    fn load_builtin(&mut self) {
        for (file, text) in builtin::BUILTIN_CLEANERS {
            match native::parse_native(text, file) {
                Ok(def) => {
                    let id = def.id.clone();
                    self.register(def, Trust::Trusted, PathBuf::from("<builtin>").join(file));
                    if !self.warnings.iter().any(|w| w.contains(&id)) {
                        // nothing to do; kept for clarity
                    }
                }
                Err(err) => self
                    .warnings
                    .push(format!("built-in cleaner '{file}' failed to parse: {err}")),
            }
        }
    }

    /// Load BleachBit's CleanerML cleaners from all of its standard locations.
    fn load_bleachbit(&mut self, trust: Trust) {
        for dir in crate::platform::bleachbit_cleaner_dirs() {
            if dir.is_dir() {
                self.load_dir(&dir, trust);
            }
        }
    }

    /// Load and translate winapp2.ini (and its `*.ini` siblings).
    fn load_winapp2(&mut self, trust: Trust) {
        for result in winapp2::load_winapp2(trust) {
            match result {
                Ok(def) => {
                    let source = PathBuf::from("winapp2.ini");
                    self.register(def, trust, source);
                }
                Err(err) => self.warnings.push(err.to_string()),
            }
        }
    }

    /// Load every `*.toml`, `*.xml` and `*.ini` cleaner in `dir`.
    pub fn load_dir(&mut self, dir: &Path, trust: Trust) {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(err) => {
                log::debug!("cannot read cleaner dir {}: {}", dir.display(), err);
                return;
            }
        };

        let mut files: Vec<PathBuf> = entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        files.sort();

        for file in files {
            let ext = file
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();

            let results: Vec<Result<CleanerDef>> = match ext.as_str() {
                "toml" => match std::fs::read_to_string(&file) {
                    Ok(text) => {
                        let name = file.display().to_string();
                        vec![native::parse_native(&text, &name)]
                    }
                    Err(err) => {
                        self.warnings
                            .push(format!("cannot read {}: {}", file.display(), err));
                        continue;
                    }
                },
                "xml" => cleanerml::load_cleanerml_dir_entry(&file, trust),
                "ini" => winapp2::load_winapp2_file(&file, trust),
                _ => continue,
            };

            for result in results {
                match result {
                    Ok(def) => {
                        let source = file.clone();
                        self.register(def, trust, source);
                    }
                    Err(err) => self.warnings.push(err.to_string()),
                }
            }
        }
    }

    /// Register a definition, stripping actions its trust level forbids.
    pub fn register(&mut self, def: CleanerDef, trust: Trust, source: PathBuf) {
        if !def.os.matches() {
            log::debug!(
                "skipping cleaner '{}' ({}): does not apply to this platform",
                def.id,
                source.display()
            );
            return;
        }

        let (def, ignored) = apply_trust(def, trust);
        let id = def.id.clone();

        for command in ignored {
            self.warnings.push(format!(
                "cleaner '{}' from {} may not use the '{command}' action (untrusted source)",
                id,
                source.display()
            ));
        }

        for problem in def.validate() {
            self.warnings.push(format!(
                "cleaner '{}' from {}: {problem}",
                id,
                source.display()
            ));
        }

        // A trusted definition always replaces an untrusted one; otherwise the
        // first writer wins so built-ins are never shadowed by system files.
        if let Some(existing) = self.cleaners.get(&id) {
            if existing.trust == Trust::Trusted && trust != Trust::Trusted {
                log::debug!(
                    "keeping trusted cleaner '{id}' instead of {}",
                    source.display()
                );
                return;
            }
        }

        self.cleaners
            .insert(id, LoadedCleaner::new(def, trust, source));
    }

    /// Look up a cleaner by id.
    pub fn get(&self, id: &str) -> Option<&LoadedCleaner> {
        self.cleaners.get(id)
    }

    /// Look up a cleaner definition by id.
    pub fn def(&self, id: &str) -> Option<&CleanerDef> {
        self.cleaners.get(id).map(|c| &c.def)
    }

    /// Every registered cleaner, ordered by id.
    pub fn all(&self) -> impl Iterator<Item = &LoadedCleaner> {
        self.cleaners.values()
    }

    /// Every cleaner id.
    pub fn ids(&self) -> Vec<String> {
        self.cleaners.keys().cloned().collect()
    }

    /// Number of registered cleaners.
    pub fn len(&self) -> usize {
        self.cleaners.len()
    }

    /// Is the registry empty?
    pub fn is_empty(&self) -> bool {
        self.cleaners.is_empty()
    }

    /// Remove a cleaner (used by `--exclude` and by the tests).
    pub fn remove(&mut self, id: &str) -> Option<LoadedCleaner> {
        self.cleaners.remove(id)
    }

    /// Resolve a user-supplied selector into concrete cleaner ids.
    ///
    /// A selector is either an exact id or a glob (`fire*`, `*fox`). Globs are
    /// matched case-insensitively so `FireFox` works on every platform.
    pub fn matching(&self, selector: &str) -> Vec<String> {
        if let Some(cleaner) = self.cleaners.get(selector) {
            return vec![cleaner.def.id.clone()];
        }

        if !selector.contains(['*', '?', '[', ']']) {
            return Vec::new();
        }

        let pattern = match globset::GlobBuilder::new(&selector.to_lowercase())
            .case_insensitive(true)
            .literal_separator(true)
            .build()
        {
            Ok(pattern) => pattern.compile_matcher(),
            Err(_) => return Vec::new(),
        };

        self.cleaners
            .keys()
            .filter(|id| pattern.is_match(id.to_lowercase()))
            .cloned()
            .collect()
    }

    /// Diagnostics collected during loading.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Add a warning by hand (used by the CLI when a selector matches nothing).
    pub fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    /// Is the application this cleaner targets currently running?
    ///
    /// Cleaning a live browser is the number one cause of "my history came
    /// back" reports, so the worker refuses to do it unless forced.
    pub fn is_running(&self, id: &str) -> bool {
        let Some(cleaner) = self.cleaners.get(id) else {
            return false;
        };
        cleaner.def.is_running()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_parse() {
        // Every shipped definition must be valid TOML with a known command.
        let registry = CleanerRegistry::load(LoadOptions::builtin_only());
        assert!(
            registry.warnings().is_empty(),
            "built-in cleaner warnings: {:?}",
            registry.warnings()
        );
        assert!(
            registry.len() >= 10,
            "only {} cleaners loaded",
            registry.len()
        );
        assert!(registry.get("system").is_some());
    }

    #[test]
    fn glob_selection() {
        let registry = CleanerRegistry::load(LoadOptions::builtin_only());
        let hits = registry.matching("fire*");
        assert!(hits.contains(&"firefox".to_string()));
        assert!(registry.matching("nonexistent").is_empty());
    }
}
