//! System-level operations: clipboard, DNS, trash, memory, free-space wiping,
//! package managers, journald, and the Windows-specific cleaners.
//!
//! These actions operate on the machine rather than on a single declared path,
//! so most of them ignore `def.path` and consult the platform layer instead.

use std::path::PathBuf;

use crate::action::context::RunContext;
use crate::action::filter::ActionFilter;
use crate::core::error::Result;
use crate::core::report::{Entry, EntryKind, Report};
use crate::definition::model::{ActionDef, VarSet};
use crate::fsutil::delete::{delete, DeleteOptions};
use crate::fsutil::freespace::{wipe_free_space, NullWipeProgress, WipeConfig};
use crate::fsutil::walk::{scan_paths, ScanOptions};

/// Dispatch a system / process / registry action.
pub fn run(
    def: &ActionDef,
    vars: &VarSet,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
) -> Report {
    let mut report = Report::new();

    match def.command.as_str() {
        "system.clipboard" => match crate::platform::clear_clipboard() {
            Ok(()) => report.push(special(cleaner, option, "Clear clipboard")),
            Err(err) => report.fail(cleaner, option, err.to_string()),
        },
        "system.dns" => match crate::platform::flush_dns() {
            Ok(()) => report.push(special(cleaner, option, "Flush DNS cache")),
            Err(err) => report.fail(cleaner, option, err.to_string()),
        },
        "system.memory" => match wipe_memory() {
            Ok(()) => report.push(special(cleaner, option, "Wipe freed memory")),
            Err(err) => report.fail(cleaner, option, err.to_string()),
        },
        "system.trash" => clean_trash(ctx, cleaner, option, &mut report),
        "system.rotated_logs" => clean_simple_list(
            crate::platform::rotated_logs(),
            ctx,
            cleaner,
            option,
            "rotated log",
            &mut report,
        ),
        "system.localizations" => clean_simple_list(
            crate::platform::localization_paths(&ctx.keep_localizations),
            ctx,
            cleaner,
            option,
            "unused locale",
            &mut report,
        ),
        "system.recent_documents" => clean_simple_list(
            recent_documents(),
            ctx,
            cleaner,
            option,
            "recent document",
            &mut report,
        ),
        "system.custom" => clean_custom(ctx, cleaner, option, &mut report),
        "system.tmp" => clean_tmp(def, ctx, cleaner, option, &mut report),
        "system.free_disk_space" => clean_free_space(def, vars, ctx, cleaner, option, &mut report),
        "system.trim" => clean_trim(def, vars, ctx, cleaner, option, &mut report),
        "system.journald" => clean_journald(ctx, cleaner, option, &mut report),
        // ---- Linux deep-clean categories (src/deep/*) ---------------------
        "system.apt_cache" => crate::deep::apt::execute(ctx, cleaner, option, &mut report, false),
        "system.dnf_cache" => crate::deep::dnf::execute(ctx, cleaner, option, &mut report),
        "system.pacman_cache" => crate::deep::pacman::execute(
            ctx,
            cleaner,
            option,
            &mut report,
            ctx.deep_options.pacman_keep,
        ),
        "system.old_kernels" => crate::deep::kernel::execute(ctx, cleaner, option, &mut report),
        "system.journal_vacuum" => crate::deep::journal::execute(
            ctx,
            cleaner,
            option,
            &mut report,
            &ctx.deep_options.journal_size.clone(),
            ctx.deep_options.journal_days,
        ),
        "system.user_caches" => crate::deep::caches::execute(ctx, cleaner, option, &mut report),
        "system.dev_npm" => crate::deep::caches::execute_dev(
            ctx,
            cleaner,
            option,
            &mut report,
            crate::deep::caches::DevTool::Npm,
        ),
        "system.dev_pip" => crate::deep::caches::execute_dev(
            ctx,
            cleaner,
            option,
            &mut report,
            crate::deep::caches::DevTool::Pip,
        ),
        "system.dev_cargo" => crate::deep::caches::execute_dev(
            ctx,
            cleaner,
            option,
            &mut report,
            crate::deep::caches::DevTool::Cargo,
        ),
        "system.dev_gradle" => crate::deep::caches::execute_dev(
            ctx,
            cleaner,
            option,
            &mut report,
            crate::deep::caches::DevTool::Gradle,
        ),
        "system.dev_go" => crate::deep::caches::execute_dev(
            ctx,
            cleaner,
            option,
            &mut report,
            crate::deep::caches::DevTool::Go,
        ),
        "system.temp_deep" => crate::deep::temp::execute(
            ctx,
            cleaner,
            option,
            &mut report,
            ctx.deep_options.temp_max_age_days,
        ),
        "system.nix_gc" => crate::deep::nix::execute(ctx, cleaner, option, &mut report),
        "system.tm_thin" => {
            crate::deep::timemachine::execute(ctx, cleaner, option, &mut report)
        }
        "system.docker_volumes" => {
            crate::deep::docker::execute_volumes(ctx, cleaner, option, &mut report)
        }
        "system.journal_user" => crate::deep::journal::execute_user(
            ctx,
            cleaner,
            option,
            &mut report,
            &ctx.deep_options.journal_size.clone(),
            ctx.deep_options.journal_days,
        ),
        "system.orphans" => crate::deep::orphans::execute(ctx, cleaner, option, &mut report),
        "system.snap" => crate::deep::snap::execute(ctx, cleaner, option, &mut report),
        "system.flatpak" => crate::deep::flatpak::execute(ctx, cleaner, option, &mut report),
        "system.apt_lists" => crate::deep::apt::execute_lists(ctx, cleaner, option, &mut report),
        "system.zypp_cache" => crate::deep::zypp::execute(ctx, cleaner, option, &mut report),
        "system.crash_reports" => crate::deep::crash::execute(
            ctx,
            cleaner,
            option,
            &mut report,
            crate::deep::crash::Scope::Dumps,
        ),
        "system.coredumps" => crate::deep::crash::execute(
            ctx,
            cleaner,
            option,
            &mut report,
            crate::deep::crash::Scope::Coredumps,
        ),
        "system.docker" => crate::deep::docker::execute(
            ctx,
            cleaner,
            option,
            &mut report,
            ctx.deep_options.docker_build_cache,
        ),
        "system.kde_cache" => crate::deep::kde::execute_cache(
            ctx,
            cleaner,
            option,
            &mut report,
            crate::deep::kde::Scope::Plasma,
        ),
        "system.kde_pim" => crate::deep::kde::execute_cache(
            ctx,
            cleaner,
            option,
            &mut report,
            crate::deep::kde::Scope::Pim,
        ),
        "system.baloo" => crate::deep::kde::execute_baloo(ctx, cleaner, option, &mut report),
        "system.gnome_cache" => crate::deep::gnome::execute_cache(
            ctx,
            cleaner,
            option,
            &mut report,
            crate::deep::gnome::Scope::Apps,
        ),
        "system.zeitgeist" => crate::deep::gnome::execute_cache(
            ctx,
            cleaner,
            option,
            &mut report,
            crate::deep::gnome::Scope::Zeitgeist,
        ),
        "system.tracker" => crate::deep::gnome::execute_tracker(ctx, cleaner, option, &mut report),
        "system.font_cache" => clean_font_cache(ctx, cleaner, option, &mut report),
        "system.recycle_bin" => clean_recycle_bin(cleaner, option, &mut report),
        "system.prefetch" => clean_from_dirs(prefetch_dirs(), ctx, cleaner, option, &mut report),
        "system.wer" => clean_from_dirs(wer_dirs(), ctx, cleaner, option, &mut report),
        "system.windows_update_cache" => {
            clean_from_dirs(wu_dirs(), ctx, cleaner, option, &mut report)
        }
        "system.delivery_optimization" => {
            clean_from_dirs(do_dirs(), ctx, cleaner, option, &mut report)
        }
        "system.explorer_caches" => {
            clean_from_dirs(explorer_caches(), ctx, cleaner, option, &mut report)
        }
        "system.thumbcache" => clean_simple_list(
            thumbcache_files(),
            ctx,
            cleaner,
            option,
            "thumbnail cache",
            &mut report,
        ),
        "system.store_cache" => clean_from_dirs(store_cache_dirs(), ctx, cleaner, option, &mut report),
        "system.d3d_shader_cache" => {
            clean_from_dirs(d3d_cache_dirs(), ctx, cleaner, option, &mut report)
        }
        "system.crash_dumps" => clean_from_dirs(crash_dump_dirs(), ctx, cleaner, option, &mut report),
        "system.memory_dumps" => clean_simple_list(
            memory_dumps(),
            ctx,
            cleaner,
            option,
            "memory dump",
            &mut report,
        ),
        "process" => run_process(def, ctx, cleaner, option, &mut report),
        "winreg" => run_winreg(def, vars, ctx, cleaner, option, &mut report),
        "apt.clean" => run_pkg("apt-get", &["clean"], ctx, cleaner, option, &mut report),
        "apt.autoclean" => run_pkg(
            "apt-get",
            &["autoclean", "-y"],
            ctx,
            cleaner,
            option,
            &mut report,
        ),
        "apt.autoremove" => run_pkg(
            "apt-get",
            &["autoremove", "-y"],
            ctx,
            cleaner,
            option,
            &mut report,
        ),
        "dnf.clean" => run_pkg("dnf", &["clean", "all"], ctx, cleaner, option, &mut report),
        "yum.clean" => run_pkg("yum", &["clean", "all"], ctx, cleaner, option, &mut report),
        "pacman.clean" => run_pkg(
            "pacman",
            &["-Scc", "--noconfirm"],
            ctx,
            cleaner,
            option,
            &mut report,
        ),
        "zypper.clean" => run_pkg(
            "zypper",
            &["clean", "--all"],
            ctx,
            cleaner,
            option,
            &mut report,
        ),
        "brew.cleanup" => run_pkg(
            "brew",
            &["cleanup", "--prune=all"],
            ctx,
            cleaner,
            option,
            &mut report,
        ),
        other => report.fail(
            cleaner,
            option,
            format!("unhandled system command '{other}'"),
        ),
    }

    report
}

fn special(cleaner: &str, option: &str, label: &str) -> Entry {
    Entry::new(EntryKind::Command, cleaner, option, label, None, 0)
}

/// Rebuild the font cache.
fn clean_trash(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    #[cfg(windows)]
    {
        // Best effort first; per-directory cleanup below always runs.
        let _ = crate::platform::windows::empty_recycle_bin();
    }
    for dir in crate::platform::trash_dirs() {
        delete_tree(&dir, ctx, cleaner, option, report);
    }
    if !ctx.dry_run {
        // touch the shell so it notices the bin is now empty
        crate::platform::shell_refresh();
    }
}

/// Delete each path in `paths` (file or the contents of a directory).
fn clean_simple_list(
    paths: Vec<PathBuf>,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    label: &str,
    report: &mut Report,
) {
    for path in paths {
        if path.is_dir() {
            delete_tree(&path, ctx, cleaner, option, report);
        } else if path.exists() {
            remove_one(&path, ctx, cleaner, option, label, report);
        }
    }
}

/// Like [`clean_simple_list`] but every entry is a directory.
fn clean_from_dirs(
    dirs: Vec<PathBuf>,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
) {
    for dir in dirs {
        delete_tree(&dir, ctx, cleaner, option, report);
    }
}

/// Delete the user-configured custom paths.
fn clean_custom(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if ctx.custom_paths.is_empty() {
        report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            "No custom paths configured",
            None,
            0,
        ));
        return;
    }
    for path in &ctx.custom_paths {
        if path.is_dir() {
            delete_tree(path, ctx, cleaner, option, report);
        } else if path.exists() {
            remove_one(path, ctx, cleaner, option, "custom path", report);
        }
    }
}

/// Delete stale files from the system temporary directory.
fn clean_tmp(def: &ActionDef, ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    let tmp = std::env::temp_dir();
    if !tmp.is_dir() {
        return;
    }
    // Default to files older than one day, matching BleachBit.
    let mut def = def.clone();
    if def.max_age_days.is_none() {
        def.max_age_days = Some(1);
    }
    let filter = match ActionFilter::compile(&def) {
        Ok(f) => f,
        Err(err) => {
            report.fail(cleaner, option, err.to_string());
            return;
        }
    };

    let options = ScanOptions::files().with_max_depth(2);
    for path in scan_paths(&tmp, &options) {
        if ctx.cancelled() {
            report.aborted = true;
            return;
        }
        if !filter.accepts(&path) {
            continue;
        }
        remove_one(&path, ctx, cleaner, option, "temp file", report);
    }
}

/// Overwrite free space on the volume containing `def.path`.
fn clean_free_space(
    def: &ActionDef,
    vars: &VarSet,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
) {
    let roots = crate::definition::model::expand_action_paths(def, vars);
    let Some(root) = roots.first() else {
        report.fail(cleaner, option, "free_disk_space needs a path");
        return;
    };
    if ctx.dry_run {
        report.push(Entry::new(
            EntryKind::WipeFreeSpace,
            cleaner,
            option,
            format!("would wipe free space on {}", root.display()),
            Some(root.as_path()),
            0,
        ));
        return;
    }
    let progress = NullWipeProgress;
    match wipe_free_space(root, WipeConfig::default(), &progress, &ctx.cancel) {
        Ok(wiped) => report.push(Entry::new(
            EntryKind::WipeFreeSpace,
            cleaner,
            option,
            format!("wiped free space on {}", root.display()),
            Some(root.as_path()),
            wiped,
        )),
        Err(err) => report.fail(cleaner, option, err.to_string()),
    }
}

/// Issue TRIM / discard for the volume containing `def.path`.
fn clean_trim(
    def: &ActionDef,
    vars: &VarSet,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
) {
    let roots = crate::definition::model::expand_action_paths(def, vars);
    let Some(root) = roots.first() else {
        return;
    };
    if ctx.dry_run {
        report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            format!("would TRIM {}", root.display()),
            Some(root.as_path()),
            0,
        ));
        return;
    }
    match crate::platform::trim(root) {
        Ok(()) => report.push(special(cleaner, option, "TRIM volume")),
        Err(err) => report.fail(cleaner, option, err.to_string()),
    }
}

/// Vacuum the systemd journal.
fn clean_journald(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    if ctx.dry_run {
        report.push(special(cleaner, option, "Vacuum systemd journal"));
        return;
    }
    #[cfg(target_os = "linux")]
    {
        match crate::platform::linux::journald_clean(ctx.deep_options.journal_days) {
            Ok(freed) => report.push(Entry::new(
                EntryKind::Vacuum,
                cleaner,
                option,
                "Vacuum systemd journal",
                None,
                freed,
            )),
            Err(err) => report.fail(cleaner, option, err.to_string()),
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        report.fail(cleaner, option, "journald cleaning is Linux-only");
    }
}

/// Rebuild the font cache.
fn clean_font_cache(ctx: &RunContext, cleaner: &str, option: &str, report: &mut Report) {
    for dir in font_cache_dirs() {
        delete_tree(&dir, ctx, cleaner, option, report);
    }
    #[cfg(target_os = "linux")]
    {
        let _ = crate::fsutil::run_command("fc-cache", &["-f"], true);
    }
    report.push(special(cleaner, option, "Rebuild font cache"));
}

/// Empty the Windows recycle bin on every drive.
fn clean_recycle_bin(cleaner: &str, option: &str, report: &mut Report) {
    #[cfg(windows)]
    {
        match crate::platform::windows::empty_recycle_bin() {
            Ok(()) => report.push(special(cleaner, option, "Empty recycle bin")),
            Err(err) => report.fail(cleaner, option, err.to_string()),
        }
    }
    #[cfg(not(windows))]
    {
        report.fail(cleaner, option, "recycle bin is Windows-only");
    }
}

/// Run an external command (`process` action).
fn run_process(
    def: &ActionDef,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
) {
    let Some(cmd) = def.cmd.clone() else {
        report.fail(cleaner, option, "process action is missing a 'cmd'");
        return;
    };
    run_shell(&cmd, def.wait, ctx, cleaner, option, report);
}

/// Run a package-manager command, synthesising the `cmd` string.
fn run_pkg(
    program: &str,
    args: &[&str],
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
) {
    let cmd = format!("{} {}", program, args.join(" "));
    run_shell(&cmd, true, ctx, cleaner, option, report);
}

/// Actually spawn `cmd` (used by both `process` and package managers).
fn run_shell(
    cmd: &str,
    wait: bool,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
) {
    let mut parts = shell_words::split(cmd)
        .unwrap_or_else(|_| cmd.split_whitespace().map(String::from).collect());
    if parts.is_empty() {
        report.fail(cleaner, option, "process action has an empty command");
        return;
    }
    let program = parts.remove(0);
    let args: Vec<&str> = parts.iter().map(String::as_str).collect();

    if ctx.dry_run {
        report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            format!("would run: {cmd}"),
            None,
            0,
        ));
        return;
    }

    match crate::fsutil::run_command(&program, &args, wait) {
        Ok((0, _, _)) => report.push(Entry::new(
            EntryKind::Command,
            cleaner,
            option,
            format!("ran: {cmd}"),
            None,
            0,
        )),
        Ok((status, _, stderr)) => report.fail(
            cleaner,
            option,
            format!("command '{cmd}' exited with {status}: {stderr}"),
        ),
        Err(err) => report.fail(cleaner, option, format!("cannot run '{cmd}': {err}")),
    }
}

/// Edit (or preview editing of) the Windows registry.
fn run_winreg(
    def: &ActionDef,
    vars: &VarSet,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
) {
    let _ = &ctx;
    let roots = crate::definition::model::expand_action_paths(def, vars);
    let Some(key) = roots.first() else {
        report.fail(cleaner, option, "winreg action needs a key path");
        return;
    };

    #[cfg(windows)]
    {
        let really = !ctx.dry_run;
        if let Some(name) = &def.reg_name {
            match crate::platform::windows::reg_delete_value(&key.to_string_lossy(), name, really) {
                Ok(true) => report.push(Entry::new(
                    EntryKind::Registry,
                    cleaner,
                    option,
                    format!("delete {}\\{name}", key.display()),
                    None,
                    0,
                )),
                Ok(false) => {}
                Err(err) => report.fail(cleaner, option, err.to_string()),
            }
        } else {
            match crate::platform::windows::reg_delete_key(
                &key.to_string_lossy(),
                &def.exclude_keys,
                really,
            ) {
                Ok(true) => report.push(Entry::new(
                    EntryKind::Registry,
                    cleaner,
                    option,
                    format!("delete key {}", key.display()),
                    None,
                    0,
                )),
                Ok(false) => {}
                Err(err) => report.fail(cleaner, option, err.to_string()),
            }
        }
    }
    #[cfg(not(windows))]
    {
        report.fail(
            cleaner,
            option,
            format!("registry editing is Windows-only (key: {})", key.display()),
        );
    }
}

// --- helpers ----------------------------------------------------------------

/// Remove a single file, honouring the guard and the dry-run flag.
fn remove_one(
    path: &std::path::Path,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    label: &str,
    report: &mut Report,
) {
    // Keep list, protected roots and links all refuse deletion here.
    if ctx.guard.check(path, false).is_err() {
        report.push(Entry::skipped(cleaner, option, path));
        return;
    }
    // Son güvenlik doğrulaması (`guarded_delete` ile aynı katman); bağ adı
    // silmelerinde Guard kararı yeterlidir.
    if !ctx.dry_run && !path.is_symlink() {
        if let Err(reason) = crate::deep::safety::check_path(path, ctx) {
            log::warn!("skipped {}: {reason}", path.display());
            report.push(Entry::skipped(cleaner, option, path));
            return;
        }
    }
    let reclaimed = if path.is_dir() {
        crate::fsutil::size::dir_size(path)
    } else {
        crate::fsutil::size::size_of(path).unwrap_or(0)
    };
    if ctx.dry_run {
        report.push(Entry::new(
            EntryKind::Delete,
            cleaner,
            option,
            format!("{label} {}", path.display()),
            Some(path),
            reclaimed,
        ));
        return;
    }
    // Yedek + denetim kaydı yalnızca gerçek dosyalarda (dizinler geri
    // yüklemede yeniden açılır, bağ adları yedeksizdir).
    let auditable = !ctx.dry_run && path.is_file() && !path.is_symlink();
    if auditable {
        crate::deep::safety::maybe_backup(ctx, path);
    }
    match delete(path, DeleteOptions::simple()) {
        Ok(true) => {
            if auditable {
                crate::deep::safety::log_operation(ctx, cleaner, option, path, reclaimed, false);
            }
            report.push(Entry::new(
                EntryKind::Delete,
                cleaner,
                option,
                format!("{label} {}", path.display()),
                Some(path),
                reclaimed,
            ))
        }
        Ok(false) => {}
        Err(err) if err.is_benign() => {}
        Err(err) => report.fail(cleaner, option, err.to_string()),
    }
}

/// Recursively delete the *contents* of `root` (never `root` itself).
fn delete_tree(
    root: &std::path::Path,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
) {
    if !root.is_dir() {
        return;
    }
    let options = ScanOptions::all()
        .with_max_depth(32)
        .with_same_filesystem(true);
    for path in scan_paths(root, &options) {
        if ctx.cancelled() {
            report.aborted = true;
            return;
        }
        remove_one(&path, ctx, cleaner, option, "tree", report);
    }
}

// --- platform glue that lives closer to the OS layer ------------------------

fn recent_documents() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(home) = crate::platform::home_dir() {
        out.push(home.join(".recently-used.xbel"));
        out.push(home.join(".local").join("share").join("recently-used.xbel"));
    }
    #[cfg(windows)]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            out.push(PathBuf::from(appdata).join("Microsoft\\Windows\\Recent"));
        }
    }
    out
}

fn font_cache_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(cache) = crate::platform::cache_dir() {
        out.push(cache.join("fontconfig"));
    }
    if let Some(home) = crate::platform::home_dir() {
        out.push(home.join(".fontconfig"));
    }
    out
}

fn prefetch_dirs() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        crate::platform::windows::prefetch_dir()
            .into_iter()
            .collect()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn wer_dirs() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        crate::platform::windows::wer_dirs()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn wu_dirs() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        crate::platform::windows::windows_update_cache()
            .into_iter()
            .collect()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn do_dirs() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        crate::platform::windows::delivery_optimization_cache()
            .into_iter()
            .collect()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn explorer_caches() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        crate::platform::windows::explorer_caches()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn thumbcache_files() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        crate::platform::windows::explorer_thumb_files()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn store_cache_dirs() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        crate::platform::windows::store_cache_dirs()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn d3d_cache_dirs() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        crate::platform::windows::d3d_shader_cache_dirs()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn crash_dump_dirs() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        crate::platform::windows::crash_dump_dirs()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

fn memory_dumps() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        crate::platform::windows::memory_dumps()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[cfg(target_os = "linux")]
fn wipe_memory() -> Result<()> {
    crate::platform::linux::wipe_memory()
}

#[cfg(not(target_os = "linux"))]
fn wipe_memory() -> Result<()> {
    Err(crate::core::error::Error::msg(
        "memory wiping is only supported on Linux",
    ))
}
