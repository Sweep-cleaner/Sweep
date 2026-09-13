//! Markdown raporu (`--markdown PATH`).
//!
//! HTML raporunun düz-metin kardeşi: bir CI işi, GitHub issue'su veya PR
//! yorumuna yapıştırılabilir. Bağımlılık yok; `--html` ile aynı özet ve
//! kategori bölümlerini üretir, yalnızca biçim farklıdır.

use std::collections::BTreeMap;
use std::path::Path;

use super::report::Report;

/// Tablo hücrelerini bozmayacak kaçış: `|` ve satır sonları.
fn cell(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace(['\n', '\r'], " ")
}

/// Kod bloğu içinde güvenle gösterilecek metin.
///
/// GFM, kod alanının *içinde* bile `|` karakterinin kaçırılmasını ister;
/// kaçırılmazsa hücre bölünür ve tablo bozulur. Ters eğik çizgiye dokunulmaz:
/// kod alanı birebir yazdırılır, Windows yolları olduğu gibi kalmalı.
fn inline(text: &str) -> String {
    let escaped = text.replace('|', "\\|").replace(['\n', '\r'], " ");
    let backticks = escaped.chars().filter(|c| *c == '`').count();
    if backticks > 0 || escaped.is_empty() {
        // En uzun backtick dizisinden bir fazlasıyla sarmala.
        let fence = "`".repeat(backticks + 1);
        format!("{fence} {escaped} {fence}")
    } else {
        format!("`{escaped}`")
    }
}

fn human(bytes: u64) -> String {
    crate::fsutil::size::bytes_to_human(bytes, false)
}

/// (cleaner, option) → (dosya sayısı, bayt) özeti.
fn by_option(report: &Report) -> BTreeMap<(String, String), (u64, u64)> {
    let mut out: BTreeMap<(String, String), (u64, u64)> = BTreeMap::new();
    for entry in &report.entries {
        if entry.reclaimed == 0 && !entry.kind.counts_as_deleted() {
            continue;
        }
        let slot = out
            .entry((entry.cleaner.clone(), entry.option.clone()))
            .or_insert((0, 0));
        slot.0 += u64::from(entry.kind.counts_as_deleted());
        slot.1 += entry.reclaimed;
    }
    out
}

/// Raporu Markdown metnine çevir.
pub fn to_markdown(report: &Report, title: &str) -> String {
    let mut out = String::new();

    out.push_str(&format!("# {}\n\n", cell(title)));

    // --- özet -------------------------------------------------------------
    out.push_str("| reclaimed | files removed | special ops | skipped | errors | duration |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- |\n");
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} | {} |\n\n",
        human(report.reclaimed()),
        report.files_removed(),
        report.special_operations(),
        report.skipped(),
        report.errors.len(),
        if report.duration_ms > 0 {
            format!("{} ms", report.duration_ms)
        } else {
            "—".to_string()
        },
    ));
    if report.aborted {
        out.push_str("> **Aborted** — the run was cancelled before completion.\n\n");
    }

    // --- kategori ---------------------------------------------------------
    let cats = by_option(report);
    if !cats.is_empty() {
        out.push_str("## By category\n\n");
        out.push_str("| cleaner.option | files | size |\n| --- | --- | --- |\n");
        for ((cleaner, option), (files, bytes)) in &cats {
            out.push_str(&format!(
                "| {}.{} | {} | {} |\n",
                cell(cleaner),
                cell(option),
                files,
                human(*bytes)
            ));
        }
        out.push('\n');
    }

    // --- girdiler ---------------------------------------------------------
    const ROW_CAP: usize = 500;
    if !report.entries.is_empty() {
        out.push_str("## Entries\n\n");
        out.push_str("| kind | target | detail | path | size |\n| --- | --- | --- | --- | --- |\n");
        for entry in report.entries.iter().take(ROW_CAP) {
            out.push_str(&format!(
                "| {} | {}.{} | {} | {} | {} |\n",
                cell(entry.kind.label()),
                cell(&entry.cleaner),
                cell(&entry.option),
                inline(&entry.label),
                entry
                    .path
                    .as_ref()
                    .map(|p| inline(&p.display().to_string()))
                    .unwrap_or_else(|| "—".to_string()),
                if entry.reclaimed > 0 {
                    human(entry.reclaimed)
                } else {
                    "—".to_string()
                },
            ));
        }
        if report.entries.len() > ROW_CAP {
            out.push_str(&format!(
                "\n_{} more entries omitted._\n",
                report.entries.len() - ROW_CAP
            ));
        }
        out.push('\n');
    }

    // --- hatalar ----------------------------------------------------------
    if !report.errors.is_empty() {
        out.push_str("## Failures\n\n");
        out.push_str("| target | message |\n| --- | --- |\n");
        for failure in &report.errors {
            out.push_str(&format!(
                "| {}.{} | {} |\n",
                cell(&failure.cleaner),
                cell(&failure.option),
                cell(&failure.message)
            ));
        }
        out.push('\n');
    }

    out.push_str(&format!("---\n_Generated by sweep {}._\n", crate::VERSION));
    out
}

/// Raporu dosyaya yaz (üst dizini açar).
pub fn write_markdown(report: &Report, title: &str, dest: &Path) -> std::io::Result<()> {
    if let Some(parent) = dest.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(dest, to_markdown(report, title))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::report::{Entry, EntryKind};

    #[test]
    fn escapes_pipes_and_renders_tables() {
        let mut report = Report::new();
        report.push(Entry::new(
            EntryKind::Delete,
            "a",
            "b",
            "we | ird",
            Some(Path::new("/tmp/a | b.txt")),
            2048,
        ));
        report.fail("a", "b", "boom | bang");
        let md = to_markdown(&report, "sweep | preview");
        // Başlık ve hücrelerde `|` kaçırılmalı, yoksa tablo bozulur.
        assert!(md.starts_with("# sweep \\| preview\n"));
        assert!(md.contains("\\| ird"));
        assert!(md.contains("boom \\| bang"));
        assert!(md.contains("## By category"));
        assert!(md.contains("## Failures"));
        assert!(md.contains(&crate::VERSION.to_string()));
    }

    #[test]
    fn empty_report_still_valid() {
        let md = to_markdown(&Report::new(), "empty");
        // reclaimed | files | special | skipped | errors | duration
        assert!(md.contains("| 0B | 0 | 0 | 0 | 0 | — |"), "{md}");
        assert!(!md.contains("## By category"));
        assert!(!md.contains("## Entries"));
    }

    #[test]
    fn duration_is_shown_when_measured() {
        let mut report = Report::new();
        report.duration_ms = 1234;
        assert!(to_markdown(&report, "t").contains("| 1234 ms |"));
    }
}
