//! Disk analizi: dizin ağacında neyin yer kapladığını gösterir.
//!
//! Silme yapmaz (`--clean` yok); bulgular `sweep bigfiles --clean`,
//! `devscan` veya Disk ekranına taşınır. Üst düzey dizinler özyinelemeli
//! boyutla, dosyalar `bigfiles` motoruyla listelenir.

use std::path::PathBuf;

use rayon::prelude::*;

use crate::action::context::RunContext;
use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "analyze";

/// Kökün doğrudan alt dizinleri (büyük önce).
pub fn top_dirs(root: &PathBuf, top: usize) -> Vec<(PathBuf, u64, usize)> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            // Bağları izleme: yalnızca gerçek dizinler (geri kalan motorla tutarlı).
            std::fs::symlink_metadata(p)
                .map(|md| md.file_type().is_dir())
                .unwrap_or(false)
        })
        .collect();
    let mut hits: Vec<(PathBuf, u64, usize)> = dirs
        .par_iter()
        .map(|d| {
            let (bytes, files) = crate::deep::dir_stats(d);
            (d.clone(), bytes, files)
        })
        .collect();
    hits.sort_by_key(|h| std::cmp::Reverse(h.1));
    hits.truncate(top);
    hits
}

/// Çözümle ve raporla (salt okunur; `ctx` yalnızca iptal bayrağı için).
pub fn scan(root: &PathBuf, top: usize, max_depth: usize, ctx: &RunContext) -> Report {
    let mut report = Report::new();
    if !root.is_dir() {
        report.fail(
            CLEANER_ID,
            "root",
            crate::i18n::et(&ctx.lang, "no such dir: {}", &[&root.display().to_string()]),
        );
        return report;
    }

    let dirs = top_dirs(root, top);
    let dir_total: u64 = dirs.iter().map(|(_, b, _)| *b).sum();
    report.push(Entry::new(
        EntryKind::Command,
        CLEANER_ID,
        "summary",
        crate::i18n::et(
            &ctx.lang,
            "{}: {} dirs, {} total",
            &[
                &root.display().to_string(),
                &dirs.len().to_string(),
                &crate::deep::safety::human(dir_total),
            ],
        ),
        Some(root),
        0,
    ));
    for (i, (path, bytes, files)) in dirs.iter().enumerate() {
        if ctx.cancelled() {
            report.aborted = true;
            break;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "dirs",
            format!(
                "#{} {name} — {} ({files} files) [{}]",
                i + 1,
                crate::deep::safety::human(*bytes),
                path.display()
            ),
            Some(path),
            0,
        ));
    }

    for hit in crate::engine::bigfiles::find_hits(
        std::slice::from_ref(root),
        max_depth,
        0,
        None,
        Some(top),
    ) {
        if ctx.cancelled() {
            report.aborted = true;
            break;
        }
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "files",
            format!(
                "{} [{}]",
                crate::deep::safety::human(hit.bytes),
                hit.path.display()
            ),
            Some(&hit.path),
            0,
        ));
    }
    report
}
