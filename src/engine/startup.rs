//! Başlangıç uygulamaları yönetimi (`sweep startup`).
//!
//! Üç platformda da yerel mekanizmalar okunur ve geri alınabilir biçimde
//! kapatılır — hiçbir girdi kalıcı olarak silinmez:
//!
//! - **Linux / diğer POSIX:** XDG autostart girdileri (`~/.config/autostart/*.desktop`,
//!   `/etc/xdg/autostart`) + `systemctl --user` birim/timer listesi. Kapatma,
//!   kullanıcı kopyasına `Hidden=true` yazar.
//! - **Windows:** `HKCU`/`HKLM` `...\CurrentVersion\Run` **ve `RunOnce`**
//!   anahtarları + `Startup` klasörleri (`%APPDATA%`, `%ProgramData%`).
//!   Kapatma, değeri `sweep-disabled` alt anahtarına taşır (Run/RunOnce),
//!   klasör girdilerini `.disabled` uzantısıyla yeniden adlandırır.
//! - **macOS:** `~/Library/LaunchAgents/*.plist`, `/Library/LaunchAgents` ve
//!   `/Library/LaunchDaemons`. Durum `launchctl list` ile okunur; kapatma
//!   plist dosyasını `.disabled` ekiyle yeniden adlandırır.
//!
//! ## Etki analizi (`--impact`)
//!
//! Her girdinin komut satırındaki ilk belirteç (çalıştırılabilir) çözülür,
//! boyutu ölçülür ve kaba bir başlatma süresi tahmin edilir:
//! `200 ms + boyut / 20 MB/s`. Bu bir *tahmindir*, kesin bir ölçüm değil —
//! amaç onlarca girdiyi "ağır / orta / hafif" diye ayırmaktır.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "startup";

/// 50 MB ve üzeri çalıştırılabilir → ağır.
const IMPACT_HIGH_BYTES: u64 = 50 * 1024 * 1024;
/// 5 MB ve üzeri çalıştırılabilir → orta.
const IMPACT_MEDIUM_BYTES: u64 = 5 * 1024 * 1024;
/// Kaba disk okuma hızı varsayımı (bayt/saniye) — tahmin modeli için.
const ASSUMED_DISK_BYTES_PER_SEC: u64 = 20 * 1024 * 1024;
/// Sabit süreç kurulum maliyeti (ms).
const FIXED_STARTUP_MS: u64 = 200;

fn lang() -> crate::i18n::Lang {
    crate::i18n::Lang::detect(&[])
}

/// Etki seviyesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImpactLevel {
    High,
    Medium,
    Low,
}

impl ImpactLevel {
    /// Rapor/CLI etiketi.
    pub fn as_str(self) -> &'static str {
        match self {
            ImpactLevel::High => "high",
            ImpactLevel::Medium => "medium",
            ImpactLevel::Low => "low",
        }
    }
}

/// Tek girdinin etki tahmini.
#[derive(Debug, Clone, Serialize)]
pub struct Impact {
    pub level: ImpactLevel,
    /// Çözülen çalıştırılabilirin boyutu (ölçülemediyse `None`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_bytes: Option<u64>,
    /// Tahmini başlatma maliyeti (ms).
    pub estimated_ms: u64,
    /// İnsan okunur gerekçe.
    pub reason: String,
}

/// Tek başlangıç girdisi.
#[derive(Debug, Clone, Serialize)]
pub struct AutostartEntry {
    pub name: String,
    pub path: PathBuf,
    pub system: bool,
    pub enabled: bool,
    /// Komut satırı (biliniyorsa).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Etki tahmini (yalnız `--impact` ile doldurulur).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impact: Option<Impact>,
}

impl AutostartEntry {
    /// Yeni girdi (etki alanı boş).
    fn new(name: String, path: PathBuf, system: bool, enabled: bool) -> Self {
        Self {
            name,
            path,
            system,
            enabled,
            command: None,
            impact: None,
        }
    }

    /// Komut satırını ata ve (istenirse) etki analizini çalıştır.
    fn with_command(mut self, command: Option<String>, analyze: bool) -> Self {
        if let Some(cmd) = &command {
            if analyze {
                self.impact = Some(analyze_impact(cmd, self.system));
            }
        }
        self.command = command;
        self
    }
}

// ---------------------------------------------------------------------------
// Etki analizi (saf yardımcılar — birim testleri bunları doğrular)
// ---------------------------------------------------------------------------

/// Komut satırını belirteçlere ayır; tırnaklı parçalar tek belirteç kalır.
///
/// Tırnaklar kaldırılır, yani `"C:\Program Files\a.exe" --x` iki belirteç
/// üretir: `C:\Program Files\a.exe` ve `--x`.
pub fn split_command(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in command.trim().chars() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None;
                } else {
                    current.push(ch);
                }
            }
            None => {
                if ch == '"' || ch == '\'' {
                    quote = Some(ch);
                } else if ch.is_whitespace() {
                    if !current.is_empty() {
                        out.push(std::mem::take(&mut current));
                    }
                } else {
                    current.push(ch);
                }
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

/// Komut satırının ilk belirtecini (çalıştırılabilir yolu) çıkar.
pub fn parse_command_token(command: &str) -> Option<String> {
    split_command(command).into_iter().next()
}

/// Komut satırından gerçekten var olan programı ve yolunu çöz.
///
/// Windows `Run` değerleri çoğu zaman tırnaksızdır ama yol boşluk içerir
/// (`C:\Program Files\Foo\foo.exe --flag`). Bu yüzden en uzun *var olan*
/// komut satırı ön ekini ararız. Hiçbiri yoksa `None` döner ve çağıran
/// "binary not found: <ilk belirteç>" diye raporlar.
pub fn resolve_command(command: &str) -> Option<(String, PathBuf)> {
    let chunks = split_command(command);
    let first = chunks.first()?.clone();
    if chunks.len() == 1 {
        return resolve_program(&first).map(|path| (first, path));
    }
    for end in (1..=chunks.len()).rev() {
        let candidate = chunks[..end].join(" ");
        if let Some(path) = resolve_program(&candidate) {
            return Some((candidate, path));
        }
    }
    None
}

/// Çalıştırılabilir boyutundan kaba başlatma süresi tahmini (ms).
pub fn estimate_ms(bytes: u64) -> u64 {
    FIXED_STARTUP_MS + (bytes / (ASSUMED_DISK_BYTES_PER_SEC / 1000))
}

/// Boyuttan etki seviyesi.
pub fn level_for_bytes(bytes: u64) -> ImpactLevel {
    if bytes >= IMPACT_HIGH_BYTES {
        ImpactLevel::High
    } else if bytes >= IMPACT_MEDIUM_BYTES {
        ImpactLevel::Medium
    } else {
        ImpactLevel::Low
    }
}

/// Bir program belirtecini gerçek dosyaya çöz (`$PATH` dahil).
pub fn resolve_program(token: &str) -> Option<PathBuf> {
    let candidate = Path::new(token);
    if token.contains('/') || token.contains('\\') {
        return candidate.is_file().then(|| candidate.to_path_buf());
    }
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let direct = dir.join(token);
        if direct.is_file() {
            return Some(direct);
        }
        // Windows'ta uzantısız `foo` → `foo.exe`.
        #[cfg(windows)]
        {
            for ext in ["exe", "com", "bat", "cmd"] {
                let with_ext = dir.join(format!("{token}.{ext}"));
                if with_ext.is_file() {
                    return Some(with_ext);
                }
            }
        }
    }
    None
}

/// Komut satırı için etki tahmini üret.
///
/// Program çözülemezse seviye `Medium` olur: bilinmeyen bir başlangıç
/// maliyetini "hafif" saymak yanıltıcı olurdu.
pub fn analyze_impact(command: &str, system: bool) -> Impact {
    let scope = if system { "system-wide" } else { "per-user" };
    let Some(token) = parse_command_token(command) else {
        return Impact {
            level: ImpactLevel::Medium,
            binary_bytes: None,
            estimated_ms: FIXED_STARTUP_MS,
            reason: format!("empty command ({scope})"),
        };
    };
    match resolve_command(command) {
        Some((program, path)) => {
            let bytes = crate::fsutil::size::size_of_or_zero(&path);
            let level = level_for_bytes(bytes);
            let short = Path::new(&program)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| program.clone());
            Impact {
                level,
                binary_bytes: Some(bytes),
                estimated_ms: estimate_ms(bytes),
                reason: format!(
                    "{short} — {} binary, ~{} start ({scope})",
                    crate::fsutil::size::bytes_to_human(bytes, false),
                    format_ms(estimate_ms(bytes))
                ),
            }
        }
        None => Impact {
            level: ImpactLevel::Medium,
            binary_bytes: None,
            estimated_ms: FIXED_STARTUP_MS,
            reason: format!("binary not found: {token} ({scope})"),
        },
    }
}

/// Milisaniyeyi okunur biçime çevir (`1.4s`, `320ms`).
pub fn format_ms(ms: u64) -> String {
    if ms >= 1000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{ms}ms")
    }
}

// ---------------------------------------------------------------------------
// Ortak arama
// ---------------------------------------------------------------------------

/// Adı ya da yolu hedefle eşleştir (uzantı ve büyük/küçük harf duyarsız).
fn matches(entry: &AutostartEntry, target: &str) -> bool {
    let t = target.trim().to_lowercase();
    if t.is_empty() {
        return false;
    }
    let name = entry.name.to_lowercase();
    let path = entry.path.to_string_lossy().to_lowercase();
    let stem = entry
        .path
        .file_stem()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    name == t || stem == t || path == t || path.ends_with(&format!("\\{t}")) || path.contains(&t)
}

/// Girdi listesinde hedefi bul.
fn find<'a>(entries: &'a [AutostartEntry], target: &str) -> Option<&'a AutostartEntry> {
    entries.iter().find(|e| matches(e, target))
}

// ---------------------------------------------------------------------------
// XDG autostart (`.desktop` dosyaları) — POSIX, macOS hariç
// ---------------------------------------------------------------------------

#[cfg(all(unix, not(target_os = "macos")))]
mod xdg {
    use super::*;

    fn user_dir() -> Option<PathBuf> {
        crate::platform::home_dir().map(|h| h.join(".config").join("autostart"))
    }

    fn system_dirs() -> Vec<PathBuf> {
        let mut dirs = vec![PathBuf::from("/etc/xdg/autostart")];
        if let Ok(xdg) = std::env::var("XDG_CONFIG_DIRS") {
            for part in std::env::split_paths(&xdg) {
                dirs.push(part.join("autostart"));
            }
        }
        dirs
    }

    fn read_field(path: &Path, field: &str) -> Option<String> {
        let text = std::fs::read_to_string(path).ok()?;
        text.lines().find_map(|line| {
            let (key, value) = line.split_once('=')?;
            if key.trim().eq_ignore_ascii_case(field) {
                Some(value.trim().to_string())
            } else {
                None
            }
        })
    }

    /// `.desktop` girdileri (kullanıcı kopyası sistem girdisini gölgeler).
    fn desktop_entries(analyze: bool) -> Vec<AutostartEntry> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut dirs: Vec<(PathBuf, bool)> = Vec::new();
        if let Some(user) = user_dir() {
            dirs.push((user, false));
        }
        dirs.extend(system_dirs().into_iter().map(|d| (d, true)));

        for (dir, system) in dirs {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e != "desktop").unwrap_or(true) {
                    continue;
                }
                let stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if !seen.insert(stem.clone()) {
                    continue;
                }
                let name = read_field(&path, "Name").unwrap_or(stem);
                let hidden = read_field(&path, "Hidden")
                    .map(|v| v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);
                let exec = read_field(&path, "Exec");
                out.push(
                    AutostartEntry::new(name, path, system, !hidden)
                        .with_command(exec, analyze),
                );
            }
        }
        out
    }

    /// `systemctl --user` birim ve timer listesi (salt okunur).
    fn systemd_entries() -> Vec<AutostartEntry> {
        if !crate::deep::have("systemctl") {
            return Vec::new();
        }
        let mut out = Vec::new();
        for args in [
            vec![
                "--user",
                "list-unit-files",
                "--type=service",
                "--state=enabled",
                "--no-legend",
                "--no-pager",
            ],
            vec!["--user", "list-timers", "--all", "--no-legend", "--no-pager"],
        ] {
            let Some(text) = crate::deep::command_output("systemctl", &args) else {
                continue;
            };
            for line in text.lines() {
                let Some(unit) = line.split_whitespace().next() else {
                    continue;
                };
                if !(unit.ends_with(".service") || unit.ends_with(".timer")) {
                    continue;
                }
                out.push(AutostartEntry::new(
                    unit.to_string(),
                    PathBuf::from(format!("systemd:user:{unit}")),
                    false,
                    true,
                ));
            }
        }
        out
    }

    pub fn list(analyze: bool) -> Vec<AutostartEntry> {
        let mut out = desktop_entries(analyze);
        out.extend(systemd_entries());
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        out
    }

    /// `Hidden=` bayrağını kullanıcı kopyasında ayarla.
    fn set_hidden(entry: &AutostartEntry, hidden: bool) -> Result<(), String> {
        let Some(user) = user_dir() else {
            return Err(crate::i18n::t(&lang(), "no home dir"));
        };
        std::fs::create_dir_all(&user).map_err(|e| format!("cannot open autostart: {e}"))?;
        let dest = user.join(
            entry
                .path
                .file_name()
                .ok_or_else(|| crate::i18n::t(&lang(), "no file name"))?,
        );
        if dest != entry.path {
            std::fs::copy(&entry.path, &dest).map_err(|e| format!("cannot copy: {e}"))?;
        }
        let mut text = std::fs::read_to_string(&dest).map_err(|e| format!("cannot read: {e}"))?;
        let mut found = false;
        let mut lines: Vec<String> = Vec::new();
        for line in text.lines() {
            let is_hidden = line
                .split_once('=')
                .map(|(key, _)| key.trim().eq_ignore_ascii_case("Hidden"))
                .unwrap_or(false);
            if is_hidden {
                lines.push(format!("Hidden={}", if hidden { "true" } else { "false" }));
                found = true;
            } else {
                lines.push(line.to_string());
            }
        }
        if !found {
            lines.push(format!("Hidden={}", if hidden { "true" } else { "false" }));
        }
        text = lines.join("\n");
        if !text.ends_with('\n') {
            text.push('\n');
        }
        std::fs::write(&dest, text).map_err(|e| format!("cannot write: {e}"))?;
        Ok(())
    }

    pub fn set(name: &str, enabled: bool) -> Result<String, String> {
        let entries = list(false);
        let entry = find(&entries, name)
            .ok_or_else(|| crate::i18n::et(&lang(), "not found: {}", &[&name]))?;
        if entry.path.to_string_lossy().starts_with("systemd:") {
            return Err(
                "systemd user units are managed with `systemctl --user`".to_string(),
            );
        }
        set_hidden(entry, !enabled)?;
        Ok(format!(
            "{}: {}",
            if enabled { "enabled" } else { "disabled" },
            entry.name
        ))
    }
}

// ---------------------------------------------------------------------------
// Windows: Run + RunOnce + Startup klasörleri
// ---------------------------------------------------------------------------

/// `HKCU`/`HKLM` altındaki otomatik başlatma anahtarları ve
/// geri alınabilir "devre dışı" karşılıkları.
#[cfg(windows)]
const RUN_KEYS: &[(&str, &str)] = &[
    (
        r"Software\Microsoft\Windows\CurrentVersion\Run",
        r"Software\Microsoft\Windows\CurrentVersion\Run\sweep-disabled",
    ),
    (
        r"Software\Microsoft\Windows\CurrentVersion\RunOnce",
        r"Software\Microsoft\Windows\CurrentVersion\RunOnce\sweep-disabled",
    ),
];

#[cfg(windows)]
mod win {
    use super::*;
    use winreg::enums::*;
    use winreg::types::FromRegValue;
    use winreg::RegKey;

    fn open_hive(system: bool) -> RegKey {
        if system {
            RegKey::predef(HKEY_LOCAL_MACHINE)
        } else {
            RegKey::predef(HKEY_CURRENT_USER)
        }
    }

    /// (değer adı, komut) çiftleri; boş ad (varsayılan değer) atlanır.
    ///
    /// winreg 0.52 exposes the values through an infallible `enum_values()`
    /// iterator of `io::Result<(String, RegValue)>`; the older `value_names()`
    /// helper does not exist in that release.
    fn read_values(key: &RegKey) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for value in key.enum_values() {
            let Ok((name, reg_value)) = value else {
                continue;
            };
            if name.is_empty() {
                continue;
            }
            // REG_EXPAND_SZ / REG_MULTI_SZ gibi türler String'e çevrilemez;
            // onları sessizce atlarız — bir başlangıç komutu olarak işe yaramaz.
            let Ok(data) = String::from_reg_value(&reg_value) else {
                continue;
            };
            out.push((name, data));
        }
        out
    }

    fn registry_entries(analyze: bool) -> Vec<AutostartEntry> {
        let mut out = Vec::new();
        for (system, label) in [(false, "HKCU"), (true, "HKLM")] {
            let hive = open_hive(system);
            for (subkey, disabled_subkey) in RUN_KEYS {
                if let Ok(key) = hive.open_subkey_with_flags(subkey, KEY_READ) {
                    for (name, command) in read_values(&key) {
                        let path = PathBuf::from(format!("{label}\\{subkey}\\{name}"));
                        out.push(
                            AutostartEntry::new(name, path, system, true)
                                .with_command(Some(command), analyze),
                        );
                    }
                }
                // Devre dışı bırakılmış (taşınmış) girdiler — geri alınabilir.
                if let Ok(key) = hive.open_subkey_with_flags(disabled_subkey, KEY_READ) {
                    for (name, command) in read_values(&key) {
                        let path = PathBuf::from(format!("{label}\\{disabled_subkey}\\{name}"));
                        out.push(
                            AutostartEntry::new(name, path, system, false)
                                .with_command(Some(command), analyze),
                        );
                    }
                }
            }
        }
        out
    }

    /// `Startup` klasörleri: `%APPDATA%` (kullanıcı) ve `%ProgramData%` (sistem).
    fn startup_folder_dirs() -> Vec<(PathBuf, bool)> {
        let mut out = Vec::new();
        let tail = "Microsoft\\Windows\\Start Menu\\Programs\\Startup";
        if let Ok(appdata) = std::env::var("APPDATA") {
            out.push((PathBuf::from(appdata).join(tail), false));
        }
        if let Ok(program_data) = std::env::var("ProgramData") {
            out.push((PathBuf::from(program_data).join(tail), true));
        }
        out
    }

    fn startup_folder_entries(analyze: bool) -> Vec<AutostartEntry> {
        let mut out = Vec::new();
        for (dir, system) in startup_folder_dirs() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                // `desktop.ini` Windows'un kendi meta dosyasıdır, girdi değildir.
                if path
                    .file_name()
                    .map(|n| n.to_string_lossy().eq_ignore_ascii_case("desktop.ini"))
                    .unwrap_or(false)
                {
                    continue;
                }
                let enabled = !path
                    .extension()
                    .map(|e| e.eq_ignore_ascii_case("disabled"))
                    .unwrap_or(false);
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
                // Kısayolun kendisi küçüktür; hedefi çözmek pahalıdır. Bu yüzden
                // etki analizi dosyanın boyutuna dayanır ve gerekçede belirtilir.
                let mut entry = AutostartEntry::new(name, path.clone(), system, enabled);
                if analyze {
                    let bytes = crate::fsutil::size::size_of_or_zero(&path);
                    entry.impact = Some(Impact {
                        level: level_for_bytes(bytes),
                        binary_bytes: Some(bytes),
                        estimated_ms: estimate_ms(bytes),
                        reason: format!(
                            "startup shortcut ({}), target not resolved",
                            crate::fsutil::size::bytes_to_human(bytes, false)
                        ),
                    });
                }
                out.push(entry);
            }
        }
        out
    }

    pub fn list(analyze: bool) -> Vec<AutostartEntry> {
        let mut out = registry_entries(analyze);
        out.extend(startup_folder_entries(analyze));
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        out
    }

    /// Kayıt defteri girdisini taşı (silme yok).
    fn move_registry_value(entry: &AutostartEntry) -> Result<(), String> {
        let hive = open_hive(entry.system);
        let access = KEY_READ | KEY_WRITE;
        // Yolu `HIVE\subkey\value` biçiminden çöz.
        let text = entry.path.to_string_lossy().into_owned();
        let (subkey, disabled_subkey) = RUN_KEYS
            .iter()
            .find(|(subkey, _)| text.contains(subkey))
            .copied()
            .ok_or_else(|| format!("unrecognized registry path: {text}"))?;

        let run = hive
            .open_subkey_with_flags(subkey, access)
            .map_err(|e| format!("cannot open {subkey}: {e}"))?;
        let disabled = hive
            .open_subkey_with_flags(disabled_subkey, access)
            .or_else(|_| hive.create_subkey(disabled_subkey).map(|(key, _)| key))
            .map_err(|e| format!("cannot open {disabled_subkey}: {e}"))?;

        // Önce hedefe yaz, sonra kaynaktan sil: yazım başarısız olursa girdi
        // kaybolmaz (kopya kalır), silme başarısız olursa çift kayıt oluşur —
        // ikisi de veri kaybından iyidir.
        let (src, dst) = if entry.enabled {
            (&run, &disabled)
        } else {
            (&disabled, &run)
        };
        let value: String = src
            .get_value(&entry.name)
            .map_err(|e| format!("cannot read value: {e}"))?;
        dst.set_value(&entry.name, &value)
            .map_err(|e| format!("cannot write value (needs elevation for HKLM): {e}"))?;
        src.delete_value(&entry.name)
            .map_err(|e| format!("cannot remove old value: {e}"))?;
        Ok(())
    }

    /// Startup klasöründeki dosyayı yeniden adlandır (`foo.lnk` ↔ `foo.lnk.disabled`).
    fn toggle_startup_file(entry: &AutostartEntry) -> Result<(), String> {
        let path = &entry.path;
        if entry.enabled {
            let dest = PathBuf::from(format!("{}.disabled", path.display()));
            std::fs::rename(path, &dest).map_err(|e| format!("cannot disable: {e}"))
        } else {
            // `.disabled` ekini kaldır.
            let text = path.to_string_lossy().into_owned();
            let dest = PathBuf::from(text.trim_end_matches(".disabled"));
            std::fs::rename(path, &dest).map_err(|e| format!("cannot enable: {e}"))
        }
    }

    pub fn set(name: &str, enabled: bool) -> Result<String, String> {
        let entries = list(false);
        let entry = find(&entries, name)
            .ok_or_else(|| crate::i18n::et(&lang(), "not found: {}", &[&name]))?;
        if entry.path.to_string_lossy().contains("\\Start Menu\\") {
            toggle_startup_file(entry)?;
        } else {
            move_registry_value(entry)?;
        }
        Ok(format!(
            "{}: {}",
            if enabled { "enabled" } else { "disabled" },
            entry.name
        ))
    }
}

// ---------------------------------------------------------------------------
// macOS: launchd agent / daemon plist'leri
// ---------------------------------------------------------------------------

#[cfg(target_os = "macos")]
mod mac {
    use super::*;

    /// Aranacak dizinler: (yol, sistem mi).
    fn launch_dirs() -> Vec<(PathBuf, bool)> {
        let mut out = Vec::new();
        if let Some(home) = crate::platform::home_dir() {
            out.push((home.join("Library/LaunchAgents"), false));
        }
        out.push((PathBuf::from("/Library/LaunchAgents"), true));
        out.push((PathBuf::from("/Library/LaunchDaemons"), true));
        out
    }

    /// `<string>` içindeki ilk değeri (Program / ProgramArguments[0]) çıkar.
    fn first_string(text: &str, key: &str) -> Option<String> {
        let at = text.find(&format!("<key>{key}</key>"))?;
        let rest = &text[at..];
        let open = rest.find("<string>")? + "<string>".len();
        let close = rest[open..].find("</string>")? + open;
        Some(rest[open..close].trim().to_string())
    }

    /// `launchctl list <label>` ile yüklü mü? (çıktı varsa yüklüdür.)
    fn is_loaded(label: &str) -> bool {
        crate::deep::command_output("launchctl", &["list", label])
            .map(|out| out.contains(label) || out.contains("PID"))
            .unwrap_or(false)
    }

    pub fn list(analyze: bool) -> Vec<AutostartEntry> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (dir, system) in launch_dirs() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if !name.ends_with(".plist") && !name.ends_with(".plist.disabled") {
                    continue;
                }
                // Aynı etiket birden çok dizinde olabilir; ilk görülen kazanır.
                let label = name
                    .trim_end_matches(".disabled")
                    .trim_end_matches(".plist")
                    .to_string();
                if !seen.insert(label.clone()) {
                    continue;
                }
                let enabled = name.ends_with(".plist");
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                let command = first_string(&text, "Program")
                    .or_else(|| first_string(&text, "ProgramArguments"));
                // `with_command` ortak yolu kullanılır (aksi halde macOS
                // derlemesinde hiç çağrılmadığı için ölü kod uyarısı verirdi).
                // launchd işlerinde `Program`/`ProgramArguments` yoksa etki
                // tahmini yine gösterilir, ama gerekçe bunu açıkça söyler.
                let mut item = AutostartEntry::new(label.clone(), path, system, enabled)
                    .with_command(command.clone(), analyze);
                if analyze && item.impact.is_none() {
                    item.impact = Some(Impact {
                        level: ImpactLevel::Medium,
                        binary_bytes: None,
                        estimated_ms: FIXED_STARTUP_MS,
                        reason: format!("launchd job {label} (no Program key)"),
                    });
                }
                // `launchctl` durumu yalnız bilgi amaçlıdır; `enabled` alanı
                // dosya adına göre belirlenir (geri alınabilir kapatma).
                if enabled && !is_loaded(&label) {
                    log::debug!("launchd job not loaded: {label}");
                }
                out.push(item);
            }
        }
        out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        out
    }

    pub fn set(name: &str, enabled: bool) -> Result<String, String> {
        let entries = list(false);
        let entry = find(&entries, name)
            .ok_or_else(|| crate::i18n::et(&lang(), "not found: {}", &[&name]))?;
        let text = entry.path.to_string_lossy().into_owned();
        let dest = if enabled {
            PathBuf::from(text.trim_end_matches(".disabled"))
        } else {
            PathBuf::from(format!("{text}.disabled"))
        };
        std::fs::rename(&entry.path, &dest).map_err(|e| {
            format!("cannot rename {}: {e} (system daemons need root)", entry.path.display())
        })?;
        Ok(format!(
            "{}: {}",
            if enabled { "enabled" } else { "disabled" },
            entry.name
        ))
    }
}

// ---------------------------------------------------------------------------
// Ortak rapor + CLI girişi
// ---------------------------------------------------------------------------

/// Bu platformdaki tüm başlangıç girdileri.
///
/// `analyze` doğruysa her girdinin etki tahmini de doldurulur (dosya sistemi
/// erişimi gerektirir; `list` hızlı kalsın diye varsayılan `false`).
pub fn list_with_impact(analyze: bool) -> Vec<AutostartEntry> {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        xdg::list(analyze)
    }
    #[cfg(windows)]
    {
        win::list(analyze)
    }
    #[cfg(target_os = "macos")]
    {
        mac::list(analyze)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = analyze;
        Vec::new()
    }
}

/// Etki analizi olmadan liste (eski imza).
pub fn list() -> Vec<AutostartEntry> {
    list_with_impact(false)
}

/// Bir girdiyi aç/kapat (geri alınabilir).
pub fn set(name: &str, enabled: bool) -> Result<String, String> {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        xdg::set(name, enabled)
    }
    #[cfg(windows)]
    {
        win::set(name, enabled)
    }
    #[cfg(target_os = "macos")]
    {
        mac::set(name, enabled)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (name, enabled);
        Err("startup management is unsupported on this platform".to_string())
    }
}

/// Girdileri rapora yaz; `impact` doğruysa etki rozeti de eklenir.
///
/// `cli::print_report` `path` doluysa etiketi hiç yazmaz, bu yüzden etki
/// modunda yol da etikete katılır ve `path` boş bırakılır — aksi halde
/// "high/medium/low" rozeti komut satırında görünmezdi. `--json` çıktısındaki
/// yapısal `entries` dizisi yolu her zaman taşır.
pub fn report_list(report: &mut Report, impact: bool) {
    for entry in list_with_impact(impact) {
        let scope = if entry.system { " (system)" } else { "" };
        let command = match &entry.command {
            Some(cmd) => format!(" — {}", truncate(cmd, 90)),
            None => String::new(),
        };
        if impact {
            let badge = match &entry.impact {
                Some(i) => format!(" [{} — {}]", i.level.as_str(), i.reason),
                None => String::new(),
            };
            report.push(Entry::new(
                EntryKind::Command,
                CLEANER_ID,
                if entry.enabled { "enabled" } else { "disabled" },
                format!(
                    "{}{}{}{} — {}",
                    entry.name,
                    badge,
                    scope,
                    command,
                    entry.path.display()
                ),
                None,
                0,
            ));
        } else {
            report.push(Entry::new(
                EntryKind::Command,
                CLEANER_ID,
                if entry.enabled { "enabled" } else { "disabled" },
                format!("{}{}{}", entry.name, scope, command),
                Some(&entry.path),
                0,
            ));
        }
    }
}

/// Uzun komut satırlarını rapor için kısalt.
fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let head: String = text.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

/// `op`: list | enable <hedef> | disable <hedef>.
///
/// `impact` yalnız `list` için anlamlıdır.
pub fn run(op: &str, target: Option<&str>, impact: bool) -> Report {
    let mut report = Report::new();
    match op {
        "list" => {
            report_list(&mut report, impact);
            if report.entries.is_empty() {
                report.push(Entry::new(
                    EntryKind::Command,
                    CLEANER_ID,
                    "list",
                    crate::i18n::t(&lang(), "no startup entries"),
                    None,
                    0,
                ));
            }
        }
        "enable" | "disable" => {
            let Some(name) = target else {
                report.fail(
                    CLEANER_ID,
                    op,
                    crate::i18n::t(&lang(), "target required (file or display name)"),
                );
                return report;
            };
            match set(name, op == "enable") {
                Ok(done) => report.push(Entry::new(
                    EntryKind::Command,
                    CLEANER_ID,
                    op,
                    done,
                    None,
                    0,
                )),
                Err(err) => report.fail(CLEANER_ID, op, err),
            }
        }
        other => report.fail(
            CLEANER_ID,
            "args",
            crate::i18n::et(
                &lang(),
                "unknown op: {} (list/enable/disable)",
                &[&other.to_string()],
            ),
        ),
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_tokens_handle_quotes_and_bare_paths() {
        assert_eq!(
            parse_command_token("\"C:\\Program Files\\Foo\\foo.exe\" --start"),
            Some("C:\\Program Files\\Foo\\foo.exe".to_string())
        );
        assert_eq!(
            parse_command_token("/usr/bin/foo --daemon"),
            Some("/usr/bin/foo".to_string())
        );
        assert_eq!(parse_command_token("   "), None);
        assert_eq!(parse_command_token(""), None);
        assert_eq!(
            parse_command_token("'/opt/My App/app' -x"),
            Some("/opt/My App/app".to_string())
        );
    }

    #[test]
    fn impact_levels_follow_the_size_thresholds() {
        assert_eq!(level_for_bytes(0), ImpactLevel::Low);
        assert_eq!(level_for_bytes(IMPACT_MEDIUM_BYTES - 1), ImpactLevel::Low);
        assert_eq!(level_for_bytes(IMPACT_MEDIUM_BYTES), ImpactLevel::Medium);
        assert_eq!(level_for_bytes(IMPACT_HIGH_BYTES - 1), ImpactLevel::Medium);
        assert_eq!(level_for_bytes(IMPACT_HIGH_BYTES), ImpactLevel::High);
    }

    #[test]
    fn estimate_scales_with_size() {
        // 20 MB tam olarak 1 saniyeye denk gelir (artı sabit maliyet).
        assert_eq!(estimate_ms(ASSUMED_DISK_BYTES_PER_SEC), FIXED_STARTUP_MS + 1000);
        assert_eq!(estimate_ms(0), FIXED_STARTUP_MS);
        assert_eq!(format_ms(320), "320ms");
        assert_eq!(format_ms(1400), "1.4s");
    }

    #[test]
    fn impact_of_a_fixture_binary_is_measured() {
        // Bilinen boyutta bir "çalıştırılabilir" dosya üret ve etkisini ölç.
        let path = std::env::temp_dir().join("sweep_startup_fixture.bin");
        std::fs::write(&path, vec![0u8; 6 * 1024 * 1024]).unwrap();
        let impact = analyze_impact(&format!("\"{}\" --run", path.display()), false);
        assert_eq!(impact.level, ImpactLevel::Medium);
        assert_eq!(impact.binary_bytes, Some(6 * 1024 * 1024));
        assert!(impact.estimated_ms > FIXED_STARTUP_MS);
        assert!(impact.reason.contains("per-user"));

        // Sistem kapsamı gerekçeye yansır.
        let system = analyze_impact(&format!("\"{}\"", path.display()), true);
        assert!(system.reason.contains("system-wide"));

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unquoted_paths_with_spaces_resolve() {
        // Gerçek Windows `Run` değerleri çoğu zaman tırnaksızdır.
        let dir = std::env::temp_dir().join("sweep startup unquoted");
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("My App.exe");
        std::fs::write(&exe, vec![0u8; 7 * 1024 * 1024]).unwrap();

        let command = format!("{} --hidden", exe.display());
        let impact = analyze_impact(&command, false);
        assert_eq!(
            impact.binary_bytes,
            Some(7 * 1024 * 1024),
            "reason: {}",
            impact.reason
        );
        assert_eq!(impact.level, ImpactLevel::Medium);
        assert!(impact.reason.contains("My App.exe"), "{}", impact.reason);

        // Boşluk ayıklaması: tırnaklı ve tırnaksız aynı sonucu vermeli.
        let quoted = format!("\"{}\" --hidden", exe.display());
        assert_eq!(
            analyze_impact(&quoted, false).binary_bytes,
            analyze_impact(&command, false).binary_bytes
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn split_command_respects_quotes() {
        let chunks = split_command("\"C:\\Program Files\\a b.exe\" --x  'y z'");
        assert_eq!(chunks, vec!["C:\\Program Files\\a b.exe", "--x", "y z"]);
        assert!(split_command("   ").is_empty());
    }

    #[test]
    fn unresolvable_binaries_are_medium_not_low() {
        let impact = analyze_impact("definitely-not-a-real-binary-xyz --flag", false);
        assert_eq!(impact.level, ImpactLevel::Medium);
        assert_eq!(impact.binary_bytes, None);
        assert!(impact.reason.contains("not found"));
    }

    #[test]
    fn empty_command_is_handled() {
        let impact = analyze_impact("   ", false);
        assert_eq!(impact.level, ImpactLevel::Medium);
        assert!(impact.reason.contains("empty command"));
    }

    #[test]
    fn truncate_keeps_short_strings_intact() {
        assert_eq!(truncate("abc", 10), "abc");
        assert_eq!(truncate("abcdefghij", 5), "abcd…");
    }

    #[test]
    fn list_and_report_never_panic() {
        let mut report = Report::new();
        report_list(&mut report, true);
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        // Girdi olmasa bile "no startup entries" satırı yazılır.
        assert!(!report.entries.is_empty());
    }

    #[test]
    fn unknown_op_is_reported() {
        let report = run("frobnicate", None, false);
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].message.contains("unknown op"));
    }

    #[test]
    fn enable_without_target_is_reported() {
        let report = run("disable", None, false);
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].message.contains("target required"));
    }
}
