//! Disk kullanımı: bir klasörün bir alt öğelerinin özyineli boyutları.
//!
//! `sweep du <yol>` salt okunurdur — hiçbir şeyi silmez, yalnızca ölçer.
//! GUI'deki Disk ekranı buradan beslenir: sürücüye tıklanır, bu motor o
//! klasörün çocuklarını (dosya = kendi boyu, klasör = özyineli toplam)
//! büyükten küçüğe dizer. Tarama [`rayon`] ile paraleldir; sembolik bağlar
//! izlenmez, başka dosya sistemine taşılmaz.

use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::action::context::RunContext;
use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "du";

/// Tek satır: klasörün bir alt öğesi.
#[derive(Debug, Clone)]
pub struct DuHit {
    pub path: PathBuf,
    pub bytes: u64,
    pub is_dir: bool,
}

/// Bir ağacın toplam baytı (yalnız dosyalar, tahsisli boyut).
fn subtree_bytes(dir: &Path) -> u64 {
    let options = crate::fsutil::walk::ScanOptions::files().with_same_filesystem(true);
    crate::fsutil::walk::scan_paths(dir, &options)
        .into_iter()
        .filter_map(|p| std::fs::symlink_metadata(&p).ok())
        .filter(|md| md.file_type().is_file())
        .map(|md| crate::fsutil::size::allocated_size(&md))
        .sum()
}

/// `root`un bir alt öğelerini ölç, büyük önce diz (`top` ile kesilir).
///
/// `root` klasör değilse boş döner (GUI "boş klasör" gösterir, hata değil).
/// NOT: `children()` ada rağmen özyineli listeler; burada gerçekten tek
/// seviye istenir, o yüzden derinlik sınırlı seçeneklerle taranır.
pub fn summarize(root: &Path, top: Option<usize>) -> Vec<DuHit> {
    if !root.is_dir() {
        return Vec::new();
    }
    let options = crate::fsutil::walk::ScanOptions {
        include_dirs: true,
        max_depth: Some(1),
        ..crate::fsutil::walk::ScanOptions::default()
    };
    let mut hits: Vec<DuHit> = crate::fsutil::walk::scan_paths(root, &options)
        .into_par_iter()
        .filter_map(|path| {
            let md = std::fs::symlink_metadata(&path).ok()?;
            let is_dir = md.file_type().is_dir();
            let bytes = if is_dir {
                subtree_bytes(&path)
            } else if md.file_type().is_file() {
                crate::fsutil::size::allocated_size(&md)
            } else {
                return None;
            };
            Some(DuHit {
                path,
                bytes,
                is_dir,
            })
        })
        .collect();

    hits.sort_by_key(|h| std::cmp::Reverse(h.bytes));
    if let Some(n) = top {
        hits.truncate(n);
    }
    hits
}

/// Ölç ve raporla. Salt okunur: `Delete` girdisi üretilmez, `--clean`
/// verilse bile silme olmaz.
pub fn scan(root: &Path, top: Option<usize>, ctx: &RunContext) -> Report {
    let mut report = Report::new();
    let hits = summarize(root, top);

    let total: u64 = hits.iter().map(|h| h.bytes).sum();
    report.push(Entry::new(
        EntryKind::Command,
        CLEANER_ID,
        "summary",
        crate::i18n::et(
            &ctx.lang,
            "{} entries, {} total (sorted, biggest first)",
            &[&hits.len().to_string(), &crate::deep::safety::human(total)],
        ),
        None,
        0,
    ));

    for hit in &hits {
        let label = format!(
            "{} — {}",
            hit.path.display(),
            crate::deep::safety::human(hit.bytes),
        );
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            if hit.is_dir { "dir" } else { "file" },
            label,
            Some(&hit.path),
            hit.bytes,
        ));
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    // Her test kendi klasörünü kullanır: kargo testleri paralel koşar,
    // ortak yol birbirinin dosyasını siler.
    fn fixture(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sweep_du_test_{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub").join("deep")).unwrap();
        std::fs::write(dir.join("a.bin"), vec![1u8; 100 * 1024]).unwrap();
        std::fs::write(dir.join("sub").join("b.bin"), vec![1u8; 300 * 1024]).unwrap();
        std::fs::write(
            dir.join("sub").join("deep").join("c.bin"),
            vec![1u8; 200 * 1024],
        )
        .unwrap();
        dir
    }

    #[test]
    fn dir_totals_are_recursive_and_sorted() {
        let dir = fixture("totals");
        let hits = summarize(&dir, None);
        assert_eq!(hits.len(), 2);
        // sub/ = 500KB (özyineli), a.bin = 100KB: klasör önce gelir.
        assert!(hits[0].is_dir);
        assert!(hits[0].bytes >= hits[1].bytes);
        assert!(hits[0].bytes >= 500 * 1024);
        assert!(hits[1].bytes >= 100 * 1024);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn top_truncates_and_missing_root_is_empty() {
        let dir = fixture("trunc");
        let hits = summarize(&dir, Some(1));
        assert_eq!(hits.len(), 1);
        assert!(summarize(&dir.join("yok"), None).is_empty());
        assert!(summarize(&dir.join("a.bin"), None).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_never_marks_deletions() {
        let dir = fixture("nodelete");
        let ctx = crate::action::context::RunContext::preview();
        let report = scan(&dir, None, &ctx);
        assert!(report.entries.iter().all(|e| !e.kind.counts_as_deleted()));
        assert!(!report.entries.is_empty());
        // Dosyalar hâlâ duruyor: salt okunur kanıtı.
        assert!(dir.join("a.bin").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
