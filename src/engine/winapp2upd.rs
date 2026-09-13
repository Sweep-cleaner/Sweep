//! winapp2.ini güncelleyici: topluluk kurallarını indirip doğrular.
//!
//! Yeni bağımlılık yok — sistemdeki `curl`/`wget` kullanılır. Dosya,
//! `--winapp2` bayrağının zaten taradığı `<config>/cleaners/winapp2.ini`
//! konumuna yazılır.

use std::path::PathBuf;

use crate::action::context::RunContext;
use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "update-winapp2";

/// Varsayılan kaynak (Winapp2 topluluk deposu).
pub const DEFAULT_URL: &str =
    "https://raw.githubusercontent.com/MoscaDotTo/Winapp2/master/Winapp2.ini";

/// Hedef dosya: `<config>/cleaners/winapp2.ini`.
pub fn dest_file() -> Option<PathBuf> {
    crate::platform::config_dir().map(|d| d.join("cleaners").join("winapp2.ini"))
}

/// İndir, doğrula, raporla.
pub fn scan(url: &str, ctx: &RunContext) -> Report {
    let mut report = Report::new();
    let Some(dest) = dest_file() else {
        report.fail(CLEANER_ID, "dest", "no config dir");
        return report;
    };

    if ctx.dry_run {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "download",
            format!("to download: {url} -> {}", dest.display()),
            Some(&dest),
            0,
        ));
        return report;
    }

    let downloader = if crate::fsutil::path_exists_in_path("curl") {
        ("curl", vec!["-fsSL", "-o"])
    } else if crate::fsutil::path_exists_in_path("wget") {
        ("wget", vec!["-q", "-O"])
    } else {
        report.fail(CLEANER_ID, "download", "no curl/wget");
        return report;
    };

    if let Some(parent) = dest.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            report.fail(
                CLEANER_ID,
                "dest",
                format!("cannot create dir: {}", parent.display()),
            );
            return report;
        }
    }
    let tmp = dest.with_extension("ini.tmp");
    let tmp_s = tmp.to_string_lossy().into_owned();
    let args: Vec<&str> = downloader
        .1
        .iter()
        .copied()
        .chain([tmp_s.as_str(), url])
        .collect();
    match crate::fsutil::run_command(downloader.0, &args, true) {
        Ok((0, _, _)) => {}
        Ok((code, _, stderr)) => {
            report.fail(
                CLEANER_ID,
                "download",
                format!("{} exit {code}: {stderr}", downloader.0),
            );
            return report;
        }
        Err(err) => {
            report.fail(CLEANER_ID, "download", format!("download failed: {err}"));
            return report;
        }
    }

    // Doğrulama: ayrıştırılamayan dosya eskisinin üstüne yazılmaz.
    let results = crate::definition::winapp2::load_winapp2_file(
        &tmp,
        crate::definition::model::Trust::Untrusted,
    );
    let ok = results.iter().filter(|r| r.is_ok()).count();
    if ok == 0 {
        let _ = std::fs::remove_file(&tmp);
        report.fail(CLEANER_ID, "validate", "no valid rules in download");
        return report;
    }
    if std::fs::rename(&tmp, &dest).is_err() {
        report.fail(CLEANER_ID, "dest", "cannot write file");
        return report;
    }
    let bytes = crate::fsutil::size::size_of_or_zero(&dest);
    report.push(Entry::new(
        EntryKind::Command,
        CLEANER_ID,
        "download",
        format!("updated: {ok} rules ({})", dest.display()),
        Some(&dest),
        bytes,
    ));
    report
}
