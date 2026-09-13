//! Rapor geçmişi: işlem logundan "ne kadar temizledim?" istatistiği.
//!
//! Log satırı: `ts | mod | cleaner.option | yol | 123B`. Yalnızca `clean`
//! kayıtları sayılır; GUI geçmiş ekranı günlük çubukları buradan beslenir.

use std::collections::BTreeMap;
use std::path::Path;

use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "history";

/// Günlük toplam.
#[derive(Debug, Clone)]
pub struct DayStat {
    pub day: String,
    pub bytes: u64,
    pub files: usize,
}

/// Toplu istatistik.
#[derive(Debug, Clone, Default)]
pub struct Stats {
    pub total_bytes: u64,
    pub total_files: usize,
    pub per_option: Vec<(String, u64, usize)>,
    pub per_day: Vec<DayStat>,
}

/// Epoch gününü YYYY-MM-DD yap (Howard Hinnant algoritması, bağımlılıksız).
fn ymd(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", y + i64::from(m <= 2), m, d)
}

fn now_days() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| (d.as_secs() / 86_400) as i64)
        .unwrap_or(0)
}

/// Logu çözümle; son `days_back` günü kapsa (0 = tamamı).
pub fn stats(log_path: &Path, days_back: u32) -> Stats {
    let mut stats = Stats::default();
    let cutoff = if days_back == 0 {
        0
    } else {
        now_days() - i64::from(days_back)
    };
    let mut per_option: BTreeMap<String, (u64, usize)> = BTreeMap::new();
    let mut per_day: BTreeMap<i64, (u64, usize)> = BTreeMap::new();

    // Ortak çözümleyici: ayırıcı içeren yollar (`/tmp/a | b.txt`) de sayılır.
    // Eski satır-içi sürüm `parts.len() != 5` diyerek bunları atlıyordu.
    for entry in crate::engine::logline::parse_file(log_path) {
        if entry.mode != crate::engine::logline::Mode::Clean {
            continue;
        }
        let day = (entry.ts / 86_400) as i64;
        if day < cutoff {
            continue;
        }
        let bytes = entry.bytes;
        stats.total_bytes += bytes;
        stats.total_files += 1;
        let slot = per_option.entry(entry.key()).or_insert((0, 0));
        slot.0 += bytes;
        slot.1 += 1;
        let day_slot = per_day.entry(day).or_insert((0, 0));
        day_slot.0 += bytes;
        day_slot.1 += 1;
    }

    let mut per_option: Vec<(String, u64, usize)> = per_option
        .into_iter()
        .map(|(k, (b, f))| (k, b, f))
        .collect();
    per_option.sort_by_key(|o| std::cmp::Reverse(o.1));
    stats.per_option = per_option;
    stats.per_day = per_day
        .into_iter()
        .map(|(day, (b, f))| DayStat {
            day: ymd(day),
            bytes: b,
            files: f,
        })
        .collect();
    stats
}

/// CLI raporu: özet + kategori + günlük döküm.
pub fn scan(log_path: &Path, days_back: u32) -> Report {
    let mut report = Report::new();
    if !log_path.is_file() {
        report.fail(
            CLEANER_ID,
            "log",
            format!("no operation log: {}", log_path.display()),
        );
        return report;
    }
    let stats = stats(log_path, days_back);
    report.push(Entry::new(
        EntryKind::Command,
        CLEANER_ID,
        "summary",
        format!(
            "total {} ({} files, {} categories)",
            crate::deep::safety::human(stats.total_bytes),
            stats.total_files,
            stats.per_option.len()
        ),
        None,
        0,
    ));
    for (option, bytes, files) in stats.per_option.iter().take(20) {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "by_option",
            format!(
                "{option}: {} ({files} files)",
                crate::deep::safety::human(*bytes)
            ),
            None,
            0,
        ));
    }
    for day in &stats.per_day {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "by_day",
            format!(
                "{}: {} ({} files)",
                day.day,
                crate::deep::safety::human(day.bytes),
                day.files
            ),
            None,
            0,
        ));
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_math() {
        assert_eq!(ymd(0), "1970-01-01");
        assert_eq!(ymd(20_000), "2024-10-04");
    }

    #[test]
    fn aggregates() {
        let dir = std::env::temp_dir().join("sweep_hist_test");
        let _ = std::fs::create_dir_all(&dir);
        let log = dir.join("ops.log");
        std::fs::write(
            &log,
            "1700000000 | clean | apt.cache | /a.deb | 100B\n\
             1700000000 | preview | apt.cache | /b.deb | 50B\n\
             1700086400 | clean | apt.cache | /c.deb | 200B\n",
        )
        .unwrap();
        let stats = stats(&log, 0);
        assert_eq!(stats.total_bytes, 300);
        assert_eq!(stats.total_files, 2);
        assert_eq!(stats.per_day.len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn separator_inside_path_is_counted() {
        // Regresyon: eski satır-içi çözümleyici alan sayısına bakıyordu ve
        // ayırıcı içeren yolları saymıyordu.
        let dir = std::env::temp_dir().join("sweep_hist_pipe");
        let _ = std::fs::create_dir_all(&dir);
        let log = dir.join("ops.log");
        std::fs::write(
            &log,
            "1700000000 | clean | a.b | /tmp/x | y.txt | 40B\n\
             1700000000 | clean | a.b | /tmp/z.txt | 60B\n",
        )
        .unwrap();
        let stats = stats(&log, 0);
        assert_eq!(stats.total_files, 2);
        assert_eq!(stats.total_bytes, 100);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
