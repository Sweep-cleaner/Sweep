//! İzleme daemon'u: disk eşiği aşılınca masaüstü bildirimi (`sweep watch`).
//!
//! Eşik üstünde kalan her yol için bir kez bildirim gönderilir; kullanım
//! histerezis payının (5 puan) altına düşünce alarm sıfırlanır, böylece
//! sınırda titreme olmaz. `--suggest` ile bildirime en büyük dosyalar
//! eklenir, kullanıcı tek komutla neyi sileceğini görür.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::core::report::{Entry, EntryKind, Report};

fn lang() -> crate::i18n::Lang {
    crate::i18n::Lang::detect(&[])
}

/// Raporlardaki cleaner kimliği.
pub const CLEANER_ID: &str = "watch";

/// Alarm sıfırlama payı (puan).
pub const HYSTERESIS: f64 = 5.0;

/// Tek alarm.
#[derive(Debug, Clone)]
pub struct Alert {
    pub path: PathBuf,
    pub used_pct: f64,
    pub free_bytes: u64,
    pub suggestion: Option<String>,
}

/// Yolların doluluklarını ölç, eşiği aşanları döndür.
pub fn check(paths: &[PathBuf], threshold: f64) -> Vec<Alert> {
    let mut out = Vec::new();
    for path in paths {
        if !path.is_dir() {
            continue;
        }
        let (Ok(free), Ok(total)) = (
            crate::platform::free_space(path),
            crate::platform::total_space(path),
        ) else {
            continue;
        };
        if total == 0 {
            continue;
        }
        let used = 100.0 * (1.0 - free as f64 / total as f64);
        if used >= threshold {
            out.push(Alert {
                path: path.clone(),
                used_pct: used,
                free_bytes: free,
                suggestion: None,
            });
        }
    }
    out
}

/// En büyük N dosya önerisi (bildirim gövdesi için tek satır).
pub fn suggest(path: &PathBuf, top: usize) -> Option<String> {
    let hits =
        crate::engine::bigfiles::find_hits(std::slice::from_ref(path), 6, 0, None, Some(top));
    if hits.is_empty() {
        return None;
    }
    let names: Vec<String> = hits
        .iter()
        .map(|h| {
            format!(
                "{} ({})",
                h.path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| h.path.display().to_string()),
                crate::deep::safety::human(h.bytes)
            )
        })
        .collect();
    Some(format!("biggest: {}", names.join(", ")))
}

/// `osascript` çift-tırnaklı AppleScript dizesi için kaçış.
pub fn escape_osascript(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// PowerShell tek-tırnaklı dize için kaçış (`'` -> `''`).
pub fn escape_ps(text: &str) -> String {
    text.replace('\'', "''")
}

/// Masaüstü bildirimi (platform zinciri; hiçbiri yoksa stderr).
pub fn notify(title: &str, body: &str) {
    #[cfg(target_os = "linux")]
    {
        for (prog, args) in [
            ("notify-send", vec!["-a", "Sweep", "-u", "critical"]),
            ("kdialog", vec!["--title", "Sweep", "--sorry"]),
            ("zenity", vec!["--warning", "--title=Sweep", "--no-wrap"]),
        ] {
            if !crate::fsutil::path_exists_in_path(prog) {
                continue;
            }
            let mut full: Vec<&str> = args;
            if prog == "notify-send" {
                full.extend([title, body]);
            } else if prog == "kdialog" {
                full.push(body);
            } else {
                full.extend(["--text", body]);
            }
            if crate::fsutil::run_command(prog, &full, true)
                .map(|(c, _, _)| c)
                .unwrap_or(1)
                == 0
            {
                return;
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        if crate::fsutil::path_exists_in_path("osascript") {
            let script = format!(
                "display notification \"{}\" with title \"{}\"",
                escape_osascript(body),
                escape_osascript(title)
            );
            let _ = crate::fsutil::run_command("osascript", &["-e", &script], true);
            return;
        }
    }
    #[cfg(target_os = "windows")]
    {
        if crate::fsutil::path_exists_in_path("powershell") {
            let ps = format!(
                "Add-Type -AssemblyName System.Windows.Forms; \
                 $n = New-Object System.Windows.Forms.NotifyIcon; \
                 $n.Icon = [System.Drawing.SystemIcons]::Warning; \
                 $n.Visible = $true; \
                 $n.ShowBalloonTip(10000, '{}', '{}', 'Warning')",
                escape_ps(title),
                escape_ps(body)
            );
            if crate::fsutil::run_command(
                "powershell",
                &["-NoProfile", "-NonInteractive", "-Command", &ps],
                true,
            )
            .map(|(c, _, _)| c)
            .unwrap_or(1)
                == 0
            {
                return;
            }
        }
    }
    eprintln!("[sweep watch] {title}: {body}");
}

/// Tek tur: alarm + (istenirse) öneri + rapor. Dönüş: alarm var mı?
pub fn once(
    paths: &[PathBuf],
    threshold: f64,
    suggest_top: usize,
    alerted: &mut HashSet<PathBuf>,
) -> (Report, bool) {
    let mut report = Report::new();
    let mut hits = check(paths, threshold);

    // Düşen alarmları sıfırla (histeresis).
    alerted.retain(|p| {
        let Ok(free) = crate::platform::free_space(p) else {
            return false;
        };
        let Ok(total) = crate::platform::total_space(p) else {
            return false;
        };
        if total == 0 {
            return false;
        }
        100.0 * (1.0 - free as f64 / total as f64) >= threshold - HYSTERESIS
    });

    let mut fired = false;
    for alert in hits.iter_mut() {
        if alerted.contains(&alert.path) {
            continue; // zaten bildirildi
        }
        alerted.insert(alert.path.clone());
        fired = true;
        if suggest_top > 0 {
            alert.suggestion = suggest(&alert.path, suggest_top);
        }
        let mut body = format!(
            "{} %{:.0} full ({} free).",
            alert.path.display(),
            alert.used_pct,
            crate::deep::safety::human(alert.free_bytes)
        );
        if let Some(suggestion) = &alert.suggestion {
            body.push_str(&format!(" {suggestion}"));
        }
        body.push_str(" Clean: sweep bigfiles --clean / sweep dupes --clean");
        notify("Sweep: disk filling up", &body);
        let path_s = alert.path.display().to_string();
        let label = crate::i18n::et(&lang(), "ALARM {} --- {}", &[&path_s, &body]);
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "alert",
            label,
            Some(&alert.path),
            0,
        ));
    }
    if !fired {
        report.push(Entry::new(
            EntryKind::Command,
            CLEANER_ID,
            "ok",
            crate::i18n::et(
                &lang(),
                "under threshold ({} paths watched)",
                &[&paths.len().to_string()],
            ),
            None,
            0,
        ));
    }
    (report, fired)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn once_fires_and_respects_hysteresis() {
        let dir = std::env::temp_dir().join("sweep_watch_test");
        let _ = std::fs::create_dir_all(&dir);
        // Eşik 0: her dizin alarm üretir; ikinci turda tekrar bildirilmez.
        let mut alerted = std::collections::HashSet::new();
        let (first, fired) = super::once(std::slice::from_ref(&dir), 0.0, 0, &mut alerted);
        assert!(fired);
        assert!(!first.entries.is_empty());
        let (second, fired_again) = super::once(std::slice::from_ref(&dir), 0.0, 0, &mut alerted);
        assert!(!fired_again);
        assert!(second.entries.iter().any(|e| e.option == "ok"));
        // Eşik 100: boş tmp dizini alarm üretmez.
        let mut fresh = std::collections::HashSet::new();
        let (_, calm) = super::once(std::slice::from_ref(&dir), 100.0, 0, &mut fresh);
        assert!(!calm);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_paths_ignored() {
        let hits = check(&[PathBuf::from("/yok-boyle-bir-dizin-sweep")], 1.0);
        assert!(hits.is_empty());
    }

    #[test]
    fn escaping_is_safe() {
        assert_eq!(escape_osascript("a\"b\\c"), "a\\\"b\\\\c");
        assert_eq!(escape_ps("it's"), "it''s");
        assert_eq!(escape_ps("plain"), "plain");
    }
}
