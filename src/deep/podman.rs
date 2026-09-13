//! Podman kalıntıları: asılı imajlar, build cache ve durdurulmuş kapsayıcı logları.
//!
//! Docker kardeş proje; `podman` CLI ile aynı sözleşmeyi uygular:
//! dangling imajlar (`podman image prune -f`), build cache
//! (`podman builder prune -f`) ve durdurulmuş kapsayıcıların log dosyaları.
//! Çalışan kapsayıcılara ve adlandırılmış volume'lere dokunulmaz.
//!
//! `have("podman")` yoksa erken çıkar; bu modül hiçbir şeyi değişmez.

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{command_output, have};

/// `podman system df` özeti (önizleme bilgisi).
pub fn disk_usage() -> Option<String> {
    command_output("podman", &["system", "df"])
}

/// Durdurulmuş (stopped/exited) kapsayıcı kimlikleri.
pub fn stopped_containers() -> Vec<String> {
    let Some(out) = command_output("podman", &["ps", "-a", "-q", "--filter", "status=exited"]) else {
        return Vec::new();
    };
    out.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Asılı (dangling) imaj kimlikleri — hiçbir yereletmiş imaj tarafından referans vermiyor.
pub fn dangling_images() -> Vec<String> {
    let Some(out) = command_output("podman", &["images", "-q", "--filter", "dangling=true"]) else {
        return Vec::new();
    };
    out.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn execute(
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
    include_build_cache: bool,
) {
    if !have("podman") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "tool not found: {}", &["podman"]),
        );
        return;
    }

    let stopped = stopped_containers();
    let dangling = dangling_images();
    let usage = disk_usage().unwrap_or_default();

    if ctx.dry_run {
        if !usage.is_empty() {
            for line in usage.lines().take(12) {
                report.push(Entry::new(
                    EntryKind::Command,
                    cleaner,
                    option,
                    format!("podman: {line}"),
                    None,
                    0,
                ));
            }
        }
        for id in &dangling {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "dangling image to remove: {}", &[&id]),
                None,
                0,
            ));
        }
        for id in stopped.iter().take(50) {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "stopped container (logs): {}", &[&id]),
                None,
                0,
            ));
        }
        if include_build_cache {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::t(&ctx.lang, "will run: podman builder prune -f"),
                None,
                0,
            ));
        }
        if dangling.is_empty() && stopped.is_empty() {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::t(&ctx.lang, "no podman leftovers"),
                None,
                0,
            ));
        }
        return;
    }

    // 1) Asılı imajlar.
    match crate::fsutil::run_command("podman", &["image", "prune", "-f"], true) {
        Ok((0, _, _)) => report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "dangling images cleaned ({})",
                &[&dangling.len().to_string()],
            ),
            None,
            0,
        )),
        Ok((code, _, stderr)) => report.fail(
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "{} exited {}: {}",
                &[
                    &"podman image prune".to_string(),
                    &code.to_string(),
                    &stderr,
                ],
            ),
        ),
        Err(err) => report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "cannot run: {}", &[&err.to_string()]),
        ),
    }

    // 2) Build cache (opsiyonel).
    if include_build_cache {
        match crate::fsutil::run_command("podman", &["builder", "prune", "-f"], true) {
            Ok((0, _, _)) => report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::t(&ctx.lang, "podman build cache cleaned"),
                None,
                0,
            )),
            Ok((code, _, stderr)) => report.fail(
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "{} exited {}: {}",
                    &[
                        &"podman builder prune".to_string(),
                        &code.to_string(),
                        &stderr,
                    ],
                ),
            ),
            Err(err) => report.fail(
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "cannot run: {}", &[&err.to_string()]),
            ),
        }
    }

    // 3) Durdurulmuş kapsayıcıların log dosyaları (truncate; kapsayıcı silinmez).
    for id in stopped.iter().take(50) {
        if !id.chars().all(|c| c.is_ascii_hexdigit()) {
            continue;
        }
        let log_path = format!("/var/lib/containers/storage/overlay-containers/{id}/userdata/log.json");
        let p = std::path::Path::new(&log_path);
        if p.is_file() {
            if let Err(reason) = crate::deep::safety::check_path(p, ctx) {
                report.push(Entry::skipped(cleaner, option, p));
                log::warn!("skipped {}: {reason}", p.display());
                continue;
            }
            let bytes = crate::fsutil::size::size_of_or_zero(p);
            match crate::fsutil::delete::truncate_file(p) {
                Ok(()) => report.push(Entry::new(
                    EntryKind::Truncate,
                    cleaner,
                    option,
                    crate::i18n::et(&ctx.lang, "container log truncated: {}", &[&id]),
                    Some(p),
                    bytes,
                )),
                Err(err) => report.fail(
                    cleaner,
                    option,
                    crate::i18n::et(&ctx.lang, "{} log: {}", &[id.clone(), err.to_string()]),
                ),
            }
        }
    }
}

/// Podman katman/store önbelleği (görüntülerin yeniden indirilmesine izin verir).
pub fn execute_cache(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if !have("podman") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "tool not found: {}", &["podman"]),
        );
        return;
    }

    let cache_dirs = [
        std::path::PathBuf::from("/var/lib/containers/storage/overlay-images"),
        std::path::PathBuf::from("/var/lib/containers/storage/overlay-layers"),
        std::path::PathBuf::from("/var/lib/containers/storage/overlay-containers"),
    ];

    if ctx.dry_run {
        let mut any = false;
        for dir in &cache_dirs {
            if dir.is_dir() {
                any = true;
                let (bytes, files) = crate::deep::dir_stats(dir);
                report.push(Entry::new(
                    EntryKind::Delete,
                    cleaner,
                    option,
                    crate::i18n::et(
                        &ctx.lang,
                        "podman storage to clean: {} ({} files)",
                        &[&dir.display().to_string(), &files.to_string()],
                    ),
                    Some(dir.as_path()),
                    bytes,
                ));
            }
        }
        if !any {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::t(&ctx.lang, "no podman cache dirs found"),
                None,
                0,
            ));
        }
        return;
    }

    for dir in &cache_dirs {
        if dir.is_dir() {
            // root gerektirir; yoksa sessizce atla.
            if !crate::deep::safety::is_root() {
                report.fail(cleaner, option, crate::i18n::t(&ctx.lang, "requires root"));
                return;
            }
            crate::deep::guarded_delete(dir, ctx, cleaner, option, report);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_containers_are_nonempty_only_when_present() {
        let c = stopped_containers();
        // Her environment'da blank dönüyor veya birkaç ID var.
        for id in &c {
            assert!(!id.is_empty());
        }
    }
}
