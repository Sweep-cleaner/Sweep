# Compatibility and platform notes

This page is a source-derived compatibility guide for the current
Sweep implementation. It is not a claim that every provider has been
runtime-tested on every operating system or distribution. A provider can be
compiled into the binary but still be unavailable because of OS filters,
missing helper programs, permissions, service state, or an application-specific
schema change.

## Embedded cleaner matrix

The current source embeds 202 native TOML definitions through
`src/definition/builtin.rs`:

| Cleaner ID | Declared scope | Main coverage |
| --- | --- | --- |
| `abiword` | Mixed; paths are OS-filtered | AbiWord word processor: cache and logs (your documents are kept). |
| `anki` | Mixed; paths are OS-filtered | Anki flashcards: webview cache and collection log files (decks and media are kept). |
| `appimage` | Linux | AppImage runtime cache (applications are kept). |
| `apt` | Linux | Debian/Ubuntu package manager cache. |
| `aptlogs` | Linux | Package manager history logs (package lists and cache are kept). |
| `arduino` | Mixed; paths are OS-filtered | Arduino IDE staging downloads, cache and logs (sketches and libraries are kept). |
| `aria2` | Mixed; paths are OS-filtered | aria2 downloader: log files and the cached DHT routing table. |
| `atom` | Mixed; paths are OS-filtered | Atom editor cache, logs and crash dumps (packages and config are kept). |
| `audacious` | Mixed; paths are OS-filtered | Audacious music player: playlist cache and logs (your music files are kept). |
| `audacity` | Mixed; paths are OS-filtered | Audio editor temp files and logs (projects and macros are kept). |
| `bash` | Unix | Shell command history. |
| `battlenet` | Mixed; paths are OS-filtered | Blizzard oyun başlatıcısı günlükleri ve önbelleği. |
| `blender` | Linux | 3B düzenleyici önbelleği ve son dosya listesi (projeler korunur). |
| `bottles` | Linux | Windows compatibility bottles: per-bottle Windows temp files (prefixes, games and saves are kept). |
| `browsers` | Mixed; paths are OS-filtered | Chrome, Chromium, Edge, Brave, Vivaldi and Opera: cache, code cache, cookies and history. |
| `calibre` | Mixed; paths are OS-filtered | Calibre ebook manager: caches and logs (your book library is never touched). |
| `clementine` | Mixed; paths are OS-filtered | Clementine music player: stream and cover-art cache plus logs (library is kept). |
| `clipboard` | Linux | Clipboard manager history (current clipboard is kept). |
| `cocoapods` | Mixed; paths are OS-filtered | CocoaPods: downloaded pod cache and spec repositories (re-fetched by pod install). |
| `comfyui` | Mixed; paths are OS-filtered | ComfyUI temp files (models and renders are kept). |
| `composer` | Mixed; paths are OS-filtered | PHP package cache (projects and auth tokens are kept). |
| `conda` | Mixed; paths are OS-filtered | Anaconda/Miniconda package cache (the equivalent of `conda clean --all`). |
| `coursier` | Mixed; paths are OS-filtered | Coursier/Scala artifact cache (re-fetched on next build). |
| `cursor` | Mixed; paths are OS-filtered | Cursor AI editor: workspace cache, web caches and logs (projects, settings and extensions are kept). |
| `darktable` | Mixed; paths are OS-filtered | Photo editor mipmap cache (library database and edits are kept). |
| `davinci` | Mixed; paths are OS-filtered | Video editor logs and render cache (projects and galleries are kept). |
| `dbeaver` | Mixed; paths are OS-filtered | DBeaver database client: workspace logs, metadata and cache (connections are kept). |
| `deadbeef` | Mixed; paths are OS-filtered | DeaDBeeF music player: cover-art and media cache plus logs (your music is kept). |
| `deluge` | Mixed; paths are OS-filtered | Deluge BitTorrent client: log files (torrents, settings and resume data are kept). |
| `dev_tools` | Mixed; paths are OS-filtered | Caches and temporary data from common developer toolchains. |
| `digikam` | Linux | digiKam photo manager: thumbnail and face-detection caches (your photos are never touched). |
| `discord` | Mixed; paths are OS-filtered | Discord chat client: cache and downloaded data. |
| `distrobox` | Linux | Distrobox container home caches (containers are kept). |
| `dnf` | Linux | Fedora / RHEL package manager cache. |
| `dolphin` | Mixed; paths are OS-filtered | Wii/GameCube emulator: shader cache and logs (saves, states and ISOs are kept). |
| `dropbox` | Mixed; paths are OS-filtered | Bulut istemcisi günlükleri (eşleştirilen dosyalar korunur). |
| `eclipse` | Mixed; paths are OS-filtered | Eclipse IDE logs and cache (workspaces and plugins are kept). |
| `element` | Mixed; paths are OS-filtered | Element Matrix client: cache, logs and crash dumps (login and room data are kept). |
| `emacs` | Mixed; paths are OS-filtered | Emacs: native-compilation and package caches (your files and settings are kept). |
| `emulators` | Mixed; paths are OS-filtered | Game emulators: logs and regenerable shader caches only (saves, states, ROMs, firmware and configs are never touched). |
| `epicgames` | Mixed; paths are OS-filtered | Epic Games mağaza başlatıcısı günlükleri ve önbelleği. |
| `evernote` | Windows | Evernote desktop: cache, logs and crash reports (your notes are kept). |
| `evince` | Linux | Evince document viewer: page and thumbnail cache (your documents are never touched). |
| `evolution` | Linux | GNOME mail client: disposable cache only (local mail in ~/.local/share/evolution is never touched). |
| `falkon` | Linux | KDE browser cache (bookmarks and settings are kept). |
| `figma` | Mixed; paths are OS-filtered | Figma desktop app: web caches, shader caches and logs (documents sync from the cloud). |
| `filezilla` | Mixed; paths are OS-filtered | FileZilla FTP client: logs and cache (sites and the transfer queue are kept). |
| `firefox` | Mixed; paths are OS-filtered | Mozilla Firefox web browser: cache, history, cookies and thumbnails. |
| `firefox_forks` | Mixed; paths are OS-filtered | LibreWolf, Zen, Waterfox, Pale Moon, Tor Browser, Floorp: önbellek, çerez ve geçmiş. |
| `flatpak_builder` | Linux | flatpak-builder build cache (manifests are kept). |
| `flutter` | Mixed; paths are OS-filtered | Dart and Flutter pub cache and tool state (downloaded packages and analytics). |
| `fonts` | Unix | Rebuild the font cache. |
| `gamemaker` | Mixed; paths are OS-filtered | GameMaker Studio cache and temp files (projects are kept). |
| `gamemode` | Linux | GameMode daemon cache. |
| `geany` | Mixed; paths are OS-filtered | Lightweight IDE log file (config and plugins are kept). |
| `geary` | Linux | Geary mail client: cache and logs (mail on the server is untouched). |
| `gimp` | Mixed; paths are OS-filtered | GIMP görüntü düzenleyici önbelleği ve günlükleri. |
| `git` | Mixed; paths are OS-filtered | GitHub CLI and pre-commit caches (repositories and credentials are kept). |
| `gnumeric` | Mixed; paths are OS-filtered | Gnumeric spreadsheet: cache and logs (your spreadsheets are kept). |
| `godot` | Mixed; paths are OS-filtered | Game engine: cache and export templates (projects are kept). |
| `gog` | Mixed; paths are OS-filtered | GOG oyun başlatıcısı günlükleri ve önbelleği. |
| `gthumb` | Linux | gThumb image viewer: thumbnail and metadata cache (your photos are never touched). |
| `guix` | Linux | Functional package manager fetch cache (store roots are kept). |
| `gwenview` | Linux | Gwenview image viewer: thumbnail and cache data (your images are never touched). |
| `handbrake` | Mixed; paths are OS-filtered | HandBrake video transcoder: saved activity logs and preview data. |
| `heroic` | Linux | GOG/Epic/Amazon oyun başlatıcısı günlükleri ve önbelleği. |
| `huggingface` | Mixed; paths are OS-filtered | Hugging Face hub housekeeping: stale locks and negative-lookup markers (model weights are kept). |
| `inkscape` | Mixed; paths are OS-filtered | Inkscape vektör grafik düzenleyici önbelleği ve günlükleri. |
| `insomnia` | Mixed; paths are OS-filtered | Insomnia API client: cache, logs and crash reports (your collections are kept). |
| `itch` | Mixed; paths are OS-filtered | itch.io app: logs only (library, downloads and credentials are kept). |
| `java` | Mixed; paths are OS-filtered | Java deployment and Web Start caches (downloaded applets and JNLP applications). |
| `jellyfin` | Mixed; paths are OS-filtered | Jellyfin Media Server: transcode cache, logs and image cache (your media and library are kept). |
| `jetbrains` | Linux | IntelliJ, PyCharm, WebStorm ve diğerleri: günlükler ve dizinler (projeler korunur). |
| `joplin` | Mixed; paths are OS-filtered | Note-taking app: cache, temp files and logs (notes and settings are kept). |
| `jupyter` | Mixed; paths are OS-filtered | Jupyter: runtime files and caches (notebooks and kernelspecs are kept). |
| `kaggle` | Mixed; paths are OS-filtered | Downloaded competition datasets (re-download with the kaggle CLI). |
| `kdenlive` | Linux | Video editor: render cache, thumbnails and logs (projects are kept). |
| `keepassxc` | Mixed; paths are OS-filtered | KeePassXC: browser-integration and icon caches (your password databases are never touched). |
| `kodi` | Mixed; paths are OS-filtered | Media center: temp files and artwork cache (library, sources and add-ons are kept). |
| `krita` | Mixed; paths are OS-filtered | Digital painting cache (brushes, documents and settings are kept). |
| `lazarus` | Unix | Pascal IDE package downloads and logs (projects and components are kept). |
| `libreoffice` | Mixed; paths are OS-filtered | LibreOffice suite: cache and recent-document registry. |
| `linux_caches` | Linux | Yeniden üretilebilir kullanıcı/uygulama önbellekleri (pip, npm, cargo, mesa, crash raporları). |
| `linux_crash` | Linux | Apport/ABRT çökme raporları ve systemd-coredump deposu (root gerekir). |
| `linux_devcache` | Unix | npm, pip, cargo ve gradle indirme/derleme önbellekleri (projeler korunur, yeniden iner). |
| `linux_docker` | Linux | Asılı imajlar, build cache, durdurulmuş kapsayıcı logları ve kullanılmayan (dangling) volume'ler (çalışan kapsayıcılara ve adlandırılmış volume'lere dokunulmaz). |
| `linux_flatpak` | Linux | Kullanılmayan Flatpak runtime'ları ve kalıntı dizinleri. |
| `linux_gnome` | Unix | GNOME uygulama önbellekleri, Tracker dizini ve Zeitgeist etkinlik günlüğü (dconf ve eklentiler korunur). |
| `linux_journal` | Linux | Systemd journal logları için boyut/zaman bütçeli vacuum — sistem (varsayılan 500M, 7 gün; root gerekir) + kullanıcı (`--user`, root GEREKMEZ). |
| `linux_kde` | Unix | Plasma kabuğu/tema/simge önbellekleri, Baloo dizini ve Akonadi dosya önbelleği (yapılandırma ve KWallet korunur). |
| `linux_kernel` | Linux | Kullanılmayan eski Linux çekirdekleri (çalışan çekirdek asla silinmez, root gerekir). |
| `linux_orphans` | Linux | Bağımlılığı kalmamış otomatik paketler (apt/dnf/pacman; root gerekir). |
| `linux_snap` | Linux | Devre dışı Snap revizyonları ve Snap indirme önbelleği (root gerekir). |
| `linux_temp` | Linux | /tmp ve /var/tmp içindeki 1 günden eski dosyalar (X11/systemd soketleri korunur). |
| `lmstudio` | Mixed; paths are OS-filtered | LM Studio: web/shader caches and logs (downloaded models are kept). |
| `looking_glass` | Linux | Looking Glass client cache. |
| `lutris` | Linux | Açık kaynak oyun yöneticisi günlükleri ve önbelleği (kurulu oyunlara dokunulmaz). |
| `lxc` | Linux | Linux container image cache (running containers are kept). |
| `macos` | macOS | macOS system caches and logs. |
| `macos_fontcache` | macOS | Font registry caches (rebuilt by the system). |
| `macos_quicklook` | macOS | QuickLook thumbnail cache (regenerated on preview). |
| `macos_timemachine` | macOS | Thin local snapshots (backups on the TM volume are kept). |
| `mailspring` | Mixed; paths are OS-filtered | Mailspring mail client: cache, logs and crash reports (mail on the server is untouched). |
| `mangohud` | Linux | MangoHud overlay cache (config is kept). |
| `mattermost` | Mixed; paths are OS-filtered | Mattermost client: cache, logs and crash dumps (login and team data are kept). |
| `media` | Mixed; paths are OS-filtered | Cache and thumbnail data from VLC, GIMP and Inkscape. |
| `megasync` | Mixed; paths are OS-filtered | MEGAsync client: cache, logs and crash dumps (your synced files are never touched). |
| `minecraft` | Mixed; paths are OS-filtered | Official launcher and game logs (worlds, mods and resource packs are kept). |
| `mlflow` | Mixed; paths are OS-filtered | Old run artifacts (experiment metadata is kept). |
| `mpv` | Mixed; paths are OS-filtered | mpv media player: watch-later state, cache and log files. |
| `neovim` | Mixed; paths are OS-filtered | Neovim editor: cache, swap leftovers and old logs (config, plugins and shada are kept). |
| `netbeans` | Mixed; paths are OS-filtered | Apache NetBeans IDE: index caches and logs (projects are kept). |
| `nextcloud` | Linux | Bulut istemcisi günlükleri ve önbelleği (eşleştirilen dosyalar ve hesap korunur). |
| `nginx` | Linux | nginx: rotated access and error logs (configuration and web content are never touched). |
| `nix` | Unix | Functional package manager fetch cache + store garbage collection (`nix-collect-garbage -d`, yalnızca ulaşılamaz yollar ve eski nesiller). |
| `nomacs` | Mixed; paths are OS-filtered | nomacs image viewer: thumbnail cache and logs (your photos are never touched). |
| `notion` | Mixed; paths are OS-filtered | Notion desktop app: cache, logs and crash reports (your workspaces are kept). |
| `obs` | Mixed; paths are OS-filtered | Yayın yazılımı günlükleri ve çökme raporları (sahneler ve profiller korunur). |
| `obsidian` | Mixed; paths are OS-filtered | Note-taking app: cache and logs only (vaults, plugins and settings are kept). |
| `okular` | Linux | Okular document viewer: page and thumbnail caches (your documents are never touched). |
| `ollama` | Mixed; paths are OS-filtered | Ollama local LLM server: interrupted download fragments (installed models are kept). |
| `onedrive` | Linux | OneDrive sync client: logs and local cache (your synced files are never touched). |
| `onlyoffice` | Mixed; paths are OS-filtered | ONLYOFFICE Desktop Editors: cache, logs and crash reports (your documents are kept). |
| `openshot` | Mixed; paths are OS-filtered | OpenShot video editor: preview cache and logs (your projects and media are kept). |
| `openwebui` | Mixed; paths are OS-filtered | Open WebUI server: cache and logs (uploads, database and models are kept). |
| `packagekit` | Linux | Cross-distro package metadata cache (re-downloaded on refresh). |
| `pacman` | Linux | Arch Linux package manager cache. |
| `paradox` | Linux | Paradox grand-strategy logs and crash dumps (saves, settings and mods are kept). |
| `pidgin` | Mixed; paths are OS-filtered | Multi-protocol chat logs (accounts are kept). |
| `platformio` | Mixed; paths are OS-filtered | Embedded toolchain cache (platforms, packages and projects are kept). |
| `plex` | Mixed; paths are OS-filtered | Plex Media Server: cache, logs and crash reports (your media and library are kept). |
| `poetry` | Mixed; paths are OS-filtered | Python package cache (virtualenvs and projects are kept). |
| `postfix` | Linux | Mail server logs (queue and config are never touched). |
| `postman` | Mixed; paths are OS-filtered | API client: cache and logs (collections and environments are kept). |
| `prismlauncher` | Linux | Minecraft başlatıcısı günlükleri ve meta önbelleği (dünyalar ve modlar korunur). |
| `proton_ge` | Linux | DXVK shader state caches (Wine builds are kept). |
| `pulseaudio` | Linux | Sound server runtime databases (regenerated, volume levels reset). |
| `pyenv` | Unix | pyenv: downloaded Python source cache (your installed versions are kept). |
| `qbittorrent` | Mixed; paths are OS-filtered | Torrent istemcisi günlükleri (torrent listesi ve inen dosyalar korunur). |
| `qemu` | Linux | QEMU emulator cache (disk images are kept). |
| `qtcreator` | Mixed; paths are OS-filtered | Qt Creator IDE: build, QML and help caches plus logs (projects are kept). |
| `qutebrowser` | Mixed; paths are OS-filtered | qutebrowser: web cache, history database and logs (settings and sessions are kept). |
| `rawtherapee` | Mixed; paths are OS-filtered | RAW editor cache (profiles and edits are kept). |
| `rclone` | Mixed; paths are OS-filtered | rclone: VFS and chunk caches plus logs (remotes and configuration are kept). |
| `remmina` | Mixed; paths are OS-filtered | Remmina remote desktop client: logs and cache (connection profiles are kept). |
| `retroarch` | Mixed; paths are OS-filtered | Multi-system emulator: logs and downloaded thumbnails (saves, states and cores are kept). |
| `rhythmbox` | Linux | Music player cache (library and playlists are kept). |
| `rstudio` | Mixed; paths are OS-filtered | RStudio: session cache and logs (projects and scripts are kept). |
| `ruby` | Mixed; paths are OS-filtered | RubyGems download cache: fetched *.gem archives (installed gems are kept). |
| `rustup` | Mixed; paths are OS-filtered | Rustup toolchain manager: download cache and temporary files (installed toolchains are kept). |
| `safari` | macOS | Apple Safari web browser: cache, history and cookies. |
| `samba` | Linux | Windows-share server logs (shares and databases are kept). |
| `sbt` | Mixed; paths are OS-filtered | Scala build caches (projects are kept). |
| `sdkman` | Unix | SDKMAN! JVM SDK manager: downloaded archives and temporary staging files. |
| `shotcut` | Mixed; paths are OS-filtered | Video editor logs (projects, presets and exports are kept). |
| `shotwell` | Linux | Shotwell photo manager: thumbnail and staging cache (your photos are never touched). |
| `signal` | Mixed; paths are OS-filtered | Signal messenger: cache, logs and crash dumps (messages and keys are kept). |
| `skype` | Mixed; paths are OS-filtered | Skype client: cache, logs and crash dumps (login and chat history are kept). |
| `slack` | Mixed; paths are OS-filtered | Ekip mesajlaşması önbelleği ve günlükleri (oturum korunur). |
| `snapcraft` | Linux | Snapcraft build cache (projects are kept). |
| `spotify` | Mixed; paths are OS-filtered | Müzik akış önbelleği (çevrimdışı şarkılar yeniden iner, oturum korunur). |
| `spyder` | Mixed; paths are OS-filtered | Spyder Python IDE: caches and logs (settings and scripts are kept). |
| `steam` | Mixed; paths are OS-filtered | Oyun istemcisi günlükleri, çökme dökümleri ve yeniden inen önbellekler (oyun dosyalarına dokunulmaz). |
| `strawberry` | Mixed; paths are OS-filtered | Strawberry music player: cover-art and stream cache plus logs (library is kept). |
| `sublime` | Mixed; paths are OS-filtered | Sublime Text editor: index cache and old logs (sessions and packages are kept). |
| `sumatrapdf` | Windows | SumatraPDF reader: cache, thumbnail cache and crash dumps. |
| `syncthing` | Mixed; paths are OS-filtered | Syncthing: logs and the local index database (your synced folders are never touched). |
| `system` | Mixed; paths are OS-filtered | Cross-platform system maintenance: temporary files, trash, clipboard, DNS and more. |
| `teams` | Mixed; paths are OS-filtered | Teams client: cache, logs and crash dumps (login and chat history are kept). |
| `telegram` | Mixed; paths are OS-filtered | Mesajlaşma istemcisi günlükleri (oturum ve sohbetler korunur). |
| `terraform` | Mixed; paths are OS-filtered | Terraform CLI: provider plugin cache and crash logs. |
| `thumbnails` | Mixed; paths are OS-filtered | Cached image and file thumbnails. |
| `thunderbird` | Mixed; paths are OS-filtered | E-posta istemcisi önbelleği ve telemetrisi (e-postalar ve hesaplar ASLA silinmez). |
| `toolbx` | Linux | Toolbx container cache (containers are kept). |
| `torbrowser` | Mixed; paths are OS-filtered | Anonymous browser cache (cleaning is recommended for privacy). |
| `transmission` | Mixed; paths are OS-filtered | Transmission BitTorrent client: log files (torrents and resume data are kept). |
| `unity` | Mixed; paths are OS-filtered | Game engine: editor logs and Asset Store downloads (projects, licenses and packages are kept). |
| `unreal` | Linux | DerivedDataCache and editor logs (projects, vault and licenses are kept). |
| `viber` | Mixed; paths are OS-filtered | Viber: cache, logs and crash reports (your messages and media are kept). |
| `vim` | Unix | Editor history and backups (config and plugins are kept). |
| `virtualbox` | Linux | Sanal makine günlükleri (disk imajları ve anlık görüntüler korunur). |
| `vlc` | Mixed; paths are OS-filtered | VLC oynatıcı önbelleği ve günlükleri. |
| `vmware` | Mixed; paths are OS-filtered | Workstation log files (VMs and inventory are kept). |
| `vscode` | Mixed; paths are OS-filtered | VS Code editor: workspace cache, logs, extension data ve Remote-SSH/container sunucu logları. |
| `vscodium` | Mixed; paths are OS-filtered | VSCodium editor: workspace cache, logs and extension data. |
| `wandb` | Mixed; paths are OS-filtered | Cached run data (cloud runs are kept). |
| `wechat` | Linux | WeChat desktop: cache and logs (your chat history is kept). |
| `whatsapp` | Mixed; paths are OS-filtered | WhatsApp Desktop cache and logs (chats and login are kept). |
| `windows` | Windows | Windows system cleaners: prefetch, recycle bin, error reports and caches. |
| `windows_defender` | Windows | Old scan history and logs (protection stays on). |
| `windows_update_cache` | Windows | Delivered update packages (re-fetched if needed). |
| `windsurf` | Mixed; paths are OS-filtered | Windsurf AI editor: workspace cache, web caches and logs (projects, settings and extensions are kept). |
| `wine` | Unix | Windows compatibility layer: temp files and downloaded runtimes (prefixes and programs are kept). |
| `wps_office` | Mixed; paths are OS-filtered | WPS Office: cache, temporary files and logs (your documents are kept). |
| `x11` | Unix | X session error logs (active session file is kept). |
| `xfce` | Linux | XFCE desktop: saved session state and Thunar caches. |
| `yt_dlp` | Mixed; paths are OS-filtered | yt-dlp: cached player scripts and extractor data (your downloads are kept). |
| `zathura` | Linux | zathura PDF viewer: cache data (your documents and reading position are kept). |
| `zed` | Mixed; paths are OS-filtered | Zed editor: logs and caches (settings, extensions and workspace state are kept). |
| `zoom` | Mixed; paths are OS-filtered | Görüntülü görüşme istemcisi önbelleği ve günlükleri. |
| `zotero` | Mixed; paths are OS-filtered | Zotero reference manager: caches, logs and crash reports (your library is kept). |
| `zypper` | Linux | openSUSE paket yöneticisi önbelleği. |

The table describes definition intent. Use `sweep list` to see active options
on the current platform and `sweep providers` to inspect action availability.

## Platform implementation

### Windows

`src/platform/windows.rs` provides native Windows behavior, including:

- Win32 registry reads and deletion helpers;
- Recycle Bin operations;
- Prefetch, WER, Windows Update, Delivery Optimization, and Explorer cache
  locations, plus thumbnail databases, Microsoft Store cache, DirectX shader
  cache, crash dumps and memory dumps;
- startup management through the `HKCU`/`HKLM` `Run` registry keys
  (`sweep startup`, reversible via a `sweep-disabled` subkey);
- memory optimization via `EmptyWorkingSet` (`sweep memopt`);
- reparse-point, mount-point, hard-link, read-only, and locked-path helpers;
- clipboard, DNS, shell refresh, free-space, and volume operations; and
- Windows diagnostics.

Registry actions are Windows-only and are treated as privileged. External
CleanerML/TOML definitions cannot use `winreg` unless the explicit
`--trust-external` override is supplied.

Windows reports TRIM as an operating-system-managed behavior rather than
exposing a generic per-path trim operation through the Unix implementation.

### Linux

The Linux module contains specialized helpers for:

- FITRIM/discard-related behavior;
- systemd journal vacuuming;
- DNS resolver detection and flushing;
- memory/free-space operations; and
- Linux-specific leftover/container paths.

`apt`, `dnf`, `yum`, `pacman`, and `zypper` actions require the matching
executable and may require elevated privileges. Old-kernel and orphan-package
removal is implemented for apt, dnf, pacman, and zypper. Journald cleanup
depends on systemd and its
permissions. Memory wiping is Linux-only.

### macOS

The macOS module provides paths and helpers for user/system caches, logs, junk
filenames, saved-state data, and Quick Look-related caches. The `macos` cleaner
is explicitly filtered to macOS. Clipboard, DNS, and system-service behavior
also depends on the current desktop/session environment.

### Unix and BSD

Generic Unix facilities are shared by `src/platform/unix.rs`, with additional
Linux and macOS modules selected where appropriate. BSD and other POSIX
systems may support generic traversal, size, deletion, trash, logs, and path
operations without supporting every Linux or macOS specialization.

## Provider availability caveats

A provider can be unavailable or fail at runtime for reasons that are not
visible from its command name:

- package-manager executables may not be installed or may require root/admin
  privileges;
- clipboard and DNS actions depend on desktop helpers, resolver services, and
  the current session;
- journald, memory, registry, recycle-bin, and system-cache operations depend
  on platform permissions and service state;
- browser SQLite schemas change between application versions, and a running
  browser can lock or mutate its databases;
- system paths may be read-only, mounted separately, or protected by policy;
  and
- an option can be filtered out by its cleaner, option, variable, or action
  `os` value.

Use preview first, then inspect the report's per-entry failures. A failure in
one provider is not proof that unrelated options were not processed.

## Storage, shredding, and free-space wiping

The ordinary `--shred` flag requests overwrite-before-delete behavior. The
shred module supports zero, one, random, DoD-style, and paranoid patterns, and
can randomize names before removal. `sweep wipe PATH` and the corresponding
free-space provider write temporary fill data on the volume, leaving a small
reserve so the operation can clean up its own temporary files. These operations
can consume substantial disk space and may be interrupted by permissions,
locks, quotas, or filesystem behavior.

On Linux, trim/discard may be more appropriate for SSD-style storage where the
platform implementation supports it. On Windows, the source treats TRIM as
normally managed by Windows rather than offering the same generic path-level
operation.

No overwrite or free-space operation can guarantee removal from every storage
layer. Recovery may remain possible through:

- SSD/NVMe over-provisioning and internal remapping;
- copy-on-write or journaled filesystems;
- snapshots, backups, sync services, and replicas;
- filesystem metadata, temporary copies, or database journals; and
- external forensic or device-level mechanisms.

Do not describe `shred` as a universal secure-erasure guarantee.

## Definition import compatibility

### CleanerML XML

CleanerML XML is normalized into the native model. The parser rejects an XML
internal-subset DTD as an XXE/expansion safety measure. Definitions loaded from
untrusted external locations lose `process` and `winreg` actions by default.

The importer is compatible with the supported subset of the current parser,
not necessarily with every historical CleanerML provider or attribute. Run
`sweep import` and inspect warnings before enabling imported cleaners.

### winapp2.ini

`winapp2.ini` is a Windows-oriented format with Windows path and registry
conventions. It is loaded through the dedicated `--winapp2` option or a
definition-directory workflow and should not be treated as a portable generic
INI file. Imported sections need the same path and privilege review as native
cleaners.

### Native TOML

New Sweep cleaners should use native TOML. It has a typed schema, explicit
OS filters, provider validation, variables, path search modes, and optional
age/size/depth filters. See [cleaner-format.md](cleaner-format.md).

## Recommended troubleshooting sequence

1. Run `sweep diagnostics` and record the platform/environment information.
2. Run `sweep providers` and confirm that the intended provider is available.
3. Run `sweep list` and verify that the cleaner option is active.
4. Preview the narrowest selector possible, preferably against disposable
   fixtures.
5. Use `--json` when an automation needs structured report data.
6. Use `--config PATH` explicitly when diagnosing configuration location or
   malformed settings.
7. Close the target application and repeat the preview if a running check or
   database lock is involved.

`--force` is not a general troubleshooting switch. It bypasses important
running-application and protected-root refusals and should only be used when
the consequences are understood.
