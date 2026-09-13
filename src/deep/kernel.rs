//! Eski çekirdekler: çalışan çekirdek ASLA silinmez.
//!
//! * Debian/Ubuntu: `dpkg --list linux-image-*` + `apt-get purge`;
//! * Fedora/RHEL: `rpm -q kernel` + `dnf remove` (installonly_limit'e saygı
//!   için en az 2 çekirdek tutulur);
//! * Arch: yuvarlanan sürüm — `/boot`'ta yetim `vmlinuz-*` aranır.

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::{command_output, have};

/// Çalışan çekirdek sürümü (`uname -r`).
pub fn running_kernel() -> String {
    command_output("uname", &["-r"])
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Kaldırılabilir eski çekirdek paketleri (çalışan + en yeni hariç).
pub fn old_kernels() -> Vec<String> {
    let running = running_kernel();
    let mut kernels: Vec<String> = Vec::new();

    if have("dpkg") {
        kernels.extend(crate::deep::apt::installed_kernels());
    }
    // rpm tabanlı dağıtımlar: Fedora (`kernel`, `kernel-core`),
    // openSUSE (`kernel-default`, `kernel-default-base`, `kernel-64k`).
    // Kurulu olmayan adlar için rpm "is not installed" döner — elenir.
    if have("rpm") {
        for name in [
            "kernel",
            "kernel-core",
            "kernel-default",
            "kernel-default-base",
            "kernel-64k",
        ] {
            if let Some(out) = command_output("rpm", &["-q", name]) {
                kernels.extend(
                    out.lines()
                        .filter(|l| !l.contains("is not installed"))
                        .map(|s| s.trim().to_string()),
                );
            }
        }
    }

    // Çalışan çekirdeği ve en yeni sürümü ele.
    kernels.retain(|k| !k.is_empty() && !running.is_empty() && !k.contains(&running));
    kernels.sort();
    kernels.dedup();
    // En yeni kalanı da tut (geri dönüş çekirdeği).
    if !kernels.is_empty() {
        kernels.pop();
    }
    kernels
}

/// Yetim /boot kalıntıları (paket yöneticisine kayıtlı olmayan vmlinuz/initrd).
pub fn orphan_boot_files() -> Vec<std::path::PathBuf> {
    let boot = std::path::Path::new("/boot");
    if !boot.is_dir() {
        return Vec::new();
    }
    let running = running_kernel();
    // Çalışan sürüm bilinmiyorsa hiçbir şeyi yetim sayma: boş `running` her
    // ada uyar ve çalışan çekirdek dahil her şey silinirdi.
    if running.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(boot) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let is_kernel_artifact = name.starts_with("vmlinuz-")
                || name.starts_with("initrd.img-")
                || name.starts_with("initramfs-")
                || name.starts_with("System.map-")
                || name.starts_with("config-");
            if is_kernel_artifact && !name.contains(&running) {
                out.push(entry.path());
            }
        }
    }
    out
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    let running = running_kernel();
    let olds = old_kernels();
    let orphans = orphan_boot_files();

    if ctx.dry_run {
        if olds.is_empty() && orphans.is_empty() {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "no old kernels (running: {})", &[&running]),
                None,
                0,
            ));
            return;
        }
        for k in &olds {
            report.push(Entry::new(
                EntryKind::Command,
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "old kernel to remove: {} (running: {})",
                    &[k.clone(), running.clone()],
                ),
                None,
                0,
            ));
        }
        for p in &orphans {
            let bytes = crate::fsutil::size::size_of_or_zero(p);
            report.push(Entry::new(
                EntryKind::Delete,
                cleaner,
                option,
                crate::i18n::et(
                    &ctx.lang,
                    "orphan boot file {}",
                    &[&p.display().to_string()],
                ),
                Some(p),
                bytes,
            ));
        }
        return;
    }

    if !crate::deep::safety::is_root() {
        report.fail(cleaner, option, crate::i18n::t(&ctx.lang, "requires root"));
        return;
    }
    // Paketli çekirdekleri dağıtım aracıyla kaldır.
    for chunk in olds.chunks(8) {
        let status = if have("apt-get") {
            let mut args = vec!["purge", "-y"];
            let refs: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
            args.extend(refs);
            crate::fsutil::run_command("apt-get", &args, true)
        } else if have("dnf") {
            let mut args = vec!["remove", "-y"];
            let refs: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
            args.extend(refs);
            crate::fsutil::run_command("dnf", &args, true)
        } else if have("zypper") {
            // openSUSE: `zypper remove -y <kernel>`; sürücü/bağımlılık soruları
            // için otomatik onay verilir.
            let mut args = vec!["remove", "-y"];
            let refs: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
            args.extend(refs);
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
                for k in chunk {
                    report.push(Entry::new(
                        EntryKind::Command,
                        cleaner,
                        option,
                        crate::i18n::et(&ctx.lang, "removed: {}", &[&k]),
                        None,
                        0,
                    ));
                    crate::deep::safety::log_operation(
                        ctx,
                        cleaner,
                        option,
                        std::path::Path::new(k),
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
                    &[&"kernel remove".to_string(), &code.to_string(), &stderr],
                ),
            ),
            Err(err) => report.fail(
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "cannot run: {}", &[&err.to_string()]),
            ),
        }
    }
    // Yetim boot dosyalarını korumalı sil.
    for p in &orphans {
        crate::deep::guarded_delete(p, ctx, cleaner, option, report);
    }
}
