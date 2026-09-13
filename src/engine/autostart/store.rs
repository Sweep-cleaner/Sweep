//! Değişiklik öncesi yedekler.
//!
//! Her değişiklik için tek bir JSON dosyası yazılır:
//! `<veri>/autostart/backups/<utc-zaman>-<güvenli-id>.json`. Dosya, değişiklik
//! öncesi ham durumu (`payload`) ve yükün sha256'sını taşır. Geri alma yedeği
//! okuyup önceki durumu **gerçekten** kurar.
//!
//! "Fail closed": yedek yazılamazsa [`Store::write`] hata döner ve çağıran
//! değişikliği yapmaz.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Tek yedek dosyası.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupFile {
    /// UTC RFC3339.
    pub ts: String,
    /// Sıralama anahtarı: Unix epoch'tan bu yana nanosaniye (aynı saniyedeki
    /// iki yedeği kesin sıralar; dosya adı sıralamasına güvenilmez).
    #[serde(default)]
    pub seq: u64,
    /// Girdi kimliği.
    pub id: String,
    pub name: String,
    /// Değişikliği yapan işlem (`disable`, `remove`, `edit`…).
    pub op: String,
    /// Değişiklik öncesi kısa durum (`enabled`/`disabled`/`absent`).
    pub before: Option<String>,
    /// Değişiklik öncesi komut (doğrulama için).
    pub before_command: Option<String>,
    /// `payload`ın sha256'sı (onaltılık).
    pub sha256: String,
    /// Ham değişiklik öncesi durum (kayıt değeri + türü, dosya baytları, görev XML'i…).
    pub payload: serde_json::Value,
}

/// `payload`ın kararlı serileştirmesi üzerinden sha256.
pub fn payload_hash(payload: &serde_json::Value) -> String {
    let text = serde_json::to_string(payload).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Dosya adı için güvenli kimlik: harf/rakam/`-`/`_` dışındakiler `_` olur.
pub fn safe_id(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for ch in id.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out.truncate(80);
    if out.is_empty() {
        out.push_str("item");
    }
    out
}

/// Yedek dizini.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

impl Store {
    /// Belirli bir dizini kullanan yedek deposu (testler için).
    pub fn at(root: PathBuf) -> Self {
        Self { root }
    }

    /// Varsayılan konum: `<veri>/autostart/backups`.
    pub fn default_store() -> Option<Self> {
        super::data_dir().map(|d| Self::at(d.join("backups")))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Veri dizinine göre göreli yol (`backups/<dosya>`).
    pub fn relative(&self, path: &Path) -> String {
        let dir = self
            .root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "backups".to_string());
        let file = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        format!("{dir}/{file}")
    }

    /// Yedek yaz. Başarıda (yol, dosya) döner.
    ///
    /// Yazdıktan sonra geri okuyup sha256'yı doğrular; tutmazsa dosyayı siler
    /// ve hata döner — çağıran değişikliği yapmamalıdır.
    #[allow(clippy::too_many_arguments)]
    pub fn write(
        &self,
        id: &str,
        name: &str,
        op: &str,
        before: Option<String>,
        before_command: Option<String>,
        payload: serde_json::Value,
    ) -> Result<(PathBuf, BackupFile), String> {
        std::fs::create_dir_all(&self.root)
            .map_err(|e| format!("cannot create backup dir {}: {e}", self.root.display()))?;
        let seq = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let file = BackupFile {
            ts: super::now_rfc3339(),
            seq,
            id: id.to_string(),
            name: name.to_string(),
            op: op.to_string(),
            before,
            before_command,
            sha256: payload_hash(&payload),
            payload,
        };
        // Aynı milisaniyede iki yazım olursa dosya adı çakışmasın.
        let mut path = self
            .root
            .join(format!("{}-{}.json", super::now_stamp(), safe_id(id)));
        let mut counter = 2;
        while path.exists() {
            path = self
                .root
                .join(format!("{}-{}-{counter}.json", super::now_stamp(), safe_id(id)));
            counter += 1;
        }
        let text = serde_json::to_string_pretty(&file)
            .map_err(|e| format!("cannot encode backup: {e}"))?;
        // Doğrudan yaz; hata olursa değişiklik yapılmaz.
        if let Err(e) = std::fs::write(&path, &text) {
            return Err(format!("cannot write backup {}: {e}", path.display()));
        }
        // Yaz-oku-doğrula.
        match self.read(&path) {
            Ok(read_back) if read_back.sha256 == file.sha256 => Ok((path, file)),
            Ok(_) => {
                let _ = std::fs::remove_file(&path);
                Err(format!(
                    "backup verification failed (hash mismatch): {}",
                    path.display()
                ))
            }
            Err(e) => {
                let _ = std::fs::remove_file(&path);
                Err(e)
            }
        }
    }

    /// Yedeği oku ve sha256'sını doğrula.
    pub fn read(&self, path: &Path) -> Result<BackupFile, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read backup {}: {e}", path.display()))?;
        let file: BackupFile = serde_json::from_str(&text)
            .map_err(|e| format!("cannot parse backup {}: {e}", path.display()))?;
        if file.sha256 != payload_hash(&file.payload) {
            return Err(format!(
                "backup is corrupt (sha256 mismatch): {}",
                path.display()
            ));
        }
        Ok(file)
    }

    /// Tüm yedekler, kronolojik (eskiden yeniye) sıralı.
    pub fn list(&self) -> Vec<(PathBuf, BackupFile)> {
        let Ok(dir) = std::fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut out: Vec<(PathBuf, BackupFile)> = dir
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map(|e| e == "json").unwrap_or(false))
            .filter_map(|p| self.read(&p).ok().map(|f| (p, f)))
            .collect();
        // `seq` kesin kronolojik sıradır; dosya adı yalnız eşitlikte devreye girer.
        out.sort_by(|a, b| {
            a.1.seq
                .cmp(&b.1.seq)
                .then_with(|| a.0.file_name().cmp(&b.0.file_name()))
        });
        out
    }

    /// Bir girdi için en son yedek (dosya adına değil, kayıtlı `id`ye göre).
    pub fn latest_for(&self, id: &str) -> Option<(PathBuf, BackupFile)> {
        self.list()
            .into_iter()
            .filter(|(_, file)| file.id == id)
            .next_back()
    }

    /// Genel olarak en son yedek (`--last`).
    pub fn latest(&self) -> Option<(PathBuf, BackupFile)> {
        self.list().into_iter().next_back()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_store(tag: &str) -> Store {
        let dir = std::env::temp_dir().join(format!(
            "sweep_store_{tag}_{}_{}",
            std::process::id(),
            super::super::short_hash(tag)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        Store::at(dir.join("backups"))
    }

    #[test]
    fn write_read_and_verify_round_trip() {
        let store = temp_store("roundtrip");
        let payload = json!({"kind": "registry", "data": "C:\\a.exe"});
        let (path, _written) = store
            .write(
                "startup|user|registry-run|Foo",
                "Foo",
                "disable",
                Some("enabled".into()),
                Some("C:\\a.exe".into()),
                payload.clone(),
            )
            .unwrap();
        assert!(path.exists());
        let read = store.read(&path).unwrap();
        assert_eq!(read.id, "startup|user|registry-run|Foo");
        assert_eq!(read.before.as_deref(), Some("enabled"));
        assert_eq!(read.sha256, payload_hash(&payload));
        assert!(store.relative(&path).starts_with("backups/"));
    }

    #[test]
    fn tampered_backup_is_rejected() {
        let store = temp_store("tamper");
        let (path, _) = store
            .write("id", "name", "edit", None, None, json!({"a": 1}))
            .unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let tampered = text.replace("\"a\": 1", "\"a\": 2");
        std::fs::write(&path, tampered).unwrap();
        let err = store.read(&path).unwrap_err();
        assert!(err.contains("corrupt"), "{err}");
    }

    #[test]
    fn fail_closed_when_backup_cannot_be_written() {
        // Bir dosyayı dizin gibi kullanmak create_dir_all'ı başarısız kılar.
        let dir = std::env::temp_dir().join(format!(
            "sweep_store_blocker_{}_{}",
            std::process::id(),
            super::super::short_hash("blocker")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let blocker = dir.join("blocker");
        std::fs::write(&blocker, b"not a directory").unwrap();
        let store = Store::at(blocker.join("backups"));
        let err = store
            .write("id", "name", "disable", None, None, json!({"x": 1}))
            .unwrap_err();
        assert!(err.contains("cannot create backup dir"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn latest_for_picks_the_newest_matching_id() {
        let store = temp_store("latest");
        store
            .write("id-a", "A", "disable", None, None, json!({"n": 1}))
            .unwrap();
        // Aynı kimliğe ikinci yedek (dosya adı zaman damgasıyla ayrışır).
        let (second, _) = store
            .write("id-a", "A", "edit", None, None, json!({"n": 2}))
            .unwrap();
        store
            .write("id-b", "B", "disable", None, None, json!({"n": 3}))
            .unwrap();
        let (path, file) = store.latest_for("id-a").unwrap();
        assert_eq!(path.file_name(), second.file_name());
        assert_eq!(file.payload["n"], 2);
        // Genel en son yedek.
        let (_, last) = store.latest().unwrap();
        assert_eq!(last.id, "id-b");
    }

    #[test]
    fn safe_id_normalises_separators() {
        assert_eq!(safe_id("startup|user|registry-run|Foo"), "startup_user_registry-run_Foo");
        assert_eq!(safe_id(""), "item");
    }
}
