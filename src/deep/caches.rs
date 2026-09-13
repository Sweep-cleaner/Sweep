//! Kullanıcı ve uygulama önbellekleri: `~/.cache` + bilinen şişkin araçlar.
//!
//! Canlı uygulamaların kilitli dizinleri (`mozilla`, `google-chrome`,
//! `thumbnails` vb.) `builtin_keep_list` tarafından zaten korunur; burada
//! ek olarak yalnızca güvenli, yeniden üretilebilir önbellekler hedeflenir.

use std::path::PathBuf;

use crate::action::RunContext;
use crate::core::report::Report;

/// Güvenle temizlenebilir önbellek alt dizinleri (ev dizinine göre).
const SAFE_CACHE_LEAVES: &[&str] = &[
    "fontconfig",
    "pip",
    "npm",
    "yarn",
    "pnpm",
    "cargo/registry",
    "gradle/caches",
    "m2/repository",
    "go-build",
    "composer/cache",
    "deno",
    "paru",
    "yay",
    "JetBrains",
    "Electron",
    "ms-playwright",
    "thumbnails/fail",
    "thumbnails/large",
    "thumbnails/normal",
    "mesa_shader_cache",
    "mozilla/firefox/Crash Reports",
    "google-chrome/Crash Reports",
    "chromium/Crash Reports",
    "BraveSoftware/Brave-Browser/Crash Reports",
    "opera/Crash Reports",
    "vivaldi/Crash Reports",
    "tracker3/files",
    "gnome-software",
    "flatpak",
    "snapcraft",
    "appstream",
    "pip/http",
    "uv/cache",
    "poetry/cache",
    "conda/pkgs",
    "docker/buildkit",
    "sccache",
];

pub fn cache_targets() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Some(cache) = crate::platform::cache_dir() else {
        return out;
    };
    for leaf in SAFE_CACHE_LEAVES {
        let dir = cache.join(leaf);
        if dir.is_dir() {
            out.push(dir);
        }
    }
    if let Some(home) = crate::platform::home_dir() {
        out.extend(flatpak_app_caches(&home));
    }
    out
}

/// Flatpak uygulamalarının ayrı önbellekleri (`~/.var/app/<id>/cache`).
///
/// `flatpak uninstall --unused` yalnızca kullanılmayan runtime'ları kaldırır;
/// kurulu uygulamaların şişen önbellekleri burada yakalanır. Yalnızca
/// `cache` yaprağı hedeflenir, uygulama verisine dokunulmaz.
pub fn flatpak_app_caches(home: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let apps = home.join(".var/app");
    let Ok(entries) = std::fs::read_dir(&apps) else {
        return out;
    };
    for entry in entries.flatten() {
        let cache = entry.path().join("cache");
        if cache.is_dir() {
            out.push(cache);
        }
    }
    out
}

/// Önizleme: hedef başına boyut.
pub fn preview_items(lang: &crate::i18n::Lang) -> Vec<crate::deep::PreviewItem> {
    cache_targets()
        .into_iter()
        .map(|dir| {
            let bytes = crate::fsutil::size::dir_size(&dir);
            crate::deep::PreviewItem::new(
                crate::i18n::et(lang, "app cache {}", &[&dir.display().to_string()]),
                Some(dir),
                bytes,
            )
        })
        .collect()
}

/// Ayrı seçenekle temizlenen geliştirici aracı önbelleği.
#[derive(Debug, Clone, Copy)]
pub enum DevTool {
    Npm,
    Pip,
    Cargo,
    Gradle,
    Go,
}

impl DevTool {
    pub fn label(self) -> &'static str {
        match self {
            DevTool::Npm => "npm",
            DevTool::Pip => "pip",
            DevTool::Cargo => "cargo",
            DevTool::Gradle => "gradle",
            DevTool::Go => "go",
        }
    }
    /// Ev dizinine göre hedefler.
    pub fn targets(self) -> Vec<PathBuf> {
        let Some(home) = crate::platform::home_dir() else {
            return Vec::new();
        };
        let cache = crate::platform::cache_dir().unwrap_or_else(|| home.join(".cache"));
        // (ev-dizini-göreli mi, önbellek-dizini-göreli mi)
        let rel: &[(&str, bool)] = match self {
            DevTool::Npm => &[(".npm", true)],
            DevTool::Pip => &[("pip", false)],
            DevTool::Cargo => &[(".cargo/registry", true), (".cargo/git", true)],
            DevTool::Gradle => &[(".gradle/caches", true), (".gradle/wrapper", true)],
            DevTool::Go => &[("go-build", false), ("go/pkg/mod", true)],
        };
        rel.iter()
            .map(|(l, from_home)| {
                if *from_home {
                    home.join(l)
                } else {
                    cache.join(l)
                }
            })
            .filter(|p| p.is_dir())
            .collect()
    }
}

pub fn execute_dev(
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
    tool: DevTool,
) {
    let items: Vec<crate::deep::PreviewItem> = tool
        .targets()
        .into_iter()
        .map(|dir| {
            crate::deep::PreviewItem::new(
                crate::i18n::et(
                    &ctx.lang,
                    "{} cache {}",
                    &[&tool.label().to_string(), &dir.display().to_string()],
                ),
                Some(dir.clone()),
                crate::fsutil::size::dir_size(&dir),
            )
        })
        .collect();
    if ctx.dry_run {
        if items.is_empty() {
            report.push(crate::core::report::Entry::new(
                crate::core::report::EntryKind::Command,
                cleaner,
                option,
                crate::i18n::et(&ctx.lang, "no {} cache", &[&tool.label().to_string()]),
                None,
                0,
            ));
            return;
        }
        crate::deep::push_preview(report, cleaner, option, items);
        return;
    }
    for item in &items {
        if let Some(path) = &item.path {
            crate::deep::clean_dir_contents(path, ctx, cleaner, option, report, 16);
            crate::deep::safety::log_operation(ctx, cleaner, option, path, item.bytes, false);
        }
    }
}

pub fn execute(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    let items = preview_items(&ctx.lang);
    if ctx.dry_run {
        crate::deep::push_preview(report, cleaner, option, items);
        return;
    }
    for item in &items {
        if let Some(path) = &item.path {
            crate::deep::clean_dir_contents(path, ctx, cleaner, option, report, 16);
            crate::deep::safety::log_operation(ctx, cleaner, option, path, item.bytes, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heavy_hitters_are_covered() {
        // GB'larca şişen gerçek dünya önbellekleri listede olmalı.
        for leaf in [
            "go-build",
            "composer/cache",
            "deno",
            "paru",
            "yay",
            "JetBrains",
            "Electron",
            "ms-playwright",
            "BraveSoftware/Brave-Browser/Crash Reports",
        ] {
            assert!(SAFE_CACHE_LEAVES.contains(&leaf), "missing leaf: {leaf}");
        }
        assert_eq!(DevTool::Go.label(), "go");
    }

    #[test]
    fn finds_flatpak_app_caches() {
        // Sahte ev dizini: yalnızca `<uygulama>/cache` yaprakları bulunur.
        let base = std::env::temp_dir().join(format!(
            "sweep-test-flatpak-{}-{}",
            std::process::id(),
            "appcaches"
        ));
        let _ = std::fs::remove_dir_all(&base);
        let good = base.join(".var/app/org.example.App/cache");
        let data = base.join(".var/app/org.example.App/data");
        std::fs::create_dir_all(&good).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        let mut found = flatpak_app_caches(&base);
        found.sort();
        assert_eq!(found, vec![good]);
        assert!(flatpak_app_caches(&base.join("yok")).is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }
}
