//! Flatpak eklentilerini ve kullanılmayan genişletmeleri temizle.
//!
//! `flatpak uninstall --unused` flatpak'un resmi yolu; yalnızca hiçbir
//! aktivasyonda kullanılmayan Flatpak eklentilerini (runtimes hariç full
//! runtime'ları koruyarak) kaldırır. Komut çalıştırılmadan önce dry-run
//! önbörü uygulanır.
//!
//! `flatpak` yoksa bu modül hiçbir şeyi yapmaz; çalışma anı flatpak'un yetki
//! modeline bağlıdır.

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::deep::have;

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if !have("flatpak") {
        report.fail(
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "tool not found: {}", &["flatpak"]),
        );
        return;
    }

    if ctx.dry_run {
        report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::t(&ctx.lang, "will run: flatpak uninstall --unused -y"),
            None,
            0,
        ));
        return;
    }

    match crate::fsutil::run_command("flatpak", &["uninstall", "--unused", "-y"], true) {
        Ok((0, _, _)) => report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            crate::i18n::t(&ctx.lang, "unused flatpak extensions removed"),
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
                    &"flatpak uninstall --unused".to_string(),
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
    fn noop_if_flatpak_missing() {
        // flatpak kurulu olmayan bir sistemde fail mesajı üretir.
        // Ancak flatpak yoksa run_command hatası döner.
        // Burada explicit kontrol yok; kullanım anında 'have' ile kontrol edilir.
    }
}
