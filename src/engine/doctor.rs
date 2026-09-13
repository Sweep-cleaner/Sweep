//! Teşhis: "neden temizlenemiyor?" sorusuna tek komut (`sweep doctor`).
//!
//! Dört bölüm: yapılandırma, harici araçlar, yetkiler, disk/systemd.
//! Her bulgu durum + somut çözüm önerisi taşır; rapor `fail` girdileri
//! çıkış kodunu belirler (sorun varsa FAILURE).

use crate::core::report::{Entry, EntryKind, Report};

fn lang() -> crate::i18n::Lang {
    crate::i18n::Lang::detect(&[])
}

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "doctor";

fn ok(report: &mut Report, section: &str, text: String) {
    report.push(Entry::new(
        EntryKind::Command,
        CLEANER_ID,
        section,
        text,
        None,
        0,
    ));
}

fn hint(report: &mut Report, section: &str, text: String) {
    report.fail(CLEANER_ID, section, text);
}

/// Yapılandırma dosyası okunabiliyor mu?
fn check_config(report: &mut Report) {
    let path = crate::config::Config::default_path();
    if !path.exists() {
        ok(
            report,
            "config",
            crate::i18n::et(
                &lang(),
                "no config, using defaults ({})",
                &[&path.display().to_string()],
            ),
        );
        return;
    }
    match crate::config::Config::load(&path) {
        Ok(_) => ok(
            report,
            "config",
            crate::i18n::et(&lang(), "ok: {}", &[&path.display().to_string()]),
        ),
        Err(err) => hint(
            report,
            "config",
            crate::i18n::et(
                &lang(),
                "broken config {}: {} — delete it to reset to defaults",
                &[&path.display().to_string(), &err.to_string()],
            ),
        ),
    }
}

/// Beklenen harici araçlar kurulu mu?
fn check_tools(report: &mut Report) {
    const TOOLS: &[(&str, &str)] = &[
        ("journalctl", "for systemd journal cleaning"),
        ("apt-get", "for APT cache/orphan cleaning (Debian/Ubuntu)"),
        ("dnf", "for DNF cleaning (Fedora/RHEL)"),
        ("pacman", "for pacman cleaning (Arch)"),
        ("zypper", "for zypper cleaning (openSUSE)"),
        ("paccache", "for pacman version pruning (pacman-contrib)"),
        ("flatpak", "for Flatpak leftovers"),
        ("snap", "for Snap revisions"),
        ("docker", "for Docker pruning"),
        ("balooctl", "for the Baloo index (KDE)"),
        ("tracker3", "for the Tracker index (GNOME)"),
        ("deborphan", "for APT orphan detection (optional)"),
        ("curl", "for winapp2.ini download (or wget)"),
        ("systemctl", "for timer + service management"),
    ];
    for (tool, why) in TOOLS {
        if crate::fsutil::path_exists_in_path(tool) {
            ok(
                report,
                "tools",
                crate::i18n::et(&lang(), "installed: {}", &[tool]),
            );
        } else {
            hint(report, "tools", format!("{tool}: missing — {why}"));
        }
    }
}

/// Kritik yazma konumlarına erişim var mı?
fn check_permissions(report: &mut Report) {
    let root = crate::deep::safety::is_root();
    ok(
        report,
        "permissions",
        crate::i18n::et(
            &lang(),
            "running as: {}",
            &[&if root { "root" } else { "user" }.to_string()],
        ),
    );
    // Root gerektiren konumlara yazma denemesi yapmadan sahiplikten anla.
    for dir in [
        "/var/log/journal",
        "/var/cache/apt/archives",
        "/var/lib/snapd/cache",
    ] {
        let path = std::path::Path::new(dir);
        if !path.is_dir() {
            continue;
        }
        let writable =
            crate::deep::safety::check_path(path, &crate::action::RunContext::clean()).is_ok();
        if writable {
            ok(
                report,
                "permissions",
                crate::i18n::et(&lang(), "writable: {}", &[&dir.to_string()]),
            );
        } else if !root {
            hint(
                report,
                "permissions",
                crate::i18n::et(
                    &lang(),
                    "{}: needs root — related cleanings skipped (run with sudo)",
                    &[&dir.to_string()],
                ),
            );
        }
    }
}

/// Disk doluluk + systemd erişilebilirliği.
fn check_system(report: &mut Report) {
    for target in ["/", "/var", "/home"] {
        let path = std::path::Path::new(target);
        if !path.is_dir() {
            continue;
        }
        match (
            crate::platform::free_space(path),
            crate::platform::total_space(path),
        ) {
            (Ok(free), Ok(total)) if total > 0 => {
                let used = 1.0 - free as f64 / total as f64;
                let line = format!(
                    "{target}: %{:.0} full ({} free)",
                    used * 100.0,
                    crate::deep::safety::human(free)
                );
                if used > 0.9 {
                    hint(
                        report,
                        "disk",
                        format!("{line} — URGENT: free space with bigfiles/dupes"),
                    );
                } else {
                    ok(report, "disk", line);
                }
            }
            _ => hint(
                report,
                "disk",
                crate::i18n::et(&lang(), "{}: cannot read space", &[&target.to_string()]),
            ),
        }
    }
    if cfg!(target_os = "linux") {
        if crate::fsutil::path_exists_in_path("systemctl") {
            match crate::fsutil::run_command("systemctl", &["--user", "is-system-running"], true) {
                Ok((_, out, _)) => ok(report, "systemd", format!("user manager: {}", out.trim())),
                Err(err) => hint(report, "systemd", format!("query failed: {err}")),
            }
        }
    } else {
        ok(
            report,
            "systemd",
            "no systemd on this platform (normal)".into(),
        );
    }
}

/// Tam teşhis raporu.
pub fn scan() -> Report {
    let mut report = Report::new();
    check_config(&mut report);
    check_tools(&mut report);
    check_permissions(&mut report);
    check_system(&mut report);
    report
}
