//! Artık-avcısı: kaldırılmış bir uygulamanın geride bıraktığı dosyaları bulur.
//!
//! Kullanıcı bir isim yazar (`sweep leftovers discord`); motor, o isme ait
//! yapılandırma/önbellek/veri kalıntılarını işletim sistemine özgü bilinen
//! köklerde arar, paket yöneticisine hâlâ kayıtlı dosyaları eleyip raporlar.
//! Silme her zamanki güvenlik hattından geçer (`Guard`, kritik dizinler,
//! symlink ve yetki kontrolleri, yedek, işlem logu).

use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::action::context::RunContext;
use crate::core::error::{Error, Result};
use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "leftovers";

/// En az isim uzunluğu (tek harfli tarama tüm diski eşleştirirdi).
pub const MIN_NAME_LEN: usize = 2;

/// Eşleşme gücü: exact > prefix > substring.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Score {
    Substring = 1,
    Prefix = 2,
    Exact = 3,
}

/// Tek bulgu.
#[derive(Debug, Clone)]
pub struct Hit {
    pub path: PathBuf,
    pub is_dir: bool,
    pub bytes: u64,
    pub score: Score,
    /// Hâlâ kurulu bir pakete aitse paket adı (silinmez).
    pub owned_by: Option<String>,
}

/// Karşılaştırma için ismi sadeleştir: küçük harf, alfanümerik dışı atılır.
fn squash(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Dosya/dizin adını ineğe göre puanla.
fn score_name(file_name: &str, needle: &str) -> Option<Score> {
    let name = squash(file_name);
    if name.is_empty() || needle.is_empty() || !name.contains(needle) {
        return None;
    }
    // Uzantısız kök tam eşleşiyorsa exact ("Discord.exe" -> "discord").
    let stem = file_name
        .rsplit_once('.')
        .map(|(s, _)| s)
        .unwrap_or(file_name);
    if squash(stem) == needle {
        return Some(Score::Exact);
    }
    if name.starts_with(needle) {
        return Some(Score::Prefix);
    }
    Some(Score::Substring)
}

/// (kök, derinlik): kullanıcı kökleri derin, sistem kökleri sığ taranır.
pub fn search_roots() -> Vec<(PathBuf, usize)> {
    let mut roots: Vec<(PathBuf, usize)> = Vec::new();
    let mut push = |p: PathBuf, depth: usize| {
        if p.is_dir() {
            roots.push((p, depth));
        }
    };

    #[cfg(target_os = "windows")]
    {
        for var in ["APPDATA", "LOCALAPPDATA", "PROGRAMDATA"] {
            if let Ok(dir) = std::env::var(var) {
                push(PathBuf::from(dir), 10);
            }
        }
        for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
            if let Ok(dir) = std::env::var(var) {
                push(PathBuf::from(dir), 3);
            }
        }
        if let Some(home) = crate::platform::home_dir() {
            push(home.join("AppData").join("LocalLow"), 8);
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Some(home) = crate::platform::home_dir() {
            for leaf in [
                "Library/Application Support",
                "Library/Caches",
                "Library/Preferences",
                "Library/Logs",
                "Library/Saved Application State",
                "Library/Containers",
                "Library/Group Containers",
            ] {
                push(home.join(leaf), 8);
            }
        }
        for sys in [
            "/Library/Application Support",
            "/Library/Caches",
            "/Library/Preferences",
            "/Library/Logs",
        ] {
            push(PathBuf::from(sys), 4);
        }
        push(PathBuf::from("/Applications"), 2);
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(home) = crate::platform::home_dir() {
            for leaf in [
                ".config",
                ".cache",
                ".local/share",
                ".local/state",
                ".mozilla",
                ".var/app",
                "snap",
            ] {
                push(home.join(leaf), 10);
            }
            // Flatpak kullanıcı dizinleri: yalnızca ilgili uygulama dalı
            // isim eşleşmesiyle bulunur, derinlik sınırlıdır.
            push(home.join(".local/share/flatpak"), 6);
        }
        // Sistem kalıntıları: sığ ve yavaş (paket sahiplik kontrolüyle).
        for sys in [
            "/etc",
            "/opt",
            "/usr/share",
            "/usr/local/share",
            "/var/lib",
            "/var/cache",
        ] {
            push(PathBuf::from(sys), 4);
        }
    }

    roots
}

/// Linux'ta dosyanın hâlâ kurulu bir pakete ait olup olmadığını sor.
#[cfg(target_os = "linux")]
fn package_owner(path: &Path) -> Option<String> {
    let text = path.to_string_lossy();
    let probes: &[(&str, &[&str])] = &[("dpkg", &["-S"]), ("rpm", &["-qf"]), ("pacman", &["-Qo"])];
    for (tool, flag) in probes {
        if !crate::fsutil::path_exists_in_path(tool) {
            continue;
        }
        let args: Vec<&str> = flag
            .iter()
            .copied()
            .chain(std::iter::once(text.as_ref()))
            .collect();
        if let Ok((0, stdout, _)) = crate::fsutil::run_command(tool, &args, true) {
            let line = stdout.lines().next().unwrap_or("").trim();
            // "no path found" benzeri çıktıları ele.
            if !line.is_empty()
                && !line.contains("no path found")
                && !line.contains("is not installed")
                && !line.contains("No package owns")
            {
                return Some(format!("{tool}: {line}"));
            }
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn package_owner(_path: &Path) -> Option<String> {
    None
}

/// Ada göre dosya kalıntılarını tara (kayıt defteri hariç).
pub fn find_hits(needle_raw: &str) -> Result<Vec<Hit>> {
    let needle = squash(needle_raw.trim());
    if needle.len() < MIN_NAME_LEN {
        return Err(Error::msg(format!(
            "name must be at least {MIN_NAME_LEN} chars"
        )));
    }

    let roots = search_roots();
    let mut hits: Vec<Hit> = roots
        .par_iter()
        .flat_map(|(root, depth)| {
            let options = crate::fsutil::walk::ScanOptions::all().with_max_depth(*depth);
            crate::fsutil::walk::scan_paths(root, &options)
                .into_iter()
                .filter_map(|path| {
                    let name = path.file_name()?.to_string_lossy().into_owned();
                    let score = score_name(&name, &needle)?;
                    let is_dir = path.is_dir();
                    let bytes = if is_dir {
                        crate::fsutil::size::dir_size(&path)
                    } else {
                        crate::fsutil::size::size_of_or_zero(&path)
                    };
                    Some(Hit {
                        path,
                        is_dir,
                        bytes,
                        score,
                        owned_by: None,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect();

    // İç içe isabetleri birleştir: üst dizin zaten listede ise altını at.
    hits.sort_by(|a, b| a.path.cmp(&b.path));
    let mut merged: Vec<Hit> = Vec::with_capacity(hits.len());
    for hit in hits {
        let nested = merged
            .iter()
            .any(|top: &Hit| top.is_dir && hit.path != top.path && hit.path.starts_with(&top.path));
        if !nested {
            merged.push(hit);
        }
    }

    // Sonuç patlamasını sınırla: en fazla 5000 isabet tutulur.
    merged.truncate(5000);
    // Paket sahipliği: yalnızca ilk 300 isabette (hız sınırı).
    // Ev dizini altındaki dosyalar hiçbir pakete ait olamaz (dpkg/rpm/pacman
    // yalnızca sistem yollarını bilir), o yüzden sorgulanmaz.
    let home = crate::platform::home_dir();
    for hit in merged.iter_mut().take(300) {
        let under_home = home
            .as_ref()
            .map(|h| hit.path.starts_with(h))
            .unwrap_or(false);
        if !under_home {
            hit.owned_by = package_owner(&hit.path);
        }
    }
    // Skora göre (yüksek önce), sonra örüntüye göre sırala.
    merged.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.path.cmp(&b.path)));
    Ok(merged)
}

/// Windows kayıt defteri kalıntıları (`HKCU/HKLM\Software` altında isim eşleşmesi).
#[cfg(target_os = "windows")]
pub fn registry_hits(needle_raw: &str) -> Vec<String> {
    use winreg::enums::*;
    use winreg::RegKey;

    let needle = needle_raw.trim().to_lowercase();
    let mut out = Vec::new();
    for (hive, prefix) in [(HKEY_CURRENT_USER, "HKCU"), (HKEY_LOCAL_MACHINE, "HKLM")] {
        let root = RegKey::predef(hive);
        for base in [
            "Software",
            "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
        ] {
            let Ok(key) = root.open_subkey(base) else {
                continue;
            };
            for name in key.enum_keys().flatten() {
                if name.to_lowercase().contains(&needle) {
                    out.push(format!("{prefix}\\{base}\\{name}"));
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[cfg(not(target_os = "windows"))]
pub fn registry_hits(_needle_raw: &str) -> Vec<String> {
    Vec::new()
}

/// Tam tarama: dosya isabetleri + (Windows'ta) kayıt defteri + süreç uyarısı.
/// `min_size` altındaki küçük isabetler rapora girmez.
pub fn scan(name: &str, min_size: u64, ctx: &RunContext) -> Report {
    let mut report = Report::new();

    if name.trim().len() < MIN_NAME_LEN {
        report.fail(
            CLEANER_ID,
            "search",
            crate::i18n::et(
                &ctx.lang,
                "name must be at least {} chars",
                &[&MIN_NAME_LEN.to_string()],
            ),
        );
        return report;
    }

    if crate::platform::is_process_running(name) {
        report.fail(
            CLEANER_ID,
            "processes",
            crate::i18n::et(
                &ctx.lang,
                "'{}' is running; close the app first",
                &[&name.to_string()],
            ),
        );
    }

    let hits = match find_hits(name) {
        Ok(h) => h,
        Err(err) => {
            report.fail(CLEANER_ID, "search", err.to_string());
            return report;
        }
    };

    if hits.is_empty() {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "search",
            crate::i18n::et(
                &ctx.lang,
                "no leftovers related to '{}'",
                &[&name.to_string()],
            ),
            None,
            0,
        ));
    }

    for hit in hits.iter().filter(|h| h.bytes >= min_size) {
        if ctx.cancelled() {
            report.aborted = true;
            break;
        }
        if let Some(owner) = &hit.owned_by {
            // Kurulu pakete ait: dokunma, bilgi olarak raporla.
            report.push(Entry::skipped(CLEANER_ID, "owned", &hit.path));
            report.fail(
                CLEANER_ID,
                "owned",
                crate::i18n::et(
                    &ctx.lang,
                    "{} belongs to installed package ({}), skipped",
                    &[hit.path.display().to_string(), owner.clone()],
                ),
            );
            continue;
        }
        let kind = match hit.score {
            Score::Exact => "exact match",
            Score::Prefix => "prefix",
            Score::Substring => "contains",
        };
        let label = format!("{kind} {}", hit.path.display());
        if ctx.dry_run {
            report.push(Entry::new(
                EntryKind::Delete,
                CLEANER_ID,
                "files",
                label,
                Some(&hit.path),
                hit.bytes,
            ));
        } else {
            crate::deep::guarded_delete(&hit.path, ctx, CLEANER_ID, "files", &mut report);
        }
    }

    // Windows kayıt defteri: önizleme her platformda derlenir, silme
    // yalnızca Windows'ta mevcuttur.
    if ctx.dry_run {
        for key in registry_hits(name) {
            report.push(Entry::new(
                EntryKind::Registry,
                CLEANER_ID,
                "registry",
                crate::i18n::et(&ctx.lang, "registry {}", &[&key]),
                None,
                0,
            ));
        }
        return report;
    }
    #[cfg(target_os = "windows")]
    for key in registry_hits(name) {
        match crate::platform::windows::reg_delete_key(&key, &[], true) {
            Ok(true) => report.push(Entry::new(
                EntryKind::Registry,
                CLEANER_ID,
                "registry",
                crate::i18n::et(&ctx.lang, "deleted {}", &[&key]),
                None,
                0,
            )),
            Ok(false) => {}
            Err(err) => report.fail(
                CLEANER_ID,
                "registry",
                crate::i18n::et(&ctx.lang, "{}: {}", &[key.clone(), err.to_string()]),
            ),
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn squash_keeps_only_alphanumerics() {
        assert_eq!(squash("Water-Fox_2"), "waterfox2");
        assert_eq!(squash("Discord PTB"), "discordptb");
        assert_eq!(squash("a.b"), "ab");
        assert_eq!(squash(""), "");
    }

    #[test]
    fn scoring_ranks_exact_above_prefix_above_substring() {
        // Exact: the stem (extension removed) squashes to the needle.
        assert_eq!(score_name("discord", "discord"), Some(Score::Exact));
        assert_eq!(score_name("Discord.exe", "discord"), Some(Score::Exact));
        // Prefix: the squashed name starts with the needle.
        assert_eq!(score_name("discord-stable", "discord"), Some(Score::Prefix));
        // Substring: the needle merely appears somewhere.
        assert_eq!(score_name("my-discord", "discord"), Some(Score::Substring));
        // No hit at all, and degenerate inputs.
        assert_eq!(score_name("firefox", "discord"), None);
        assert_eq!(score_name("", "discord"), None);
        assert_eq!(score_name("anything", ""), None);
        assert!(Score::Exact > Score::Prefix);
        assert!(Score::Prefix > Score::Substring);
    }

    #[test]
    fn too_short_names_are_refused() {
        // A one-letter search would match half the disk.
        assert!(find_hits("a").is_err());
        assert!(find_hits("").is_err());
    }
}
