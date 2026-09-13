//! Nix store çöp toplama: ulaşılamaz store yolları + eski nesiller.
//!
//! `nix-collect-garbage -d` resmi yoldur: yalnızca hiçbir profil/nesil
//! tarafından referans verilmeyen store yollarını siler ve tüm profillerin
//! eski nesillerini kaldırır. Kullanımdaki hiçbir şeye dokunmaz, kök yetkisi
//! gerekmez (daemon kurulu tek-kullanıcı kurulumda yeterlidir). Önizleme
//! `nix-store --gc --print-dead` ile salt-okunur yapılır.

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{command_output, have};

/// Ulaşılamaz (dead) store yolları — salt-okunur liste, silmez.
pub fn dead_paths() -> Vec<String> {
    let Some(out) = command_output("nix-store", &["--gc", "--print-dead"]) else {
        return Vec::new();
    };
    out.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| l.starts_with("/nix/store/"))
        .collect()
}

/// Store'da kaç nesil varsa özet bilgi (yalnızca bilgi amaçlı).
pub fn generation_count() -> Option<String> {
    command_output("nix-env", &["--list-generations"])
        .map(|out| out.lines().count().to_string())
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if !have("nix-store") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "tool not found: {}", &["nix-store (Nix?)"]),
        );
        return;
    }
    let dead = dead_paths();
    let gens = generation_count();

    if ctx.dry_run {
        if dead.is_empty() {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::t(&ctx.lang, "no dead nix store paths"),
                None,
                0,
            ));
            return;
        }
        report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "{} dead store paths to collect",
                &[&dead.len().to_string()],
            ),
            None,
            0,
        ));
        report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::t(&ctx.lang, "will run: nix-collect-garbage -d"),
            None,
            0,
        ));
        if let Some(n) = gens {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "{} profile generations (old ones will be removed)",
                    &[&n],
                ),
                None,
                0,
            ));
        }
        return;
    }

    match crate::fsutil::run_command("nix-collect-garbage", &["-d"], true) {
        Ok((0, _, _)) => report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "nix store garbage collected ({} dead paths)",
                &[&dead.len().to_string()],
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
                    &"nix-collect-garbage".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dead_paths_are_store_paths_only() {
        // nix yoksa boş liste; kuruluysa çıktı yalnızca /nix/store ile başlar.
        let paths = dead_paths();
        for p in &paths {
            assert!(p.starts_with("/nix/store/"));
        }
    }
}