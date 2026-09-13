//! Öksüz (yetim) paketler: bağımlılığı kalmamış, otomatik kurulmuş paketler.
//!
//! Güvenlik notu: kaldırma işlemi dağıtımın kendi aracıyla ve yalnızca
//! kullanıcı onayıyla yapılır; bu modül önce `--dry-run` çıktısını ayrıştırıp
//! önizleme üretir, gerçek silmede tek tek paket adı geçirir.

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{command_output, have};

/// Öksüz paket adları (dağıtıma göre).
pub fn orphan_packages() -> Vec<String> {
    // APT: deborphan varsa onu, yoksa autoremove --dry-run çıktısını kullan.
    if have("deborphan") {
        if let Some(out) = command_output("deborphan", &[]) {
            let list: Vec<String> = out
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            if !list.is_empty() {
                return list;
            }
        }
    }
    if have("apt-get") {
        if let Some(out) = command_output("apt-get", &["autoremove", "--dry-run", "-y"]) {
            return out
                .lines()
                .filter(|l| l.trim_start().starts_with("Remv "))
                .filter_map(|l| l.split_whitespace().nth(1))
                .map(|s| s.to_string())
                .collect();
        }
    }
    // DNF: `dnf autoremove --setopt=assumeno` provası. Paket adları
    // "Removing:" başlığından SONRAKİ satırlardadır; başlığın kendisi asla
    // paket değildir.
    if have("dnf") {
        if let Some(out) = command_output("dnf", &["autoremove", "--setopt=assumeno=True", "-y"]) {
            let mut pkgs = Vec::new();
            let mut capture = false;
            for line in out.lines() {
                let trimmed = line.trim();
                if trimmed.contains("Removing:")
                    || trimmed == "Removing"
                    || trimmed.starts_with("Removing ")
                {
                    capture = true;
                    continue;
                }
                if !capture || pkgs.len() >= 200 {
                    continue;
                }
                if trimmed.is_empty()
                    || trimmed.ends_with(':')
                    || trimmed.starts_with("Transaction")
                    || trimmed.starts_with("Operation")
                {
                    break;
                }
                if let Some(name) = trimmed.split_whitespace().next() {
                    pkgs.push(name.to_string());
                }
            }
            return pkgs;
        }
    }
    // Pacman: açıkça kurulmamış + gerekli olmayanlar.
    if have("pacman") {
        if let Some(out) = command_output("pacman", &["-Qdtq"]) {
            return out
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
        }
    }
    // Zypper (openSUSE): `zypper packages --unneeded`; yalnızca kurulu
    // (`i` durumu) ve `package` türündeki satırlar alınır.
    if have("zypper") {
        if let Some(out) = command_output("zypper", &["packages", "--unneeded"]) {
            let mut pkgs = Vec::new();
            for line in out.lines() {
                let cols: Vec<&str> = line.split('|').map(|s| s.trim()).collect();
                if cols.len() >= 3 && cols[0] == "i" && cols[2] == "package" {
                    if !cols[1].is_empty() {
                        pkgs.push(cols[1].to_string());
                    }
                }
            }
            if !pkgs.is_empty() {
                return pkgs;
            }
        }
    }
    Vec::new()
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    let orphans = orphan_packages();
    if ctx.dry_run {
        if orphans.is_empty() {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::t(&ctx.lang, "no orphan packages"),
                None,
                0,
            ));
        }
        for pkg in &orphans {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "orphan package to remove: {}", &[&pkg]),
                None,
                0,
            ));
        }
        return;
    }
    if orphans.is_empty() {
        return;
    }
    if !crate::deep::safety::is_root() {
        report.fail(cleaner, option, crate::i18n::t(&ctx.lang, "requires root"));
        return;
    }
    for chunk in orphans.chunks(16) {
        let refs: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
        let status = if have("apt-get") {
            let mut args = vec!["autoremove", "-y"];
            args.extend(refs.iter().copied());
            crate::fsutil::run_command("apt-get", &args, true)
        } else if have("dnf") {
            let mut args = vec!["autoremove", "-y"];
            args.extend(refs.iter().copied());
            crate::fsutil::run_command("dnf", &args, true)
        } else if have("pacman") {
            let mut args = vec!["-Rns", "--noconfirm"];
            args.extend(refs.iter().copied());
            crate::fsutil::run_command("pacman", &args, true)
        } else if have("zypper") {
            let mut args = vec!["remove", "-y"];
            args.extend(refs.iter().copied());
            crate::fsutil::run_command("zypper", &args, true)
        } else {
            report.fail(
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "tool not found: {}", &["package manager"]),
            );
            return;
        };
        match status {
            Ok((0, _, _)) => {
                for pkg in chunk {
                    report.push(Entry::new(
                        EntryKind::Command,
                        cleaner,
                        option,
                        crate::i18n::et(&ctx.lang, "removed: {}", &[&pkg]),
                        None,
                        0,
                    ));
                    crate::deep::safety::log_operation(
                        ctx,
                        cleaner,
                        option,
                        std::path::Path::new(pkg),
                        0,
                        false,
                    );
                }
            }
            Ok((code, _, stderr)) => report.fail(
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "{} exited {}: {}",
                    &[&"orphan remove".to_string(), &code.to_string(), &stderr],
                ),
            ),
            Err(err) => report.fail(
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "cannot run: {}", &[&err.to_string()]),
            ),
        }
    }
}
