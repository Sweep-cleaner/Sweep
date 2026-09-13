//! Geliştirici artığı taraması: yeniden üretilebilir derleme/önbellek dizinleri.
//!
//! `node_modules`, Rust `target/`, `__pycache__`, `.venv`, `.next` … hepsi
//! bir komutla yeniden kurulur, o yüzden silmek güvenlidir. Jenerik
//! isimler (`build`, `dist`, `target`, `venv`) yalnızca yanında proje
//! bildirimi (`package.json`, `Cargo.toml` …) varsa eşleşir; böylece
//! `~/build` gibi kişisel dizinler asla hedef olmaz.

use std::path::{Path, PathBuf};

use rayon::prelude::*;

use crate::action::context::RunContext;
use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "devscan";

/// (dizin adı, geri yükleme ipucu, bildirim gerektirir mi).
pub const CRUFT: &[(&str, &str, bool)] = &[
    ("node_modules", "npm ci / yarn install", false),
    ("__pycache__", "auto-regenerated", false),
    (".next", "next build", false),
    (".nuxt", "nuxt build", false),
    (".turbo", "turbo run", false),
    (".parcel-cache", "parcel", false),
    (".vite", "vite build", false),
    (".svelte-kit", "svelte-kit sync", false),
    (".angular", "ng build", false),
    (".expo", "expo start", false),
    (".mypy_cache", "mypy", false),
    (".pytest_cache", "pytest", false),
    (".ruff_cache", "ruff", false),
    (".tox", "tox", false),
    (".nox", "nox", false),
    (".gradle", "gradle", false),
    (".dart_tool", "dart pub get", false),
    ("_build", "mix compile", false),
    ("DerivedData", "rebuilt by Xcode", false),
    // Jenerikler: bildirim şart.
    ("target", "cargo build", true),
    ("build", "project build", true),
    ("dist", "project build", true),
    ("out", "IDE rebuild", true),
    ("logs", "regenerated logs", true),
    ("crash-reports", "diagnostic dumps", true),
    ("venv", "recreate venv", true),
    (".venv", "recreate venv", true),
    ("Pods", "pod install", true),
];

/// Jenerik isimler için kabul edilen proje bildirimleri.
const MANIFESTS: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "setup.py",
    "setup.cfg",
    "requirements.txt",
    "go.mod",
    "Gemfile",
    "pubspec.yaml",
    "mix.exs",
    "Podfile",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "CMakeLists.txt",
    "Makefile",
    "composer.json",
    "Cargo.lock",
    "package-lock.json",
];

fn cruft_info(name: &str) -> Option<(&'static str, bool)> {
    CRUFT
        .iter()
        .find(|(n, _, _)| *n == name)
        .map(|(_, hint, needs_manifest)| (*hint, *needs_manifest))
}

/// Üst dizinde proje bildirimi var mı?
fn manifest_ok(dir: &Path) -> bool {
    dir.parent()
        .map(|parent| MANIFESTS.iter().any(|m| parent.join(m).is_file()))
        .unwrap_or(false)
}

/// Tek bulgu.
#[derive(Debug, Clone)]
pub struct CruftHit {
    pub path: PathBuf,
    pub kind: String,
    pub hint: String,
    pub bytes: u64,
    pub files: usize,
}

/// Varsayılan kökler: çalışma dizini + yaygın proje klasörleri.
pub fn default_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }
    if let Some(home) = crate::platform::home_dir() {
        for leaf in [
            "projects",
            "Projects",
            "workspace",
            "src",
            "code",
            "dev",
            "repos",
            "Developer",
        ] {
            let p = home.join(leaf);
            if p.is_dir() {
                roots.push(p);
            }
        }
    }
    roots.sort();
    roots.dedup();
    roots
}

/// Köklerde artık dizinleri bul (silmez; boyutlarıyla listeler).
/// `min_size` altındaki cılız isabetler elenir.
pub fn find_hits(roots: &[PathBuf], max_depth: usize, min_size: u64) -> Vec<CruftHit> {
    let mut hits: Vec<CruftHit> = roots
        .par_iter()
        .flat_map(|root| {
            if !root.is_dir() {
                return Vec::new();
            }
            let options = crate::fsutil::walk::ScanOptions::all().with_max_depth(max_depth);
            crate::fsutil::walk::scan_paths(root, &options)
                .into_iter()
                .filter(|p| p.is_dir())
                .filter_map(|path| {
                    let name = path.file_name()?.to_string_lossy().into_owned();
                    let (hint, needs_manifest) = cruft_info(&name)?;
                    if needs_manifest && !manifest_ok(&path) {
                        return None;
                    }
                    let (bytes, files) = crate::deep::dir_stats(&path);
                    Some(CruftHit {
                        path,
                        kind: name,
                        hint: hint.to_string(),
                        bytes,
                        files,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect();

    // İç içe isabetler (örn. üstte target, altta build): üstü tut.
    hits.sort_by(|a, b| a.path.cmp(&b.path));
    let mut merged: Vec<CruftHit> = Vec::with_capacity(hits.len());
    for hit in hits {
        let nested = merged
            .iter()
            .any(|top: &CruftHit| hit.path != top.path && hit.path.starts_with(&top.path));
        if !nested {
            merged.push(hit);
        }
    }
    // Büyük önce.
    merged.sort_by_key(|h| std::cmp::Reverse(h.bytes));
    merged.into_iter().filter(|h| h.bytes >= min_size).collect()
}

/// Tara ve raporla; `ctx.dry_run == false` ise korumalı sil.
pub fn scan(roots: &[PathBuf], max_depth: usize, min_size: u64, ctx: &RunContext) -> Report {
    let mut report = Report::new();
    let hits = find_hits(roots, max_depth, min_size);

    if hits.is_empty() {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "cruft",
            crate::i18n::t(&ctx.lang, "no dev cruft found"),
            None,
            0,
        ));
        return report;
    }

    for hit in &hits {
        if ctx.dry_run {
            report.push(Entry::new(
                EntryKind::Delete,
                CLEANER_ID,
                &hit.kind,
                crate::i18n::et(
                    &ctx.lang,
                    "{} ({} files, {})",
                    &[
                        &hit.path.display().to_string(),
                        &hit.files.to_string(),
                        &hit.hint,
                    ],
                ),
                Some(&hit.path),
                hit.bytes,
            ));
        } else {
            crate::deep::remove_tree(&hit.path, ctx, CLEANER_ID, &hit.kind, &mut report);
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cruft_table_sane() {
        assert!(cruft_info("node_modules").is_some());
        assert!(cruft_info("target").map(|(_, m)| m).unwrap_or(false));
        assert!(!cruft_info("node_modules").map(|(_, m)| m).unwrap_or(true));
        assert!(cruft_info("Documents").is_none());
        assert!(cruft_info("out").map(|(_, m)| m).unwrap_or(false));
        assert!(cruft_info("logs").map(|(_, m)| m).unwrap_or(false));
        assert!(cruft_info("crash-reports").map(|(_, m)| m).unwrap_or(false));
    }

    #[test]
    fn generic_needs_manifest() {
        let dir = std::env::temp_dir().join("sweep_devscan_test");
        let _ = std::fs::create_dir_all(dir.join("build"));
        // Bildirim yok -> eşleşmemeli.
        assert!(!manifest_ok(&dir.join("build")));
        std::fs::write(dir.join("package.json"), "{}").unwrap();
        assert!(manifest_ok(&dir.join("build")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
