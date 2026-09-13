//! Anlık görüntü: temizlik öncesi geri dönüş noktası (`sweep snapshot`).
//!
//! Önce `snapper`, yoksa `timeshift` kullanılır; ikisi de yoksa ne yapılacağı
//! söylenir. `rollback` bilinçli olarak `--yes` ister: sistem durumunu geri
//! sarar, kayıp veri riski taşır.

use crate::core::error::{Error, Result};
use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "snapshot";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backend {
    Snapper,
    Timeshift,
}

fn backend() -> Result<Backend> {
    if cfg!(not(target_os = "linux")) {
        return Err(Error::msg("snapshots are Linux-only"));
    }
    if crate::fsutil::path_exists_in_path("snapper") {
        Ok(Backend::Snapper)
    } else if crate::fsutil::path_exists_in_path("timeshift") {
        Ok(Backend::Timeshift)
    } else {
        Err(Error::msg(
            "no snapper/timeshift — on btrfs run 'snapper -c root create-config /'",
        ))
    }
}

fn run(prog: &str, args: &[&str]) -> Result<String> {
    match crate::fsutil::run_command(prog, args, true) {
        Ok((0, out, _)) => Ok(out),
        Ok((code, _, stderr)) => Err(Error::msg(format!("{prog} exit {code}: {stderr}"))),
        Err(err) => Err(Error::msg(format!("{prog} error: {err}"))),
    }
}

/// Snapshot hedefi güvenli mi? (`..`, yol ayracı ve `-` ön-eki yasak.)
pub fn valid_target(target: &str) -> bool {
    if target.is_empty() || target.len() > 64 {
        return false;
    }
    if target == "." || target == ".." {
        return false;
    }
    if target.starts_with('-') || target.starts_with('.') {
        return false;
    }
    target
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Kullanıcı açıklamasını tek satıra indirip kısalt.
fn sanitize_desc(desc: &str) -> String {
    let flat: String = desc.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(128).collect()
}

/// Temizlik öncesi anlık görüntü al. Dönüş: rapor.
pub fn create(desc: &str) -> Report {
    let mut report = Report::new();
    let backend = match backend() {
        Ok(b) => b,
        Err(err) => {
            report.fail(CLEANER_ID, "backend", err.to_string());
            return report;
        }
    };
    if !crate::deep::safety::is_root() {
        report.fail(CLEANER_ID, "root", "snapshots need root (run with sudo)");
        return report;
    }
    let desc = if desc.is_empty() {
        format!("sweep-{}", jiffy())
    } else {
        sanitize_desc(desc)
    };
    let result = match backend {
        Backend::Snapper => run("snapper", &["-c", "root", "create", "-d", &desc]),
        Backend::Timeshift => run("timeshift", &["--create", "--comments", &desc, "--yes"]),
    };
    match result {
        Ok(out) => {
            let id = out.lines().last().unwrap_or("").trim().to_string();
            report.push(Entry::new(
                EntryKind::Command,
                CLEANER_ID,
                "create",
                format!("snapshot taken ({desc}) {id}"),
                None,
                0,
            ));
        }
        Err(err) => report.fail(CLEANER_ID, "create", err.to_string()),
    }
    report
}

/// Kayıtlı anlık görüntüleri listele.
pub fn list() -> Report {
    let mut report = Report::new();
    let backend = match backend() {
        Ok(b) => b,
        Err(err) => {
            report.fail(CLEANER_ID, "backend", err.to_string());
            return report;
        }
    };
    let result = match backend {
        Backend::Snapper => run("snapper", &["-c", "root", "list"]),
        Backend::Timeshift => run("timeshift", &["--list"]),
    };
    match result {
        Ok(out) => {
            let mut count = 0;
            for line in out.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if line.starts_with('#')
                    || line.starts_with("---+")
                    || line.starts_with("Type")
                    || line.starts_with("No snapshots")
                {
                    continue;
                }
                if count >= 60 {
                    break;
                }
                report.push(Entry::new(
                    EntryKind::Command,
                    CLEANER_ID,
                    "list",
                    line.to_string(),
                    None,
                    0,
                ));
                count += 1;
            }
            if count == 0 {
                report.push(Entry::new(
                    EntryKind::Command,
                    CLEANER_ID,
                    "list",
                    "no snapshots",
                    None,
                    0,
                ));
            }
        }
        Err(err) => report.fail(CLEANER_ID, "list", err.to_string()),
    }
    report
}

/// Anlık görüntü sil (snapper: numara, timeshift: ad).
pub fn delete(target: &str) -> Report {
    let mut report = Report::new();
    let backend = match backend() {
        Ok(b) => b,
        Err(err) => {
            report.fail(CLEANER_ID, "backend", err.to_string());
            return report;
        }
    };
    if !crate::deep::safety::is_root() {
        report.fail(CLEANER_ID, "root", "root needed to delete");
        return report;
    }
    if !valid_target(target) {
        report.fail(CLEANER_ID, "args", format!("invalid target: {target}"));
        return report;
    }
    let result = match backend {
        Backend::Snapper => run("snapper", &["-c", "root", "delete", target]),
        Backend::Timeshift => run("timeshift", &["--delete", "--snapshot", target, "--yes"]),
    };
    match result {
        Ok(_) => report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "delete",
            format!("deleted: {target}"),
            None,
            0,
        )),
        Err(err) => report.fail(CLEANER_ID, "delete", err.to_string()),
    }
    report
}

/// Sistemi anlık görüntüye geri sar (TEHLİKELİ: --yes şart).
pub fn rollback(target: &str) -> Report {
    let mut report = Report::new();
    let backend = match backend() {
        Ok(b) => b,
        Err(err) => {
            report.fail(CLEANER_ID, "backend", err.to_string());
            return report;
        }
    };
    if !crate::deep::safety::is_root() {
        report.fail(CLEANER_ID, "root", "root needed for rollback");
        return report;
    }
    if !valid_target(target) {
        report.fail(CLEANER_ID, "args", format!("invalid target: {target}"));
        return report;
    }
    let result = match backend {
        // snapper rollback = hedefi varsayılan alt hacim yapar (yeniden başlatma gerekir).
        Backend::Snapper => run("snapper", &["-c", "root", "rollback", target]),
        Backend::Timeshift => run("timeshift", &["--restore", "--snapshot", target, "--yes"]),
    };
    match result {
        Ok(_) => report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "rollback",
            format!("rolled back to: {target} — REBOOT NOW"),
            None,
            0,
        )),
        Err(err) => report.fail(CLEANER_ID, "rollback", err.to_string()),
    }
    report
}

fn jiffy() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_validated() {
        assert!(valid_target("1"));
        assert!(valid_target("my-snap_1.2"));
        assert!(!valid_target(""));
        assert!(!valid_target("."));
        assert!(!valid_target(".."));
        assert!(!valid_target("-x"));
        assert!(!valid_target(".hidden"));
        assert!(!valid_target("a/b"));
        assert!(!valid_target("a\\b"));
        assert!(!valid_target("x; rm -rf"));
    }

    #[test]
    fn desc_sanitized() {
        assert_eq!(sanitize_desc("a\nb"), "a b");
        assert!(sanitize_desc(&"x".repeat(200)).len() <= 128);
    }
}
