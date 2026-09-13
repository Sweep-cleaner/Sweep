//! Akıllı artık tarama (Residue Intelligence): kaldırılmış uygulamaların,
//! ölü proje derleme çıktılarının, eski sürüm önbelleklerinin ve boş
//! dizinlerin bağlam-farkındalıklı tespiti.
//!
//! Her bulgu bir [`Residue`] taşır: tür, boyut, `confidence` (0.0–1.0) ve
//! `safe_to_delete` bayrağı. Silme her zaman [`crate::deep::guarded_delete`]
//! hattından geçer; güvenliği şüpheli öğeler `clean` akışında atlanır ve
//! raporda `needs-approval` olarak işaretlenir.
//!
//! Güvenlik çizgileri:
//! - Kullanıcı dizinleri (`Documents`, `Desktop`, `Pictures`, …) ASLA taranmaz.
//! - `.git` içeren/bulan hiçbir yol silme adayı olmaz.
//! - `confidence < 0.6` olanlar varsayılan eşikte silinmez.
//! - Windows Registry taraması salt-okunurdur (`reg query`).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::action::RunContext;
use crate::core::report::{Entry, EntryKind, Report};

pub const CLEANER_ID: &str = "residue";

/// Varsayılan silme eşiği: altındakiler kullanıcı onayı ister.
pub const DEFAULT_MIN_CONFIDENCE: f32 = 0.6;

/// Artık türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidueKind {
    /// Kaldırılmış uygulamanın config/cache/share kalıntısı.
    Uninstall,
    /// Ölü projenin derleme çıktısı (`target/`, `__pycache__`, …).
    Build,
    /// Eski sürüm / indirme önbelleği.
    Version,
    /// Boş ya da yalnızca artık dosya içeren dizin.
    Empty,
}

impl ResidueKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ResidueKind::Uninstall => "uninstall",
            ResidueKind::Build => "build",
            ResidueKind::Version => "version",
            ResidueKind::Empty => "empty",
        }
    }
}

/// Tek artık bulgusu.
#[derive(Debug, Clone)]
pub struct Residue {
    pub kind: ResidueKind,
    pub path: PathBuf,
    /// Bayt (dizinlerde sınırlı toplam; bkz. [`measured_size`]).
    pub bytes: u64,
    /// 0.0–1.0 güven skoru.
    pub confidence: f32,
    /// Kullanıcı belgesi/medya/save içermiyorsa true.
    pub safe_to_delete: bool,
    /// İnsan dilinde gerekçe (İngilizce; rapor anahtarı değil).
    pub reason: String,
    /// Dosya silme yerine özel komut gerektiriyorsa (örn. `tmutil`).
    pub command: Option<Vec<String>>,
}

/// Tarama seçenekleri (`sweep residue` bayrakları).
#[derive(Debug, Clone)]
pub struct ResidueOptions {
    /// Build/empty taraması kökleri (boşsa ev dizini).
    pub roots: Vec<PathBuf>,
    /// `clean` silme eşiği (varsayılan [`DEFAULT_MIN_CONFIDENCE`]).
    pub min_confidence: f32,
    /// Boş dizinleri `clean` dışında tut.
    pub keep_empty_dirs: bool,
    /// Build/empty yürüyüş derinliği.
    pub max_depth: usize,
}

impl Default for ResidueOptions {
    fn default() -> Self {
        Self {
            roots: Vec::new(),
            min_confidence: DEFAULT_MIN_CONFIDENCE,
            keep_empty_dirs: false,
            max_depth: 5,
        }
    }
}

// ---------------------------------------------------------------------------
// Ortak muhafızlar
// ---------------------------------------------------------------------------

/// ASLA taranmayacak kullanıcı dizin adları (ev dizini görece).
fn protected_home_leaves() -> &'static [&'static str] {
    &[
        "Documents",
        "Desktop",
        "Pictures",
        "Downloads",
        "Music",
        "Videos",
        "Public",
        "Templates",
        "Belgeler",
        "Masaüstü",
        "Resimler",
    ]
}

/// Yol korumalı bölgede mi? (ev kökü, kullanıcı yaprakları, `.git`.)
fn is_protected(path: &Path, home: Option<&Path>) -> bool {
    if path.components().any(|c| c.as_os_str() == ".git") {
        return true;
    }
    let Some(home) = home else {
        return false;
    };
    let Ok(rel) = path.strip_prefix(home) else {
        return false;
    };
    if rel.as_os_str().is_empty() {
        return true;
    }
    match rel.components().next() {
        Some(first) => protected_home_leaves()
            .iter()
            .any(|leaf| first.as_os_str() == *leaf),
        None => true,
    }
}

/// Kullanıcı belgesi/medya uzantıları (görülürse `safe_to_delete = false`).
fn user_data_extensions() -> &'static [&'static str] {
    &[
        "doc", "docx", "odt", "pdf", "xls", "xlsx", "ppt", "pptx", "txt", "md", "tex",
        "jpg", "jpeg", "png", "gif", "webp", "heic", "raw", "cr2", "psd", "xcf",
        "kra", "mp4", "mkv", "avi", "mov", "webm", "mp3", "flac", "ogg", "wav",
        "sav", "sl2", "ess", "skse",
    ]
}

/// Ağaçta (sınırlı) kullanıcı verisi var mı?
fn contains_user_data(dir: &Path) -> bool {
    use crate::fsutil::walk::{ScanOptions, scan_paths};
    let opts = ScanOptions::files().with_max_depth(3);
    scan_paths(dir, &opts)
        .into_iter()
        .take(400)
        .any(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| {
                    user_data_extensions()
                        .contains(&e.to_ascii_lowercase().as_str())
                })
                .unwrap_or(false)
        })
}

/// Dizin boyutu (çok büyük ağaçlarda sınırlı yürüyüş).
fn measured_size(dir: &Path) -> u64 {
    use crate::fsutil::walk::{ScanOptions, scan_paths};
    if dir.is_file() {
        return dir.metadata().map(|m| m.len()).unwrap_or(0);
    }
    scan_paths(dir, &ScanOptions::files().with_max_depth(6))
        .into_iter()
        .take(5000)
        .filter_map(|p| p.metadata().ok().map(|m| m.len()))
        .sum()
}

// ---------------------------------------------------------------------------
// 1.1 Kaldırılmış uygulama artıkları
// ---------------------------------------------------------------------------

/// Aday binary adları (`"Visual Studio Code"` → code, visual-studio-code, …).
fn candidate_bins(app_dir_name: &str) -> Vec<String> {
    let mut out = vec![app_dir_name.to_string(), app_dir_name.to_lowercase()];
    let squashed: String = app_dir_name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let flat: String = squashed.chars().filter(|c| *c != '-').collect();
    out.push(squashed);
    out.push(flat);
    out.sort();
    out.dedup();
    out
}

/// `.desktop` gövdelerinde aday binary adı geçiyor mu?
fn desktop_mentions(dir: &Path, bins: &[String]) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("desktop") {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_lowercase();
        if bins.iter().any(|b| b == &stem) {
            return true;
        }
        if let Ok(body) = std::fs::read_to_string(&path) {
            for line in body.lines() {
                let line = line.trim();
                if let Some(exec) = line.strip_prefix("Exec=") {
                    let first = exec
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .rsplit('/')
                        .next()
                        .unwrap_or("")
                        .to_lowercase();
                    if bins.iter().any(|b| b == &first) {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Linux/macOS ortak çekirdeği: verilen dizinlerdeki uygulama kalıntıları.
///
/// `is_installed` platforma göre sağlanır (binary PATH'te mi / .app var mı?).
/// `desktop_dirs` yalnızca Linux'ta `.desktop` eşleşmesi için kullanılır.
fn uninstall_candidates_generic(
    app_dirs: &[(PathBuf, f32)],
    is_installed: &dyn Fn(&str) -> bool,
) -> Vec<Residue> {
    let mut out = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for (dir, confidence) in app_dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if !path.is_dir() || !seen.insert(path.clone()) {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() || name.starts_with('.') {
                continue;
            }
            if is_installed(&name) {
                continue;
            }
            let safe = !contains_user_data(&path);
            out.push(Residue {
                kind: ResidueKind::Uninstall,
                bytes: if safe { measured_size(&path) } else { 0 },
                path,
                confidence: if safe { *confidence } else { confidence.min(0.5) },
                safe_to_delete: safe,
                reason: format!("'{name}' is not installed; leftover data dir"),
                command: None,
            });
        }
    }
    out
}

/// Linux: `~/.config`, `~/.local/share`, `~/.cache` (+ `.desktop` kanıtı).
pub fn uninstall_candidates_linux(home: &Path) -> Vec<Residue> {
    let config = home.join(".config");
    let share = home.join(".local").join("share");
    let cache = home.join(".cache");
    let user_desktops = share.join("applications");
    let sys_desktops = PathBuf::from("/usr/share/applications");
    let is_installed = |name: &str| {
        let bins = candidate_bins(name);
        bins.iter()
            .any(|b| crate::fsutil::path_exists_in_path(b))
            || desktop_mentions(&user_desktops, &bins)
            || desktop_mentions(&sys_desktops, &bins)
    };
    uninstall_candidates_generic(
        &[
            (config, 0.9),
            (share, 0.9),
            (cache, 0.7),
        ],
        &is_installed,
    )
}

/// macOS: `~/Library/{Application Support,Caches}` + Preferences plist'leri.
pub fn uninstall_candidates_macos(home: &Path) -> Vec<Residue> {
    let support = home.join("Library").join("Application Support");
    let caches = home.join("Library").join("Caches");
    let prefs = home.join("Library").join("Preferences");
    let apps = PathBuf::from("/Applications");
    let is_installed = |name: &str| {
        let bins = candidate_bins(name);
        if bins
            .iter()
            .any(|b| crate::fsutil::path_exists_in_path(b))
        {
            return true;
        }
        let Ok(rd) = std::fs::read_dir(&apps) else {
            return false;
        };
        rd.flatten().any(|e| {
            e.path()
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|stem| {
                    let lower = stem.to_lowercase();
                    bins.iter().any(|b| b == &lower)
                })
                .unwrap_or(false)
        })
    };
    let mut out = uninstall_candidates_generic(
        &[(support, 0.9), (caches, 0.7)],
        &is_installed,
    );
    // `~/Library/Preferences/<vendor>.<App>.plist` → son bileşen uygulama adı.
    if let Ok(rd) = std::fs::read_dir(&prefs) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("plist") {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("");
            let app = stem.rsplit('.').next().unwrap_or("").to_string();
            if app.len() < 3 || is_installed(&app) {
                continue;
            }
            out.push(Residue {
                kind: ResidueKind::Uninstall,
                bytes: path.metadata().map(|m| m.len()).unwrap_or(0),
                path,
                confidence: 0.85,
                safe_to_delete: true,
                reason: format!("'{app}' is not installed; orphan preference file"),
                command: None,
            });
        }
    }
    out
}

/// Windows: kaldırılmış uygulamaların AppData kalıntıları (salt-okunur tespit).
///
/// `reg_apps`: kaldırma kayıt defterindeki görünen adlar (`DisplayName`);
/// `program_dirs`: `Program Files*` altındaki dizin adları (küçük harf).
pub fn uninstall_candidates_windows(
    roaming: &Path,
    local: &Path,
    reg_apps: &[String],
    program_dirs: &[String],
) -> Vec<Residue> {
    let norm = |s: &str| s.to_lowercase();
    let installed_like = |name: &str| {
        let n = norm(name);
        program_dirs.iter().any(|d| d == &n || n.contains(d.as_str()) || d.contains(n.as_str()))
            || reg_apps.iter().any(|a| {
                let a = norm(a);
                a == n || a.contains(n.as_str()) || n.contains(a.as_str())
            })
            || crate::fsutil::path_exists_in_path(&n)
    };
    let mut out = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for base in [roaming, local] {
        let Ok(rd) = std::fs::read_dir(base) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if !path.is_dir() || !seen.insert(path.clone()) {
                continue;
            }
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() || installed_like(&name) {
                continue;
            }
            let safe = !contains_user_data(&path);
            out.push(Residue {
                kind: ResidueKind::Uninstall,
                bytes: if safe { measured_size(&path) } else { 0 },
                path,
                confidence: if safe { 0.85 } else { 0.5 },
                safe_to_delete: safe,
                reason: format!("'{name}' has no program dir or uninstall entry"),
                command: None,
            });
        }
    }
    out
}

/// `HKLM\...\Uninstall` altındaki `DisplayName` değerleri (salt-okunur).
#[cfg(windows)]
pub fn windows_uninstall_names() -> Vec<String> {
    match crate::fsutil::run_command(
        "reg",
        &[
            "query",
            "HKLM\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
            "/s",
            "/v",
            "DisplayName",
        ],
        true,
    ) {
        Ok((0, out, _)) => out
            .lines()
            .filter_map(|l| l.trim().strip_prefix("DisplayName"))
            .filter_map(|rest| {
                rest.split_whitespace().skip(1).collect::<Vec<_>>().join(" ").into()
            })
            .map(|s: String| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(not(windows))]
pub fn windows_uninstall_names() -> Vec<String> {
    Vec::new()
}

// ---------------------------------------------------------------------------
// 1.2 Geliştirici proje artıkları
// ---------------------------------------------------------------------------

/// Proje kök işareti var mı? (en fazla 2 üst dizine bakılır).
fn has_marker(dir: &Path, markers: &[&str]) -> bool {
    let mut cur = dir.to_path_buf();
    for _ in 0..3 {
        for m in markers {
            if cur.join(m).exists() {
                return true;
            }
        }
        if !cur.pop() {
            break;
        }
    }
    false
}

/// Dosya/dizin mtime'ı günden eski mi? (okunamazsa "eski" sayılmaz.)
fn older_than_days(path: &Path, days: u64) -> bool {
    let Ok(md) = std::fs::symlink_metadata(path) else {
        return false;
    };
    let Ok(mtime) = md.modified() else {
        return false;
    };
    SystemTime::now()
        .duration_since(mtime)
        .map(|d| d.as_secs() > days * 86400)
        .unwrap_or(false)
}

/// `(yapı dizini adı, gerekli proje işareti, güven)`.
/// İşaret YOKSA ölü projedir; staleness ayrıca değerlendirilir.
fn build_rules() -> &'static [(&'static str, &'static [&'static str], f32)] {
    &[
        ("target", &["Cargo.toml"], 0.8),
        ("__pycache__", &["requirements.txt", "pyproject.toml", "setup.py", "setup.cfg"], 0.75),
        (".venv", &["requirements.txt", "pyproject.toml", "setup.py", "setup.cfg"], 0.75),
        ("venv", &["requirements.txt", "pyproject.toml", "setup.py", "setup.cfg"], 0.75),
        (".next", &["package.json", "next.config.js", "next.config.mjs", "next.config.ts"], 0.8),
        ("dist", &["package.json", "pyproject.toml", "setup.py", "Cargo.toml"], 0.7),
        ("build", &["package.json", "build.gradle", "pom.xml", "setup.py"], 0.7),
        ("vendor", &["go.mod"], 0.8),
    ]
}

/// Kökler altında ölü proje derleme çıktıları.
pub fn build_candidates(roots: &[PathBuf], max_depth: usize) -> Vec<Residue> {
    use crate::fsutil::walk::{ScanOptions, scan_paths};
    let rules = build_rules();
    let mut out = Vec::new();
    for root in roots {
        if !root.is_dir() {
            continue;
        }
        for path in scan_paths(
            root,
            &ScanOptions::all()
                .with_max_depth(max_depth)
                .with_same_filesystem(true),
        ) {
            if !path.is_dir() {
                continue;
            }
            if path.components().any(|c| c.as_os_str() == ".git") {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            let Some((_, markers, confidence)) =
                rules.iter().find(|(n, _, _)| *n == name)
            else {
                continue;
            };
            if has_marker(&path, markers) {
                // İşaret var: aktif proje — Node tarafı staleness'e bakar.
                if name == ".next" || name == "dist" || name == "build" {
                    if !older_than_days(&path, 30) {
                        continue;
                    }
                } else {
                    continue;
                }
            }
            out.push(Residue {
                kind: ResidueKind::Build,
                bytes: measured_size(&path),
                path: path.clone(),
                confidence: *confidence,
                safe_to_delete: true,
                reason: format!("stale '{name}' without project markers nearby"),
                command: None,
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// 1.3 Sürüm / cache artıkları (sabit bilinen yollar)
// ---------------------------------------------------------------------------

/// Bilinen sürüm önbelleği dizinleri (varsa raporlanır).
pub fn version_candidates() -> Vec<Residue> {
    let mut dirs: Vec<(PathBuf, f32, &str)> = Vec::new();
    if let Some(home) = crate::platform::home_dir() {
        dirs.push((
            home.join(".cache").join("pip").join("http-v2"),
            0.75,
            "pip http cache (re-downloaded on demand)",
        ));
        dirs.push((
            home.join(".cargo").join("registry").join("cache"),
            0.75,
            "cargo registry archives (re-fetched on next build)",
        ));
        if cfg!(target_os = "macos") {
            dirs.push((
                home.join("Library")
                    .join("Caches")
                    .join("com.apple.FontRegistry"),
                0.8,
                "font registry cache (rebuilt by the system)",
            ));
            dirs.push((
                PathBuf::from("/Library/Caches/com.apple.FontRegistry"),
                0.8,
                "system font registry cache (rebuilt by the system)",
            ));
        }
        if cfg!(windows) {
            if let Ok(windir) = std::env::var("windir") {
                dirs.push((
                    PathBuf::from(&windir)
                        .join("SoftwareDistribution")
                        .join("Download"),
                    0.85,
                    "delivered update packages (Windows Update re-fetches)",
                ));
            }
        }
    }
    dirs.into_iter()
        .filter(|(p, _, _)| p.is_dir())
        .map(|(path, confidence, why)| Residue {
            kind: ResidueKind::Version,
            bytes: measured_size(&path),
            path,
            confidence,
            safe_to_delete: true,
            reason: why.to_string(),
            command: None,
        })
        .collect()
}

/// macOS Time Machine yerel anlık görüntüleri (salt-okunur liste).
#[cfg(target_os = "macos")]
pub fn timemachine_snapshots() -> Vec<String> {
    if !crate::fsutil::path_exists_in_path("tmutil") {
        return Vec::new();
    }
    match crate::fsutil::run_command("tmutil", &["listlocalsnapshots", "/"], true) {
        Ok((0, out, _)) => out
            .lines()
            .filter_map(|l| l.split('.').nth_back(1).map(|s| s.trim().to_string()))
            .filter(|s| !s.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(not(target_os = "macos"))]
pub fn timemachine_snapshots() -> Vec<String> {
    Vec::new()
}

// ---------------------------------------------------------------------------
// 1.4 Boş / yarı boş dizinler
// ---------------------------------------------------------------------------

/// Yoksayılabilir artık dosya adları (bırakılırsa dizin "boş" sayılır).
fn junk_filenames() -> &'static [&'static str] {
    &[".DS_Store", "Thumbs.db", "desktop.ini", ".localized"]
}

/// Dizin boş ya da yalnızca junk içeriyor mu?
fn is_effectively_empty(dir: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        // Sembolik bağ izlenmez: hem döngüleri keser hem hedefi korur.
        if std::fs::symlink_metadata(&path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(true)
        {
            return false;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if path.is_dir() {
            if !is_effectively_empty(&path) {
                return false;
            }
        } else if !junk_filenames().contains(&name) {
            return false;
        }
    }
    true
}

/// Kökler altında etkin-boş dizinler (confidence 1.0).
pub fn empty_candidates(roots: &[PathBuf], max_depth: usize) -> Vec<Residue> {
    use crate::fsutil::walk::{ScanOptions, scan_paths};
    let home = crate::platform::home_dir();
    let mut out = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for root in roots {
        if !root.is_dir() || is_protected(root, home.as_deref()) {
            continue;
        }
        for path in scan_paths(
            root,
            &ScanOptions::all()
                .with_max_depth(max_depth)
                .with_same_filesystem(true),
        ) {
            if !path.is_dir() || !seen.insert(path.clone()) {
                continue;
            }
            if path == *root || is_protected(&path, home.as_deref()) {
                continue;
            }
            if is_effectively_empty(&path) {
                out.push(Residue {
                    kind: ResidueKind::Empty,
                    bytes: 0,
                    path: path.clone(),
                    confidence: 1.0,
                    safe_to_delete: true,
                    reason: "empty or junk-only directory".to_string(),
                    command: None,
                });
            }
        }
    }
    // İç içe boş dizinler: en üsttekini tut (altları onunla gider).
    out.sort_by(|a, b| a.path.cmp(&b.path));
    let mut keep: Vec<Residue> = Vec::new();
    for r in out {
        if keep.iter().any(|k: &Residue| r.path.starts_with(&k.path)) {
            continue;
        }
        keep.push(r);
    }
    keep
}

// ---------------------------------------------------------------------------
// Rapor + silme akışı
// ---------------------------------------------------------------------------

fn describe(r: &Residue) -> String {
    format!(
        "[{}] conf {:.2} {} ({}) — {}",
        r.kind.as_str(),
        r.confidence,
        r.path.display(),
        crate::deep::safety::human(r.bytes),
        r.reason
    )
}

/// Tüm toplayıcılar (sıralı: bayt büyükten küçüğe).
pub fn collect(opts: &ResidueOptions) -> Vec<Residue> {
    let mut all = Vec::new();
    if let Some(home) = crate::platform::home_dir() {
        if cfg!(target_os = "macos") {
            all.extend(uninstall_candidates_macos(&home));
        } else if cfg!(windows) {
            let roaming = std::env::var("APPDATA").map(PathBuf::from);
            let local = std::env::var("LOCALAPPDATA").map(PathBuf::from);
            let reg = windows_uninstall_names();
            let program_dirs: Vec<String> = ["ProgramFiles", "ProgramFiles(x86)"]
                .iter()
                .filter_map(|v| std::env::var(v).ok())
                .flat_map(|base| {
                    std::fs::read_dir(PathBuf::from(base))
                        .map(|rd| {
                            rd.flatten()
                                .filter_map(|e| {
                                    e.file_name().to_str().map(|s| s.to_lowercase())
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                })
                .collect();
            if let (Ok(ro), Ok(lo)) = (roaming, local) {
                all.extend(uninstall_candidates_windows(&ro, &lo, &reg, &program_dirs));
            }
        } else {
            all.extend(uninstall_candidates_linux(&home));
        }
    }
    let roots: Vec<PathBuf> = if opts.roots.is_empty() {
        crate::platform::home_dir().into_iter().collect()
    } else {
        opts.roots.clone()
    };
    let roots: Vec<PathBuf> = roots.into_iter().filter(|r| r.is_dir()).collect();
    all.extend(build_candidates(&roots, opts.max_depth));
    all.extend(empty_candidates(&roots, opts.max_depth));
    all.extend(version_candidates());
    if cfg!(target_os = "macos") {
        for snap in timemachine_snapshots() {
            all.push(Residue {
                kind: ResidueKind::Version,
                bytes: 0,
                path: PathBuf::from("/"),
                confidence: 0.7,
                safe_to_delete: true,
                reason: format!("Time Machine local snapshot {snap}"),
                command: Some(vec![
                    "tmutil".to_string(),
                    "thinlocalsnapshots".to_string(),
                    "/".to_string(),
                    "2g".to_string(),
                ]),
            });
        }
    }
    all.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    all
}

/// Bulgu listesinden bilgi raporu (`preview` akışı da kullanır).
pub fn report_of(items: &[Residue]) -> Report {
    let mut report = Report::new();
    for r in items {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            r.kind.as_str(),
            describe(r),
            Some(&r.path),
            r.bytes,
        ));
    }
    report
}

/// Liste modu: bütün bulgular bilgi girdisi olur.
pub fn scan(opts: &ResidueOptions, ctx: &RunContext) -> Report {
    let all = collect(opts);
    if all.is_empty() {
        let mut report = Report::new();
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "scan",
            crate::i18n::t(&ctx.lang, "no residue found"),
            None,
            0,
        ));
        return report;
    }
    report_of(&all)
}

/// Özel komutla silinen bulgu (örn. Time Machine snapshot inceltme).
fn exec_command(r: &Residue, option: &str, report: &mut Report) {
    let Some(cmd) = &r.command else {
        return;
    };
    let args: Vec<&str> = cmd[1..].iter().map(|s| s.as_str()).collect();
    match crate::fsutil::run_command(&cmd[0], &args, true) {
        Ok((0, _, _)) => report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            option,
            describe(r),
            Some(&r.path),
            r.bytes,
        )),
        Ok((code, _, err)) => report.fail(
            CLEANER_ID,
            option,
            format!("{} exit {code}: {}", cmd.join(" "), err.trim()),
        ),
        Err(e) => report.fail(CLEANER_ID, option, e.to_string()),
    }
}

/// `clean`: eşik + güvenlik filtresi, sonra korumalı silme / özel komut.
pub fn clean(opts: &ResidueOptions, ctx: &RunContext) -> Report {
    let mut report = Report::new();
    if !ctx.dry_run {
        crate::deep::safety::log_run_start(ctx, "residue.clean");
    }
    let threshold = opts.min_confidence.max(DEFAULT_MIN_CONFIDENCE).max(0.0).min(1.0);
    for r in collect(opts).iter().filter(|r| {
        !(r.kind == ResidueKind::Empty && opts.keep_empty_dirs)
    }) {
        if ctx.cancelled() {
            report.aborted = true;
            break;
        }
        if r.confidence < threshold {
            report.push(Entry::skipped(CLEANER_ID, "low-confidence", &r.path));
            continue;
        }
        if !r.safe_to_delete {
            report.push(Entry::skipped(CLEANER_ID, "needs-approval", &r.path));
            report.fail(
                CLEANER_ID,
                "needs-approval",
                format!("{} may hold user data; review first", r.path.display()),
            );
            continue;
        }
        if ctx.dry_run {
            report.push(Entry::new(
                EntryKind::Delete,
                CLEANER_ID,
                r.kind.as_str(),
                describe(r),
                Some(&r.path),
                r.bytes,
            ));
            continue;
        }
        if r.command.is_some() {
            exec_command(r, r.kind.as_str(), &mut report);
            continue;
        }
        crate::deep::guarded_delete(&r.path, ctx, CLEANER_ID, r.kind.as_str(), &mut report);
    }
    report
}

/// Seçili bulguları sil (`preview` akışı): güvenli olmayanlar yine de
/// korunur ve `needs-approval` olarak raporlanır.
pub fn clean_selected(items: &[Residue], ctx: &RunContext) -> Report {
    let mut report = Report::new();
    if !ctx.dry_run {
        crate::deep::safety::log_run_start(ctx, "residue.selected");
    }
    for r in items {
        if ctx.cancelled() {
            report.aborted = true;
            break;
        }
        if !r.safe_to_delete {
            report.push(Entry::skipped(CLEANER_ID, "needs-approval", &r.path));
            report.fail(
                CLEANER_ID,
                "needs-approval",
                format!("{} may hold user data; review first", r.path.display()),
            );
            continue;
        }
        if ctx.dry_run {
            report.push(Entry::new(
                EntryKind::Delete,
                CLEANER_ID,
                "selected",
                describe(r),
                Some(&r.path),
                r.bytes,
            ));
            continue;
        }
        if r.command.is_some() {
            exec_command(r, "selected", &mut report);
            continue;
        }
        crate::deep::guarded_delete(&r.path, ctx, CLEANER_ID, "selected", &mut report);
    }
    report
}

/// `preview` seçim ayrıştırıcı: "1,3-5" / "all" / "none" → indeksler (0-bazlı).
pub fn parse_selection(input: &str, total: usize) -> Vec<usize> {
    let input = input.trim().to_lowercase();
    if input == "all" {
        return (0..total).collect();
    }
    if input.is_empty() || input == "none" {
        return Vec::new();
    }
    let mut out: Vec<usize> = Vec::new();
    for part in input.split([',', ' ']) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            if let (Ok(mut lo), Ok(mut hi)) =
                (a.trim().parse::<usize>(), b.trim().parse::<usize>())
            {
                if lo > hi {
                    std::mem::swap(&mut lo, &mut hi);
                }
                for n in lo.saturating_sub(1)..hi.min(total) {
                    if !out.contains(&n) {
                        out.push(n);
                    }
                }
            }
            continue;
        }
        if let Ok(n) = part.parse::<usize>() {
            if n >= 1 && n <= total && !out.contains(&(n - 1)) {
                out.push(n - 1);
            }
        }
    }
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(root: &Path, files: &[&str]) {
        for f in files {
            // Sondaki `/` "bu dizini oluştur" demektir. `Path::parent()`
            // `<root>/empty/` için `<root>` döndüğü için dizinin kendisi
            // açıkça oluşturulmalı — aksi halde hiç var olmaz.
            if let Some(dir) = f.strip_suffix('/') {
                std::fs::create_dir_all(root.join(dir)).unwrap();
                continue;
            }
            let p = root.join(f);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&p, b"x").unwrap();
        }
    }

    #[test]
    fn residue_empty_dir_scores_full() {
        let dir = std::env::temp_dir().join("sweep_residue_empty");
        let _ = std::fs::remove_dir_all(&dir);
        tree(
            &dir,
            &[
                "empty/",
                "junkonly/.DS_Store",
                "junkonly/Thumbs.db",
                "livedoc/note.txt",
                ".git/objects/x",
            ],
        );
        let found = empty_candidates(&[dir.clone()], 4);
        let names: Vec<String> = found
            .iter()
            .map(|r| {
                r.path
                    .strip_prefix(&dir)
                    .unwrap()
                    .display()
                    .to_string()
            })
            .collect();
        assert!(names.contains(&"empty".to_string()), "{names:?}");
        assert!(names.contains(&"junkonly".to_string()), "{names:?}");
        assert!(!names.iter().any(|n| n.starts_with("livedoc")), "{names:?}");
        assert!(!names.iter().any(|n| n.contains(".git")), "{names:?}");
        assert!(found.iter().all(|r| r.confidence == 1.0 && r.safe_to_delete));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn residue_build_markers() {
        let dir = std::env::temp_dir().join("sweep_residue_build");
        let _ = std::fs::remove_dir_all(&dir);
        tree(
            &dir,
            &[
                "deadproj/target/debug/a",
                "liveproj/Cargo.toml",
                "liveproj/target/debug/a",
                "pyold/__pycache__/m.pyc",
                "pynew/pyproject.toml",
                "pynew/__pycache__/m.pyc",
                "nodedead/.next/cache/x",
                "node/.git/HEAD",
                "node/node_modules/x",
            ],
        );
        // `.next` tazelik kuralına takılmasın diye eskitle.
        filetime_old(&dir.join("nodedead/.next"));
        let found = build_candidates(&[dir.clone()], 5);
        // Karşılaştırma bileşen bazında: `Path::display()` Windows'ta `\`
        // üretir, bu yüzden `/` ayraçlı bir dizeyle karşılaştırmak hiç eşleşmez.
        let has = |suffix: &str| {
            let want: Vec<&str> = suffix.split('/').collect();
            found.iter().any(|r| {
                let rel: Vec<String> = r
                    .path
                    .strip_prefix(&dir)
                    .map(|p| {
                        p.components()
                            .map(|c| c.as_os_str().to_string_lossy().into_owned())
                            .collect()
                    })
                    .unwrap_or_default();
                rel.len() >= want.len()
                    && want.iter().zip(rel.iter()).all(|(w, got)| *w == got.as_str())
            })
        };
        assert!(has("deadproj/target"), "{found:?}");
        assert!(!has("liveproj/target"), "{found:?}");
        assert!(has("pyold/__pycache__"), "{found:?}");
        assert!(!has("pynew/__pycache__"), "{found:?}");
        assert!(has("nodedead/.next"), "{found:?}");
        assert!(!found.iter().any(|r| {
            r.path.components().any(|c| c.as_os_str() == ".git")
        }));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    fn filetime_old(path: &Path) {
        // `File::set_modified` ile mtime eskitleme (yalnızca test).
        if let Ok(f) = std::fs::File::options().write(true).open(path) {
            let t = SystemTime::now() - std::time::Duration::from_secs(40 * 86400);
            let _ = f.set_modified(t);
        }
    }

    #[cfg(not(unix))]
    fn filetime_old(_path: &Path) {}

    #[test]
    fn residue_confidence_thresholds() {
        assert!((DEFAULT_MIN_CONFIDENCE - 0.6).abs() < f32::EPSILON);
        let gate = |conf: f32, safe: bool, threshold: f32| {
            conf >= threshold.max(DEFAULT_MIN_CONFIDENCE) && safe
        };
        assert!(gate(0.9, true, 0.6));
        assert!(!gate(0.59, true, 0.0));
        assert!(!gate(0.9, false, 0.6));
        assert!(gate(1.0, true, 0.9));
        assert!(!gate(0.85, true, 0.9));
    }

    #[test]
    fn residue_user_data() {
        let dir = std::env::temp_dir().join("sweep_residue_userdata");
        let _ = std::fs::remove_dir_all(&dir);
        tree(&dir, &["docs/report.pdf", "bin/tool"]);
        assert!(contains_user_data(&dir.join("docs")));
        assert!(!contains_user_data(&dir.join("bin")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn residue_selection() {
        assert_eq!(parse_selection("all", 3), vec![0, 1, 2]);
        assert!(parse_selection("none", 3).is_empty());
        assert!(parse_selection("", 3).is_empty());
        assert_eq!(parse_selection("1,3", 3), vec![0, 2]);
        assert_eq!(parse_selection("2-3", 3), vec![1, 2]);
        assert_eq!(parse_selection("3-2", 3), vec![1, 2]);
        assert_eq!(parse_selection("0,9,2", 3), vec![1]);
        assert_eq!(parse_selection(" 1 , 2-2 ", 3), vec![0, 1]);
    }

    #[test]
    fn residue_uninstall_heuristic() {
        let dir = std::env::temp_dir().join("sweep_residue_uninst");
        let _ = std::fs::remove_dir_all(&dir);
        let config = dir.join("config");
        let share = dir.join("share");
        tree(&dir, &["config/goneapp/settings.ini", "config/hereapp/x"]);
        let found = uninstall_candidates_generic(
            &[(config, 0.9), (share, 0.9)],
            &|name| name == "hereapp",
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].confidence, 0.9);
        assert!(found[0].safe_to_delete);
        assert_eq!(found[0].kind, ResidueKind::Uninstall);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn residue_protected_leaves() {
        let home = PathBuf::from("/home/u");
        assert!(is_protected(&home.join("Documents/x"), Some(&home)));
        assert!(is_protected(&home.join(".git/x"), Some(&home)));
        assert!(is_protected(&home, Some(&home)));
        assert!(!is_protected(&home.join(".cache/app"), Some(&home)));
        assert!(!is_protected(&PathBuf::from("/tmp/x"), Some(&home)));
    }
}
