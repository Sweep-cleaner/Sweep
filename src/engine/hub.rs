//! Topluluk kural deposu: `sweep hub list/install/remove/update`.
//!
//! Kurallar TOML index'inden bulunur, sha256 ile doğrulanır, kullanıcı
//! dizinine kurulur ve **güvensiz** sayılır. Harici tanımlar, sağlayıcı
//! kayıt defterindeki `privileged` bayrağına göre ayrıcalıklı eylemleri
//! (`process`, `winreg`, `apt.autoremove`, …) çalıştıramaz — `--trust-external`
//! verilmedikçe. Ayrıcalıklı eylem içeren kurallar kurulum öncesi taramada
//! reddedilir; tarama kuralı gerçekten parse edip her eylemin komutunu kayıt
//! defterine göre kontrol eder, böylece tek tırnaklı (`command = 'process'`)
//! yazımlar da yakalanır.

use std::path::PathBuf;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "hub";

/// Index girdisi (uzaktaki index.toml şeması).
#[derive(Debug, Clone, Deserialize)]
pub struct HubEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub version: String,
    /// Index'e göreli dosya yolu (örn. "gaming.toml").
    pub file: String,
    /// Dosyanın sha256 özeti (hex).
    pub sha256: String,
}

#[derive(Debug, Deserialize)]
struct Index {
    #[serde(default)]
    cleaner: Vec<HubEntry>,
}

/// Kullanıcı hub dizini (`<config>/hub`, taşınabilir modda `<base>/hub`).
pub fn hub_dir() -> Option<PathBuf> {
    if let Some(base) = crate::config::portable_base() {
        return Some(base.join("hub"));
    }
    crate::platform::config_dir().map(|d| d.join("hub"))
}

/// Önbellekteki index yolu.
fn cached_index() -> Option<PathBuf> {
    hub_dir().map(|d| d.join("index.toml"))
}

fn download(url: &str, dest: &std::path::Path) -> Result<(), String> {
    let (prog, args): (&str, Vec<&str>) = if crate::fsutil::path_exists_in_path("curl") {
        ("curl", vec!["-fsSL", "-o"])
    } else if crate::fsutil::path_exists_in_path("wget") {
        ("wget", vec!["-q", "-O"])
    } else {
        return Err("no curl/wget".into());
    };
    let dest_s = dest.to_string_lossy().into_owned();
    let mut full: Vec<&str> = args;
    full.extend([dest_s.as_str(), url]);
    match crate::fsutil::run_command(prog, &full, true) {
        Ok((0, _, _)) => Ok(()),
        Ok((code, _, stderr)) => Err(format!("{prog} exit {code}: {stderr}")),
        Err(err) => Err(format!("indirilemedi: {err}")),
    }
}

fn sha256_file(path: &PathBuf) -> Result<String, String> {
    use std::fmt::Write as _;
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read: {e}"))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let mut hex = String::with_capacity(64);
    for b in hasher.finalize() {
        let _ = write!(hex, "{b:02x}");
    }
    Ok(hex)
}

fn parse_index(text: &str) -> Result<Vec<HubEntry>, String> {
    toml::from_str::<Index>(text)
        .map(|i| i.cleaner)
        .map_err(|e| format!("index bozuk: {e}"))
}

/// Ayrıcalıklı eylem taraması: kurulum öncesi güvenlik filtresi.
///
/// Kuralı gerçekten parse edip her eylemin `command` değerini sağlayıcı
/// kayıt defterindeki `privileged` bayrağına göre kontrol eder. Bu sayede:
/// * `command = 'process'` gibi tek tırnaklı yazımlar da yakalanır (ham metin
///   araması bu yüzden atlanabilirdi);
/// * sağlayıcı tablosuna yeni ayrıcalıklı komut eklenince buradaki liste elle
///   güncellenmeden otomatik kapsanır.
fn privileged_actions(def_text: &str) -> Vec<String> {
    let Ok(def) = crate::definition::native::parse_native(def_text, "hub-rule") else {
        // Bozuk kural zaten kurulumda `parse_native` ile reddedilir; burada
        // ayrıcalıklı eylem bulamadık diyip asıl doğrulamaya bırakırız.
        return Vec::new();
    };
    let mut found = Vec::new();
    for option in &def.options {
        for action in &option.actions {
            if crate::action::provider::is_privileged(&action.command) {
                found.push(action.command.clone());
            }
        }
    }
    found
}

/// Index'i yenile (önbelleğe yaz).
pub fn refresh(url: &str) -> Result<Vec<HubEntry>, String> {
    let dest = cached_index().ok_or_else(|| "no config dir".to_string())?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("cannot create dir: {e}"))?;
    }
    let tmp = dest.with_extension("toml.tmp");
    download(url, &tmp)?;
    let text = std::fs::read_to_string(&tmp).map_err(|e| format!("cannot read: {e}"))?;
    let entries = parse_index(&text)?;
    std::fs::rename(&tmp, &dest).map_err(|e| format!("cannot write cache: {e}"))?;
    Ok(entries)
}

/// Önbellekteki index'i oku (yoksa boş).
pub fn cached() -> Vec<HubEntry> {
    cached_index()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| parse_index(&t).ok())
        .unwrap_or_default()
}

fn index_base(url: &str) -> String {
    url.rsplit_once('/')
        .map(|(b, _)| b.to_string())
        .unwrap_or_default()
}

/// Kuralı kur (indir + sha256 + güvenlik filtresi + TOML doğrulama).
pub fn install(entry: &HubEntry, index_url: &str) -> Result<String, String> {
    let dir = hub_dir().ok_or_else(|| "no config dir".to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create dir: {e}"))?;
    if entry.file.contains('/') || entry.file.contains('\\') || entry.file.contains("..") {
        return Err(format!("unsafe file name: {}", entry.file));
    }
    if !entry.file.ends_with(".toml") {
        return Err("only .toml rules supported".into());
    }
    let url = format!("{}/{}", index_base(index_url), entry.file);
    let dest = dir.join(&entry.file);
    let tmp = dest.with_extension("toml.tmp");
    download(&url, &tmp)?;
    let sum = sha256_file(&tmp)?;
    if sum != entry.sha256.to_lowercase() {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!(
            "sha256 mismatch (index: {}…, got: {}…)",
            &entry.sha256[..8.min(entry.sha256.len())],
            &sum[..8]
        ));
    }
    let text = std::fs::read_to_string(&tmp).map_err(|e| format!("cannot read: {e}"))?;
    let risky = privileged_actions(&text);
    if !risky.is_empty() {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!(
            "contains privileged actions ({}), refused",
            risky.join(", ")
        ));
    }
    // TOML şema doğrulaması (bozuk kural depoyu kirletmesin).
    crate::definition::native::parse_native(&text, &entry.id)
        .map_err(|e| format!("invalid rule: {e}"))?;
    std::fs::rename(&tmp, &dest).map_err(|e| format!("cannot write: {e}"))?;
    Ok(format!("installed: {} ({})", entry.name, dest.display()))
}

/// Kurulu kuralı kaldır.
pub fn remove(id: &str) -> Result<String, String> {
    let dir = hub_dir().ok_or_else(|| "no config dir".to_string())?;
    let target = format!("{id}.toml");
    let dest = dir.join(&target);
    if !dest.is_file() {
        return Err(format!("not installed: {id}"));
    }
    std::fs::remove_file(&dest).map_err(|e| format!("cannot delete: {e}"))?;
    Ok(format!("removed: {id}"))
}

/// Kurulu kuralların dosya adları.
pub fn installed() -> Vec<String> {
    let Some(dir) = hub_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "toml").unwrap_or(false))
        .filter(|p| p.file_name().map(|n| n != "index.toml").unwrap_or(false))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    out.sort();
    out
}

fn entry_report(entry: &HubEntry, installed: bool, report: &mut Report) {
    report.push(Entry::new(
        EntryKind::Command,
        CLEANER_ID,
        if installed { "installed" } else { "available" },
        format!(
            "{} — {} (by: {}, v{}){}",
            entry.id,
            if entry.description.is_empty() {
                &entry.name
            } else {
                &entry.description
            },
            if entry.author.is_empty() {
                "?"
            } else {
                &entry.author
            },
            if entry.version.is_empty() {
                "?"
            } else {
                &entry.version
            },
            if installed { " [installed]" } else { "" }
        ),
        None,
        0,
    ));
}

/// `op`: list | install <id> | remove <id> | update.
pub fn run(op: &str, target: Option<&str>, url: &str) -> Report {
    let mut report = Report::new();
    match op {
        "list" => {
            let entries = cached();
            if entries.is_empty() {
                report.push(Entry::new(
                    EntryKind::Command,
                    CLEANER_ID,
                    "list",
                    "empty cache — run `sweep hub update` first",
                    None,
                    0,
                ));
                return report;
            }
            let have = installed();
            for entry in &entries {
                entry_report(entry, have.contains(&entry.id), &mut report);
            }
        }
        "update" => match refresh(url) {
            Ok(entries) => {
                // Kurulu kuralları sessizce tazele.
                let have = installed();
                let mut refreshed = 0;
                for entry in &entries {
                    if have.contains(&entry.id) && install(entry, url).is_ok() {
                        refreshed += 1;
                    }
                }
                report.push(Entry::new(
                    EntryKind::Command,
                    CLEANER_ID,
                    "update",
                    format!(
                        "index refreshed: {} rules, {refreshed} installed updated",
                        entries.len()
                    ),
                    None,
                    0,
                ));
            }
            Err(err) => report.fail(CLEANER_ID, "update", err),
        },
        "install" => {
            let Some(id) = target else {
                report.fail(CLEANER_ID, "install", "rule id required (`sweep hub list`)");
                return report;
            };
            let entries = cached();
            let Some(entry) = entries.iter().find(|e| e.id == id) else {
                report.fail(
                    CLEANER_ID,
                    "install",
                    format!("not in index: {id} (run `sweep hub update` first)"),
                );
                return report;
            };
            match install(entry, url) {
                Ok(done) => report.push(Entry::new(
                    EntryKind::Command,
                    CLEANER_ID,
                    "install",
                    done,
                    None,
                    0,
                )),
                Err(err) => report.fail(CLEANER_ID, "install", err),
            }
        }
        "remove" => {
            let Some(id) = target else {
                report.fail(CLEANER_ID, "remove", "rule id required");
                return report;
            };
            match remove(id) {
                Ok(done) => report.push(Entry::new(
                    EntryKind::Command,
                    CLEANER_ID,
                    "remove",
                    done,
                    None,
                    0,
                )),
                Err(err) => report.fail(CLEANER_ID, "remove", err),
            }
        }
        other => report.fail(
            CLEANER_ID,
            "args",
            format!("unknown op: {other} (list/install/remove/update)"),
        ),
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_privileged() {
        let double =
            "id = \"t\"\nname = \"T\"\n[[option]]\nid = \"x\"\nlabel = \"y\"\n[[option.action]]\ncommand = \"process\"\n";
        assert!(privileged_actions(double).contains(&"process".to_string()));
        // Tek tırnaklı yazım eski ham-metin taramasını atlatıyordu.
        let single =
            "id = \"t\"\nname = \"T\"\n[[option]]\nid = \"x\"\nlabel = \"y\"\n[[option.action]]\ncommand = 'process'\n";
        assert!(privileged_actions(single).contains(&"process".to_string()));
        // Ayrıcalıklı olmayan eylem serbest.
        let safe =
            "id = \"t\"\nname = \"T\"\n[[option]]\nid = \"x\"\nlabel = \"y\"\n[[option.action]]\ncommand = \"delete\"\n";
        assert!(privileged_actions(safe).is_empty());
    }

    #[test]
    fn rejects_path_traversal() {
        let entry = HubEntry {
            id: "x".into(),
            name: "x".into(),
            description: String::new(),
            author: String::new(),
            version: String::new(),
            file: "../evil.toml".into(),
            sha256: String::new(),
        };
        assert!(install(&entry, "https://example.com/index.toml").is_err());
    }

    #[test]
    fn parses_index() {
        let text = "[[cleaner]]\nid = \"gaming\"\nname = \"Gaming\"\nfile = \"gaming.toml\"\nsha256 = \"abc\"\n";
        let entries = parse_index(text).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, "gaming");
    }
}
