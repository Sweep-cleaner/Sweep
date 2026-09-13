//! Linux derin temizlik kategorileri.
//!
//! Her kategori ayrı bir modüldedir; hepsi aynı sözleşmeyi uygular:
//!
//! * `preview_items()` — silmeden önce (dry-run) etiket + yol + bayt listesi;
//! * `execute()` — gerçek temizlik; her yıkıcı adım [`crate::deep::safety`]
//!   kontrollerinden (kritik dizin, symlink, yetki) ve
//!   [`RunContext`](crate::action::RunContext) içindeki `Guard`'dan geçer.
//!
//! Mevcut `preview`/`clean` akışı, `Guard`, rapor ve paralel worker düzeni
//! aynen korunur; bu modüller yalnızca `system.*` action'larının içini
//! doldurur.

pub mod apt;
pub mod caches;
pub mod crash;
pub mod cursor;
pub mod dnf;
pub mod docker;
pub mod flatpak;
pub mod flatpak_data;
pub mod flatpak_ext;
pub mod gnome;
pub mod helix;
pub mod journal;
pub mod kde;
pub mod kernel;
pub mod lxc;
pub mod managers;
pub mod nix;
pub mod orphans;
pub mod pacman;
pub mod podman;
pub mod safety;
pub mod snap;
pub mod spotify;
pub mod temp;
pub mod timemachine;
pub mod zypp;

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::fsutil::delete::{delete, DeleteOptions};

/// Kullanıcının ince ayarları (config.toml'dan okunur, CLI'da geçersiz kılınamaz).
#[derive(Debug, Clone)]
pub struct DeepOptions {
    /// journal vacuum boyutu (örn. "500M").
    pub journal_size: String,
    /// journal vacuum günü.
    pub journal_days: u32,
    /// pacman'ın tutacağı sürüm sayısı.
    pub pacman_keep: usize,
    /// temp dosyaları yaş eşiği (gün).
    pub temp_max_age_days: u32,
    /// docker build cache'i de süpürülsün mü?
    pub docker_build_cache: bool,
}

impl Default for DeepOptions {
    fn default() -> Self {
        Self {
            journal_size: "500M".into(),
            journal_days: 7,
            pacman_keep: 2,
            temp_max_age_days: 1,
            docker_build_cache: false,
        }
    }
}

impl From<&crate::config::Config> for DeepOptions {
    fn from(config: &crate::config::Config) -> Self {
        Self {
            journal_size: config.journal_size.clone(),
            journal_days: config.journal_days,
            pacman_keep: config.pacman_keep,
            temp_max_age_days: config.temp_max_age_days,
            docker_build_cache: config.docker_build_cache,
        }
    }
}

/// Dry-run önizlemesinde gösterilen tek kalem.
#[derive(Debug, Clone)]
pub struct PreviewItem {
    /// İnsan dilinde açıklama.
    pub label: String,
    /// Etkilenen yol (komut çıktısıysa `None`).
    pub path: Option<PathBuf>,
    /// Geri kazanılacak tahmini bayt.
    pub bytes: u64,
}

impl PreviewItem {
    pub fn new(label: impl Into<String>, path: Option<PathBuf>, bytes: u64) -> Self {
        Self {
            label: label.into(),
            path,
            bytes,
        }
    }
}

/// Önizleme kalemlerini rapora `Delete`/`Command` girdisi olarak işle.
pub fn push_preview(report: &mut Report, cleaner: &str, option: &str, items: Vec<PreviewItem>) {
    for item in items {
        let kind = if item.path.is_some() {
            EntryKind::Delete
        } else {
            EntryKind::Command
        };
        report.push(Entry::new(
            kind,
            cleaner,
            option,
            item.label,
            item.path.as_deref(),
            item.bytes,
        ));
    }
}

/// Korumalı silme: kritik-dizin + Guard + symlink + yetki kontrolleri.
///
/// Dizinler önce içten boşaltılır (`delete()` boş dizin ister); symlink
/// kendisi silinir, hedefe girilmez. Başarıda `true` döner.
pub fn guarded_delete(
    path: &std::path::Path,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
) -> bool {
    if let Err(reason) = safety::check_path(path, ctx) {
        log::warn!("skipped {}: {reason}", path.display());
        let entry = Entry::skipped(cleaner, option, path);
        ctx.progress.on_entry(&entry);
        report.push(entry);
        safety::log_operation(ctx, cleaner, option, path, 0, true);
        return false;
    }
    if ctx.dry_run {
        let bytes = crate::fsutil::size::size_of_or_zero(path);
        let entry = Entry::new(
            EntryKind::Delete,
            cleaner,
            option,
            crate::i18n::et(&ctx.lang, "to clean: {}", &[&path.display().to_string()]),
            Some(path),
            bytes,
        );
        ctx.progress.on_entry(&entry);
        report.push(entry);
        return false;
    }
    let bytes = crate::fsutil::size::size_of_or_zero(path);
    safety::maybe_backup(ctx, path);
    let is_real_dir = path.is_dir()
        && std::fs::symlink_metadata(path)
            .map(|md| !md.file_type().is_symlink())
            .unwrap_or(false);
    if is_real_dir {
        clean_dir_contents(path, ctx, cleaner, option, report, 32);
        if ctx.cancelled() {
            report.aborted = true;
            return false;
        }
    }
    match delete(path, DeleteOptions::simple()) {
        Ok(true) => {
            let entry = Entry::new(
                EntryKind::Delete,
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "deleted {}", &[&path.display().to_string()]),
                Some(path),
                bytes,
            );
            ctx.progress.on_entry(&entry);
            report.push(entry);
            safety::log_operation(ctx, cleaner, option, path, bytes, false);
            true
        }
        Ok(false) => false,
        Err(err) if err.is_benign() => false,
        Err(err) => {
            report.fail(cleaner, option, format!("{}: {err}", path.display()));
            false
        }
    }
}

/// Bir dizinin *içeriğini* (dizinin kendisini değil) korumalı şekilde sil.
pub fn clean_dir_contents(
    root: &std::path::Path,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
    max_depth: usize,
) {
    if !root.is_dir() {
        return;
    }
    // Aynı dosya sistemi: bağlı başka birimin içine inilmez.
    let options = crate::fsutil::walk::ScanOptions::all()
        .with_max_depth(max_depth)
        .with_same_filesystem(true);
    for path in crate::fsutil::walk::scan_paths(root, &options) {
        if ctx.cancelled() {
            report.aborted = true;
            return;
        }
        guarded_delete(&path, ctx, cleaner, option, report);
    }
}

/// Köküyle birlikte tüm ağacı sil (dry-run'da tek toplu önizleme kaydı).
pub fn remove_tree(
    root: &std::path::Path,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
) {
    if !root.exists() {
        return;
    }
    if ctx.dry_run {
        let (bytes, files) = dir_stats(root);
        let entry = Entry::new(
            EntryKind::Delete,
            cleaner,
            option,
            crate::i18n::et(
                &ctx.lang,
                "to clean: {} ({} files)",
                &[&root.display().to_string(), &files.to_string()],
            ),
            Some(root),
            bytes,
        );
        ctx.progress.on_entry(&entry);
        report.push(entry);
        return;
    }
    clean_dir_contents(root, ctx, cleaner, option, report, 32);
    if ctx.cancelled() {
        report.aborted = true;
        return;
    }
    guarded_delete(root, ctx, cleaner, option, report);
}

/// Dizin istatistiği: (tahmini bayt, dosya sayısı).
pub fn dir_stats(root: &std::path::Path) -> (u64, usize) {
    let options = crate::fsutil::walk::ScanOptions::files();
    let files = crate::fsutil::walk::scan_paths(root, &options);
    let bytes = files
        .iter()
        .map(|p| crate::fsutil::size::size_of_or_zero(p))
        .sum();
    (bytes, files.len())
}

/// Harici komutun çıktısını al; yoksa `None` (araç kurulu değil).
pub fn command_output(program: &str, args: &[&str]) -> Option<String> {
    match crate::fsutil::run_command(program, args, true) {
        Ok((0, stdout, _)) => Some(stdout),
        Ok((_, _, _)) => None,
        Err(_) => None,
    }
}

/// Program `$PATH`'ta var mı?
pub fn have(program: &str) -> bool {
    crate::fsutil::path_exists_in_path(program)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guarded_delete_removes_nonempty_dir() {
        let root = std::env::temp_dir().join("sweep_guarded_test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub").join("f.txt"), "data").unwrap();
        let mut ctx = crate::action::RunContext::clean();
        // The default context keep-lists the temp *root* on Windows (installers
        // recreate it), which would refuse this child directory. Use a bare
        // guard so the test exercises the delete path on every platform.
        ctx.guard = crate::core::keep::Guard::default_guard();
        let mut report = crate::core::report::Report::new();
        assert!(guarded_delete(&root, &ctx, "t", "o", &mut report));
        assert!(!root.exists());
    }
}
