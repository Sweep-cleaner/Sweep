//! Güvenlik katmanı: kritik dizin whitelist'i, symlink/yetki kontrolleri,
//! işlem logu, yedek alma ve boyut raporu yardımcıları.
//!
//! `Guard` (keep-list + protected roots) zaten her silmede devrededir; bu
//! modül Linux derin temizliğine özgü ikinci savunma hattıdır.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use crate::action::RunContext;

/// Asla dokunulmayacak kritik sistem kökleri.
///
/// Kullanıcının istediği `@command:proc, sys, dev, boot, etc, usr` kapsamı
/// burada sabitlenir; `--force` bile bu listeyi gevşetmez (yalnızca `Guard`
/// korumalı köklerini etkiler).
pub const CRITICAL_DIRS: &[&str] = &[
    "/",
    "/bin",
    "/boot",
    "/dev",
    "/etc",
    "/lib",
    "/lib32",
    "/lib64",
    "/libx32",
    "/proc",
    "/root",
    "/run",
    "/sbin",
    "/srv",
    "/sys",
    "/usr",
    "/tmp",
    "/var",
    "/var/lib",
    "/var/db",
    "/opt",
    "/System",
    "/Library",
    "/Applications",
    "/Network",
    "/private",
];

/// Kullanıcı verisi kökleri: ev dizininin kendisi ve kişisel klasörler.
fn user_data_roots() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = crate::platform::home_dir() {
        out.push(home.clone());
        for leaf in [
            "Desktop",
            "Documents",
            "Downloads",
            "Pictures",
            "Music",
            "Videos",
            ".ssh",
            ".gnupg",
        ] {
            out.push(home.join(leaf));
        }
    }
    out
}

/// Alt ağacıyla birlikte korunan kökler (`/` ve `/var` yalnızca kendisidir:
// çocukları tek tek değerlendirilir).
const SUBTREE_CRITICAL: &[&str] = &[
    "/proc",
    "/sys",
    "/dev",
    "/boot",
    "/etc",
    "/usr",
    "/bin",
    "/sbin",
    "/lib",
    "/lib32",
    "/lib64",
    "/libx32",
    "/root",
    "/run",
    "/srv",
    "/opt",
    "/System",
    "/Library",
    "/Applications",
    "/Network",
    "/private",
    "/var/lib",
    "/var/db",
];

/// Yol kritik mi? Kökün kendisi her zaman kritiktir; alt ağaç yalnızca
/// [`SUBTREE_CRITICAL`] listesindeki köklerde korunur (açık istisnalar hariç).
pub fn is_critical(path: &Path) -> bool {
    let norm = crate::core::path::normalize(path);
    for root in CRITICAL_DIRS {
        if crate::core::path::path_equal(&norm, Path::new(root)) {
            return true;
        }
    }
    for root in user_data_roots() {
        if crate::core::path::path_equal(&norm, &root) {
            return true;
        }
    }
    for root in SUBTREE_CRITICAL {
        let root = Path::new(root);
        if crate::core::path::path_starts_with(&norm, root)
            && !crate::core::path::path_equal(&norm, root)
            && !is_explicitly_cleanable(&norm)
        {
            return true;
        }
    }
    false
}

/// Kritik ağaçların içinde yer alıp yine de temizlenebilir alt yollar.
fn is_explicitly_cleanable(path: &Path) -> bool {
    let text = path.to_string_lossy().replace('\\', "/");
    const ALLOW: &[&str] = &[
        "/var/log/journal",
        "/var/cache/apt/archives",
        "/var/cache/dnf",
        "/var/cache/yum",
        "/var/cache/pacman/pkg",
        "/var/cache/zypp",
        "/var/lib/snapd/cache",
        "/var/lib/flatpak",
        "/var/lib/docker/containers",
        "/var/tmp",
    ];
    ALLOW
        .iter()
        .any(|a| text == *a || text.starts_with(&format!("{a}/")))
}

/// Tam güvenlik denetimi: kritik dizin, Guard, symlink, yetki.
///
/// `Ok(())` ise silme yapılabilir; `Err` insan dilinde nedendir.
pub fn check_path(path: &Path, ctx: &RunContext) -> Result<(), String> {
    // Son doğrulama mutlak yol ister: göreli yol hangi ağaca çözüleceği
    // çalışma dizinine bağlıdır, sessizce yanlış yeri silebilir.
    if path.is_relative() {
        return Err(crate::i18n::t(&ctx.lang, "relative path refused"));
    }
    if is_critical(path) {
        return Err(crate::i18n::t(&ctx.lang, "critical system dir (whitelist)"));
    }
    if let Err(err) = ctx.guard.check(path, false) {
        return Err(crate::i18n::et(
            &ctx.lang,
            "guard blocked: {}",
            &[&err.to_string()],
        ));
    }
    // Symlink: hedefin kendisi asla izlenmez; yalnızca bağın kendisi
    // silinebilir ve o da yalnızca temizlenebilir bir dizin içindeyse.
    if let Ok(md) = std::fs::symlink_metadata(path) {
        if md.file_type().is_symlink() {
            match std::fs::read_link(path) {
                Ok(target) => {
                    let joined = if target.is_absolute() {
                        target
                    } else {
                        path.parent().map(|p| p.join(&target)).unwrap_or(target)
                    };
                    // `..` içeren göreli hedefleri sözlüksel olarak çöz: yoksa
                    // `temizlenebilir/alt/../../etc` biçiminde bir bağ kritik
                    // denetiminden kaçardı.
                    let resolved = crate::core::path::normalize(&joined);
                    if is_critical(&resolved) || is_critical(path) {
                        return Err(crate::i18n::t(&ctx.lang, "symlink leads into critical dir"));
                    }
                }
                Err(err) => {
                    return Err(crate::i18n::et(
                        &ctx.lang,
                        "broken symlink unreadable: {}",
                        &[&err.to_string()],
                    ))
                }
            }
        }
    }
    // Yetki: root gerektiren konumlarda rootsuz çalışmayı sessizce atla.
    #[cfg(unix)]
    {
        if is_root_only(path) && unsafe { libc::geteuid() } != 0 {
            return Err(crate::i18n::t(&ctx.lang, "needs root (skipped)"));
        }
    }
    // Üst dizin yazılabilir mi?
    if !ctx.dry_run {
        if let Some(parent) = path.parent() {
            if parent.exists() && std::fs::metadata(parent).is_ok() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::{MetadataExt, PermissionsExt};
                    if let Ok(md) = std::fs::metadata(parent) {
                        let mode = md.permissions().mode();
                        let euid = unsafe { libc::geteuid() };
                        let writable = if euid == 0 {
                            true
                        } else if md.uid() == euid {
                            mode & 0o200 != 0
                        } else {
                            mode & 0o002 != 0
                        };
                        if !writable {
                            return Err(crate::i18n::t(&ctx.lang, "no write permission"));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Sistem alanı mı (root gerektirir)?
#[cfg(unix)]
fn is_root_only(path: &Path) -> bool {
    let text = path.to_string_lossy().replace('\\', "/");
    text.starts_with("/var/log/")
        || text.starts_with("/var/cache/")
        || text.starts_with("/var/lib/")
        || text.starts_with("/boot/")
        || text.starts_with("/usr/")
        || text == "/var/log"
        || text == "/var/cache"
}

/// Varsayılan işlem logu dosyası: `~/.local/share/sweep/operations.log`
/// (taşınabilir modda `<base>/operations.log`).
pub fn default_log_file() -> PathBuf {
    if let Some(base) = crate::config::portable_base() {
        return base.join("operations.log");
    }
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sweep")
        .join("operations.log")
}

/// İşlem logunu ekleme kipinde aç; **sembolik bağı asla izleme**.
///
/// Sweep root olarak çalışabilir (apt, journald, `/var/log/nginx`), log dosyası
/// ise kullanıcının kendi veri dizininde durur. O yol örneğin
/// `/etc/cron.d/x`'e bağlanmışsa, sade `open(..., append)` ayrıcalıksız bir
/// kullanıcının root'a rastgele bir dosyaya (içeriği kısmen kullanıcı tanımlı
/// cleaner adlarından gelen) metin yazdırmasını sağlar. POSIX'te `O_NOFOLLOW`
/// artı Windows'ta açık reparse-point denetimi bunu kapatır.
fn open_log_append(log_path: &Path) -> std::io::Result<std::fs::File> {
    if crate::core::keep::is_link(log_path) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "refusing to write the operation log through a symlink",
        ));
    }
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }
    options.open(log_path)
}

/// İşlemi log dosyasına ekle: `zaman | mod | yol | bayt | kuru/gerçek`.
pub fn log_operation(
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    path: &Path,
    bytes: u64,
    skipped: bool,
) {
    let log_path = ctx.log_file.clone().unwrap_or_else(default_log_file);
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mode = if ctx.dry_run {
        "preview"
    } else if skipped {
        "skip"
    } else {
        "clean"
    };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let line = format!(
        "{ts} | {mode} | {cleaner}.{option} | {} | {bytes}B\n",
        path.display()
    );
    match open_log_append(&log_path) {
        Ok(mut file) => {
            let _ = file.write_all(line.as_bytes());
        }
        Err(err) => log::warn!(
            "operation log not written ({}): {}",
            log_path.display(),
            err
        ),
    }
}

/// Temizlik koşusu başlangıç işareti: `ts | run | <id> | <seçim> | 0B`.
///
/// `undo --last` / `--run` bu işaretlere göre gruplar; eski işaretçisiz
/// loglar da desteklenir (işaret yoksa tüm log tek koşu sayılır). Dönen
/// kimlik aynı saniyedeki ardışık koşularda bile tektir (pid + sayaç).
pub fn log_run_start(ctx: &RunContext, selection: &str) -> String {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let id = format!(
        "r{ts}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let log_path = ctx.log_file.clone().unwrap_or_else(default_log_file);
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut summary = selection.trim().to_string();
    if summary.is_empty() {
        summary = "all".to_string();
    }
    // Tek satırda kalmalı: ayırıcı ve satır sonu temizlenir (yol değildir,
    // yalnızca görüntü metnidir; çözümleyici sondan okuduğu için ` | `
    // içerse bile zararsızdır).
    summary = summary.replace(['\r', '\n'], " ");
    if summary.len() > 240 {
        // Bayt kesimi panik yapmasın: karakter sınırına yuvarla
        // (`floor_char_boundary` 1.88'de kararsız, elle inilir).
        let mut end = 240;
        while !summary.is_char_boundary(end) {
            end -= 1;
        }
        summary.truncate(end);
    }
    let line = format!("{ts} | run | {id} | {summary} | 0B\n");
    match open_log_append(&log_path) {
        Ok(mut file) => {
            let _ = file.write_all(line.as_bytes());
        }
        Err(err) => log::warn!(
            "operation log not written ({}): {}",
            log_path.display(),
            err
        ),
    }
    id
}

/// `--backup-dir` verildiyse dosyayı dizin yapısını koruyarak yedekle.
///
/// 100 MB üstü dosyalar yedeklenmez (log'a not düşülür).
pub fn maybe_backup(ctx: &RunContext, path: &Path) {
    let Some(backup_root) = ctx.backup_dir.clone() else {
        return;
    };
    if ctx.dry_run || !path.is_file() {
        return;
    }
    const LIMIT: u64 = 100 * 1024 * 1024;
    let size = crate::fsutil::size::size_of_or_zero(path);
    if size > LIMIT {
        log::warn!(
            "backup skipped ({} > 100MB): {}",
            crate::fsutil::size::bytes_to_human(size, false),
            path.display()
        );
        return;
    }
    let rel = path
        .strip_prefix("/")
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|_| PathBuf::from(path.file_name().unwrap_or_default()));
    let dest = backup_root.join(rel);
    if let Some(parent) = dest.parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return;
        }
    }
    if std::fs::copy(path, &dest).is_ok() {
        log::info!("yedeklendi: {} -> {}", path.display(), dest.display());
    }
}

/// Bayt sayısını rapor satırı için insan diline çevir.
pub fn human(bytes: u64) -> String {
    crate::fsutil::size::bytes_to_human(bytes, false)
}

/// Efektif root mu? (Unix'te euid==0; diğer platformlarda false.)
pub fn is_root() -> bool {
    #[cfg(unix)]
    {
        unsafe { libc::geteuid() == 0 }
    }
    #[cfg(not(unix))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The critical-dir table is a POSIX one; on Windows the equivalent net is
    // `Guard::protected_roots()` (see `core::keep::tests`).
    #[cfg(unix)]
    #[test]
    fn critical_roots() {
        // Köklerin kendisi her zaman kritik.
        assert!(is_critical(Path::new("/")));
        assert!(is_critical(Path::new("/usr")));
        assert!(is_critical(Path::new("/etc")));
        assert!(is_critical(Path::new("/var")));
        // Sistem alt ağaçları kritik.
        assert!(is_critical(Path::new("/usr/bin/foo")));
        assert!(is_critical(Path::new("/etc/ssh/sshd_config")));
        assert!(is_critical(Path::new("/proc/1/cmdline")));
        // Temizlenebilir konumlar kritik DEĞİL (bu testin anlamı!).
        assert!(!is_critical(Path::new("/tmp/foo")));
        assert!(!is_critical(Path::new("/var/tmp/bar")));
        assert!(!is_critical(Path::new("/var/log/journal/baz")));
        assert!(!is_critical(Path::new("/var/cache/apt/archives/x.deb")));
        assert!(!is_critical(Path::new("/home/user/.cache/pip")));
    }

    #[test]
    fn relative_paths_are_refused() {
        // Son doğrulama mutlak yol ister: göreli yolun nereye çözüleceği
        // çalışma dizinine bağlıdır.
        let ctx = crate::action::RunContext::preview();
        assert!(check_path(Path::new("relative/f.txt"), &ctx).is_err());
        assert!(check_path(Path::new("../etc/passwd"), &ctx).is_err());
    }

    #[test]
    fn dotdot_symlink_target_stays_critical() {
        // `/tmp/sweep-cleanable/sub` altındaki bir bağ `../../../etc/passwd`
        // gösteriyorsa sözlüksel çözüm `/etc/passwd` olur: normalize
        // edilmezse kritik denetimi kaçırırdı.
        let joined = Path::new("/tmp/sweep-cleanable/sub").join("../../../etc/passwd");
        let resolved = crate::core::path::normalize(&joined);
        assert_eq!(resolved, PathBuf::from("/etc/passwd"));
        assert!(is_critical(&resolved));
        // İki `..` yalnızca `/tmp` altına çıkarır: kritik değildir.
        let shallow = Path::new("/tmp/sweep-cleanable/sub").join("../../etc/passwd");
        assert!(!is_critical(&crate::core::path::normalize(&shallow)));
    }

    /// Sweep can run as root, and the operation log lives in the user's own
    /// data directory. A plain `open(.., append)` would follow a planted
    /// symlink and let an unprivileged user make root append to an arbitrary
    /// file, so the log must be opened without following links.
    #[test]
    fn log_refuses_a_symlinked_path() {
        let base = std::env::temp_dir().join("sweep_log_link_test");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let target = base.join("target.log");
        std::fs::write(&target, b"").unwrap();
        let link = base.join("operations.log");

        let made = {
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink(&target, &link).is_ok()
            }
            #[cfg(windows)]
            {
                std::os::windows::fs::symlink_file(&target, &link).is_ok()
            }
        };
        // `symlink_file` reporting Ok does NOT mean the link exists: Windows
        // returns success without creating anything when the caller lacks
        // SeCreateSymbolicLinkPrivilege. Verify the link for real, otherwise the
        // assertion below would test a plain, non-existent path.
        let really_a_link = std::fs::symlink_metadata(&link)
            .map(|md| md.file_type().is_symlink())
            .unwrap_or(false);
        if !made || !really_a_link {
            // Windows needs elevation / Developer Mode to create links.
            let _ = std::fs::remove_dir_all(&base);
            return;
        }

        assert!(
            open_log_append(&link).is_err(),
            "a symlinked log path must be refused"
        );
        // The link target must be untouched.
        assert_eq!(std::fs::read(&target).unwrap().len(), 0);

        let _ = std::fs::remove_dir_all(&base);
    }
}
