<div align="center">

# 🧹 Sweep

### Fast, safe, cross-platform system cleaner for the command line.

*Rust rewrite inspired by [BleachBit](https://www.bleachbit.org) — with deeper Linux coverage, a real preview-first workflow, and a reusable library.*

<br/>

[![CI](https://img.shields.io/github/actions/workflow/status/sweep-cleaner/sweep/ci.yml?style=for-the-badge&logo=githubactions&logoColor=white&label=CI)](https://github.com/sweep-cleaner/sweep/actions/workflows/ci.yml)
[![License: GPL v3](https://img.shields.io/badge/License-GPL--3.0--or--later-blue?style=for-the-badge&logo=gnu&logoColor=white)](COPYING)
[![Rust 1.88+](https://img.shields.io/badge/rust-1.88%2B-orange?style=for-the-badge&logo=rust&logoColor=white)](rust-toolchain.toml)
[![Platforms](https://img.shields.io/badge/platforms-Linux%20%7C%20macOS%20%7C%20Windows-2ea44f?style=for-the-badge)](#-platforms)
[![Built-in cleaners](https://img.shields.io/badge/cleaners-109-7c3aed?style=for-the-badge)](cleaners/)

<br/>

[**Features**](#-features) · [**Install**](#-installation) · [**Quick start**](#-quick-start) · [**CLI tour**](#-cli-tour) · [**Screenshots**](#-screenshots) · [**Compare**](#-how-it-compares) · [**Docs**](docs/) · [**Contributing**](CONTRIBUTING.md)

<br/>

```text
$ sweep preview --all --exclude system.trash
  firefox.cache            412 files    1.8 GB
  firefox.history          1   file     12  MB
  discord.cache            27  files    340 MB
  apt.deep_cache           184 packages  3.1 GB
  ────────────────────────────────────────
  Total reclaimed (est.)                5.2 GB

  Proceed? [y/N] y
  ✓ 412 files removed · 0 errors · log: ~/.local/share/sweep/log.jsonl
```

</div>

---

## ✨ Features

| Area | What you get |
| --- | --- |
| **Built-in cleaners** | **109** native TOML definitions covering system files, Firefox, Chrome/Chromium, Discord, Signal, Teams, Skype, Element, Mattermost, Evolution, VS Code, Sublime, Neovim, Obsidian, Joplin, Postman, JetBrains, LibreOffice, Blender, Kdenlive, Godot, OBS, Kodi, Steam, Lutris, Heroic, RetroArch, Dolphin, Minecraft, Spotify, Telegram, Thunderbird, Nextcloud, qBittorrent, Zoom, Slack, Prism Launcher, VirtualBox, Dropbox, Wine, Git clients, Go/Yarn/pnpm, Android Studio, Podman, PackageKit, Unity, Unreal, emulators, itch.io, Blender, Krita, Darktable, DaVinci, Kdenlive, Shotcut, Audacity, Eclipse, VSCodium, Atom, Geany, Lazarus, Arduino, PlatformIO, sbt, Composer, Poetry, Tor Browser, Falkon, WhatsApp, Pidgin, Rhythmbox, Vim, media, thumbnails, fonts, and more. |
| **Deep Linux cleaning** | Package caches (`apt`, `dnf`, `pacman`, `zypper`), old kernels & orphans on apt/dnf/pacman/zypper, system + per-user journal vacuuming, Nix store GC, Snap / Flatpak / Docker (images, build cache, logs, dangling volumes) cleanup, KDE & GNOME (Baloo, Tracker, Akonadi), VS Code remote-server data. |
| **Disk intelligence** | `leftovers` (orphaned app data), `devscan` (regenerable dev cruft), `dupes` (size + content hash), `bigfiles`, `du` (per-folder sizes, read-only), `analyze` (read-only treemap). |
| **Preview-first workflow** | Every destructive command **previews → confirms → logs**. Backups via `--backup-dir`. Single-command `undo` for recent cleanups. |
| **Multiple action types** | Filesystem, structured-file (JSON/YAML/INI), SQLite VACUUM, system (services, journal, package managers), Windows registry, and external processes — guarded by default. |
| **Compatibility layer** | Optional BleachBit `CleanerML` XML and `winapp2.ini` loading (community rules via `sweep hub`). |
| **Reports** | Human-readable, machine-readable JSON, standalone **HTML** and **Markdown** reports for any command, with wall-clock duration. |
| **Embeddable** | A reusable `sweep` Rust library (`use sweep::…`) powers the `sweep` binary. |
| **Optional GUI** | Qt6/QML shell over the `--json` CLI (`packaging/qml/`): 11 themes, animated dashboard (aurora, glass cards, donut/wave charts), confetti on clean, system tray + notifications, Windows 11 Mica & dark title bar, UAC-elevated cleaning, per-core CPU charts, persistent settings. |
| **Security & safety** | Protected-path and keep-list guards, secure-deletion (`shred`) and free-space-wiping (`wipe`) primitives, fail-closed defaults. |
| **Scheduling** | First-class systemd user timer (`sweep schedule --enable`). |
| **Startup & tasks** | Unified autostart + scheduled-task management (`sweep startup`, `sweep tasks`) with risk classification, reversible changes, backups and an append-only audit ledger. |
| **Portability** | `--portable /mnt/usb` keeps config + log + hub in one folder. |

---

## 📦 Installation

<div align="center">

| Method | Command |
| --- | --- |
| **Arch (AUR)** | `yay -S sweep` (also `packaging/aur`) |
| **Fedora (COPR)** | `dnf copr enable @sweep/sweep && dnf install sweep` |
| **Debian / Ubuntu** | package build in `packaging/debian` |
| **Flatpak** | `flatpak-builder --install packaging/flatpak/org.sweep.Sweep.json` |
| **Podman / Docker** | `podman build -t sweep -f packaging/podman/Containerfile .` |
| **From source** | `cargo build --release` |
| **Binaries** | Pre-built for Linux / Windows / macOS on [Releases](https://github.com/sweep-cleaner/sweep/releases) |

</div>

```sh
# Build the CLI:
git clone https://github.com/sweep-cleaner/sweep
cd sweep
cargo build --release

# Add the GUI (Qt6/QML shell, needs a display):
podman build -t sweep-qml -f packaging/qml/Containerfile.qml .
```

---

## 🚀 Quick start

```sh
# 1. See what you'd clean (nothing is deleted):
sweep preview firefox.cache apt.deep_cache

# 2. List the biggest space hogs:
sweep bigfiles --min-size 500MB --top 20

# 3. Hunt leftovers of an uninstalled app:
sweep leftovers discord --preview   # then --clean when ready

# 4. Schedule a weekly, unattended cleanup:
sweep schedule --enable --hour 3 -- system.journal_vacuum apt.deep_cache
```

> [!WARNING]
> Cleaning is destructive. Preview a narrowly selected operation first, close applications that own the target files, and keep backups of important data. Removing browser history or cookies can log you out and remove privacy data. Package-manager, registry, external-process, system-cache, shredding, and free-space operations can have broader effects. **`wipe` and secure deletion are irreversible** and cannot guarantee recovery resistance on snapshots, backups, journals, copy-on-write filesystems, SSD/NVMe over-provisioning, or other storage layers.

---

## 🧭 CLI tour

### Discover

```sh
sweep list                      # every available cleaner
sweep list firefox              # one cleaner's options
sweep list --search browser     # filter by id / name / description
sweep list --os linux           # only cleaners that apply to a given OS
sweep list --running            # only cleaners whose app is live right now
sweep info firefox              # full definition: options, actions, paths
sweep providers                 # platform-filtered action providers
sweep diagnostics               # runtime diagnostics
sweep doctor                    # overall system health
```

### Configuration

```sh
sweep config path               # where the config file lives
sweep config show               # every setting with its current value
sweep config get shred
sweep config set journal_days 14
sweep config init               # write a default config file
```

### Preview → Clean

```sh
sweep preview firefox.history
sweep preview --all --exclude system.trash
sweep clean   firefox.history
sweep clean   firefox.cache apt.deep_cache --remember   # remember selection
sweep clean   --last                                    # replay it later

sweep --json preview firefox.history              # machine-readable
sweep preview --all --html /tmp/report.html       # standalone HTML
sweep preview --all --markdown /tmp/report.md     # paste into an issue/PR
sweep --config /etc/sweep.toml preview --all      # explicit config
```

### Disk intelligence

```sh
sweep bigfiles --min-size 500MB --top 20
sweep bigfiles ~/Downloads --older-than 90d --clean
sweep dupes      ~/Pictures --clean --min-size 500KB
sweep devscan    ~/projects  --clean
sweep leftovers  discord
sweep analyze    ~ --top 15
sweep deepscan   /tmp/junk-tree --preview
```

### System & services

```sh
sweep startup list --risk critical        # autostart entries + logon/boot tasks
sweep startup disable "Some App"          # reversible: backed up, then logged
sweep startup rollback --last             # restore the previous state
sweep tasks list                          # scheduled tasks (Task Scheduler / cron)
sweep startup history --limit 20          # append-only audit ledger
sweep services list --user
sweep services restart pipewire.service --user
sweep shred ~/secrets.txt                        # overwrite then delete
sweep wipe  /mnt/data                            # free-space operation
```

### Snapshots, history, scheduling

```sh
sweep snapshot create "before-cleanup"
sweep snapshot list
sweep history --days 30
sweep schedule --enable --hour 3 -- system.journal_vacuum apt.deep_cache
sweep watch --threshold 90 --interval 300         # daemon
sweep watch --once                               # exit 3 on alarm (cron-friendly)
```

### Community rules

```sh
sweep hub update
sweep hub list
sweep hub install gaming
sweep import path/to/cleaner.toml                # validate a definition
```

### Portable & scripted

```sh
sweep --portable /mnt/usb/sweep preview --all    # everything in one folder
sweep --yes  clean firefox.cache                 # non-interactive
sweep --quiet clean --all                       # no progress bars
```

### Shell integration

```sh
sweep completions bash > /etc/bash_completion.d/sweep
sweep completions zsh  > "${fpath[1]}/_sweep"
sweep completions fish > ~/.config/fish/completions/sweep.fish
sweep man --output /usr/local/share/man/man1     # writes sweep.1
```

Pre-generated copies live in [`packaging/completions`](packaging/completions) and [`packaging/man`](packaging/man); CI fails when they drift from what the binary actually emits.

> `sweep wipe PATH` is a **free-space operation on the volume containing `PATH`** — not a preview. See [docs/compatibility.md](docs/compatibility.md) before using it.
> `sweep deepscan ROOT` is destructive unless `--preview` is supplied.
> `leftovers`, `devscan`, and `dupes` are **list-first**: they only report matches until `--clean` is given; `--clean` still shows the preview and asks for confirmation (pass `--yes` for scripts). Duplicates keep the first file of each group; `devscan` only touches regenerable directories (`node_modules`, `target`, …).

### Selectors & exclusions

| Form | Meaning |
| --- | --- |
| `*` / `all` | every active option |
| `cleaner` | every active option for one cleaner |
| `cleaner.option` | one option |
| `fire*` | case-insensitive cleaner glob |

`--all` is equivalent to selecting every active option when the command supports it. `--exclude` removes matching selections from the resolved set — prefer exact exclusions like `--exclude system.trash` for destructive cleanups.

### Global flags

| Flag | Purpose |
| --- | --- |
| `--config PATH` | Use a specific configuration file. |
| `--shred` | Overwrite files before deletion (overrides config). |
| `--force` | Allow protected-system-path operations and bypass running-app refusal. Keep-list is still respected. |
| `--trust-external` | Allow external definitions to use privileged `process` and `winreg` actions. |
| `--bleachbit` | Also load BleachBit CleanerML definitions from standard locations. |
| `--winapp2` | Also load and translate `winapp2.ini`. |
| `--define PATH` | Load an additional cleaner-definition directory (repeatable). |
| `--json` | Machine-readable JSON instead of human summary. |
| `--html PATH` | Also write a standalone HTML report. |
| `--markdown PATH` | Also write a Markdown report (CI / issue friendly). |
| `--quiet` | Suppress progress bars. |
| `--debug` | Enable debug logging. |

`preview` and `clean` additionally accept `--last` (reuse the selection saved by a previous `--remember` run) and `--remember` (save the selection; `remember_last = true` in the config does it automatically).

Definitions loaded from disk are **untrusted by default**: in that mode `process` and `winreg` actions are removed before execution. `--trust-external` changes that boundary — it does **not** make a definition safe, so inspect its source and every path/action before enabling it.

---

## 🖼 Screenshots

The GUI is a Qt6/QML shell over the `--json` CLI. Run today with:

```sh
podman build -t sweep-qml -f packaging/qml/Containerfile.qml .
podman run --rm -e DISPLAY="$DISPLAY" -v /tmp/.X11-unix:/tmp/.X11-unix:ro sweep-qml
```

Shipped today: dashboard with live CPU/RAM/disk cards, reclaim-share donut, cleaner tree with per-option size previews, and a system-details panel. ASCII preview:

```text
┌─ Sweep ───────────────────────────────────────────────────────┐
│  Disk  ▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓░░░░░░░  72%  /  1.1 TB free of 3.8 TB  │
│                                                                │
│  ▾ Browsers                                  2.1 GB reclaimable │
│      ▸ Firefox                  [cache ✓] [history ☐] [cookies☐]│
│      ▸ Chromium                 [cache ☐]                       │
│  ▾ System                                    4.7 GB reclaimable │
│      ▸ apt                      [deep_cache ✓] [orphans ☐]     │
│      ▸ journalctl               [vacuum ✓]                      │
│  ▸ Dev · Media · Office · Chat · Games                          │
│                                                                │
│  [ Preview ]   [ Clean selected (2) ]   [ Undo last ]           │
└────────────────────────────────────────────────────────────────┘
```

---

## ⚖️ How it compares

| Capability | BleachBit | Stacer | Sweeper | **Sweep** |
| --- | :---: | :---: | :---: | :---: |
| Package caches (apt / dnf / pacman) | ✅ | partial | ❌ | ✅ per-file size preview |
| Old kernels / orphan packages | partial | ❌ | ❌ | ✅ |
| Journal / Snap / Flatpak / Docker | ❌ | ❌ | ❌ | ✅ |
| KDE / GNOME caches (Baloo, Tracker, Akonadi) | shallow | ❌ | shallow | ✅ deep |
| Leftover hunter (`leftovers`) | ❌ | ❌ | ❌ | ✅ with package-ownership guard |
| Dev cruft / duplicates / big files | ❌ | partial | ❌ | ✅ `devscan`, `dupes`, `bigfiles` |
| Dry-run + confirm + backup + log | partial | partial | basic | ✅ + `undo` |
| Scheduled cleanup | ❌ | partial | ❌ | ✅ systemd user timer |
| GUI stack | GTK | Electron | Qt | Qt6/QML |
| `winapp2.ini` community rules | ✅ | ❌ | ❌ | ✅ `sweep hub` |

---

## ⚙️ Configuration

TOML. Field names match `Config` in `src/config/mod.rs`:

```toml
shred = false
languages = ["en", "tr"]
whitelist_files = ["~/Documents/keep.txt"]
whitelist_folders = ["~/Projects/important"]
custom_paths = ["~/tmp/disposable-cache"]
deepscan_roots   = ["~/tmp/disposable-cache"]
deepscan_patterns = ["*.tmp", "*.log", "Thumbs.db"]
remember_last = false
last_selection = ["firefox.cache"]

# Deep-clean budgets (system.journal_vacuum, pacman_cache, temp_deep, docker):
journal_size        = "500M"
journal_days        = 7
pacman_keep         = 2
temp_max_age_days   = 1
docker_build_cache  = false

# Backup + log defaults (CLI flags override these):
backup_dir = ""
log_file   = ""
```

The interface language follows `languages[0]` (supported: `en`, `tr`, `es`, `ru`, `fr`, `de`, `pt`, `it`), falling back to `$LANG`. Missing keys fall back to English — see `src/i18n.rs` to add a language. Both CLI and GUI translate through the same dictionary (change it live in **Settings**).

You never have to edit the file by hand:

```sh
sweep config path                  # where it is
sweep config show                  # every setting + value
sweep config get shred             # one value (script-friendly)
sweep config set shred true        # validated write (atomic replace)
sweep config set languages tr,en   # lists: comma-separated or a JSON array
sweep config init                  # write the defaults
```

`remember_last = true` makes every `clean` save `last_selection`, which `sweep clean --last` then replays. A one-off `sweep clean … --remember` saves it without touching the config.

> ℹ️ A missing file falls back to defaults. If the configured file is unreadable or malformed, the CLI logs a warning and uses default settings. Use `--config PATH` when the exact location matters.

---

## 🛡 Safety model

The engine is designed to **fail closed** in common dangerous cases:

- `Guard` combines the user keep list with built-in protected-path conventions.
- Protected system roots, important user-data roots, and sensitive locations are **not** ordinary cleanup targets.
- Traversal does **not** follow symbolic links or Windows reparse points by default; mount points and links are treated conservatively.
- Running-application checks can skip cleaners while a target application is active. `--force` bypasses that refusal — close applications instead.
- Preview reports intended entries and estimated sizes without intentionally changing ordinary filesystem targets.
- Individual failures are recorded in the report instead of aborting every unrelated operation.
- External definitions cannot run processes or edit the Windows registry unless the explicit `--trust-external` override is supplied.

These protections are not a substitute for backups or review. There are still races between preview and execution, permissions can change, and some system-level actions are necessarily broader than ordinary path deletion.

---

## 🖥 Platforms

Platform implementations exist for **Linux**, **macOS**, **Windows**, and generic Unix/BSD systems. Providers and cleaner options are filtered by platform, and some operations additionally require an installed helper, a running system service, or elevated privileges.

Source layout:

| Path | Responsibility |
| --- | --- |
| `src/core` | Errors, paths, reports, keep/protection rules. |
| `src/fsutil` | Traversal, sizes, deletion, free-space operations. |
| `src/shred` | Overwrite, random rename, truncation, shredding. |
| `src/platform` | Windows / Linux / macOS / Unix abstractions. |
| `src/definition` | Native TOML, CleanerML XML, `winapp2.ini` loading. |
| `src/action` | Filesystem, structured, SQLite, system, registry, process actions. |
| `src/engine` | Selection, workers, deep-scan, leftovers, devscan, dupes, bigfiles, schedule, undo. |
| `src/deep` | One module per Linux deep-clean category (`apt` … `zeitgeist`). |
| `packaging/qml` | Qt6/QML GUI shell over the `--json` CLI. |
| `src/config` | Persisted cross-cleaner configuration. |
| `cleaners/` | 202 native TOML definitions embedded via `include_str!`. |
| `packaging/` | AUR, COPR, Debian, Flatpak, systemd units. |
| `docs/` | Cleaner format, compatibility, overview, benchmarks. |

> `cleaners/` covers system files, browsers (Firefox + forks, Chromium-based), Discord, dev tools, VS Code, JetBrains, LibreOffice, media apps, Steam, Lutris, Heroic, OBS, Spotify, Telegram, Thunderbird, Nextcloud, qBittorrent, Zoom, Slack, Prism Launcher, VirtualBox, Dropbox, thumbnails, fonts, and package managers. The `verify_tomls.py` script statically checks TOML structure, accepted schema tokens, registered commands, and dispatcher reachability without compiling Rust.

---

## ✍️ Defining cleaners

Native cleaner definitions are TOML. The authoritative authoring guide is [**docs/cleaner-format.md**](docs/cleaner-format.md). A definition contains:

- a cleaner identity,
- optional running checks and path variables,
- options, and
- one or more actions per option.

CleanerML XML and `winapp2.ini` are loaded as optional external formats and normalized into the same internal model — review imported definitions for provenance, path breadth, and privileged actions.

---

## 🤝 Contributing

PRs welcome — please read [**CONTRIBUTING.md**](CONTRIBUTING.md) first. Every PR that touches deletion paths must tick the safety checklist in the PR template, keep `verify_tomls.py` green, and update [**CHANGELOG.md**](CHANGELOG.md). By participating you agree to the [**Code of Conduct**](CODE_OF_CONDUCT.md).

- 🐛 **Bug reports** → use the issue templates
- 🔒 **Security issues** → see [**SECURITY.md**](SECURITY.md) — *never* open a public issue
- 🌐 **Translations** → see `src/i18n.rs`; supported today: `en`, `tr`

---

## 📚 Documentation

- [Changelog](CHANGELOG.md)
- [Cleaner TOML format](docs/cleaner-format.md)
- [Platform & cleaner compatibility](docs/compatibility.md)
- [Project overview](docs/overview.md)
- [Benchmarks](docs/BENCHMARKS.md)
- [Contributor & safety workflow](CONTRIBUTING.md)
- [Licensing & third-party provenance](THIRD_PARTY.md)
- [GPL-3.0-or-later license text](COPYING)

---

## ⭐ Star history

If Sweep saved you disk space, consider giving it a star — it helps others find the project.

<a href="https://star-history.com/#sweep-cleaner/sweep&Date">
  <picture>
    <img alt="Star History Chart" src="https://api.star-history.com/svg?repos=sweep-cleaner/sweep&type=Date" />
  </picture>
</a>

---

## 📄 License

**Sweep** is free software under the [**GPL-3.0-or-later**](COPYING) license.

<div align="center">

<sub>Made with 🧹 by the Sweep contributors — independent implementation, not affiliated with the BleachBit project.</sub>

</div>
