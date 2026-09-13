//! Time Machine yerel anlık görüntüleri: `tmutil thinlocalsnapshots`.
//!
//! Yerel snapshot'lar diski sessizce doldurur; inceltme resmi yoldur.
//! Önizleme `tmutil listlocalsnapshots /` ile salt-okunur yapılır.
//! Yalnızca macOS'te anlamlıdır (temizleyici `os = "macos"` ile kapılıdır).

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{command_output, have};

/// Yerel snapshot tarihleri — salt-okunur liste, silmez.
pub fn local_snapshots() -> Vec<String> {
    let Some(out) = command_output("tmutil", &["listlocalsnapshots", "/"]) else {
        return Vec::new();
    };
    out.lines()
        .filter_map(|l| l.split('.').nth_back(1).map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if !cfg!(target_os = "macos") {
        report.fail(cleaner, option, "Time Machine snapshots need macOS");
        return;
    }
    if !have("tmutil") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "tool not found: {}", &["tmutil"]),
        );
        return;
    }
    let snaps = local_snapshots();
    if ctx.dry_run {
        if snaps.is_empty() {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                "no Time Machine local snapshots".to_string(),
                None,
                0,
            ));
            return;
        }
        report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            format!(
                "{} local snapshots ({}); will run: tmutil thinlocalsnapshots / 2g",
                snaps.len(),
                snaps.join(", ")
            ),
            None,
            0,
        ));
        return;
    }

    match crate::fsutil::run_command("tmutil", &["thinlocalsnapshots", "/", "2g"], true) {
        Ok((0, _, _)) => report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            format!("thinned {} local snapshots", snaps.len()),
            None,
            0,
        )),
        Ok((code, _, stderr)) => report.fail(
            cleaner,
            option,
            format!("tmutil exited {code}: {}", stderr.trim()),
        ),
        Err(err) => report.fail(cleaner, option, err.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_lines_parsed() {
        // `tmutil listlocalsnapshots /` çıktısı: `com.apple.TimeMachine.2024-…local`
        let line = "com.apple.TimeMachine.2024-01-02-030405.local";
        let date = line.split('.').nth_back(1).unwrap().trim();
        assert_eq!(date, "2024-01-02-030405");
        // Dışı (boş çıktı) güvenle boş listedir.
        let _ = local_snapshots();
    }
}
