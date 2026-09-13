//! Otomatik başlangıç ve zamanlanmış görev yönetimi (birleşik model).
//!
//! `startup` modülü tek bir liste üretir: kayıt defteri `Run`/`RunOnce`,
//! `Policies\Explorer\Run`, `Wow6432Node` aynaları, `Startup` klasörleri,
//! `Winlogon`, oturum açma betikleri **ve** oturum açma/başlatma tetikleyicili
//! zamanlanmış görevler. GUI tek bir listede gösterir; CLI `startup` ve
//! `tasks` komutları aynı modeli paylaşır.
//!
//! Her değişiklik önce yedeklenir ([`store`]), sonra uygulanır ve denetim
//! defterine yazılır ([`ledger`]). Yedek yazılamazsa değişiklik yapılmaz
//! ("fail closed"). Geri alma, yedeği okuyup önceki durumu gerçekten kurar.
//!
//! Keşif [`backend::Backend`] arayüzünün arkasındadır: sınıflandırma, filtre
//! ve defter mantığı gerçek kayıt defterine dokunmadan test edilebilir.

use std::path::PathBuf;

use serde::Serialize;
use sha2::{Digest, Sha256};

pub mod api;
pub mod backend;
pub mod classify;
pub mod engine;
pub mod ledger;
pub mod store;
#[cfg(unix)]
pub mod unix;
#[cfg(windows)]
pub mod windows;

pub use classify::{classify, Classification};
pub use crate::engine::startup::Impact;

/// Girdinin türü: başlangıç girdisi ya da zamanlanmış görev.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Kind {
    #[serde(rename = "startup")]
    Startup,
    #[serde(rename = "task")]
    Task,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Startup => "startup",
            Kind::Task => "task",
        }
    }
}

impl std::str::FromStr for Kind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "startup" | "start" | "auto" => Ok(Kind::Startup),
            "task" | "tasks" | "scheduled" => Ok(Kind::Task),
            other => Err(format!("unknown kind: {other} (startup|task)")),
        }
    }
}

/// Kapsam: kullanıcı ya da makine geneli.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Scope {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "system")]
    System,
}

impl Scope {
    pub fn as_str(self) -> &'static str {
        match self {
            Scope::User => "user",
            Scope::System => "system",
        }
    }
}

impl std::str::FromStr for Scope {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "user" | "u" => Ok(Scope::User),
            "system" | "machine" | "s" => Ok(Scope::System),
            other => Err(format!("unknown scope: {other} (user|system)")),
        }
    }
}

/// Girdinin geldiği mekanizma.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Source {
    #[serde(rename = "registry-run")]
    RegistryRun,
    #[serde(rename = "registry-runonce")]
    RegistryRunOnce,
    #[serde(rename = "registry-run-policy")]
    RegistryRunPolicy,
    #[serde(rename = "startup-folder")]
    StartupFolder,
    #[serde(rename = "startup-folder-common")]
    StartupFolderCommon,
    #[serde(rename = "winlogon")]
    Winlogon,
    #[serde(rename = "logon-script")]
    LogonScript,
    #[serde(rename = "xdg-autostart")]
    XdgAutostart,
    #[serde(rename = "systemd-user")]
    SystemdUser,
    #[serde(rename = "systemd-system")]
    SystemdSystem,
    #[serde(rename = "launchd-agent")]
    LaunchdAgent,
    #[serde(rename = "launchd-daemon")]
    LaunchdDaemon,
    #[serde(rename = "cron")]
    Cron,
    #[serde(rename = "shell-rc")]
    ShellRc,
    #[serde(rename = "scheduled-task")]
    ScheduledTask,
    #[serde(rename = "systemd-timer")]
    SystemdTimer,
    #[serde(rename = "cron-system")]
    CronSystem,
}

/// Tüm kaynaklar (filtre yardımı ve testler için).
pub const ALL_SOURCES: &[Source] = &[
    Source::RegistryRun,
    Source::RegistryRunOnce,
    Source::RegistryRunPolicy,
    Source::StartupFolder,
    Source::StartupFolderCommon,
    Source::Winlogon,
    Source::LogonScript,
    Source::XdgAutostart,
    Source::SystemdUser,
    Source::SystemdSystem,
    Source::LaunchdAgent,
    Source::LaunchdDaemon,
    Source::Cron,
    Source::ShellRc,
    Source::ScheduledTask,
    Source::SystemdTimer,
    Source::CronSystem,
];

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Source::RegistryRun => "registry-run",
            Source::RegistryRunOnce => "registry-runonce",
            Source::RegistryRunPolicy => "registry-run-policy",
            Source::StartupFolder => "startup-folder",
            Source::StartupFolderCommon => "startup-folder-common",
            Source::Winlogon => "winlogon",
            Source::LogonScript => "logon-script",
            Source::XdgAutostart => "xdg-autostart",
            Source::SystemdUser => "systemd-user",
            Source::SystemdSystem => "systemd-system",
            Source::LaunchdAgent => "launchd-agent",
            Source::LaunchdDaemon => "launchd-daemon",
            Source::Cron => "cron",
            Source::ShellRc => "shell-rc",
            Source::ScheduledTask => "scheduled-task",
            Source::SystemdTimer => "systemd-timer",
            Source::CronSystem => "cron-system",
        }
    }
}

impl std::str::FromStr for Source {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let t = s.trim().to_lowercase().replace('_', "-");
        ALL_SOURCES
            .iter()
            .copied()
            .find(|src| src.as_str() == t)
            .ok_or_else(|| format!("unknown source: {s}"))
    }
}

/// Sınıflandırma kategorisi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum Category {
    #[serde(rename = "os")]
    Os,
    #[serde(rename = "security")]
    Security,
    #[serde(rename = "driver")]
    Driver,
    #[serde(rename = "updater")]
    Updater,
    #[serde(rename = "third-party")]
    ThirdParty,
    #[serde(rename = "unknown")]
    Unknown,
}

pub const ALL_CATEGORIES: &[Category] = &[
    Category::Os,
    Category::Security,
    Category::Driver,
    Category::Updater,
    Category::ThirdParty,
    Category::Unknown,
];

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Os => "os",
            Category::Security => "security",
            Category::Driver => "driver",
            Category::Updater => "updater",
            Category::ThirdParty => "third-party",
            Category::Unknown => "unknown",
        }
    }
}

impl std::str::FromStr for Category {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        let t = s.trim().to_lowercase().replace('_', "-");
        ALL_CATEGORIES
            .iter()
            .copied()
            .find(|c| c.as_str() == t)
            .ok_or_else(|| format!("unknown category: {s}"))
    }
}

/// Risk seviyesi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Risk {
    Low,
    Medium,
    High,
    Critical,
}

impl Risk {
    pub fn as_str(self) -> &'static str {
        match self {
            Risk::Critical => "critical",
            Risk::High => "high",
            Risk::Medium => "medium",
            Risk::Low => "low",
        }
    }
}

impl std::str::FromStr for Risk {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "critical" => Ok(Risk::Critical),
            "high" => Ok(Risk::High),
            "medium" | "med" => Ok(Risk::Medium),
            "low" => Ok(Risk::Low),
            other => Err(format!("unknown risk: {other} (critical|high|medium|low)")),
        }
    }
}

/// Tetikleyici.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Trigger {
    Logon,
    Boot,
    Schedule,
    Event,
    Idle,
    Unknown,
}

impl Trigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Trigger::Logon => "logon",
            Trigger::Boot => "boot",
            Trigger::Schedule => "schedule",
            Trigger::Event => "event",
            Trigger::Idle => "idle",
            Trigger::Unknown => "unknown",
        }
    }
}

impl std::str::FromStr for Trigger {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "logon" | "log-in" | "login" => Ok(Trigger::Logon),
            "boot" | "startup" => Ok(Trigger::Boot),
            "schedule" | "time" | "timer" => Ok(Trigger::Schedule),
            "event" => Ok(Trigger::Event),
            "idle" => Ok(Trigger::Idle),
            "unknown" | "" => Ok(Trigger::Unknown),
            other => Err(format!("unknown trigger: {other}")),
        }
    }
}

/// Kayıt defteri kovanı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hive {
    Hkcu,
    Hklm,
}

impl Hive {
    pub fn label(self) -> &'static str {
        match self {
            Hive::Hkcu => "HKCU",
            Hive::Hklm => "HKLM",
        }
    }
}

/// Bir girdinin gerçek sistemde nasıl ele alınacağını taşıyan tutamaç.
///
/// JSON sözleşmesinde görünmez (`#[serde(skip)]`); yalnız arka ucun
/// okuma/yazma işlemleri için gereklidir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Handle {
    /// Windows kayıt defteri değeri. `base_subkey` etkin konumdur; kapatma
    /// değeri `<base_subkey>\sweep-disabled` alt anahtarına taşır.
    Registry {
        hive: Hive,
        base_subkey: String,
        value: String,
    },
    /// Dosya. `hidden_flag` ise `.desktop` gibi `Hidden=` ile kapatılır,
    /// değilse `<path>.disabled` adına yeniden adlandırılır.
    File { path: PathBuf, hidden_flag: bool },
    /// Windows zamanlanmış görevi.
    Task {
        task_path: String,
        task_name: String,
        protected: bool,
    },
    /// systemd birimi (salt okunur).
    Systemd { unit: String, user: bool },
    /// Değiştirilemez girdi (Winlogon, cron, shell rc...).
    ReadOnly,
    /// Testlerde kullanılan sahte arka uç adresi.
    Fake(String),
}

/// Birleşik girdi modeli — hem başlangıç girdileri hem görevler.
///
/// Alan adları ve enum yazımları GUI sözleşmesidir; değiştirmeyin.
#[derive(Debug, Clone, Serialize)]
pub struct Item {
    pub id: String,
    pub kind: Kind,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    pub source: Source,
    pub location: String,
    pub scope: Scope,
    pub enabled: bool,
    /// Tetikleyici; uygulanamıyorsa `null`.
    pub trigger: Option<Trigger>,
    pub category: Category,
    pub risk: Risk,
    pub reversible: bool,
    pub editable: bool,
    pub removable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_result: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impact: Option<Impact>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(skip)]
    pub handle: Handle,
}

impl Item {
    /// Yeni girdi; kimlik [`finalize_ids`] ile atanır.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        kind: Kind,
        name: impl Into<String>,
        source: Source,
        location: impl Into<String>,
        scope: Scope,
        enabled: bool,
        trigger: Option<Trigger>,
        handle: Handle,
    ) -> Self {
        let name = name.into();
        let location = location.into();
        let class = classify::classify(&name, None, &location, source);
        Self {
            id: item_id(kind, scope, source, &name),
            kind,
            name,
            command: None,
            source,
            location,
            scope,
            enabled,
            trigger,
            category: class.category,
            risk: class.risk,
            reversible: true,
            editable: true,
            removable: true,
            last_run: None,
            next_run: None,
            last_result: None,
            author: None,
            impact: None,
            notes: class.notes,
            handle,
        }
    }

    /// Sınıflandırmayı yeniden uygula (komut atandıktan sonra).
    pub fn reclassify(&mut self) {
        let class = classify::classify(&self.name, self.command.as_deref(), &self.location, self.source);
        self.category = class.category;
        self.risk = class.risk;
        if class.notes.is_some() {
            self.notes = class.notes;
        }
    }

    /// Kısa durum etiketi: `enabled` / `disabled`.
    pub fn state(&self) -> &'static str {
        if self.enabled {
            "enabled"
        } else {
            "disabled"
        }
    }
}

/// Kararlı kimlik: `kind|scope|source|name`.
///
/// Ad içindeki `|` ayıraçla karışmasın diye `/` yapılır; yeni satır boşluğa
/// çevrilir. Aynı girdi her çalıştırmada aynı kimliği üretir.
pub fn item_id(kind: Kind, scope: Scope, source: Source, name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| match c {
            '|' => '/',
            '\n' | '\r' => ' ',
            other => other,
        })
        .collect();
    let clean = clean.trim();
    let clean = if clean.is_empty() { "-" } else { clean };
    format!(
        "{}|{}|{}|{}",
        kind.as_str(),
        scope.as_str(),
        source.as_str(),
        clean
    )
}

/// Aynı taban kimliğe düşen girdileri konum karmasıyla ayır.
///
/// Örnek: `HKCU\...\Run` ve `HKLM\Software\Wow6432Node\...\Run` içindeki aynı
/// adlı iki değer. Konumdan türetilen kısa sha256 eki kararlıdır.
pub fn finalize_ids(items: &mut [Item]) {
    use std::collections::HashMap;
    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, item) in items.iter().enumerate() {
        groups.entry(item.id.clone()).or_default().push(i);
    }
    for idxs in groups.into_values() {
        if idxs.len() <= 1 {
            continue;
        }
        for i in idxs {
            let suffix = short_hash(&items[i].location);
            items[i].id = format!("{}~{}", items[i].id, suffix);
        }
    }
}

/// Bir metnin sha256'sının ilk 8 onaltılık hanesi.
pub fn short_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(8);
    for byte in digest.iter().take(4) {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Veri dizini: `<config>/sweep/autostart` (taşınabilir modda `<base>/autostart`).
pub fn data_dir() -> Option<PathBuf> {
    if let Some(base) = crate::config::portable_base() {
        return Some(base.join("autostart"));
    }
    crate::platform::config_dir().map(|d| d.join("autostart"))
}

// ---------------------------------------------------------------------------
// Zaman (harici crate yok — sivil takvim dönüşümü)
// ---------------------------------------------------------------------------

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    // Howard Hinnant'ın `civil_from_days` algoritması (proleptik Gregoryen).
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn utc_parts() -> (i64, u32, u32, u32, u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, mo, d) = civil_from_days(days);
    (
        y,
        mo,
        d,
        (rem / 3600) as u32,
        ((rem % 3600) / 60) as u32,
        (rem % 60) as u32,
    )
}

/// UTC RFC3339 zaman damgası (`2026-09-13T08:11:22Z`).
pub fn now_rfc3339() -> String {
    let (y, mo, d, h, mi, s) = utc_parts();
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Dosya adı için sıkışık zaman damgası (`20260913T081122Z`).
pub fn now_stamp() -> String {
    let (y, mo, d, h, mi, s) = utc_parts();
    format!("{y:04}{mo:02}{d:02}T{h:02}{mi:02}{s:02}Z")
}

/// İşletim sisteminden kullanıcı adı (`DOMAIN\user` Windows'ta küçük harfle).
pub fn os_user() -> String {
    #[cfg(windows)]
    {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "unknown".to_string());
        let domain = std::env::var("USERDOMAIN").unwrap_or_default();
        if domain.is_empty() {
            user
        } else {
            format!("{}\\{}", domain.to_lowercase(), user.to_lowercase())
        }
    }
    #[cfg(not(windows))]
    {
        std::env::var("USER")
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap_or_else(|_| "unknown".to_string())
    }
}

// ---------------------------------------------------------------------------
// Filtre / sıralama
// ---------------------------------------------------------------------------

/// Sıralama anahtarı.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortKey {
    #[default]
    Name,
    Impact,
    Source,
}

impl std::str::FromStr for SortKey {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.trim().to_lowercase().as_str() {
            "name" | "" => Ok(SortKey::Name),
            "impact" => Ok(SortKey::Impact),
            "source" => Ok(SortKey::Source),
            other => Err(format!("unknown sort key: {other} (name|impact|source)")),
        }
    }
}

/// Liste filtresi. Tüm alanlar `None` ise hiçbir şey elenmez.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub kind: Option<Kind>,
    pub scope: Option<Scope>,
    pub category: Option<Category>,
    pub risk: Option<Risk>,
    pub source: Option<Source>,
    /// Tetikleyici filtresi (görevler için).
    pub trigger: Option<Trigger>,
    /// `Some(true)` yalnız etkin, `Some(false)` yalnız kapalı.
    pub enabled: Option<bool>,
    pub search: Option<String>,
    pub sort: SortKey,
}

impl Filter {
    fn keeps(&self, item: &Item) -> bool {
        if let Some(kind) = self.kind {
            if item.kind != kind {
                return false;
            }
        }
        if let Some(scope) = self.scope {
            if item.scope != scope {
                return false;
            }
        }
        if let Some(cat) = self.category {
            if item.category != cat {
                return false;
            }
        }
        if let Some(risk) = self.risk {
            if item.risk != risk {
                return false;
            }
        }
        if let Some(source) = self.source {
            if item.source != source {
                return false;
            }
        }
        if let Some(trigger) = self.trigger {
            if item.trigger != Some(trigger) {
                return false;
            }
        }
        if let Some(enabled) = self.enabled {
            if item.enabled != enabled {
                return false;
            }
        }
        if let Some(query) = &self.search {
            let q = query.trim().to_lowercase();
            if q.is_empty() {
                return true;
            }
            let hay = format!(
                "{} {} {} {}",
                item.name,
                item.command.as_deref().unwrap_or(""),
                item.location,
                item.source.as_str()
            )
            .to_lowercase();
            if !hay.contains(&q) {
                return false;
            }
        }
        true
    }
}

/// Filtreyi uygula ve sırala (serileştirmeden **önce**).
pub fn filter_items(items: &[Item], filter: &Filter) -> Vec<Item> {
    let mut out: Vec<Item> = items.iter().filter(|i| filter.keeps(i)).cloned().collect();
    match filter.sort {
        SortKey::Name => out.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then_with(|| a.source.as_str().cmp(b.source.as_str()))
        }),
        SortKey::Source => out.sort_by(|a, b| {
            a.source
                .as_str()
                .cmp(b.source.as_str())
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        }),
        // Etki: en ağır önce; etkisi bilinmeyenler sona.
        SortKey::Impact => out.sort_by(|a, b| {
            let ma = a.impact.as_ref().map(|i| i.estimated_ms).unwrap_or(0);
            let mb = b.impact.as_ref().map(|i| i.estimated_ms).unwrap_or(0);
            mb.cmp(&ma)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        }),
    }
    out
}

// ---------------------------------------------------------------------------
// Arama (id → tam ad → benzersiz alt dizi)
// ---------------------------------------------------------------------------

fn id_list(items: &[&Item]) -> String {
    items
        .iter()
        .take(8)
        .map(|i| i.id.clone())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Hedefi sırayla çöz: **önce id, sonra tam ad, sonra benzersiz ad alt dizisi**.
///
/// Birden çok eşleşme "ambiguous" hatasıdır — tahmin edilmez.
pub fn lookup<'a>(items: &'a [Item], target: &str) -> Result<&'a Item, String> {
    let t = target.trim();
    if t.is_empty() {
        return Err("target required (id or name)".to_string());
    }
    if let Some(hit) = items.iter().find(|i| i.id == t) {
        return Ok(hit);
    }
    let lower = t.to_lowercase();
    let exact: Vec<&Item> = items
        .iter()
        .filter(|i| i.name.to_lowercase() == lower)
        .collect();
    match exact.len() {
        1 => return Ok(exact[0]),
        n if n > 1 => {
            return Err(format!(
                "ambiguous name '{t}' matches {n} entries; use an id: {}",
                id_list(&exact)
            ))
        }
        _ => {}
    }
    let subset: Vec<&Item> = items
        .iter()
        .filter(|i| i.name.to_lowercase().contains(&lower))
        .collect();
    match subset.len() {
        1 => Ok(subset[0]),
        0 => Err(format!("not found: {t}")),
        n => Err(format!(
            "ambiguous substring '{t}' matches {n} entries; narrow it or use an id: {}",
            id_list(&subset)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic(kind: Kind, name: &str, source: Source, scope: Scope, location: &str, enabled: bool) -> Item {
        let mut item = Item::new(
            kind,
            name,
            source,
            location,
            scope,
            enabled,
            Some(Trigger::Logon),
            Handle::Fake(location.to_string()),
        );
        item.command = Some(format!("C:\\Program Files\\{name}\\{name}.exe"));
        item.reclassify();
        item
    }

    #[test]
    fn ids_are_stable_across_two_builds() {
        let build = || {
            let mut items = vec![
                synthetic(Kind::Startup, "OneDrive", Source::RegistryRun, Scope::User, "HKCU\\Run", true),
                synthetic(Kind::Startup, "OneDrive", Source::RegistryRun, Scope::System, "HKLM\\Run", true),
                synthetic(Kind::Task, "Backup", Source::ScheduledTask, Scope::User, "\\Backup", true),
            ];
            finalize_ids(&mut items);
            items.into_iter().map(|i| i.id).collect::<Vec<_>>()
        };
        let a = build();
        let b = build();
        assert_eq!(a, b, "id'ler kararlı olmalı");
    }

    #[test]
    fn colliding_ids_get_a_location_hash() {
        // Aynı ad+source+scope, farklı konum → gerçekten farklı kimlik.
        let mut items = vec![
            synthetic(Kind::Startup, "NvBackend", Source::RegistryRun, Scope::System, "HKLM\\Run", true),
            synthetic(Kind::Startup, "NvBackend", Source::RegistryRun, Scope::System, "HKLM\\Wow6432Node\\Run", true),
        ];
        finalize_ids(&mut items);
        assert_ne!(items[0].id, items[1].id);
        assert!(items[0].id.contains('~'));
        // Karma eki kararlı: aynı konum aynı ek.
        let mut again = vec![
            synthetic(Kind::Startup, "NvBackend", Source::RegistryRun, Scope::System, "HKLM\\Run", true),
            synthetic(Kind::Startup, "NvBackend", Source::RegistryRun, Scope::System, "HKLM\\Wow6432Node\\Run", true),
        ];
        finalize_ids(&mut again);
        assert_eq!(items[0].id, again[0].id);
        assert_eq!(items[1].id, again[1].id);
    }

    #[test]
    fn lookup_prefers_id_then_exact_name_then_unique_substring() {
        let mut items = vec![
            synthetic(Kind::Startup, "OneDrive", Source::RegistryRun, Scope::User, "HKCU\\Run", true),
            synthetic(Kind::Startup, "OneDriveSetup", Source::RegistryRun, Scope::User, "HKCU\\RunOnce", false),
        ];
        finalize_ids(&mut items);

        // 1) id tam eşleşir ("OneDrive" hem id hem tam ad; id kazanır).
        let by_id = lookup(&items, &items[0].id).unwrap();
        assert_eq!(by_id.name, "OneDrive");

        // 2) tam ad.
        let by_name = lookup(&items, "OneDrive").unwrap();
        assert_eq!(by_name.id, items[0].id);

        // 3) benzersiz alt dizi.
        let by_sub = lookup(&items, "DriveSet").unwrap();
        assert_eq!(by_sub.name, "OneDriveSetup");

        // Belirsiz alt dizi hata verir, tahmin etmez.
        let err = lookup(&items, "Drive").unwrap_err();
        assert!(err.contains("ambiguous"), "{err}");

        // Bulunamayan.
        assert!(lookup(&items, "nope").unwrap_err().contains("not found"));
    }

    #[test]
    fn ambiguous_exact_name_is_an_error() {
        let mut items = vec![
            synthetic(Kind::Startup, "Same", Source::RegistryRun, Scope::User, "HKCU\\Run", true),
            synthetic(Kind::Startup, "Same", Source::RegistryRunOnce, Scope::User, "HKCU\\RunOnce", true),
        ];
        finalize_ids(&mut items);
        let err = lookup(&items, "Same").unwrap_err();
        assert!(err.contains("ambiguous"), "{err}");
    }

    #[test]
    fn filter_and_sort_apply_before_serialisation() {
        let items = vec![
            synthetic(Kind::Startup, "Beta", Source::RegistryRun, Scope::User, "HKCU\\Run", true),
            synthetic(Kind::Startup, "alpha", Source::RegistryRun, Scope::System, "HKLM\\Run", false),
            synthetic(Kind::Task, "Gamma", Source::ScheduledTask, Scope::User, "\\Gamma", true),
        ];
        let f = Filter {
            kind: Some(Kind::Startup),
            sort: SortKey::Name,
            ..Filter::default()
        };
        let out = filter_items(&items, &f);
        assert_eq!(out.len(), 2);
        // Ada göre büyük/küçük harf duyarsız sıra.
        assert_eq!(out[0].name, "alpha");
        assert_eq!(out[1].name, "Beta");

        let only_disabled = Filter {
            enabled: Some(false),
            ..Filter::default()
        };
        assert_eq!(filter_items(&items, &only_disabled).len(), 1);

        let search = Filter {
            search: Some("gamm".to_string()),
            ..Filter::default()
        };
        assert_eq!(filter_items(&items, &search)[0].name, "Gamma");
    }

    #[test]
    fn filtering_500_items_is_fast() {
        let items: Vec<Item> = (0..520)
            .map(|i| {
                let src = if i % 3 == 0 {
                    Source::RegistryRun
                } else if i % 3 == 1 {
                    Source::StartupFolder
                } else {
                    Source::ScheduledTask
                };
                let scope = if i % 2 == 0 { Scope::User } else { Scope::System };
                let kind = if i % 5 == 0 { Kind::Task } else { Kind::Startup };
                synthetic(kind, &format!("Item{i:04}"), src, scope, &format!("loc{i}"), i % 4 != 0)
            })
            .collect();
        let start = std::time::Instant::now();
        let f = Filter {
            kind: Some(Kind::Startup),
            enabled: Some(true),
            search: Some("item01".to_string()),
            sort: SortKey::Impact,
            ..Filter::default()
        };
        let out = filter_items(&items, &f);
        let elapsed = start.elapsed();
        assert!(!out.is_empty());
        // Cömert sınır: 500 girdilik filtre milisaniyeler almalı.
        assert!(
            elapsed.as_millis() < 200,
            "filtre çok yavaş: {elapsed:?}"
        );
    }

    #[test]
    fn json_contract_field_names_and_enum_spellings() {
        let mut item = synthetic(
            Kind::Startup,
            "OneDrive",
            Source::RegistryRun,
            Scope::User,
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            true,
        );
        item.command = Some("C:\\Program Files\\Microsoft OneDrive\\OneDrive.exe /background".into());
        item.trigger = Some(Trigger::Logon);
        item.last_run = None;
        item.author = None;
        item.impact = None;
        item.notes = Some("test".into());
        item.reclassify();
        let value = serde_json::to_value(&item).unwrap();
        let obj = value.as_object().unwrap();
        for key in [
            "id", "kind", "name", "command", "source", "location", "scope", "enabled",
            "trigger", "category", "risk", "reversible", "editable", "removable", "notes",
        ] {
            assert!(obj.contains_key(key), "eksik alan: {key}");
        }
        // Optional alanlar None iken atlanır.
        assert!(!obj.contains_key("last_run"));
        assert!(!obj.contains_key("impact"));
        // trigger null olabilir ve her zaman yazılır.
        assert!(obj.contains_key("trigger"));
        assert_eq!(obj["kind"], "startup");
        assert_eq!(obj["scope"], "user");
        assert_eq!(obj["source"], "registry-run");
        assert_eq!(obj["trigger"], "logon");
        assert_eq!(obj["category"], "third-party");
        assert_eq!(obj["risk"], "low");
        assert!(obj["reversible"].is_boolean());

        // Tüm kaynak yazımları sözleşmeye uyuyor.
        let expected = [
            "registry-run", "registry-runonce", "registry-run-policy", "startup-folder",
            "startup-folder-common", "winlogon", "logon-script", "xdg-autostart", "systemd-user",
            "systemd-system", "launchd-agent", "launchd-daemon", "cron", "shell-rc",
            "scheduled-task", "systemd-timer", "cron-system",
        ];
        let got: Vec<&str> = ALL_SOURCES.iter().map(|s| s.as_str()).collect();
        assert_eq!(got, expected);
        for (s, _) in expected.iter().zip(ALL_SOURCES) {
            assert_eq!(s.parse::<Source>().unwrap().as_str(), *s);
        }
    }

    #[test]
    fn trigger_is_null_when_not_applicable() {
        let mut item = synthetic(Kind::Startup, "X", Source::Cron, Scope::User, "crontab", true);
        item.trigger = None;
        let value = serde_json::to_value(&item).unwrap();
        assert!(value["trigger"].is_null());
    }

    #[test]
    fn time_formatting_is_rfc3339_and_stable() {
        let ts = now_rfc3339();
        assert_eq!(ts.len(), 20, "{ts}");
        assert!(ts.ends_with('Z'), "{ts}");
        assert_eq!(&ts[4..5], "-");
        assert_eq!(&ts[10..11], "T");
        let stamp = now_stamp();
        assert_eq!(stamp.len(), 16, "{stamp}");
        assert!(stamp.ends_with('Z'));
    }
}
