//! Gizlilik kalkanı: telemetri kapatma, son kullanılan dosyalar ve pano.
//!
//! `sweep privacy shield` işletim sisteminin telemetri/diagnostik toplamasını
//! kapatır; `sweep privacy wipe` son kullanılan dosya listelerini siler ve
//! panoyu boşaltır.
//!
//! - **Windows:** `AllowTelemetry=0` ve `DisableWebSearch=1` kayıt defteri
//!   ayarları (HKLM yazımı yönetici ister); `%APPDATA%\Microsoft\Windows\Recent`
//!   içeriği; pano `platform::clear_clipboard()`.
//! - **Linux:** `whoopsie` / `apport` servisleri ve journal iletimi kapatılır;
//!   `~/.local/share/recently-used.xbel` silinir; pano `xclip` ile boşaltılır.
//! - **macOS:** `defaults write` ile diagnostik gönderimi kapatılır;
//!   `com.apple.recentitems.plist` ve paylaşılan dosya listeleri sıfırlanır;
//!   pano `pbcopy < /dev/null` ile boşaltılır.
//!
//! Her adım `--dry-run` destekler. Hiçbir zaman özyinelemeli dizin silme
//! yapılmaz: son kullanılanlar klasörünün *içindeki* dosyalar tek tek silinir,
//! klasörün kendisi korunur.

use std::path::PathBuf;

use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "privacy";

/// Gizlilik seçenekleri.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PrivacyOptions {
    /// Hiçbir şeyi değiştirme; yalnız ne yapılacağını raporla.
    pub dry_run: bool,
}

/// Telemetri kapatma adımlarının insan okunur planı (dry-run + rapor için).
///
/// Saf fonksiyon — hiçbir şey çalıştırmaz, hiçbir kayıt defteri anahtarına
/// dokunmaz; yalnız bu platformda uygulanacak adımları listeler.
pub fn shield_plan() -> Vec<String> {
    #[cfg(windows)]
    {
        win::tweaks()
            .into_iter()
            .map(|t| format!("registry {}\\{} = {}", t.path, t.name, t.value))
            .collect()
    }
    #[cfg(target_os = "linux")]
    {
        linux_units()
            .into_iter()
            .map(|unit| format!("systemctl disable --now {unit}"))
            .collect()
    }
    #[cfg(target_os = "macos")]
    {
        macos_defaults()
            .into_iter()
            .map(|(domain, key, value)| format!("defaults write {domain} {key} {value}"))
            .collect()
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        Vec::new()
    }
}

/// Son kullanılan dosya kayıtları: (yol, klasör mü?).
///
/// Windows'ta `Recent` klasörü, diğerlerinde tek dosyalar döner. Klasörler
/// özyinelemeli silinmez; yalnız içindeki dosyalar temizlenir.
pub fn recent_targets() -> Vec<(PathBuf, bool)> {
    let mut out = Vec::new();

    // Her platform kendi hedeflerini ekler; `home` yalnız gereken dalda okunur
    // (Windows'ta `%APPDATA%` kullanılır, `home` hiç anılmaz).
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(home) = crate::platform::home_dir() {
            out.push((home.join(".recently-used.xbel"), false));
            out.push((home.join(".local/share/recently-used.xbel"), false));
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Some(home) = crate::platform::home_dir() {
            out.push((home.join("Library/Preferences/com.apple.recentitems.plist"), false));
            out.push((
                home.join(
                    "Library/Application Support/com.apple.sharedfilelist/com.apple.LSSharedFileList.RecentDocuments.sfl2",
                ),
                false,
            ));
        }
    }
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            out.push((PathBuf::from(appdata).join("Microsoft\\Windows\\Recent"), true));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Windows kayıt defteri
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod win {
    use winreg::enums::*;
    use winreg::RegKey;

    /// Tek bir telemetri ayarı.
    #[derive(Debug, Clone, Copy)]
    pub struct Tweak {
        /// `true` ise HKLM (yönetici ister), değilse HKCU.
        pub system: bool,
        /// Alt anahtar yolu.
        pub path: &'static str,
        /// Değer adı.
        pub name: &'static str,
        /// Yazılacak DWORD.
        pub value: u32,
    }

    /// Kapatılacak telemetri ayarları.
    ///
    /// `AllowTelemetry=0` kurumsal politikadır ve yönetici ister; kullanıcı
    /// tarafındaki arama ayarları yükseltme olmadan yazılabilir.
    pub fn tweaks() -> Vec<Tweak> {
        vec![
            Tweak {
                system: true,
                path: r"SOFTWARE\Policies\Microsoft\Windows\DataCollection",
                name: "AllowTelemetry",
                value: 0,
            },
            Tweak {
                system: true,
                path: r"SOFTWARE\Policies\Microsoft\Windows\DataCollection",
                name: "DoNotShowFeedbackNotifications",
                value: 1,
            },
            Tweak {
                system: false,
                path: r"SOFTWARE\Microsoft\Windows\CurrentVersion\Search",
                name: "DisableWebSearch",
                value: 1,
            },
            Tweak {
                system: false,
                path: r"SOFTWARE\Microsoft\Windows\CurrentVersion\Search",
                name: "BingSearchEnabled",
                value: 0,
            },
        ]
    }

    /// Ayarları uygula; yazılanların sayısını ve atlananları döndür.
    pub fn apply(report: &mut super::Report) {
        for tweak in tweaks() {
            let hive = if tweak.system {
                RegKey::predef(HKEY_LOCAL_MACHINE)
            } else {
                RegKey::predef(HKEY_CURRENT_USER)
            };
            let key = hive.create_subkey(tweak.path).map(|(key, _)| key);
            match key {
                Ok(key) => match key.set_value(tweak.name, &tweak.value) {
                    Ok(()) => super::push(
                        report,
                        "shield",
                        format!(
                            "telemetry disabled: {}\\{} = {}",
                            tweak.path, tweak.name, tweak.value
                        ),
                        0,
                    ),
                    Err(err) => {
                        let hint = if tweak.system {
                            " (needs administrator)"
                        } else {
                            ""
                        };
                        report.fail(
                            super::CLEANER_ID,
                            "shield",
                            format!("{}\\{}: {err}{hint}", tweak.path, tweak.name),
                        );
                    }
                },
                Err(err) => report.fail(
                    super::CLEANER_ID,
                    "shield",
                    format!("cannot open {}\\{err}", tweak.path),
                ),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Linux systemd birimleri
// ---------------------------------------------------------------------------

/// Kapatılacak systemd birimleri (telemetri / hata bildirimi).
#[cfg(target_os = "linux")]
pub fn linux_units() -> Vec<&'static str> {
    vec![
        "whoopsie.service",
        "apport.service",
        "systemd-journal-upload.service",
        "systemd-journal-remote.service",
    ]
}

/// Birim var mı? (`systemctl list-unit-files` çıktısına bakar.)
#[cfg(target_os = "linux")]
fn unit_exists(unit: &str) -> bool {
    crate::deep::command_output("systemctl", &["list-unit-files", "--no-legend", unit])
        .map(|out| out.contains(unit))
        .unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn shield_linux(opts: &PrivacyOptions, report: &mut Report) {
    if !crate::deep::have("systemctl") {
        report.fail(CLEANER_ID, "shield", "systemctl not found");
        return;
    }
    for unit in linux_units() {
        if !unit_exists(unit) {
            continue;
        }
        if opts.dry_run {
            push(
                report,
                "shield",
                format!("would run: systemctl disable --now {unit}"),
                0,
            );
            continue;
        }
        match crate::fsutil::run_command("systemctl", &["disable", "--now", unit], true) {
            Ok((0, _, _)) => push(
                report,
                "shield",
                format!("telemetry service disabled: {unit}"),
                0,
            ),
            Ok((code, _, err)) => report.fail(
                CLEANER_ID,
                "shield",
                format!("{unit} exited {code} (needs root?): {}", err.trim()),
            ),
            Err(err) => report.fail(CLEANER_ID, "shield", format!("{unit}: {err}")),
        }
    }
}

// ---------------------------------------------------------------------------
// macOS defaults
// ---------------------------------------------------------------------------

/// Yazılacak `defaults` anahtarları: (domain, key, value).
#[cfg(target_os = "macos")]
pub fn macos_defaults() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("com.apple.CrashReporter", "DialogType", "none"),
        ("com.apple.SubmitDiagInfo", "AutoSubmit", "-bool false"),
        ("com.apple.AdLib", "allowApplePersonalizedAdvertising", "-bool false"),
    ]
}

#[cfg(target_os = "macos")]
fn shield_macos(opts: &PrivacyOptions, report: &mut Report) {
    if !crate::deep::have("defaults") {
        report.fail(CLEANER_ID, "shield", "defaults not found");
        return;
    }
    for (domain, key, value) in macos_defaults() {
        if opts.dry_run {
            push(
                report,
                "shield",
                format!("would run: defaults write {domain} {key} {value}"),
                0,
            );
            continue;
        }
        let args: Vec<&str> = std::iter::once("write")
            .chain(std::iter::once(domain))
            .chain(std::iter::once(key))
            .chain(value.split_whitespace())
            .collect();
        match crate::fsutil::run_command("defaults", &args, true) {
            Ok((0, _, _)) => push(
                report,
                "shield",
                format!("diagnostics disabled: {domain}.{key}"),
                0,
            ),
            Ok((code, _, err)) => report.fail(
                CLEANER_ID,
                "shield",
                format!("{domain}.{key} exited {code}: {}", err.trim()),
            ),
            Err(err) => report.fail(CLEANER_ID, "shield", format!("{domain}.{key}: {err}")),
        }
    }
}

// ---------------------------------------------------------------------------
// Ortak rapor satırı
// ---------------------------------------------------------------------------

/// Ortak rapor satırı.
fn push(report: &mut Report, option: &str, label: String, bytes: u64) {
    report.push(Entry::new(
        EntryKind::Command,
        CLEANER_ID,
        option,
        label,
        None,
        bytes,
    ));
}

/// Telemetriyi kapat.
pub fn shield(opts: &PrivacyOptions) -> Report {
    let mut report = Report::new();

    #[cfg(windows)]
    {
        if opts.dry_run {
            for line in shield_plan() {
                push(&mut report, "shield", format!("would write {line}"), 0);
            }
        } else {
            win::apply(&mut report);
        }
    }

    #[cfg(target_os = "linux")]
    shield_linux(opts, &mut report);

    #[cfg(target_os = "macos")]
    shield_macos(opts, &mut report);

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = opts;
        report.fail(CLEANER_ID, "shield", "privacy shield is unsupported here");
    }

    if report.entries.is_empty() && report.errors.is_empty() {
        push(
            &mut report,
            "shield",
            "no telemetry service found to disable".to_string(),
            0,
        );
    }
    report
}

/// Bir klasörün içindeki dosyaları tek tek sil (klasörün kendisi kalır).
fn wipe_dir_contents(dir: &std::path::Path, report: &mut Report) -> u64 {
    let mut removed = 0u64;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Özyinelemeli silme yok: yalnız doğrudan çocuk dosyalar.
            continue;
        }
        let bytes = crate::fsutil::size::size_of_or_zero(&path);
        match std::fs::remove_file(&path) {
            Ok(()) => {
                removed += bytes;
                push(
                    report,
                    "recent",
                    format!("removed {}", path.display()),
                    bytes,
                );
            }
            Err(err) => report.fail(CLEANER_ID, "recent", format!("{}: {err}", path.display())),
        }
    }
    removed
}

/// Son kullanılan dosya kayıtlarını ve panoyu temizle.
pub fn wipe(opts: &PrivacyOptions) -> Report {
    let mut report = Report::new();

    for (path, is_dir) in recent_targets() {
        if !path.exists() {
            continue;
        }
        if opts.dry_run {
            push(
                &mut report,
                "recent",
                format!("would wipe {}", path.display()),
                0,
            );
            continue;
        }
        if is_dir {
            wipe_dir_contents(&path, &mut report);
        } else {
            let bytes = crate::fsutil::size::size_of_or_zero(&path);
            match std::fs::remove_file(&path) {
                Ok(()) => push(
                    &mut report,
                    "recent",
                    format!("removed {}", path.display()),
                    bytes,
                ),
                Err(err) => report.fail(
                    CLEANER_ID,
                    "recent",
                    format!("{}: {err}", path.display()),
                ),
            }
        }
    }

    if opts.dry_run {
        push(&mut report, "clipboard", "would clear the clipboard".to_string(), 0);
    } else {
        match crate::platform::clear_clipboard() {
            Ok(()) => push(
                &mut report,
                "clipboard",
                "clipboard cleared".to_string(),
                0,
            ),
            // GUI oturumu yoksa (SSH, saf konsol) pano zaten yoktur: hata değil.
            Err(err) => push(
                &mut report,
                "clipboard",
                format!("clipboard skipped: {err}"),
                0,
            ),
        }
    }

    report
}

/// `sweep privacy <op>` girişi. `op`: shield | wipe.
pub fn run(op: &str, opts: &PrivacyOptions) -> Report {
    match op {
        "shield" | "telemetry" => shield(opts),
        "wipe" | "clean" => wipe(opts),
        other => {
            let mut report = Report::new();
            report.fail(
                CLEANER_ID,
                "args",
                crate::i18n::et(
                    &crate::i18n::Lang::detect(&[]),
                    "unknown op: {} (shield/wipe)",
                    &[&other.to_string()],
                ),
            );
            report
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shield_plan_is_never_empty_on_supported_platforms() {
        let plan = shield_plan();
        #[cfg(any(windows, target_os = "linux", target_os = "macos"))]
        assert!(!plan.is_empty(), "plan: {plan:?}");
        #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
        assert!(plan.is_empty());
    }

    #[test]
    fn dry_run_shield_changes_nothing() {
        let report = shield(&PrivacyOptions { dry_run: true });
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        assert!(!report.entries.is_empty());
        assert_eq!(report.reclaimed(), 0);
    }

    #[test]
    fn dry_run_wipe_changes_nothing() {
        let report = wipe(&PrivacyOptions { dry_run: true });
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        // Pano adımı her zaman raporlanır.
        assert!(report.entries.iter().any(|e| e.option == "clipboard"));
        assert_eq!(report.reclaimed(), 0);
    }

    #[test]
    fn unknown_op_fails_cleanly() {
        let report = run("nope", &PrivacyOptions::default());
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].message.contains("unknown op"));
    }

    #[test]
    fn wipe_dir_contents_keeps_the_directory() {
        let root = std::env::temp_dir().join("sweep_privacy_recent_fixture");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("keepdir")).unwrap();
        std::fs::write(root.join("a.lnk"), vec![0u8; 40]).unwrap();
        std::fs::write(root.join("b.lnk"), vec![0u8; 60]).unwrap();

        let mut report = Report::new();
        let removed = wipe_dir_contents(&root, &mut report);
        assert_eq!(removed, 100);
        assert!(root.is_dir());
        // Alt klasör korunur (özyinelemeli silme yok).
        assert!(root.join("keepdir").is_dir());
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn recent_targets_are_platform_appropriate() {
        let targets = recent_targets();
        // Ev dizini varsa en az bir hedef beklenir.
        if crate::platform::home_dir().is_some() {
            #[cfg(not(windows))]
            assert!(!targets.is_empty());
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        assert!(targets.iter().any(|(p, _)| p.ends_with("recently-used.xbel")));
        // `%APPDATA%` yoksa (kısıtlı bir CI kabuğu gibi) Windows hedef
        // listesi boş kalır; bu bir hata değil, ortam eksikliğidir.
        #[cfg(windows)]
        if std::env::var("APPDATA").is_ok() {
            assert!(targets.iter().any(|(_, is_dir)| *is_dir));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_units_cover_whoopsie_and_apport() {
        let units = linux_units();
        assert!(units.iter().any(|u| u.contains("whoopsie")));
        assert!(units.iter().any(|u| u.contains("apport")));
        assert!(units.iter().any(|u| u.contains("journal-upload")));
    }

    #[cfg(windows)]
    #[test]
    fn windows_tweaks_disable_telemetry_and_web_search() {
        let tweaks = win::tweaks();
        assert!(tweaks
            .iter()
            .any(|t| t.name == "AllowTelemetry" && t.value == 0 && t.system));
        assert!(tweaks
            .iter()
            .any(|t| t.name == "DisableWebSearch" && t.value == 1 && !t.system));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_defaults_disable_diagnostics() {
        let defaults = macos_defaults();
        assert!(defaults.iter().any(|(_, key, _)| *key == "AutoSubmit"));
        assert!(defaults.iter().any(|(_, key, _)| *key == "DialogType"));
    }
}
