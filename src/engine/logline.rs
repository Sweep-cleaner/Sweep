//! İşlem logu satırı çözümleyici.
//!
//! Log biçimi (`deep::safety::log_operation` yazar):
//!
//! ```text
//! <unix-ts> | <mod> | <cleaner>.<option> | <yol> | <bayt>B
//! ```
//!
//! Alanlar `" | "` ile ayrılır. **Yolun kendisi ayırıcıyı içerebilir**
//! (`/tmp/a | b.txt`), bu yüzden satır soldan *değil* şöyle okunur: ilk üç
//! alan sabit, son alan bayt, aradaki her şey yoldur. Bu kural bir kez burada
//! tanımlanır; `undo` ve `history` (ve ileride eklenecek her tüketici) aynı
//! çözümleyiciyi kullanır.
//!
//! Eskiden bu mantık iki yerde kopyalıydı ve `history` sürümü
//! `parts.len() != 5` diyerek ayırıcı içeren yolları sessizce atıyordu;
//! tek kaynak bu sapmayı ortadan kaldırır.

use std::path::{Path, PathBuf};

/// Kayıt modu: hangi aşamada yazıldığı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Gerçekten silindi ( `--backup-dir` varsa yedeği alınmış olabilir).
    Clean,
    /// Yalnızca önizleme: hiçbir şey değişmedi.
    Preview,
    /// Koruma listesi / kritik dizin nedeniyle atlandı.
    Skip,
    /// Temizlik koşusunun başlangıç işareti (`undo --last/--run` gruplaması).
    Run,
}

impl Mode {
    /// Metinden modu oku.
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim() {
            "clean" => Some(Mode::Clean),
            "preview" => Some(Mode::Preview),
            "skip" => Some(Mode::Skip),
            "run" => Some(Mode::Run),
            _ => None,
        }
    }

    /// Logda kullanılan sabit ad.
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Clean => "clean",
            Mode::Preview => "preview",
            Mode::Skip => "skip",
            Mode::Run => "run",
        }
    }
}

/// Çözümlenmiş tek log satırı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    /// Unix zaman damgası (saniye).
    pub ts: u64,
    /// Kaydın yazıldığı aşama.
    pub mode: Mode,
    /// Cleaner kimliği (`cleaner.option` ayrımından önceki parça).
    pub cleaner: String,
    /// Seçenek kimliği (`cleaner.option` ayrımından sonraki parça).
    pub option: String,
    /// Etkilenen yol.
    pub path: PathBuf,
    /// Geri kazanılan bayt.
    pub bytes: u64,
}

impl LogLine {
    /// `cleaner.option` birleşik anahtarı (istatistiklerde kullanılır).
    pub fn key(&self) -> String {
        format!("{}.{}", self.cleaner, self.option)
    }
}

/// `a.b` → `("a", "b")`; nokta yoksa `("a", "")`.
fn split_key(text: &str) -> (String, String) {
    match text.split_once('.') {
        Some((cleaner, option)) => (cleaner.to_string(), option.to_string()),
        None => (text.to_string(), String::new()),
    }
}

/// Tek satırı çözümle. Bozuksa `None`.
pub fn parse(line: &str) -> Option<LogLine> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.is_empty() {
        return None;
    }
    let parts: Vec<&str> = line.split(" | ").collect();
    if parts.len() < 5 {
        return None;
    }
    let mode = Mode::parse(parts[1])?;
    let ts: u64 = parts[0].trim().parse().ok()?;
    let bytes = parts
        .last()?
        .trim()
        .strip_suffix('B')?
        .trim()
        .parse::<u64>()
        .ok()?;
    // parts[3..len-1]: yolun kendisi ayırıcı içerebilir.
    let path = PathBuf::from(parts[3..parts.len() - 1].join(" | "));
    let (cleaner, option) = split_key(parts[2].trim());
    if cleaner.is_empty() || path.as_os_str().is_empty() {
        return None;
    }
    Some(LogLine {
        ts,
        mode,
        cleaner,
        option,
        path,
        bytes,
    })
}

/// Bir log dosyasındaki tüm geçerli satırlar (bozuk satırlar atlanır).
pub fn parse_file(path: &Path) -> Vec<LogLine> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    text.lines().filter_map(parse).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_line() {
        let line = parse("1700000000 | clean | apt.cache | /a.deb | 100B").unwrap();
        assert_eq!(line.ts, 1_700_000_000);
        assert_eq!(line.mode, Mode::Clean);
        assert_eq!(line.cleaner, "apt");
        assert_eq!(line.option, "cache");
        assert_eq!(line.path, PathBuf::from("/a.deb"));
        assert_eq!(line.bytes, 100);
        assert_eq!(line.key(), "apt.cache");
    }

    #[test]
    fn separator_inside_path_survives() {
        // undo ve history bunu aynı şekilde okumak zorunda: eski history
        // sürümü bu satırı (6 alan) sessizce atlıyordu.
        let line = parse("1700000000 | clean | bigfiles.bigfiles | /tmp/a | b.txt | 1B").unwrap();
        assert_eq!(line.path, PathBuf::from("/tmp/a | b.txt"));
        assert_eq!(line.bytes, 1);
    }

    #[test]
    fn run_marker_parses() {
        // Koşu işareti: `cleaner` = koşu kimliği, yol = seçim özeti.
        let line = parse("1700000000 | run | r17-42-0 | firefox.cache, x.y | 0B").unwrap();
        assert_eq!(line.mode, Mode::Run);
        assert_eq!(line.cleaner, "r17-42-0");
        assert_eq!(line.path, PathBuf::from("firefox.cache, x.y"));
    }

    #[test]
    fn modes_and_rejects() {
        assert_eq!(
            parse("1 | preview | a.b | /x | 1B").unwrap().mode,
            Mode::Preview
        );
        assert_eq!(parse("1 | skip | a.b | /x | 1B").unwrap().mode, Mode::Skip);
        // Bilinmeyen mod, eksik alan, baytsız satır, boş satır.
        assert!(parse("1 | wat | a.b | /x | 1B").is_none());
        assert!(parse("1 | clean | a.b | /x").is_none());
        assert!(parse("1 | clean | a.b | /x | kB").is_none());
        assert!(parse("").is_none());
        assert!(parse("1 | clean | a.b |  | 1B").is_none());
    }

    #[test]
    fn parses_whole_file_skipping_garbage() {
        let dir = std::env::temp_dir().join("sweep_logline_test");
        let _ = std::fs::create_dir_all(&dir);
        let log = dir.join("ops.log");
        std::fs::write(
            &log,
            "1 | clean | a.b | /x | 10B\nnot a log line\n2 | clean | a.b | /y | 20B\n",
        )
        .unwrap();
        let lines = parse_file(&log);
        assert_eq!(lines.len(), 2);
        assert_eq!(lines.iter().map(|l| l.bytes).sum::<u64>(), 30);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
