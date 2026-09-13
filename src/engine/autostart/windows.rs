//! Windows arka ucu: kayıt defteri, Startup klasörleri, Winlogon, oturum açma
//! betikleri ve zamanlanmış görevler.
//!
//! Keşif PowerShell (`Get-ScheduledTask`, `[Environment]::GetFolderPath`) ve
//! `winreg` ile yapılır; değişiklikler `schtasks /change|/delete|/create` ile
//! uygulanır (yerelleştirmeden bağımsız, kabuk yorumlaması yok — argümanlar
//! doğrudan `Command`'a verilir).

use std::path::{Path, PathBuf};

use winreg::enums::*;
use winreg::types::FromRegValue;
use winreg::{RegKey, RegValue};

use super::backend::{Backend, Discovery};
use super::*;

/// Windows sistem arka ucu.
pub struct WindowsBackend;

impl WindowsBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn hive_key(hive: Hive) -> RegKey {
    match hive {
        Hive::Hkcu => RegKey::predef(HKEY_CURRENT_USER),
        Hive::Hklm => RegKey::predef(HKEY_LOCAL_MACHINE),
    }
}

/// İzlenecek kayıt defteri anahtarları: (kovan, alt anahtar, kaynak, kapsam).
fn run_subkeys() -> Vec<(Hive, String, Source, Scope)> {
    let run = r"Software\Microsoft\Windows\CurrentVersion\Run";
    let runonce = r"Software\Microsoft\Windows\CurrentVersion\RunOnce";
    let policy = r"Software\Microsoft\Windows\CurrentVersion\Policies\Explorer\Run";
    let wow = |tail: &str| format!(r"Software\Wow6432Node\{tail}");
    vec![
        (Hive::Hkcu, run.to_string(), Source::RegistryRun, Scope::User),
        (Hive::Hkcu, runonce.to_string(), Source::RegistryRunOnce, Scope::User),
        (Hive::Hklm, run.to_string(), Source::RegistryRun, Scope::System),
        (Hive::Hklm, runonce.to_string(), Source::RegistryRunOnce, Scope::System),
        (Hive::Hkcu, policy.to_string(), Source::RegistryRunPolicy, Scope::User),
        (Hive::Hklm, policy.to_string(), Source::RegistryRunPolicy, Scope::System),
        (Hive::Hklm, wow(r"Microsoft\Windows\CurrentVersion\Run"), Source::RegistryRun, Scope::System),
        (Hive::Hklm, wow(r"Microsoft\Windows\CurrentVersion\RunOnce"), Source::RegistryRunOnce, Scope::System),
        (Hive::Hklm, wow(r"Microsoft\Windows\CurrentVersion\Policies\Explorer\Run"), Source::RegistryRunPolicy, Scope::System),
    ]
}

/// Değeri görüntülenebilir metne çevir (REG_SZ, REG_EXPAND_SZ, REG_MULTI_SZ).
fn raw_to_display(raw: &RegValue) -> Option<String> {
    match raw.vtype {
        REG_SZ | REG_EXPAND_SZ => String::from_reg_value(raw).ok(),
        REG_MULTI_SZ => Vec::<String>::from_reg_value(raw)
            .ok()
            .map(|parts| parts.join("; ")),
        _ => None,
    }
}

/// Metni kayıt defteri baytlarına çevir (UTF-16LE, sonlandırıcı dahil).
fn string_to_reg_bytes(value: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(value.len() * 2 + 2);
    for unit in value.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes.extend_from_slice(&[0, 0]);
    bytes
}

fn string_reg_value(value: &str, vtype: RegType) -> RegValue {
    RegValue {
        bytes: string_to_reg_bytes(value),
        vtype,
    }
}

/// `RegType`i sayısal koda çevir (yedek JSON'ı için).
fn vtype_code(vtype: RegType) -> i64 {
    vtype as i64
}

/// Sayısal koddan `RegType`e dön.
fn vtype_from_code(code: i64) -> RegType {
    match code {
        1 => REG_SZ,
        2 => REG_EXPAND_SZ,
        3 => REG_BINARY,
        4 => REG_DWORD,
        5 => REG_DWORD_BIG_ENDIAN,
        7 => REG_MULTI_SZ,
        11 => REG_QWORD,
        _ => REG_SZ,
    }
}

fn ps(script: &str) -> Option<String> {
    for exe in ["powershell", "pwsh"] {
        if let Some(out) = crate::deep::command_output(
            exe,
            &[
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                script,
            ],
        ) {
            return Some(out);
        }
    }
    None
}

fn run_schtasks(args: &[&str]) -> Result<String, String> {
    match crate::fsutil::run_command("schtasks", args, true) {
        Ok((0, stdout, _)) => Ok(stdout),
        Ok((code, stdout, stderr)) => Err(format!(
            "schtasks exited {code}: {}",
            if stderr.trim().is_empty() {
                stdout.trim()
            } else {
                stderr.trim()
            }
        )),
        Err(err) => Err(format!("cannot run schtasks: {err}")),
    }
}

// ---------------------------------------------------------------------------
// Keşif
// ---------------------------------------------------------------------------

fn discover_registry() -> Vec<Item> {
    let mut out = Vec::new();
    for (hive, base, source, scope) in run_subkeys() {
        let key = hive_key(hive);
        // Etkin değerler.
        if let Ok(sub) = key.open_subkey_with_flags(&base, KEY_READ) {
            for value in sub.enum_values() {
                let Ok((name, raw)) = value else { continue };
                if name.is_empty() {
                    continue;
                }
                let Some(data) = raw_to_display(&raw) else { continue };
                let mut item = Item::new(
                    Kind::Startup,
                    name.clone(),
                    source,
                    format!("{}\\{}", hive.label(), base),
                    scope,
                    true,
                    Some(Trigger::Logon),
                    Handle::Registry {
                        hive,
                        base_subkey: base.clone(),
                        value: name.clone(),
                    },
                );
                item.command = Some(data);
                item.reclassify();
                out.push(item);
            }
        }
        // Kapatılmış (taşınmış) değerler — geri alınabilir.
        let disabled = format!("{base}\\sweep-disabled");
        if let Ok(sub) = key.open_subkey_with_flags(&disabled, KEY_READ) {
            for value in sub.enum_values() {
                let Ok((name, raw)) = value else { continue };
                if name.is_empty() {
                    continue;
                }
                let Some(data) = raw_to_display(&raw) else { continue };
                let mut item = Item::new(
                    Kind::Startup,
                    name.clone(),
                    source,
                    format!("{}\\{}", hive.label(), disabled),
                    scope,
                    false,
                    Some(Trigger::Logon),
                    Handle::Registry {
                        hive,
                        base_subkey: base.clone(),
                        value: name.clone(),
                    },
                );
                item.command = Some(data);
                item.reclassify();
                item.notes = Some(match item.notes.take() {
                    Some(note) => format!("{note}; disabled by sweep (sweep-disabled)"),
                    None => "disabled by sweep (sweep-disabled)".to_string(),
                });
                out.push(item);
            }
        }
    }
    out
}

/// `Startup` klasörleri; bilinen klasör API'siyle çözülür (sabit yol değil).
fn known_startup_dirs() -> Vec<(PathBuf, bool, Source)> {
    let mut out = Vec::new();
    let script = "[Console]::OutputEncoding=[Text.Encoding]::UTF8;\
        Write-Output ([Environment]::GetFolderPath('Startup'));\
        Write-Output ([Environment]::GetFolderPath('CommonStartup'))";
    let mut resolved: Vec<PathBuf> = Vec::new();
    if let Some(text) = ps(script) {
        resolved = text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(PathBuf::from)
            .collect();
    }
    // Yedek: ortam değişkenleri (bilinen klasör çözülemezse).
    if resolved.len() < 2 {
        let tail = r"Microsoft\Windows\Start Menu\Programs\Startup";
        if let Ok(appdata) = std::env::var("APPDATA") {
            if resolved.is_empty() {
                resolved.push(PathBuf::from(appdata).join(tail));
            }
        }
        if let Ok(program_data) = std::env::var("ProgramData") {
            if resolved.len() < 2 {
                resolved.push(PathBuf::from(program_data).join(tail));
            }
        }
    }
    for (i, path) in resolved.into_iter().take(2).enumerate() {
        if i == 0 {
            out.push((path, false, Source::StartupFolder));
        } else {
            out.push((path, true, Source::StartupFolderCommon));
        }
    }
    out
}

fn strip_disabled_suffix(name: &str) -> Option<&str> {
    const SUFFIX: &str = ".disabled";
    if name.len() > SUFFIX.len() && name.get(..name.len() - SUFFIX.len()).is_some() {
        let tail = &name[name.len() - SUFFIX.len()..];
        if tail.eq_ignore_ascii_case(SUFFIX) {
            return Some(&name[..name.len() - SUFFIX.len()]);
        }
    }
    None
}

/// Dosya adını (varsa `.disabled` ekini atarak) görünen ada çevir.
fn file_display_name(raw: &str) -> String {
    let base = strip_disabled_suffix(raw).unwrap_or(raw);
    Path::new(base)
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| base.to_string())
}

fn discover_startup_folders() -> Vec<Item> {
    let mut out = Vec::new();
    let mut seen: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    for (dir, system, source) in known_startup_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let raw = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if raw.eq_ignore_ascii_case("desktop.ini") {
                continue;
            }
            let enabled = strip_disabled_suffix(&raw).is_none();
            let enabled_name = strip_disabled_suffix(&raw).unwrap_or(&raw);
            // `location` etkin yolu taşır: enable/disable arasında kimlik sabit kalsın.
            let enabled_path = dir.join(enabled_name);
            if !seen.insert(enabled_path.clone()) {
                continue;
            }
            let mut item = Item::new(
                Kind::Startup,
                file_display_name(&raw),
                source,
                enabled_path.to_string_lossy().into_owned(),
                if system { Scope::System } else { Scope::User },
                enabled,
                Some(Trigger::Logon),
                Handle::File {
                    path: enabled_path.clone(),
                    hidden_flag: false,
                },
            );
            item.editable = false;
            item.notes = Some("startup shortcut (target not resolved)".to_string());
            out.push(item);
        }
    }
    out
}

fn discover_winlogon() -> Vec<Item> {
    let subkey = r"Software\Microsoft\Windows NT\CurrentVersion\Winlogon";
    let Ok(key) = hive_key(Hive::Hklm).open_subkey_with_flags(subkey, KEY_READ) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for name in ["Shell", "Userinit"] {
        let Ok(raw) = key.get_raw_value(name) else {
            continue;
        };
        let Some(data) = raw_to_display(&raw) else {
            continue;
        };
        let mut item = Item::new(
            Kind::Startup,
            format!("Winlogon: {name}"),
            Source::Winlogon,
            format!("HKLM\\{subkey}"),
            Scope::System,
            true,
            Some(Trigger::Logon),
            Handle::ReadOnly,
        );
        item.command = Some(data);
        item.reclassify();
        item.reversible = false;
        item.editable = false;
        item.removable = false;
        out.push(item);
    }
    out
}

/// `Policies\System\Scripts` / `Group Policy\Scripts` altındaki betikler.
fn collect_scripts(
    hive: Hive,
    root: &str,
    scope: Scope,
    stage: &str,
    trigger: Trigger,
    out: &mut Vec<Item>,
) {
    let Ok(key) = hive_key(hive).open_subkey_with_flags(root, KEY_READ) else {
        return;
    };
    // `Logon` / `Startup` aşaması.
    let Ok(stage_key) = key.open_subkey_with_flags(stage, KEY_READ) else {
        return;
    };
    for entry in stage_key.enum_keys() {
        let Ok(num) = entry else { continue };
        let Ok(script_key) = stage_key.open_subkey_with_flags(&num, KEY_READ) else {
            continue;
        };
        let script: String = script_key.get_value("Script").unwrap_or_default();
        let params: String = script_key.get_value("Parameters").unwrap_or_default();
        if script.trim().is_empty() {
            continue;
        }
        let command = if params.trim().is_empty() {
            script.clone()
        } else {
            format!("{script} {params}")
        };
        let mut item = Item::new(
            Kind::Startup,
            format!("{stage} script {num}"),
            Source::LogonScript,
            format!("{}\\{root}\\{stage}\\{num}", hive.label()),
            scope,
            true,
            Some(trigger),
            Handle::ReadOnly,
        );
        item.command = Some(command);
        item.reclassify();
        item.category = Category::Os;
        item.risk = Risk::High;
        item.reversible = false;
        item.editable = false;
        item.removable = false;
        item.notes = Some("logon script (policy) — managed by system policy".to_string());
        out.push(item);
    }
}

fn discover_logon_scripts() -> Vec<Item> {
    let mut out = Vec::new();
    collect_scripts(
        Hive::Hklm,
        r"Software\Microsoft\Windows\CurrentVersion\Policies\System\Scripts",
        Scope::System,
        "Logon",
        Trigger::Logon,
        &mut out,
    );
    collect_scripts(
        Hive::Hklm,
        r"Software\Microsoft\Windows\CurrentVersion\Policies\System\Scripts",
        Scope::System,
        "Startup",
        Trigger::Boot,
        &mut out,
    );
    collect_scripts(
        Hive::Hkcu,
        r"Software\Microsoft\Windows\CurrentVersion\Group Policy\Scripts",
        Scope::User,
        "Logon",
        Trigger::Logon,
        &mut out,
    );
    collect_scripts(
        Hive::Hkcu,
        r"Software\Microsoft\Windows\CurrentVersion\Group Policy\Scripts",
        Scope::User,
        "Startup",
        Trigger::Boot,
        &mut out,
    );
    out
}

fn discover_startup() -> Vec<Item> {
    let mut out = discover_registry();
    out.extend(discover_startup_folders());
    out.extend(discover_winlogon());
    out.extend(discover_logon_scripts());
    out
}

// --- zamanlanmış görevler --------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct PsTask {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    command: Option<String>,
    #[serde(default)]
    triggers: Option<String>,
    #[serde(default)]
    last_run: Option<String>,
    #[serde(default)]
    next_run: Option<String>,
    #[serde(default)]
    last_result: Option<String>,
    #[serde(default)]
    user: Option<String>,
}

const TASK_QUERY: &str = "[Console]::OutputEncoding=[Text.Encoding]::UTF8;\
$ErrorActionPreference='SilentlyContinue';\
$out = @(Get-ScheduledTask | ForEach-Object {\
  $t = $_; $i = $null; try { $i = $t | Get-ScheduledTaskInfo } catch {};\
  $lr = $null; if ($i -and $i.LastRunTime) { $lr = $i.LastRunTime.ToString('o') };\
  $nr = $null; if ($i -and $i.NextRunTime) { $nr = $i.NextRunTime.ToString('o') };\
  $res = $null; if ($i) { $res = [string]$i.LastTaskResult };\
  [pscustomobject]@{\
    name=$t.TaskName; path=$t.TaskPath; state=[string]$t.State; author=$t.Author;\
    command=(($t.Actions | ForEach-Object { (\"$($_.Execute) $($_.Arguments)\").Trim() }) -join '; ');\
    triggers=(($t.Triggers | ForEach-Object { $_.CimClass.CimClassName }) -join ',');\
    last_run=$lr; next_run=$nr; last_result=$res;\
    user=[string]$t.Principal.UserId\
  }\
});\
ConvertTo-Json -InputObject $out -Depth 3 -Compress";

fn parse_task_json(text: &str) -> Vec<PsTask> {
    let value: serde_json::Value = match serde_json::from_str(text.trim()) {
        Ok(v) => v,
        Err(err) => {
            log::debug!("scheduled task JSON parse failed: {err}");
            return Vec::new();
        }
    };
    match value {
        serde_json::Value::Array(_) => serde_json::from_value(value).unwrap_or_default(),
        serde_json::Value::Object(_) => serde_json::from_value::<PsTask>(value)
            .map(|t| vec![t])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn trigger_of(triggers: &str) -> Trigger {
    let t = triggers.to_lowercase();
    if t.contains("logontrigger") {
        Trigger::Logon
    } else if t.contains("boottrigger") {
        Trigger::Boot
    } else if t.contains("idletrigger") {
        Trigger::Idle
    } else if t.contains("eventtrigger")
        || t.contains("registrationtrigger")
        || t.contains("wnfstatechangetrigger")
        || t.contains("sessionstatechangetrigger")
    {
        Trigger::Event
    } else if t.contains("timetrigger")
        || t.contains("dailytrigger")
        || t.contains("weeklytrigger")
        || t.contains("monthlytrigger")
    {
        Trigger::Schedule
    } else {
        Trigger::Unknown
    }
}

// --- görev XML yedeği -------------------------------------------------------
//
// `Get-ScheduledTask` bazı tetikleyicileri (ör. `<WnfStateChangeTrigger>`) taban
// `MSFT_TaskTrigger` sınıfı olarak döner ve COM işleyicili görevlerde komut
// satırı boştur. `System32\Tasks` altındaki ham XML yerelleştirmeden bağımsız
// olduğu için tetikleyici/komut oradan tamamlanır (salt okunur).

fn read_task_xml(task_path: &str, task_name: &str) -> Option<String> {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let dir = task_path.trim_start_matches(['\\', '/']).replace('/', "\\");
    let file = PathBuf::from(root)
        .join("System32")
        .join("Tasks")
        .join(dir)
        .join(task_name);
    let bytes = std::fs::read(&file).ok()?;
    // Görev dosyaları UTF-16LE'dir (BOM'lu ya da düz).
    let text = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else if bytes.iter().skip(1).step_by(2).filter(|b| **b == 0).count() > bytes.len() / 4 {
        // BOM'suz UTF-16LE sezgisi.
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        String::from_utf8_lossy(&bytes).into_owned()
    };
    Some(text)
}

fn xml_fields(xml: &str, tag: &str) -> Vec<String> {
    let pattern = format!(r"(?is)<{tag}(?:\s[^>]*)?>(.*?)</{tag}>");
    let Ok(re) = regex::Regex::new(&pattern) else {
        return Vec::new();
    };
    re.captures_iter(xml)
        .map(|c| {
            let raw = c[1].trim();
            let raw = raw.strip_prefix("<![CDATA[").unwrap_or(raw);
            let raw = raw.strip_suffix("]]>").unwrap_or(raw);
            raw.trim().to_string()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

fn trigger_from_xml(xml: &str) -> Option<Trigger> {
    let lower = xml.to_lowercase();
    if lower.contains("<logontrigger") {
        Some(Trigger::Logon)
    } else if lower.contains("<boottrigger") {
        Some(Trigger::Boot)
    } else if lower.contains("<idletrigger") {
        Some(Trigger::Idle)
    } else if lower.contains("<timetrigger") || lower.contains("<calendartrigger") {
        Some(Trigger::Schedule)
    } else if lower.contains("<eventtrigger")
        || lower.contains("<wnfstatechangetrigger")
        || lower.contains("<sessionstatechangetrigger")
        || lower.contains("<registrationtrigger")
    {
        Some(Trigger::Event)
    } else {
        None
    }
}

fn command_from_xml(xml: &str) -> Option<String> {
    // Her `<Exec>` bloğunu ayrı ele al: Command ve Arguments aynı eyleme aittir.
    let mut out = Vec::new();
    if let Ok(exec) = regex::Regex::new(r"(?is)<Exec(?:\s[^>]*)?>(.*?)</Exec>") {
        for cap in exec.captures_iter(xml) {
            let block = &cap[1];
            if let Some(command) = xml_fields(block, "Command").into_iter().next() {
                let args = xml_fields(block, "Arguments")
                    .into_iter()
                    .next()
                    .unwrap_or_default();
                out.push(if args.is_empty() {
                    command
                } else {
                    format!("{command} {args}")
                });
            }
        }
    }
    if !out.is_empty() {
        return Some(out.join("; "));
    }
    if let Ok(com) = regex::Regex::new(r"(?is)<ComHandler(?:\s[^>]*)?>(.*?)</ComHandler>") {
        for cap in com.captures_iter(xml) {
            let block = &cap[1];
            if let Some(class) = xml_fields(block, "ClassId").into_iter().next() {
                let data = xml_fields(block, "Data")
                    .into_iter()
                    .next()
                    .unwrap_or_default();
                out.push(if data.is_empty() {
                    format!("COM handler {class}")
                } else {
                    format!("COM handler {class} {data}")
                });
            }
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out.join("; "))
    }
}

/// Epoch dışı / "hiç çalışmadı" zamanlarını ele.
fn usable_time(raw: &Option<String>) -> Option<String> {
    let text = raw.as_deref()?.trim();
    if text.is_empty() || text.starts_with("0001") {
        return None;
    }
    let year: i32 = text.get(..4).and_then(|y| y.parse().ok()).unwrap_or(0);
    if year < 2000 {
        None
    } else {
        Some(text.to_string())
    }
}

fn task_scope(path: &str, user: &str, protected: bool) -> Scope {
    let u = user.to_uppercase();
    let system_account = u.contains("SYSTEM")
        || u.contains("LOCAL SERVICE")
        || u.contains("NETWORK SERVICE")
        || u.contains("S-1-5-18")
        || u.contains("S-1-5-19")
        || u.contains("S-1-5-20");
    if protected || system_account || path.starts_with("\\Microsoft\\") {
        Scope::System
    } else {
        Scope::User
    }
}

fn discover_tasks() -> Vec<Item> {
    let Some(text) = ps(TASK_QUERY) else {
        log::debug!("scheduled task query unavailable (Get-ScheduledTask)");
        return Vec::new();
    };
    let mut out = Vec::new();
    for task in parse_task_json(&text) {
        let name = match task.name {
            Some(n) if !n.trim().is_empty() => n,
            _ => continue,
        };
        let path = task.path.unwrap_or_else(|| "\\".to_string());
        let full_path = format!("{path}{name}");
        let command = task
            .command
            .as_deref()
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty());
        let protected = full_path.to_lowercase().starts_with("\\microsoft\\")
            || task
                .author
                .as_deref()
                .map(|a| a.to_lowercase().contains("microsoft"))
                .unwrap_or(false);
        let mut trigger = task
            .triggers
            .as_deref()
            .map(trigger_of)
            .unwrap_or(Trigger::Unknown);
        let mut command = command;
        // CIM taban sınıfı ya da boş komut: ham görev XML'inden tamamla.
        if trigger == Trigger::Unknown || command.is_none() {
            if let Some(xml) = read_task_xml(&path, &name) {
                if trigger == Trigger::Unknown {
                    trigger = trigger_from_xml(&xml).unwrap_or(Trigger::Unknown);
                }
                if command.is_none() {
                    command = command_from_xml(&xml);
                }
            }
        }
        let scope = task_scope(&path, task.user.as_deref().unwrap_or(""), protected);
        let enabled = !task
            .state
            .as_deref()
            .map(|s| s.eq_ignore_ascii_case("disabled"))
            .unwrap_or(false);
        let mut item = Item::new(
            Kind::Task,
            name.clone(),
            Source::ScheduledTask,
            full_path.clone(),
            scope,
            enabled,
            Some(trigger),
            Handle::Task {
                task_path: path.clone(),
                task_name: name.clone(),
                protected,
            },
        );
        item.command = command;
        item.reclassify();
        item.last_run = usable_time(&task.last_run);
        item.next_run = usable_time(&task.next_run);
        item.last_result = task.last_result.filter(|r| !r.trim().is_empty());
        item.author = task.author.filter(|a| !a.trim().is_empty());
        item.removable = !protected && scope == Scope::User;
        item.editable = !protected;
        if protected {
            item.notes = Some("protected OS task (\\Microsoft\\ or Microsoft-signed)".to_string());
        }
        out.push(item);
    }
    out
}

// ---------------------------------------------------------------------------
// Yakalama / geri kurma yardımcıları
// ---------------------------------------------------------------------------

fn registry_locations(item: &Item) -> Result<(Hive, String, String), String> {
    match &item.handle {
        Handle::Registry {
            hive,
            base_subkey,
            value,
        } => Ok((*hive, base_subkey.clone(), value.clone())),
        _ => Err("not a registry entry".to_string()),
    }
}

fn task_ids(item: &Item) -> Result<(String, String, bool), String> {
    match &item.handle {
        Handle::Task {
            task_path,
            task_name,
            protected,
        } => Ok((task_path.clone(), task_name.clone(), *protected)),
        _ => Err("not a scheduled task".to_string()),
    }
}

fn disabled_subkey(base: &str) -> String {
    format!("{base}\\sweep-disabled")
}

fn registry_state(hive: Hive, base: &str, value: &str) -> Result<String, String> {
    let key = hive_key(hive);
    if let Ok(sub) = key.open_subkey_with_flags(base, KEY_READ) {
        if sub.get_raw_value(value).is_ok() {
            return Ok("enabled".to_string());
        }
    }
    let disabled = disabled_subkey(base);
    if let Ok(sub) = key.open_subkey_with_flags(&disabled, KEY_READ) {
        if sub.get_raw_value(value).is_ok() {
            return Ok("disabled".to_string());
        }
    }
    Ok("absent".to_string())
}

/// Kayıt değerini konumlar arasında taşı (silmez, `sweep-disabled`e taşır).
fn registry_move(hive: Hive, base: &str, value: &str, to_enabled: bool) -> Result<(), String> {
    let key = hive_key(hive);
    let disabled = disabled_subkey(base);
    let (src_sub, dst_sub) = if to_enabled {
        (disabled.clone(), base.to_string())
    } else {
        (base.to_string(), disabled.clone())
    };
    let src = key
        .open_subkey_with_flags(&src_sub, KEY_READ | KEY_WRITE)
        .map_err(|e| format!("cannot open {src_sub}: {e}"))?;
    let raw = src
        .get_raw_value(value)
        .map_err(|e| format!("cannot read value '{value}': {e}"))?;
    let dst = key
        .open_subkey_with_flags(&dst_sub, KEY_READ | KEY_WRITE)
        .or_else(|_| key.create_subkey(&dst_sub).map(|(k, _)| k))
        .map_err(|e| format!("cannot open {dst_sub} (needs administrator): {e}"))?;
    dst.set_raw_value(value, &raw)
        .map_err(|e| format!("cannot write value (needs administrator): {e}"))?;
    src.delete_value(value)
        .map_err(|e| format!("cannot remove old value: {e}"))?;
    Ok(())
}

fn file_state(path: &Path) -> String {
    if path.is_file() {
        return "enabled".to_string();
    }
    let disabled = PathBuf::from(format!("{}.disabled", path.display()));
    if disabled.is_file() {
        return "disabled".to_string();
    }
    "absent".to_string()
}

fn rename_to(path: &Path, disabled: bool) -> Result<(), String> {
    if disabled {
        let dest = PathBuf::from(format!("{}.disabled", path.display()));
        std::fs::rename(path, &dest).map_err(|e| format!("cannot disable {}: {e}", path.display()))
    } else {
        let dest = PathBuf::from(path.to_string_lossy().trim_end_matches(".disabled"));
        std::fs::rename(path, &dest).map_err(|e| format!("cannot enable {}: {e}", path.display()))
    }
}

impl Backend for WindowsBackend {
    fn discover(&self, what: Discovery) -> Vec<Item> {
        let mut out = Vec::new();
        if matches!(what, Discovery::Startup | Discovery::All) {
            out.extend(discover_startup());
        }
        if matches!(what, Discovery::Tasks | Discovery::All) {
            out.extend(discover_tasks());
        }
        out
    }

    fn capture(&self, item: &Item) -> Result<serde_json::Value, String> {
        match &item.handle {
            Handle::Registry { .. } => {
                let (hive, base, value) = registry_locations(item)?;
                let state = registry_state(hive, &base, &value)?;
                let key = hive_key(hive);
                let sub_name = if state == "disabled" {
                    disabled_subkey(&base)
                } else {
                    base.clone()
                };
                let sub = key
                    .open_subkey_with_flags(&sub_name, KEY_READ)
                    .map_err(|e| format!("cannot open {sub_name}: {e}"))?;
                let raw = sub
                    .get_raw_value(&value)
                    .map_err(|e| format!("cannot read value '{value}': {e}"))?;
                let display = raw_to_display(&raw).unwrap_or_default();
                Ok(serde_json::json!({
                    "kind": "registry",
                    "hive": hive.label(),
                    "subkey": base,
                    "value": value,
                    "enabled": state == "enabled",
                    "vtype": vtype_code(raw.vtype),
                    "bytes": raw.bytes,
                    "data": display,
                }))
            }
            Handle::File { path, hidden_flag } => {
                let state = file_state(path);
                let actual = if state == "disabled" {
                    PathBuf::from(format!("{}.disabled", path.display()))
                } else {
                    path.clone()
                };
                let bytes = std::fs::read(&actual).unwrap_or_default();
                Ok(serde_json::json!({
                    "kind": "file",
                    "path": path.to_string_lossy(),
                    "enabled": state == "enabled",
                    "hidden": hidden_flag,
                    "bytes": bytes,
                }))
            }
            Handle::Task {
                task_path,
                task_name,
                ..
            } => {
                std::env::set_var("SWEEP_TASK_NAME", task_name);
                std::env::set_var("SWEEP_TASK_PATH", task_path);
                let xml = ps("[Console]::OutputEncoding=[Text.Encoding]::UTF8;\
                    Export-ScheduledTask -TaskName $env:SWEEP_TASK_NAME -TaskPath $env:SWEEP_TASK_PATH")
                    .unwrap_or_default();
                let state = self.state_of(item)?;
                Ok(serde_json::json!({
                    "kind": "task",
                    "task_path": task_path,
                    "task_name": task_name,
                    "enabled": state == "enabled",
                    "xml": xml.trim(),
                }))
            }
            Handle::ReadOnly => Err("entry is read-only and cannot be backed up".to_string()),
            Handle::Systemd { .. } => Err("systemd unit is read-only".to_string()),
            Handle::Fake(_) => Err("fake handle on windows backend".to_string()),
        }
    }

    fn set_enabled(&self, item: &Item, enabled: bool) -> Result<(), String> {
        if item.enabled == enabled {
            return Ok(());
        }
        match &item.handle {
            Handle::Registry { .. } => {
                let (hive, base, value) = registry_locations(item)?;
                registry_move(hive, &base, &value, enabled)
            }
            Handle::File { path, .. } => {
                // `path` is the logical (enabled) name. While the item is
                // disabled the file on disk is `<path>.disabled`, so enabling
                // must rename *that* file back - renaming the logical name
                // fails with ENOENT because it does not exist yet.
                if enabled {
                    let src = PathBuf::from(format!("{}.disabled", path.display()));
                    if src.is_file() {
                        std::fs::rename(&src, path)
                            .map_err(|e| format!("cannot enable {}: {e}", path.display()))
                    } else {
                        rename_to(path, false)
                    }
                } else {
                    rename_to(path, true)
                }
            }
            Handle::Task { .. } => {
                let (path, name, _) = task_ids(item)?;
                let full = format!("{path}{name}");
                let flag = if enabled { "/enable" } else { "/disable" };
                run_schtasks(&["/change", "/tn", full.as_str(), flag])?;
                Ok(())
            }
            _ => Err("entry is not reversible".to_string()),
        }
    }

    fn edit(&self, item: &Item, command: &str) -> Result<(), String> {
        match &item.handle {
            Handle::Registry { .. } => {
                let (hive, base, value) = registry_locations(item)?;
                let state = registry_state(hive, &base, &value)?;
                if state == "absent" {
                    return Err("registry value is absent".to_string());
                }
                let sub_name = if state == "disabled" {
                    disabled_subkey(&base)
                } else {
                    base.clone()
                };
                let key = hive_key(hive);
                let sub = key
                    .open_subkey_with_flags(&sub_name, KEY_READ | KEY_WRITE)
                    .map_err(|e| format!("cannot open {sub_name} (needs administrator): {e}"))?;
                // VARSAYILAN türü koru (REG_EXPAND_SZ → genişletme semantiği).
                let vtype = sub
                    .get_raw_value(&value)
                    .map(|raw| raw.vtype)
                    .unwrap_or(REG_SZ);
                let vtype = if vtype == REG_EXPAND_SZ { REG_EXPAND_SZ } else { REG_SZ };
                sub.set_raw_value(&value, &string_reg_value(command, vtype))
                    .map_err(|e| format!("cannot write value (needs administrator): {e}"))?;
                Ok(())
            }
            Handle::Task { .. } => {
                let (path, name, protected) = task_ids(item)?;
                if protected {
                    return Err("protected OS task cannot be edited".to_string());
                }
                let full = format!("{path}{name}");
                run_schtasks(&["/change", "/tn", full.as_str(), "/tr", command])?;
                Ok(())
            }
            _ => Err("entry is not editable".to_string()),
        }
    }

    fn remove(&self, item: &Item) -> Result<(), String> {
        match &item.handle {
            Handle::Registry { .. } => {
                let (hive, base, value) = registry_locations(item)?;
                let state = registry_state(hive, &base, &value)?;
                let sub_name = if state == "disabled" {
                    disabled_subkey(&base)
                } else {
                    base.clone()
                };
                let key = hive_key(hive);
                let sub = key
                    .open_subkey_with_flags(&sub_name, KEY_READ | KEY_WRITE)
                    .map_err(|e| format!("cannot open {sub_name} (needs administrator): {e}"))?;
                sub.delete_value(&value)
                    .map_err(|e| format!("cannot delete value (needs administrator): {e}"))?;
                Ok(())
            }
            Handle::File { path, .. } => {
                let state = file_state(path);
                let actual = if state == "disabled" {
                    PathBuf::from(format!("{}.disabled", path.display()))
                } else {
                    path.clone()
                };
                std::fs::remove_file(&actual)
                    .map_err(|e| format!("cannot remove {}: {e}", actual.display()))
            }
            Handle::Task { .. } => {
                let (path, name, protected) = task_ids(item)?;
                if protected {
                    return Err("protected OS task cannot be removed".to_string());
                }
                let full = format!("{path}{name}");
                run_schtasks(&["/delete", "/tn", full.as_str(), "/f"])?;
                Ok(())
            }
            _ => Err("entry is not removable".to_string()),
        }
    }

    fn restore(&self, payload: &serde_json::Value) -> Result<(), String> {
        let kind = payload
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or_default();
        match kind {
            "registry" => {
                let hive = match payload.get("hive").and_then(|h| h.as_str()) {
                    Some("HKLM") => Hive::Hklm,
                    _ => Hive::Hkcu,
                };
                let base = payload
                    .get("subkey")
                    .and_then(|s| s.as_str())
                    .ok_or("registry backup missing subkey")?;
                let value = payload
                    .get("value")
                    .and_then(|s| s.as_str())
                    .ok_or("registry backup missing value")?;
                let enabled = payload
                    .get("enabled")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(true);
                let bytes: Vec<u8> = payload
                    .get("bytes")
                    .and_then(|b| serde_json::from_value(b.clone()).ok())
                    .unwrap_or_default();
                let vtype = vtype_from_code(
                    payload
                        .get("vtype")
                        .and_then(|v| v.as_i64())
                        .unwrap_or(1),
                );
                let key = hive_key(hive);
                let target = if enabled { base.to_string() } else { disabled_subkey(base) };
                let other = if enabled { disabled_subkey(base) } else { base.to_string() };
                let sub = key
                    .open_subkey_with_flags(&target, KEY_READ | KEY_WRITE)
                    .or_else(|_| key.create_subkey(&target).map(|(k, _)| k))
                    .map_err(|e| format!("cannot open {target} (needs administrator): {e}"))?;
                sub.set_raw_value(value, &RegValue { bytes, vtype })
                    .map_err(|e| format!("cannot restore value (needs administrator): {e}"))?;
                if let Ok(prev) = key.open_subkey_with_flags(&other, KEY_READ | KEY_WRITE) {
                    let _ = prev.delete_value(value);
                }
                Ok(())
            }
            "file" => {
                let path = PathBuf::from(
                    payload
                        .get("path")
                        .and_then(|p| p.as_str())
                        .ok_or("file backup missing path")?,
                );
                let enabled = payload
                    .get("enabled")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(true);
                let bytes: Vec<u8> = payload
                    .get("bytes")
                    .and_then(|b| serde_json::from_value(b.clone()).ok())
                    .unwrap_or_default();
                let disabled_path = PathBuf::from(format!("{}.disabled", path.display()));
                // İki konumdan da temizle, sonra hedefe yaz.
                let _ = std::fs::remove_file(&path);
                let _ = std::fs::remove_file(&disabled_path);
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let dest = if enabled { &path } else { &disabled_path };
                std::fs::write(dest, &bytes)
                    .map_err(|e| format!("cannot restore {}: {e}", dest.display()))
            }
            "task" => {
                let task_path = payload
                    .get("task_path")
                    .and_then(|p| p.as_str())
                    .unwrap_or("\\");
                let task_name = payload
                    .get("task_name")
                    .and_then(|p| p.as_str())
                    .ok_or("task backup missing name")?;
                let enabled = payload
                    .get("enabled")
                    .and_then(|e| e.as_bool())
                    .unwrap_or(true);
                let xml = payload
                    .get("xml")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default();
                let full = format!("{task_path}{task_name}");
                // Görev hâlâ varsa yalnız durumu düzelt.
                let exists = self.task_state(task_path, task_name)? != "absent";
                if exists {
                    let flag = if enabled { "/enable" } else { "/disable" };
                    run_schtasks(&["/change", "/tn", full.as_str(), flag])?;
                    return Ok(());
                }
                if xml.trim().is_empty() {
                    return Err("task is absent and no XML backup is available".to_string());
                }
                let tmp = std::env::temp_dir().join(format!(
                    "sweep-task-{}.xml",
                    super::short_hash(task_name)
                ));
                std::fs::write(&tmp, xml)
                    .map_err(|e| format!("cannot write task XML {}: {e}", tmp.display()))?;
                let tmp_text = tmp.to_string_lossy().into_owned();
                let result = run_schtasks(&[
                    "/create",
                    "/tn",
                    full.as_str(),
                    "/xml",
                    tmp_text.as_str(),
                    "/f",
                ]);
                let _ = std::fs::remove_file(&tmp);
                result?;
                if !enabled {
                    run_schtasks(&["/change", "/tn", full.as_str(), "/disable"])?;
                }
                Ok(())
            }
            other => Err(format!("unknown backup kind: {other}")),
        }
    }

    fn state_of(&self, item: &Item) -> Result<String, String> {
        match &item.handle {
            Handle::Registry { .. } => {
                let (hive, base, value) = registry_locations(item)?;
                registry_state(hive, &base, &value)
            }
            Handle::File { path, .. } => Ok(file_state(path)),
            Handle::Task { .. } => {
                let (path, name, _) = task_ids(item)?;
                self.task_state(&path, &name)
            }
            _ => Err("entry has no readable state".to_string()),
        }
    }
}

impl WindowsBackend {
    fn task_state(&self, task_path: &str, task_name: &str) -> Result<String, String> {
        std::env::set_var("SWEEP_TASK_NAME", task_name);
        std::env::set_var("SWEEP_TASK_PATH", task_path);
        let script = "[Console]::OutputEncoding=[Text.Encoding]::UTF8;\
            $t = Get-ScheduledTask -TaskName $env:SWEEP_TASK_NAME -TaskPath $env:SWEEP_TASK_PATH -ErrorAction SilentlyContinue;\
            if ($t) { [string]$t.State } else { 'ABSENT' }";
        let Some(out) = ps(script) else {
            return Err("cannot query task state (Get-ScheduledTask unavailable)".to_string());
        };
        let text = out.trim();
        if text.is_empty() || text.eq_ignore_ascii_case("ABSENT") {
            Ok("absent".to_string())
        } else if text.eq_ignore_ascii_case("Disabled") {
            Ok("disabled".to_string())
        } else {
            Ok("enabled".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_mapping_covers_the_class_names() {
        assert_eq!(trigger_of("MSFT_TaskLogonTrigger"), Trigger::Logon);
        assert_eq!(trigger_of("MSFT_TaskBootTrigger"), Trigger::Boot);
        assert_eq!(trigger_of("MSFT_TaskDailyTrigger"), Trigger::Schedule);
        assert_eq!(trigger_of("MSFT_TaskTimeTrigger"), Trigger::Schedule);
        assert_eq!(trigger_of("MSFT_TaskEventTrigger"), Trigger::Event);
        assert_eq!(trigger_of("MSFT_TaskIdleTrigger"), Trigger::Idle);
        assert_eq!(trigger_of("MSFT_TaskSessionStateChangeTrigger"), Trigger::Event);
        assert_eq!(trigger_of("MSFT_TaskRegistrationTrigger"), Trigger::Event);
        assert_eq!(trigger_of(""), Trigger::Unknown);
    }

    #[test]
    fn task_xml_fallback_recovers_trigger_and_command() {
        let xml = r#"<?xml version="1.0"?>
<Task><Triggers><WnfStateChangeTrigger><Enabled>true</Enabled></WnfStateChangeTrigger></Triggers>
<Actions Context="Author"><Exec><Command>C:\Windows\System32\foo.exe</Command><Arguments>--run</Arguments></Exec></Actions></Task>"#;
        assert_eq!(trigger_from_xml(xml), Some(Trigger::Event));
        assert_eq!(
            command_from_xml(xml).as_deref(),
            Some("C:\\Windows\\System32\\foo.exe --run")
        );

        // Logon + birden çok Exec eylemi.
        let multi = r#"<Task><Triggers><LogonTrigger/></Triggers>
<Actions><Exec><Command>a.exe</Command></Exec><Exec><Command>b.exe</Command><Arguments>-x</Arguments></Exec></Actions></Task>"#;
        assert_eq!(trigger_from_xml(multi), Some(Trigger::Logon));
        assert_eq!(command_from_xml(multi).as_deref(), Some("a.exe; b.exe -x"));

        // COM işleyicili görev: gerçek ClassId/Data (CDATA sarılı).
        let com = r#"<Task><Triggers><TimeTrigger/></Triggers>
<Actions><ComHandler><ClassId>{84F0FAE1-C27B-4F6F-807B-28CF6F96287D}</ClassId><Data><![CDATA[/RuntimeWide]]></Data></ComHandler></Actions></Task>"#;
        assert_eq!(trigger_from_xml(com), Some(Trigger::Schedule));
        assert_eq!(
            command_from_xml(com).as_deref(),
            Some("COM handler {84F0FAE1-C27B-4F6F-807B-28CF6F96287D} /RuntimeWide")
        );

        // Tetikleyici yok → None (unknown olarak kalır).
        assert_eq!(trigger_from_xml("<Task><Triggers /></Task>"), None);
    }

    #[test]
    fn disabled_suffix_is_stripped_case_insensitively() {
        assert_eq!(strip_disabled_suffix("foo.lnk.disabled"), Some("foo.lnk"));
        assert_eq!(strip_disabled_suffix("Foo.LNK.DISABLED"), Some("Foo.LNK"));
        assert_eq!(strip_disabled_suffix("foo.lnk"), None);
        assert_eq!(strip_disabled_suffix(".disabled"), None);
        assert_eq!(file_display_name("foo.lnk.disabled"), "foo");
        assert_eq!(file_display_name("bar.exe"), "bar");
    }

    #[test]
    fn registry_round_trip_preserves_type_and_disabled_state() {
        // Gerçek kayıt defterine YAZMAZ: yalnız saf yardımcıları sınar.
        let raw = string_reg_value("C:\\x.exe --flag", REG_EXPAND_SZ);
        assert_eq!(raw.vtype, REG_EXPAND_SZ);
        assert_eq!(raw_to_display(&raw).as_deref(), Some("C:\\x.exe --flag"));
        // Baytlar UTF-16LE + sonlandırıcı.
        assert_eq!(raw.bytes.len(), "C:\\x.exe --flag".len() * 2 + 2);
    }

    #[test]
    fn task_scope_detects_system_accounts() {
        assert_eq!(task_scope("\\Custom\\", "SYSTEM", false), Scope::System);
        assert_eq!(task_scope("\\Microsoft\\Windows\\", "u", false), Scope::System);
        assert_eq!(task_scope("\\Custom\\", "DOMAIN\\user", false), Scope::User);
    }

    #[test]
    fn usable_time_drops_never_run_values() {
        assert_eq!(usable_time(&None), None);
        assert_eq!(usable_time(&Some("".into())), None);
        assert_eq!(usable_time(&Some("1899-12-30T00:00:00".into())), None);
        assert_eq!(
            usable_time(&Some("2026-09-13T08:11:22.0000000".into())).as_deref(),
            Some("2026-09-13T08:11:22.0000000")
        );
    }
}
