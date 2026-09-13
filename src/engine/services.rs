//! Servis yöneticisi: systemd birimleri (`sweep services`).
//!
//! Salt okunur `list` + `start/stop/restart/enable/disable`. Birim adı
//! doğrulanır (yalnızca `[A-Za-z0-9@_:.-]+\.service`), komut kabuksuz
//! çalıştırılır. Yalnızca Linux + systemd.

use crate::core::error::{Error, Result};
use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "services";

fn ensure_linux() -> Result<()> {
    if cfg!(not(target_os = "linux")) {
        return Err(Error::msg("services is Linux + systemd only"));
    }
    if !crate::fsutil::path_exists_in_path("systemctl") {
        return Err(Error::msg("no systemctl"));
    }
    Ok(())
}

/// Birim adı güvenli mi? (yol ayracı, boşluk, kabuk meta-karakteri yasak).
pub fn valid_unit(unit: &str) -> bool {
    if unit.len() > 256 || !unit.ends_with(".service") {
        return false;
    }
    unit.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '@' | ':'))
}

fn scope_args(user: bool) -> Vec<&'static str> {
    if user {
        vec!["--user"]
    } else {
        Vec::new()
    }
}

/// Yapısal servis kaydı (GUI ve CLI ortak kullanır).
#[derive(Debug, Clone)]
pub struct ServiceUnit {
    pub name: String,
    pub active: String,
    pub sub: String,
    pub desc: String,
}

/// Servisleri sorgula (ham liste; en fazla 500 kayıt, isme göre sıralı).
///
/// `systemctl list-units --plain` sütunları:
/// `UNIT LOAD ACTIVE SUB DESCRIPTION` — örn. `cron.service loaded active
/// running Regular background program processing daemon`.
/// `LOAD` her zaman `loaded` olduğundan atlanır: `cols[0]` UNIT,
/// `cols[2]` ACTIVE, `cols[3]` SUB, `cols[4..]` DESCRIPTION.
pub fn query_units(user: bool) -> std::result::Result<Vec<ServiceUnit>, String> {
    ensure_linux().map_err(|e| e.to_string())?;
    let mut args = scope_args(user);
    args.extend([
        "list-units",
        "--type=service",
        "--all",
        "--no-legend",
        "--plain",
    ]);
    let out = match crate::fsutil::run_command("systemctl", &args, true) {
        Ok((0, stdout, _)) => stdout,
        Ok((code, _, stderr)) => return Err(format!("systemctl exit {code}: {stderr}")),
        Err(err) => return Err(format!("systemctl error: {err}")),
    };
    let mut units = Vec::new();
    for line in out.lines().take(500) {
        let cols: Vec<&str> = line.split_whitespace().collect();
        if cols.len() < 4 {
            continue;
        }
        if !valid_unit(cols[0]) {
            continue;
        }
        units.push(ServiceUnit {
            name: cols[0].to_string(),
            active: cols[2].to_string(),
            sub: cols[3].to_string(),
            desc: cols.get(4..).map(|c| c.join(" ")).unwrap_or_default(),
        });
    }
    units.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(units)
}

/// Servis listesi (etkinlik durumuyla).
pub fn list(user: bool) -> Report {
    let mut report = Report::new();
    match query_units(user) {
        Ok(units) => {
            if units.is_empty() {
                report.push(Entry::new(
                    EntryKind::Command,
                    CLEANER_ID,
                    "list",
                    "no services found",
                    None,
                    0,
                ));
            }
            for unit in units {
                report.push(Entry::new(
                    EntryKind::Command,
                    CLEANER_ID,
                    unit.active.clone(),
                    format!("{} [{}] {}", unit.name, unit.sub, unit.desc),
                    None,
                    0,
                ));
            }
        }
        Err(err) => report.fail(CLEANER_ID, "list", err),
    }
    report
}

/// `op`: start | stop | restart | enable | disable.
pub fn control(unit: &str, op: &str, user: bool) -> Report {
    let mut report = Report::new();
    if let Err(err) = ensure_linux() {
        report.fail(CLEANER_ID, op, err.to_string());
        return report;
    }
    if !matches!(op, "start" | "stop" | "restart" | "enable" | "disable") {
        report.fail(CLEANER_ID, "args", format!("unknown op: {op}"));
        return report;
    }
    if !valid_unit(unit) {
        report.fail(CLEANER_ID, op, format!("invalid unit: {unit}"));
        return report;
    }
    if !user && !crate::deep::safety::is_root() {
        report.fail(
            CLEANER_ID,
            op,
            "root needed for system services (use --user)",
        );
        return report;
    }
    let mut args = scope_args(user);
    args.extend([op, unit]);
    match crate::fsutil::run_command("systemctl", &args, true) {
        Ok((0, _, _)) => report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            op,
            format!("{unit}: {op} done"),
            None,
            0,
        )),
        Ok((code, _, stderr)) => report.fail(
            CLEANER_ID,
            op,
            format!("systemctl {op} {unit} exit {code}: {stderr}"),
        ),
        Err(err) => report.fail(CLEANER_ID, op, format!("systemctl error: {err}")),
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_names() {
        assert!(valid_unit("bluetooth.service"));
        assert!(valid_unit("getty@tty1.service"));
        assert!(!valid_unit("../../etc/passwd"));
        assert!(!valid_unit("x; rm -rf.service"));
        assert!(!valid_unit("noext"));
    }
}
