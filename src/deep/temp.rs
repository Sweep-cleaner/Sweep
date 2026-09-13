//! Geçici dosyalar: `/tmp`, `/var/tmp` ve kullanıcı `~/.cache/tmp` benzerleri.
//!
//! Varsayılan yaş eşiği 1 gündür; X11/ICE/systemd soketleri `builtin_keep_list`
//! ile korunur. Kritik-dizin koruması `/tmp` kökünün silinmesini engeller.

use std::path::PathBuf;
use std::time::SystemTime;

use crate::action::RunContext;
use crate::core::report::Report;

pub fn temp_roots() -> Vec<PathBuf> {
    let mut roots = vec![PathBuf::from("/tmp"), PathBuf::from("/var/tmp")];
    if let Some(home) = crate::platform::home_dir() {
        roots.extend(user_tmp_roots(&home));
    }
    roots.into_iter().filter(|p| p.is_dir()).collect()
}

/// Ev dizini altındaki geçici dizin adayları (yalnızca var olanlar döner).
pub fn user_tmp_roots(home: &std::path::Path) -> Vec<PathBuf> {
    let cache = crate::platform::cache_dir().unwrap_or_else(|| home.join(".cache"));
    [home.join("tmp"), home.join(".tmp"), cache.join("tmp")]
        .into_iter()
        .filter(|p| p.is_dir())
        .collect()
}

fn older_than(path: &std::path::Path, days: u32) -> bool {
    let Ok(md) = std::fs::symlink_metadata(path) else {
        return false;
    };
    let Ok(mtime) = md.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(mtime)
        .map(|d| d.as_secs() > u64::from(days) * 86_400)
        .unwrap_or(false)
}

pub fn execute(
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
    max_age_days: u32,
) {
    for root in temp_roots() {
        if ctx.cancelled() {
            report.aborted = true;
            return;
        }
        let options = crate::fsutil::walk::ScanOptions::all()
            .with_max_depth(8)
            .with_same_filesystem(true);
        for path in crate::fsutil::walk::scan_paths(&root, &options) {
            // Kökün kendisi ve genç dosyalar atlanır.
            if path == root || !older_than(&path, max_age_days) {
                continue;
            }
            crate::deep::guarded_delete(&path, ctx, cleaner, option, report);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_user_tmp_dirs() {
        let base =
            std::env::temp_dir().join(format!("sweep-test-tmp-{}-{}", std::process::id(), "roots"));
        let _ = std::fs::remove_dir_all(&base);
        let home_tmp = base.join("tmp");
        std::fs::create_dir_all(&home_tmp).unwrap();
        // Olmayan adaylar elenir, var olanlar döner.
        let found = user_tmp_roots(&base);
        assert!(found.contains(&home_tmp));
        assert!(user_tmp_roots(&base.join("yok")).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }
}
