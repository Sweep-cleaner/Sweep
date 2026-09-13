//! Yinelenen dosya bulucu: iki fazlı eleme ile hızlı ve güvenli.
//!
//! Önce dosyalar boyuta göre gruplanır (aynı boyutta tek dosya elenir),
//! sonra adayların içeriği özetlenir (SipHash, rayon ile paralel).
//! `--keep` hangi kopyanın korunacağını seçer (varsayılan: sözlüksel ilk);
//! diğerleri korumalı silme hattından (`Guard`, kritik dizin, yedek, log)
//! geçirilir ya da `--link` ile korunan dosyaya hard linklenir.
//! Sembolik bağlar asla izlenmez ve hedef olmaz.

use std::collections::HashMap;
use std::hash::Hasher;
use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::action::context::RunContext;
use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "dupes";

/// Varsayılan en küçük dosya boyutu ("1MB").
pub const DEFAULT_MIN_SIZE: u64 = 1024 * 1024;

/// Özetleme bloğu (64 KiB).
const CHUNK: usize = 64 * 1024;

/// Tek grup: aynı içerik, birden fazla yol (sıralı).
#[derive(Debug, Clone)]
pub struct DupeGroup {
    pub size: u64,
    pub files: Vec<PathBuf>,
}

/// Grupta hangi kopya korunur (`--keep`).
///
/// Kopyalar tanım gereği aynı içeriğe sahip olduğu için tahsisli boyutlar
/// neredeyse her zaman eşittir; yani *eşitliği bozan* kural davranışı
/// belirler. Tümü deterministiktir (hiçbiri rastgele seçmez):
///   * `Largest`  – boyut eşitse **en yeni** mtime, sonra ters-sözlüksel yol.
///   * `Smallest` – boyut eşitse **en eski** mtime, sonra sözlüksel yol.
///   * `First`    – sözlüksel ilk yol.
/// Seyrek dosyalarda tahsisli boyut gerçekten farklılaşabilir; o zaman boyut
/// kazanır ve bu kurallar hiç işlemez.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeepPolicy {
    /// Sözlüksel ilk yol (tarihsel varsayılan).
    #[default]
    First,
    /// En yeni mtime.
    Newest,
    /// En eski mtime.
    Oldest,
    /// En büyük tahsisli boyut (seyrek dosyalarda farklılaşır).
    Largest,
    /// En küçük tahsisli boyut.
    Smallest,
}

impl KeepPolicy {
    /// `--keep` değerini çöz (`first|newest|oldest|largest|smallest`).
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.to_ascii_lowercase().as_str() {
            "first" => Ok(KeepPolicy::First),
            "newest" => Ok(KeepPolicy::Newest),
            "oldest" => Ok(KeepPolicy::Oldest),
            "largest" => Ok(KeepPolicy::Largest),
            "smallest" => Ok(KeepPolicy::Smallest),
            other => Err(format!(
                "--keep must be first|newest|oldest|largest|smallest, got '{other}'"
            )),
        }
    }
}

/// Sıralama anahtarı: (tahsisli bayt, mtime). Okunamayan dosya her
/// politikada en sona düşer (asla korunan olmaz), eşitlikte yol karar verir.
fn rank(path: &Path) -> (u64, Option<std::time::SystemTime>) {
    match std::fs::symlink_metadata(path) {
        Ok(md) if md.file_type().is_file() => (
            crate::fsutil::size::allocated_size(&md),
            md.modified().ok(),
        ),
        _ => (0, None),
    }
}

/// Grupta korunacak dosyayı seç. Deterministik: eşitlikte sözlüksel ilk yol.
pub fn pick_keep(files: &[PathBuf], policy: KeepPolicy) -> Option<PathBuf> {
    if files.is_empty() {
        return None;
    }
    // Önce okunabilenler; hiçbiri okunamazsa sözlüksel ilk (silme hattı
    // hatayı raporlar, sessiz veri kaybı olmaz).
    let mut cands: Vec<&PathBuf> = files.iter().collect();
    let any_readable = cands.iter().any(|p| rank(p).1.is_some());
    if any_readable {
        cands.retain(|p| rank(p).1.is_some());
    }
    let best = match policy {
        KeepPolicy::First => cands.into_iter().min(),
        KeepPolicy::Newest => cands.into_iter().max_by(|a, b| {
            rank(a)
                .1
                .cmp(&rank(b).1)
                .then_with(|| b.cmp(a))
        }),
        KeepPolicy::Oldest => cands.into_iter().min_by(|a, b| {
            rank(a)
                .1
                .cmp(&rank(b).1)
                .then_with(|| a.cmp(b))
        }),
        KeepPolicy::Largest => cands.into_iter().max_by(|a, b| {
            rank(a)
                .0
                .cmp(&rank(b).0)
                .then_with(|| rank(a).1.cmp(&rank(b).1))
                .then_with(|| b.cmp(a))
        }),
        KeepPolicy::Smallest => cands.into_iter().min_by(|a, b| {
            rank(a)
                .0
                .cmp(&rank(b).0)
                .then_with(|| rank(a).1.cmp(&rank(b).1))
                .then_with(|| a.cmp(b))
        }),
    }?;
    Some((*best).clone())
}

/// Varsayılan kökler: kullanıcı veri klasörleri, yoksa çalışma dizini.
pub fn default_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = crate::platform::home_dir() {
        for leaf in ["Pictures", "Documents", "Downloads", "Music", "Videos"] {
            let p = home.join(leaf);
            if p.is_dir() {
                roots.push(p);
            }
        }
    }
    if roots.is_empty() {
        if let Ok(cwd) = std::env::current_dir() {
            roots.push(cwd);
        }
    }
    roots
}

/// Dosyanın içerik özeti (boyut önden bilinir, özet + boyut birlikte anahtar).
pub fn hash_file(path: &Path) -> Option<u64> {
    hash_range(path, None)
}

/// İlk `limit` baytın özeti (iki fazlı elemenin hızlı ön filtresi).
/// `None` limit yok demektir (tüm dosya).
fn hash_range(path: &Path, limit: Option<u64>) -> Option<u64> {
    use std::io::Read as _;

    let file = std::fs::File::open(path).ok()?;
    let mut stream: Box<dyn std::io::Read> = match limit {
        Some(n) => Box::new(file.take(n)),
        None => Box::new(file),
    };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = stream.read(&mut buf).ok()?;
        if n == 0 {
            break;
        }
        hasher.write(&buf[..n]);
    }
    Some(hasher.finish())
}

/// Sembolik bağları ve dizinleri ele, normal dosyaları topla.
fn collect_files(roots: &[PathBuf], max_depth: usize, min_size: u64) -> Vec<(PathBuf, u64)> {
    roots
        .par_iter()
        .flat_map(|root| {
            if !root.is_dir() {
                return Vec::new();
            }
            let options = crate::fsutil::walk::ScanOptions::files().with_max_depth(max_depth);
            crate::fsutil::walk::scan_paths(root, &options)
                .into_iter()
                .filter_map(|path| {
                    // Bağları atla: hedefi iki kez sayma, dışarı taşma.
                    let md = std::fs::symlink_metadata(&path).ok()?;
                    if !md.file_type().is_file() {
                        return None;
                    }
                    let size = crate::fsutil::size::allocated_size(&md);
                    if size < min_size {
                        return None;
                    }
                    Some((path, size))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Grupları bul (silmez).
///
/// Üç fazlı eleme: boyut → ilk 64 KiB özeti → tam özet + bayt bayt doğrulama.
/// Büyük dosyalar aynı boyutta olup ilk blokta farklıysa tam okunmaz.
pub fn find_groups(roots: &[PathBuf], max_depth: usize, min_size: u64) -> Vec<DupeGroup> {
    // 1. faz: boyuta göre.
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    for (path, size) in collect_files(roots, max_depth, min_size) {
        by_size.entry(size).or_default().push(path);
    }

    // 2. faz: ön özet (boyut + ilk blok), paralel.
    let candidates: Vec<(u64, Vec<PathBuf>)> = by_size
        .into_par_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .flat_map(|(size, paths)| {
            let mut by_prefix: HashMap<u64, Vec<PathBuf>> = HashMap::new();
            for path in &paths {
                if let Some(h) = hash_range(path, Some(CHUNK as u64)) {
                    by_prefix.entry(h).or_default().push(path.clone());
                }
            }
            by_prefix
                .into_iter()
                .filter(|(_, v)| v.len() > 1)
                .map(|(_, files)| (size, files))
                .collect::<Vec<_>>()
        })
        .collect();

    // 3. faz: adayların tam özeti + bayt bayt doğrulama (paralel).
    let mut groups: Vec<DupeGroup> = candidates
        .into_par_iter()
        .flat_map(|(size, paths)| {
            let mut by_hash: HashMap<u64, Vec<PathBuf>> = HashMap::new();
            for path in &paths {
                if let Some(h) = hash_file(path) {
                    // Özet çakışmasına karşı aynı özetli dosyaları bayt bayt
                    // doğrula: ilk dosyayla karşılaştır.
                    let bucket = by_hash.entry(h).or_default();
                    let same = bucket
                        .first()
                        .map(|first| files_equal(first, path))
                        .unwrap_or(true);
                    if same {
                        bucket.push(path.clone());
                    } else {
                        // Çakışma: ayrı grup (pratikte neredeyse imkânsız).
                        by_hash
                            .entry(h.wrapping_add(1))
                            .or_default()
                            .push(path.clone());
                    }
                }
            }
            by_hash
                .into_iter()
                .filter(|(_, v)| v.len() > 1)
                .map(|(_, mut files)| {
                    files.sort();
                    // Örtüşen kökler aynı yolu iki kez verebilir; tekille
                    // yoksa korunan dosya "kopya" diye silinir/linklenir.
                    files.dedup();
                    DupeGroup { size, files }
                })
                .filter(|g| g.files.len() > 1)
                .collect::<Vec<_>>()
        })
        .collect();

    // Büyük israf önce.
    groups.sort_by_key(|g| std::cmp::Reverse(g.size * g.files.len() as u64));
    groups
}

/// İki dosyayı bayt bayt karşılaştır.
fn files_equal(a: &Path, b: &Path) -> bool {
    use std::io::Read as _;

    let (Ok(fa), Ok(fb)) = (std::fs::File::open(a), std::fs::File::open(b)) else {
        return false;
    };
    let mut ra = std::io::BufReader::new(fa);
    let mut rb = std::io::BufReader::new(fb);
    let mut ba = vec![0u8; CHUNK];
    let mut bb = vec![0u8; CHUNK];
    loop {
        let (Ok(na), Ok(nb)) = (ra.read(&mut ba), rb.read(&mut bb)) else {
            return false;
        };
        if na != nb || ba[..na] != bb[..nb] {
            return false;
        }
        if na == 0 {
            return true;
        }
    }
}

/// Kopyayı korunan dosyaya hard linkle (alanı geri kazanır, tek inode).
///
/// Sıra: kopya yedeğe alınır → link kurulur → yedek silinir. Link başarısız
/// olursa yedek geri taşınır, veri kaybı olmaz. Farklı diskte link
/// kurulamaz (hata raporlanır, dosya olduğu gibi kalır).
fn hardlink_dupe(
    kept: &Path,
    dup: &Path,
    size: u64,
    lang: &crate::i18n::Lang,
    report: &mut Report,
) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let dev = |p: &Path| {
            std::fs::symlink_metadata(p)
                .ok()
                .filter(|m| m.file_type().is_file())
                .map(|m| m.dev())
        };
        match (dev(kept), dev(dup)) {
            (Some(a), Some(b)) if a != b => {
                report.fail(
                    CLEANER_ID,
                    "dupes",
                    format!(
                        "cannot hardlink across filesystems: {}",
                        dup.display()
                    ),
                );
                return;
            }
            (None, _) | (_, None) => {
                report.fail(
                    CLEANER_ID,
                    "dupes",
                    format!("cannot stat for hardlink: {}", dup.display()),
                );
                return;
            }
            _ => {}
        }
        // Zaten aynı inode ise yapacak iş yok.
        let ino = |p: &Path| {
            std::fs::symlink_metadata(p)
                .ok()
                .map(|m| (m.dev(), m.ino()))
        };
        if ino(kept) == ino(dup) {
            report.push(Entry::skipped(CLEANER_ID, "linked", dup));
            return;
        }
    }
    let backup = dup.with_extension("sweep-link-bak");
    // Yedek adı doluysa başlamadan dur (önceki yarım işin üstüne yazma).
    if backup.exists() {
        report.fail(
            CLEANER_ID,
            "dupes",
            format!(
                "staging file exists, refusing to overwrite: {}",
                backup.display()
            ),
        );
        return;
    }
    if std::fs::rename(dup, &backup).is_err() {
        report.fail(
            CLEANER_ID,
            "dupes",
            format!("cannot stage for hardlink: {}", dup.display()),
        );
        return;
    }
    match std::fs::hard_link(kept, dup) {
        Ok(()) => {
            let _ = std::fs::remove_file(&backup);
            report.push(Entry::new(
                EntryKind::Command,
                CLEANER_ID,
                "dupes",
                crate::i18n::et(lang, "hardlinked {}", &[&dup.display().to_string()]),
                Some(dup),
                size,
            ));
        }
        Err(err) => {
            let _ = std::fs::rename(&backup, dup);
            report.fail(
                CLEANER_ID,
                "dupes",
                format!("hardlink {} failed ({err}), original kept", dup.display()),
            );
        }
    }
}

/// Tara ve raporla; `ctx.dry_run == false` ise `--keep` politikasına göre
/// bir dosya korunur, diğerleri silinir (`--link` ile hard linklenir).
pub fn scan(
    roots: &[PathBuf],
    max_depth: usize,
    min_size: u64,
    keep: KeepPolicy,
    link: bool,
    ctx: &RunContext,
) -> Report {
    let mut report = Report::new();
    let groups = find_groups(roots, max_depth, min_size);

    if groups.is_empty() {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "dupes",
            crate::i18n::t(&ctx.lang, "no duplicate files"),
            None,
            0,
        ));
        return report;
    }

    for group in &groups {
        let waste = group.size * (group.files.len() as u64 - 1);
        let kept = pick_keep(&group.files, keep).unwrap_or_else(|| group.files[0].clone());
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "group",
            crate::i18n::et(
                &ctx.lang,
                "{} copies × {} (kept: {}, waste: {})",
                &[
                    &group.files.len().to_string(),
                    &crate::deep::safety::human(group.size),
                    &kept.display().to_string(),
                    &crate::deep::safety::human(waste),
                ],
            ),
            Some(&kept),
            0,
        ));
        for dup in group.files.iter().filter(|p| **p != kept) {
            if ctx.dry_run {
                report.push(Entry::new(
                    EntryKind::Delete,
                    CLEANER_ID,
                    "dupes",
                    crate::i18n::et(&ctx.lang, "duplicate {}", &[&dup.display().to_string()]),
                    Some(dup),
                    group.size,
                ));
            } else if link {
                hardlink_dupe(&kept, dup, group.size, &ctx.lang, &mut report);
            } else {
                crate::deep::guarded_delete(dup, ctx, CLEANER_ID, "dupes", &mut report);
            }
        }
        if ctx.dry_run {
            report.push(Entry::skipped(CLEANER_ID, "kept", &kept));
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_duplicates() {
        let dir = std::env::temp_dir().join("sweep_dupes_test");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("a.bin"), vec![7u8; 2048]).unwrap();
        std::fs::write(dir.join("b.bin"), vec![7u8; 2048]).unwrap();
        std::fs::write(dir.join("c.bin"), vec![8u8; 2048]).unwrap();
        let groups = find_groups(std::slice::from_ref(&dir), 4, 0);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].files.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn min_size_filters() {
        let dir = std::env::temp_dir().join("sweep_dupes_min");
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("a.bin"), vec![7u8; 64]).unwrap();
        std::fs::write(dir.join("b.bin"), vec![7u8; 64]).unwrap();
        // Not: boyut disk-tahsislidir (bloklar); 64 baytlık dosya ~4KiB tutar.
        let groups = find_groups(std::slice::from_ref(&dir), 4, 1024 * 1024);
        assert!(groups.is_empty());
        let present = find_groups(std::slice::from_ref(&dir), 4, 0);
        assert_eq!(present.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn keep_policy_parses_all_forms() {
        assert_eq!(KeepPolicy::parse("first").unwrap(), KeepPolicy::First);
        assert_eq!(KeepPolicy::parse("NEWEST").unwrap(), KeepPolicy::Newest);
        assert_eq!(KeepPolicy::parse("oldest").unwrap(), KeepPolicy::Oldest);
        assert_eq!(KeepPolicy::parse("largest").unwrap(), KeepPolicy::Largest);
        assert_eq!(KeepPolicy::parse("smallest").unwrap(), KeepPolicy::Smallest);
        assert!(KeepPolicy::parse("random").is_err());
        assert_eq!(KeepPolicy::default(), KeepPolicy::First);
    }

    fn stamp(path: &std::path::Path, secs_ago: u64) {
        let t = std::time::SystemTime::now()
            - std::time::Duration::from_secs(secs_ago.max(1));
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    #[test]
    fn pick_keep_honours_newest_oldest_first() {
        let dir = std::env::temp_dir().join("sweep_dupes_keep");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // Aynı içerik, farklı yaş: ada en eski, adc en yeni.
        for name in ["ada.bin", "adb.bin", "adc.bin"] {
            std::fs::write(dir.join(name), vec![3u8; 512]).unwrap();
        }
        stamp(&dir.join("ada.bin"), 300);
        stamp(&dir.join("adb.bin"), 200);
        stamp(&dir.join("adc.bin"), 100);
        let files: Vec<PathBuf> =
            ["adc.bin", "ada.bin", "adb.bin"].map(|n| dir.join(n)).into();
        assert_eq!(
            pick_keep(&files, KeepPolicy::First).unwrap(),
            dir.join("ada.bin")
        );
        assert_eq!(
            pick_keep(&files, KeepPolicy::Newest).unwrap(),
            dir.join("adc.bin")
        );
        assert_eq!(
            pick_keep(&files, KeepPolicy::Oldest).unwrap(),
            dir.join("ada.bin")
        );
        // Grup içi boyutlar eşit olduğu için eşitliği bozan kural devreye
        // girer (bkz. KeepPolicy belgesi): `Largest` en yeni mtime'ı,
        // `Smallest` en eski mtime'ı seçer. ada en eski, adc en yeni.
        assert_eq!(
            pick_keep(&files, KeepPolicy::Largest).unwrap(),
            dir.join("adc.bin")
        );
        assert_eq!(
            pick_keep(&files, KeepPolicy::Smallest).unwrap(),
            dir.join("ada.bin")
        );
        assert!(pick_keep(&[], KeepPolicy::First).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn hardlink_shares_inode_and_keeps_content() {
        use std::os::unix::fs::MetadataExt;
        let dir = std::env::temp_dir().join("sweep_dupes_link");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let kept = dir.join("kept.bin");
        let dup = dir.join("dup.bin");
        std::fs::write(&kept, vec![9u8; 1024]).unwrap();
        std::fs::write(&dup, vec![9u8; 1024]).unwrap();
        let mut report = Report::new();
        let lang = crate::i18n::Lang::new("en");
        hardlink_dupe(&kept, &dup, 1024, &lang, &mut report);
        assert!(report.errors.is_empty());
        let (mk, md) = (
            std::fs::symlink_metadata(&kept).unwrap(),
            std::fs::symlink_metadata(&dup).unwrap(),
        );
        assert_eq!((mk.dev(), mk.ino()), (md.dev(), md.ino()));
        assert_eq!(std::fs::read(&dup).unwrap(), vec![9u8; 1024]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
