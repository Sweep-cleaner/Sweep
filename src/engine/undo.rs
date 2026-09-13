//! Geri alma: `--backup-dir` ile yapılan temizlikleri işlem logu
//! üzerinden geri yükler (`sweep undo`).
//!
//! Yalnızca yedeği bulunan ve hedefi şu an mevcut olmayan dosyalar
//! geri yazılır; üstüne yazma asla yapılmaz.

use std::path::{Path, PathBuf};

use crate::action::context::RunContext;
use crate::core::report::{Entry, EntryKind, Report};
use crate::engine::logline::{self, LogLine};

fn lang() -> crate::i18n::Lang {
    crate::i18n::Lang::detect(&[])
}

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "undo";

/// Geri yüklenebilir tek kayıt.
#[derive(Debug, Clone)]
pub struct Restore {
    pub original: PathBuf,
    pub backup: PathBuf,
    pub bytes: u64,
}

/// Log satırını ayrıştır; yalnızca gerçek silmeler (`clean`) geri alınabilir.
///
/// Ayrıştırmanın kendisi [`logline::parse`] içinde tanımlıdır — yolun ` | `
/// içerebilmesi, mod adları ve bayt alanı orada ele alınır.
fn split_line(line: &str) -> Option<LogLine> {
    let parsed = logline::parse(line)?;
    (parsed.mode == logline::Mode::Clean).then_some(parsed)
}

/// Yedek kökündeki karşılığı bul (Unix mutlak yollar + `C:\...` Windows yolları).
fn backup_for(original: &Path, backup_root: &Path) -> PathBuf {
    let text = original.to_string_lossy();
    let stripped = text
        .strip_prefix('/')
        .or_else(|| text.strip_prefix('\\'))
        .map(str::to_string)
        .unwrap_or_else(|| {
            // `C:\x` / `C:/x` öneki atılır.
            let mut s = text.as_ref();
            if s.len() >= 2 && s.as_bytes()[1] == b':' {
                s = &s[2..];
            }
            s.trim_start_matches(['/', '\\']).to_string()
        });
    // Windows ayraçları her platformda bileşenlere bölünür, yoksa Linux'ta
    // `x\f.txt` tek dosya adı sayılırdı.
    let mut out = backup_root.to_path_buf();
    for part in stripped.split(['/', '\\']).filter(|p| !p.is_empty()) {
        out.push(part);
    }
    out
}

/// Çözümlenmiş satırdan geri yükleme kaydı (yedek yoksa `None`).
fn restore_for(parsed: &LogLine, backup_root: &Path) -> Option<Restore> {
    let backup = backup_for(&parsed.path, backup_root);
    if !backup.is_file() {
        return None;
    }
    Some(Restore {
        original: parsed.path.clone(),
        backup,
        bytes: parsed.bytes,
    })
}

/// Log satırı: `ts | mod | cleaner.option | yol | 123B`.
fn parse_line(line: &str, backup_root: &Path) -> Option<Restore> {
    restore_for(&split_line(line)?, backup_root)
}

/// Geri alınacak koşu aralığı.
#[derive(Debug, Clone, Default)]
pub enum Scope {
    /// Tüm log (işaretli + işaretsiz tüm silmeler).
    #[default]
    All,
    /// Son işaretli koşu (işaret yoksa tüm log).
    Last,
    /// Kimliği verilen koşu (`ts | run | <id> | …` işareti).
    Run(String),
}

/// İşaretli tek temizlik koşusu (liste ekranı + `--run` hedefi).
#[derive(Debug, Clone)]
pub struct Run {
    pub id: String,
    pub ts: u64,
    pub selection: String,
    pub files: usize,
    pub bytes: u64,
    pub restorable: usize,
}

/// Logu koşu bölütlerine ayır: her `run` işareti yeni bölüt açar,
/// işaretten önceki satırlar kimliksiz (`None`) bölüttedir.
struct Segment {
    id: Option<String>,
    ts: u64,
    selection: String,
    cleans: Vec<LogLine>,
}

fn segments(log_path: &Path) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    let mut cur = Segment {
        id: None,
        ts: 0,
        selection: String::new(),
        cleans: Vec::new(),
    };
    let mut touched = false;
    for line in crate::engine::logline::parse_file(log_path) {
        if line.mode == logline::Mode::Run {
            if touched || !cur.cleans.is_empty() {
                out.push(std::mem::replace(
                    &mut cur,
                    Segment {
                        id: None,
                        ts: 0,
                        selection: String::new(),
                        cleans: Vec::new(),
                    },
                ));
            }
            // İşaret satırında `cleaner` = koşu kimliği, `path` = seçim özeti.
            cur.id = Some(line.cleaner.clone());
            cur.ts = line.ts;
            cur.selection = line.path.to_string_lossy().into_owned();
            touched = true;
            continue;
        }
        if line.mode != logline::Mode::Clean {
            continue;
        }
        touched = true;
        cur.cleans.push(line);
    }
    if touched {
        out.push(cur);
    }
    out
}

/// Koşu listesi (yeniden eskiye). Yalnızca en az bir silme içeren bölütler.
pub fn runs(log_path: &Path, backup_root: &Path) -> Vec<Run> {
    let mut runs: Vec<Run> = segments(log_path)
        .into_iter()
        .filter_map(|seg| {
            let id = seg.id?;
            if seg.cleans.is_empty() {
                return None;
            }
            let bytes = seg.cleans.iter().map(|l| l.bytes).sum();
            let restorable = seg
                .cleans
                .iter()
                .filter(|l| backup_for(&l.path, backup_root).is_file())
                .count();
            Some(Run {
                id,
                ts: seg.ts,
                selection: seg.selection,
                files: seg.cleans.len(),
                bytes,
                restorable,
            })
        })
        .collect();
    runs.reverse();
    runs
}

/// Kapsama giren bölütlerin `clean` satırları (sıralı).
fn scoped_cleans(log_path: &Path, scope: &Scope) -> Result<Vec<LogLine>, String> {
    let segs = segments(log_path);
    let picked: Vec<&Segment> = match scope {
        Scope::All => segs.iter().collect(),
        Scope::Last => segs.iter().last().map(|s| vec![s]).unwrap_or_default(),
        Scope::Run(id) => {
            let found: Vec<&Segment> = segs
                .iter()
                .filter(|s| s.id.as_deref() == Some(id))
                .collect();
            if found.is_empty() {
                return Err(crate::i18n::et(&lang(), "unknown run: {}", &[id]));
            }
            found
        }
    };
    Ok(picked.into_iter().flat_map(|s| s.cleans.clone()).collect())
}

/// Logdaki geri yüklenebilir kayıtlar + yedeksiz silinen sayısı.
pub fn collect(log_path: &Path, backup_root: &Path) -> (Vec<Restore>, usize) {
    collect_scoped(log_path, backup_root, &Scope::All).unwrap_or_default()
}

/// Kapsamlı toplama: bilinmeyen koşu kimliği `Err` ile açıkça bildirilir.
pub fn collect_scoped(
    log_path: &Path,
    backup_root: &Path,
    scope: &Scope,
) -> Result<(Vec<Restore>, usize), String> {
    let cleans = scoped_cleans(log_path, scope)?;
    // Aynı dosya birden çok kez silinmiş olabilir: sonuncusu geçerli.
    let mut restores: std::collections::HashMap<PathBuf, Restore> =
        std::collections::HashMap::new();
    let mut cleaned: usize = 0;
    for line in &cleans {
        cleaned += 1;
        if let Some(r) = restore_for(line, backup_root) {
            restores.insert(r.original.clone(), r);
        }
    }
    let mut restores: Vec<Restore> = restores.into_values().collect();
    restores.sort_by(|a, b| a.original.cmp(&b.original));
    let without_backup = cleaned.saturating_sub(restores.len());
    Ok((restores, without_backup))
}

/// Koşu listesini raporla (`undo --list`, GUI geçmiş ekranı).
///
/// Her girdinin `option` alanı koşu kimliğidir (`--run <id>` hedefi),
/// `label` insan dilinde özet taşır.
pub fn list_runs(log_path: &Path, backup_root: &Path) -> Report {
    let mut report = Report::new();
    if !log_path.is_file() {
        report.fail(
            CLEANER_ID,
            "log",
            crate::i18n::et(
                &lang(),
                "no operation log: {}",
                &[&log_path.display().to_string()],
            ),
        );
        return report;
    }
    let runs = runs(log_path, backup_root);
    if runs.is_empty() {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "runs",
            crate::i18n::t(&lang(), "no restorable runs in this log"),
            None,
            0,
        ));
        return report;
    }
    for run in &runs {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            &run.id,
            crate::i18n::et(
                &lang(),
                "run {}: {} files ({})",
                &[&run.id, &run.restorable.to_string(), &run.selection],
            ),
            None,
            run.bytes,
        ));
    }
    report
}

/// Önizle ya da geri yükle.
///
/// Güvenlik politikası:
/// * **Doğrulama engeli fail-closed:** yedeği ortadan kaybolmuş ya da üst
///   dizini açılamayan kayıt varsa **hiçbir dosya yazılmaz**, neden açık
///   hata olarak raporlanır (sessiz kısmi geri yükleme yok).
/// * **Yedeksiz silmeler:** kapsamda yedeği hiç alınmamış dosya varsa (ör.
///   `--backup-dir` siz silmeler, 100 MB üstü dosyalar, `apt-get clean`
///   gibi araç-içi silmeler) geri alınabilir olanlar yazılır, yedeksizler
///   tek toplu hata girdisiyle açıkça listelenir ve çıkış kodu sıfırdan
///   farklı olur. Katı abort burada yanlış olurdu: tek bir yedeksiz dosya
///   yüzlerce geri alınabilir dosyayı kilitlerdi.
/// * **Üstüne yazma yok:** hedefte dosya varsa atlanır (`skipped`).
pub fn scan(log_path: &Path, backup_root: &Path, ctx: &RunContext, scope: &Scope) -> Report {
    let mut report = Report::new();
    if !log_path.is_file() {
        report.fail(
            CLEANER_ID,
            "log",
            crate::i18n::et(
                &lang(),
                "no operation log: {}",
                &[&log_path.display().to_string()],
            ),
        );
        return report;
    }
    let (restores, without_backup) = match collect_scoped(log_path, backup_root, scope) {
        Ok(found) => found,
        Err(err) => {
            report.fail(CLEANER_ID, "run", err);
            return report;
        }
    };

    if restores.is_empty() {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "restore",
            crate::i18n::et(
                &lang(),
                "nothing restorable ({} without backup)",
                &[&without_backup.to_string()],
            ),
            None,
            0,
        ));
        return report;
    }

    if ctx.dry_run {
        for r in &restores {
            report.push(Entry::new(
                EntryKind::Command,
                CLEANER_ID,
                "restore",
                crate::i18n::et(
                    &lang(),
                    "to restore: {}",
                    &[&r.original.display().to_string()],
                ),
                Some(&r.original),
                r.bytes,
            ));
        }
        if without_backup > 0 {
            report.fail(
                CLEANER_ID,
                "restore",
                crate::i18n::et(
                    &lang(),
                    "{} deletions have no backup (unrecoverable)",
                    &[&without_backup.to_string()],
                ),
            );
        }
        return report;
    }

    // Fail-closed doğrulama: tek bir engel bile varsa hiçbir şey yazılmaz.
    for r in &restores {
        if r.original.exists() {
            continue; // üzerine yazma yok; aşağıda `skipped` kaydı düşülür
        }
        if !r.backup.is_file() {
            report.fail(
                CLEANER_ID,
                "restore",
                crate::i18n::et(
                    &lang(),
                    "cannot roll back: {}",
                    &[&crate::i18n::et(
                        &lang(),
                        "backup gone: {}",
                        &[&r.backup.display().to_string()],
                    )],
                ),
            );
            return report;
        }
        if let Some(parent) = r.original.parent() {
            if std::fs::create_dir_all(parent).is_err() {
                report.fail(
                    CLEANER_ID,
                    "restore",
                    crate::i18n::et(
                        &lang(),
                        "cannot roll back: {}",
                        &[&crate::i18n::et(
                            &lang(),
                            "cannot create dir: {}",
                            &[&parent.display().to_string()],
                        )],
                    ),
                );
                return report;
            }
        }
    }

    for r in &restores {
        if r.original.exists() {
            report.push(Entry::skipped(CLEANER_ID, "exists", &r.original));
            continue;
        }
        match std::fs::copy(&r.backup, &r.original) {
            Ok(_) => report.push(Entry::new(
                EntryKind::Command,
                CLEANER_ID,
                "restore",
                crate::i18n::et(
                    &lang(),
                    "restored: {}",
                    &[&r.original.display().to_string()],
                ),
                Some(&r.original),
                r.bytes,
            )),
            Err(err) => report.fail(
                CLEANER_ID,
                "restore",
                crate::i18n::et(
                    &lang(),
                    "{}: {}",
                    &[&r.original.display().to_string(), &err.to_string()],
                ),
            ),
        }
    }
    if without_backup > 0 {
        report.fail(
            CLEANER_ID,
            "restore",
            crate::i18n::et(
                &lang(),
                "{} deletions have no backup (unrecoverable)",
                &[&without_backup.to_string()],
            ),
        );
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_lines() {
        let dir = std::env::temp_dir().join("sweep_undo_test");
        let _ = std::fs::create_dir_all(dir.join("bak/etc"));
        std::fs::write(dir.join("bak/etc/app.conf"), "x").unwrap();
        let line = "1700000000 | clean | apt.cache | /etc/app.conf | 1B";
        let r = parse_line(line, &dir.join("bak")).unwrap();
        assert_eq!(r.original, PathBuf::from("/etc/app.conf"));
        assert!(parse_line("1 | preview | a.b | /x | 1B", &dir.join("bak")).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // `|` is not a legal filename character on Windows, so the on-disk half of
    // this test only exists where we can create such a file. The parser half is
    // covered by `pipe_in_path_survives_parsing` on every platform.
    #[cfg(unix)]
    #[test]
    fn pipe_in_path_parses() {
        let dir = std::env::temp_dir().join("sweep_undo_pipe");
        let _ = std::fs::create_dir_all(dir.join("bak/tmp"));
        std::fs::write(dir.join("bak/tmp/a | b.txt"), "x").unwrap();
        let line = "1700000000 | clean | bigfiles.bigfiles | /tmp/a | b.txt | 1B";
        let r = parse_line(line, &dir.join("bak")).unwrap();
        assert_eq!(r.original, PathBuf::from("/tmp/a | b.txt"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pipe_in_path_survives_parsing() {
        // A path may contain the field separator itself; the line is read from
        // the right so everything between the fixed head and the byte count is
        // the path. `history` used to drop such lines entirely.
        let line = "1700000000 | clean | bigfiles.bigfiles | /tmp/a | b.txt | 1B";
        let parsed = split_line(line).unwrap();
        assert_eq!(parsed.path, PathBuf::from("/tmp/a | b.txt"));
        assert_eq!(parsed.bytes, 1);
        assert_eq!(parsed.key(), "bigfiles.bigfiles");
    }

    #[test]
    fn windows_path_maps_under_backup() {
        let dir = std::env::temp_dir().join("sweep_undo_win");
        let _ = std::fs::create_dir_all(dir.join("bak/x"));
        std::fs::write(dir.join("bak/x/f.txt"), "x").unwrap();
        let got = backup_for(Path::new("C:\\x\\f.txt"), &dir.join("bak"));
        assert_eq!(got, dir.join("bak/x/f.txt"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn duplicates_keep_last() {
        let dir = std::env::temp_dir().join("sweep_undo_dup");
        let _ = std::fs::create_dir_all(dir.join("bak/etc"));
        std::fs::write(dir.join("bak/etc/a.conf"), "22").unwrap();
        let log = dir.join("ops.log");
        std::fs::write(
            &log,
            "1 | clean | a.b | /etc/a.conf | 1B\n2 | clean | a.b | /etc/a.conf | 2B\n",
        )
        .unwrap();
        let (restores, without) = collect(&log, &dir.join("bak"));
        assert_eq!(restores.len(), 1);
        assert_eq!(restores[0].bytes, 2);
        assert_eq!(without, 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn two_run_log(dir: &std::path::Path) -> PathBuf {
        let _ = std::fs::create_dir_all(dir.join("bak/tmp"));
        std::fs::write(dir.join("bak/tmp/a.txt"), "a").unwrap();
        std::fs::write(dir.join("bak/tmp/b.txt"), "b").unwrap();
        let log = dir.join("ops.log");
        std::fs::write(
            &log,
            "10 | run | r1 | firefox.cache | 0B\n\
             11 | clean | firefox.cache | /tmp/a.txt | 1B\n\
             12 | run | r2 | system.tmp | 0B\n\
             13 | clean | system.tmp | /tmp/b.txt | 1B\n",
        )
        .unwrap();
        log
    }

    #[test]
    fn scopes_select_runs() {
        let dir = std::env::temp_dir().join("sweep_undo_scope");
        let _ = std::fs::remove_dir_all(&dir);
        let log = two_run_log(&dir);
        let bak = dir.join("bak");
        let (all, _) = collect_scoped(&log, &bak, &Scope::All).unwrap();
        assert_eq!(all.len(), 2);
        let (last, _) = collect_scoped(&log, &bak, &Scope::Last).unwrap();
        assert_eq!(last.len(), 1);
        assert_eq!(last[0].original, PathBuf::from("/tmp/b.txt"));
        let (r1, _) = collect_scoped(&log, &bak, &Scope::Run("r1".into())).unwrap();
        assert_eq!(r1.len(), 1);
        assert_eq!(r1[0].original, PathBuf::from("/tmp/a.txt"));
        assert!(collect_scoped(&log, &bak, &Scope::Run("nope".into())).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn last_without_markers_is_whole_log() {
        // İşaretsiz eski loglar: `--last` her şeyi kapsar (sessiz daraltma yok).
        let dir = std::env::temp_dir().join("sweep_undo_legacy");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::create_dir_all(dir.join("bak/tmp"));
        std::fs::write(dir.join("bak/tmp/a.txt"), "a").unwrap();
        let log = dir.join("ops.log");
        std::fs::write(&log, "11 | clean | x.y | /tmp/a.txt | 1B\n").unwrap();
        let (last, _) = collect_scoped(&log, &dir.join("bak"), &Scope::Last).unwrap();
        assert_eq!(last.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn runs_list_newest_first() {
        let dir = std::env::temp_dir().join("sweep_undo_runs");
        let _ = std::fs::remove_dir_all(&dir);
        let log = two_run_log(&dir);
        let runs = runs(&log, &dir.join("bak"));
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].id, "r2");
        assert_eq!(runs[0].files, 1);
        assert_eq!(runs[0].restorable, 1);
        assert_eq!(runs[1].id, "r1");
        let listed = list_runs(&log, &dir.join("bak"));
        assert!(listed.errors.is_empty());
        assert_eq!(listed.entries.len(), 2);
        assert_eq!(listed.entries[0].option, "r2");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Test hedefi her platformda mutlak, yedeği `backup_for` ile eşleşen yol.
    fn target_in(dir: &std::path::Path) -> (PathBuf, PathBuf) {
        let target = dir.join("victim/a.txt");
        let backup = backup_for(&target, &dir.join("bak"));
        (target, backup)
    }

    #[test]
    fn without_backup_is_explicit_error_but_restorable_is_written() {
        // Yedeksiz silme geri alınamaz: açık hata + sıfır-dışı çıkış, ama
        // yedeği olan dosya yine de geri yazılır (katı abort yüzlerce
        // dosyayı tek yedeksiz yüzünden kilitlerdi).
        let dir = std::env::temp_dir().join("sweep_undo_failclosed");
        let _ = std::fs::remove_dir_all(&dir);
        let (target, backup) = target_in(&dir);
        std::fs::create_dir_all(backup.parent().unwrap()).unwrap();
        std::fs::write(&backup, "a").unwrap();
        let log = dir.join("ops.log");
        std::fs::write(
            &log,
            format!(
                "10 | run | r1 | x.y | 0B\n\
                 11 | clean | x.y | {} | 1B\n\
                 12 | clean | x.y | {} | 1B\n",
                target.display(),
                target.with_file_name("gone.txt").display(),
            ),
        )
        .unwrap();
        let ctx = RunContext::clean();
        let report = scan(&log, &dir.join("bak"), &ctx, &Scope::All);
        assert!(!report.errors.is_empty());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "a");
        // Önizleme ise yazar-mış gibi listeler ama dokunmaz.
        let _ = std::fs::remove_file(&target);
        let preview = scan(&log, &dir.join("bak"), &RunContext::preview(), &Scope::All);
        assert_eq!(preview.entries.len(), 1);
        assert!(!target.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validation_blocker_writes_nothing() {
        // Doğrulama engeli (üst dizin açılamıyor): fail-closed, sıfır yazma.
        let dir = std::env::temp_dir().join("sweep_undo_blocked");
        let _ = std::fs::remove_dir_all(&dir);
        let (target, backup) = target_in(&dir);
        std::fs::create_dir_all(backup.parent().unwrap()).unwrap();
        std::fs::write(&backup, "a").unwrap();
        // Üst dizin yerine dosya: `create_dir_all` başarısız olur.
        let blocked_parent = dir.join("notadir");
        std::fs::write(&blocked_parent, "x").unwrap();
        let blocked_target = blocked_parent.join("child.txt");
        let blocked_backup = backup_for(&blocked_target, &dir.join("bak"));
        std::fs::create_dir_all(blocked_backup.parent().unwrap()).unwrap();
        std::fs::write(&blocked_backup, "b").unwrap();
        let log = dir.join("ops.log");
        std::fs::write(
            &log,
            format!(
                "11 | clean | x.y | {} | 1B\n12 | clean | x.y | {} | 1B\n",
                target.display(),
                blocked_target.display(),
            ),
        )
        .unwrap();
        let report = scan(&log, &dir.join("bak"), &RunContext::clean(), &Scope::All);
        assert!(!report.errors.is_empty());
        assert!(!target.exists());
        assert!(!blocked_target.exists());
        assert!(!report
            .entries
            .iter()
            .any(|e| e.option == "restore" && e.label.starts_with("restored")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn real_restore_never_overwrites_existing() {
        let dir = std::env::temp_dir().join("sweep_undo_nooverwrite");
        let _ = std::fs::remove_dir_all(&dir);
        let (target, backup) = target_in(&dir);
        std::fs::create_dir_all(backup.parent().unwrap()).unwrap();
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::write(&backup, "backup").unwrap();
        std::fs::write(&target, "live").unwrap();
        let log = dir.join("ops.log");
        std::fs::write(
            &log,
            format!("11 | clean | x.y | {} | 6B\n", target.display()),
        )
        .unwrap();
        let report = scan(&log, &dir.join("bak"), &RunContext::clean(), &Scope::All);
        assert!(report.errors.is_empty());
        assert!(report
            .entries
            .iter()
            .any(|e| e.kind == crate::core::report::EntryKind::Skip));
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "live");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
