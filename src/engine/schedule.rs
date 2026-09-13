//! Zamanlanmış temizlik: günlük sessiz `sweep clean` koşusu.
//!
//! Arka uç platforma göre seçilir: Linux'ta systemd user timer,
//! Windows'ta Task Scheduler (`schtasks`), macOS'te launchd agent
//! (`~/Library/LaunchAgents/com.sweep.clean.plist` + `launchctl`).
//!
//! `sweep schedule --enable --hour 3 -- system.journald apt.deep_cache`
//! her gece 03:00'te sessiz temizlik yapar.

use std::path::PathBuf;

use crate::core::error::{Error, Result};
use crate::i18n::Lang;

/// Zamanlayıcı arka ucu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Systemd,
    TaskScheduler,
    Launchd,
}

/// Bu platformda hangi arka uç kullanılır?
pub fn backend() -> Backend {
    #[cfg(windows)]
    {
        Backend::TaskScheduler
    }
    #[cfg(target_os = "macos")]
    {
        Backend::Launchd
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Backend::Systemd
    }
}

/// Kullanıcıya gösterilen arka uç adı (`sweep schedule` çıktısı için).
pub fn backend_name() -> &'static str {
    match backend() {
        Backend::Systemd => "systemd",
        Backend::TaskScheduler => "Task Scheduler",
        Backend::Launchd => "launchd",
    }
}

/// Kullanıcı birim dizini (`~/.config/systemd/user`, yalnızca systemd).
pub fn unit_dir() -> Result<PathBuf> {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .ok()
        .or_else(|| crate::platform::home_dir().map(|h| h.join(".config")))
        .ok_or_else(|| Error::msg("no config dir"))?;
    Ok(base.join("systemd").join("user"))
}

/// launchd agent etiketi ve plist konumu.
pub fn launchd_label() -> &'static str {
    "com.sweep.clean"
}

/// launchd bellek agent etiketi.
pub fn launchd_mem_label() -> &'static str {
    "com.sweep.memopt"
}

/// `~/Library/LaunchAgents/com.sweep.memopt.plist`.
pub fn launchd_mem_plist() -> Result<PathBuf> {
    let home = crate::platform::home_dir().ok_or_else(|| Error::msg("no home dir"))?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", launchd_mem_label())))
}

/// `~/Library/LaunchAgents/com.sweep.clean.plist`.
pub fn launchd_plist() -> Result<PathBuf> {
    let home = crate::platform::home_dir().ok_or_else(|| Error::msg("no home dir"))?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{}.plist", launchd_label())))
}

/// Task Scheduler görev yolu (`Sweep` klasörü altında).
pub fn task_name() -> &'static str {
    "Sweep\\SweepClean"
}

/// Task Scheduler bellek görevi yolu (güvenli memopt, yükseltmesiz).
pub fn task_mem_name() -> &'static str {
    "Sweep\\SweepMemory"
}

/// Selection entry for a scheduled cleanup (`*`, `all`, `cleaner` or `cleaner.option`).
fn valid_selection(entry: &str) -> bool {
    if entry == "*" || entry.eq_ignore_ascii_case("all") {
        return true;
    }
    let (cleaner, option) = match entry.split_once('.') {
        Some(pair) => pair,
        None => (entry, ""),
    };
    fn part_ok(part: &str) -> bool {
        !part.is_empty()
            && part.len() <= 64
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '*' | '?'))
    }
    if option.is_empty() {
        part_ok(cleaner)
    } else {
        part_ok(cleaner) && part_ok(option)
    }
}

/// Bütün seçim girdilerini doğrula; ilk hatada dur.
fn check_selection(selection: &[String]) -> Result<()> {
    for entry in selection {
        if !valid_selection(entry) {
            return Err(Error::msg(format!("invalid selection: {entry}")));
        }
    }
    Ok(())
}

fn check_hour(hour: u32) -> Result<()> {
    if hour > 23 {
        return Err(Error::msg("--hour must be 0-23"));
    }
    Ok(())
}

fn sweep_exe() -> String {
    std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "/usr/bin/sweep".to_string())
}

/// `clean --yes ...` kuyruğu (boş seçim = varsayılan).
fn clean_args(selection: &[String]) -> String {
    if selection.is_empty() {
        "system.journal_vacuum".to_string()
    } else {
        selection.join(" ")
    }
}

fn selection_summary(selection: &[String]) -> String {
    if selection.is_empty() {
        "default".to_string()
    } else {
        selection.join(" ")
    }
}

// ---------------------------------------------------------------------------
// systemd gövdeleri
// ---------------------------------------------------------------------------

fn service_body(selection: &[String]) -> Result<String> {
    check_selection(selection)?;
    let args = clean_args(selection);
    Ok(format!(
        "[Unit]\nDescription=Sweep scheduled cleanup\n\n[Service]\nType=oneshot\n\
         ExecStart={} clean --yes {args}\nNice=19\nIOSchedulingClass=idle\n\
         NoNewPrivileges=true\n\n[Install]\nWantedBy=default.target\n",
        sweep_exe()
    ))
}

fn timer_body(hour: u32) -> String {
    format!(
        "[Unit]\nDescription=Sweep daily cleanup timer\n\n[Timer]\n\
         OnCalendar=*-*-* {hour:02}:00:00\nOnBootSec=15min\nPersistent=true\n\
         RandomizedDelaySec=1800\nAccuracySec=1h\n\n\
         [Install]\nWantedBy=timers.target\n"
    )
}

/// Bellek servisi: yükseltmesiz güvenli memopt koşar.
fn mem_service_body() -> String {
    format!(
        "[Unit]\nDescription=Sweep scheduled memory optimization\n\n[Service]\nType=oneshot\n\
         ExecStart={} memopt --quiet\nNice=19\nIOSchedulingClass=idle\n\
         NoNewPrivileges=true\n\n[Install]\nWantedBy=default.target\n",
        sweep_exe()
    )
}

fn mem_timer_body(hour: u32) -> String {
    format!(
        "[Unit]\nDescription=Sweep daily memory timer\n\n[Timer]\n\
         OnCalendar=*-*-* {hour:02}:00:00\nOnBootSec=15min\nPersistent=true\n\
         RandomizedDelaySec=1800\nAccuracySec=1h\n\n\
         [Install]\nWantedBy=timers.target\n"
    )
}

fn systemctl(args: &[&str]) -> Result<String> {
    if !crate::fsutil::path_exists_in_path("systemctl") {
        return Err(Error::msg("no systemctl (systemd needed)"));
    }
    match crate::fsutil::run_command("systemctl", args, true) {
        Ok((0, out, _)) => Ok(out.trim().to_string()),
        Ok((code, _, stderr)) => Err(Error::msg(format!(
            "systemctl {} exit {code}: {}",
            args.join(" "),
            stderr.trim()
        ))),
        Err(err) => Err(Error::msg(format!("systemctl error: {err}"))),
    }
}

// ---------------------------------------------------------------------------
// Task Scheduler (Windows): schtasks.exe üstünden kullanıcı görevi
// ---------------------------------------------------------------------------

/// XML/komut satırı için `&<>"` kaçışı (plist gövdesiyle ortak).
fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn schtasks(args: &[&str]) -> Result<String> {
    if !crate::fsutil::path_exists_in_path("schtasks") {
        return Err(Error::msg("no schtasks (Windows Task Scheduler needed)"));
    }
    match crate::fsutil::run_command("schtasks", args, true) {
        Ok((0, out, _)) => Ok(out.trim().to_string()),
        Ok((code, _, stderr)) => Err(Error::msg(format!(
            "schtasks {} exit {code}: {}",
            args.join(" "),
            stderr.trim()
        ))),
        Err(err) => Err(Error::msg(format!("schtasks error: {err}"))),
    }
}

/// `/TR` için tam komut: exe yolu boşluklu olabilir, tırnakla.
fn task_run_command(selection: &[String]) -> Result<String> {
    check_selection(selection)?;
    Ok(format!(
        "\"{}\" clean --yes {}",
        sweep_exe(),
        clean_args(selection)
    ))
}

fn task_enable(selection: &[String], hour: u32, lang: &Lang) -> Result<String> {
    check_hour(hour)?;
    let run = task_run_command(selection)?;
    let time = format!("{hour:02}:00");
    schtasks(&[
        "/Create",
        "/TN",
        task_name(),
        "/TR",
        run.as_str(),
        "/SC",
        "DAILY",
        "/ST",
        time.as_str(),
        "/F",
    ])?;
    Ok(crate::i18n::et(
        lang,
        "sched_installed",
        &[time.as_str(), backend_name(), selection_summary(selection).as_str()],
    ))
}

fn task_disable(lang: &Lang) -> Result<String> {
    // Yoksa da hata verme: amaç kaldırılmış olması.
    let _ = schtasks(&["/Delete", "/TN", task_name(), "/F"]);
    Ok(crate::i18n::t(lang, "sched_removed"))
}

/// `/TR` için bellek komutu: güvenli memopt, sessiz.
fn mem_task_run_command() -> String {
    format!("\"{}\" memopt --quiet", sweep_exe())
}

fn task_memopt_enable(hour: u32, lang: &Lang) -> Result<String> {
    check_hour(hour)?;
    let run = mem_task_run_command();
    let time = format!("{hour:02}:00");
    schtasks(&[
        "/Create",
        "/TN",
        task_mem_name(),
        "/TR",
        run.as_str(),
        "/SC",
        "DAILY",
        "/ST",
        time.as_str(),
        "/F",
    ])?;
    Ok(crate::i18n::et(
        lang,
        "sched_installed",
        &[time.as_str(), backend_name(), "memopt"],
    ))
}

fn task_memopt_disable(lang: &Lang) -> Result<String> {
    let _ = schtasks(&["/Delete", "/TN", task_mem_name(), "/F"]);
    Ok(crate::i18n::t(lang, "sched_removed"))
}

fn task_memopt_status() -> String {
    match schtasks(&["/Query", "/TN", task_mem_name(), "/FO", "LIST", "/V"]) {
        Ok(out) => {
            let state = out
                .lines()
                .find(|l| l.trim_start().starts_with("Status:"))
                .map(|l| l.split(':').nth(1).unwrap_or("?").trim().to_string())
                .unwrap_or_else(|| "?".into());
            format!("{}: {state}", task_mem_name())
        }
        Err(e) => format!("status unknown: {e}"),
    }
}

fn task_status() -> String {
    match schtasks(&["/Query", "/TN", task_name(), "/FO", "LIST", "/V"]) {
        Ok(out) => {
            let state = out
                .lines()
                .find(|l| l.trim_start().starts_with("Status:"))
                .map(|l| l.split(':').nth(1).unwrap_or("?").trim().to_string())
                .unwrap_or_else(|| "?".into());
            format!("Sweep\\SweepClean: {state}")
        }
        Err(e) => format!("status unknown: {e}"),
    }
}

// ---------------------------------------------------------------------------
// launchd (macOS): ~/Library/LaunchAgents plist + launchctl
// ---------------------------------------------------------------------------

/// launchd agent plist gövdesi (saf test edilebilir çekirdek).
fn plist_body(exe: &str, selection: &[String], hour: u32) -> Result<String> {
    check_selection(selection)?;
    // Boş seçim = systemd ile aynı varsayılan (seçimsiz `clean` koşmaz).
    let effective: Vec<String> = if selection.is_empty() {
        vec!["system.journal_vacuum".to_string()]
    } else {
        selection.to_vec()
    };
    let mut args = format!(
        "    <string>{}</string>\n    <string>clean</string>\n    <string>--yes</string>\n",
        xml_escape(exe)
    );
    for entry in &effective {
        args.push_str(&format!("    <string>{}</string>\n", xml_escape(entry)));
    }
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n\
         \x20   <key>Label</key>\n    <string>{}</string>\n\
         \x20   <key>ProgramArguments</key>\n    <array>\n{args}    </array>\n\
         \x20   <key>StartCalendarInterval</key>\n    <dict>\n\
         \x20       <key>Hour</key>\n        <integer>{hour}</integer>\n\
         \x20       <key>Minute</key>\n        <integer>0</integer>\n    </dict>\n\
         </dict>\n</plist>\n",
        launchd_label()
    ))
}

fn launchctl(args: &[&str]) -> Result<String> {
    if !crate::fsutil::path_exists_in_path("launchctl") {
        return Err(Error::msg("no launchctl (macOS needed)"));
    }
    match crate::fsutil::run_command("launchctl", args, true) {
        Ok((0, out, _)) => Ok(out.trim().to_string()),
        Ok((code, _, stderr)) => Err(Error::msg(format!(
            "launchctl {} exit {code}: {}",
            args.join(" "),
            stderr.trim()
        ))),
        Err(err) => Err(Error::msg(format!("launchctl error: {err}"))),
    }
}

fn launchd_enable(selection: &[String], hour: u32, lang: &Lang) -> Result<String> {
    check_hour(hour)?;
    let plist = launchd_plist()?;
    if let Some(parent) = plist.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    // Eski kopya yüklüyse önce indir (load üstüne yazmaz).
    let plist_arg = plist.display().to_string();
    let _ = launchctl(&["unload", "-w", plist_arg.as_str()]);
    std::fs::write(&plist, plist_body(&sweep_exe(), selection, hour)?)
        .map_err(|e| Error::io(&plist, e))?;
    launchctl(&["load", "-w", plist_arg.as_str()])?;
    Ok(crate::i18n::et(
        lang,
        "sched_installed",
        &[
            format!("{hour:02}:00").as_str(),
            backend_name(),
            selection_summary(selection).as_str(),
        ],
    ))
}

fn launchd_disable(lang: &Lang) -> Result<String> {
    if let Ok(plist) = launchd_plist() {
        let plist_arg = plist.display().to_string();
        let _ = launchctl(&["unload", "-w", plist_arg.as_str()]);
        let _ = std::fs::remove_file(&plist);
    }
    Ok(crate::i18n::t(lang, "sched_removed"))
}

/// Bellek plist gövdesi (saf test edilebilir çekirdek).
fn mem_plist_body(exe: &str, hour: u32) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \
         \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n\
         <plist version=\"1.0\">\n<dict>\n\
         \x20   <key>Label</key>\n    <string>{}</string>\n\
         \x20   <key>ProgramArguments</key>\n    <array>\n\
         \x20   <string>{}</string>\n    <string>memopt</string>\n    <string>--quiet</string>\n    </array>\n\
         \x20   <key>StartCalendarInterval</key>\n    <dict>\n\
         \x20       <key>Hour</key>\n        <integer>{hour}</integer>\n\
         \x20       <key>Minute</key>\n        <integer>0</integer>\n    </dict>\n\
         </dict>\n</plist>\n",
        launchd_mem_label(),
        xml_escape(exe)
    )
}

fn launchd_memopt_enable(hour: u32, lang: &Lang) -> Result<String> {
    check_hour(hour)?;
    let plist = launchd_mem_plist()?;
    if let Some(parent) = plist.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    let plist_arg = plist.display().to_string();
    let _ = launchctl(&["unload", "-w", plist_arg.as_str()]);
    std::fs::write(&plist, mem_plist_body(&sweep_exe(), hour)).map_err(|e| Error::io(&plist, e))?;
    launchctl(&["load", "-w", plist_arg.as_str()])?;
    Ok(crate::i18n::et(
        lang,
        "sched_installed",
        &[format!("{hour:02}:00").as_str(), backend_name(), "memopt"],
    ))
}

fn launchd_memopt_disable(lang: &Lang) -> Result<String> {
    if let Ok(plist) = launchd_mem_plist() {
        let plist_arg = plist.display().to_string();
        let _ = launchctl(&["unload", "-w", plist_arg.as_str()]);
        let _ = std::fs::remove_file(&plist);
    }
    Ok(crate::i18n::t(lang, "sched_removed"))
}

fn launchd_memopt_status() -> String {
    match launchctl(&["list"]) {
        Ok(out) => {
            if out
                .lines()
                .any(|l| l.split_whitespace().any(|c| c == launchd_mem_label()))
            {
                format!("{}: loaded", launchd_mem_label())
            } else {
                format!("{}: not loaded", launchd_mem_label())
            }
        }
        Err(e) => format!("status unknown: {e}"),
    }
}

fn launchd_status() -> String {
    match launchctl(&["list"]) {
        Ok(out) => {
            if out.lines().any(|l| l.split_whitespace().any(|c| c == launchd_label())) {
                format!("{}: loaded", launchd_label())
            } else {
                format!("{}: not loaded", launchd_label())
            }
        }
        Err(e) => format!("status unknown: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Ortak giriş noktaları
// ---------------------------------------------------------------------------

fn systemd_enable(selection: &[String], hour: u32, lang: &Lang) -> Result<String> {
    check_hour(hour)?;
    let dir = unit_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
    let service = dir.join("sweep-clean.service");
    let timer = dir.join("sweep-clean.timer");
    std::fs::write(&service, service_body(selection)?).map_err(|e| Error::io(&service, e))?;
    std::fs::write(&timer, timer_body(hour)).map_err(|e| Error::io(&timer, e))?;
    systemctl(&["--user", "daemon-reload"])?;
    systemctl(&["--user", "enable", "--now", "sweep-clean.timer"])?;
    Ok(crate::i18n::et(
        lang,
        "sched_installed",
        &[
            format!("{hour:02}:00").as_str(),
            backend_name(),
            selection_summary(selection).as_str(),
        ],
    ))
}

fn systemd_disable(lang: &Lang) -> Result<String> {
    let _ = systemctl(&["--user", "disable", "--now", "sweep-clean.timer"]);
    if let Ok(dir) = unit_dir() {
        let _ = std::fs::remove_file(dir.join("sweep-clean.timer"));
        let _ = std::fs::remove_file(dir.join("sweep-clean.service"));
        let _ = systemctl(&["--user", "daemon-reload"]);
    }
    Ok(crate::i18n::t(lang, "sched_removed"))
}

fn systemd_memopt_enable(hour: u32, lang: &Lang) -> Result<String> {
    check_hour(hour)?;
    let dir = unit_dir()?;
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
    let service = dir.join("sweep-memopt.service");
    let timer = dir.join("sweep-memopt.timer");
    std::fs::write(&service, mem_service_body()).map_err(|e| Error::io(&service, e))?;
    std::fs::write(&timer, mem_timer_body(hour)).map_err(|e| Error::io(&timer, e))?;
    systemctl(&["--user", "daemon-reload"])?;
    systemctl(&["--user", "enable", "--now", "sweep-memopt.timer"])?;
    Ok(crate::i18n::et(
        lang,
        "sched_installed",
        &[format!("{hour:02}:00").as_str(), backend_name(), "memopt"],
    ))
}

fn systemd_memopt_disable(lang: &Lang) -> Result<String> {
    let _ = systemctl(&["--user", "disable", "--now", "sweep-memopt.timer"]);
    if let Ok(dir) = unit_dir() {
        let _ = std::fs::remove_file(dir.join("sweep-memopt.timer"));
        let _ = std::fs::remove_file(dir.join("sweep-memopt.service"));
        let _ = systemctl(&["--user", "daemon-reload"]);
    }
    Ok(crate::i18n::t(lang, "sched_removed"))
}

fn systemd_memopt_status() -> String {
    systemctl(&["--user", "is-active", "sweep-memopt.timer"])
        .map(|s| format!("sweep-memopt.timer: {s}"))
        .unwrap_or_else(|e| format!("status unknown: {e}"))
}

fn systemd_status() -> String {
    systemctl(&["--user", "is-active", "sweep-clean.timer"])
        .map(|s| format!("sweep-clean.timer: {s}"))
        .unwrap_or_else(|e| format!("status unknown: {e}"))
}

/// Timer'ı kur ve etkinleştir. Dönüş: insan dilinde özet.
pub fn enable(selection: &[String], hour: u32, lang: &Lang) -> Result<String> {
    match backend() {
        Backend::Systemd => systemd_enable(selection, hour, lang),
        Backend::TaskScheduler => task_enable(selection, hour, lang),
        Backend::Launchd => launchd_enable(selection, hour, lang),
    }
}

/// Timer'ı durdur, kaldır.
pub fn disable(lang: &Lang) -> Result<String> {
    match backend() {
        Backend::Systemd => systemd_disable(lang),
        Backend::TaskScheduler => task_disable(lang),
        Backend::Launchd => launchd_disable(lang),
    }
}

/// Bellek görevini kur ve etkinleştir (güvenli memopt, yükseltmesiz).
pub fn enable_memopt(hour: u32, lang: &Lang) -> Result<String> {
    match backend() {
        Backend::Systemd => systemd_memopt_enable(hour, lang),
        Backend::TaskScheduler => task_memopt_enable(hour, lang),
        Backend::Launchd => launchd_memopt_enable(hour, lang),
    }
}

/// Bellek görevini durdur, kaldır.
pub fn disable_memopt(lang: &Lang) -> Result<String> {
    match backend() {
        Backend::Systemd => systemd_memopt_disable(lang),
        Backend::TaskScheduler => task_memopt_disable(lang),
        Backend::Launchd => launchd_memopt_disable(lang),
    }
}

/// Bellek görevi durumu.
pub fn memopt_status() -> String {
    match backend() {
        Backend::Systemd => systemd_memopt_status(),
        Backend::TaskScheduler => task_memopt_status(),
        Backend::Launchd => launchd_memopt_status(),
    }
}

/// Timer durumu.
pub fn status() -> String {
    match backend() {
        Backend::Systemd => systemd_status(),
        Backend::TaskScheduler => task_status(),
        Backend::Launchd => launchd_status(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selections_validated() {
        assert!(valid_selection("*"));
        assert!(valid_selection("all"));
        assert!(valid_selection("system.journal_vacuum"));
        assert!(valid_selection("fire*"));
        assert!(!valid_selection(""));
        assert!(!valid_selection("a;b"));
        assert!(!valid_selection("x y"));
        assert!(!valid_selection(".option"));
        assert!(service_body(&["ok.clean".to_string()]).is_ok());
        assert!(service_body(&["bad;rm".to_string()]).is_err());
    }

    #[test]
    fn timer_has_boot_and_accuracy() {
        let body = timer_body(3);
        assert!(body.contains("OnCalendar=*-*-* 03:00:00"));
        assert!(body.contains("OnBootSec=15min"));
        assert!(body.contains("AccuracySec=1h"));
    }

    #[test]
    fn service_uses_current_exe() {
        let body = service_body(&[]).unwrap();
        assert!(body.contains("clean --yes system.journal_vacuum"));
        assert!(!body.contains("/usr/bin/sweep clean") || sweep_exe() == "/usr/bin/sweep");
    }

    #[test]
    fn task_command_quotes_exe_and_validates() {
        let run = task_run_command(&["firefox.cache".to_string()]).unwrap();
        assert!(run.starts_with('"'));
        assert!(run.contains("clean --yes firefox.cache"));
        assert!(task_run_command(&["bad;rm".to_string()]).is_err());
    }

    #[test]
    fn plist_has_label_and_calendar() {
        let body = plist_body("/usr/local/bin/sweep", &[], 3).unwrap();
        assert!(body.contains("<string>com.sweep.clean</string>"));
        assert!(body.contains("<key>StartCalendarInterval</key>"));
        assert!(body.contains("<integer>3</integer>"));
        assert!(body.contains("<string>/usr/local/bin/sweep</string>"));
        // Boş seçim systemd varsayılanını yazar (seçimsiz clean yok).
        assert!(body.contains("<string>system.journal_vacuum</string>"));
        assert!(plist_body("/bin/sweep", &["a;b".to_string()], 3).is_err());
    }

    #[test]
    fn plist_escapes_exe_path() {
        let body = plist_body("/a&b/<sweep>", &[], 3).unwrap();
        assert!(body.contains("/a&amp;b/&lt;sweep&gt;"));
    }

    #[test]
    fn hour_checked_on_every_backend() {
        assert!(check_hour(23).is_ok());
        assert!(check_hour(24).is_err());
    }

    #[test]
    fn memopt_runs_quiet_memopt_on_every_backend() {
        let service = mem_service_body();
        assert!(service.contains("memopt --quiet"));
        assert!(!service.contains("clean"));
        assert!(service.contains("IOSchedulingClass=idle"));
        let timer = mem_timer_body(5);
        assert!(timer.contains("OnCalendar=*-*-* 05:00:00"));
        assert!(timer.contains("OnBootSec=15min"));
        let run = mem_task_run_command();
        assert!(run.starts_with('"'));
        assert!(run.contains("memopt --quiet"));
        assert_eq!(task_mem_name(), "Sweep\\SweepMemory");
        assert_eq!(launchd_mem_label(), "com.sweep.memopt");
        assert!(launchd_mem_plist().is_ok());
    }

    #[test]
    fn memopt_plist_has_label_and_memopt() {
        let body = mem_plist_body("/usr/local/bin/sweep", 4);
        assert!(body.contains("<string>com.sweep.memopt</string>"));
        assert!(body.contains("<string>memopt</string>"));
        assert!(body.contains("<string>--quiet</string>"));
        assert!(body.contains("<integer>4</integer>"));
        assert!(!body.contains("clean"));
        let escaped = mem_plist_body("/a&b/<sweep>", 4);
        assert!(escaped.contains("/a&amp;b/&lt;sweep&gt;"));
    }
}
