//! Büyük dosya avcısı: kullanıcı eşiğinin üstündeki dosyaları boyut
//! sırasına dizer (`sweep bigfiles --min-size 500MB --top 20`).
//!
//! İsteğe bağlı yaş filtresiyle (`--older-than 90d`) "büyük VE eski"
//! dosyalar bulunur — disk açmanın en hızlı yolu. Silme her zamanki
//! korumalı hattan geçer.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rayon::prelude::*;

use crate::action::context::RunContext;
use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "bigfiles";

/// Varsayılan eşik ("100MB").
pub const DEFAULT_MIN_SIZE: u64 = 100 * 1024 * 1024;

/// Tek bulgu.
#[derive(Debug, Clone)]
pub struct BigHit {
    pub path: PathBuf,
    pub bytes: u64,
    /// Değişiklik zamanı (yaş filtresi ve raporda gösterim için).
    pub mtime: Option<SystemTime>,
}

/// "90d" / "12w" / "6m" / "2y" / "48h" -> gün sayısı.
pub fn parse_age(text: &str) -> Option<u32> {
    let text = text.trim().to_lowercase();
    let (num, unit) = text.split_at(text.find(|c: char| !c.is_ascii_digit())?);
    let n: u32 = num.parse().ok()?;
    let days = match unit.trim() {
        "h" | "hour" | "hours" | "saat" => n / 24,
        "d" | "day" | "days" | "gün" | "gun" => n,
        "w" | "week" | "weeks" | "hafta" => n.saturating_mul(7),
        "m" | "month" | "months" | "ay" => n.saturating_mul(30),
        "y" | "year" | "years" | "yıl" | "yil" => n.saturating_mul(365),
        _ => return None,
    };
    Some(days)
}

fn older_than(path: &Path, days: u32) -> bool {
    let Ok(md) = std::fs::symlink_metadata(path) else {
        return false;
    };
    let Ok(mtime) = md.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(mtime)
        .map(|d| d.as_secs() >= u64::from(days) * 86_400)
        .unwrap_or(false)
}

/// Köklerde eşiği aşan dosyaları bul (büyük önce, `top` ile kesilir).
pub fn find_hits(
    roots: &[PathBuf],
    max_depth: usize,
    min_size: u64,
    older_than_days: Option<u32>,
    top: Option<usize>,
) -> Vec<BigHit> {
    let mut hits: Vec<BigHit> = roots
        .par_iter()
        .flat_map(|root| {
            if !root.is_dir() {
                return Vec::new();
            }
            let options = crate::fsutil::walk::ScanOptions::files().with_max_depth(max_depth);
            crate::fsutil::walk::scan_paths(root, &options)
                .into_iter()
                .filter_map(|path| {
                    let md = std::fs::symlink_metadata(&path).ok()?;
                    if !md.file_type().is_file() {
                        return None;
                    }
                    let bytes = crate::fsutil::size::allocated_size(&md);
                    if bytes < min_size {
                        return None;
                    }
                    let mtime = md.modified().ok();
                    if let Some(days) = older_than_days {
                        if !older_than(&path, days) {
                            return None;
                        }
                    }
                    Some(BigHit { path, bytes, mtime })
                })
                .collect::<Vec<_>>()
        })
        .collect();

    hits.sort_by_key(|h| std::cmp::Reverse(h.bytes));
    if let Some(n) = top {
        hits.truncate(n);
    }
    hits
}

fn age_text(mtime: Option<SystemTime>) -> String {
    let Some(t) = mtime else {
        return "bilinmiyor".into();
    };
    let days = SystemTime::now()
        .duration_since(t)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0);
    if days == 0 {
        "today".into()
    } else if days < 30 {
        format!("{days} days")
    } else if days < 365 {
        format!("{} months", days / 30)
    } else {
        format!("{} years", days / 365)
    }
}

/// Tara ve raporla; `ctx.dry_run == false` ise korumalı sil.
pub fn scan(
    roots: &[PathBuf],
    max_depth: usize,
    min_size: u64,
    older_than_days: Option<u32>,
    top: Option<usize>,
    ctx: &RunContext,
) -> Report {
    let mut report = Report::new();
    let hits = find_hits(roots, max_depth, min_size, older_than_days, top);

    if hits.is_empty() {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "bigfiles",
            crate::i18n::et(
                &ctx.lang,
                "no files above threshold (> {})",
                &[&crate::deep::safety::human(min_size)],
            ),
            None,
            0,
        ));
        return report;
    }

    let total: u64 = hits.iter().map(|h| h.bytes).sum();
    report.push(Entry::new(
        EntryKind::Command,
        CLEANER_ID,
        "summary",
        crate::i18n::et(
            &ctx.lang,
            "{} files, {} total (sorted, biggest first)",
            &[&hits.len().to_string(), &crate::deep::safety::human(total)],
        ),
        None,
        0,
    ));

    for (rank, hit) in hits.iter().enumerate() {
        let label = format!(
            "#{} {} — {} (age: {})",
            rank + 1,
            hit.path.display(),
            crate::deep::safety::human(hit.bytes),
            age_text(hit.mtime),
        );
        if ctx.dry_run {
            report.push(Entry::new(
                EntryKind::Delete,
                CLEANER_ID,
                "bigfiles",
                label,
                Some(&hit.path),
                hit.bytes,
            ));
        } else {
            crate::deep::guarded_delete(&hit.path, ctx, CLEANER_ID, "bigfiles", &mut report);
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_parses() {
        assert_eq!(parse_age("90d"), Some(90));
        assert_eq!(parse_age("2w"), Some(14));
        assert_eq!(parse_age("6m"), Some(180));
        assert_eq!(parse_age("1y"), Some(365));
        assert_eq!(parse_age("3hafta"), Some(21));
        assert!(parse_age("x").is_none());
        assert!(parse_age("10x").is_none());
    }

    #[test]
    fn top_truncates_and_sorts() {
        let dir = std::env::temp_dir().join("sweep_bigfiles_test");
        let _ = std::fs::create_dir_all(&dir);
        // Blok tahsisi yüzünden küçük dosyalar eşit görünür; belirgin boylar kullan.
        std::fs::write(dir.join("s.bin"), vec![1u8; 100 * 1024]).unwrap();
        std::fs::write(dir.join("m.bin"), vec![1u8; 300 * 1024]).unwrap();
        std::fs::write(dir.join("l.bin"), vec![1u8; 200 * 1024]).unwrap();
        let hits = find_hits(std::slice::from_ref(&dir), 4, 0, None, Some(2));
        assert_eq!(hits.len(), 2);
        assert!(hits[0].bytes >= hits[1].bytes);
        assert_eq!(hits[0].path.file_name().unwrap(), "m.bin");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
