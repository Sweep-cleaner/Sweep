//! Sınıflandırma: ad + komut + konumdan kategori ve risk.
//!
//! Saf fonksiyon ([`classify`]) — dosya sistemi, kayıt defteri ya da başka
//! bir G/Ç'ye dokunmaz; tablo tabanlı birim testleriyle doğrulanır.
//!
//! Kural sırası önemlidir (ilk eşleşen kazanır):
//! 1. `Winlogon` / `Policies\Explorer\Run` → `os` + `critical`
//! 2. güvenlik yazılımı → `security` + `high`
//! 3. sürücü yardımcısı → `driver`
//! 4. güncelleyici → `updater`
//! 5. işletim sistemi bileşeni → `os` + `high`
//! 6. bilinen üçüncü taraf → `third-party` + `low`
//! 7. geri kalan → `unknown` + `medium` (**asla sessizce düşük risk**)

use super::{Category, Risk, Source};

/// Sınıflandırma sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classification {
    pub category: Category,
    pub risk: Risk,
    pub notes: Option<String>,
}

impl Classification {
    fn new(category: Category, risk: Risk, notes: &str) -> Self {
        Self {
            category,
            risk,
            notes: if notes.is_empty() {
                None
            } else {
                Some(notes.to_string())
            },
        }
    }
}

/// Güvenlik yazılımı izleri (`category: security`). Kapatma uyarı üretir.
const SECURITY: &[&str] = &[
    "defender", "msmpeng", "nissrv", "securityhealth", "windows security",
    "carbon black", "cbdefense", "crowdstrike", "csagent", "falcon",
    "sentinelone", "sentinelagent", "cylance", "cortex", "traps",
    "kaspersky", "avp", "bitdefender", "bdagent", "avast", "avgnt", "avgsvc",
    "eset", "ekrn", "nod32", "malwarebytes", "mbam", "norton", "nortonsecurity",
    "mcafee", "mcuicnt", "sophos", "trend micro", "tmccsf", "webroot",
    "zonealarm", "comodo", "360tray", "qihoo", "huorong", "avira", "avg",
    "fsecure", "gdata", "panda", "adaware", "vipre", "emsisofter",
    "antivirus", "antimalware", "firewall", "endpoint protection",
];

/// Sürücü yardımcıları (`category: driver`).
const DRIVER: &[&str] = &[
    "\\drivers\\", "driver", "nvcontainer", "nvdisplay", "nvidia", "nvtmru",
    "igfxtray", "igfxpers", "hkcmd", "rtkaud", "rtkngui", "realtek",
    "amd", "radeon", "ati ", "intel", "graphics", "chipset", "audio",
    "synaptics", "elan", "touchpad", "trackpad", "razer", "corsair",
    "steelseries", "logi", "logitech", "keyboard", "mouse", "bluetooth",
    "wacom", "tablet", "asus", "msi ", "gigabyte", "canon", "epson",
    "brother", "hp printer", "print driver", "dellsupport", "dell",
    "lenovo", "thinkpad", "vantage", "myasus", "armoury",
];

/// Güncelleyiciler (`category: updater`).
const UPDATER: &[&str] = &[
    "update", "updater", "upgrade", "autoupdate", "check_update",
    "googleupdate", "edgeupdate", "microsoftedgeupdate", "teamsupdate",
    "installer", "squirrel", "wuauclt", "usoclient", "mousocoreworker",
    "wuapihost", "updatechecker", "softwareupdate", "sparkle",
];

/// İşletim sistemi izleri (`category: os`).
const OS_PATH: &[&str] = &[
    "\\windows\\", "/windows/", "\\system32\\", "\\syswow64\\",
    "\\windowsapps\\", "%windir%", "\\winsxs\\", "\\microsoft.net\\",
    "\\windows nt\\",
];

/// İşletim sistemi adı izleri (yalnız ad için, yol değil).
const OS_NAME: &[&str] = &["windows ", "windows", "microsoft windows", "windowsapps"];

/// Bilinen üçüncü taraf satıcılar (`category: third-party`).
const THIRD_PARTY: &[&str] = &[
    "onedrive", "dropbox", "google drive", "googledrive", "icloud", "box sync",
    "spotify", "steam", "epic games", "battle.net", "gog galaxy", "origin",
    "riot", "valorant", "league of legends", "minecraft", "roblox", "lunar client",
    "discord", "slack", "teams", "zoom", "telegram", "signal", "skype",
    "whatsapp", "thunderbird", "outlook", "notion", "obsidian", "evernote",
    "obs", "adobe", "creative cloud", "itunes", "quicktime", "vlc",
    "docker", "virtualbox", "vmware", "jetbrains", "intellij", "pycharm",
    "vscode", "visual studio code", "code.exe", "git", "node", "python",
    "java", "oracle", "cortana", "xbox", "gamebar", "yourphone",
    "phone link", "to do", "todoist", "everything", "greenshot", "sharex",
    "autohotkey", "powertoys", "rainmeter", "f.lux", "flux.exe",
    "mozilla", "firefox", "chrome", "google chrome", "chromium", "brave",
    "opera", "vivaldi", "edge", "cloudflare", "warp", "tailscale", "wireguard",
    "openvpn", "protonvpn", "nordvpn", "expressvpn", "1password", "bitwarden",
    "keepass", "lastpass", "syncthing", "resilio", "megasync", "pcloud",
    "nvidia geforce experience", "geforce experience", "logi options",
    "razer synapse", "corsair icue", "wallpaper engine", "msi afterburner",
    "bandicam", "fraps", "streamlabs", "twitch", "elgato", "logitech capture",
];

/// Bilinen üçüncü taraf adları — çok kısa/kural dışı adlar için ayrı liste.
const THIRD_PARTY_EXACT: &[&str] = &["f.lux", "flux", "code", "git", "node"];

fn haystack(name: &str, command: Option<&str>) -> String {
    let mut s = name.to_lowercase();
    s.push(' ');
    if let Some(cmd) = command {
        s.push_str(&cmd.to_lowercase());
    }
    s
}

fn hits(hay: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| hay.contains(n))
}

/// Girdiyi sınıflandır (saf fonksiyon).
///
/// `location` yalnız zamanlanmış görev yolları için anlamlıdır: `\` ile
/// başlayan ve `microsoft` içeren konum OS görevidir. Kayıt defteri
/// konumları (`HKCU\...`) yanlışlıkla OS sayılmasın diye bu ayrım gerekir.
pub fn classify(
    name: &str,
    command: Option<&str>,
    location: &str,
    source: Source,
) -> Classification {
    // 1) Kritik OS mekanizmaları.
    if source == Source::Winlogon || source == Source::RegistryRunPolicy {
        return Classification::new(
            Category::Os,
            Risk::Critical,
            "OS logon policy — do not edit or remove",
        );
    }
    let hay = haystack(name, command);
    if hay.contains("winlogon") || hay.contains("userinit") {
        return Classification::new(
            Category::Os,
            Risk::Critical,
            "OS logon component",
        );
    }
    // Zamanlanmış görev yolu `\Microsoft\...` → OS.
    let loc = location.to_lowercase();
    let task_os = loc.starts_with('\\') && loc.contains("microsoft");

    // 2) Güvenlik.
    if hits(&hay, SECURITY) {
        return Classification::new(
            Category::Security,
            Risk::High,
            "security software — disabling may reduce protection",
        );
    }
    if task_os && hits(&hay, &["defender", "security", "firewall", "antimalware"]) {
        return Classification::new(
            Category::Security,
            Risk::High,
            "OS security task — disabling may reduce protection",
        );
    }

    // 3) Sürücü yardımcıları.
    if hits(&hay, DRIVER) {
        return Classification::new(Category::Driver, Risk::Medium, "");
    }

    // 4) Güncelleyiciler.
    if hits(&hay, UPDATER) {
        return Classification::new(Category::Updater, Risk::Low, "");
    }

    // 5) İşletim sistemi bileşeni (yol ya da ad).
    if hits(&hay, OS_PATH) || hits(&name.to_lowercase(), OS_NAME) || task_os {
        return Classification::new(Category::Os, Risk::High, "OS component");
    }

    // 6) Bilinen üçüncü taraf.
    if hits(&hay, THIRD_PARTY) || hits(&name.to_lowercase(), THIRD_PARTY_EXACT) {
        return Classification::new(Category::ThirdParty, Risk::Low, "");
    }

    // 7) Tanınmayan: asla düşük risk değil.
    Classification::new(
        Category::Unknown,
        Risk::Medium,
        "unrecognised entry — review before disabling",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(name: &str, command: Option<&str>) -> Classification {
        classify(name, command, "", Source::RegistryRun)
    }

    #[test]
    fn table_drives_category_and_risk() {
        let cases: &[(&str, Option<&str>, Category, Risk)] = &[
            // Güvenlik her şeyden önce gelir (Windows Defender yolu OS olsa da).
            (
                "Windows Defender",
                Some("C:\\Program Files\\Windows Defender\\MsMpEng.exe"),
                Category::Security,
                Risk::High,
            ),
            (
                "SecurityHealth",
                Some("C:\\Windows\\System32\\SecurityHealthSystray.exe"),
                Category::Security,
                Risk::High,
            ),
            // Sürücü.
            (
                "RtkAudUService64",
                Some("C:\\Windows\\System32\\RtkAudUService64.exe"),
                Category::Driver,
                Risk::Medium,
            ),
            (
                "NvBackend",
                Some("C:\\Program Files\\NVIDIA Corporation\\Update Core\\NvBackend.exe"),
                Category::Driver,
                Risk::Medium,
            ),
            // Güncelleyici.
            (
                "GoogleUpdate",
                Some("C:\\Program Files (x86)\\Google\\Update\\GoogleUpdate.exe"),
                Category::Updater,
                Risk::Low,
            ),
            (
                "MicrosoftEdgeUpdate",
                Some("C:\\Program Files (x86)\\Microsoft\\EdgeUpdate\\MicrosoftEdgeUpdate.exe"),
                Category::Updater,
                Risk::Low,
            ),
            // OS bileşeni.
            (
                "WindowsApps Helper",
                Some("C:\\Windows\\System32\\something.exe"),
                Category::Os,
                Risk::High,
            ),
            // Üçüncü taraf.
            (
                "OneDrive",
                Some("C:\\Program Files\\Microsoft OneDrive\\OneDrive.exe /background"),
                Category::ThirdParty,
                Risk::Low,
            ),
            (
                "Spotify",
                Some("C:\\Users\\u\\AppData\\Roaming\\Spotify\\Spotify.exe"),
                Category::ThirdParty,
                Risk::Low,
            ),
            (
                "CloudflareWARP",
                Some("C:\\Program Files\\Cloudflare\\Cloudflare WARP\\Cloudflare WARP.exe"),
                Category::ThirdParty,
                Risk::Low,
            ),
            (
                "Mozilla-Firefox-308046B0AF4A39CB",
                Some("\"C:\\Program Files\\Mozilla Firefox\\firefox.exe\" -os-autostart"),
                Category::ThirdParty,
                Risk::Low,
            ),
            (
                "Lunar Client",
                Some("\"C:\\Users\\u\\AppData\\Local\\Programs\\Lunar Client\\Lunar Client.exe\" --hidden"),
                Category::ThirdParty,
                Risk::Low,
            ),
            // Tanınmayan.
            ("Zorbex", Some("C:\\tools\\zorbex.exe --run"), Category::Unknown, Risk::Medium),
            ("", None, Category::Unknown, Risk::Medium),
        ];
        for (name, command, category, risk) in cases {
            let got = cat(name, *command);
            assert_eq!(
                (got.category, got.risk),
                (*category, *risk),
                "name={name:?} command={command:?} -> {got:?}"
            );
        }
    }

    #[test]
    fn winlogon_and_policy_are_critical() {
        let win = classify("Shell", Some("explorer.exe"), "", Source::Winlogon);
        assert_eq!(win.category, Category::Os);
        assert_eq!(win.risk, Risk::Critical);
        let policy = classify("Foo", Some("foo.exe"), "", Source::RegistryRunPolicy);
        assert_eq!(policy.risk, Risk::Critical);
    }

    #[test]
    fn unknown_is_never_low_risk() {
        for (name, command) in [
            ("totally-unknown-thing", None),
            ("x", Some("x")),
            ("", None),
        ] {
            let got = cat(name, command);
            assert_eq!(got.category, Category::Unknown, "{name:?}");
            assert!(
                got.risk >= Risk::Medium,
                "{name:?} düşük risk olamaz: {:?}",
                got.risk
            );
        }
    }

    #[test]
    fn registry_locations_are_not_mistaken_for_os_tasks() {
        // Kayıt defteri konumu `Software\Microsoft\Windows\...` içerir ama
        // girdi üçüncü taraf olabilir — konum tek başına OS yapmamalı.
        let got = classify(
            "OneDrive",
            Some("C:\\Program Files\\Microsoft OneDrive\\OneDrive.exe"),
            "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
            Source::RegistryRun,
        );
        assert_eq!(got.category, Category::ThirdParty);
        assert_eq!(got.risk, Risk::Low);
    }

    #[test]
    fn microsoft_task_paths_are_os() {
        let got = classify(
            "MareBackup",
            Some("C:\\Windows\\System32\\MareBackup.exe"),
            "\\Microsoft\\Windows\\Backup\\MareBackup",
            Source::ScheduledTask,
        );
        assert_eq!(got.category, Category::Os);
        assert!(got.risk >= Risk::High);
    }

    #[test]
    fn security_notes_are_present() {
        let got = cat("Windows Defender", None);
        assert_eq!(got.category, Category::Security);
        assert!(got.notes.as_deref().unwrap_or("").contains("security"));
    }
}
