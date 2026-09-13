//! Cleaner definitions compiled into the binary.
//!
//! Embedding them with `include_str!` means `sweep` is a single file with no
//! external data directory, and it removes a whole class of "installation
//! broken" bug reports. User-supplied cleaners are still read from disk at
//! runtime and always win over a built-in with the same id.
//!
//! Keep this list alphabetically sorted — the docs and the `--list` output
//! follow the same order.

/// One embedded definition: `(file name, TOML source)`.
pub const BUILTIN_CLEANERS: &[(&str, &str)] = &[
    ("abiword.toml", include_str!("../../cleaners/abiword.toml")),
    ("anki.toml", include_str!("../../cleaners/anki.toml")),
    (
        "appimage.toml",
        include_str!("../../cleaners/appimage.toml"),
    ),
    ("apt.toml", include_str!("../../cleaners/apt.toml")),
    ("aptlogs.toml", include_str!("../../cleaners/aptlogs.toml")),
    ("arduino.toml", include_str!("../../cleaners/arduino.toml")),
    ("aria2.toml", include_str!("../../cleaners/aria2.toml")),
    ("atom.toml", include_str!("../../cleaners/atom.toml")),
    (
        "audacious.toml",
        include_str!("../../cleaners/audacious.toml"),
    ),
    (
        "audacity.toml",
        include_str!("../../cleaners/audacity.toml"),
    ),
    ("bash.toml", include_str!("../../cleaners/bash.toml")),
    (
        "battlenet.toml",
        include_str!("../../cleaners/battlenet.toml"),
    ),
    ("blender.toml", include_str!("../../cleaners/blender.toml")),
    ("bottles.toml", include_str!("../../cleaners/bottles.toml")),
    (
        "browsers.toml",
        include_str!("../../cleaners/browsers.toml"),
    ),
    ("calibre.toml", include_str!("../../cleaners/calibre.toml")),
    (
        "clementine.toml",
        include_str!("../../cleaners/clementine.toml"),
    ),
    (
        "clipboard.toml",
        include_str!("../../cleaners/clipboard.toml"),
    ),
    (
        "cocoapods.toml",
        include_str!("../../cleaners/cocoapods.toml"),
    ),
    (
        "comfyui.toml",
        include_str!("../../cleaners/comfyui.toml"),
    ),
    (
        "composer.toml",
        include_str!("../../cleaners/composer.toml"),
    ),
    ("conda.toml", include_str!("../../cleaners/conda.toml")),
    (
        "coursier.toml",
        include_str!("../../cleaners/coursier.toml"),
    ),
    ("cursor.toml", include_str!("../../cleaners/cursor.toml")),
    (
        "darktable.toml",
        include_str!("../../cleaners/darktable.toml"),
    ),
    ("davinci.toml", include_str!("../../cleaners/davinci.toml")),
    ("dbeaver.toml", include_str!("../../cleaners/dbeaver.toml")),
    (
        "deadbeef.toml",
        include_str!("../../cleaners/deadbeef.toml"),
    ),
    ("deluge.toml", include_str!("../../cleaners/deluge.toml")),
    (
        "dev_tools.toml",
        include_str!("../../cleaners/dev_tools.toml"),
    ),
    ("digikam.toml", include_str!("../../cleaners/digikam.toml")),
    ("discord.toml", include_str!("../../cleaners/discord.toml")),
    ("distrobox.toml", include_str!("../../cleaners/distrobox.toml")),
    ("dnf.toml", include_str!("../../cleaners/dnf.toml")),
    ("dolphin.toml", include_str!("../../cleaners/dolphin.toml")),
    ("dropbox.toml", include_str!("../../cleaners/dropbox.toml")),
    ("eclipse.toml", include_str!("../../cleaners/eclipse.toml")),
    ("element.toml", include_str!("../../cleaners/element.toml")),
    ("emacs.toml", include_str!("../../cleaners/emacs.toml")),
    (
        "emulators.toml",
        include_str!("../../cleaners/emulators.toml"),
    ),
    (
        "epicgames.toml",
        include_str!("../../cleaners/epicgames.toml"),
    ),
    (
        "evernote.toml",
        include_str!("../../cleaners/evernote.toml"),
    ),
    ("evince.toml", include_str!("../../cleaners/evince.toml")),
    (
        "evolution.toml",
        include_str!("../../cleaners/evolution.toml"),
    ),
    ("falkon.toml", include_str!("../../cleaners/falkon.toml")),
    ("figma.toml", include_str!("../../cleaners/figma.toml")),
    (
        "filezilla.toml",
        include_str!("../../cleaners/filezilla.toml"),
    ),
    ("firefox.toml", include_str!("../../cleaners/firefox.toml")),
    (
        "firefox_forks.toml",
        include_str!("../../cleaners/firefox_forks.toml"),
    ),
    ("flatpak_builder.toml", include_str!("../../cleaners/flatpak_builder.toml")),
    ("flutter.toml", include_str!("../../cleaners/flutter.toml")),
    ("fonts.toml", include_str!("../../cleaners/fonts.toml")),
    (
        "gamemaker.toml",
        include_str!("../../cleaners/gamemaker.toml"),
    ),
    ("gamemode.toml", include_str!("../../cleaners/gamemode.toml")),
    ("geany.toml", include_str!("../../cleaners/geany.toml")),
    ("geary.toml", include_str!("../../cleaners/geary.toml")),
    ("gimp.toml", include_str!("../../cleaners/gimp.toml")),
    ("git.toml", include_str!("../../cleaners/git.toml")),
    (
        "gnumeric.toml",
        include_str!("../../cleaners/gnumeric.toml"),
    ),
    ("godot.toml", include_str!("../../cleaners/godot.toml")),
    ("gog.toml", include_str!("../../cleaners/gog.toml")),
    ("gthumb.toml", include_str!("../../cleaners/gthumb.toml")),
    ("guix.toml", include_str!("../../cleaners/guix.toml")),
    (
        "gwenview.toml",
        include_str!("../../cleaners/gwenview.toml"),
    ),
    (
        "handbrake.toml",
        include_str!("../../cleaners/handbrake.toml"),
    ),
    ("heroic.toml", include_str!("../../cleaners/heroic.toml")),
    (
        "huggingface.toml",
        include_str!("../../cleaners/huggingface.toml"),
    ),
    (
        "inkscape.toml",
        include_str!("../../cleaners/inkscape.toml"),
    ),
    (
        "insomnia.toml",
        include_str!("../../cleaners/insomnia.toml"),
    ),
    ("itch.toml", include_str!("../../cleaners/itch.toml")),
    ("java.toml", include_str!("../../cleaners/java.toml")),
    (
        "jellyfin.toml",
        include_str!("../../cleaners/jellyfin.toml"),
    ),
    (
        "jetbrains.toml",
        include_str!("../../cleaners/jetbrains.toml"),
    ),
    ("joplin.toml", include_str!("../../cleaners/joplin.toml")),
    ("jupyter.toml", include_str!("../../cleaners/jupyter.toml")),
    ("kaggle.toml", include_str!("../../cleaners/kaggle.toml")),
    (
        "kdenlive.toml",
        include_str!("../../cleaners/kdenlive.toml"),
    ),
    (
        "keepassxc.toml",
        include_str!("../../cleaners/keepassxc.toml"),
    ),
    ("kodi.toml", include_str!("../../cleaners/kodi.toml")),
    ("krita.toml", include_str!("../../cleaners/krita.toml")),
    ("lazarus.toml", include_str!("../../cleaners/lazarus.toml")),
    (
        "libreoffice.toml",
        include_str!("../../cleaners/libreoffice.toml"),
    ),
    (
        "linux_caches.toml",
        include_str!("../../cleaners/linux_caches.toml"),
    ),
    (
        "linux_crash.toml",
        include_str!("../../cleaners/linux_crash.toml"),
    ),
    (
        "linux_devcache.toml",
        include_str!("../../cleaners/linux_devcache.toml"),
    ),
    (
        "linux_docker.toml",
        include_str!("../../cleaners/linux_docker.toml"),
    ),
    (
        "linux_flatpak.toml",
        include_str!("../../cleaners/linux_flatpak.toml"),
    ),
    (
        "linux_gnome.toml",
        include_str!("../../cleaners/linux_gnome.toml"),
    ),
    (
        "linux_journal.toml",
        include_str!("../../cleaners/linux_journal.toml"),
    ),
    (
        "linux_kde.toml",
        include_str!("../../cleaners/linux_kde.toml"),
    ),
    (
        "linux_kernel.toml",
        include_str!("../../cleaners/linux_kernel.toml"),
    ),
    (
        "linux_orphans.toml",
        include_str!("../../cleaners/linux_orphans.toml"),
    ),
    (
        "linux_snap.toml",
        include_str!("../../cleaners/linux_snap.toml"),
    ),
    (
        "linux_temp.toml",
        include_str!("../../cleaners/linux_temp.toml"),
    ),
    ("lmstudio.toml", include_str!("../../cleaners/lmstudio.toml")),
    ("looking_glass.toml", include_str!("../../cleaners/looking_glass.toml")),
    ("lutris.toml", include_str!("../../cleaners/lutris.toml")),
    ("lxc.toml", include_str!("../../cleaners/lxc.toml")),
    ("macos.toml", include_str!("../../cleaners/macos.toml")),
    ("macos_fontcache.toml", include_str!("../../cleaners/macos_fontcache.toml")),
    ("macos_quicklook.toml", include_str!("../../cleaners/macos_quicklook.toml")),
    ("macos_timemachine.toml", include_str!("../../cleaners/macos_timemachine.toml")),
    (
        "mailspring.toml",
        include_str!("../../cleaners/mailspring.toml"),
    ),
    ("mangohud.toml", include_str!("../../cleaners/mangohud.toml")),
    (
        "mattermost.toml",
        include_str!("../../cleaners/mattermost.toml"),
    ),
    ("media.toml", include_str!("../../cleaners/media.toml")),
    (
        "megasync.toml",
        include_str!("../../cleaners/megasync.toml"),
    ),
    (
        "minecraft.toml",
        include_str!("../../cleaners/minecraft.toml"),
    ),
    ("mlflow.toml", include_str!("../../cleaners/mlflow.toml")),
    ("mpv.toml", include_str!("../../cleaners/mpv.toml")),
    ("neovim.toml", include_str!("../../cleaners/neovim.toml")),
    (
        "netbeans.toml",
        include_str!("../../cleaners/netbeans.toml"),
    ),
    (
        "nextcloud.toml",
        include_str!("../../cleaners/nextcloud.toml"),
    ),
    ("nginx.toml", include_str!("../../cleaners/nginx.toml")),
    ("nix.toml", include_str!("../../cleaners/nix.toml")),
    ("nomacs.toml", include_str!("../../cleaners/nomacs.toml")),
    ("notion.toml", include_str!("../../cleaners/notion.toml")),
    ("obs.toml", include_str!("../../cleaners/obs.toml")),
    (
        "obsidian.toml",
        include_str!("../../cleaners/obsidian.toml"),
    ),
    ("okular.toml", include_str!("../../cleaners/okular.toml")),
    ("ollama.toml", include_str!("../../cleaners/ollama.toml")),
    (
        "onedrive.toml",
        include_str!("../../cleaners/onedrive.toml"),
    ),
    (
        "onlyoffice.toml",
        include_str!("../../cleaners/onlyoffice.toml"),
    ),
    (
        "openshot.toml",
        include_str!("../../cleaners/openshot.toml"),
    ),
    (
        "openwebui.toml",
        include_str!("../../cleaners/openwebui.toml"),
    ),
    (
        "packagekit.toml",
        include_str!("../../cleaners/packagekit.toml"),
    ),
    ("pacman.toml", include_str!("../../cleaners/pacman.toml")),
    (
        "paradox.toml",
        include_str!("../../cleaners/paradox.toml"),
    ),
    ("pidgin.toml", include_str!("../../cleaners/pidgin.toml")),
    (
        "platformio.toml",
        include_str!("../../cleaners/platformio.toml"),
    ),
    ("plex.toml", include_str!("../../cleaners/plex.toml")),
    ("poetry.toml", include_str!("../../cleaners/poetry.toml")),
    ("postfix.toml", include_str!("../../cleaners/postfix.toml")),
    ("postman.toml", include_str!("../../cleaners/postman.toml")),
    (
        "prismlauncher.toml",
        include_str!("../../cleaners/prismlauncher.toml"),
    ),
    ("proton_ge.toml", include_str!("../../cleaners/proton_ge.toml")),
    (
        "pulseaudio.toml",
        include_str!("../../cleaners/pulseaudio.toml"),
    ),
    ("pyenv.toml", include_str!("../../cleaners/pyenv.toml")),
    (
        "qbittorrent.toml",
        include_str!("../../cleaners/qbittorrent.toml"),
    ),
    ("qemu.toml", include_str!("../../cleaners/qemu.toml")),
    (
        "qtcreator.toml",
        include_str!("../../cleaners/qtcreator.toml"),
    ),
    (
        "qutebrowser.toml",
        include_str!("../../cleaners/qutebrowser.toml"),
    ),
    (
        "rawtherapee.toml",
        include_str!("../../cleaners/rawtherapee.toml"),
    ),
    ("rclone.toml", include_str!("../../cleaners/rclone.toml")),
    ("remmina.toml", include_str!("../../cleaners/remmina.toml")),
    (
        "retroarch.toml",
        include_str!("../../cleaners/retroarch.toml"),
    ),
    (
        "rhythmbox.toml",
        include_str!("../../cleaners/rhythmbox.toml"),
    ),
    ("rstudio.toml", include_str!("../../cleaners/rstudio.toml")),
    ("ruby.toml", include_str!("../../cleaners/ruby.toml")),
    ("rustup.toml", include_str!("../../cleaners/rustup.toml")),
    ("safari.toml", include_str!("../../cleaners/safari.toml")),
    ("samba.toml", include_str!("../../cleaners/samba.toml")),
    ("sbt.toml", include_str!("../../cleaners/sbt.toml")),
    ("sdkman.toml", include_str!("../../cleaners/sdkman.toml")),
    ("shotcut.toml", include_str!("../../cleaners/shotcut.toml")),
    (
        "shotwell.toml",
        include_str!("../../cleaners/shotwell.toml"),
    ),
    ("signal.toml", include_str!("../../cleaners/signal.toml")),
    ("skype.toml", include_str!("../../cleaners/skype.toml")),
    ("slack.toml", include_str!("../../cleaners/slack.toml")),
    ("snapcraft.toml", include_str!("../../cleaners/snapcraft.toml")),
    ("spotify.toml", include_str!("../../cleaners/spotify.toml")),
    ("spyder.toml", include_str!("../../cleaners/spyder.toml")),
    ("steam.toml", include_str!("../../cleaners/steam.toml")),
    (
        "strawberry.toml",
        include_str!("../../cleaners/strawberry.toml"),
    ),
    ("sublime.toml", include_str!("../../cleaners/sublime.toml")),
    (
        "sumatrapdf.toml",
        include_str!("../../cleaners/sumatrapdf.toml"),
    ),
    (
        "syncthing.toml",
        include_str!("../../cleaners/syncthing.toml"),
    ),
    ("system.toml", include_str!("../../cleaners/system.toml")),
    ("teams.toml", include_str!("../../cleaners/teams.toml")),
    (
        "telegram.toml",
        include_str!("../../cleaners/telegram.toml"),
    ),
    (
        "terraform.toml",
        include_str!("../../cleaners/terraform.toml"),
    ),
    (
        "thumbnails.toml",
        include_str!("../../cleaners/thumbnails.toml"),
    ),
    (
        "thunderbird.toml",
        include_str!("../../cleaners/thunderbird.toml"),
    ),
    ("toolbx.toml", include_str!("../../cleaners/toolbx.toml")),
    (
        "torbrowser.toml",
        include_str!("../../cleaners/torbrowser.toml"),
    ),
    (
        "transmission.toml",
        include_str!("../../cleaners/transmission.toml"),
    ),
    ("unity.toml", include_str!("../../cleaners/unity.toml")),
    ("unreal.toml", include_str!("../../cleaners/unreal.toml")),
    ("viber.toml", include_str!("../../cleaners/viber.toml")),
    ("vim.toml", include_str!("../../cleaners/vim.toml")),
    (
        "virtualbox.toml",
        include_str!("../../cleaners/virtualbox.toml"),
    ),
    ("vlc.toml", include_str!("../../cleaners/vlc.toml")),
    ("vmware.toml", include_str!("../../cleaners/vmware.toml")),
    ("vscode.toml", include_str!("../../cleaners/vscode.toml")),
    (
        "vscodium.toml",
        include_str!("../../cleaners/vscodium.toml"),
    ),
    ("wandb.toml", include_str!("../../cleaners/wandb.toml")),
    ("wechat.toml", include_str!("../../cleaners/wechat.toml")),
    (
        "whatsapp.toml",
        include_str!("../../cleaners/whatsapp.toml"),
    ),
    ("windows.toml", include_str!("../../cleaners/windows.toml")),
    ("windows_defender.toml", include_str!("../../cleaners/windows_defender.toml")),
    (
        "windows_update_cache.toml",
        include_str!("../../cleaners/windows_update_cache.toml"),
    ),
    ("windsurf.toml", include_str!("../../cleaners/windsurf.toml")),
    ("wine.toml", include_str!("../../cleaners/wine.toml")),
    (
        "wps_office.toml",
        include_str!("../../cleaners/wps_office.toml"),
    ),
    ("x11.toml", include_str!("../../cleaners/x11.toml")),
    ("xfce.toml", include_str!("../../cleaners/xfce.toml")),
    ("yt_dlp.toml", include_str!("../../cleaners/yt_dlp.toml")),
    ("zathura.toml", include_str!("../../cleaners/zathura.toml")),
    ("zed.toml", include_str!("../../cleaners/zed.toml")),
    ("zoom.toml", include_str!("../../cleaners/zoom.toml")),
    ("zotero.toml", include_str!("../../cleaners/zotero.toml")),
    ("zypper.toml", include_str!("../../cleaners/zypper.toml")),
];

/// Number of definitions shipped in the binary.
pub const fn count() -> usize {
    BUILTIN_CLEANERS.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_duplicate_definitions() {
        let mut seen = std::collections::HashSet::new();
        for (file, _) in BUILTIN_CLEANERS {
            assert!(seen.insert(*file), "cleaner '{file}' is registered twice");
        }
    }

    #[test]
    fn list_stays_alphabetically_sorted() {
        // The module docs promise this ordering and `sweep list` follows it, so
        // a misplaced entry is a (silent) regression rather than a style nit.
        let names: Vec<&str> = BUILTIN_CLEANERS.iter().map(|(file, _)| *file).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(
            names, sorted,
            "BUILTIN_CLEANERS must stay sorted by file name"
        );
    }

    #[test]
    fn recovered_cleaners_are_registered() {
        // These files existed under `cleaners/` but were missing from this list,
        // so `sweep <id>` answered "unknown cleaner" even though the definition
        // was valid. Keep them wired up.
        for id in ["battlenet", "epicgames", "gog", "gimp", "inkscape", "vlc"] {
            let file = format!("{id}.toml");
            assert!(
                BUILTIN_CLEANERS.iter().any(|(name, _)| *name == file),
                "'{file}' must be registered in BUILTIN_CLEANERS"
            );
        }
    }

    #[test]
    fn browser_profile_vars_cover_every_desktop_os() {
        // Browsers ship on Linux, macOS and Windows alike; a profile
        // variable with no value for one of them means the cleaner silently
        // does nothing there (this is how Tor Browser lost macOS).
        let text = BUILTIN_CLEANERS
            .iter()
            .find(|(name, _)| *name == "torbrowser.toml")
            .expect("torbrowser.toml is registered")
            .1;
        let def =
            crate::definition::native::parse_native(text, "torbrowser.toml").unwrap();
        let profile = def
            .vars
            .iter()
            .find(|v| v.name == "profile")
            .expect("torbrowser has a profile var");
        for os in ["linux", "macos", "windows"] {
            assert!(
                profile
                    .values
                    .iter()
                    .any(|v| v.os.0.as_deref() == Some(os)),
                "torbrowser profile has no '{os}' value"
            );
        }
    }

    #[test]
    fn new_deep_cleaners_are_registered() {
        // v0.4.0 deep-cleaner wave: each new file must be wired into
        // BUILTIN_CLEANERS or `sweep <id>` answers "unknown cleaner".
        for id in [
            "bottles",
            "comfyui",
            "coursier",
            "cursor",
            "distrobox",
            "figma",
            "flatpak_builder",
            "gamemode",
            "huggingface",
            "kaggle",
            "lmstudio",
            "looking_glass",
            "macos_fontcache",
            "macos_quicklook",
            "macos_timemachine",
            "mangohud",
            "mlflow",
            "ollama",
            "openwebui",
            "paradox",
            "proton_ge",
            "qemu",
            "ruby",
            "snapcraft",
            "toolbx",
            "vmware",
            "wandb",
            "windsurf",
            "windows_defender",
            "windows_update_cache",
            "zed",
        ] {
            let file = format!("{id}.toml");
            assert!(
                BUILTIN_CLEANERS.iter().any(|(name, _)| *name == file),
                "'{file}' must be registered in BUILTIN_CLEANERS"
            );
        }
    }

    #[test]
    fn safari_is_registered_and_macos_only() {
        let text = BUILTIN_CLEANERS
            .iter()
            .find(|(name, _)| *name == "safari.toml")
            .expect("safari.toml is registered")
            .1;
        let def = crate::definition::native::parse_native(text, "safari.toml").unwrap();
        assert_eq!(def.os.0.as_deref(), Some("macos"));
        assert!(def.options.iter().any(|o| o.id == "cache"));
    }

    #[test]
    fn embedded_sources_are_not_empty() {
        for (file, text) in BUILTIN_CLEANERS {
            assert!(
                !text.trim().is_empty(),
                "'{file}' embedded an empty definition"
            );
        }
    }
}
