# Changelog

All notable changes to Sweep are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - 2026-09-13

### Added

- Safe rollback and backup system (one-click undo, selective restore):
  - every real clean run now appends a `run` marker (`ts | run | <id> |
    <selection> | 0B`) to `operations.log` (worker, residue, shred,
    deepscan); old marker-less logs still read as one scope;
  - `sweep undo --last` rolls back the most recent run, `sweep undo --run
    <id>` a chosen run, `sweep undo --list` prints runs newest-first for
    the GUI history screen;
  - fail-closed restore policy: a validation blocker (vanished backup,
    uncreatable parent dir) aborts the whole scope with zero writes and an
    explicit error; files that never had a backup are reported as an
    explicit aggregated error (non-zero exit) while restorable files are
    still written; existing targets are never overwritten (`skipped`);
  - final safety verification before every delete: the ordinary cleaners
    (`action::file`, `action::system`) now run the same `check_path` gate
    as the deep cleaners (protected-path, symlink-target, critical-dir,
    root-only, writable-parent) on top of the advisory `Guard` /
    `safe_to_delete` decision, and `check_path` additionally refuses
    relative paths; recursive deep walks (`clean_dir_contents`, temp,
    system `delete_tree`) stay on the same filesystem;
  - ordinary file deletes/truncates are now backed up (`--backup-dir`) and
    audit-logged like deep deletes, so one-click undo covers them (dirs
    and bare link names stay unlogged: dirs are recreated on restore);
  - `sweep clean` with a Deep/System selection prints a system-backup
    recommendation before continuing;
  - GUI: post-clean "Undo available" button (`undo --last`), history screen
    lists previous runs with per-run Restore, danger-tier selections show a
    backup dialog ("Create Backup" opens the OS-native backup target:
    Windows Backup settings, Time Machine settings, Déjà Dup — with a
    manual-backup fallback message; "Continue" proceeds), warn-tier
    selections must preview first; GUI cleans always pass `--backup-dir`
    so undo works. New `u.*` UI strings in all 8 languages; new engine
    strings translated to Turkish (other languages fall back to English
    per convention).

### Fixed

- GUI window chrome and controls (Windows feedback):
  - sidebar glyphs bumped 16→19 px (tiny/missing in Segoe UI Symbol fallback);
  - default window size 1360x860 → 1240x780;
  - Memory screen swap panel was dead on all platforms (`sys.swapTotal` /
    `sys.swapUsed` were read by QML but never declared) — new `swapTotal` /
    `swapUsed` properties fed from `/proc/meminfo` (Linux) and the pagefile
    counters (Windows); a missing `sweep` engine now shows an amber
    `g.no_engine` banner (8 languages) instead of a silent empty screen, and
    sync calls report `cannot start <bin>` instead of a bare `timeout`;
  - NeonSlider hit-area 30→40 px, knob 17→22 px (hard to grab inside the
    Settings Flickable);
  - native gray title bar removed (`Qt.FramelessWindowHint`) with a custom
    42 px title bar (drag to move, double-click to maximize, min/max/close)
    plus a bottom-right resize grip; edge-resize and Aero-snap are gone by
    design, tray-on-close still works.
  - close fade now also plays when minimizing to tray (previously the tray
    path hid instantly, so no animation was visible); default size
    1240x780 → 1140x720 (min 1000x620); window corners rounded (radius 12,
    0 when maximized) via a clipped chrome rect over a transparent window.
  - duplicate startup screen removed: the legacy `StartupMgr`
    (StartupScreen, old `startupList` API) nav entry and routes are gone;
    the unified "Başlangıç ve Görevler" (AutostartScreen: autostart entries
    + scheduled tasks, audit history, rollback) is the only one. The
    orphaned `StartupScreen.qml` stays registered until it is `git rm`'d.
  - Schedule and Autostart screens now explain privilege failures: when a
    task/entry operation returns access-denied text and the app is not
    elevated, the result keeps the raw message and appends the existing
    `g.needs_root` hint (8 languages, no new keys) instead of leaving a
    cryptic `schtasks ... Access is denied` on screen. Same pattern as the
    Memory screen.
  - Autostart state filter chips (`on`/`off`) are now translated to the
    `enabled`/`disabled` flags the bridge expects (the flag was silently
    dropped before; client-side filtering masked it).
  - installer asks before touching a running Sweep: `.onInit` detects
    `sweep-qml.exe` / `sweep.exe` via `tasklist` and offers Yes (taskkill +
    continue) or No (quit) instead of failing later with "error opening
    file for writing"; the uninstaller now also deletes the `SweepClean`
    and `SweepMemory` scheduled tasks (they used to survive uninstall and
    fire a deleted binary).
  - privilege failures are now actionable: Schedule and Autostart screens
    offer a "Retry as admin" button (`g.elev_retry`, 8 languages) that runs
    the same operation elevated via UAC; results arrive on the new
    `schedElevatedReady` / `startupElevatedReady` bridge signals (previously
    `emitJsonForOp` dropped them as "unknown elevated op"). Elevated command
    lines now quote space-containing arguments.
  - reduced-motion check: the Settings motion switch already pauses the
    Aurora drift and collapses all transitions, and it persists via
    `animScale` — no change needed for weak GPUs.
  - first-launch welcome + guided tour: a full-screen animated "Welcome to
    Sweep" stage with the Sweep logo in an accent-ringed medallion
    (counter-rotating orbit sparks, gentle float, drifting background
    washes, staggered side-content entrance, motion-safe) next to the
    title, subtitle, feature rows and Take a tour / Skip; the 4-stop tour
    (Dashboard → Cleaner → Autostart → Schedule) drives the real screens
    with a caption card reusing existing titles. Completion persists via a
    new `welcomed` setting; 7 new strings in all 8 languages.
- `verify_qml.py` now also checks that every `sys.*` read resolves to a
  `SysInfo` Q_PROPERTY and every `sweep.*()` call to a bridge Q_INVOKABLE
  (the exact bug class behind the dead swap panel).
- Cleaner screen never loaded its list: `reload()` was never called on entry
  and wrote to undeclared `working`/`cleanProgress` properties, so the call
  died before `sweep.listCleaners()` ran (empty list on every platform).
  Both properties are declared now and the list loads on entry; `verify_qml.py`
  gained a `root.*`-write check so this class can never ship silently again.
- Quitting mid-scan orphaned the `sweep` CLI child (visible as a second
  "sweep" in Task Manager): `aboutToQuit` now kills the in-flight process.
- Real quit plays a 220 ms fade-out (`exitFade`); minimize-to-tray still
  hides instantly.
- The Windows GUI never became usable. Two independent faults, both fixed:
  - the installer shipped a `sweep-qml.exe` linked against the UCRT
    (`api-ms-win-crt-*`) while the bundled Qt 6.8.1 `win64_mingw` DLLs link
    `msvcrt.dll`. Two CRTs in one process killed it with `0xC0000005` inside
    `ntdll.dll` before any window existed (14 minidumps, Application event log
    id 1000). The GUI is now always built with Qt's matching MinGW 13.1 with the
    compilers pinned explicitly, and the built *and* staged binary is verified
    before packaging.
  - `packaging/qml/main.cpp` created the tray menu with `new QMenu()` under a
    `QGuiApplication`. `QMenu` is a QWidget, so this hit
    `QWidget: Cannot create a QWidget without QApplication` - a `qFatal`, i.e.
    `abort()` - on every machine where a system tray exists. The window appeared
    and instantly died unpainted (plus a WER "fail fast" dialog). Headless smoke
    tests never caught it because `isSystemTrayAvailable()` is false there. The
    app now uses `QApplication`; `Qt6::Widgets` was already linked and deployed.
- Packaging could ship a stale GUI. The stage swap used an unchecked `mv`/
    `Move-Item`, so with the previous stage still present the fresh stage landed
    *inside* it and `makensis` packaged the previous binary, dragging
    `stage.new.*` directories into the installer and `%ProgramFiles%\Sweep`.
    The build driver now stages into a timestamped directory, passes it to
    `makensis` with `-DSTAGE=` and never relies on a swap.
- `preview` / `clean` over a large selection (the GUI's *Select all*, or
  `--all`) looked like a hang. Three independent causes, all fixed:
  - the GUI decoded its **whole** accumulated stdout with
    `QString::fromUtf8()` on every `readyReadStandardOutput` chunk. Rust's
    stdout is line-buffered, so a tens-of-megabytes `--json` report arrives in
    thousands of chunks: O(n²) decoding on the GUI thread, which then stopped
    draining the pipe and made the child block on write. `onOutput` now decodes
    only the newly arrived chunk and keeps just the unfinished last line;
  - the file action recursively re-sized **every** directory it visited
    (`dir_size`, a full `WalkDir` per directory — O(files × depth) extra stats
    on a cache tree), and then threw the number away for non-empty ones, which
    is the only kind that matters here: `delete` uses `remove_dir`, so a
    non-empty directory can never be removed by that path. Directories now
    report 0 bytes and their contents carry the total, exactly as the preview
    already assumed;
  - `Guard::check` normalised each candidate path twice (once for the keep
    list, once for the protected roots); it now normalises once.
- Elevated (UAC) `clean` on Windows returned a *human* report: the bridge ran
  `sweep clean --yes …` through `ShellExecuteExW` without `--json`, so the GUI's
  JSON parser rejected the output and the screen reported "clean failed" while
  the deletion had in fact gone through. `runElevated` now prefixes
  `--quiet --json` for every elevated operation.

### Added

- `--summary`: an aggregated report form for very large selections. Instead of
  one line — or one JSON object — per touched path it reports exact
  `cleaner.option` totals (`by_option` in JSON, the existing per-category
  breakdown in text) plus a bounded, evenly-spread sample of entries, with
  `entries_truncated` saying whether any were dropped. Every counter
  (`files_removed`, `reclaimed_bytes`, `items_count`, …) stays exact, so
  consumers that only read totals are unaffected. The Qt6/QML shell now always
  runs `--json --summary`, which turns a `preview --all` report from tens of
  megabytes into a few kilobytes; the Dashboard donut and the cleaner list read
  the `by_option` totals directly and fall back to summing entries when an
  older engine does not send them.

- Disk analyzer: new `sweep du <path>` engine (parallel per-child recursive
  sizes, read-only, never deletes) plus a Disk screen — volume bars,
  double-click folder drill-down with breadcrumbs and an Up button, size bars,
  cancel support. Read-only by design.
- Automatic memory maintenance (mode C, both layers): in-app threshold keeper
  (checks every 5 min against live data, trims with safe memopt only when RAM
  passes 80%, 15 min cooldown, toggle persisted in QSettings) plus a separate
  daily `memopt --quiet` system task via `sweep schedule --enable --memopt`
  (Task Scheduler `Sweep\SweepMemory` on Windows, `sweep-memopt.timer` on
  systemd, `com.sweep.memopt` on launchd), manageable from a new Memory-task
  card on the Schedule screen.
- `packaging/dist/windows/build-setup.ps1` - one-command Windows driver:
  `cargo build --release` -> build the GUI with Qt's own MinGW 13.1 -> verify
  the binary -> stage + `windeployqt` -> verify the stage ->
  `makensis -DSTAGE=` -> verify that the installer really contains the verified
  `sweep-qml.exe` (sha256). Any failed guard refuses to package and exits
  non-zero. `-SkipCli` / `-SkipGuiBuild` reuse existing artifacts.
- `packaging/windows/verify_gui_package.py` - acceptance check for a staged or
  installed package. Parses the PE import table (no `strings` guessing) and
  fails on UCRT linkage, a missing GUI/runtime/QML file, or packaged
  `stage.new.*` leftovers. Works on a directory or a single executable.
- `packaging/windows/gui-probe.ps1` and `gui-capture.ps1` - launch a build,
  report mechanically whether a top-level window appears and for how long, and
  record one JSON line per attempt in
  `packaging/windows/diagnostics/gui-launch-log.jsonl`.
- `SWEEP_GRAB=<file.png>` in `packaging/qml/main.cpp` - the window renders its
  own scene to a PNG and exits, so "does the GUI really paint?" can be answered
  even when screen capture is defeated by occlusion, a locked session or RDP.
- `sweep.nsi`: `-DSTAGE=<dir>` override, compile-time `!error` guards that make
  a GUI-less installer impossible, the primary Start Menu entry now opens the
  GUI (`Sweep (CLI)` keeps the terminal tool), and install/uninstall clean up
  `stage.new.*` leftovers from earlier broken builds.
- `packaging/dist/windows/build-setup.sh` gained the same guards: Qt's MinGW
  13.1 is required (no silent fallback), compilers are pinned, the freshly built
  binary is checked, the stage publish cannot nest, and `makensis` receives the
  exact verified directory.

- Autostart & scheduled-tasks management, end to end. The `startup` command grew
  `rollback`, `history` and `export` alongside `list|enable|disable|remove|edit`
  (with `--kind|--scope|--category|--risk|--source|--search|--sort|--enabled|
  --disabled` filters applied *before* serialisation), a new `tasks` command
  covers Windows Task Scheduler tasks (and Unix cron/systemd timers), and
  `sweep startup history --to <path> --format json|csv` now writes the audit
  ledger to disk. Every entry carries a category, a machine-derived risk level
  (`critical|high|medium|low` — an unrecognised entry is never `low`), a
  trigger, a scope and `reversible|editable|removable` capability flags.
- Every mutating operation is **reversible and audited**: change order is
  policy check → backup (fail closed: no backup, no change) → apply → re-read
  and verify → append to the ledger. Rollback restores the previous state from
  the newest unconsumed backup of that entry (or the last change with
  `--last`); unverified applications are recorded as `unverified`, not `ok`.
  The append-only ledger (`audit.jsonl`, one JSON line per attempted
  operation) and the backups (`backups/<utc>-<id>.json`) live under
  `<config>/sweep/autostart`, or `<base>/autostart` in portable mode.
- New GUI screen **Startup & Scheduled Tasks** (`qml/AutostartScreen.qml`):
  one merged list of autostart entries and scheduled tasks with per-row
  category/risk/state/impact badges, scope, trigger and last run; instant
  search plus kind/scope/category/risk/state filters and name/source/impact
  sorting (single JS pass over the merged list, `ListView` row reuse); a detail
  panel with the full command, location, trigger, author, impact reason, notes
  and capability flags; per-item enable/disable/remove/edit/rollback and bulk
  enable/disable; a confirmation dialog that gates `critical`-risk items behind
  an explicit "I understand the risk" acknowledgement; an audit-history view
  (newest first, 100 by default) with JSON/CSV export and per-row rollback; and
  empty/error/loading states. The bridge passes filters as CLI flags — QML never
  builds a command line. Documented in `docs/autostart.md`.


## [0.4.0] - 2026-09-11

### Added

- Disk-dışı sistem zekası: 3 OS native memory opt, startup manager with
  impact analysis, sysinfo/SMART health, network cache flush, privacy shield.
  Concretely: `sweep memopt --aggressive --dry-run` (Linux `vm.drop_caches`
  1/3 plus a `swapoff`/`swapon` cycle, Windows `EmptyWorkingSet` plus a
  `NtSetSystemInformation` standby-list purge, macOS `purge` with
  `memory_pressure` analysis); `sweep startup list --impact` with high/medium/
  low impact estimates (XDG `.desktop` + `systemctl --user`, registry
  `Run`/`RunOnce` + Startup folders, launchd agents/daemons) and reversible
  disable (`Hidden=true` / `sweep-disabled` / `.disabled`); `sweep sysinfo
  --json|--health` (CPU, memory, disks, uptime, temperature and SMART with a
  green/yellow/red summary); `sweep network flush|clean` (DNS resolver cache,
  npm/pip/yarn/cargo/gem caches, `git gc --auto`, `docker network prune`); and
  `sweep privacy shield|wipe` (telemetry off, recent-files wipe, clipboard
  clear). Every new command supports `--dry-run`, reports the standard JSON
  shape (`duration_ms`, `bytes_affected`, `items_count`) and ships with unit
  tests; five new GUI screens (Memory, Startup, System health, Network,
  Privacy) are wired into the sidebar, the QML module and all 8 languages.
- Residue Intelligence Engine: context-aware leftover scanner
  (`sweep residue scan|preview|clean`) across Linux/Windows/macOS — uninstall
  residue (missing binary + orphan config), dead build output (`target/`
  without `Cargo.toml`, stale `.next/`, marker-less `__pycache__`/`.venv`),
  version caches and junk-only empty dirs, each with a confidence score
  (binary-gone 0.9, cache-only 0.7, empty 1.0) and a `safe_to_delete` flag.
  Documents/Desktop/Pictures are never scanned, `.git` trees are never
  touched, sub-0.6 findings stay preview-only, and Time Machine thinning
  runs through a dedicated `system.tm_thin` provider.
- 21 new cleaners (202 total): Windows Update Cache, Windows Defender,
  macOS Time Machine/Font/QuickLook, ComfyUI, Kaggle, wandb, MLflow,
  Proton-GE, GameMode, MangoHud, Looking Glass, VMware, QEMU, Distrobox,
  Toolbx, Flatpak Builder, Snapcraft, Paradox game logs and Coursier
  cache. VirtualBox was already covered; Automatic1111 was refused
  (install-relative layout, outputs are user media — no safe global
  path); Deno, Docker Desktop, Rocket.Chat, gcloud, Helix and NuGet
  were refused as duplicates of covered cleaners or single-path
  no-verify cases.
- Cross-platform `sweep schedule`: Windows Task Scheduler (`schtasks`) and
  macOS launchd (`~/Library/LaunchAgents/com.sweep.clean.plist` + `launchctl`)
  backends alongside systemd; bare `sweep schedule` now prints status, and a
  new Schedule screen in the Qt GUI manages hour/selection per backend.
- Smarter `sweep dupes`: `--keep first|newest|oldest|largest|smallest` selects
  the surviving copy per group (deterministic, unreadable files never win),
  and `--link` replaces duplicates with hard links to the kept copy via a
  rename-link-unlink sequence (cross-filesystem and staging collisions fail
  safe). Overlapping scan roots no longer list the same path twice.
- 3 new AI/dev cleaners (181 total): LM Studio, Open WebUI and Zed
  (HuggingFace and Ollama shipped earlier). Model weights, uploads and
  editor settings/extensions are explicitly kept.
- GUI i18n now covers all 8 languages (es/ru/fr/de/pt/it dictionaries added
  next to en/tr, including the new Schedule screen); Rust-side dictionaries
  keep full key parity (`hardlinked {}`, `sched_installed`, `sched_removed`).
- Linux: Nix store garbage collection (`nix` cleaner's new `gc` option) via
  `nix-collect-garbage -d`, with a read-only `nix-store --gc --print-dead`
  preview. Only unreachable store paths and old generations are removed.
- Linux: per-user systemd journal vacuum (`journalctl --user`, no root
  needed) as `linux_journal.user_vacuum`.
- Linux: dangling Docker volume removal (`docker volume prune`) as
  `linux_docker.volumes`; referenced/named volumes are never touched.
- Linux: openSUSE (zypper) support for old-kernel removal (`kernel-default`,
  `kernel-default-base`, `kernel-64k`) and orphan packages
  (`zypper packages --unneeded`).
- Linux: VS Code Remote-SSH/container server log and cached-data cleanup
  (`vscode.remote_server`); `system.user_caches` gained the `sccache` leaf.
- `sweep config` sub-command: `show`, `path`, `get KEY`, `set KEY VALUE` and
  `init`, so the TOML file no longer has to be edited by hand. `set` validates
  types and lists (comma-separated or JSON array) and writes atomically.
- `sweep info <cleaner>`: full definition detail — trust, source file, running
  check, variables, per-option warnings and every action with its filters.
- `sweep list --search TEXT`, `--os NAME` and `--running` filters.
- `--markdown PATH` report output next to `--html PATH`, for pasting results
  into issues, PRs and CI logs.
- `--remember` / `--last` on `preview` and `clean`: save a selection and replay
  it later. `remember_last = true` in the config does it automatically, which
  finally gives `last_selection` a purpose.
- `Report.duration_ms` is now measured (a `Stopwatch` in `core::report`) and
  appears in JSON, Markdown and the human summary.
- CI jobs that regenerate the completions and man page and fail when they drift
  from the CLI definition, plus a cleaner-id uniqueness/format check.
- The Windows/macOS CI matrix now runs `cargo test` instead of only
  `cargo check`, so POSIX-only assumptions in the suite are caught.
- Deep scan: `system.user_caches` gained leaves — `go-build`, `composer/cache`,
  `deno`, `paru`, `yay`, `JetBrains`, `Electron`, `ms-playwright`, and
  Brave/Opera/Vivaldi crash reports.
- Deep scan: Flatpak application caches (`~/.var/app/*/cache`) are now cleaned;
  `flatpak uninstall --unused` only removes idle runtimes, leaving installed
  apps' swollen caches behind.
- Deep scan: new `system.dev_go` option cleans `~/.cache/go-build` and
  `~/go/pkg/mod` (re-downloaded on the next `go build`).
- 20 new built-in cleaners: Anki, Calibre, Conda, Deluge, digiKam, Flutter/Dart,
  Geary, Gwenview, HandBrake, Java (deployment cache), Mailspring, mpv, Notion,
  Okular, Rustup, SDKMAN!, Shotwell, SumatraPDF, Terraform and XFCE. Each one
  targets only disposable data (cache, logs, temporary files, crash dumps);
  documents, media libraries, mail, decks and save files are explicitly kept.
- Recovered 6 cleaner files that already existed under `cleaners/` but were never
  wired into `BUILTIN_CLEANERS`, so they were silently unreachable: Battle.net,
  Epic Games Launcher, GOG Galaxy, GIMP, Inkscape and VLC. The embedded set now
  matches the files on disk (129 definitions).
- New `safari` cleaner (macOS only): page/favicon cache, history and cookies —
  the default macOS browser previously had no coverage. Tor Browser also gained
  its missing macOS profile path, and the stray Linux-only
  `~/.cache/mozilla/firefox` action in the `firefox` cleaner is now gated
  `os = "linux"`.
- 7 new deep cleaners: Ollama (interrupted `*.partial` pulls), Hugging Face
  (stale hub locks and `.no_exist` markers), Cursor and Windsurf (workspace and
  web caches, logs, remote-server logs), Ruby (cached `*.gem` archives), Figma
  (web/shader caches, logs, crash reports) and Bottles (per-bottle Windows
  temp). Model weights, installed gems, documents and bottle prefixes are
  explicitly kept. Deep extensions to existing cleaners: Discord (Code/GPU
  cache, Crashpad) and macOS profile paths, Telegram (dumps, temp), Zoom
  (avatars), Heroic (GPUCache), Minecraft (crash reports), Prism Launcher
  (now cross-platform: logs, crash reports, cache) and Snap (app caches,
  snapd cache).
- Unit tests for `core::report`, `fsutil::size`, `engine::worker::resolve`,
  `engine::logline` and the config field accessors; this round also adds
  coverage for `core::path`, the `winapp2` importer, and `fsutil::walk`
  (absolute glob roots are preserved, `has_glob` detection, and that the walker
  never follows a symlink out of its declared root).

### Changed

- The Qt GUI gained a `SWEEP_SCREEN=<route>` environment hook (`main.cpp`)
  plus `packaging/qml/smoke-wsl.sh`, so every route can be loaded headlessly
  with `QT_QPA_PLATFORM=offscreen` and QML errors surface on stderr. Without it
  the shell always opens on Dashboard and a display-less run could only ever
  validate one screen. Unset, behaviour is unchanged.
- **Windows SmartScreen no longer has to stay blocked.** The
  "Windows protected your PC" dialog is SmartScreen refusing an *unsigned*
  binary, not malware detection — and nothing in the source can change that,
  only a signature can. `packaging/windows/sign-dev.ps1` now creates a
  self-signed code-signing certificate, trusts it for the current user and
  authenticodes the built `sweep.exe` / `sweep-qml.exe`, so local builds launch
  without the dialog (`-Remove` undoes it). For public releases,
  `.github/workflows/release.yml` signs both binaries automatically when the
  `WINDOWS_SIGN_CERT_BASE64` and `WINDOWS_SIGN_CERT_PASSWORD` secrets are set,
  and otherwise still publishes while logging a notice that the binaries are
  unsigned.
- GUI redesign pass: removed the decorative noise that made the shell read as a
  demo rather than a tool — the row of six theme swatches in the top bar, the
  emoji navigation icons, the infinite logo/active-item glow pulses, the hover
  accent underline, the card gloss bands and the button shimmer sweep. The top
  bar now carries a single live status pill, navigation uses monochrome
  geometric glyphs, the three quick-action cards collapsed into one segmented
  card, the disk metric shows a progress bar instead of a one-point
  "sparkline", and both charts have a real empty state. Display copy is now
  consistently Turkish.
- Removed the phantom `gui` cargo feature from `ci.yml` and the `sweep-gui`
  build from `release.yml`: the iced GUI was deleted earlier in this cycle but
  the workflows still referenced it, so both were guaranteed to fail. Release
  archives now ship generated completions and the man page instead.
- `undo` and `history` share one log-line parser (`engine::logline`).
- Preview-first CLI workflow (`preview` → confirm → `clean`) with JSON, Markdown
  and standalone HTML report output.
- `system.temp_deep` scan depth raised from 4 to 8, and `~/.cache/tmp` added to
  its roots, so code and docs now agree.
- 129 built-in native TOML cleaner definitions under `cleaners/`, including
  Signal, Teams, Skype, Element, Mattermost, Evolution, Kdenlive, Git clients,
  Go/Yarn/pnpm toolchains, Sublime Text, Neovim, Obsidian, Joplin, Postman,
  Android Studio, Wine, Podman, Godot, Kodi, Dolphin, Minecraft, RetroArch,
  Unity, emulators (PCSX2/RPCS3/Ryujinx/yuzu/Cemu) and itch.io, plus PackageKit,
  CUPS and fwupd system caches and Lutris runner cleanup; and Unreal Engine,
  GameMaker, Krita, darktable, RawTherapee, DaVinci Resolve, Shotcut, Audacity,
  Eclipse, VSCodium, Atom, Geany, Lazarus, Arduino, PlatformIO, sbt, Composer,
  Poetry, Tor Browser, Falkon, AppImage, Nix, Guix, LXC, PulseAudio, clipboard
  history, APT logs, Samba, Postfix, Pidgin, Rhythmbox, X11, Vim and Bash.
- `dev_tools` now covers Maven, uv, Bun, Deno, Gradle daemon logs and Cargo git
  checkouts; `devscan` also matches manifest-guarded `out/`, `logs/` and
  `crash-reports/` project directories (Forge/Fabric run folders included).
- Disk-intelligence commands: `leftovers`, `devscan`, `dupes`, `bigfiles`,
  `analyze`, `deepscan`.
- Optional BleachBit CleanerML XML (`--bleachbit`) and `winapp2.ini`
  (`--winapp2`) import paths, loaded as untrusted definitions by default.
- `verify_tomls.py` static validator for embedded cleaner definitions. It now also
  cross-checks `cleaners/*.toml` against `BUILTIN_CLEANERS`: a definition that
  exists on disk but is missing from the registry is an error (it would be
  compiled away and silently unreachable), as is a duplicate or dangling
  `include_str!` entry. `builtin.rs` gained tests for duplicates, alphabetical
  ordering, empty sources and the recovered cleaners.
- `verify_qml.py`, the same idea for the Qt6/QML shell: every `qml/**/*.qml` must
  be listed exactly once in `CMakeLists.txt`'s `QML_FILES`, every custom
  component reference must resolve to a registered file, and brackets must
  balance. Wired into CI as the `qml-shell` job.
- 41 more built-in cleaners, picked from the tools people actually have
  installed: Emacs, Qt Creator, NetBeans, DBeaver, FileZilla, Remmina, Insomnia,
  Jupyter, RStudio, Spyder, Zotero, Transmission, aria2, yt-dlp, Syncthing,
  rclone, OneDrive, MEGAsync, Viber, WeChat, qutebrowser, Audacious, DeaDBeeF,
  Clementine, Strawberry, Plex, Jellyfin, OpenShot, gThumb, nomacs, Evince,
  zathura, ONLYOFFICE, WPS Office, Gnumeric, AbiWord, Evernote, KeePassXC,
  pyenv, CocoaPods and nginx. The embedded set is now 170 definitions.
- `docs/compatibility.md`'s cleaner matrix is generated from the files on disk
  (id, declared scope and description), so it can no longer drift from
  `cleaners/`.
- Podman/Docker packaging (`packaging/podman/Containerfile`, ~94 MB image) plus a
  reproducible BleachBit/Stacer/Sweep comparison harness under `packaging/bench/`.
- Qt6/QML shell over the `--json` CLI (`packaging/qml/`): async bridge with
  cancel, live sysinfo, neon theme set, donut/waveform canvas charts.
- Removed the iced GUI (`src/ui`, `sweep-gui` binary, `gui` feature): the Qt6/QML
  shell is now the only graphical interface.

- Removed dead code found by an unused-public-item sweep: `APP_NAME`/`PROJECT_NAME`,
  `action::check_cleaner` (an exact duplicate of `CleanerDef::validate`),
  `action::run_option`, `ActionFilter::is_trivial`, `provider::lookup_for`,
  `Guard::would_skip`, `CleanerDef::is_usable`, `CleanerRegistry::split_selector`
  and `running_cleaners`, `managers::installed_cleaners`,
  `cleanerml::load_cleanerml_dir`, `size::path_size`, `walk::for_each_child`, and
  the unused `freespace::clean_orphans`/`wipe_or_trim` wrappers. Nine platform
  helpers (`macos::{system_caches,log_dirs,is_apple_junk,saved_state_dir,quicklook_cache}`,
  `windows::{delete_on_reboot,shell_refresh_dir,update_uninstall_dirs}`,
  `linux::has_resolved`) are still unwired; they are left in place rather than
  deleted so the capability is not silently dropped.

### Security

- Protected-path and keep-list guards fail closed on destructive operations.
- External definitions lose `process` and `winreg` actions unless
  `--trust-external` is passed explicitly.
- `deep::safety::is_critical` normalises `..` components *lexically* before the
  critical-directory allowlist check, closing a bypass where a path such as
  `cleanable/sub/../../etc/passwd` could resolve outside the allowlist. A
  regression test (`dotdot_symlink_target_stays_critical`) guards it.
- Every filesystem action path must now be absolute: relative paths (or `winapp2.ini`
  `FileKey`s whose variable is undefined) are dropped instead of being resolved
  against the process working directory — see the entry under *Fixed*.
- Glob expansion keeps its declared root on every platform: `fsutil::walk` now
  preserves an absolute drive-letter prefix on Windows (see the entry under
  *Fixed*), so a cleaner glob can never be redirected at the process CWD.

### Fixed

- **The crate did not compile off Linux.** The `#[cfg(windows)]` and
  `#[cfg(target_os = "macos")]` branches had never been type-checked, so
  `cargo check` failed outright on Windows (six errors) and on macOS (one).
  Windows: `Option::copied` used on a plain `&Option<T>` (`cli/mod.rs`), and in
  `engine/startup.rs` the winreg 0.52 API (`value_names()` does not exist;
  `create_subkey` returns a `(RegKey, RegDisposition)` pair) plus a
  move-then-borrow inside a `format!`. macOS: `platform::trim()` reached for
  `Error::msg` without importing `Error`. All three targets now type-check
  cleanly, tests included.
- **`--keep largest`/`smallest` had an undocumented tie-break.** Duplicates are
  identical content by definition, so their allocated sizes tie almost every
  time — meaning the tie-break, not the size, was deciding which copy survives.
  Nothing stated what that tie-break was, and the test asserted an older
  "lexical rule" intent the code never implemented, so `cargo test` was red.
  The contract is now written on `KeepPolicy` (Largest falls back to the newest
  mtime then reverse-lexical; Smallest to the oldest then lexical) and the test
  pins it. **No behaviour changed** — the same copy survives as before.
  `cargo test --locked` is 206 passed / 0 failed.
- **The GUI crashed on any machine without a working OpenGL driver.** Qt's
  `win64_mingw` build ships no ANGLE (`libEGL`/`libGLESv2` are absent), so Qt
  Quick depends on the machine's native OpenGL. Where that driver is missing or
  broken Qt Quick cannot create a context, raises `qFatal`, and the process dies
  with an access violation inside `ntdll` — with no message, because a Windows
  subsystem app has no console. The GUI now renders with the software scene
  graph by default; hardware acceleration is opt-in via `SWEEP_GPU=1` (which
  still probes for a GL context and falls back if it cannot get one). Safe side
  first, because a probe can report a context that then fails to render: a
  false negative means the app never opens, a false positive only costs a
  little speed. Building against Qt's MSVC variant (which does ship ANGLE)
  removes the dependency entirely and remains the preferred long-term fix.
- **The GUI opened a console window behind the window.** It was linked against
  the console subsystem; it now reports PE subsystem 2 (Windows GUI) while the
  CLI deliberately stays on 3. The fix is *not* `WIN32_EXECUTABLE` — that makes
  Qt link `Qt6EntryPoint`, which fails to link against MinGW-w64 16 (UCRT) with
  a Qt 6.8.1 `win64_mingw` build (`undefined reference to __imp___argc`). The
  subsystem is set directly instead, avoiding `Qt6EntryPoint` altogether.
- **The Qt GUI would not compile under Qt's QML compiler.**
  `qml/NeonCheckBox.qml` had two statements collapsed onto one line
  (`spacing: 11            Rectangle {`); Qt's `qmlcachegen` rejects that with
  "Expected token ','". Split, and the GUI now builds with Qt 6.8.1 on Windows.
- **The Windows installer shipped a GUI that could not start.** `sweep.nsi`
  packaged `sweep-qml.exe` and `qml/` but none of the Qt runtime that
  `windeployqt` produces, so an installed GUI died on a missing `Qt6Core.dll`.
  The installer now includes the DLLs and the plugin directories, and the
  finish page launches the GUI instead of a console.
- **The installed app showed no icon.** The icon is embedded in `sweep.exe`
  (via `build.rs`), but Windows renders a blank entry without a `DisplayIcon`
  registry value and a shortcut icon. `sweep.ico` is now installed alongside the
  binary, both Start Menu shortcuts point at it, and Add/Remove Programs has
  `DisplayIcon`.
- **Nine GUI strings were never translated.** The new screens looked up
  `g.close`, `g.cores`, `g.cpu`, `g.memory`, `g.open`, `g.refresh`, `g.swap`,
  `g.working` and `reclaimed` before those keys existed in the Qt Translator,
  and QML renders the raw key when a translation is missing — so the UI would
  have shown literal `g.working` text. All nine are translated in the eight
  languages now, and `verify_qml.py` gained a coverage check (negative-tested)
  so a missing key fails the verifier instead of reaching users.
- **The four new screens showed the wrong header subtitle.** Main.qml's
  `screenSubtitle` had no branch for `StartupMgr`, `SysInfo`, `Network` or
  `Privacy` (and still used the old `s.optimize` key for `Optimize`), so those
  screens fell through to the Settings copy — "Theme, transparency and motion" —
  in the top bar. Fixed, and `verify_qml.py` now asserts that every nav route
  loads, is titled and is subtitled (negative-tested), so a half-wired route
  fails the verifier instead of shipping.
- **Three latent test-harness bugs that kept `cargo test --locked` red on
  Windows.** The residue fixture's `tree()` helper never created a
  trailing-slash entry, because `Path::parent()` of `<root>/empty/` is `<root>`
  — so the "empty directory" test asserted on a directory that was never
  created. `residue_build_markers` compared `Path::display()` against
  `/`-separated suffixes, which cannot match on Windows. And
  `log_refuses_a_symlinked_path` trusted `symlink_file`'s `Ok(())` even though
  Windows reports success while creating nothing when the caller lacks
  `SeCreateSymbolicLinkPrivilege` — it now verifies the link really exists,
  which makes the assertion stricter rather than weaker. The suite is at
  205 passed / 1 failed; the remaining failure is a real keep-policy tie-break
  question, tracked in GATES.md G37.
- **`--force` no longer disables the protected-root set.** `Guard::new` took
  `cli.force` as its `allow_protected` argument, so `sweep clean --force` would
  delete `/etc`, `C:\Windows` or the home directory itself if a definition
  pointed there. The flag now only relaxes the running-application refusal; the
  immutable-root check has no override, and a test
  (`protected_roots_are_never_overridable`) pins that down.
- **The operation log was opened through symlinks.** Sweep runs as root for
  `apt`/`journald`/`/var/log/nginx`, while the log lives under the user's own
  data directory — so an unprivileged user could point
  `~/.local/share/sweep/operations.log` at `/etc/cron.d/x` and have root append
  attacker-influenced text to it. The log is now opened with `O_NOFOLLOW`
  (POSIX) plus an explicit reparse-point refusal (Windows), covered by
  `log_refuses_a_symlinked_path`.
- **Cleaner walks crossed filesystem boundaries.** `ScanOptions::same_filesystem`
  existed and was fully implemented, but nothing ever set it — so a recursive
  delete walked *into* any volume mounted under the target, and a USB stick at
  `~/.cache/usb` would be emptied along with the cache. Deletion walks (`delete`
  and the structured `ini`/`json` providers) and `glob_paths` now stay on the
  root's device; `same_filesystem_keeps_children_on_the_root_device` guards the
  failure mode where the device check silently matches nothing.
- The free-space wiper removed leftover fill files with a raw
  `std::fs::remove_file`, bypassing the audited delete path; it now goes through
  `DeleteOptions::lenient()`, which also tolerates a file that vanished between
  listing and removal.
- **Three QML components were never registered in the module.** `NeonCard.qml`,
  `NeonBadge.qml` and `Settings.qml` existed under `packaging/qml/qml/` but were
  missing from `qt_add_qml_module(... QML_FILES ...)`, so they were compiled
  away — the Dashboard's CPU/RAM/DISK tiles (`NeonCard`) and the entire Settings
  screen (`Main.qml` loads `Settings.qml`) could not resolve in a packaged build.
  All three are registered now, and `verify_qml.py` guards the whole class.
- `ChartComponents.onPaint` called `ctx.save()` and then returned early on the
  empty-data and one-point branches, leaving the canvas state stack unbalanced on
  every repaint. All branches now fall through to a single `ctx.restore()`.
- **Relative cleaner paths are now rejected everywhere.** A path such as
  `Temp\*.log` (or a `winapp2.ini` `FileKey1` with an undefined variable) was
  silently expanded against the process working directory, so `sweep clean`
  started from a different folder would delete a *different* set of files — and
  an imported `winapp2.ini` could smuggle one in deliberately. The single path
  funnel `definition::model::expand_action_paths` now drops non-absolute
  filesystem paths (logging a warning), `definition::path::expand_defined` lets
  importers refuse undefined variables instead of collapsing to a root-relative
  fragment, and `winapp2` rejects any non-absolute `FileKey` directory.
  `verify_tomls.py` enforces the same rule statically. The root check itself is
  now cross-platform: a path is considered rooted if it is absolute on *this* OS
  **or** matches the other platform's root syntax (`looks_absolute`), so a
  Linux-style `/etc/x` is no longer wrongly dropped on the Windows build (caught
  by a new unit test).
- **Glob expansion on Windows silently matched nothing for absolute drive-letter
  patterns.** `fsutil::walk::glob_paths` rebuilt the literal prefix of a glob such
  as `C:\Users\foo\*.log` as a *drive-relative* path (`C:Users\foo`) that
  resolves against the process CWD on that drive, so Windows cleaner globs with an
  absolute drive root walked the wrong directory (usually none). `split_glob_root`
  now re-attaches the separator and keeps the prefix absolute (`C:\Users\foo`). A
  regression test on `glob_paths` guards it.
- **`gimp` cleaner deleted the user's personal brushes.** The `cleaners/gimp.toml`
  cache option removed `~/.config/GIMP/2.10/brushes`, which holds user-installed
  brushes rather than regenerable cache data. That action was dropped now that the
  cleaner is registered; the filter/plugin cache and logs are still cleaned.
- **`winapp2` importer had a case-sensitivity bug that disabled every `Detect*`
  check and the whole `LangSecRef` → cleaner-id table.** Its INI reader preserved
  key case while every lookup was lower-case, so `LangSecRef`, `DetectFile` and
  `DetectOS` from real Winapp2 files never matched. Keys are now folded to lower
  case (values and section names keep their case), so Firefox sections map to
  `winapp2_mozilla` etc. and sections gated on a missing file are correctly
  skipped.
- `core::path::looks_absolute` now requires a real drive letter; `1:\foo` and
  `:C\foo` are no longer mistaken for rooted paths.
- `core::path` test coverage went from two Windows-skipping tests to twelve that
  run on every platform (expand variants, `..`-at-root clamping, component-wise
  containment, case switch, `looks_absolute`, `join_opt`, `extended_path`), so the
  safety-critical path module is exercised where it matters most.
- Built-in keep list no longer blocks dedicated cleaners: `firefox.cache` targets
  were silently skipped (benchmark-proven).
- Preview no longer double-counts directory trees (directory bytes were added on
  top of the individually listed children).
- `system.*` deletions report measured bytes instead of 0B on success.
- **MSRV corrected from 1.77 to 1.88.** The declared minimum was eleven releases
  too low: the locked dependency graph needs 1.88 (globset 0.4.20), and clap 4.6
  requires edition 2024 (1.85), which cargo 1.82 cannot even parse — so `cargo
  check --locked` on the old MSRV job could never succeed. `rust-toolchain.toml`
  (1.82 → 1.88), the README badge and CONTRIBUTING now agree, and the CI MSRV job
  reads `rust-version` from `Cargo.toml` instead of hard-coding a number that can
  drift. Verified: `cargo check --locked` and `cargo test --locked` both pass on
  rustc 1.88.0.
- `history` silently discarded operation-log lines whose *path* contains the `|`
  field separator (it required exactly five fields). Both `history` and `undo` now
  parse from the right through the shared `engine::logline` parser, so such paths
  are counted and restorable.
- `Report.duration_ms` was serialised but never set; `Stopwatch` plus
  `Report::finish()` now populate it.
- Markdown reports escape `|` inside code spans, so a path or label containing a
  pipe no longer breaks the table.
- `deep::safety::tests::critical_roots`, `deep::tests::guarded_delete_*` and
  `engine::undo::tests::pipe_in_path_parses` no longer fail on Windows: the
  critical-dir table is POSIX-only (Windows relies on `Guard::protected_roots`)
  and the builtin keep list protects the Windows temp root, which made their
  scratch directories undeletable.
- `engine::memopt` no longer warns on non-Linux builds (Linux-only imports and
  helpers are `cfg`-gated), and `src/definition/cleanerml.rs` had an orphan doc
  comment that tripped `clippy::empty_line_after_doc_comments`.
- **Distribution packages (Qt6/QML shell):** emoji rendering was broken because
  `QQuickWindow` has no `font` property, so assigning `font.family` crashed Qt
  (`Cannot assign to non-existent property "font"`). The global font chain
  (`Noto Sans, Noto Color Emoji, …`) is now set in `main.cpp` via `app.setFont()`;
  `Main.qml` is untouched and emoji render correctly. `sweep memopt` now prompts
  for the sudo password in the GUI (`SweepBridge::memoptWithSudo(password)` →
  `sudo -S`) instead of failing when root is required.

## [0.1.0] - 2026-09-05

Initial public snapshot of the Rust rewrite.

[Unreleased]: https://github.com/sweep-cleaner/sweep/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/sweep-cleaner/sweep/compare/v0.1.0...v0.4.0
[0.1.0]: https://github.com/sweep-cleaner/sweep/releases/tag/v0.1.0
