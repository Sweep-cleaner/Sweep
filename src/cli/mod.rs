//! Command-line interface.
//!
//! `sweep` exposes the whole library through a small clap-driven front-end.
//! Every destructive sub-command (`clean`, `deepscan`, `wipe`) first runs in
//! preview form internally so the user can see what *would* happen; the actual
//! deletion only happens when the sub-command is `clean`/`wipe` and `--force`
//! is not required for the keep list.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};

use sweep::action::RunContext;
use sweep::config::Config;
use sweep::core::keep::Guard;
use sweep::core::report::Report;
use sweep::definition::{CleanerRegistry, LoadOptions};
use sweep::engine::deepscan;
use sweep::engine::progress::{CliProgress, NoProgress, SharedProgress};
use sweep::engine::worker::{resolve, run};
use sweep::platform;

mod gen;

/// Sweep — fast, safe, cross-platform system cleaner.
#[derive(Parser)]
#[command(
    name = "sweep",
    version,
    about = "Fast, safe system cleaner (BleachBit-inspired, written in Rust)",
    long_about = None,
)]
pub struct Cli {
    /// Path to the configuration file.
    #[arg(long, global = true)]
    config: Option<PathBuf>,

    /// Overwrite files before deleting (secure deletion), overriding the config.
    #[arg(long, global = true)]
    shred: bool,

    /// Allow cleaning protected system paths and the keep list (expert only).
    #[arg(long, global = true)]
    force: bool,

    /// Treat cleaners loaded from disk as trusted (lets them run processes / edit the registry).
    #[arg(long, global = true)]
    trust_external: bool,

    /// Also load BleachBit's CleanerML definitions from their standard locations.
    #[arg(long, global = true)]
    bleachbit: bool,

    /// Also load and translate winapp2.ini.
    #[arg(long, global = true)]
    winapp2: bool,

    /// Extra directory of cleaner definitions to load.
    #[arg(long = "define", global = true)]
    defines: Vec<PathBuf>,

    /// Print machine-readable JSON instead of a human summary.
    #[arg(long, global = true)]
    json: bool,

    /// Aggregate the report: exact per-option totals plus a bounded entry sample.
    #[arg(long, global = true)]
    summary: bool,

    /// Do not print progress bars.
    #[arg(long, global = true)]
    quiet: bool,

    /// Enable debug logging.
    #[arg(long, global = true)]
    debug: bool,

    /// Skip the pre-clean confirmation prompt (required for non-interactive runs).
    #[arg(long, global = true)]
    yes: bool,

    /// Copy files here (mirroring layout) before deleting them.
    #[arg(long, global = true)]
    backup_dir: Option<PathBuf>,

    /// Append every operation to this log file.
    #[arg(long, global = true)]
    log_file: Option<PathBuf>,

    /// Portable mode: keep config, log and hub under DIR (USB-friendly).
    #[arg(long, global = true)]
    portable: Option<PathBuf>,

    /// Also write a standalone HTML report to this file.
    #[arg(long, global = true)]
    html: Option<PathBuf>,

    /// Also write a Markdown report to this file (CI / issue friendly).
    #[arg(long, global = true)]
    markdown: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

/// Top-level sub-command.
#[derive(Subcommand)]
enum Command {
    /// List cleaners and their options.
    List {
        /// Optional cleaner id to show options for.
        cleaner: Option<String>,
        /// Only cleaners whose id, name or description contains this text.
        #[arg(long)]
        search: Option<String>,
        /// Only cleaners that (also) apply to this OS (linux, macos, windows…).
        #[arg(long)]
        os: Option<String>,
        /// Only cleaners whose target application is currently running.
        #[arg(long)]
        running: bool,
    },
    /// Show everything Sweep knows about one cleaner (definition detail).
    Info {
        /// Cleaner id (case-insensitive).
        cleaner: String,
    },
    /// Preview what would be cleaned (no changes are made).
    Preview {
        /// `cleaner`, `cleaner.option`, a glob, or `*` for everything.
        selectors: Vec<String>,
        /// Clean every active option of every applicable cleaner.
        #[arg(long)]
        all: bool,
        /// Exclude these `cleaner.option` selectors.
        #[arg(long)]
        exclude: Vec<String>,
        /// Reuse the selection remembered by the last `--remember` run.
        #[arg(long)]
        last: bool,
        /// Remember this selection for a later `--last` run.
        #[arg(long)]
        remember: bool,
    },
    /// Clean (actually deletes files).
    Clean {
        /// `cleaner`, `cleaner.option`, a glob, or `*` for everything.
        selectors: Vec<String>,
        /// Clean every active option of every applicable cleaner.
        #[arg(long)]
        all: bool,
        /// Exclude these `cleaner.option` selectors.
        #[arg(long)]
        exclude: Vec<String>,
        /// Reuse the selection remembered by the last `--remember` run.
        #[arg(long)]
        last: bool,
        /// Remember this selection for a later `--last` run.
        #[arg(long)]
        remember: bool,
    },
    /// Deep-scan directories for junk by name pattern.
    Deepscan {
        /// Root directories to scan (defaults to the configured roots).
        roots: Vec<PathBuf>,
        /// Extra glob patterns (repeatable).
        #[arg(long)]
        pattern: Vec<String>,
        /// Just list matches without deleting.
        #[arg(long)]
        preview: bool,
    },
    /// Find leftover files of an (uninstalled) app by name.
    Leftovers {
        /// Application name to hunt for (case-insensitive, min 2 chars).
        name: String,
        /// Actually delete the leftovers (default only lists them).
        #[arg(long)]
        clean: bool,
        /// Only report hits at least this big (e.g. 10MB).
        #[arg(long)]
        min_size: Option<String>,
    },
    /// Smart residue scan: uninstall leftovers, dead build output, version
    /// caches and empty dirs with confidence scores (default lists them).
    Residue {
        #[command(subcommand)]
        action: Option<ResidueAction>,
        /// Roots for build/empty scans (default: home directory).
        #[arg(long)]
        roots: Vec<PathBuf>,
        /// Only delete findings at or above this confidence (0.0-1.0,
        /// default 0.6; below stays preview-only).
        #[arg(long)]
        confidence: Option<f32>,
        /// Keep empty directories even when cleaning.
        #[arg(long)]
        keep_empty_dirs: bool,
    },
    /// Find regenerable developer cruft (node_modules, target, __pycache__...).
    Devscan {
        /// Roots to scan (defaults to cwd + ~/projects, ~/src, ...).
        roots: Vec<PathBuf>,
        /// Actually delete the cruft (default only lists it).
        #[arg(long)]
        clean: bool,
        /// Max recursion depth (default 6).
        #[arg(long)]
        max_depth: Option<usize>,
        /// Only report cruft at least this big (e.g. 50MB).
        #[arg(long)]
        min_size: Option<String>,
    },
    /// Find the largest files above a size threshold, biggest first.
    Bigfiles {
        /// Roots to scan (defaults to home directory).
        roots: Vec<PathBuf>,
        /// Actually delete the files (default only lists them).
        #[arg(long)]
        clean: bool,
        /// Minimum file size (default 100MB, e.g. 500MB).
        #[arg(long)]
        min_size: Option<String>,
        /// Only files older than this (e.g. 90d, 12w, 6m, 1y).
        #[arg(long)]
        older_than: Option<String>,
        /// Show only the top N files.
        #[arg(long)]
        top: Option<usize>,
        /// Max recursion depth (default 12).
        #[arg(long)]
        max_depth: Option<usize>,
    },
    /// Measure a folder's immediate children (recursive sizes), biggest first.
    Du {
        /// Folder to measure.
        path: PathBuf,
        /// Show only the top N entries.
        #[arg(long)]
        top: Option<usize>,
    },
    /// Find duplicate files by size + content hash.
    Dupes {
        /// Roots to scan (defaults to Pictures, Documents, Downloads...).
        roots: Vec<PathBuf>,
        /// Actually delete duplicates, keeping one copy per group.
        #[arg(long)]
        clean: bool,
        /// Minimum file size (default 1MB, e.g. 100KB).
        #[arg(long)]
        min_size: Option<String>,
        /// Max recursion depth (default 12).
        #[arg(long)]
        max_depth: Option<usize>,
        /// Which copy to keep: first|newest|oldest|largest|smallest.
        #[arg(long, default_value = "first")]
        keep: String,
        /// Replace duplicates with hard links to the kept copy instead of
        /// deleting them (same content, one inode; must share a filesystem).
        #[arg(long)]
        link: bool,
    },
    /// Overwrite free space on the volume containing a path.
    Wipe {
        /// Path on the volume to wipe.
        path: PathBuf,
    },
    /// Manage the scheduled daily cleanup (systemd / Task Scheduler / launchd).
    Schedule {
        /// Install and enable the daily timer.
        #[arg(long)]
        enable: bool,
        /// Stop and remove the timer.
        #[arg(long)]
        disable: bool,
        /// Hour of day for the run (0-23, default 3).
        #[arg(long, default_value_t = 3)]
        hour: u32,
        /// Cleaners to run (default: journal + package caches).
        selection: Vec<String>,
        /// Manage the daily memory-trim task instead (safe memopt, no admin).
        #[arg(long)]
        memopt: bool,
    },
    /// Restore files deleted with --backup-dir, using the operation log.
    Undo {
        /// Operation log to replay (defaults to the standard log).
        #[arg(long)]
        log_file: Option<PathBuf>,
        /// Backup root used during cleaning.
        #[arg(long)]
        backup_dir: Option<PathBuf>,
        /// Only the most recent cleaning run (run marker, else whole log).
        #[arg(long)]
        last: bool,
        /// Only the cleaning run with this id (`undo --list` shows ids).
        #[arg(long, conflicts_with = "last")]
        run: Option<String>,
        /// List cleaning runs (newest first) instead of restoring.
        #[arg(long, conflicts_with = "last", conflicts_with = "run")]
        list: bool,
    },
    /// Download and validate the community winapp2.ini rules.
    UpdateWinapp2 {
        /// Custom source URL.
        #[arg(long)]
        url: Option<String>,
    },
    /// Manage community cleaner rules (list/install/remove/update).
    Hub {
        /// list, install, remove or update.
        op: String,
        /// Rule id (for install/remove).
        target: Option<String>,
        /// Custom index URL.
        #[arg(long)]
        url: Option<String>,
    },
    /// Securely shred files (overwrite then delete).
    Shred {
        /// Files to shred.
        paths: Vec<PathBuf>,
    },
    /// Analyze disk usage (read-only directory + file ranking).
    Analyze {
        /// Root to analyze (defaults to home).
        root: Option<PathBuf>,
        /// How many entries per section (default 15).
        #[arg(long, default_value_t = 15)]
        top: usize,
        /// Max recursion depth for the file ranking (default 8).
        #[arg(long)]
        max_depth: Option<usize>,
    },
    /// Manage everything that starts automatically (autostart entries + logon/boot tasks).
    #[command(after_help = "Lookup order for <id|name>: exact id, then exact name, then a unique \
name substring. An ambiguous substring is an error — use an id from `sweep startup list --json`.")]
    Startup {
        /// list, enable, disable, remove, edit, rollback, history or export.
        op: String,
        /// Entry id or name (id is matched first; see the note below).
        target: Option<String>,
        /// Also estimate each entry's startup impact (high/medium/low).
        #[arg(long)]
        impact: bool,
        /// Only entries of this kind.
        #[arg(long)]
        kind: Option<String>,
        /// Only user or system entries.
        #[arg(long)]
        scope: Option<String>,
        /// Only this category (os, security, driver, updater, third-party, unknown).
        #[arg(long)]
        category: Option<String>,
        /// Only this risk level (critical, high, medium, low).
        #[arg(long)]
        risk: Option<String>,
        /// Only this source (registry-run, scheduled-task, …).
        #[arg(long)]
        source: Option<String>,
        /// Only disabled entries.
        #[arg(long)]
        disabled: bool,
        /// Only enabled entries.
        #[arg(long)]
        enabled: bool,
        /// Substring search over name, command, location and source.
        #[arg(long)]
        search: Option<String>,
        /// Sort order: name, impact or source.
        #[arg(long, default_value = "name")]
        sort: String,
        /// New command for `edit`.
        #[arg(long)]
        command: Option<String>,
        /// Roll back the most recent change (instead of a specific entry).
        #[arg(long)]
        last: bool,
        /// Show what would happen without changing anything.
        #[arg(long)]
        dry_run: bool,
        /// Max history lines (0 = all).
        #[arg(long, default_value_t = 50)]
        limit: usize,
        /// Destination file for `export`.
        #[arg(long)]
        to: Option<PathBuf>,
        /// Export format: json or csv (inferred from the extension when omitted).
        #[arg(long)]
        format: Option<String>,
    },
    /// Manage Windows Task Scheduler tasks (and Unix cron/systemd timers).
    #[command(after_help = "Lookup order for <name|id>: exact id, then exact name, then a unique \
name substring. Protected \\Microsoft\\ tasks need --force and are always recorded.")]
    Tasks {
        /// list, enable, disable, rollback, history or export.
        op: String,
        /// Task id or name (id is matched first).
        target: Option<String>,
        /// Only enabled tasks.
        #[arg(long)]
        enabled: bool,
        /// Only disabled tasks.
        #[arg(long)]
        disabled: bool,
        /// Only tasks with this trigger (logon, boot, schedule, event, idle, unknown).
        #[arg(long)]
        trigger: Option<String>,
        /// Substring search over name, command and location.
        #[arg(long)]
        search: Option<String>,
        /// Sort order: name, impact or source.
        #[arg(long, default_value = "name")]
        sort: String,
        /// Roll back the most recent change (instead of a specific task).
        #[arg(long)]
        last: bool,
        /// Show what would happen without changing anything.
        #[arg(long)]
        dry_run: bool,
        /// Max history lines (0 = all).
        #[arg(long, default_value_t = 50)]
        limit: usize,
        /// Destination file for `export`.
        #[arg(long)]
        to: Option<PathBuf>,
        /// Export format: json or csv (inferred from the extension when omitted).
        #[arg(long)]
        format: Option<String>,
    },
    /// Manage systemd services.
    Services {
        /// list, start, stop, restart, enable or disable.
        op: String,
        /// Unit name (for control operations).
        unit: Option<String>,
        /// Manage user services instead of system ones.
        #[arg(long)]
        user: bool,
    },
    /// Diagnose config, tools, permissions, disk and systemd.
    Doctor,
    /// Optimize memory: drop unused page cache (Linux), trim working sets
    /// (Windows), purge inactive memory (macOS).
    Memopt {
        /// Also clear the deeper layers (swap on Linux, standby list on
        /// Windows). Needs root/administrator.
        #[arg(long)]
        aggressive: bool,
        /// Estimate the effect without changing anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Report system information and health (CPU, memory, disks, SMART).
    Sysinfo {
        /// Only print the green/yellow/red summary with its reasons.
        #[arg(long)]
        health: bool,
    },
    /// Network cache maintenance: DNS resolver cache and package caches.
    Network {
        /// flush (DNS resolver cache) or clean (package/git/docker caches).
        op: String,
        /// Show what would happen without running anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Privacy shield: disable telemetry, wipe recent files and the clipboard.
    Privacy {
        /// shield (disable telemetry) or wipe (recent files + clipboard).
        op: String,
        /// Show what would happen without changing anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// Manage filesystem snapshots (snapper/timeshift).
    Snapshot {
        /// create, list, delete or rollback.
        op: String,
        /// Description (create) or snapshot id (delete/rollback).
        target: Option<String>,
    },
    /// Show cleaning history from the operation log.
    History {
        /// Operation log (defaults to the standard log).
        #[arg(long)]
        log_file: Option<PathBuf>,
        /// Only this many past days (0 = all time).
        #[arg(long, default_value_t = 30)]
        days: u32,
    },
    /// Watch disk usage and notify when a threshold is crossed.
    Watch {
        /// Roots to watch (defaults to / and home).
        paths: Vec<PathBuf>,
        /// Usage percent that triggers an alarm (default 90).
        #[arg(long, default_value_t = 90)]
        threshold: u8,
        /// Seconds between checks (ignored with --once).
        #[arg(long, default_value_t = 300)]
        interval: u64,
        /// Check once and exit (code 3 = alarm fired).
        #[arg(long)]
        once: bool,
        /// Include this many biggest files in the alarm (0 = off).
        #[arg(long, default_value_t = 5)]
        suggest: usize,
    },
    /// List every action command Sweep understands.
    Providers,
    /// Print platform diagnostics.
    Diagnostics,
    /// Validate a cleaner definition file or directory without running it.
    Import {
        /// File or directory to validate.
        path: PathBuf,
    },
    /// Inspect and edit the configuration file (show/path/init/get/set).
    Config {
        /// `show`, `path`, `init`, `get KEY` or `set KEY VALUE`.
        op: String,
        /// Setting name (for get/set).
        key: Option<String>,
        /// New value (for set).
        value: Option<String>,
    },
    /// Generate a shell completion script (bash, zsh or fish).
    Completions {
        /// Target shell: bash, zsh or fish.
        shell: String,
    },
    /// Generate a roff man page (for packaging).
    Man {
        /// Write sweep.1 into this directory instead of stdout.
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

/// Output verbosity / format used when printing a [`Report`].
#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Human,
    Json,
    /// Human output without the per-path list: totals plus the per-option
    /// breakdown. Used by `--summary` for very large selections.
    Summary,
    /// JSON with exact totals plus `by_option` aggregates and only a sampled
    /// `entries` array — the form the GUI consumes.
    JsonSummary,
}

impl Format {
    /// Does this flavour print JSON (full or summarised)?
    fn is_json(self) -> bool {
        matches!(self, Format::Json | Format::JsonSummary)
    }
}

/// `sweep residue` verbs (default when omitted: `scan`).
#[derive(Clone, Copy, Subcommand)]
enum ResidueAction {
    /// List every residue finding with confidence and size.
    Scan,
    /// Interactively pick findings, then delete the selection.
    Preview,
    /// Delete findings at or above `--confidence` (default 0.6).
    Clean,
}

pub fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    if cli.debug {
        std::env::set_var("RUST_LOG", "debug");
    }
    if let Some(base) = &cli.portable {
        // Diğer tüm yol kararlarından ÖNCE ayarlanmalı.
        std::env::set_var("SWEEP_PORTABLE", base);
    }
    let _ = env_logger::try_init();

    let config_path = cli.config.clone().unwrap_or_else(Config::default_path);
    let config = Config::load(&config_path).unwrap_or_else(|err| {
        log::warn!("could not load config {}: {err}", config_path.display());
        Config::default()
    });

    let rt = match execute(&cli, &config) {
        Ok(code) => code,
        Err(err) => {
            if cli.json {
                println!("{}", serde_json::json!({ "error": err.to_string() }));
            } else {
                eprintln!("error: {err}");
            }
            std::process::ExitCode::FAILURE
        }
    };

    rt
}

/// Build the cleaner registry from the CLI's loading preferences.
fn build_registry(cli: &Cli) -> CleanerRegistry {
    let mut options = LoadOptions::builtin_only();
    if cli.bleachbit {
        options.bleachbit = true;
    }
    if cli.winapp2 {
        options.winapp2 = true;
    }
    if cli.trust_external {
        options = options.trusting_external();
    }
    for dir in &cli.defines {
        options = options.with_dir(dir.clone());
    }
    // Topluluk kuralları varsa her zaman yüklenir (güvensiz: process/winreg yok).
    if let Some(hub) = sweep::engine::hub::hub_dir() {
        if hub.is_dir() {
            options = options.with_dir(hub);
        }
    }
    CleanerRegistry::load(options)
}

/// Seçili seçeneklerin `warning` metinleri (TOML'daki uyarılar silmeden önce gösterilir).
fn collect_warnings(
    registry: &CleanerRegistry,
    selection: &[sweep::engine::worker::SelectedOption],
) -> Vec<String> {
    let mut out = Vec::new();
    for item in selection {
        let warning = registry
            .get(&item.cleaner)
            .and_then(|loaded| loaded.def.option(&item.option))
            .and_then(|opt| opt.warning.clone());
        if let Some(text) = warning {
            let line = format!("{}.{}: {text}", item.cleaner, item.option);
            if !out.contains(&line) {
                out.push(line);
            }
        }
    }
    out
}

/// Deep/System katmanı seçildiyse (`system` temizleyici ya da uyarı
/// rozetli seçenek) işlem öncesi sistem yedeği önerilir.
fn needs_backup_hint(
    registry: &CleanerRegistry,
    selection: &[sweep::engine::worker::SelectedOption],
) -> bool {
    selection.iter().any(|item| {
        item.cleaner == "system"
            || registry
                .get(&item.cleaner)
                .and_then(|loaded| loaded.def.option(&item.option))
                .and_then(|opt| opt.warning.clone())
                .is_some_and(|w| !w.is_empty())
    })
}

/// Uyarıları önizleme çıktısının üstüne yaz (JSON'da susturulur).
fn print_warnings(warnings: &[String], format: Format) {
    if warnings.is_empty() || format.is_json() {
        return;
    }
    println!("\n⚠ UYARILAR:");
    for w in warnings {
        println!("  ! {w}");
    }
}

/// Build the execution context from flags + config.
fn build_context(cli: &Cli, config: &Config, dry_run: bool, format: Format) -> RunContext {
    // `--force` deliberately does NOT reach the protected-root set: it only
    // relaxes the running-application refusal. A flag that could delete
    // `/etc` or `C:\Windows` would be a footgun, not a feature.
    let guard = Guard::new(config.keep_list()).with_builtins();
    // Progress is for humans; every JSON flavour (full or summary) stays clean.
    let progress: SharedProgress = if !format.is_json() && !cli.quiet && !dry_run {
        CliProgress::new(indicatif::ProgressBar::new_spinner())
    } else {
        Arc::new(NoProgress::new())
    };

    RunContext {
        guard,
        dry_run,
        global_shred: cli.shred || config.shred,
        shred_spec: sweep::shred::ShredSpec::default_spec(),
        cancel: Arc::new(AtomicBool::new(false)),
        progress,
        keep_localizations: config.languages.clone(),
        custom_paths: config.custom_paths(),
        deepscan_roots: config.deepscan_roots(),
        backup_dir: cli.backup_dir.clone().or_else(|| {
            let raw = config.backup_dir.trim();
            (!raw.is_empty()).then(|| PathBuf::from(raw))
        }),
        log_file: cli.log_file.clone().or_else(|| {
            let raw = config.log_file.trim();
            (!raw.is_empty()).then(|| PathBuf::from(raw))
        }),
        deep_options: sweep::deep::DeepOptions::from(config),
        lang: lang_of(config),
    }
}

fn execute(cli: &Cli, config: &Config) -> sweep::core::error::Result<std::process::ExitCode> {
    let format = match (cli.json, cli.summary) {
        (true, true) => Format::JsonSummary,
        (true, false) => Format::Json,
        (false, true) => Format::Summary,
        (false, false) => Format::Human,
    };

    match &cli.command {
        Command::List {
            cleaner,
            search,
            os,
            running,
        } => {
            let registry = build_registry(cli);
            print_list(
                &registry,
                cleaner,
                search.as_deref(),
                os.as_deref(),
                *running,
                format,
            );
            Ok(std::process::ExitCode::SUCCESS)
        }
        Command::Info { cleaner } => {
            let registry = build_registry(cli);
            print_info(&registry, cleaner, format)?;
            Ok(std::process::ExitCode::SUCCESS)
        }
        Command::Config { op, key, value } => {
            run_config(cli, config, op, key.as_deref(), value.as_deref(), format)
        }
        Command::Preview {
            selectors,
            all,
            exclude,
            last,
            remember,
        }
        | Command::Clean {
            selectors,
            all,
            exclude,
            last,
            remember,
        } => {
            let is_clean = matches!(cli.command, Command::Clean { .. });
            let registry = build_registry(cli);
            // `--last` replaces the positional selectors, it does not add to them.
            let selectors: Vec<String> = if *last {
                if config.last_selection.is_empty() {
                    return Err(sweep::core::error::Error::msg(
                        "no remembered selection: run a clean with --remember first",
                    ));
                }
                config.last_selection.clone()
            } else {
                selectors.clone()
            };
            if selectors.is_empty() && !*all {
                return Err(sweep::core::error::Error::msg(
                    "nothing selected (pass a cleaner, --all, or --last)",
                ));
            }
            let selection = resolve(&registry, &selectors, *all, exclude)?;
            let lang = lang_of(config);
            if is_clean && !format.is_json() && needs_backup_hint(&registry, &selection) {
                eprintln!(
                    "  ! {}",
                    sweep::i18n::t(
                        &lang,
                        "We recommend creating a system backup before continuing."
                    )
                );
            }
            if is_clean && !cli.yes {
                let preview_ctx = build_context(cli, config, true, format);
                let preview = run(&registry, &selection, &preview_ctx, cli.force);
                let warnings = collect_warnings(&registry, &selection);
                print_warnings(&warnings, format);
                print_report(&preview, format, false, &lang);
                if !confirm_clean(&preview, &lang, &warnings) {
                    println!("{}", sweep::i18n::t(&lang, "cancelled"));
                    return Ok(std::process::ExitCode::SUCCESS);
                }
            }
            let ctx = build_context(cli, config, !is_clean, format);
            let report = run(&registry, &selection, &ctx, cli.force);
            print_report(&report, format, is_clean, &lang);
            let title = if is_clean {
                "sweep clean"
            } else {
                "sweep preview"
            };
            write_html(cli, &report, title);
            write_md(cli, &report, title);
            remember_selection(cli, config, &selection, is_clean, *remember);
            Ok(exit_code(&report))
        }
        Command::Deepscan {
            roots,
            pattern,
            preview,
        } => {
            let ctx = build_context(cli, config, *preview, format);
            let roots = if roots.is_empty() {
                config.deepscan_roots()
            } else {
                roots.to_vec()
            };
            let mut patterns = config.deepscan_patterns.clone();
            patterns.extend(pattern.clone());
            let rules = deepscan::compile_rules(&patterns);
            if !preview {
                sweep::deep::safety::log_run_start(&ctx, "deepscan");
            }
            let report = deepscan::scan(&roots, &rules, &ctx);
            print_report(&report, format, !*preview, &lang_of(config));
            write_html(cli, &report, "sweep deepscan");
            write_md(cli, &report, "sweep deepscan");
            Ok(exit_code(&report))
        }
        Command::Leftovers {
            name,
            clean,
            min_size,
        } => {
            let floor = parse_size_flag(min_size.as_ref(), 0, "--min-size")?;
            run_preview_clean(cli, config, format, *clean, &[], "sweep leftovers", |ctx| {
                sweep::engine::leftovers::scan(name, floor, ctx)
            })
        }
        Command::Residue {
            action,
            roots,
            confidence,
            keep_empty_dirs,
        } => {
            let opts = sweep::engine::residue::ResidueOptions {
                roots: roots.clone(),
                // `confidence` is `&Option<f32>`; `Option<f32>` is `Copy`, so
                // dereferencing is enough — `Option::copied` does not exist.
                min_confidence: (*confidence)
                    .unwrap_or(sweep::engine::residue::DEFAULT_MIN_CONFIDENCE),
                keep_empty_dirs: *keep_empty_dirs,
                max_depth: 5,
            };
            match (*action).unwrap_or(ResidueAction::Scan) {
                ResidueAction::Scan => run_preview_clean(
                    cli,
                    config,
                    format,
                    false,
                    &[],
                    "sweep residue scan",
                    |ctx| sweep::engine::residue::scan(&opts, ctx),
                ),
                ResidueAction::Clean => run_preview_clean(
                    cli,
                    config,
                    format,
                    true,
                    &[],
                    "sweep residue clean",
                    |ctx| sweep::engine::residue::clean(&opts, ctx),
                ),
                ResidueAction::Preview => {
                    residue_preview(cli, config, format, &opts)
                }
            }
        }
        Command::Devscan {
            roots,
            clean,
            max_depth,
            min_size,
        } => {
            let roots = if roots.is_empty() {
                sweep::engine::devscan::default_roots()
            } else {
                roots.clone()
            };
            let depth = max_depth.unwrap_or(6);
            let floor = parse_size_flag(min_size.as_ref(), 0, "--min-size")?;
            run_preview_clean(cli, config, format, *clean, &[], "sweep devscan", |ctx| {
                sweep::engine::devscan::scan(&roots, depth, floor, ctx)
            })
        }
        Command::Bigfiles {
            roots,
            clean,
            min_size,
            older_than,
            top,
            max_depth,
        } => {
            let roots = if roots.is_empty() {
                let home = sweep::platform::home_dir()
                    .or_else(|| std::env::current_dir().ok())
                    .unwrap_or_else(|| PathBuf::from("."));
                vec![home]
            } else {
                roots.clone()
            };
            let floor = parse_size_flag(
                min_size.as_ref(),
                sweep::engine::bigfiles::DEFAULT_MIN_SIZE,
                "--min-size",
            )?;
            let age = match older_than {
                Some(text) => Some(sweep::engine::bigfiles::parse_age(text).ok_or_else(|| {
                    sweep::core::error::Error::msg(sweep::i18n::et(
                        &sweep::i18n::Lang::detect(&[]),
                        "invalid --older-than '{}' (e.g. 90d, 12w, 6m, 1y)",
                        &[&text],
                    ))
                })?),
                None => None,
            };
            let depth = max_depth.unwrap_or(12);
            run_preview_clean(cli, config, format, *clean, &[], "sweep bigfiles", |ctx| {
                sweep::engine::bigfiles::scan(&roots, depth, floor, age, *top, ctx)
            })
        }
        Command::Du { path, top } => {
            // Salt okunur: temizlik bayrağı yok, her zaman önizleme akışı.
            run_preview_clean(cli, config, format, false, &[], "sweep du", |ctx| {
                sweep::engine::du::scan(path, *top, ctx)
            })
        }
        Command::Dupes {
            roots,
            clean,
            min_size,
            max_depth,
            keep,
            link,
        } => {
            let roots = if roots.is_empty() {
                sweep::engine::dupes::default_roots()
            } else {
                roots.clone()
            };
            let floor = parse_size_flag(
                min_size.as_ref(),
                sweep::engine::dupes::DEFAULT_MIN_SIZE,
                "--min-size",
            )?;
            let depth = max_depth.unwrap_or(12);
            let keep = sweep::engine::dupes::KeepPolicy::parse(keep).map_err(|e| {
                sweep::core::error::Error::msg(e)
            })?;
            let link = *link;
            run_preview_clean(cli, config, format, *clean, &[], "sweep dupes", |ctx| {
                sweep::engine::dupes::scan(&roots, depth, floor, keep, link, ctx)
            })
        }
        Command::Schedule {
            enable,
            disable,
            hour,
            selection,
            memopt,
        } => {
            // Bayraksız çağrı durumu gösterir (GUI de bunu kullanır).
            if !enable && !disable {
                let status = if *memopt {
                    sweep::engine::schedule::memopt_status()
                } else {
                    sweep::engine::schedule::status()
                };
                if cli.json {
                    println!("{}", serde_json::json!({ "schedule": status }));
                } else {
                    println!("{status}");
                }
                return Ok(std::process::ExitCode::SUCCESS);
            }
            if *enable == *disable {
                return Err(sweep::core::error::Error::msg(
                    "pass one of --enable or --disable",
                ));
            }
            let lang = sweep::i18n::Lang::detect(&config.languages);
            let summary = if *enable {
                if *memopt {
                    sweep::engine::schedule::enable_memopt(*hour, &lang)?
                } else {
                    sweep::engine::schedule::enable(selection, *hour, &lang)?
                }
            } else if *memopt {
                sweep::engine::schedule::disable_memopt(&lang)?
            } else {
                sweep::engine::schedule::disable(&lang)?
            };
            if cli.json {
                println!("{}", serde_json::json!({ "schedule": summary }));
            } else {
                println!("{summary}");
                if *memopt {
                    println!("{}", sweep::engine::schedule::memopt_status());
                } else {
                    println!("{}", sweep::engine::schedule::status());
                }
            }
            Ok(std::process::ExitCode::SUCCESS)
        }
        Command::Undo {
            log_file,
            backup_dir,
            last,
            run,
            list,
        } => {
            let log = log_file
                .clone()
                .unwrap_or_else(sweep::deep::safety::default_log_file);
            let Some(backup) = backup_dir.clone().or_else(|| cli.backup_dir.clone()) else {
                return Err(sweep::core::error::Error::msg(sweep::i18n::t(
                    &sweep::i18n::Lang::detect(&config.languages),
                    "undo needs --backup-dir (the backup dir used during cleaning)",
                )));
            };
            if *list {
                let report = sweep::engine::undo::list_runs(&log, &backup);
                let lang = lang_of(config);
                print_report(&report, format, false, &lang);
                return Ok(exit_code(&report));
            }
            let scope = match run.clone() {
                Some(id) => sweep::engine::undo::Scope::Run(id),
                None if *last => sweep::engine::undo::Scope::Last,
                None => sweep::engine::undo::Scope::All,
            };
            let lang = lang_of(config);
            let ctx = build_context(cli, config, false, format);
            // Undo her zaman önce ne yapacağını gösterir.
            let preview_ctx = build_context(cli, config, true, format);
            let preview = sweep::engine::undo::scan(&log, &backup, &preview_ctx, &scope);
            print_report(&preview, format, false, &lang);
            if preview.entries.is_empty() {
                return Ok(exit_code(&preview));
            }
            if !cli.yes && !confirm_clean(&preview, &lang, &[]) {
                println!("{}", sweep::i18n::t(&lang, "restored_none"));
                return Ok(std::process::ExitCode::SUCCESS);
            }
            let report = sweep::engine::undo::scan(&log, &backup, &ctx, &scope);
            print_report(&report, format, true, &lang);
            write_html(cli, &report, "sweep undo");
            write_md(cli, &report, "sweep undo");
            Ok(exit_code(&report))
        }
        Command::UpdateWinapp2 { url } => {
            let source = url
                .clone()
                .unwrap_or_else(|| sweep::engine::winapp2upd::DEFAULT_URL.into());
            let ctx = build_context(cli, config, false, format);
            let report = sweep::engine::winapp2upd::scan(&source, &ctx);
            print_report(&report, format, true, &lang_of(config));
            write_html(cli, &report, "sweep update-winapp2");
            write_md(cli, &report, "sweep update-winapp2");
            Ok(exit_code(&report))
        }
        Command::Shred { paths } => {
            let lang = lang_of(config);
            let preview_ctx = build_context(cli, config, true, format);
            let preview = sweep::engine::shredrun::scan(paths, &preview_ctx);
            print_report(&preview, format, false, &lang);
            if preview.errors.is_empty() && !cli.yes && !confirm_clean(&preview, &lang, &[]) {
                println!("{}", sweep::i18n::t(&lang, "cancelled"));
                return Ok(std::process::ExitCode::SUCCESS);
            }
            if !preview.errors.is_empty() && !cli.yes {
                return Ok(exit_code(&preview));
            }
            let ctx = build_context(cli, config, false, format);
            let report = sweep::engine::shredrun::scan(paths, &ctx);
            print_report(&report, format, true, &lang);
            write_html(cli, &report, "sweep shred");
            write_md(cli, &report, "sweep shred");
            Ok(exit_code(&report))
        }
        Command::Analyze {
            root,
            top,
            max_depth,
        } => {
            let root = root.clone().unwrap_or_else(|| {
                sweep::platform::home_dir()
                    .or_else(|| std::env::current_dir().ok())
                    .unwrap_or_else(|| PathBuf::from("."))
            });
            let ctx = build_context(cli, config, true, format);
            let report = sweep::engine::analyze::scan(&root, *top, max_depth.unwrap_or(8), &ctx);
            print_report(&report, format, false, &lang_of(config));
            write_html(cli, &report, "sweep analyze");
            write_md(cli, &report, "sweep analyze");
            Ok(exit_code(&report))
        }
        Command::Startup {
            op,
            target,
            impact,
            kind,
            scope,
            category,
            risk,
            source,
            disabled,
            enabled,
            search,
            sort,
            command,
            last,
            dry_run,
            limit,
            to,
            format: export_format,
        } => {
            let watch = sweep::core::report::Stopwatch::start();
            let opts = sweep::engine::autostart::api::Options {
                op: op.clone(),
                target: target.clone(),
                command: command.clone(),
                last: *last,
                dry_run: *dry_run,
                force: cli.force,
                limit: *limit,
                to: to.clone(),
                format: export_format.clone(),
                kind: kind.clone(),
                scope: scope.clone(),
                category: category.clone(),
                risk: risk.clone(),
                source: source.clone(),
                trigger: None,
                enabled: *enabled,
                disabled: *disabled,
                search: search.clone(),
                sort: sort.clone(),
                impact: *impact,
                json: cli.json,
            };
            let (mut report, text) = sweep::engine::autostart::api::cli_startup(&opts);
            report.finish(watch);
            println!("{text}");
            write_html(cli, &report, "sweep startup");
            write_md(cli, &report, "sweep startup");
            Ok(exit_code(&report))
        }
        Command::Tasks {
            op,
            target,
            enabled,
            disabled,
            trigger,
            search,
            sort,
            last,
            dry_run,
            limit,
            to,
            format: export_format,
        } => {
            let watch = sweep::core::report::Stopwatch::start();
            let opts = sweep::engine::autostart::api::Options {
                op: op.clone(),
                target: target.clone(),
                trigger: trigger.clone(),
                enabled: *enabled,
                disabled: *disabled,
                search: search.clone(),
                sort: sort.clone(),
                last: *last,
                dry_run: *dry_run,
                force: cli.force,
                limit: *limit,
                to: to.clone(),
                format: export_format.clone(),
                impact: false,
                json: cli.json,
                ..Default::default()
            };
            let (mut report, text) = sweep::engine::autostart::api::cli_tasks(&opts);
            report.finish(watch);
            println!("{text}");
            write_html(cli, &report, "sweep tasks");
            write_md(cli, &report, "sweep tasks");
            Ok(exit_code(&report))
        }
        Command::Services { op, unit, user } => {
            let report = if op == "list" {
                sweep::engine::services::list(*user)
            } else {
                let Some(unit) = unit else {
                    return Err(sweep::core::error::Error::msg(sweep::i18n::t(
                        &sweep::i18n::Lang::detect(&config.languages),
                        "unit name required (e.g. bluetooth.service)",
                    )));
                };
                sweep::engine::services::control(unit, op, *user)
            };
            print_report(&report, format, true, &lang_of(config));
            write_html(cli, &report, "sweep services");
            write_md(cli, &report, "sweep services");
            Ok(exit_code(&report))
        }
        Command::Doctor => {
            let report = sweep::engine::doctor::scan();
            print_report(&report, format, false, &lang_of(config));
            write_html(cli, &report, "sweep doctor");
            write_md(cli, &report, "sweep doctor");
            Ok(exit_code(&report))
        }
        Command::Memopt {
            aggressive,
            dry_run,
        } => {
            let opts = sweep::engine::memory::MemoryOptions {
                aggressive: *aggressive,
                dry_run: *dry_run,
            };
            let watch = sweep::core::report::Stopwatch::start();
            let mut report = sweep::engine::memory::run(&opts);
            report.finish(watch);
            print_report(&report, format, true, &lang_of(config));
            write_html(cli, &report, "sweep memopt");
            write_md(cli, &report, "sweep memopt");
            Ok(exit_code(&report))
        }
        Command::Sysinfo { health } => {
            let opts = sweep::engine::sysinfo::SysInfoOptions {
                health_only: *health,
            };
            let watch = sweep::core::report::Stopwatch::start();
            let (info, mut report) = sweep::engine::sysinfo::run(&opts);
            report.finish(watch);
            if cli.json {
                // `--json` is the full dump: the report plus the raw structure.
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "report": report.to_json(),
                        "sysinfo": info,
                    }))
                    .unwrap_or_default()
                );
            } else {
                print_report(&report, format, false, &lang_of(config));
            }
            write_html(cli, &report, "sweep sysinfo");
            write_md(cli, &report, "sweep sysinfo");
            Ok(exit_code(&report))
        }
        Command::Network { op, dry_run } => {
            let opts = sweep::engine::network::NetworkOptions { dry_run: *dry_run };
            let watch = sweep::core::report::Stopwatch::start();
            let mut report = sweep::engine::network::run(op, &opts);
            report.finish(watch);
            print_report(&report, format, true, &lang_of(config));
            write_html(cli, &report, "sweep network");
            write_md(cli, &report, "sweep network");
            Ok(exit_code(&report))
        }
        Command::Privacy { op, dry_run } => {
            let opts = sweep::engine::privacy::PrivacyOptions { dry_run: *dry_run };
            let watch = sweep::core::report::Stopwatch::start();
            let mut report = sweep::engine::privacy::run(op, &opts);
            report.finish(watch);
            print_report(&report, format, true, &lang_of(config));
            write_html(cli, &report, "sweep privacy");
            write_md(cli, &report, "sweep privacy");
            Ok(exit_code(&report))
        }
        Command::Snapshot { op, target } => {
            let report = match op.as_str() {
                "create" => sweep::engine::snapshot::create(target.as_deref().unwrap_or("")),
                "list" => sweep::engine::snapshot::list(),
                "delete" => {
                    let Some(id) = target else {
                        return Err(sweep::core::error::Error::msg(sweep::i18n::t(
                            &sweep::i18n::Lang::detect(&config.languages),
                            "snapshot id required",
                        )));
                    };
                    sweep::engine::snapshot::delete(id)
                }
                "rollback" => {
                    let Some(id) = target else {
                        return Err(sweep::core::error::Error::msg(sweep::i18n::t(
                            &sweep::i18n::Lang::detect(&config.languages),
                            "snapshot id required",
                        )));
                    };
                    if !cli.yes {
                        let lang = lang_of(config);
                        let mut preview = Report::new();
                        preview.fail(
                            "snapshot",
                            "rollback",
                            sweep::i18n::et(
                                &sweep::i18n::Lang::detect(&config.languages),
                                "SYSTEM WILL ROLL BACK TO {} — reboot required",
                                &[&id],
                            ),
                        );
                        print_report(&preview, format, false, &lang);
                        if !confirm_clean(&preview, &lang, &[]) {
                            println!("{}", sweep::i18n::t(&lang, "cancelled"));
                            return Ok(std::process::ExitCode::SUCCESS);
                        }
                    }
                    sweep::engine::snapshot::rollback(id)
                }
                other => {
                    return Err(sweep::core::error::Error::msg(format!(
                        "unknown op: {other} (create/list/delete/rollback)"
                    )));
                }
            };
            print_report(&report, format, true, &lang_of(config));
            write_html(cli, &report, "sweep snapshot");
            write_md(cli, &report, "sweep snapshot");
            Ok(exit_code(&report))
        }
        Command::History { log_file, days } => {
            let log = log_file
                .clone()
                .unwrap_or_else(sweep::deep::safety::default_log_file);
            let report = sweep::engine::history::scan(&log, *days);
            print_report(&report, format, false, &lang_of(config));
            write_html(cli, &report, "sweep history");
            write_md(cli, &report, "sweep history");
            Ok(exit_code(&report))
        }
        Command::Watch {
            paths,
            threshold,
            interval,
            once,
            suggest,
        } => {
            if *threshold == 0 || *threshold > 100 {
                return Err(sweep::core::error::Error::msg(sweep::i18n::t(
                    &sweep::i18n::Lang::detect(&config.languages),
                    "--threshold must be 1-100",
                )));
            }
            let paths = if paths.is_empty() {
                let mut defaults = vec![PathBuf::from("/")];
                if let Some(home) = sweep::platform::home_dir() {
                    if home.as_os_str() != "/" {
                        defaults.push(home);
                    }
                }
                defaults.into_iter().filter(|p| p.is_dir()).collect()
            } else {
                paths.clone()
            };
            let mut alerted = std::collections::HashSet::new();
            loop {
                let (report, fired) = sweep::engine::watch::once(
                    &paths,
                    f64::from(*threshold),
                    *suggest,
                    &mut alerted,
                );
                print_report(&report, format, false, &lang_of(config));
                if *once {
                    return Ok(if fired {
                        std::process::ExitCode::from(3)
                    } else {
                        std::process::ExitCode::SUCCESS
                    });
                }
                std::thread::sleep(std::time::Duration::from_secs((*interval).max(10)));
            }
        }
        Command::Hub { op, target, url } => {
            let index = url.clone().unwrap_or_else(|| config.hub_url.clone());
            let report = sweep::engine::hub::run(op, target.as_deref(), &index);
            print_report(&report, format, true, &lang_of(config));
            write_html(cli, &report, "sweep hub");
            write_md(cli, &report, "sweep hub");
            Ok(exit_code(&report))
        }
        Command::Wipe { path } => {
            let lang = lang_of(config);
            if !cli.yes {
                let preview_ctx = build_context(cli, config, true, format);
                let preview = wipe_volume(path, &preview_ctx);
                print_report(&preview, format, false, &lang);
                if !confirm_clean(&preview, &lang, &[]) {
                    println!("{}", sweep::i18n::t(&lang, "cancelled"));
                    return Ok(std::process::ExitCode::SUCCESS);
                }
            }
            let ctx = build_context(cli, config, false, format);
            let report = wipe_volume(path, &ctx);
            print_report(&report, format, true, &lang_of(config));
            write_html(cli, &report, "sweep wipe");
            write_md(cli, &report, "sweep wipe");
            Ok(exit_code(&report))
        }
        Command::Providers => {
            print_providers(format);
            Ok(std::process::ExitCode::SUCCESS)
        }
        Command::Diagnostics => {
            print_diagnostics(format);
            Ok(std::process::ExitCode::SUCCESS)
        }
        Command::Import { path } => {
            let warnings = validate_path(path);
            if cli.json {
                println!("{}", serde_json::json!({ "warnings": warnings }));
            } else {
                for w in &warnings {
                    println!("warning: {w}");
                }
                println!("validated {} (0 errors)", path.display());
            }
            Ok(std::process::ExitCode::SUCCESS)
        }
        Command::Completions { shell } => match gen::completions(shell) {
            Ok(script) => {
                print!("{script}");
                Ok(std::process::ExitCode::SUCCESS)
            }
            Err(err) => Err(sweep::core::error::Error::msg(err)),
        },
        Command::Man { output } => {
            let page = gen::man();
            match output {
                Some(dir) => {
                    std::fs::create_dir_all(dir)
                        .map_err(|e| sweep::core::error::Error::msg(e.to_string()))?;
                    let dest = dir.join("sweep.1");
                    std::fs::write(&dest, page)
                        .map_err(|e| sweep::core::error::Error::msg(e.to_string()))?;
                    if !cli.json {
                        println!("wrote {}", dest.display());
                    }
                }
                None => print!("{page}"),
            }
            Ok(std::process::ExitCode::SUCCESS)
        }
    }
}

/// Alias for `system.free_disk_space` style wiping.
fn wipe_volume(path: &std::path::Path, ctx: &RunContext) -> Report {
    let progress = sweep::fsutil::freespace::NullWipeProgress;
    let mut report = Report::new();
    if ctx.dry_run {
        report.push(sweep::core::report::Entry::new(
            sweep::core::report::EntryKind::WipeFreeSpace,
            "system",
            "free_disk_space",
            format!("would wipe {}", path.display()),
            Some(path),
            0,
        ));
        return report;
    }
    match sweep::fsutil::freespace::wipe_free_space(
        path,
        sweep::fsutil::freespace::WipeConfig::default(),
        &progress,
        &ctx.cancel,
    ) {
        Ok(wiped) => report.push(sweep::core::report::Entry::new(
            sweep::core::report::EntryKind::WipeFreeSpace,
            "system",
            "free_disk_space",
            format!("wiped {}", path.display()),
            Some(path),
            wiped,
        )),
        Err(err) => report.fail("system", "free_disk_space", err.to_string()),
    }
    report
}

// --- human / json printers --------------------------------------------------

/// `--search/--os/--running` filtrelerini uygula (JSON ve insan çıktısı ortak).
fn filtered<'a>(
    registry: &'a CleanerRegistry,
    search: Option<&str>,
    os: Option<&str>,
    running_only: bool,
) -> Vec<&'a sweep::definition::model::LoadedCleaner> {
    let needle = search
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    let wanted_os = os
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty());
    let mut out: Vec<&'a sweep::definition::model::LoadedCleaner> = registry
        .all()
        .filter(|loaded| {
            let def = &loaded.def;
            if let Some(needle) = &needle {
                let haystack =
                    format!("{} {} {}", def.id, def.name, def.description).to_lowercase();
                if !haystack.contains(needle) {
                    return false;
                }
            }
            if let Some(wanted) = &wanted_os {
                // A cleaner without an `os` filter applies everywhere.
                match &def.os.0 {
                    Some(filter) if !filter.to_lowercase().contains(wanted.as_str()) => {
                        return false
                    }
                    _ => {}
                }
            }
            !running_only || def.is_running()
        })
        .collect();
    out.sort_by(|a, b| a.def.id.cmp(&b.def.id));
    out
}

fn print_list(
    registry: &CleanerRegistry,
    cleaner: &Option<String>,
    search: Option<&str>,
    os: Option<&str>,
    running_only: bool,
    format: Format,
) {
    let shown = filtered(registry, search, os, running_only);

    if format.is_json() {
        let mut map = serde_json::Map::new();
        for loaded in &shown {
            let def = &loaded.def;
            let options: Vec<serde_json::Value> = def
                .active_options()
                .map(|o| {
                    serde_json::json!({
                        "id": o.id,
                        "label": o.label,
                        "description": o.description,
                    })
                })
                .collect();
            map.insert(
                def.id.clone(),
                serde_json::json!({
                    "name": def.name,
                    "description": def.description,
                    "os": def.os.0.clone().unwrap_or_else(|| "any".into()),
                    "trust": if matches!(loaded.trust, sweep::definition::model::Trust::Trusted) { "trusted" } else { "untrusted" },
                    "running": def.is_running(),
                    "options": options,
                }),
            );
        }
        println!("{}", serde_json::Value::Object(map));
        return;
    }

    if let Some(id) = cleaner {
        let Some(loaded) = registry.get(id) else {
            println!("unknown cleaner '{id}'");
            return;
        };
        let def = &loaded.def;
        println!("{} ({})", def.name, def.id);
        if !def.description.is_empty() {
            println!("  {}", def.description);
        }
        for option in def.active_options() {
            println!("  - {} : {}", option.id, option.label);
            if !option.description.is_empty() {
                println!("      {}", option.description);
            }
        }
        return;
    }

    println!("{} cleaners:\n", shown.len());
    for loaded in &shown {
        let def = &loaded.def;
        let running = if def.is_running() { " [running]" } else { "" };
        println!("  {:<18} {}{}", def.id, def.name, running);
    }
    if shown.is_empty() && (search.is_some() || os.is_some() || running_only) {
        println!("\n  (no cleaner matches the filters)");
    }
}

/// `sweep info <cleaner>`: tanımın tamamı (seçenekler, eylemler, yollar, güven).
fn print_info(
    registry: &CleanerRegistry,
    cleaner: &str,
    format: Format,
) -> sweep::core::error::Result<()> {
    let loaded = registry
        .get(cleaner)
        .or_else(|| {
            let lower = cleaner.to_lowercase();
            registry.all().find(|c| c.def.id.to_lowercase() == lower)
        })
        .ok_or_else(|| sweep::core::error::Error::UnknownCleaner(cleaner.to_string()))?;
    let def = &loaded.def;

    if format.is_json() {
        let options: Vec<serde_json::Value> = def
            .options
            .iter()
            .map(|o| {
                let actions: Vec<serde_json::Value> = o
                    .actions
                    .iter()
                    .map(|a| {
                        serde_json::json!({
                            "command": a.command,
                            "search": a.search.as_str(),
                            "path": a.path,
                            "os": a.os.0.clone().unwrap_or_else(|| "any".into()),
                            "regex": a.regex,
                            "nregex": a.nregex,
                            "max_age_days": a.max_age_days,
                            "min_size": a.min_size,
                            "max_depth": a.max_depth,
                        })
                    })
                    .collect();
                serde_json::json!({
                    "id": o.id,
                    "label": o.label,
                    "description": o.description,
                    "warning": o.warning,
                    "os": o.os.0.clone().unwrap_or_else(|| "any".into()),
                    "active": o.os.matches(),
                    "actions": actions,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "id": def.id,
                "name": def.name,
                "description": def.description,
                "os": def.os.0.clone().unwrap_or_else(|| "any".into()),
                "source": loaded.source,
                "trust": if matches!(loaded.trust, sweep::definition::model::Trust::Trusted) { "trusted" } else { "untrusted" },
                "running": def.is_running(),
                "running_reason": def.running_reason(),
                "vars": def.vars.iter().map(|v| &v.name).collect::<Vec<_>>(),
                "options": options,
                "problems": def.validate(),
            }))
            .unwrap_or_default()
        );
        return Ok(());
    }

    println!("{} ({})", def.name, def.id);
    if !def.description.is_empty() {
        println!("  {}\n", def.description);
    }
    println!("  os            : {}", def.os.0.as_deref().unwrap_or("any"));
    println!(
        "  trust         : {}",
        if matches!(loaded.trust, sweep::definition::model::Trust::Trusted) {
            "trusted"
        } else {
            "untrusted (process/winreg actions stripped)"
        }
    );
    println!("  source        : {}", loaded.source.display());
    println!(
        "  running       : {}",
        def.running_reason().unwrap_or_else(|| "no".to_string())
    );
    if !def.vars.is_empty() {
        println!(
            "  vars          : {}",
            def.vars
                .iter()
                .map(|v| v.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let problems = def.validate();
    if !problems.is_empty() {
        println!("\n  problems:");
        for problem in &problems {
            println!("    ! {problem}");
        }
    }

    for option in &def.options {
        let active = if option.os.matches() {
            ""
        } else {
            "  (not on this OS)"
        };
        println!("\n  ▸ {}{}", option.id, active);
        println!("      label : {}", option.label);
        if !option.description.is_empty() {
            println!("      about : {}", option.description);
        }
        if let Some(warning) = &option.warning {
            println!("      ⚠ {warning}");
        }
        for action in &option.actions {
            let mut detail = format!("{} [{}]", action.command, action.search.as_str());
            if !action.path.is_empty() {
                detail.push_str(&format!(" {}", action.path));
            }
            println!("      - {detail}");
            let mut filters: Vec<String> = Vec::new();
            if let Some(re) = &action.regex {
                filters.push(format!("regex={re}"));
            }
            if let Some(re) = &action.nregex {
                filters.push(format!("nregex={re}"));
            }
            if let Some(days) = action.max_age_days {
                filters.push(format!("max_age={days}d"));
            }
            if let Some(size) = action.min_size {
                filters.push(format!(
                    "min_size={}",
                    sweep::fsutil::size::bytes_to_human(size, false)
                ));
            }
            if let Some(size) = action.max_size {
                filters.push(format!(
                    "max_size={}",
                    sweep::fsutil::size::bytes_to_human(size, false)
                ));
            }
            if let Some(depth) = action.max_depth {
                filters.push(format!("max_depth={depth}"));
            }
            if action.os.0.is_some() {
                filters.push(format!("os={}", action.os.0.as_deref().unwrap_or("any")));
            }
            if !filters.is_empty() {
                println!("          {}", filters.join(" "));
            }
        }
    }
    Ok(())
}

/// `sweep config …`: yapılandırma dosyasını göster / düzenle.
fn run_config(
    cli: &Cli,
    config: &Config,
    op: &str,
    key: Option<&str>,
    value: Option<&str>,
    format: Format,
) -> sweep::core::error::Result<std::process::ExitCode> {
    let path = cli.config.clone().unwrap_or_else(Config::default_path);
    let json = format.is_json();

    match op {
        "path" => {
            if json {
                println!("{}", serde_json::json!({ "path": path }));
            } else {
                println!("{}", path.display());
            }
        }
        "show" => {
            if json {
                let map: serde_json::Map<String, serde_json::Value> = config
                    .fields()
                    .into_iter()
                    .map(|(name, value)| (name.to_string(), serde_json::Value::String(value)))
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "path": path,
                        "exists": path.exists(),
                        "settings": map,
                    }))
                    .unwrap_or_default()
                );
            } else {
                println!("config: {}\n", path.display());
                for (name, value) in config.fields() {
                    println!("  {:<20} {}", name, value);
                }
            }
        }
        "init" => {
            if path.exists() && !cli.yes {
                return Err(sweep::core::error::Error::msg(format!(
                    "{} already exists (pass --yes to overwrite)",
                    path.display()
                )));
            }
            config.save(&path).map_err(|err| {
                sweep::core::error::Error::msg(format!("cannot write {}: {err}", path.display()))
            })?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "written": path, "settings": config.fields().len() })
                );
            } else {
                println!("wrote {}", path.display());
            }
        }
        "get" => {
            let Some(name) = key else {
                return Err(sweep::core::error::Error::msg(
                    "usage: sweep config get KEY",
                ));
            };
            let Some(value) = config.get_field(name) else {
                return Err(sweep::core::error::Error::msg(format!(
                    "unknown setting '{name}' (try `sweep config show`)"
                )));
            };
            if json {
                println!("{}", serde_json::json!({ "key": name, "value": value }));
            } else {
                println!("{value}");
            }
        }
        "set" => {
            let (Some(name), Some(value)) = (key, value) else {
                return Err(sweep::core::error::Error::msg(
                    "usage: sweep config set KEY VALUE",
                ));
            };
            let mut updated = config.clone();
            updated.set_field(name, value)?;
            updated.save(&path).map_err(|err| {
                sweep::core::error::Error::msg(format!("cannot write {}: {err}", path.display()))
            })?;
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "key": name, "value": updated.get_field(name), "path": path })
                );
            } else {
                println!("{} = {}", name, updated.get_field(name).unwrap_or_default());
            }
        }
        other => {
            return Err(sweep::core::error::Error::msg(format!(
                "unknown op '{other}' (show/path/init/get/set)"
            )))
        }
    }
    Ok(std::process::ExitCode::SUCCESS)
}

/// `--remember` (veya `remember_last`) açıksa son seçimi config'e yaz.
fn remember_selection(
    cli: &Cli,
    config: &Config,
    selection: &[sweep::engine::worker::SelectedOption],
    is_clean: bool,
    remember: bool,
) {
    if !is_clean || selection.is_empty() || !(remember || config.remember_last) {
        return;
    }
    let path = cli.config.clone().unwrap_or_else(Config::default_path);
    let mut saved = Config::load(&path).unwrap_or_else(|_| config.clone());
    saved.remember_last = true;
    saved.last_selection = selection
        .iter()
        .map(|s| format!("{}.{}", s.cleaner, s.option))
        .collect();
    if let Err(err) = saved.save(&path) {
        log::warn!("could not remember selection ({}): {err}", path.display());
    }
}

fn lang_of(config: &Config) -> sweep::i18n::Lang {
    sweep::i18n::Lang::detect(&config.languages)
}

/// `--html PATH` verildiyse raporu bağımsız HTML dosyası olarak yaz.
fn write_html(cli: &Cli, report: &Report, title: &str) {
    if let Some(dest) = &cli.html {
        if let Err(err) = sweep::core::report_html::write_html(report, title, dest) {
            eprintln!("html yazılamadı ({}): {err}", dest.display());
        } else {
            println!("HTML raporu: {}", dest.display());
        }
    }
}

/// `--markdown PATH` verildiyse raporu Markdown olarak yaz.
fn write_md(cli: &Cli, report: &Report, title: &str) {
    if let Some(dest) = &cli.markdown {
        if let Err(err) = sweep::core::report_md::write_markdown(report, title, dest) {
            eprintln!("markdown yazılamadı ({}): {err}", dest.display());
        } else {
            println!("Markdown raporu: {}", dest.display());
        }
    }
}

/// How many individual entries a `--summary` report keeps as a sample.
const SUMMARY_SAMPLE_ENTRIES: usize = 200;

fn print_report(report: &Report, format: Format, _cleaned: bool, lang: &sweep::i18n::Lang) {
    if format.is_json() {
        let json = if matches!(format, Format::JsonSummary) {
            report.to_json_summary(SUMMARY_SAMPLE_ENTRIES)
        } else {
            report.to_json()
        };
        println!("{}", serde_json::to_string_pretty(&json).unwrap());
        return;
    }

    // `--summary` drops the one-line-per-path list. That list is what turned a
    // `preview --all` into tens of megabytes of text — and, for the GUI, of
    // JSON — once the big cache trees were selected; the per-option breakdown
    // printed below carries the same information in a few dozen lines.
    if !matches!(format, Format::Summary) {
        for entry in &report.entries {
            let path = entry
                .path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| entry.label.clone());
            let size = if entry.reclaimed > 0 {
                format!(
                    " ({})",
                    sweep::fsutil::size::bytes_to_human(entry.reclaimed, false)
                )
            } else {
                String::new()
            };
            println!("  [{}] {}{}", entry.kind.label(), path, size);
        }
    }

    print_totals(report, lang);
}

/// Failures, totals, duration and the per-option breakdown — the tail every
/// human-readable report ends with, whether or not the entries were listed.
fn print_totals(report: &Report, lang: &sweep::i18n::Lang) {
    if !report.errors.is_empty() {
        println!("\n{}:", sweep::i18n::t(lang, "failures"));
        for failure in &report.errors {
            println!("  - {}: {}", failure.option, failure.message);
        }
    }

    println!(
        "\n{}: {} | {}: {} | {}: {} | {}: {}",
        sweep::i18n::t(lang, "reclaimed"),
        sweep::fsutil::size::bytes_to_human(report.reclaimed(), false),
        sweep::i18n::t(lang, "files_removed"),
        report.files_removed(),
        sweep::i18n::t(lang, "special_ops"),
        report.special_operations(),
        sweep::i18n::t(lang, "skipped"),
        report.skipped(),
    );
    if report.duration_ms > 0 {
        println!("duration: {} ms", report.duration_ms);
    }
    print_size_breakdown(report, lang);
    if report.aborted {
        println!("{}", sweep::i18n::t(lang, "aborted"));
    }
}

/// Boyut bazlı ayrıntılı rapor: her `cleaner.option` için dosya sayısı + bayt.
fn print_size_breakdown(report: &Report, lang: &sweep::i18n::Lang) {
    use std::collections::BTreeMap;
    let mut by_option: BTreeMap<(String, String), (u64, u64)> = BTreeMap::new();
    for entry in &report.entries {
        if entry.reclaimed == 0 && !entry.kind.counts_as_deleted() {
            continue;
        }
        let slot = by_option
            .entry((entry.cleaner.clone(), entry.option.clone()))
            .or_insert((0, 0));
        slot.0 += u64::from(entry.kind.counts_as_deleted());
        slot.1 += entry.reclaimed;
    }
    if by_option.is_empty() {
        return;
    }
    println!("\n{}:", sweep::i18n::t(lang, "by_category"));
    println!(
        "  {:<28} {:>8} {:>12}",
        "cleaner.option",
        sweep::i18n::t(lang, "files"),
        sweep::i18n::t(lang, "size")
    );
    for ((cleaner, option), (files, bytes)) in &by_option {
        println!(
            "  {:<28} {:>8} {:>12}",
            format!("{cleaner}.{option}"),
            files,
            sweep::fsutil::size::bytes_to_human(*bytes, false),
        );
    }
}

fn print_providers(format: Format) {
    if format.is_json() {
        let list: Vec<serde_json::Value> = sweep::action::provider::list()
            .iter()
            .filter(|p| p.available())
            .map(|p| {
                serde_json::json!({
                    "command": p.name,
                    "summary": p.summary,
                    "needs_path": p.needs_path,
                    "destructive": p.destructive,
                    "privileged": p.privileged,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::Value::Array(list)).unwrap()
        );
        return;
    }

    println!("Action commands:\n");
    for provider in sweep::action::provider::list() {
        if !provider.available() {
            continue;
        }
        let tags = if provider.destructive {
            " [destructive]"
        } else {
            ""
        };
        let privi = if provider.privileged {
            " [privileged]"
        } else {
            ""
        };
        println!(
            "  {:<24} {}{}{}",
            provider.name, provider.summary, tags, privi
        );
    }
    println!(
        "\n{} commands available on this platform.",
        sweep::action::provider::list()
            .iter()
            .filter(|p| p.available())
            .count()
    );
}

fn print_diagnostics(format: Format) {
    let lines = platform::diagnostics();
    let managers = sweep::deep::managers::detect();
    if format.is_json() {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "os": platform::os_name(),
                "diagnostics": lines,
                "managers": managers,
            }))
            .unwrap()
        );
        return;
    }
    println!("Platform: {}", platform::os_name());
    for line in lines {
        println!("  {line}");
    }
    println!("Package managers:");
    for m in &managers {
        println!(
            "  {} [{}]",
            m.label,
            if m.found { "installed" } else { "missing" }
        );
    }
}

/// Validate a cleaner file/directory, returning warnings.
fn validate_path(path: &std::path::Path) -> Vec<String> {
    let mut warnings = Vec::new();
    let trust = sweep::definition::model::Trust::Untrusted;
    if path.is_dir() {
        let mut registry = CleanerRegistry::new();
        registry.load_dir(path, trust);
        warnings.extend(registry.warnings().iter().cloned());
    } else if path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("toml"))
        .unwrap_or(false)
    {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(err) => return vec![format!("cannot read {}: {err}", path.display())],
        };
        match sweep::definition::native::parse_native(&text, &path.display().to_string()) {
            Ok(def) => warnings.extend(def.validate()),
            Err(err) => warnings.push(err.to_string()),
        }
    } else if path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("xml"))
        .unwrap_or(false)
    {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(err) => return vec![format!("cannot read {}: {err}", path.display())],
        };
        match sweep::definition::cleanerml::parse_cleanerml(
            &text,
            &path.display().to_string(),
            trust,
        ) {
            Ok(def) => warnings.extend(def.validate()),
            Err(err) => warnings.push(err.to_string()),
        }
    } else if path
        .extension()
        .map(|e| e.eq_ignore_ascii_case("ini"))
        .unwrap_or(false)
    {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(err) => return vec![format!("cannot read {}: {err}", path.display())],
        };
        match sweep::definition::winapp2::parse_winapp2(&text, &path.display().to_string(), trust) {
            Ok(defs) => {
                if defs.is_empty() {
                    warnings.push("no recognizable rules found".into());
                }
                for def in defs {
                    warnings.extend(def.validate());
                }
            }
            Err(err) => warnings.push(err.to_string()),
        }
    } else {
        warnings.push("unsupported file type (expected .toml, .xml or .ini)".into());
    }
    warnings
}

/// `--min-size` benzeri boyut bayrağını bayta çevir (yoksa varsayılan).
fn parse_size_flag(
    raw: Option<&String>,
    default: u64,
    flag: &str,
) -> sweep::core::error::Result<u64> {
    match raw {
        Some(text) => sweep::fsutil::size::human_to_bytes(text).map_err(|err| {
            sweep::core::error::Error::msg(sweep::i18n::et(
                &sweep::i18n::Lang::detect(&[]),
                "invalid {} '{}': {}",
                &[flag.to_string(), text.clone(), err.to_string()],
            ))
        }),
        None => Ok(default),
    }
}

/// Önizleme-önce-sonra-temizle akışı (leftovers/devscan/dupes comparten).
fn run_preview_clean(
    cli: &Cli,
    config: &Config,
    format: Format,
    clean: bool,
    warnings: &[String],
    title: &str,
    run: impl Fn(&RunContext) -> Report,
) -> sweep::core::error::Result<std::process::ExitCode> {
    use sweep::core::report::Stopwatch;
    let lang = lang_of(config);
    let preview_watch = Stopwatch::start();
    let preview_ctx = build_context(cli, config, true, format);
    let mut preview = run(&preview_ctx);
    preview.finish(preview_watch);
    print_warnings(warnings, format);
    print_report(&preview, format, false, &lang);
    write_md(cli, &preview, title);
    if !clean {
        write_html(cli, &preview, title);
        return Ok(exit_code(&preview));
    }
    if !cli.yes && !confirm_clean(&preview, &lang, warnings) {
        println!("{}", sweep::i18n::t(&lang, "cancelled"));
        return Ok(std::process::ExitCode::SUCCESS);
    }
    let watch = Stopwatch::start();
    let ctx = build_context(cli, config, false, format);
    let mut report = run(&ctx);
    report.finish(watch);
    print_report(&report, format, true, &lang);
    write_html(cli, &report, title);
    write_md(cli, &report, title);
    Ok(exit_code(&report))
}

/// Pre-clean confirmation: show totals, require explicit `y`.
fn confirm_clean(preview: &Report, lang: &sweep::i18n::Lang, warnings: &[String]) -> bool {
    use std::io::{IsTerminal, Write as _};
    if !std::io::stdin().is_terminal() {
        eprintln!("{}", sweep::i18n::t(lang, "need_yes"));
        return false;
    }
    for w in warnings {
        eprintln!("  ! {w}");
    }
    eprintln!(
        "{}",
        sweep::i18n::et(
            lang,
            "preview_summary",
            &[
                &preview.files_removed().to_string(),
                &sweep::i18n::t(lang, "files"),
                &sweep::fsutil::size::bytes_to_human(preview.reclaimed(), false),
                &sweep::i18n::t(lang, "reclaimed"),
                &preview.errors.len().to_string(),
                &sweep::i18n::t(lang, "failures"),
                &sweep::i18n::t(lang, "confirm_prompt"),
            ],
        )
    );
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return false;
    }
    matches!(
        line.trim().to_lowercase().as_str(),
        "y" | "yes"
            | "e"
            | "evet"
            | "s"
            | "si"
            | "sim"
            | "sì"
            | "o"
            | "oui"
            | "j"
            | "ja"
            | "d"
            | "da"
            | "д"
            | "да"
    )
}

/// `sweep residue preview`: numbered list, interactive range selection,
/// then the standard preview → confirm → delete flow on the selection.
fn residue_preview(
    cli: &Cli,
    config: &Config,
    format: Format,
    opts: &sweep::engine::residue::ResidueOptions,
) -> sweep::core::error::Result<std::process::ExitCode> {
    use std::io::{IsTerminal, Write as _};
    use sweep::core::report::Stopwatch;
    let lang = lang_of(config);
    let items = sweep::engine::residue::collect(opts);
    if items.is_empty() || cli.json || !std::io::stdin().is_terminal() {
        // Etkileşim yok: düz liste raporu.
        let watch = Stopwatch::start();
        let mut report = sweep::engine::residue::report_of(&items);
        if items.is_empty() {
            report = sweep::engine::residue::scan(
                opts,
                &build_context(cli, config, true, format),
            );
        }
        report.finish(watch);
        print_report(&report, format, false, &lang);
        write_md(cli, &report, "sweep residue preview");
        write_html(cli, &report, "sweep residue preview");
        return Ok(exit_code(&report));
    }
    for (i, r) in items.iter().enumerate() {
        eprintln!(
            "  [{}] [{}] conf {:.2}{} {} ({}) — {}",
            i + 1,
            r.kind.as_str(),
            r.confidence,
            if r.safe_to_delete { "" } else { " NEEDS-REVIEW" },
            r.path.display(),
            sweep::fsutil::size::bytes_to_human(r.bytes, false),
            r.reason,
        );
    }
    eprintln!(
        "{}",
        sweep::i18n::t(&lang, "select residues (1,3-5 / all / none)")
    );
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    if std::io::stdin().read_line(&mut line).is_err() {
        return Ok(std::process::ExitCode::SUCCESS);
    }
    let picked: Vec<sweep::engine::residue::Residue> =
        sweep::engine::residue::parse_selection(&line, items.len())
            .into_iter()
            .map(|i| items[i].clone())
            .collect();
    if picked.is_empty() {
        println!("{}", sweep::i18n::t(&lang, "cancelled"));
        return Ok(std::process::ExitCode::SUCCESS);
    }
    run_preview_clean(cli, config, format, true, &[], "sweep residue preview", |ctx| {
        sweep::engine::residue::clean_selected(&picked, ctx)
    })
}

/// Map a report to a process exit code.
fn exit_code(report: &Report) -> std::process::ExitCode {
    if report.errors.is_empty() && !report.aborted {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}
