//! Docker kalıntıları: asılı imajlar, durdurulmuş kapsayıcılar, build cache.
//!
//! Varsayılan olarak YALNIZCA güvenli alt kümeler hedeflenir:
//! dangling imajlar (`docker image prune -f`), build cache
//! (`docker builder prune -f`) ve durdurulmuş kapsayıcı logları.
//! Çalışan kapsayıcılara ve adlandırılmış volume'lere dokunulmaz.

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{command_output, have};

/// `docker system df` özeti (önizleme bilgisi).
pub fn disk_usage() -> Option<String> {
    command_output("docker", &["system", "df"])
}

/// Durdurulmuş kapsayıcı kimlikleri.
pub fn stopped_containers() -> Vec<String> {
    let Some(out) = command_output("docker", &["ps", "-a", "-q", "--filter", "status=exited"])
    else {
        return Vec::new();
    };
    out.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Asılı (dangling) imaj kimlikleri.
pub fn dangling_images() -> Vec<String> {
    let Some(out) = command_output("docker", &["images", "-q", "--filter", "dangling=true"]) else {
        return Vec::new();
    };
    out.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Kullanılmayan (dangling) volume'ler: hiçbir kapsayıcı referans vermiyor.
pub fn dangling_volumes() -> Vec<String> {
    let Some(out) = command_output("docker", &["volume", "ls", "-q", "-f", "dangling=true"]) else {
        return Vec::new();
    };
    out.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Yalnızca dangling volume'leri kaldır (`docker volume prune -f`).
///
/// `volume prune` varsayılan olarak yalnızca hiçbir kapsayıcının referans
/// vermediği volume'leri siler; adlandırılmış ve bağlı veri volume'leri
/// dokunulmadan kalır.
pub fn execute_volumes(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if !have("docker") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "tool not found: {}", &["docker"]),
        );
        return;
    }
    let volumes = dangling_volumes();
    if ctx.dry_run {
        if volumes.is_empty() {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::t(&ctx.lang, "no dangling docker volumes"),
                None,
                0,
            ));
            return;
        }
        for v in &volumes {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "dangling volume to remove: {}", &[&v]),
                None,
                0,
            ));
        }
        return;
    }
    match crate::fsutil::run_command("docker", &["volume", "prune", "-f"], true) {
        Ok((0, _, _)) => report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "docker volumes pruned ({})",
                &[&volumes.len().to_string()],
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
                    &"docker volume prune".to_string(),
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

pub fn execute(
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
    include_build_cache: bool,
) {
    if !have("docker") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "tool not found: {}", &["docker"]),
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
                    format!("docker: {line}"),
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
                crate::i18n::t(&ctx.lang, "will run: docker builder prune -f"),
                None,
                0,
            ));
        }
        if dangling.is_empty() && stopped.is_empty() {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::t(&ctx.lang, "no docker leftovers"),
                None,
                0,
            ));
        }
        return;
    }

    // 1) Asılı imajlar.
    match crate::fsutil::run_command("docker", &["image", "prune", "-f"], true) {
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
                    &"docker image prune".to_string(),
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
        match crate::fsutil::run_command("docker", &["builder", "prune", "-f"], true) {
            Ok((0, _, _)) => report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::t(&ctx.lang, "docker build cache cleaned"),
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
                        &"docker builder prune".to_string(),
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
        let log_path = format!("/var/lib/docker/containers/{id}/{id}-json.log");
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
