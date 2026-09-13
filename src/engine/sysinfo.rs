//! Sistem bilgisi ve sağlığı — CPU, bellek, disk, sıcaklık, SMART, uptime.
//!
//! Taşınabilir çekirdek `sysinfo` ile toplanır; platforma özel derinlik
//! (SMART sağlığı, sıcaklık) yerel araçlarla eklenir:
//!
//! - **Linux:** `smartctl -a` (SMART), `/sys/class/thermal/thermal_zone*/temp`
//!   ve `sensors` (sıcaklık).
//! - **Windows:** PowerShell CIM — `Get-PhysicalDisk` (SMART/HealthStatus),
//!   `MSAcpi_ThermalZoneTemperature` (sıcaklık).
//! - **macOS:** `system_profiler SPSmartStorageDataType` (SMART) ve
//!   `osx-cpu-temp` (sıcaklık, kuruluysa).
//!
//! Hiçbir adım hata döndürmez: bir ölçüm alınamazsa ilgili alan `None` kalır ve
//! sağlık özeti "bilinmiyor" olarak raporlanır. `sweep sysinfo --health`
//! yeşil/sarı/kırmızı özet verir.

use crate::core::report::{Entry, EntryKind, Report};
use serde::Serialize;

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "sysinfo";

/// Sıcaklık bu değerin üstündeyse "sarı" uyarı.
const THERMAL_WARN_C: f64 = 85.0;
/// Sıcaklık bu değerin üstündeyse "kırmızı".
const THERMAL_CRIT_C: f64 = 95.0;
/// Disk doluluk oranı bu yüzdenin üstündeyse "sarı".
const DISK_WARN_PCT: f64 = 90.0;
/// Disk doluluk oranı bu yüzdenin üstündeyse "kırmızı".
const DISK_CRIT_PCT: f64 = 97.0;
/// Bellek kullanımı bu yüzdenin üstündeyse "sarı".
const MEM_WARN_PCT: f64 = 90.0;
/// Bellek kullanımı bu yüzdenin üstündeyse "kırmızı".
const MEM_CRIT_PCT: f64 = 97.0;

/// Sağlık seviyesi (yeşil / sarı / kırmızı).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthLevel {
    Green,
    Yellow,
    Red,
}

impl HealthLevel {
    /// Kısa makine adı.
    pub fn as_str(self) -> &'static str {
        match self {
            HealthLevel::Green => "green",
            HealthLevel::Yellow => "yellow",
            HealthLevel::Red => "red",
        }
    }
}

/// SMART özet durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SmartStatus {
    Ok,
    Warning,
    Critical,
    Unknown,
}

impl SmartStatus {
    /// `smartctl` "SMART overall-health" satırından durum çıkar.
    ///
    /// PASSED → Ok, FAILED → Critical; başka bir şey (ör. "UNKNOWN!") → Warning.
    pub fn from_health_line(line: &str) -> Self {
        let lower = line.to_lowercase();
        if lower.contains("passed") {
            SmartStatus::Ok
        } else if lower.contains("failed") {
            SmartStatus::Critical
        } else if lower.contains("unknown") {
            SmartStatus::Warning
        } else {
            SmartStatus::Unknown
        }
    }

    /// Windows `HealthStatus` metnini eşle (Healthy / Warning / Unhealthy).
    pub fn from_windows_health(text: &str) -> Self {
        match text.trim().to_lowercase().as_str() {
            "healthy" | "ok" => SmartStatus::Ok,
            "warning" => SmartStatus::Warning,
            "unhealthy" | "critical" => SmartStatus::Critical,
            _ => SmartStatus::Unknown,
        }
    }

    /// macOS `SMARTStatus` metnini eşle.
    pub fn from_macos_status(text: &str) -> Self {
        match text.trim().to_lowercase().as_str() {
            "verified" => SmartStatus::Ok,
            "failing" => SmartStatus::Critical,
            "not supported" | "" => SmartStatus::Unknown,
            _ => SmartStatus::Warning,
        }
    }
}

/// Tek bir depolama aygıtının SMART özeti.
#[derive(Debug, Clone, Serialize)]
pub struct SmartInfo {
    pub device: String,
    pub status: SmartStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature_c: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power_on_hours: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reallocated_sectors: Option<u64>,
}

/// CPU künyesi.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CpuInfo {
    pub model: String,
    pub vendor: String,
    pub cores: usize,
    pub threads: usize,
    pub frequency_mhz: u64,
}

/// Bellek künyesi (bayt).
#[derive(Debug, Clone, Default, Serialize)]
pub struct MemoryInfo {
    pub total: u64,
    pub used: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

/// Bir bağlı dosya sistemi.
#[derive(Debug, Clone, Serialize)]
pub struct DiskInfo {
    pub name: String,
    pub mount: String,
    pub file_system: String,
    pub total: u64,
    pub free: u64,
}

/// Sağlık özeti: seviye + gerekçe satırları.
#[derive(Debug, Clone, Serialize)]
pub struct Health {
    pub level: HealthLevel,
    pub notes: Vec<String>,
}

/// Tam sistem künyesi (`sweep sysinfo --json` çıktısı).
#[derive(Debug, Clone, Serialize)]
pub struct SysInfo {
    pub os: String,
    pub os_version: String,
    pub kernel: String,
    pub arch: String,
    pub host: String,
    pub uptime_secs: u64,
    pub cpu: CpuInfo,
    pub memory: MemoryInfo,
    pub disks: Vec<DiskInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thermal_c: Option<f64>,
    pub smart: Vec<SmartInfo>,
    pub health: Health,
}

/// `sweep sysinfo` seçenekleri.
#[derive(Debug, Clone, Copy, Default)]
pub struct SysInfoOptions {
    /// Yalnız yeşil/sarı/kırmızı özet + gerekçeler (tam döküm yerine).
    pub health_only: bool,
}

/// Yüzde (0.0-100.0); bölen sıfırsa 0.
fn pct(part: u64, whole: u64) -> f64 {
    if whole == 0 {
        0.0
    } else {
        (part as f64 / whole as f64) * 100.0
    }
}

/// Uptime'i "3g 4s 12d" biçimine çevir.
pub fn human_uptime(secs: u64) -> String {
    let days = secs / 86_400;
    let hours = (secs % 86_400) / 3_600;
    let minutes = (secs % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {minutes}m")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}

// ---------------------------------------------------------------------------
// Saf ayrıştırıcılar (birim testleri bunları doğrular)
// ---------------------------------------------------------------------------

/// `smartctl -a` çıktısından genel sağlık satırını bul.
pub fn parse_smartctl_health(text: &str) -> SmartStatus {
    text.lines()
        .find(|line| {
            let lower = line.to_lowercase();
            lower.contains("overall-health") || lower.contains("smart health status")
        })
        .map(SmartStatus::from_health_line)
        .unwrap_or(SmartStatus::Unknown)
}

/// `smartctl -a` çıktısından sayısal bir attribute değeri çıkar.
///
/// Satır biçimi: `  5 Reallocated_Sector_Ct   0x0033   100   100   010    Pre-fail  Always  -       0`
/// İlk sayısal sütun ham değerdir; adı verilen attribute'un son sayısını alırız.
pub fn parse_smartctl_attr(text: &str, attr: &str) -> Option<u64> {
    for line in text.lines() {
        let lower = line.to_lowercase();
        if !lower.contains(&attr.to_lowercase()) {
            continue;
        }
        // Satırın sonundaki (ya da "-" sonrası) ilk tam sayı ham değerdir.
        let raw = line
            .rsplit(['-', ' '])
            .find(|token| !token.is_empty() && token.chars().all(|c| c.is_ascii_digit()))?;
        return raw.parse::<u64>().ok();
    }
    None
}

/// `smartctl` "Temperature_Celsius" ya da "Airflow_Temperature_Cel" satırı.
pub fn parse_smartctl_temperature(text: &str) -> Option<f64> {
    for attr in ["temperature_celsius", "airflow_temperature_cel"] {
        if let Some(v) = parse_smartctl_attr(text, attr) {
            // 0 ya da 65535 gibi anlamsız değerleri ele.
            if v > 0 && v < 200 {
                return Some(v as f64);
            }
        }
    }
    None
}

/// `sensors` çıktısından ilk makul sıcaklığı (Santigrat) çıkar.
///
/// Örnek: `Package id 0:  +45.0°C  (high = +80.0°C, crit = +100.0°C)`
pub fn parse_sensors_temp(text: &str) -> Option<f64> {
    for line in text.lines() {
        let Some(colon) = line.find(':') else {
            continue;
        };
        let head = &line[..colon];
        if !(head.contains("Package") || head.contains("Core") || head.contains("Tctl")) {
            continue;
        }
        let rest = &line[colon + 1..];
        // İlk `+NN.N°C` ya da `NN.N°C` parçasını al.
        let Some(pos) = rest.find("°C") else {
            continue;
        };
        let before = &rest[..pos];
        let number: String = before
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if let Ok(value) = number.parse::<f64>() {
            if value > 0.0 && value < 200.0 {
                return Some(value);
            }
        }
    }
    None
}

/// `/sys/class/thermal/thermal_zone*/temp` miliderece değerini Santigrat'a çevir.
pub fn parse_thermal_zone(raw: &str) -> Option<f64> {
    let milli: f64 = raw.trim().parse().ok()?;
    let celsius = milli / 1000.0;
    if celsius > 0.0 && celsius < 200.0 {
        Some(celsius)
    } else {
        None
    }
}

/// Windows `MSAcpi_ThermalZoneTemperature` değeri: Kelvin'in onda biri.
pub fn parse_windows_thermal(raw: &str) -> Option<f64> {
    let tenths_kelvin: f64 = raw.trim().parse().ok()?;
    let celsius = tenths_kelvin / 10.0 - 273.15;
    if celsius > 0.0 && celsius < 200.0 {
        Some(celsius)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Sağlık değerlendirmesi (saf fonksiyon)
// ---------------------------------------------------------------------------

/// Toplanan künyeden yeşil/sarı/kırmızı özet üret.
pub fn assess(info: &SysInfo) -> Health {
    let mut notes = Vec::new();
    let mut level = HealthLevel::Green;

    let mut bump = |candidate: HealthLevel, note: String| {
        notes.push(note);
        level = match (level, candidate) {
            (HealthLevel::Red, _) | (_, HealthLevel::Red) => HealthLevel::Red,
            (HealthLevel::Yellow, _) | (_, HealthLevel::Yellow) => HealthLevel::Yellow,
            _ => HealthLevel::Green,
        };
    };

    // Bellek
    let mem_pct = pct(info.memory.used, info.memory.total);
    if info.memory.total > 0 && mem_pct >= MEM_CRIT_PCT {
        bump(
            HealthLevel::Red,
            format!("memory critically full ({mem_pct:.0}%)"),
        );
    } else if info.memory.total > 0 && mem_pct >= MEM_WARN_PCT {
        bump(
            HealthLevel::Yellow,
            format!("memory pressure high ({mem_pct:.0}%)"),
        );
    }

    // Takas
    if info.memory.swap_total > 0 {
        let swap_pct = pct(info.memory.swap_used, info.memory.swap_total);
        if swap_pct >= MEM_WARN_PCT {
            bump(
                HealthLevel::Yellow,
                format!("swap heavily used ({swap_pct:.0}%)"),
            );
        }
    }

    // Diskler
    for disk in &info.disks {
        if disk.total == 0 {
            continue;
        }
        let used_pct = pct(disk.total.saturating_sub(disk.free), disk.total);
        if used_pct >= DISK_CRIT_PCT {
            bump(
                HealthLevel::Red,
                format!("{} is {used_pct:.0}% full", disk.mount),
            );
        } else if used_pct >= DISK_WARN_PCT {
            bump(
                HealthLevel::Yellow,
                format!("{} is {used_pct:.0}% full", disk.mount),
            );
        }
    }

    // SMART
    for smart in &info.smart {
        match smart.status {
            SmartStatus::Critical => bump(
                HealthLevel::Red,
                format!("SMART failure predicted on {}", smart.device),
            ),
            SmartStatus::Warning => bump(
                HealthLevel::Yellow,
                format!("SMART warning on {}", smart.device),
            ),
            _ => {}
        }
        if let Some(sectors) = smart.reallocated_sectors {
            if sectors > 0 {
                bump(
                    HealthLevel::Yellow,
                    format!("{} has {sectors} reallocated sectors", smart.device),
                );
            }
        }
    }

    // Sıcaklık
    if let Some(temp) = info.thermal_c {
        if temp >= THERMAL_CRIT_C {
            bump(
                HealthLevel::Red,
                format!("thermal throttling risk ({temp:.0}°C)"),
            );
        } else if temp >= THERMAL_WARN_C {
            bump(HealthLevel::Yellow, format!("running warm ({temp:.0}°C)"));
        }
    }

    // Uzun uptime: bilgilendirici, seviyeyi yükseltmez.
    if info.uptime_secs > 90 * 86_400 {
        notes.push(format!(
            "uptime {} — a reboot may help",
            human_uptime(info.uptime_secs)
        ));
    }

    Health { level, notes }
}

// ---------------------------------------------------------------------------
// Toplama
// ---------------------------------------------------------------------------

/// Tüm sistem künyesini topla. Hiçbir alan hata döndürmez.
pub fn collect() -> SysInfo {
    let mut system = sysinfo::System::new_all();
    system.refresh_all();

    let cpus = system.cpus();
    let cpu = CpuInfo {
        model: cpus
            .first()
            .map(|c| c.brand().trim().to_string())
            .unwrap_or_default(),
        vendor: cpus
            .first()
            .map(|c| c.vendor_id().trim().to_string())
            .unwrap_or_default(),
        cores: num_cpus::get_physical(),
        threads: cpus.len(),
        frequency_mhz: cpus.first().map(|c| c.frequency()).unwrap_or(0),
    };

    let memory = MemoryInfo {
        total: system.total_memory(),
        used: system.used_memory(),
        available: system.available_memory(),
        swap_total: system.total_swap(),
        swap_used: system.used_swap(),
    };

    let disks = sysinfo::Disks::new_with_refreshed_list()
        .list()
        .iter()
        .map(|disk| DiskInfo {
            name: disk.name().to_string_lossy().into_owned(),
            mount: disk.mount_point().display().to_string(),
            file_system: disk.file_system().to_string_lossy().into_owned(),
            total: disk.total_space(),
            free: disk.available_space(),
        })
        .collect();

    let mut info = SysInfo {
        os: sysinfo::System::name().unwrap_or_else(|| crate::platform::os_name().to_string()),
        os_version: sysinfo::System::os_version().unwrap_or_default(),
        kernel: sysinfo::System::kernel_version().unwrap_or_default(),
        arch: sysinfo::System::cpu_arch().unwrap_or_default(),
        host: sysinfo::System::host_name().unwrap_or_default(),
        uptime_secs: sysinfo::System::uptime(),
        cpu,
        memory,
        disks,
        thermal_c: None,
        smart: Vec::new(),
        health: Health {
            level: HealthLevel::Green,
            notes: Vec::new(),
        },
    };

    info.thermal_c = read_temperature();
    info.smart = read_smart();
    info.health = assess(&info);
    info
}

// ---------------------------------------------------------------------------
// Platforma özel: sıcaklık
// ---------------------------------------------------------------------------

/// İşlemci sıcaklığı (°C) — alınamazsa `None`.
fn read_temperature() -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        if let Some(temp) = linux_thermal_zone() {
            return Some(temp);
        }
        return crate::deep::command_output("sensors", &[])
            .and_then(|out| parse_sensors_temp(&out));
    }

    #[cfg(windows)]
    {
        let script = "(Get-CimInstance -Namespace root/wmi -ClassName \
                      MSAcpi_ThermalZoneTemperature -ErrorAction SilentlyContinue | \
                      Select-Object -First 1 -ExpandProperty CurrentTemperature)";
        return crate::deep::command_output(
            "powershell",
            &["-NoProfile", "-NonInteractive", "-Command", script],
        )
        .and_then(|out| parse_windows_thermal(&out));
    }

    #[cfg(target_os = "macos")]
    {
        return crate::deep::command_output("osx-cpu-temp", &[])
            .and_then(|out| parse_sensors_temp(&format!("Core: {out}")));
    }

    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    {
        None
    }
}

/// Linux: ilk makul `thermal_zone` değeri.
#[cfg(target_os = "linux")]
fn linux_thermal_zone() -> Option<f64> {
    for index in 0..8 {
        let path = format!("/sys/class/thermal/thermal_zone{index}/temp");
        if let Ok(raw) = std::fs::read_to_string(&path) {
            if let Some(temp) = parse_thermal_zone(&raw) {
                return Some(temp);
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Platforma özel: SMART
// ---------------------------------------------------------------------------

/// Aygıtların SMART özeti. Hiçbir araç yoksa boş liste.
fn read_smart() -> Vec<SmartInfo> {
    #[cfg(target_os = "linux")]
    {
        linux_smart()
    }
    #[cfg(windows)]
    {
        windows_smart()
    }
    #[cfg(target_os = "macos")]
    {
        macos_smart()
    }
    #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
    {
        Vec::new()
    }
}

/// Linux: `smartctl -a` ile her blok aygıtı tara.
#[cfg(target_os = "linux")]
fn linux_smart() -> Vec<SmartInfo> {
    if !crate::deep::have("smartctl") {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut devices: Vec<String> = Vec::new();
    for letter in b'a'..=b'z' {
        let path = format!("/dev/sd{}", letter as char);
        if std::path::Path::new(&path).exists() {
            devices.push(path);
        }
    }
    for index in 0..8 {
        let path = format!("/dev/nvme{index}n1");
        if std::path::Path::new(&path).exists() {
            devices.push(path);
        }
    }
    for device in devices {
        let Some(text) = crate::deep::command_output("smartctl", &["-a", &device]) else {
            continue;
        };
        out.push(SmartInfo {
            device,
            status: parse_smartctl_health(&text),
            temperature_c: parse_smartctl_temperature(&text),
            power_on_hours: parse_smartctl_attr(&text, "power_on_hours"),
            reallocated_sectors: parse_smartctl_attr(&text, "reallocated_sector_ct"),
        });
    }
    out
}

/// Windows: `Get-PhysicalDisk` → FriendlyName + HealthStatus.
#[cfg(windows)]
fn windows_smart() -> Vec<SmartInfo> {
    let script = "Get-PhysicalDisk | Select-Object FriendlyName,HealthStatus | \
                  ConvertTo-Json -Compress";
    let Some(raw) = crate::deep::command_output(
        "powershell",
        &["-NoProfile", "-NonInteractive", "-Command", script],
    ) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Vec::new();
    };
    // Tek disk varsa PowerShell nesne (dizi değil) döndürür.
    let items: Vec<&serde_json::Value> = match &value {
        serde_json::Value::Array(list) => list.iter().collect(),
        other => vec![other],
    };
    items
        .into_iter()
        .filter_map(|item| {
            let device = item.get("FriendlyName")?.as_str()?.to_string();
            let status = item
                .get("HealthStatus")
                .and_then(|v| v.as_str())
                .map(SmartStatus::from_windows_health)
                .unwrap_or(SmartStatus::Unknown);
            Some(SmartInfo {
                device,
                status,
                temperature_c: None,
                power_on_hours: None,
                reallocated_sectors: None,
            })
        })
        .collect()
}

/// macOS: `system_profiler SPSmartStorageDataType -json`.
#[cfg(target_os = "macos")]
fn macos_smart() -> Vec<SmartInfo> {
    let Some(raw) = crate::deep::command_output(
        "system_profiler",
        &["SPSmartStorageDataType", "-json"],
    ) else {
        return Vec::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return Vec::new();
    };
    let Some(list) = value.get("SPSmartStorageDataType").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|item| {
            let device = item
                .get("device_name")
                .or_else(|| item.get("_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("disk")
                .to_string();
            let status = item
                .get("SMARTStatus")
                .and_then(|v| v.as_str())
                .map(SmartStatus::from_macos_status)
                .unwrap_or(SmartStatus::Unknown);
            Some(SmartInfo {
                device,
                status,
                temperature_c: None,
                power_on_hours: None,
                reallocated_sectors: None,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Rapor
// ---------------------------------------------------------------------------

/// Künyeyi [`Report`]'a çevir; `health_only` ise yalnız özet + gerekçeler.
pub fn report(info: &SysInfo, opts: &SysInfoOptions) -> Report {
    let mut report = Report::new();

    report.push(Entry::new(
        EntryKind::Command,
        CLEANER_ID,
        "health",
        format!(
            "health: {} ({} note(s))",
            info.health.level.as_str().to_uppercase(),
            info.health.notes.len()
        ),
        None,
        0,
    ));
    for note in &info.health.notes {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "note",
            note.clone(),
            None,
            0,
        ));
    }

    if opts.health_only {
        return report;
    }

    report.push(Entry::new(
        EntryKind::Command,
        CLEANER_ID,
        "os",
        format!(
            "{} {} ({}) — {} {}",
            info.os, info.os_version, info.kernel, info.host, info.arch
        ),
        None,
        0,
    ));
    report.push(Entry::new(
        EntryKind::Command,
        CLEANER_ID,
        "uptime",
        format!("uptime {}", human_uptime(info.uptime_secs)),
        None,
        0,
    ));
    report.push(Entry::new(
        EntryKind::Command,
        CLEANER_ID,
        "cpu",
        format!(
            "{} — {} cores / {} threads @ {} MHz",
            info.cpu.model, info.cpu.cores, info.cpu.threads, info.cpu.frequency_mhz
        ),
        None,
        0,
    ));
    report.push(Entry::new(
        EntryKind::Command,
        CLEANER_ID,
        "memory",
        format!(
            "memory {:.0}% used ({}) — swap {:.0}% ({})",
            pct(info.memory.used, info.memory.total),
            crate::fsutil::size::bytes_to_human(info.memory.used, false),
            pct(info.memory.swap_used, info.memory.swap_total),
            crate::fsutil::size::bytes_to_human(info.memory.swap_used, false),
        ),
        None,
        0,
    ));
    for disk in &info.disks {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "disk",
            format!(
                "{} [{}] {} free of {} ({:.0}% used)",
                disk.mount,
                disk.file_system,
                crate::fsutil::size::bytes_to_human(disk.free, false),
                crate::fsutil::size::bytes_to_human(disk.total, false),
                pct(disk.total.saturating_sub(disk.free), disk.total),
            ),
            None,
            0,
        ));
    }
    if let Some(temp) = info.thermal_c {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "thermal",
            format!("temperature {temp:.1}°C"),
            None,
            0,
        ));
    } else {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "thermal",
            "temperature unavailable".to_string(),
            None,
            0,
        ));
    }
    for smart in &info.smart {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "smart",
            format!(
                "{} SMART: {}{}",
                smart.device,
                match smart.status {
                    SmartStatus::Ok => "OK",
                    SmartStatus::Warning => "WARNING",
                    SmartStatus::Critical => "CRITICAL",
                    SmartStatus::Unknown => "unknown",
                },
                smart
                    .temperature_c
                    .map(|t| format!(" ({t:.0}°C)"))
                    .unwrap_or_default()
            ),
            None,
            0,
        ));
    }
    if info.smart.is_empty() {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "smart",
            "no SMART data (install smartmontools / run elevated)".to_string(),
            None,
            0,
        ));
    }

    report
}

/// Künyeyi topla ve raporla.
pub fn run(opts: &SysInfoOptions) -> (SysInfo, Report) {
    let info = collect();
    let report = report(&info, opts);
    (info, report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_info() -> SysInfo {
        SysInfo {
            os: "Test".into(),
            os_version: "1.0".into(),
            kernel: "6.0".into(),
            arch: "x86_64".into(),
            host: "host".into(),
            uptime_secs: 3600,
            cpu: CpuInfo {
                model: "Test CPU".into(),
                vendor: "Test".into(),
                cores: 4,
                threads: 8,
                frequency_mhz: 3200,
            },
            memory: MemoryInfo {
                total: 16 * 1024 * 1024 * 1024,
                used: 4 * 1024 * 1024 * 1024,
                available: 12 * 1024 * 1024 * 1024,
                swap_total: 0,
                swap_used: 0,
            },
            disks: vec![DiskInfo {
                name: "sda".into(),
                mount: "/".into(),
                file_system: "ext4".into(),
                total: 100,
                free: 50,
            }],
            thermal_c: Some(45.0),
            smart: Vec::new(),
            health: Health {
                level: HealthLevel::Green,
                notes: Vec::new(),
            },
        }
    }

    #[test]
    fn smartctl_health_lines_are_classified() {
        assert_eq!(
            parse_smartctl_health("SMART overall-health self-assessment test result: PASSED"),
            SmartStatus::Ok
        );
        assert_eq!(
            parse_smartctl_health("SMART overall-health self-assessment test result: FAILED!"),
            SmartStatus::Critical
        );
        assert_eq!(
            parse_smartctl_health("SMART Health Status: UNKNOWN!"),
            SmartStatus::Warning
        );
        assert_eq!(parse_smartctl_health("no smart data here"), SmartStatus::Unknown);
    }

    #[test]
    fn smartctl_attributes_and_temperature_parse() {
        let text = "\
ID# ATTRIBUTE_NAME          FLAG     VALUE WORST THRESH TYPE      UPDATED  WHEN_FAILED RAW_VALUE
  5 Reallocated_Sector_Ct   0x0033   100   100   010    Pre-fail  Always       -       0
  9 Power_On_Hours          0x0032   090   090   000    Old_age   Always       -       12345
194 Temperature_Celsius     0x0022   060   045   000    Old_age   Always       -       40
";
        assert_eq!(parse_smartctl_attr(text, "reallocated_sector_ct"), Some(0));
        assert_eq!(parse_smartctl_attr(text, "power_on_hours"), Some(12345));
        assert_eq!(parse_smartctl_temperature(text), Some(40.0));
        assert_eq!(parse_smartctl_attr(text, "no_such_attr"), None);
    }

    #[test]
    fn sensors_and_thermal_zones_parse() {
        let sensors = "\
coretemp-isa-0000
Adapter: ISA adapter
Package id 0:  +45.0°C  (high = +80.0°C, crit = +100.0°C)
Core 0:        +44.0°C  (high = +80.0°C, crit = +100.0°C)
";
        assert_eq!(parse_sensors_temp(sensors), Some(45.0));
        // Sıcaklık satırı yoksa None.
        assert_eq!(parse_sensors_temp("fan1: 1200 RPM\n"), None);

        assert_eq!(parse_thermal_zone("45000\n"), Some(45.0));
        // Anlamsız (0) değer yok sayılır.
        assert_eq!(parse_thermal_zone("0\n"), None);
        assert_eq!(parse_thermal_zone("not-a-number"), None);

        // Kelvin'in onda biri: 3181 → 318.1 K → 44.95 °C.
        // Kayan nokta çıkarması tam temsil edilemez, bu yüzden toleranslı
        // karşılaştırılır (44.950000000000045 gibi bir sonuç beklenir).
        let win = parse_windows_thermal("3181").expect("3181 should parse");
        assert!((win - 44.95).abs() < 0.01, "got {win}");
        assert_eq!(parse_windows_thermal("0"), None);
    }

    #[test]
    fn windows_and_macos_statuses_map() {
        assert_eq!(SmartStatus::from_windows_health("Healthy"), SmartStatus::Ok);
        assert_eq!(SmartStatus::from_windows_health("Warning"), SmartStatus::Warning);
        assert_eq!(
            SmartStatus::from_windows_health("Unhealthy"),
            SmartStatus::Critical
        );
        assert_eq!(SmartStatus::from_windows_health("weird"), SmartStatus::Unknown);

        assert_eq!(SmartStatus::from_macos_status("Verified"), SmartStatus::Ok);
        assert_eq!(SmartStatus::from_macos_status("Failing"), SmartStatus::Critical);
        assert_eq!(
            SmartStatus::from_macos_status("Not Supported"),
            SmartStatus::Unknown
        );
    }

    #[test]
    fn healthy_system_is_green() {
        let health = assess(&base_info());
        assert_eq!(health.level, HealthLevel::Green);
        assert!(health.notes.is_empty());
    }

    #[test]
    fn full_disk_and_swap_raise_the_level() {
        let mut info = base_info();
        info.disks[0].free = 1; // %99 dolu
        info.memory.swap_total = 100;
        info.memory.swap_used = 95;
        let health = assess(&info);
        assert_eq!(health.level, HealthLevel::Red);
        assert!(health.notes.iter().any(|n| n.contains("full")));
        assert!(health.notes.iter().any(|n| n.contains("swap")));
    }

    #[test]
    fn smart_failure_forces_red() {
        let mut info = base_info();
        info.smart.push(SmartInfo {
            device: "/dev/sda".into(),
            status: SmartStatus::Critical,
            temperature_c: None,
            power_on_hours: None,
            reallocated_sectors: None,
        });
        assert_eq!(assess(&info).level, HealthLevel::Red);

        info.smart[0].status = SmartStatus::Warning;
        assert_eq!(assess(&info).level, HealthLevel::Yellow);
    }

    #[test]
    fn thermal_thresholds_are_applied() {
        let mut info = base_info();
        info.thermal_c = Some(88.0);
        assert_eq!(assess(&info).level, HealthLevel::Yellow);
        info.thermal_c = Some(99.0);
        assert_eq!(assess(&info).level, HealthLevel::Red);
    }

    #[test]
    fn report_health_only_hides_detail() {
        let info = base_info();
        let summary = report(&info, &SysInfoOptions { health_only: true });
        let full = report(&info, &SysInfoOptions { health_only: false });
        assert_eq!(summary.entries.len(), 1);
        assert!(full.entries.len() > summary.entries.len());
        assert!(summary.errors.is_empty());
        assert!(full.errors.is_empty());
    }

    #[test]
    fn uptime_is_human_readable() {
        assert_eq!(human_uptime(59), "0m");
        assert_eq!(human_uptime(3_600), "1h 0m");
        assert_eq!(human_uptime(90_000), "1d 1h 0m");
    }

    #[test]
    fn collect_never_panics_and_reports_health() {
        let info = collect();
        // Donanım ne olursa olsun sağlık seviyesi geçerli bir değer olmalı.
        assert!(matches!(
            info.health.level,
            HealthLevel::Green | HealthLevel::Yellow | HealthLevel::Red
        ));
        // En az bir mantıksal çekirdek ve bir iş parçacığı raporlanır.
        assert!(info.cpu.cores >= 1);
        assert!(info.cpu.threads >= 1);
    }
}
