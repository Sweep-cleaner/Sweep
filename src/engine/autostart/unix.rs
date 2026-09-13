//! Unix (Linux / macOS) arka ucu.
//!
//! Linux/macOS için **salt okunur keşif yeterlidir**; yine de kullanıcı
//! kapsamlı XDG `.desktop` girdileri (`Hidden=`) ve launchd agent'ları
//! (`.disabled` yeniden adlandırma) geri alınabilir biçimde yönetilir.
//! systemd birimleri, cron ve shell rc satırları salt okunurdur.

use std::path::{Path, PathBuf};

use super::backend::{Backend, Discovery};
use super::*;

/// Unix sistem arka ucu.
pub struct UnixBackend;

impl UnixBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UnixBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn home() -> Option<PathBuf> {
    crate::platform::home_dir()
}

fn read_text(path: &Path) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// `.desktop` alanı oku (`Hidden=true` vb.).
fn desktop_field(text: &str, field: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        if key.trim().eq_ignore_ascii_case(field) {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

/// `Hidden=true` mı?
fn hidden_flag(text: &str) -> bool {
    desktop_field(text, "Hidden")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn file_state(path: &Path) -> String {
    if path.is_file() {
        return "enabled".to_string();
    }
    if PathBuf::from(format!("{}.disabled", path.display())).is_file() {
        return "disabled".to_string();
    }
    "absent".to_string()
}

fn actual_file(path: &Path) -> PathBuf {
    if path.is_file() {
        path.to_path_buf()
    } else {
        PathBuf::from(format!("{}.disabled", path.display()))
    }
}

/// `.desktop` girdisini `Hidden=` bayrağıyla üret.
fn set_desktop_hidden(path: &Path, hidden: bool) -> Result<(), String> {
    let text = read_text(path).unwrap_or_default();
    let mut lines: Vec<String> = Vec::new();
    let mut found = false;
    for line in text.lines() {
        if line
            .split_once('=')
            .map(|(k, _)| k.trim().eq_ignore_ascii_case("Hidden"))
            .unwrap_or(false)
        {
            lines.push(format!("Hidden={}", if hidden { "true" } else { "false" }));
            found = true;
        } else {
            lines.push(line.to_string());
        }
    }
    if !found {
        lines.push(format!("Hidden={}", if hidden { "true" } else { "false" }));
    }
    let mut out = lines.join("\n");
    if !out.ends_with('\n') {
        out.push('\n');
    }
    std::fs::write(path, out).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

// --- XDG autostart (Linux) --------------------------------------------------

#[cfg(all(unix, not(target_os = "macos")))]
fn xdg_dirs() -> Vec<(PathBuf, bool)> {
    let mut dirs = Vec::new();
    if let Some(home) = home() {
        dirs.push((home.join(".config/autostart"), false));
    }
    dirs.push((PathBuf::from("/etc/xdg/autostart"), true));
    if let Ok(xdg) = std::env::var("XDG_CONFIG_DIRS") {
        for part in std::env::split_paths(&xdg) {
            dirs.push((part.join("autostart"), true));
        }
    }
    dirs
}

#[cfg(all(unix, not(target_os = "macos")))]
fn xdg_items() -> Vec<Item> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (dir, system) in xdg_dirs() {
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
            let text = read_text(&path).unwrap_or_default();
            let name = desktop_field(&text, "Name").unwrap_or(stem);
            let enabled = !hidden_flag(&text);
            let command = desktop_field(&text, "Exec");
            let mut item = Item::new(
                Kind::Startup,
                name,
                Source::XdgAutostart,
                path.to_string_lossy().into_owned(),
                if system { Scope::System } else { Scope::User },
                enabled,
                Some(Trigger::Logon),
                if system {
                    Handle::ReadOnly
                } else {
                    Handle::File {
                        path: path.clone(),
                        hidden_flag: true,
                    }
                },
            );
            item.command = command;
            item.reclassify();
            if system {
                item.reversible = false;
                item.editable = false;
                item.removable = false;
                item.notes = Some("system XDG autostart (read-only)".to_string());
            }
            out.push(item);
        }
    }
    out
}

// --- systemd -----------------------------------------------------------------

#[cfg(all(unix, not(target_os = "macos")))]
fn systemd_units(user: bool, startup: bool) -> Vec<Item> {
    if !crate::deep::have("systemctl") {
        return Vec::new();
    }
    let mut out = Vec::new();
    let scope_flag = if user { "--user" } else { "--system" };
    if startup {
        // `default.target`e bağlı, etkin servisler.
        let args = if user {
            vec!["--user", "list-unit-files", "--type=service", "--state=enabled", "--no-legend", "--no-pager"]
        } else {
            vec!["--system", "list-unit-files", "--type=service", "--state=enabled", "--no-legend", "--no-pager"]
        };
        if let Some(text) = crate::deep::command_output("systemctl", &args) {
            for line in text.lines() {
                let Some(unit) = line.split_whitespace().next() else {
                    continue;
                };
                if !unit.ends_with(".service") {
                    continue;
                }
                let mut item = Item::new(
                    Kind::Startup,
                    unit.to_string(),
                    if user { Source::SystemdUser } else { Source::SystemdSystem },
                    format!("systemd:{scope_flag}:{unit}"),
                    if user { Scope::User } else { Scope::System },
                    true,
                    Some(Trigger::Logon),
                    Handle::Systemd {
                        unit: unit.to_string(),
                        user,
                    },
                );
                item.reversible = false;
                item.editable = false;
                item.removable = false;
                item.notes = Some("systemd unit (manage with systemctl)".to_string());
                out.push(item);
            }
        }
    } else {
        let args = vec![scope_flag, "list-timers", "--all", "--no-legend", "--no-pager"];
        if let Some(text) = crate::deep::command_output("systemctl", &args) {
            for line in text.lines() {
                let Some(unit) = line.split_whitespace().next() else {
                    continue;
                };
                if !unit.ends_with(".timer") {
                    continue;
                }
                let mut item = Item::new(
                    Kind::Task,
                    unit.to_string(),
                    Source::SystemdTimer,
                    format!("systemd:{scope_flag}:{unit}"),
                    if user { Scope::User } else { Scope::System },
                    true,
                    Some(Trigger::Schedule),
                    Handle::Systemd {
                        unit: unit.to_string(),
                        user,
                    },
                );
                item.reversible = false;
                item.editable = false;
                item.removable = false;
                item.notes = Some("systemd timer (manage with systemctl)".to_string());
                out.push(item);
            }
        }
    }
    out
}

// --- launchd (macOS) --------------------------------------------------------

#[cfg(target_os = "macos")]
fn launchd_dirs() -> Vec<(PathBuf, bool, Source)> {
    let mut out = Vec::new();
    if let Some(home) = home() {
        out.push((home.join("Library/LaunchAgents"), false, Source::LaunchdAgent));
    }
    out.push((PathBuf::from("/Library/LaunchAgents"), true, Source::LaunchdAgent));
    out.push((PathBuf::from("/Library/LaunchDaemons"), true, Source::LaunchdDaemon));
    out
}

#[cfg(target_os = "macos")]
fn launchd_items() -> Vec<Item> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for (dir, system, source) in launchd_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let raw = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if !raw.ends_with(".plist") && !raw.ends_with(".plist.disabled") {
                continue;
            }
            let label = raw
                .trim_end_matches(".disabled")
                .trim_end_matches(".plist")
                .to_string();
            if !seen.insert(label.clone()) {
                continue;
            }
            let enabled = raw.ends_with(".plist");
            let text = read_text(&path).unwrap_or_default();
            let command = plist_first_string(&text, "Program")
                .or_else(|| plist_first_string(&text, "ProgramArguments"));
            let mut item = Item::new(
                Kind::Startup,
                label,
                source,
                path.to_string_lossy().into_owned(),
                if system { Scope::System } else { Scope::User },
                enabled,
                Some(Trigger::Logon),
                Handle::File {
                    path: PathBuf::from(path.to_string_lossy().trim_end_matches(".disabled").to_string()),
                    hidden_flag: false,
                },
            );
            item.command = command;
            item.reclassify();
            item.editable = false;
            if system {
                item.removable = false;
                item.notes = Some("launchd daemon (system, read-only)".to_string());
            }
            out.push(item);
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn plist_first_string(text: &str, key: &str) -> Option<String> {
    let at = text.find(&format!("<key>{key}</key>"))?;
    let rest = &text[at..];
    let open = rest.find("<string>")? + "<string>".len();
    let close = rest[open..].find("</string>")? + open;
    Some(rest[open..close].trim().to_string())
}

// --- cron + shell rc (ortak) ------------------------------------------------

fn cron_lines(user: bool) -> Vec<String> {
    if user {
        crate::deep::command_output("crontab", &["-l"])
            .map(|t| t.lines().map(|l| l.to_string()).collect())
            .unwrap_or_default()
    } else {
        let mut out = Vec::new();
        if let Ok(entries) = std::fs::read_dir("/etc/cron.d") {
            for entry in entries.flatten() {
                if let Some(text) = read_text(&entry.path()) {
                    for line in text.lines() {
                        if !line.trim().starts_with('#') && !line.trim().is_empty() {
                            out.push(line.to_string());
                        }
                    }
                }
            }
        }
        if let Ok(text) = std::fs::read_to_string("/etc/crontab") {
            for line in text.lines() {
                if !line.trim().starts_with('#') && !line.trim().is_empty() {
                    out.push(line.to_string());
                }
            }
        }
        out
    }
}

fn cron_items() -> Vec<Item> {
    let mut out = Vec::new();
    for (user, source, scope) in [
        (true, Source::Cron, Scope::User),
        (false, Source::CronSystem, Scope::System),
    ] {
        for line in cron_lines(user) {
            let trimmed = line.trim().to_string();
            if trimmed.is_empty() {
                continue;
            }
            let reboot = trimmed.to_lowercase().starts_with("@reboot");
            let command = if reboot {
                trimmed["@reboot".len()..].trim().to_string()
            } else {
                trimmed.clone()
            };
            let (kind, trigger) = if reboot {
                (Kind::Startup, Trigger::Boot)
            } else {
                (Kind::Task, Trigger::Schedule)
            };
            let mut item = Item::new(
                kind,
                format!("cron: {}", truncate(&command, 48)),
                source,
                format!("cron:{}", if user { "user" } else { "system" }),
                scope,
                true,
                Some(trigger),
                Handle::ReadOnly,
            );
            item.command = Some(command);
            item.reclassify();
            item.reversible = false;
            item.editable = false;
            item.removable = false;
            item.notes = Some("cron entry (read-only; edit with crontab -e)".to_string());
            out.push(item);
        }
    }
    out
}

fn shellrc_items() -> Vec<Item> {
    let Some(home) = home() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for file in [".bashrc", ".zshrc", ".profile", ".bash_profile"] {
        let path = home.join(file);
        let Some(text) = read_text(&path) else {
            continue;
        };
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            if !looks_like_daemon_launch(trimmed) {
                continue;
            }
            let mut item = Item::new(
                Kind::Startup,
                format!("{file}:{}", idx + 1),
                Source::ShellRc,
                path.to_string_lossy().into_owned(),
                Scope::User,
                true,
                Some(Trigger::Logon),
                Handle::ReadOnly,
            );
            item.command = Some(trimmed.to_string());
            item.reclassify();
            item.reversible = false;
            item.editable = false;
            item.removable = false;
            item.notes = Some(format!("shell rc line {} ({file})", idx + 1));
            out.push(item);
        }
    }
    out
}

/// Shell rc satırı bir artalan süreci başlatıyor mu? (sezgisel)
fn looks_like_daemon_launch(line: &str) -> bool {
    let lower = line.to_lowercase();
    if lower.starts_with("export ") || lower.starts_with("alias ") || lower.starts_with("source ") {
        return false;
    }
    lower.ends_with('&')
        || lower.contains("nohup ")
        || lower.contains("systemd-run")
        || lower.contains("setsid ")
        || lower.starts_with("exec ")
        || lower.contains("daemon")
        || lower.contains("tmux new")
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        text.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

fn discover_startup() -> Vec<Item> {
    let mut out = Vec::new();
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        out.extend(xdg_items());
        out.extend(systemd_units(true, true));
    }
    #[cfg(target_os = "macos")]
    {
        out.extend(launchd_items());
    }
    out.extend(
        cron_items()
            .into_iter()
            .filter(|i| i.kind == Kind::Startup),
    );
    out.extend(shellrc_items());
    out
}

fn discover_tasks() -> Vec<Item> {
    let mut out = Vec::new();
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        out.extend(systemd_units(true, false));
    }
    out.extend(
        cron_items()
            .into_iter()
            .filter(|i| i.kind == Kind::Task),
    );
    out
}

impl Backend for UnixBackend {
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
        let (path, hidden) = match &item.handle {
            Handle::File { path, hidden_flag } => (path.clone(), *hidden_flag),
            _ => return Err("entry is read-only".to_string()),
        };
        if hidden {
            let text = read_text(&path).unwrap_or_default();
            Ok(serde_json::json!({
                "kind": "file",
                "path": path.to_string_lossy(),
                "enabled": !hidden_flag(&text),
                "hidden": true,
                "bytes": text.into_bytes(),
            }))
        } else {
            let state = file_state(&path);
            let bytes = std::fs::read(actual_file(&path)).unwrap_or_default();
            Ok(serde_json::json!({
                "kind": "file",
                "path": path.to_string_lossy(),
                "enabled": state == "enabled",
                "hidden": false,
                "bytes": bytes,
            }))
        }
    }

    fn set_enabled(&self, item: &Item, enabled: bool) -> Result<(), String> {
        if item.enabled == enabled {
            return Ok(());
        }
        match &item.handle {
            Handle::File {
                path,
                hidden_flag: true,
            } => set_desktop_hidden(path, !enabled),
            Handle::File {
                path,
                hidden_flag: false,
            } => {
                if enabled {
                    let src = PathBuf::from(format!("{}.disabled", path.display()));
                    std::fs::rename(&src, path)
                        .map_err(|e| format!("cannot enable {}: {e}", path.display()))
                } else {
                    let dest = PathBuf::from(format!("{}.disabled", path.display()));
                    std::fs::rename(path, &dest)
                        .map_err(|e| format!("cannot disable {}: {e}", path.display()))
                }
            }
            _ => Err("entry is not reversible".to_string()),
        }
    }

    fn edit(&self, item: &Item, command: &str) -> Result<(), String> {
        let Handle::File {
            path,
            hidden_flag: true,
        } = &item.handle
        else {
            return Err("entry is not editable".to_string());
        };
        let text = read_text(path).unwrap_or_default();
        let mut lines: Vec<String> = Vec::new();
        let mut found = false;
        for line in text.lines() {
            if line
                .split_once('=')
                .map(|(k, _)| k.trim().eq_ignore_ascii_case("Exec"))
                .unwrap_or(false)
            {
                lines.push(format!("Exec={command}"));
                found = true;
            } else {
                lines.push(line.to_string());
            }
        }
        if !found {
            lines.push(format!("Exec={command}"));
        }
        let mut out = lines.join("\n");
        if !out.ends_with('\n') {
            out.push('\n');
        }
        std::fs::write(path, out).map_err(|e| format!("cannot write {}: {e}", path.display()))
    }

    fn remove(&self, item: &Item) -> Result<(), String> {
        match &item.handle {
            Handle::File { path, .. } => std::fs::remove_file(actual_file(path))
                .map_err(|e| format!("cannot remove {}: {e}", path.display())),
            _ => Err("entry is not removable".to_string()),
        }
    }

    fn restore(&self, payload: &serde_json::Value) -> Result<(), String> {
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
        let hidden = payload
            .get("hidden")
            .and_then(|e| e.as_bool())
            .unwrap_or(false);
        let bytes: Vec<u8> = payload
            .get("bytes")
            .and_then(|b| serde_json::from_value(b.clone()).ok())
            .unwrap_or_default();
        let disabled = PathBuf::from(format!("{}.disabled", path.display()));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(&disabled);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let dest = if hidden || enabled { &path } else { &disabled };
        std::fs::write(dest, &bytes).map_err(|e| format!("cannot restore {}: {e}", dest.display()))
    }

    fn state_of(&self, item: &Item) -> Result<String, String> {
        match &item.handle {
            Handle::File {
                path,
                hidden_flag: true,
            } => {
                let Some(text) = read_text(path) else {
                    return Ok("absent".to_string());
                };
                Ok(if hidden_flag(&text) {
                    "disabled".to_string()
                } else {
                    "enabled".to_string()
                })
            }
            Handle::File {
                path,
                hidden_flag: false,
            } => Ok(file_state(path)),
            _ => Err("entry has no readable state".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hidden_flag_reads_the_desktop_field() {
        let text = "[Desktop Entry]\nName=Foo\nExec=foo\nHidden=true\n";
        assert!(hidden_flag(text));
        assert!(!hidden_flag("[Desktop Entry]\nHidden=false\n"));
        assert!(!hidden_flag("[Desktop Entry]\nExec=bar\n"));
        assert_eq!(desktop_field(text, "name").as_deref(), Some("Foo"));
    }

    #[test]
    fn shell_rc_heuristic_ignores_exports_and_aliases() {
        assert!(looks_like_daemon_launch("nohup ./agent &"));
        assert!(looks_like_daemon_launch("/usr/bin/foo --daemon &"));
        assert!(looks_like_daemon_launch("exec wm"));
        assert!(!looks_like_daemon_launch("export PATH=$PATH:/opt/bin"));
        assert!(!looks_like_daemon_launch("alias ll='ls -l'"));
        assert!(!looks_like_daemon_launch("# nohup foo &"));
    }

    #[test]
    fn file_state_distinguishes_enabled_disabled_absent() {
        let dir = std::env::temp_dir().join(format!(
            "sweep_unix_file_{}_{}",
            std::process::id(),
            super::super::short_hash("file_state")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("a.desktop");
        assert_eq!(file_state(&path), "absent");
        std::fs::write(&path, "x").unwrap();
        assert_eq!(file_state(&path), "enabled");
        std::fs::rename(&path, dir.join("a.desktop.disabled")).unwrap();
        assert_eq!(file_state(&path), "disabled");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
