//! User configuration (persisted as TOML in the config directory).
//!
//! Sweep keeps its own state — BleachBit's `~/.config/bleachbit/bleachbit.ini`
//! is read for familiarity but never required. The config controls the
//! *cross-cleaner* behaviour: secure deletion, which localisations to keep,
//! user keep-list, custom paths and deep-scan roots.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::core::error::{Error, Result};

/// Taşınabilir kök: `--portable DIR` verildiyse tüm durum bu dizindedir.
pub fn portable_base() -> Option<PathBuf> {
    std::env::var_os("SWEEP_PORTABLE").map(PathBuf::from)
}

/// Default location: `$XDG_CONFIG_HOME/sweep/config.toml` (or the
/// platform equivalent under the config dir). Portable modda `<base>/config.toml`.
pub fn default_path() -> PathBuf {
    if let Some(base) = portable_base() {
        return base.join("config.toml");
    }
    if let Some(dir) = crate::platform::config_dir() {
        // `config_dir()` zaten `.../sweep` ile biter.
        dir.join("config.toml")
    } else {
        PathBuf::from("sweep.toml")
    }
}

/// Önceki sürümün çift `sweep/sweep` yolu (taşıma uyumluluğu için okunur).
fn legacy_path(path: &std::path::Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    if parent.file_name().map(|n| n == "sweep").unwrap_or(false) {
        Some(parent.join("sweep").join(path.file_name()?))
    } else {
        None
    }
}

/// The persistent configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Overwrite files before deleting them.
    pub shred: bool,
    /// Locale codes (e.g. `en`, `de`) to keep when `system.localizations` runs.
    pub languages: Vec<String>,
    /// Files the user never wants deleted (keep list).
    pub whitelist_files: Vec<String>,
    /// Folders the user never wants deleted (keep list).
    pub whitelist_folders: Vec<String>,
    /// Extra paths cleaned by the `system.custom` option.
    pub custom_paths: Vec<String>,
    /// Roots scanned by `sweep deepscan` when none are given on the CLI.
    pub deepscan_roots: Vec<String>,
    /// Glob patterns used by the deep scan.
    pub deepscan_patterns: Vec<String>,
    /// Remember the last selection between runs.
    pub remember_last: bool,
    /// Last selection (only meaningful when `remember_last` is set).
    pub last_selection: Vec<String>,
    /// `system.journal_vacuum` boyutu (örn. "500M").
    pub journal_size: String,
    /// `system.journal_vacuum` günü.
    pub journal_days: u32,
    /// `system.pacman_cache` tutacağı sürüm sayısı.
    pub pacman_keep: usize,
    /// `system.temp_deep` yaş eşiği (gün).
    pub temp_max_age_days: u32,
    /// `system.docker` build cache'i de süpürsün mü?
    pub docker_build_cache: bool,
    /// Topluluk kural dizini (index.toml adresi).
    pub hub_url: String,
    /// Silmeden önce yedekleme dizini (boş = yedek yok).
    pub backup_dir: String,
    /// İşlem logu dosyası (boş = varsayılan konum).
    pub log_file: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            shred: false,
            languages: default_languages(),
            whitelist_files: Vec::new(),
            whitelist_folders: Vec::new(),
            custom_paths: Vec::new(),
            deepscan_roots: Vec::new(),
            deepscan_patterns: crate::engine::deepscan::default_patterns(),
            remember_last: false,
            last_selection: Vec::new(),
            journal_size: "500M".into(),
            journal_days: 7,
            pacman_keep: 2,
            temp_max_age_days: 1,
            docker_build_cache: false,
            hub_url: "https://raw.githubusercontent.com/sweep-cleaner/hub/main/index.toml".into(),
            backup_dir: String::new(),
            log_file: String::new(),
        }
    }
}

/// Best-effort guess at the user's current locale.
fn default_languages() -> Vec<String> {
    let mut langs = Vec::new();
    for var in ["LANG", "LANGUAGE", "LC_ALL"] {
        if let Ok(value) = std::env::var(var) {
            if let Some(code) = value.split(['.', '@', '_']).next() {
                let code = code.trim().to_lowercase();
                if !code.is_empty() && !langs.contains(&code) {
                    langs.push(code);
                }
            }
        }
    }
    if langs.is_empty() {
        langs.push("en".to_string());
    }
    langs
}

impl Config {
    /// Default location (associated form of the [`default_path()`] free function).
    pub fn default_path() -> PathBuf {
        default_path()
    }

    /// Load from `path`, falling back to [`Config::default`] if absent.
    pub fn load(path: &std::path::Path) -> Result<Self> {
        if !path.exists() {
            if let Some(legacy) = legacy_path(path) {
                if legacy.exists() {
                    return Self::load(&legacy);
                }
            }
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(path).map_err(|source| Error::io(path, source))?;
        let config: Config = toml::from_str(&text).map_err(|err| Error::Config(err.to_string()))?;
        Ok(config)
    }

    /// Save to `path`, creating the parent directory if needed.
    pub fn save(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|source| Error::io(parent, source))?;
        }
        let text = toml::to_string_pretty(self).map_err(|err| Error::Config(err.to_string()))?;
        // Önce temp dosyaya yazıp taşı: yarıda kesilen yazım config'i bozmasın.
        let temp = path.with_extension("toml.tmp");
        std::fs::write(&temp, text).map_err(|source| Error::io(&temp, source))?;
        std::fs::rename(&temp, path).map_err(|source| Error::io(path, source))
    }

    /// Build the keep list decorated with the built-in conventions.
    pub fn keep_list(&self) -> crate::core::keep::KeepList {
        let mut list = crate::core::keep::KeepList::new();
        for file in &self.whitelist_files {
            list.add_file(crate::core::path::expand(file));
        }
        for folder in &self.whitelist_folders {
            list.add_folder(crate::core::path::expand(folder));
        }
        list
    }

    /// Custom paths, expanded.
    pub fn custom_paths(&self) -> Vec<PathBuf> {
        self.custom_paths
            .iter()
            .map(|p| crate::core::path::expand(p))
            .collect()
    }

    /// Deep-scan roots, expanded.
    pub fn deepscan_roots(&self) -> Vec<PathBuf> {
        self.deepscan_roots
            .iter()
            .map(|p| crate::core::path::expand(p))
            .collect()
    }

    // --- name-keyed access (powers `sweep config get/set/show`) -------------

    /// Every setting as a `(name, value)` pair, in declaration order.
    ///
    /// Lists are rendered comma-separated; a value that itself contains a
    /// comma falls back to a JSON array so the output stays round-trippable.
    pub fn fields(&self) -> Vec<(&'static str, String)> {
        vec![
            ("shred", self.shred.to_string()),
            ("languages", format_list(&self.languages)),
            ("whitelist_files", format_list(&self.whitelist_files)),
            ("whitelist_folders", format_list(&self.whitelist_folders)),
            ("custom_paths", format_list(&self.custom_paths)),
            ("deepscan_roots", format_list(&self.deepscan_roots)),
            ("deepscan_patterns", format_list(&self.deepscan_patterns)),
            ("remember_last", self.remember_last.to_string()),
            ("last_selection", format_list(&self.last_selection)),
            ("journal_size", self.journal_size.clone()),
            ("journal_days", self.journal_days.to_string()),
            ("pacman_keep", self.pacman_keep.to_string()),
            ("temp_max_age_days", self.temp_max_age_days.to_string()),
            ("docker_build_cache", self.docker_build_cache.to_string()),
            ("hub_url", self.hub_url.clone()),
            ("backup_dir", self.backup_dir.clone()),
            ("log_file", self.log_file.clone()),
        ]
    }

    /// Read one setting by name. `None` when the name is unknown.
    pub fn get_field(&self, name: &str) -> Option<String> {
        self.fields()
            .into_iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
    }

    /// Write one setting by name.
    ///
    /// Lists accept either `a,b,c` or a JSON array (`["a", "b"]`). Numbers are
    /// range-checked so a typo cannot silently produce a nonsense budget.
    pub fn set_field(&mut self, name: &str, value: &str) -> Result<()> {
        macro_rules! assign {
            ($slot:expr, $parse:ty) => {{
                let parsed: $parse = value.trim().parse().map_err(|_| {
                    Error::Config(format!(
                        "{}: expected {}, got '{}'",
                        name,
                        stringify!($parse),
                        value
                    ))
                })?;
                $slot = parsed;
            }};
        }

        match name {
            "shred" => assign!(self.shred, bool),
            "languages" => self.languages = parse_list(value)?,
            "whitelist_files" => self.whitelist_files = parse_list(value)?,
            "whitelist_folders" => self.whitelist_folders = parse_list(value)?,
            "custom_paths" => self.custom_paths = parse_list(value)?,
            "deepscan_roots" => self.deepscan_roots = parse_list(value)?,
            "deepscan_patterns" => self.deepscan_patterns = parse_list(value)?,
            "remember_last" => assign!(self.remember_last, bool),
            "last_selection" => self.last_selection = parse_list(value)?,
            "journal_size" => self.journal_size = value.to_string(),
            "journal_days" => assign!(self.journal_days, u32),
            "pacman_keep" => assign!(self.pacman_keep, usize),
            "temp_max_age_days" => assign!(self.temp_max_age_days, u32),
            "docker_build_cache" => assign!(self.docker_build_cache, bool),
            "hub_url" => self.hub_url = value.to_string(),
            "backup_dir" => self.backup_dir = value.to_string(),
            "log_file" => self.log_file = value.to_string(),
            other => {
                return Err(Error::Config(format!(
                    "unknown setting '{other}' (try `sweep config show`)"
                )))
            }
        }
        Ok(())
    }

    /// Names of every setting, for completion and error messages.
    pub fn field_names() -> &'static [&'static str] {
        &[
            "shred",
            "languages",
            "whitelist_files",
            "whitelist_folders",
            "custom_paths",
            "deepscan_roots",
            "deepscan_patterns",
            "remember_last",
            "last_selection",
            "journal_size",
            "journal_days",
            "pacman_keep",
            "temp_max_age_days",
            "docker_build_cache",
            "hub_url",
            "backup_dir",
            "log_file",
        ]
    }
}

/// Render a list the way [`parse_list`] can read back.
fn format_list(values: &[String]) -> String {
    if values.iter().any(|v| v.contains(',')) {
        serde_json::to_string(values).unwrap_or_else(|_| values.join(","))
    } else {
        values.join(",")
    }
}

/// Parse `a,b,c` or a JSON array into a list of strings.
fn parse_list(text: &str) -> Result<Vec<String>> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    if text.starts_with('[') {
        return serde_json::from_str(text).map_err(|err| {
            Error::Config(format!(
                "expected a comma-separated list or JSON array: {err}"
            ))
        });
    }
    Ok(text
        .split(',')
        .map(|part| part.trim().to_string())
        .filter(|part| !part.is_empty())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let config = Config::default();
        let path = std::env::temp_dir().join(format!("sweep_cfg_{}.toml", std::process::id()));
        config.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(config.shred, loaded.shred);
        assert_eq!(config.languages, loaded.languages);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn get_set_round_trips() {
        let mut config = Config::default();
        config.set_field("journal_days", "30").unwrap();
        assert_eq!(config.get_field("journal_days").as_deref(), Some("30"));
        config.set_field("languages", "tr, en").unwrap();
        assert_eq!(config.languages, vec!["tr", "en"]);
        // JSON arrays work too, which is how commas survive inside a value.
        config
            .set_field("whitelist_files", r#"["a,b.txt", "c.txt"]"#)
            .unwrap();
        assert_eq!(config.whitelist_files, vec!["a,b.txt", "c.txt"]);
        assert!(config
            .get_field("whitelist_files")
            .unwrap()
            .starts_with('['));
        config.set_field("shred", "true").unwrap();
        assert!(config.shred);
    }

    #[test]
    fn set_validates() {
        let mut config = Config::default();
        assert!(config.set_field("journal_days", "soon").is_err());
        assert!(config.set_field("pacman_keep", "-1").is_err());
        assert!(config.set_field("shred", "maybe").is_err());
        assert!(config.set_field("nope", "1").is_err());
        assert!(config.get_field("nope").is_none());
        // Unknown names are reported instead of being silently ignored.
        let err = config.set_field("nope", "1").unwrap_err().to_string();
        assert!(err.contains("unknown setting"), "{err}");
    }

    #[test]
    fn every_field_is_reachable_by_name() {
        // `sweep config show` prints these; a field added to the struct but
        // forgotten here would be invisible (and unsettable) from the CLI.
        let config = Config::default();
        let shown: Vec<&str> = config.fields().iter().map(|(k, _)| *k).collect();
        assert_eq!(shown, Config::field_names());
        for name in Config::field_names() {
            assert!(config.get_field(name).is_some(), "{name} missing");
        }
    }
}
