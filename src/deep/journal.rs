//! Systemd journal + klasik `/var/log` kalıntıları.
//!
//! `journalctl --vacuum-size` / `--vacuum-time` kullanılır; boyut önizlemesi
//! `/var/log/journal` dizin boyutundan hesaplanır.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{command_output, have};

pub fn journal_dir() -> Option<PathBuf> {
    let p = PathBuf::from("/var/log/journal");
    p.is_dir().then_some(p)
}

pub fn journal_size() -> u64 {
    journal_dir()
        .map(|d| crate::fsutil::size::dir_size(&d))
        .unwrap_or(0)
}

/// `journalctl --disk-usage` çıktısındaki bayt (örn. "1.2G").
pub fn journal_disk_usage() -> Option<String> {
    let out = command_output("journalctl", &["--disk-usage"])?;
    out.lines().next().map(|s| s.trim().to_string())
}

/// vacuum_size: "500M" gibi; vacuum_time: "7d" gibi. Boşsa varsayılan uygulanır.
pub fn execute(
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
    vacuum_size: &str,
    vacuum_days: u32,
) {
    if !have("journalctl") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "tool not found: {}", &["journalctl (systemd?)"]),
        );
        return;
    }
    let before = journal_size();
    let usage = journal_disk_usage().unwrap_or_default();
    if ctx.dry_run {
        report.push(Entry::new(
            EntryKind::Vacuum,
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "will run: journalctl --vacuum-size={} --vacuum-time={}d (now {}, ~{})",
                &[
                    &vacuum_size.to_string(),
                    &vacuum_days.to_string(),
                    &usage,
                    &crate::deep::safety::human(before),
                ],
            ),
            journal_dir().as_deref(),
            before,
        ));
        // Rotated klasik loglar da önizlensin.
        for p in crate::platform::rotated_logs() {
            report.push(Entry::new(
                EntryKind::Delete,
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "rotated log {}", &[&p.display().to_string()]),
                Some(&p),
                crate::fsutil::size::size_of_or_zero(&p),
            ));
        }
        return;
    }
    if !crate::deep::safety::is_root() {
        report.fail(cleaner, option, crate::i18n::t(&ctx.lang, "requires root"));
        return;
    }
    let size_arg = format!("--vacuum-size={vacuum_size}");
    let time_arg = format!("--vacuum-time={vacuum_days}d");
    for args in [
        &[size_arg.as_str()] as &[&str],
        &[time_arg.as_str()] as &[&str],
    ] {
        match crate::fsutil::run_command("journalctl", args, true) {
            Ok((0, _, _)) => {}
            Ok((code, _, stderr)) => {
                report.fail(
                    cleaner,
                    option,
                    crate::i18n::et(
                        &ctx.lang,
                        "{} exited {}: {}",
                        &[
                            &format!("journalctl {}", args.join(" ")),
                            &code.to_string(),
                            &stderr,
                        ],
                    ),
                );
                return;
            }
            Err(err) => {
                report.fail(
                    cleaner,
                    option,
                    crate::i18n::et(&ctx.lang, "cannot run: {}", &[&err.to_string()]),
                );
                return;
            }
        }
    }
    let after = journal_size();
    let freed = before.saturating_sub(after);
    report.push(Entry::new(
        EntryKind::Vacuum,
        cleaner,
        option,
        crate::i18n::et(
            &ctx.lang,
            "systemd journal cleaned ({} reclaimed)",
            &[&crate::deep::safety::human(freed)],
        ),
        None,
        freed,
    ));
    crate::deep::safety::log_operation(
        ctx,
        cleaner,
        option,
        std::path::Path::new("/var/log/journal"),
        freed,
        false,
    );
}

/// Kullanıcı journal'ını vacuum'la (`journalctl --user ...`). Kök yetkisi
/// GEREKMEZ — sistem vacuum'undan farklı olarak kullanıcının kendi journal
/// deposu hedeflenir.
pub fn execute_user(
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
    vacuum_size: &str,
    vacuum_days: u32,
) {
    if !have("journalctl") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "tool not found: {}", &["journalctl (systemd?)"]),
        );
        return;
    }
    let usage = command_output("journalctl", &["--user", "--disk-usage"]).unwrap_or_default();
    if ctx.dry_run {
        report.push(Entry::new(
            EntryKind::Vacuum,
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "will run: journalctl --user --vacuum-size={} --vacuum-time={}d (now {})",
                &[
                    &vacuum_size.to_string(),
                    &vacuum_days.to_string(),
                    &usage,
                ],
            ),
            None,
            0,
        ));
        return;
    }
    let size_arg = format!("--vacuum-size={vacuum_size}");
    let time_arg = format!("--vacuum-time={vacuum_days}d");
    for args in [
        &[size_arg.as_str()] as &[&str],
        &[time_arg.as_str()] as &[&str],
    ] {
        let mut full: Vec<&str> = vec!["--user"];
        full.extend_from_slice(args);
        match crate::fsutil::run_command("journalctl", &full, true) {
            Ok((0, _, _)) => {}
            Ok((code, _, stderr)) => {
                report.fail(
                    cleaner,
                    option,
                    crate::i18n::et(
                        &ctx.lang,
                        "{} exited {}: {}",
                        &[
                            &format!("journalctl {}", full.join(" ")),
                            &code.to_string(),
                            &stderr,
                        ],
                    ),
                );
                return;
            }
            Err(err) => {
                report.fail(
                    cleaner,
                    option,
                    crate::i18n::et(&ctx.lang, "cannot run: {}", &[&err.to_string()]),
                );
                return;
            }
        }
    }
    report.push(Entry::new(
        EntryKind::Vacuum,
        cleaner,
        option,
        crate::i18n::t(&ctx.lang, "user systemd journal cleaned"),
        None,
        0,
    ));
    crate::deep::safety::log_operation(
        ctx,
        cleaner,
        option,
        std::path::Path::new("journalctl --user"),
        0,
        false,
    );
}
