//! Ağ temizliği: DNS çözümleyici önbelleği ve paket yöneticisi önbellekleri.
//!
//! `sweep network flush` yalnız DNS önbelleğini boşaltır (geri dönüşsüz bir
//! şey silmez — çözümleyici bir sonraki sorguda yeniden doldurur).
//! `sweep network clean` paket yöneticisi indirme önbelleklerini, git
//! depolarındaki `git gc` ve docker ağ kalıntılarını temizler.
//!
//! Platform ayrımı `crate::platform::flush_dns()` içinde yapılır:
//! `resolvectl flush-caches` / `systemd-resolve --flush-caches` (Linux),
//! `ipconfig /flushdns` (Windows), `dscacheutil -flushcache` +
//! `killall -HUP mDNSResponder` (macOS).
//!
//! Her adım `--dry-run` destekler: hiçbir komut çalıştırılmaz, yalnız ne
//! yapılacağı ve tahmini kazanç raporlanır.

use std::path::PathBuf;

use crate::core::report::{Entry, EntryKind, Report};

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "network";

/// `git gc` taramasının ineceği en fazla derinlik (ev dizini geniş olabilir).
const GIT_MAX_DEPTH: usize = 3;
/// Bir çalıştırmada bakılacak en fazla depo sayısı (tarama maliyeti sınırlı).
const GIT_MAX_REPOS: usize = 25;

/// Ağ temizliği seçenekleri.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NetworkOptions {
    /// Hiçbir şey çalıştırma; yalnız planı ve tahmini kazancı raporla.
    pub dry_run: bool,
}

/// Tek bir paket yöneticisi önbelleği.
#[derive(Debug, Clone)]
pub struct ToolCache {
    /// Rapor satırlarında görünen ad.
    name: &'static str,
    /// Çalıştırılacak program.
    program: &'static str,
    /// Programa verilecek argümanlar.
    args: &'static [&'static str],
    /// Kazanç ölçümü için bakılacak önbellek dizinleri.
    dirs: Vec<PathBuf>,
}

impl ToolCache {
    /// Bu araç kurulu mu?
    fn available(&self) -> bool {
        crate::deep::have(self.program)
    }

    /// Önbellek dizinlerinin toplam baytı.
    fn measured_bytes(&self) -> u64 {
        self.dirs
            .iter()
            .map(|dir| crate::deep::dir_stats(dir).0)
            .sum()
    }

    /// İnsan okunur komut satırı.
    fn command_line(&self) -> String {
        if self.args.is_empty() {
            self.program.to_string()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

/// Bilinen paket yöneticisi önbellekleri (kurulu olanlar çalıştırılır).
///
/// Saf fonksiyon: yalnız ev dizinine göre yolları üretir, hiçbir şey çalıştırmaz.
pub fn tool_caches() -> Vec<ToolCache> {
    let home = crate::platform::home_dir();
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let mut tools = Vec::new();

    let dirs_for = |unix: &[&str], win: &[&str]| -> Vec<PathBuf> {
        let mut out = Vec::new();
        if let Some(home) = &home {
            for rel in unix {
                out.push(home.join(rel));
            }
        }
        if let Some(local) = &local {
            for rel in win {
                out.push(local.join(rel));
            }
        }
        out
    };

    tools.push(ToolCache {
        name: "npm",
        program: "npm",
        args: &["cache", "clean", "--force"],
        dirs: dirs_for(&[".npm/_cacache"], &["npm-cache", "npm"]),
    });
    tools.push(ToolCache {
        name: "pip",
        program: "pip",
        args: &["cache", "purge"],
        dirs: dirs_for(&[".cache/pip"], &["pip/Cache"]),
    });
    tools.push(ToolCache {
        name: "yarn",
        program: "yarn",
        args: &["cache", "clean"],
        dirs: dirs_for(&[".cache/yarn"], &["Yarn/Cache"]),
    });
    tools.push(ToolCache {
        name: "cargo",
        // `cargo`'nun kendi "registry cache temizle" alt komutu yoktur;
        // indirilmiş .crate arşivleri `~/.cargo/registry/cache` altında birikir
        // ve bir sonraki derlemede yeniden indirilir. Onları biz boşaltırız.
        program: "cargo",
        args: &[],
        dirs: dirs_for(&[".cargo/registry/cache"], &[]),
    });
    tools.push(ToolCache {
        name: "gem",
        program: "gem",
        args: &["cleanup"],
        dirs: Vec::new(),
    });

    tools
}

/// Paket yöneticisi önbelleklerini temizle.
fn clean_package_caches(opts: &NetworkOptions, report: &mut Report) {
    for tool in tool_caches() {
        if !tool.available() {
            continue;
        }
        let before = tool.measured_bytes();

        if opts.dry_run {
            let estimate = if tool.name == "cargo" { before } else { 0 };
            let label = if estimate > 0 {
                format!(
                    "would run: {} (up to ~{} cached)",
                    tool.command_line(),
                    crate::deep::safety::human(estimate)
                )
            } else {
                format!("would run: {}", tool.command_line())
            };
            push(report, "cache", label, 0);
            continue;
        }

        // cargo: programı çalıştırmak yerine indirme önbelleğini boşaltırız.
        if tool.name == "cargo" {
            match clear_dir_contents(&tool.dirs) {
                Ok(removed) => push(
                    report,
                    "cache",
                    format!(
                        "cargo registry cache cleared (~{} freed)",
                        crate::deep::safety::human(removed)
                    ),
                    removed,
                ),
                Err(err) => report.fail(CLEANER_ID, "cache", format!("cargo: {err}")),
            }
            continue;
        }

        match crate::fsutil::run_command(tool.program, tool.args, true) {
            Ok((0, _, _)) => {
                let freed = before.saturating_sub(tool.measured_bytes());
                push(
                    report,
                    "cache",
                    format!(
                        "{} cache cleaned (~{} freed)",
                        tool.name,
                        crate::deep::safety::human(freed)
                    ),
                    freed,
                );
            }
            Ok((code, _, err)) => report.fail(
                CLEANER_ID,
                "cache",
                format!("{} exited {code}: {}", tool.name, err.trim()),
            ),
            Err(err) => report.fail(CLEANER_ID, "cache", format!("{}: {err}", tool.name)),
        }
    }
}

/// Verilen dizinlerin *içini* boşalt (dizinler kalır), silinen baytı döndür.
fn clear_dir_contents(dirs: &[PathBuf]) -> Result<u64, String> {
    let mut removed = 0u64;
    for dir in dirs {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let bytes = if path.is_dir() {
                crate::deep::dir_stats(&path).0
            } else {
                crate::fsutil::size::size_of_or_zero(&path)
            };
            let result = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
            };
            match result {
                Ok(()) => removed += bytes,
                Err(err) => return Err(format!("{}: {err}", path.display())),
            }
        }
    }
    Ok(removed)
}

/// Ev dizini altında `.git` klasörü olan depoları bul (derinlik ve sayı sınırlı).
///
/// Saf gezinme: hiçbir şey silmez, hiçbir komut çalıştırmaz.
pub fn find_git_repos(root: &std::path::Path, max_depth: usize, limit: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut queue = vec![(root.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = queue.pop() {
        if out.len() >= limit || depth > max_depth {
            continue;
        }
        if dir.join(".git").exists() {
            out.push(dir);
            continue; // iç içe depo aramaya gerek yok
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            // Gizli dizinleri ve bilinen gürültülü ağaçları atla.
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.') || name == "node_modules" || name == "target" {
                continue;
            }
            queue.push((path, depth + 1));
        }
    }
    out.sort();
    out.truncate(limit);
    out
}

/// Her git deposunda `git gc --auto` çalıştır.
fn clean_git(opts: &NetworkOptions, report: &mut Report) {
    if !crate::deep::have("git") {
        return;
    }
    let Some(home) = crate::platform::home_dir() else {
        return;
    };
    let repos = find_git_repos(&home, GIT_MAX_DEPTH, GIT_MAX_REPOS);
    if repos.is_empty() {
        return;
    }
    if opts.dry_run {
        push(
            report,
            "git",
            format!("would run: git gc --auto in {} repositories", repos.len()),
            0,
        );
        return;
    }
    let mut ok = 0usize;
    for repo in &repos {
        match crate::fsutil::run_command_in("git", &["gc", "--auto"], repo, true) {
            Ok((0, _, _)) => ok += 1,
            // gc hatası ölümcül değildir: depo bozuk olabilir, devam ederiz.
            Ok((_, _, _)) | Err(_) => {}
        }
    }
    push(
        report,
        "git",
        format!("git gc --auto finished in {ok}/{} repositories", repos.len()),
        0,
    );
}

/// Docker ağ kalıntılarını temizle (`docker network prune -f`).
fn clean_docker(opts: &NetworkOptions, report: &mut Report) {
    if !crate::deep::have("docker") {
        return;
    }
    if opts.dry_run {
        push(report, "docker", "would run: docker network prune -f".to_string(), 0);
        return;
    }
    match crate::fsutil::run_command("docker", &["network", "prune", "-f"], true) {
        Ok((0, out, _)) => {
            let reclaimed = parse_docker_reclaimed(&out);
            push(
                report,
                "docker",
                format!(
                    "docker networks pruned (~{} freed)",
                    crate::deep::safety::human(reclaimed)
                ),
                reclaimed,
            );
        }
        Ok((code, _, err)) => report.fail(
            CLEANER_ID,
            "docker",
            format!("docker network prune exited {code}: {}", err.trim()),
        ),
        Err(err) => report.fail(CLEANER_ID, "docker", format!("docker: {err}")),
    }
}

/// `docker ... prune` çıktısındaki "Total reclaimed space: 12.3MB" satırını çöz.
pub fn parse_docker_reclaimed(text: &str) -> u64 {
    for line in text.lines() {
        let lower = line.to_lowercase();
        if !lower.contains("reclaimed") {
            continue;
        }
        let Some(colon) = line.rfind(':') else {
            continue;
        };
        let value = line[colon + 1..].trim();
        if let Ok(bytes) = crate::fsutil::size::human_to_bytes(value) {
            return bytes;
        }
    }
    0
}

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

/// DNS önbelleğini boşalt.
///
/// `dry_run` seçeneği burada anlamsızdır: boşaltma geri dönüşsüz bir şey
/// silmez, yalnız önbelleği düşürür — bu yüzden her zaman çalıştırılır.
pub fn flush_dns(dry_run: bool) -> Report {
    let mut report = Report::new();
    if dry_run {
        push(
            &mut report,
            "dns",
            "would flush the DNS resolver cache".to_string(),
            0,
        );
        return report;
    }
    match crate::platform::flush_dns() {
        Ok(()) => push(
            &mut report,
            "dns",
            "DNS resolver cache flushed".to_string(),
            0,
        ),
        Err(err) => report.fail(CLEANER_ID, "dns", err.to_string()),
    }
    report
}

/// Paket önbellekleri + git + docker temizliği.
pub fn clean(opts: &NetworkOptions) -> Report {
    let mut report = Report::new();
    clean_package_caches(opts, &mut report);
    clean_git(opts, &mut report);
    clean_docker(opts, &mut report);
    if report.entries.is_empty() && report.errors.is_empty() {
        push(
            &mut report,
            "cache",
            "no package manager caches found".to_string(),
            0,
        );
    }
    report
}

/// `sweep network <op>` girişi. `op`: flush | clean.
pub fn run(op: &str, opts: &NetworkOptions) -> Report {
    match op {
        "flush" | "dns" => flush_dns(opts.dry_run),
        "clean" => clean(opts),
        other => {
            let mut report = Report::new();
            report.fail(
                CLEANER_ID,
                "args",
                crate::i18n::et(
                    &crate::i18n::Lang::detect(&[]),
                    "unknown op: {} (flush/clean)",
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
    fn tool_caches_cover_the_documented_managers() {
        let tools = tool_caches();
        let names: Vec<&str> = tools.iter().map(|t| t.name).collect();
        for expected in ["npm", "pip", "yarn", "cargo", "gem"] {
            assert!(names.contains(&expected), "missing {expected}");
        }
        // npm/yarn/gem gerçek bir temizleme komutu taşır; cargo boş (dizin bazlı).
        let npm = tools.iter().find(|t| t.name == "npm").unwrap();
        assert_eq!(npm.program, "npm");
        assert!(npm.args.contains(&"--force"));
        let cargo = tools.iter().find(|t| t.name == "cargo").unwrap();
        assert!(cargo.args.is_empty());
        assert!(cargo.dirs.iter().any(|d| d.ends_with("cache")));
    }

    #[test]
    fn command_lines_are_readable() {
        let tools = tool_caches();
        let pip = tools.iter().find(|t| t.name == "pip").unwrap();
        assert_eq!(pip.command_line(), "pip cache purge");
    }

    #[test]
    fn dry_run_never_touches_anything() {
        let opts = NetworkOptions { dry_run: true };
        let report = clean(&opts);
        assert!(report.errors.is_empty(), "errors: {:?}", report.errors);
        assert!(!report.entries.is_empty());
        // Dry-run hiçbir kazanç iddia etmez.
        assert_eq!(report.reclaimed(), 0);
    }

    #[test]
    fn dns_dry_run_reports_without_running() {
        let report = flush_dns(true);
        assert!(report.errors.is_empty());
        assert_eq!(report.entries.len(), 1);
        assert!(report.entries[0].label.contains("would flush"));
        assert_eq!(report.reclaimed(), 0);
    }

    #[test]
    fn unknown_op_fails_cleanly() {
        let report = run("nope", &NetworkOptions::default());
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].message.contains("unknown op"));
    }

    #[test]
    fn git_repo_discovery_finds_a_fixture_tree() {
        let root = std::env::temp_dir().join("sweep_net_git_fixture");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("alpha").join(".git")).unwrap();
        std::fs::create_dir_all(root.join("beta").join("nested").join(".git")).unwrap();
        // node_modules ve gizli dizinler taranmamalı.
        std::fs::create_dir_all(root.join("node_modules").join("pkg").join(".git")).unwrap();
        std::fs::create_dir_all(root.join(".hidden").join(".git")).unwrap();

        let repos = find_git_repos(&root, 4, 25);
        assert_eq!(repos.len(), 2, "repos: {repos:?}");
        assert!(repos.iter().any(|p| p.ends_with("alpha")));
        assert!(repos.iter().any(|p| p.ends_with("nested")));

        // limit uygulanır
        assert_eq!(find_git_repos(&root, 4, 1).len(), 1);
        // derinlik sınırı: nested depo 2. seviyede, max_depth=1 onu bulamaz
        let shallow = find_git_repos(&root, 1, 25);
        assert_eq!(shallow.len(), 1);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn docker_reclaimed_parsing() {
        // Docker SI birimleri kullanır: 1.5MB = 1_500_000 bayt.
        assert_eq!(
            parse_docker_reclaimed("Deleted Networks:\nfoo\n\nTotal reclaimed space: 1.5MB"),
            1_500_000
        );
        assert_eq!(parse_docker_reclaimed("nothing here"), 0);
    }

    #[test]
    fn clear_dir_contents_removes_files_but_keeps_the_dir() {
        let root = std::env::temp_dir().join("sweep_net_clear_fixture");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("a.txt"), vec![0u8; 100]).unwrap();
        std::fs::write(root.join("sub").join("b.txt"), vec![0u8; 50]).unwrap();

        let freed = clear_dir_contents(&[root.clone()]).unwrap();
        assert_eq!(freed, 150);
        assert!(root.is_dir(), "the cache dir itself must survive");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);

        let _ = std::fs::remove_dir_all(&root);
    }
}
