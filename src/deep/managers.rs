//! Sistem paket yöneticisi tespiti.
//!
//! `sweep diagnostics --json` ve GUI'deki "yalnızca kurulu olanlar" filtresi
//! buradan beslenir. Liste bilinçli olarak dardır: yalnızca dağıtımın
//! sistem paket yöneticileri (kullanıcı-alanı araçları değil).

/// Tek yönetici: ikili adı + görünen etiket.
#[derive(Debug, Clone, Copy)]
pub struct Manager {
    /// `$PATH`'ta aranan ikili (örn. `pacman`).
    pub bin: &'static str,
    /// İnsan-dili etiket.
    pub label: &'static str,
    /// Bu yöneticinin süpürdüğü cleaner kimliği (`cleaners/*.toml`).
    /// Karşılığı yoksa boş (örn. `apk`).
    pub cleaner: &'static str,
}

/// Yalnızca sistemsel yöneticiler.
pub const SYSTEM_MANAGERS: &[Manager] = &[
    Manager {
        bin: "apt-get",
        label: "APT (Debian/Ubuntu)",
        cleaner: "apt",
    },
    Manager {
        bin: "dnf",
        label: "DNF (Fedora/RHEL)",
        cleaner: "dnf",
    },
    Manager {
        bin: "yum",
        label: "YUM (RHEL-uyumlu)",
        cleaner: "dnf",
    },
    Manager {
        bin: "pacman",
        label: "pacman (Arch)",
        cleaner: "pacman",
    },
    Manager {
        bin: "zypper",
        label: "zypper (openSUSE)",
        cleaner: "zypper",
    },
    Manager {
        bin: "apk",
        label: "APK (Alpine)",
        cleaner: "",
    },
    Manager {
        bin: "xbps-install",
        label: "XBPS (Void)",
        cleaner: "",
    },
    Manager {
        bin: "eopkg",
        label: "eopkg (Solus)",
        cleaner: "",
    },
    Manager {
        bin: "emerge",
        label: "Portage (Gentoo)",
        cleaner: "",
    },
];

/// Tespit sonucu (serde ile JSON'a dökülür).
#[derive(Debug, Clone, serde::Serialize)]
pub struct ManagerStatus {
    pub bin: &'static str,
    pub label: &'static str,
    pub cleaner: &'static str,
    pub found: bool,
}

/// `$PATH`'taki sistem yöneticilerini tara.
pub fn detect() -> Vec<ManagerStatus> {
    SYSTEM_MANAGERS
        .iter()
        .map(|m| ManagerStatus {
            bin: m.bin,
            label: m.label,
            cleaner: m.cleaner,
            found: crate::deep::have(m.bin),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_without_crashing() {
        let all = detect();
        assert_eq!(all.len(), SYSTEM_MANAGERS.len());
        // En az tablo-shape garantisi: her girdide bin dolu.
        assert!(all.iter().all(|m| !m.bin.is_empty()));
    }
}
