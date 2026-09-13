//! Denetim defteri: her denenen işlem için tek satır JSONL.
//!
//! Dosya `<veri>/autostart/audit.jsonl` altındadır ve **ekleme-only**'dir.
//! Süreç yeniden başlasa da okunabilir; bellekte önbellek tutulmaz. Bozuk ya
//! da yarım kalmış son satır sessizce atlanır (panik yok).

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Tek denetim satırı. Alan adları ve sırası sözleşmedir.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LedgerEntry {
    /// UTC RFC3339 (`2026-09-13T08:11:22Z`).
    pub ts: String,
    /// İşletim sisteminden kullanıcı (`DOMAIN\user`).
    pub user: String,
    /// `list|enable|disable|remove|edit|rollback` …
    pub op: String,
    pub id: String,
    pub name: String,
    pub source: String,
    pub location: String,
    pub before: Option<String>,
    pub after: Option<String>,
    /// `ok` | `failed` | `skipped` | `unverified`.
    pub result: String,
    /// Yedeğin veri dizinine göreli yolu (yoksa `null`).
    pub backup: Option<String>,
    pub detail: Option<String>,
}

impl LedgerEntry {
    /// Yeni satır iskeleti; çağıran alanları doldurur.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        op: impl Into<String>,
        id: impl Into<String>,
        name: impl Into<String>,
        source: impl Into<String>,
        location: impl Into<String>,
    ) -> Self {
        Self {
            ts: super::now_rfc3339(),
            user: super::os_user(),
            op: op.into(),
            id: id.into(),
            name: name.into(),
            source: source.into(),
            location: location.into(),
            before: None,
            after: None,
            result: "failed".to_string(),
            backup: None,
            detail: None,
        }
    }

    /// Değiştirici kurucu: durum alanları.
    pub fn with_states(
        mut self,
        before: Option<String>,
        after: Option<String>,
        result: impl Into<String>,
    ) -> Self {
        self.before = before;
        self.after = after;
        self.result = result.into();
        self
    }

    /// Değiştirici kurucu: yedek yolu ve ayrıntı.
    pub fn with_backup(mut self, backup: Option<String>, detail: Option<String>) -> Self {
        self.backup = backup;
        self.detail = detail;
        self
    }

    /// CSV satırı (başlık hariç).
    pub fn to_csv_line(&self) -> String {
        let cells = [
            self.ts.as_str(),
            self.user.as_str(),
            self.op.as_str(),
            self.id.as_str(),
            self.name.as_str(),
            self.source.as_str(),
            self.location.as_str(),
            self.before.as_deref().unwrap_or(""),
            self.after.as_deref().unwrap_or(""),
            self.result.as_str(),
            self.backup.as_deref().unwrap_or(""),
            self.detail.as_deref().unwrap_or(""),
        ];
        cells
            .iter()
            .map(|c| csv_escape(c))
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// CSV başlığı (sözleşmedeki alan sırası).
pub const CSV_HEADER: &str = "ts,user,op,id,name,source,location,before,after,result,backup,detail";

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// JSONL defteri.
#[derive(Debug, Clone)]
pub struct Ledger {
    path: PathBuf,
}

impl Ledger {
    /// Belirli bir dosyayı kullanan defter (testler için).
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Varsayılan konum: `<veri>/autostart/audit.jsonl`.
    pub fn default_path() -> Option<PathBuf> {
        super::data_dir().map(|d| d.join("audit.jsonl"))
    }

    /// Varsayılan konumdaki defter; veri dizini yoksa `None`.
    pub fn open_default() -> Option<Self> {
        Self::default_path().map(Self::new)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Bir satır ekle (dosya yoksa oluşturulur). Hata diske yazılamadıysa döner.
    pub fn append(&self, entry: &LedgerEntry) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("cannot create ledger dir {}: {e}", parent.display()))?;
        }
        let line = serde_json::to_string(entry).map_err(|e| format!("cannot encode ledger: {e}"))?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| format!("cannot open ledger {}: {e}", self.path.display()))?;
        writeln!(file, "{line}").map_err(|e| format!("cannot write ledger: {e}"))?;
        file.flush().map_err(|e| format!("cannot flush ledger: {e}"))?;
        Ok(())
    }

    /// Tüm satırları oku. Bozuk/yarım satırlar atlanır; dosya yoksa boş liste.
    pub fn read_all(&self) -> Vec<LedgerEntry> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(t) => t,
            Err(_) => return Vec::new(),
        };
        text.lines()
            .filter(|l| !l.trim().is_empty())
            .filter_map(|line| serde_json::from_str::<LedgerEntry>(line).ok())
            .collect()
    }

    /// Son `limit` satır (en yeni sonda kalır). `limit == 0` hepsi.
    pub fn tail(&self, limit: usize) -> Vec<LedgerEntry> {
        let mut all = self.read_all();
        if limit > 0 && all.len() > limit {
            all.drain(0..all.len() - limit);
        }
        all
    }

    /// JSON dizisi olarak serileştir (pretty).
    pub fn to_json(&self, entries: &[LedgerEntry]) -> String {
        serde_json::to_string_pretty(entries).unwrap_or_else(|_| "[]".to_string())
    }

    /// CSV olarak serileştir (başlık + satırlar).
    pub fn to_csv(&self, entries: &[LedgerEntry]) -> String {
        let mut out = String::from(CSV_HEADER);
        out.push('\n');
        for entry in entries {
            out.push_str(&entry.to_csv_line());
            out.push('\n');
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_ledger(tag: &str) -> Ledger {
        let dir = std::env::temp_dir().join(format!(
            "sweep_ledger_{tag}_{}_{}",
            std::process::id(),
            super::super::short_hash(tag)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Ledger::new(dir.join("audit.jsonl"))
    }

    fn sample(op: &str) -> LedgerEntry {
        LedgerEntry::new(op, "startup|user|registry-run|OneDrive", "OneDrive", "registry-run", "HKCU\\Run")
    }

    #[test]
    fn append_read_round_trip() {
        let ledger = temp_ledger("roundtrip");
        let mut first = sample("disable");
        first = first.with_states(Some("enabled".into()), Some("disabled".into()), "ok");
        first = first.with_backup(Some("autostart/backups/x.json".into()), None);
        ledger.append(&first).unwrap();
        let second = sample("enable").with_states(Some("disabled".into()), Some("enabled".into()), "ok");
        ledger.append(&second).unwrap();

        let all = ledger.read_all();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0], first);
        assert_eq!(all[1], second);
        // Alan adları sözleşmeye uygun ve hepsi yazılıyor (null dahil).
        let raw = std::fs::read_to_string(ledger.path()).unwrap();
        let value: serde_json::Value = serde_json::from_str(raw.lines().next().unwrap()).unwrap();
        for key in [
            "ts", "user", "op", "id", "name", "source", "location", "before", "after",
            "result", "backup", "detail",
        ] {
            assert!(value.get(key).is_some(), "eksik alan: {key}");
        }
        assert!(value["detail"].is_null());
        assert_eq!(value["result"], "ok");
        // Yeniden başlatma benzetimi: yeni bir defter nesnesi aynı dosyayı okur.
        let reopened = Ledger::new(ledger.path().to_path_buf());
        assert_eq!(reopened.read_all().len(), 2);
    }

    #[test]
    fn corrupt_last_line_is_tolerated() {
        let ledger = temp_ledger("corrupt");
        let good = sample("disable").with_states(Some("enabled".into()), Some("disabled".into()), "ok");
        ledger.append(&good).unwrap();
        // Yarım yazılmış son satır.
        let mut file = std::fs::OpenOptions::new()
            .append(true)
            .open(ledger.path())
            .unwrap();
        writeln!(file, "{{\"ts\":\"2026-09-13T08:1").unwrap();
        drop(file);

        let all = ledger.read_all();
        assert_eq!(all.len(), 1, "bozuk satır atlanmalı: {all:?}");
        assert_eq!(all[0], good);

        // Bozuk satırdan sonra ekleme hâlâ çalışır ve okunur.
        let next = sample("enable").with_states(Some("disabled".into()), Some("enabled".into()), "ok");
        ledger.append(&next).unwrap();
        let all = ledger.read_all();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn csv_export_escapes_and_covers_all_fields() {
        let ledger = temp_ledger("csv");
        let mut entry = sample("edit");
        entry.detail = Some("new command: \"a,b\"".to_string());
        entry = entry.with_states(Some("enabled".into()), Some("enabled".into()), "ok");
        let csv = ledger.to_csv(&[entry]);
        let mut lines = csv.lines();
        assert_eq!(lines.next().unwrap(), CSV_HEADER);
        let row = lines.next().unwrap();
        assert!(row.contains("\"new command: \"\"a,b\"\"\""), "{row}");
        // 12 sütun: kaçışlı virgüller ayrıştırıcıyı bozmamalı (kaba kontrol).
        assert_eq!(row.matches("\"").count() % 2, 0);
    }

    #[test]
    fn json_export_is_an_array() {
        let ledger = temp_ledger("json");
        ledger.append(&sample("disable")).unwrap();
        let text = ledger.to_json(&ledger.read_all());
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(value.is_array());
        assert_eq!(value.as_array().unwrap().len(), 1);
    }

    #[test]
    fn tail_returns_the_newest_entries() {
        let ledger = temp_ledger("tail");
        for i in 0..5 {
            let mut e = sample("disable");
            e.name = format!("item{i}");
            ledger.append(&e).unwrap();
        }
        let tail = ledger.tail(2);
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0].name, "item3");
        assert_eq!(tail[1].name, "item4");
    }

    #[test]
    fn missing_file_reads_empty_without_panic() {
        let ledger = temp_ledger("missing");
        assert!(ledger.read_all().is_empty());
    }
}
