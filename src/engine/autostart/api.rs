//! CLI yüzeyi: `sweep startup …` ve `sweep tasks …`.
//!
//! Filtre/sıralama **serileştirmeden önce** uygulanır. İnsan çıktısı kısa
//! tutulur; `--json` belgelenen yapıyı döner.

use std::path::{Path, PathBuf};

use crate::core::report::{Entry, EntryKind, Report};

use super::backend::Discovery;
use super::engine::{is_protected, resolve_format, Change, Engine};
use super::ledger::LedgerEntry;
use super::{Filter, Item, Kind, SortKey};

/// Komut satırı seçenekleri (hem `startup` hem `tasks` için ortak).
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub op: String,
    pub target: Option<String>,
    pub command: Option<String>,
    pub last: bool,
    pub dry_run: bool,
    pub force: bool,
    pub limit: usize,
    pub to: Option<PathBuf>,
    pub format: Option<String>,
    pub kind: Option<String>,
    pub scope: Option<String>,
    pub category: Option<String>,
    pub risk: Option<String>,
    pub source: Option<String>,
    pub trigger: Option<String>,
    pub enabled: bool,
    pub disabled: bool,
    pub search: Option<String>,
    pub sort: String,
    pub impact: bool,
    pub json: bool,
}

fn parse_optional<T: std::str::FromStr<Err = String>>(
    value: &Option<String>,
    flag: &str,
) -> Result<Option<T>, String> {
    match value {
        Some(raw) if !raw.trim().is_empty() => raw
            .parse::<T>()
            .map(Some)
            .map_err(|e| format!("--{flag}: {e}")),
        _ => Ok(None),
    }
}

fn build_filter(opts: &Options) -> Result<Filter, String> {
    Ok(Filter {
        kind: parse_optional::<Kind>(&opts.kind, "kind")?,
        scope: parse_optional(&opts.scope, "scope")?,
        category: parse_optional(&opts.category, "category")?,
        risk: parse_optional(&opts.risk, "risk")?,
        source: parse_optional(&opts.source, "source")?,
        trigger: parse_optional(&opts.trigger, "trigger")?,
        enabled: if opts.enabled {
            Some(true)
        } else if opts.disabled {
            Some(false)
        } else {
            None
        },
        search: opts.search.clone().filter(|s| !s.trim().is_empty()),
        sort: opts.sort.parse::<SortKey>().unwrap_or_default(),
    })
}

/// Hatayı rapora yaz ve yazdırılacak metni döndür.
fn fail(report: &mut Report, op: &str, message: impl Into<String>) -> String {
    let message = message.into();
    report.fail("startup", op, message.clone());
    format!("error: {message}")
}

/// Liste JSON zarfı: `{"report":…, "entries":[…], "items_count":N}`.
fn list_json(report: &Report, items: &[Item]) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "report": report.to_json(),
        "entries": items,
        "items_count": items.len(),
    }))
    .unwrap_or_default()
}

fn action_json(report: &Report, change: &Change, dry_run: bool) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "report": report.to_json(),
        "action": change,
        "dry_run": dry_run,
    }))
    .unwrap_or_default()
}

fn render_list(title: &str, items: &[Item], impact: bool) -> String {
    let mut out = format!("{title} — {} item(s)\n", items.len());
    for item in items {
        let state = if item.enabled { "enabled " } else { "disabled" };
        let command = item
            .command
            .as_deref()
            .map(|c| truncate(c, 72))
            .unwrap_or_else(|| item.location.clone());
        let impact_badge = match (&item.impact, impact) {
            (Some(i), true) => format!(
                " [{} {}]",
                i.level.as_str(),
                crate::engine::startup::format_ms(i.estimated_ms)
            ),
            _ => String::new(),
        };
        let trigger = if matches!(item.kind, Kind::Task) {
            format!(" ({})", item.trigger.map(|t| t.as_str()).unwrap_or("unknown"))
        } else {
            String::new()
        };
        out.push_str(&format!(
            "  [{state}] {:<20} {:<6} {:<10} {:<11} {}{}{}\n",
            item.source.as_str(),
            item.scope.as_str(),
            item.risk.as_str(),
            item.category.as_str(),
            item.name,
            trigger,
            impact_badge,
        ));
        out.push_str(&format!("        {command}\n"));
    }
    if items.is_empty() {
        out.push_str("  (none)\n");
    }
    out
}

fn render_change(change: &Change, dry_run: bool) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{}: {} [{}]\n",
        change.op, change.name, change.result
    ));
    out.push_str(&format!("  id:       {}\n", change.id));
    if !change.source.is_empty() {
        out.push_str(&format!("  source:   {}\n", change.source));
    }
    if !change.location.is_empty() {
        out.push_str(&format!("  location: {}\n", change.location));
    }
    if let Some(before) = &change.before {
        out.push_str(&format!(
            "  state:    {} -> {}\n",
            before,
            change.after.as_deref().unwrap_or("?")
        ));
    }
    if let Some(backup) = &change.backup {
        out.push_str(&format!("  backup:   {backup}\n"));
    }
    if let Some(detail) = &change.detail {
        out.push_str(&format!("  detail:   {detail}\n"));
    }
    if dry_run {
        out.push_str("  (dry-run: nothing was changed)\n");
    }
    out
}

fn render_history(entries: &[LedgerEntry]) -> String {
    let mut out = format!("audit history — {} line(s)\n", entries.len());
    for e in entries {
        out.push_str(&format!(
            "  {} {:<8} {:<9} {}{}\n",
            e.ts,
            e.op,
            e.result,
            e.name,
            e.detail
                .as_deref()
                .map(|d| format!(" — {d}"))
                .unwrap_or_default()
        ));
    }
    if entries.is_empty() {
        out.push_str("  (none)\n");
    }
    out
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        text.chars().take(max.saturating_sub(1)).collect::<String>() + "…"
    }
}

/// Değiştirici işlem sonucunu (rapor, metin) ikilisine çevir.
fn mutation_result(opts: &Options, result: (Report, Option<Change>)) -> (Report, String) {
    let (report, change) = result;
    match change {
        Some(change) => {
            let text = if opts.json {
                action_json(&report, &change, opts.dry_run)
            } else {
                render_change(&change, opts.dry_run)
            };
            (report, text)
        }
        None => {
            let message = report
                .errors
                .first()
                .map(|f| f.message.clone())
                .unwrap_or_else(|| {
                    format!("{} '{}' failed", opts.op, opts.target.as_deref().unwrap_or(""))
                });
            (report, format!("error: {message}"))
        }
    }
}

fn require_target(opts: &Options, report: &mut Report) -> Option<String> {
    match opts.target.clone().filter(|t| !t.trim().is_empty()) {
        Some(target) => Some(target),
        None => {
            fail(report, &opts.op, "target required (id or name)");
            None
        }
    }
}

fn history_text(opts: &Options, engine: &Engine, report: &mut Report) -> (Report, String) {
    let entries = engine.history(opts.limit);
    report.push(Entry::new(
        EntryKind::Command,
        "startup",
        "history",
        format!("{} audit line(s)", entries.len()),
        None,
        0,
    ));
    let text = if opts.json {
        serde_json::to_string_pretty(&serde_json::json!({
            "report": report.to_json(),
            "entries": entries,
            "items_count": entries.len(),
        }))
        .unwrap_or_default()
    } else {
        render_history(&entries)
    };
    (report.clone(), text)
}

fn export_text(
    opts: &Options,
    engine: &Engine,
    items: &[Item],
    to: &Path,
    report: &mut Report,
) -> (Report, String) {
    let format = resolve_format(to, opts.format.as_deref());
    match engine.export_items(items, to, &format) {
        Ok(bytes) => {
            report.push(Entry::new(
                EntryKind::Command,
                "startup",
                "export",
                format!("exported {} item(s) to {}", items.len(), to.display()),
                None,
                0,
            ));
            let text = if opts.json {
                serde_json::to_string_pretty(&serde_json::json!({
                    "report": report.to_json(),
                    "exported": {
                        "path": to.to_string_lossy(),
                        "format": format,
                        "bytes": bytes,
                        "items_count": items.len(),
                    }
                }))
                .unwrap_or_default()
            } else {
                format!(
                    "exported {} item(s) to {} ({} bytes, {format})",
                    items.len(),
                    to.display(),
                    bytes
                )
            };
            (report.clone(), text)
        }
        Err(err) => {
            let text = fail(report, "export", err);
            (report.clone(), text)
        }
    }
}

/// Denetim defterini dosyaya yaz (`history --to <path>`).
///
/// Girdi listesinin aksine burada keşif yapılmaz; defter olduğu gibi yazılır.
/// `--limit` yazılan satır sayısını belirler (0 = hepsi).
fn history_export_text(
    opts: &Options,
    engine: &Engine,
    to: &Path,
    report: &mut Report,
) -> (Report, String) {
    let format = resolve_format(to, opts.format.as_deref());
    match engine.export_history(to, &format, opts.limit) {
        Ok(bytes) => {
            let lines = engine.history(opts.limit).len();
            report.push(Entry::new(
                EntryKind::Command,
                "startup",
                "history",
                format!("exported {lines} audit line(s) to {}", to.display()),
                None,
                0,
            ));
            let text = if opts.json {
                serde_json::to_string_pretty(&serde_json::json!({
                    "report": report.to_json(),
                    "exported": {
                        "path": to.to_string_lossy(),
                        "format": format,
                        "bytes": bytes,
                        "items_count": lines,
                    }
                }))
                .unwrap_or_default()
            } else {
                format!(
                    "exported {lines} audit line(s) to {} ({} bytes, {format})",
                    to.display(),
                    bytes
                )
            };
            (report.clone(), text)
        }
        Err(err) => {
            let text = fail(report, "history", err);
            (report.clone(), text)
        }
    }
}

/// `--to` zorunlu; keşiften **önce** doğrula (boşuna sistem taraması yapma).
fn export_destination(opts: &Options) -> Result<PathBuf, String> {
    opts.to
        .clone()
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| "--to <path> is required".to_string())
}

/// `startup` komutunu çalıştır. `(rapor, yazdırılacak metin)` döner.
pub fn cli_startup(opts: &Options) -> (Report, String) {
    let engine = Engine::system();
    let mut report = Report::new();
    match opts.op.as_str() {
        "list" => {
            let filter = match build_filter(opts) {
                Ok(filter) => filter,
                Err(err) => {
                    let text = fail(&mut report, "list", err);
                    return (report, text);
                }
            };
            let items = engine.list_startup(&filter, opts.impact);
            super::engine::report_items(&mut report, &items);
            let text = if opts.json {
                list_json(&report, &items)
            } else {
                render_list("Startup entries", &items, opts.impact)
            };
            (report, text)
        }
        "enable" | "disable" | "remove" | "edit" => {
            let Some(target) = require_target(opts, &mut report) else {
                return (report, String::new());
            };
            let result = engine.change_in(
                &opts.op,
                &target,
                opts.command.as_deref(),
                opts.dry_run,
                opts.force,
                Discovery::All,
            );
            mutation_result(opts, result)
        }
        "rollback" => {
            let result = engine.rollback_in(
                opts.target.as_deref(),
                opts.last,
                opts.dry_run,
                Discovery::All,
            );
            mutation_result(opts, result)
        }
        // `history --to <path>` defteri dosyaya yazar (GUI'nin dışa aktarma
        // düğmesi). `--to` yoksa eski davranış korunur: satırları basar.
        "history" => match export_destination(opts) {
            Ok(to) => history_export_text(opts, &engine, &to, &mut report),
            Err(_) => history_text(opts, &engine, &mut report),
        },
        "export" => {
            let to = match export_destination(opts) {
                Ok(to) => to,
                Err(err) => {
                    let text = fail(&mut report, "export", err);
                    return (report, text);
                }
            };
            let filter = match build_filter(opts) {
                Ok(filter) => filter,
                Err(err) => {
                    let text = fail(&mut report, "export", err);
                    return (report, text);
                }
            };
            let items = engine.list_startup(&filter, false);
            export_text(opts, &engine, &items, &to, &mut report)
        }
        other => {
            let text = fail(
                &mut report,
                "args",
                format!(
                    "unknown op: {other} (list|enable|disable|remove|edit|rollback|history|export)"
                ),
            );
            (report, text)
        }
    }
}

/// `tasks` komutu. Yalnız zamanlanmış görevler üzerinde çalışır.
pub fn cli_tasks(opts: &Options) -> (Report, String) {
    let engine = Engine::system();
    let mut report = Report::new();
    match opts.op.as_str() {
        "list" => {
            let mut filter = match build_filter(opts) {
                Ok(filter) => filter,
                Err(err) => {
                    let text = fail(&mut report, "list", err);
                    return (report, text);
                }
            };
            if filter.kind.is_none() {
                filter.kind = Some(Kind::Task);
            }
            let items = engine.list_tasks(&filter, opts.impact);
            super::engine::report_items(&mut report, &items);
            let text = if opts.json {
                list_json(&report, &items)
            } else {
                render_list("Scheduled tasks", &items, opts.impact)
            };
            (report, text)
        }
        "enable" | "disable" => {
            let Some(target) = require_target(opts, &mut report) else {
                return (report, String::new());
            };
            let result = engine.change_in(
                &opts.op,
                &target,
                None,
                opts.dry_run,
                opts.force,
                Discovery::Tasks,
            );
            mutation_result(opts, result)
        }
        "rollback" => {
            let result = engine.rollback_in(
                opts.target.as_deref(),
                opts.last,
                opts.dry_run,
                Discovery::Tasks,
            );
            mutation_result(opts, result)
        }
        "history" => history_text(opts, &engine, &mut report),
        "export" => {
            let to = match export_destination(opts) {
                Ok(to) => to,
                Err(err) => {
                    let text = fail(&mut report, "export", err);
                    return (report, text);
                }
            };
            let mut filter = match build_filter(opts) {
                Ok(filter) => filter,
                Err(err) => {
                    let text = fail(&mut report, "export", err);
                    return (report, text);
                }
            };
            if filter.kind.is_none() {
                filter.kind = Some(Kind::Task);
            }
            let items = engine.list_tasks(&filter, false);
            export_text(opts, &engine, &items, &to, &mut report)
        }
        other => {
            let text = fail(
                &mut report,
                "args",
                format!("unknown op: {other} (list|enable|disable|rollback|history|export)"),
            );
            (report, text)
        }
    }
}

/// Kullanılmayan ama API yüzeyinde tutulan yardımcı: korumalılık sınaması.
pub fn item_is_protected(item: &Item) -> bool {
    is_protected(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base(op: &str) -> Options {
        Options {
            op: op.to_string(),
            sort: "name".to_string(),
            limit: 50,
            ..Options::default()
        }
    }

    #[test]
    fn filter_parsing_rejects_bad_enums() {
        let mut opts = base("list");
        opts.risk = Some("nope".into());
        let err = build_filter(&opts).unwrap_err();
        assert!(err.contains("--risk"), "{err}");

        opts.risk = Some("HIGH".into());
        let filter = build_filter(&opts).unwrap();
        assert_eq!(filter.risk, Some(super::super::Risk::High));
    }

    #[test]
    fn enabled_and_disabled_flags_map_to_filter() {
        let mut opts = base("list");
        opts.disabled = true;
        assert_eq!(build_filter(&opts).unwrap().enabled, Some(false));
        opts.enabled = true;
        assert_eq!(build_filter(&opts).unwrap().enabled, Some(true));
    }

    #[test]
    fn unknown_op_reports_an_error_without_panicking() {
        let (report, text) = cli_startup(&base("frobnicate"));
        assert_eq!(report.errors.len(), 1);
        assert!(text.contains("unknown op"), "{text}");
        let (report, text) = cli_tasks(&base("frobnicate"));
        assert_eq!(report.errors.len(), 1);
        assert!(text.contains("unknown op"), "{text}");
    }

    #[test]
    fn mutating_ops_require_a_target() {
        for op in ["enable", "disable", "remove", "edit"] {
            let (report, _) = cli_startup(&base(op));
            assert_eq!(report.errors.len(), 1, "{op}");
        }
    }

    #[test]
    fn export_requires_a_destination() {
        let (report, text) = cli_startup(&base("export"));
        assert_eq!(report.errors.len(), 1);
        assert!(text.contains("--to"), "{text}");
    }

    #[test]
    fn item_protection_helper_agrees_with_engine() {
        let item = Item::new(
            Kind::Startup,
            "X",
            super::super::Source::RegistryRun,
            "HKCU\\Run",
            super::super::Scope::User,
            true,
            None,
            super::super::Handle::Fake("x".into()),
        );
        assert!(!item_is_protected(&item));
    }
}
