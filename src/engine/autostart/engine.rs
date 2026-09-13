//! Yüksek seviye işlemler: listele, değiştir, geri al, geçmiş, dışa aktar.
//!
//! Her değiştirici işlem şu sırayı izler: **policy kontrolü → yedek (fail
//! closed) → uygula → yeniden okuyup doğrula → deftere yaz**. Doğrulama
//! tutmazsa sonuç `ok` değil `unverified` olur.

use std::path::{Path, PathBuf};

use crate::core::report::{Entry, EntryKind, Report};

use super::backend::{Backend, Discovery};
use super::ledger::{Ledger, LedgerEntry};
use super::store::Store;
use super::{
    filter_items, lookup, Filter, Handle, Impact, Item, Kind, Scope, Source, Trigger,
};

/// Defterdeki bir değişikliğin özeti (CLI/JSON çıktısı için de kullanılır).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Change {
    pub op: String,
    pub id: String,
    pub name: String,
    pub source: String,
    pub location: String,
    pub before: Option<String>,
    pub after: Option<String>,
    pub result: String,
    pub backup: Option<String>,
    pub detail: Option<String>,
}

impl Change {
    fn to_ledger(&self) -> LedgerEntry {
        LedgerEntry::new(
            self.op.clone(),
            self.id.clone(),
            self.name.clone(),
            self.source.clone(),
            self.location.clone(),
        )
        .with_states(self.before.clone(), self.after.clone(), self.result.clone())
        .with_backup(self.backup.clone(), self.detail.clone())
    }
}

/// Motor: arka uç + yedek deposu + denetim defteri.
pub struct Engine {
    backend: Box<dyn Backend>,
    store: Option<Store>,
    ledger: Option<Ledger>,
}

impl Engine {
    /// Testlerin enjekte edebildiği kurucu.
    pub fn new(backend: Box<dyn Backend>, store: Option<Store>, ledger: Option<Ledger>) -> Self {
        Self {
            backend,
            store,
            ledger,
        }
    }

    /// Gerçek sistem arka ucu + varsayılan veri dizini.
    pub fn system() -> Self {
        Self {
            backend: super::backend::system_backend(),
            store: Store::default_store(),
            ledger: Ledger::open_default(),
        }
    }

    pub fn backend(&self) -> &dyn Backend {
        self.backend.as_ref()
    }

    fn discover_finalized(&self, what: Discovery) -> Vec<Item> {
        let mut items = self.backend.discover(what);
        super::finalize_ids(&mut items);
        items
    }

    /// `startup list`: başlangıç girdileri + oturum açma/başlatma görevleri.
    pub fn list_startup(&self, filter: &Filter, analyze: bool) -> Vec<Item> {
        let mut items: Vec<Item> = self
            .discover_finalized(Discovery::All)
            .into_iter()
            .filter(|i| match i.kind {
                Kind::Startup => true,
                Kind::Task => matches!(i.trigger, Some(Trigger::Logon) | Some(Trigger::Boot)),
            })
            .collect();
        self.finish_list(&mut items, filter, analyze)
    }

    /// `tasks list`: tüm zamanlanmış görevler.
    pub fn list_tasks(&self, filter: &Filter, analyze: bool) -> Vec<Item> {
        let mut items = self.discover_finalized(Discovery::Tasks);
        self.finish_list(&mut items, filter, analyze)
    }

    fn finish_list(&self, items: &mut [Item], filter: &Filter, analyze: bool) -> Vec<Item> {
        if analyze {
            for item in items.iter_mut() {
                if item.impact.is_none() {
                    if let Some(command) = &item.command {
                        item.impact = Some(crate::engine::startup::analyze_impact(
                            command,
                            item.scope == Scope::System,
                        ));
                    }
                }
            }
        }
        filter_items(items, filter)
    }

    /// Bir girdiyi hedefle: önce id, sonra tam ad, sonra benzersiz alt dizi.
    pub fn find(&self, target: &str, what: Discovery) -> Result<Item, String> {
        let items = self.discover_finalized(what);
        lookup(&items, target).cloned()
    }

    fn append(&self, entry: &LedgerEntry) -> Result<(), String> {
        match &self.ledger {
            Some(ledger) => ledger.append(entry),
            None => Err("no ledger (data dir unavailable)".to_string()),
        }
    }

    /// `op` ∈ enable|disable|remove|edit (tüm girdiler).
    pub fn change(
        &self,
        op: &str,
        target: &str,
        command: Option<&str>,
        dry_run: bool,
        force: bool,
    ) -> (Report, Option<Change>) {
        self.change_in(op, target, command, dry_run, force, Discovery::All)
    }

    /// `op` ∈ enable|disable|remove|edit; `what` aramayı daraltır.
    pub fn change_in(
        &self,
        op: &str,
        target: &str,
        command: Option<&str>,
        dry_run: bool,
        force: bool,
        what: Discovery,
    ) -> (Report, Option<Change>) {
        let mut report = Report::new();
        let items = self.discover_finalized(what);
        let item = match lookup(&items, target) {
            Ok(item) => item.clone(),
            Err(err) => {
                let entry = LedgerEntry::new(op, "", target, "", "")
                    .with_states(None, None, "failed")
                    .with_backup(None, Some(err.clone()));
                let _ = self.append(&entry);
                report.fail("startup", op, err);
                return (report, None);
            }
        };

        // --- policy ---------------------------------------------------------
        if let Err(reason) = policy_check(op, &item, command, force) {
            let change = Change {
                op: op.to_string(),
                id: item.id.clone(),
                name: item.name.clone(),
                source: item.source.as_str().to_string(),
                location: item.location.clone(),
                before: Some(item.state().to_string()),
                after: None,
                result: "skipped".to_string(),
                backup: None,
                detail: Some(reason.clone()),
            };
            let _ = self.append(&change.to_ledger());
            report.push(Entry::new(
                EntryKind::Skip,
                "startup",
                op,
                format!("{}: {} — {}", op, item.name, reason),
                None,
                0,
            ));
            return (report, Some(change));
        }

        let before = item.state().to_string();
        let after = match op {
            "enable" => "enabled".to_string(),
            "disable" => "disabled".to_string(),
            "remove" => "absent".to_string(),
            _ => before.clone(),
        };

        // Zaten istenen durumda: boşuna yedek yazma, "ok" deme.
        if matches!(op, "enable" | "disable") && before == after {
            let detail = format!("already {before}");
            let change = Change {
                op: op.to_string(),
                id: item.id.clone(),
                name: item.name.clone(),
                source: item.source.as_str().to_string(),
                location: item.location.clone(),
                before: Some(before.clone()),
                after: Some(after),
                result: "skipped".to_string(),
                backup: None,
                detail: Some(detail),
            };
            let _ = self.append(&change.to_ledger());
            report.push(Entry::new(
                EntryKind::Skip,
                "startup",
                op,
                format!("{}: already {before}", item.name),
                None,
                0,
            ));
            return (report, Some(change));
        }

        // --- dry-run --------------------------------------------------------
        if dry_run {
            let detail = format!("dry-run: would {op} '{}'", item.name);
            let change = Change {
                op: op.to_string(),
                id: item.id.clone(),
                name: item.name.clone(),
                source: item.source.as_str().to_string(),
                location: item.location.clone(),
                before: Some(before),
                after: Some(after),
                result: "skipped".to_string(),
                backup: None,
                detail: Some(detail),
            };
            let _ = self.append(&change.to_ledger());
            report.push(Entry::new(
                EntryKind::Command,
                "startup",
                op,
                format!("dry-run: would {op} '{}'", item.name),
                None,
                0,
            ));
            return (report, Some(change));
        }

        // --- yedek (fail closed) -------------------------------------------
        let Some(store) = &self.store else {
            let entry = LedgerEntry::new(
                op,
                item.id.clone(),
                item.name.clone(),
                item.source.as_str(),
                item.location.clone(),
            )
            .with_states(Some(before.clone()), Some(after.clone()), "failed")
            .with_backup(None, Some("no backup store available".to_string()));
            let _ = self.append(&entry);
            report.fail(
                "startup",
                op,
                format!("{}: no backup store available — change not applied", item.name),
            );
            return (report, None);
        };
        let payload = match self.backend.capture(&item) {
            Ok(payload) => payload,
            Err(err) => {
                let entry = LedgerEntry::new(
                    op,
                    item.id.clone(),
                    item.name.clone(),
                    item.source.as_str(),
                    item.location.clone(),
                )
                .with_states(Some(before.clone()), Some(after.clone()), "failed")
                .with_backup(None, Some(format!("capture failed: {err}")));
                let _ = self.append(&entry);
                report.fail("startup", op, format!("{}: {err}", item.name));
                return (report, None);
            }
        };
        let (backup_path, _) = match store.write(
            &item.id,
            &item.name,
            op,
            Some(before.clone()),
            item.command.clone(),
            payload,
        ) {
            Ok(written) => written,
            Err(err) => {
                let entry = LedgerEntry::new(
                    op,
                    item.id.clone(),
                    item.name.clone(),
                    item.source.as_str(),
                    item.location.clone(),
                )
                .with_states(Some(before.clone()), Some(after.clone()), "failed")
                .with_backup(None, Some(format!("backup failed: {err}")));
                let _ = self.append(&entry);
                report.fail(
                    "startup",
                    op,
                    format!("{}: backup failed, change not applied: {err}", item.name),
                );
                return (report, None);
            }
        };
        let backup_rel = store.relative(&backup_path);

        // --- uygula ---------------------------------------------------------
        let applied = match op {
            "enable" => self.backend.set_enabled(&item, true),
            "disable" => self.backend.set_enabled(&item, false),
            "remove" => self.backend.remove(&item),
            "edit" => match command {
                Some(cmd) => self.backend.edit(&item, cmd),
                None => Err("--command is required for edit".to_string()),
            },
            other => Err(format!("unsupported op: {other}")),
        };
        if let Err(err) = applied {
            let entry = LedgerEntry::new(
                op,
                item.id.clone(),
                item.name.clone(),
                item.source.as_str(),
                item.location.clone(),
            )
            .with_states(Some(before.clone()), Some(after.clone()), "failed")
            .with_backup(Some(backup_rel.clone()), Some(err.clone()));
            let _ = self.append(&entry);
            report.fail("startup", op, format!("{}: {err}", item.name));
            return (report, None);
        }

        // --- doğrula --------------------------------------------------------
        let (result, detail) = self.verify_change(op, &item, &after, command);
        let change = Change {
            op: op.to_string(),
            id: item.id.clone(),
            name: item.name.clone(),
            source: item.source.as_str().to_string(),
            location: item.location.clone(),
            before: Some(before),
            after: Some(after.clone()),
            result: result.clone(),
            backup: Some(backup_rel),
            detail,
        };
        if let Err(err) = self.append(&change.to_ledger()) {
            report.fail("startup", op, format!("ledger write failed: {err}"));
        }
        if result == "ok" {
            report.push(Entry::new(
                EntryKind::Command,
                "startup",
                op,
                format!("{}: {}", op, item.name),
                None,
                0,
            ));
        } else {
            report.fail(
                "startup",
                op,
                format!(
                    "{}: change applied but not verified (expected {after})",
                    item.name
                ),
            );
        }
        (report, Some(change))
    }

    /// Uygulama sonrası durumu yeniden oku; komut değişikliğini de doğrula.
    fn verify_change(
        &self,
        op: &str,
        item: &Item,
        expected: &str,
        command: Option<&str>,
    ) -> (String, Option<String>) {
        let observed = match self.backend.state_of(item) {
            Ok(state) => state,
            Err(err) => return ("unverified".to_string(), Some(format!("post-check failed: {err}"))),
        };
        if observed != expected {
            return (
                "unverified".to_string(),
                Some(format!("post-check saw '{observed}', expected '{expected}'")),
            );
        }
        if op == "edit" {
            if let Some(wanted) = command {
                let items = self.discover_finalized(Discovery::All);
                let now = items.iter().find(|i| i.id == item.id).and_then(|i| i.command.clone());
                if now.as_deref() != Some(wanted) {
                    return (
                        "unverified".to_string(),
                        Some(format!("post-check command is {now:?}, expected {wanted:?}")),
                    );
                }
            }
        }
        ("ok".to_string(), None)
    }

    /// Daha önce geri alınmış yedek yolları (göreli) — tekrar geri alma
    /// geçmişte bir adım daha geriye gitsin diye.
    fn consumed_backups(&self) -> std::collections::HashSet<String> {
        self.history(0)
            .into_iter()
            .filter(|e| {
                e.op == "rollback"
                    && e.result != "skipped"
                    && e.result != "failed"
                    && e.backup.is_some()
            })
            .filter_map(|e| e.backup)
            .collect()
    }

    /// Yedekleri eskiyen yeniye seç; tüketilmişleri ve (varsa) kimliğe
    /// uymayanları atla.
    fn unconsumed(
        &self,
        store: &Store,
        id: Option<&str>,
    ) -> Option<(PathBuf, super::store::BackupFile)> {
        let consumed = self.consumed_backups();
        store
            .list()
            .into_iter()
            .filter(|(path, file)| {
                !consumed.contains(&store.relative(path))
                    && id.map(|wanted| file.id == wanted).unwrap_or(true)
            })
            .next_back()
    }

    /// Son yedeği (ya da bir girdinin son yedeğini) geri kur.
    pub fn rollback(
        &self,
        target: Option<&str>,
        last: bool,
        dry_run: bool,
    ) -> (Report, Option<Change>) {
        self.rollback_in(target, last, dry_run, Discovery::All)
    }

    /// Geri alma; `what` aramayı daraltır.
    pub fn rollback_in(
        &self,
        target: Option<&str>,
        last: bool,
        dry_run: bool,
        what: Discovery,
    ) -> (Report, Option<Change>) {
        let mut report = Report::new();
        let Some(store) = &self.store else {
            report.fail("startup", "rollback", "no backup store available");
            return (report, None);
        };

        let resolved = if last || target.is_none() {
            self.unconsumed(store, None)
        } else {
            let target = target.unwrap();
            // Girdiyi çözüp kimliğinden yedeği bul; olmazsa ada göre ara.
            let id = self.find(target, what).ok().map(|item| item.id);
            self.unconsumed(store, id.as_deref()).or_else(|| {
                // Kimliği çözemediysek ada göre en yeni tüketilmemiş yedek.
                let consumed = self.consumed_backups();
                store
                    .list()
                    .into_iter()
                    .filter(|(path, file)| {
                        !consumed.contains(&store.relative(path))
                            && file.name.eq_ignore_ascii_case(target)
                    })
                    .next_back()
            })
        };

        let Some((path, backup)) = resolved else {
            let entry = LedgerEntry::new(
                "rollback",
                "",
                target.unwrap_or("--last"),
                "",
                "",
            )
            .with_states(None, None, "failed")
            .with_backup(None, Some("no backup found".to_string()));
            let _ = self.append(&entry);
            report.fail(
                "startup",
                "rollback",
                format!("no backup found for {}", target.unwrap_or("--last")),
            );
            return (report, None);
        };

        if let Err(err) = store.read(&path) {
            let entry = LedgerEntry::new(
                "rollback",
                backup.id.clone(),
                backup.name.clone(),
                "",
                "",
            )
            .with_states(None, None, "failed")
            .with_backup(Some(store.relative(&path)), Some(err.clone()));
            let _ = self.append(&entry);
            report.fail("startup", "rollback", err);
            return (report, None);
        }

        let backup_rel = store.relative(&path);
        let before = backup.before.clone();
        if dry_run {
            let detail = format!("dry-run: would restore '{}' to {:?}", backup.name, backup.before);
            let change = Change {
                op: "rollback".to_string(),
                id: backup.id.clone(),
                name: backup.name.clone(),
                source: String::new(),
                location: String::new(),
                before: backup.before.clone(),
                after: backup.before.clone(),
                result: "skipped".to_string(),
                backup: Some(backup_rel),
                detail: Some(detail),
            };
            let _ = self.append(&change.to_ledger());
            report.push(Entry::new(
                EntryKind::Command,
                "rollback",
                "rollback",
                format!("dry-run: would restore '{}'", backup.name),
                None,
                0,
            ));
            return (report, Some(change));
        }

        if let Err(err) = self.backend.restore(&backup.payload) {
            let entry = LedgerEntry::new(
                "rollback",
                backup.id.clone(),
                backup.name.clone(),
                "",
                "",
            )
            .with_states(before.clone(), backup.before.clone(), "failed")
            .with_backup(Some(backup_rel.clone()), Some(err.clone()));
            let _ = self.append(&entry);
            report.fail("startup", "rollback", format!("{}: {err}", backup.name));
            return (report, None);
        }

        // Gerçekten geri geldi mi? Sistemden yeniden oku.
        let items = self.discover_finalized(Discovery::All);
        let restored = items.iter().find(|i| i.id == backup.id);
        let (result, detail) = match restored {
            Some(item) => {
                let state = self
                    .backend
                    .state_of(item)
                    .unwrap_or_else(|_| "unknown".to_string());
                let command_ok = backup
                    .before_command
                    .as_ref()
                    .map(|wanted| item.command.as_deref() == Some(wanted.as_str()))
                    .unwrap_or(true);
                if backup.before.as_deref() == Some(state.as_str()) && command_ok {
                    ("ok".to_string(), None)
                } else {
                    (
                        "unverified".to_string(),
                        Some(format!(
                            "post-rollback state '{state}' / command ok={command_ok}"
                        )),
                    )
                }
            }
            None => (
                "unverified".to_string(),
                Some("item not found after rollback".to_string()),
            ),
        };

        let change = Change {
            op: "rollback".to_string(),
            id: backup.id.clone(),
            name: backup.name.clone(),
            source: String::new(),
            location: String::new(),
            before,
            after: backup.before.clone(),
            result: result.clone(),
            backup: Some(backup_rel),
            detail,
        };
        if let Err(err) = self.append(&change.to_ledger()) {
            report.fail("startup", "rollback", format!("ledger write failed: {err}"));
        }
        if result == "ok" {
            report.push(Entry::new(
                EntryKind::Command,
                "rollback",
                "rollback",
                format!("restored: {}", backup.name),
                None,
                0,
            ));
        } else {
            report.fail(
                "startup",
                "rollback",
                format!("{}: rollback not verified", backup.name),
            );
        }
        (report, Some(change))
    }

    /// Denetim defterinin son `limit` satırı.
    pub fn history(&self, limit: usize) -> Vec<LedgerEntry> {
        self.ledger
            .as_ref()
            .map(|l| l.tail(limit))
            .unwrap_or_default()
    }

    pub fn history_json(&self, limit: usize) -> String {
        let entries = self.history(limit);
        serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string())
    }

    /// Girdileri JSON ya da CSV olarak yaz; yazılan bayt sayısını döner.
    pub fn export_items(&self, items: &[Item], to: &Path, format: &str) -> Result<u64, String> {
        let (text, _) = render_items(items, format)?;
        if let Some(parent) = to.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
            }
        }
        std::fs::write(to, text.as_bytes())
            .map_err(|e| format!("cannot write {}: {e}", to.display()))?;
        Ok(text.len() as u64)
    }

    /// Denetim defterini JSON/CSV olarak yaz.
    pub fn export_history(&self, to: &Path, format: &str, limit: usize) -> Result<u64, String> {
        let entries = self.history(limit);
        let text = match format.to_lowercase().as_str() {
            "csv" => self
                .ledger
                .as_ref()
                .map(|l| l.to_csv(&entries))
                .unwrap_or_default(),
            _ => serde_json::to_string_pretty(&entries).unwrap_or_else(|_| "[]".to_string()),
        };
        if let Some(parent) = to.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
            }
        }
        std::fs::write(to, text.as_bytes())
            .map_err(|e| format!("cannot write {}: {e}", to.display()))?;
        Ok(text.len() as u64)
    }
}

/// Uzantı ya da `--format`tan biçim seç.
pub fn resolve_format(to: &Path, format: Option<&str>) -> String {
    match format.map(|f| f.to_lowercase()) {
        Some(f) if f == "csv" => "csv".to_string(),
        Some(f) if f == "json" => "json".to_string(),
        _ => match to
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .as_deref()
        {
            Some("csv") => "csv".to_string(),
            _ => "json".to_string(),
        },
    }
}

/// Girdi listesini JSON ya da CSV metnine çevir.
pub fn render_items(items: &[Item], format: &str) -> Result<(String, &'static str), String> {
    if format.eq_ignore_ascii_case("csv") {
        let mut out = String::from(
            "id,kind,name,command,source,location,scope,enabled,trigger,category,risk,reversible,editable,removable,last_run,next_run,last_result,author\n",
        );
        for item in items {
            let cells = [
                item.id.clone(),
                item.kind.as_str().to_string(),
                item.name.clone(),
                item.command.clone().unwrap_or_default(),
                item.source.as_str().to_string(),
                item.location.clone(),
                item.scope.as_str().to_string(),
                item.enabled.to_string(),
                item.trigger.map(|t| t.as_str().to_string()).unwrap_or_default(),
                item.category.as_str().to_string(),
                item.risk.as_str().to_string(),
                item.reversible.to_string(),
                item.editable.to_string(),
                item.removable.to_string(),
                item.last_run.clone().unwrap_or_default(),
                item.next_run.clone().unwrap_or_default(),
                item.last_result.clone().unwrap_or_default(),
                item.author.clone().unwrap_or_default(),
            ];
            out.push_str(
                &cells
                    .iter()
                    .map(|c| csv_escape(c))
                    .collect::<Vec<_>>()
                    .join(","),
            );
            out.push('\n');
        }
        Ok((out, "csv"))
    } else {
        let value = serde_json::json!({
            "generated": super::now_rfc3339(),
            "items_count": items.len(),
            "items": items,
        });
        let text = serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?;
        Ok((text, "json"))
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Policy: kullanıcı isteği güvenli mi?
fn policy_check(op: &str, item: &Item, command: Option<&str>, force: bool) -> Result<(), String> {
    match op {
        "enable" | "disable" => {
            if !item.reversible {
                return Err("entry is not reversible (read-only mechanism)".to_string());
            }
            if is_protected(item) && !force {
                return Err(
                    "protected OS entry — pass --force to change it anyway (recorded)".to_string(),
                );
            }
            Ok(())
        }
        "remove" => {
            if item.risk == super::Risk::Critical {
                return Err("refusing to remove a critical entry".to_string());
            }
            if is_protected(item) {
                return Err("refusing to remove a protected OS entry".to_string());
            }
            if item.scope == Scope::System {
                return Err("refusing to remove a system-scope entry".to_string());
            }
            if !item.removable {
                return Err("entry is not removable".to_string());
            }
            Ok(())
        }
        "edit" => {
            if !item.editable {
                return Err("entry is not editable".to_string());
            }
            if command.map(|c| c.trim().is_empty()).unwrap_or(true) {
                return Err("--command is required for edit".to_string());
            }
            Ok(())
        }
        other => Err(format!("unsupported op: {other} (enable|disable|remove|edit)")),
    }
}

/// Korumalı girdi mi? (Microsoft OS görevi ya da Winlogon.)
pub fn is_protected(item: &Item) -> bool {
    matches!(&item.handle, Handle::Task { protected: true, .. }) || item.source == Source::Winlogon
}

/// Girdi listesini rapor satırlarına çevir (HTML/Markdown için).
pub fn report_items(report: &mut Report, items: &[Item]) {
    for item in items {
        report.push(Entry::new(
            EntryKind::Command,
            "startup",
            item.state(),
            format!(
                "{}{} — {}{}",
                item.name,
                format_args!(" [{}]", item.source.as_str()),
                item.command
                    .as_deref()
                    .map(|c| truncate(c, 80))
                    .unwrap_or_else(|| item.location.clone()),
                match &item.impact {
                    Some(Impact { level, .. }) => format!(" [{}]", level.as_str()),
                    None => String::new(),
                }
            ),
            None,
            0,
        ));
    }
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        text.to_string()
    } else {
        let head: String = text.chars().take(max.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::autostart::backend::fake::FakeBackend;
    use crate::engine::autostart::store::Store;
    use crate::engine::autostart::{finalize_ids, Handle, Item};

    struct Fixture {
        engine: Engine,
        backend: std::sync::Arc<FakeBackend>,
        dir: std::path::PathBuf,
    }

    fn fixture(tag: &str) -> Fixture {
        let dir = std::env::temp_dir().join(format!(
            "sweep_engine_{tag}_{}_{}",
            std::process::id(),
            super::super::short_hash(tag)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let store = Store::at(dir.join("backups"));
        let ledger = Ledger::new(dir.join("audit.jsonl"));
        let backend = std::sync::Arc::new(FakeBackend::new());
        let mut items = vec![
            make_item("OneDrive", Source::RegistryRun, Scope::User, true, true),
            make_item("SysThing", Source::RegistryRun, Scope::System, true, true),
            make_item("Broken", Source::RegistryRun, Scope::User, true, false),
        ];
        finalize_ids(&mut items);
        for item in &items {
            backend.add(item);
        }
        fixture_registered(tag, backend, store, ledger, dir)
    }

    fn make_item(
        name: &str,
        source: Source,
        scope: Scope,
        removable: bool,
        editable: bool,
    ) -> Item {
        let mut item = Item::new(
            Kind::Startup,
            name,
            source,
            format!("{}\\{}", scope.as_str(), name),
            scope,
            true,
            Some(Trigger::Logon),
            Handle::Fake(name.to_string()),
        );
        item.command = Some(format!("C:\\Program Files\\{name}\\{name}.exe"));
        item.removable = removable;
        item.editable = editable;
        item
    }

    fn fixture_registered(
        _tag: &str,
        backend: std::sync::Arc<FakeBackend>,
        store: Store,
        ledger: Ledger,
        dir: std::path::PathBuf,
    ) -> Fixture {
        let engine = Engine::new(Box::new(SharedBackend(backend.clone())), Some(store), Some(ledger));
        Fixture {
            engine,
            backend,
            dir,
        }
    }

    /// `Box<dyn Backend>` için paylaşılan sarmalayıcı (testte durumu gözler).
    struct SharedBackend(std::sync::Arc<FakeBackend>);
    impl Backend for SharedBackend {
        fn discover(&self, what: Discovery) -> Vec<Item> {
            self.0.discover(what)
        }
        fn capture(&self, item: &Item) -> Result<serde_json::Value, String> {
            self.0.capture(item)
        }
        fn set_enabled(&self, item: &Item, enabled: bool) -> Result<(), String> {
            self.0.set_enabled(item, enabled)
        }
        fn edit(&self, item: &Item, command: &str) -> Result<(), String> {
            self.0.edit(item, command)
        }
        fn remove(&self, item: &Item) -> Result<(), String> {
            self.0.remove(item)
        }
        fn restore(&self, payload: &serde_json::Value) -> Result<(), String> {
            self.0.restore(payload)
        }
        fn state_of(&self, item: &Item) -> Result<String, String> {
            self.0.state_of(item)
        }
    }

    #[test]
    fn disable_then_rollback_restores_the_previous_value() {
        let fx = fixture("disable_rollback");
        let (report, change) = fx.engine.change("disable", "OneDrive", None, false, false);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let change = change.unwrap();
        assert_eq!(change.result, "ok");
        assert_eq!(change.after.as_deref(), Some("disabled"));
        assert!(change.backup.is_some(), "yedek yolu kaydedilmeli");
        // Gerçekten kapandı mı? Arka uçtan oku.
        let item = fx.engine.find("OneDrive", Discovery::All).unwrap();
        assert!(!item.enabled);
        assert_eq!(fx.backend.state_of(&item).unwrap(), "disabled");

        // Geri al ve gerçekten açıldığını arka uçtan doğrula.
        let (report, change) = fx.engine.rollback(Some("OneDrive"), false, false);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let change = change.unwrap();
        assert_eq!(change.result, "ok", "detail: {:?}", change.detail);
        let item = fx.engine.find("OneDrive", Discovery::All).unwrap();
        assert!(item.enabled, "geri alma önceki değeri kurmalı");

        // Defter her iki işlemi de taşımalı.
        let history = fx.engine.history(0);
        assert_eq!(history.len(), 2, "{history:?}");
        assert_eq!(history[0].op, "disable");
        assert_eq!(history[0].result, "ok");
        assert_eq!(history[1].op, "rollback");
        assert_eq!(history[1].result, "ok");
        let _ = std::fs::remove_dir_all(&fx.dir);
    }

    #[test]
    fn dry_run_changes_nothing_and_writes_no_backup() {
        let fx = fixture("dryrun");
        let (report, change) = fx.engine.change("disable", "OneDrive", None, true, false);
        assert!(report.errors.is_empty());
        let change = change.unwrap();
        assert_eq!(change.result, "skipped");
        assert!(change.detail.as_deref().unwrap_or("").contains("dry-run"));
        assert!(change.backup.is_none(), "dry-run yedek yazmamalı");
        // Durum değişmedi.
        let item = fx.engine.find("OneDrive", Discovery::All).unwrap();
        assert!(item.enabled, "dry-run hiçbir şeyi değiştirmemeli");
        // Yedek dosyası oluşmadı.
        assert!(
            fx.dir.join("backups").read_dir().map(|mut d| d.next().is_none()).unwrap_or(true),
            "dry-run yedek dizini oluşturmamalı"
        );
        // Tek denetim satırı: skipped.
        let history = fx.engine.history(1);
        assert_eq!(history[0].result, "skipped");
        let _ = std::fs::remove_dir_all(&fx.dir);
    }

    #[test]
    fn remove_refuses_system_scope_and_remembers_the_attempt() {
        let fx = fixture("remove");
        let (report, change) = fx.engine.change("remove", "SysThing", None, false, false);
        assert_eq!(report.errors.len(), 0, "policy reddi hata değil, skipped");
        let change = change.unwrap();
        assert_eq!(change.result, "skipped");
        assert!(change.detail.as_deref().unwrap_or("").contains("system-scope"));
        // Girdi hâlâ orada.
        assert!(fx.engine.find("SysThing", Discovery::All).is_ok());
        // Denetim kaydı tutuldu.
        let history = fx.engine.history(1);
        assert_eq!(history[0].op, "remove");
        assert_eq!(history[0].result, "skipped");
        let _ = std::fs::remove_dir_all(&fx.dir);
    }

    #[test]
    fn edit_replaces_command_and_rollback_restores_it() {
        let fx = fixture("edit");
        let original = fx.engine.find("OneDrive", Discovery::All).unwrap().command.unwrap();
        let (report, change) = fx
            .engine
            .change("edit", "OneDrive", Some("C:\\new\\onedrive.exe"), false, false);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let change = change.unwrap();
        assert_eq!(change.result, "ok", "{:?}", change.detail);
        let item = fx.engine.find("OneDrive", Discovery::All).unwrap();
        assert_eq!(item.command.as_deref(), Some("C:\\new\\onedrive.exe"));

        let (report, change) = fx.engine.rollback(Some("OneDrive"), false, false);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(change.unwrap().result, "ok");
        let item = fx.engine.find("OneDrive", Discovery::All).unwrap();
        assert_eq!(item.command.as_deref(), Some(original.as_str()));
        let _ = std::fs::remove_dir_all(&fx.dir);
    }

    #[test]
    fn remove_then_rollback_recreates_the_item() {
        let fx = fixture("remove_rollback");
        let (report, change) = fx.engine.change("remove", "OneDrive", None, false, false);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(change.unwrap().after.as_deref(), Some("absent"));
        assert!(fx.engine.find("OneDrive", Discovery::All).is_err());
        // Geri alma girdiyi yeniden kurmalı.
        let (report, change) = fx.engine.rollback(Some("OneDrive"), false, false);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let change = change.unwrap();
        assert_eq!(change.result, "ok", "{:?}", change.detail);
        assert!(fx.engine.find("OneDrive", Discovery::All).is_ok());
        let _ = std::fs::remove_dir_all(&fx.dir);
    }

    #[test]
    fn rollback_with_last_uses_the_newest_backup() {
        let fx = fixture("last");
        fx.engine.change("disable", "OneDrive", None, false, false);
        fx.engine.change("disable", "Broken", None, false, false);
        let (report, change) = fx.engine.rollback(None, true, false);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(change.unwrap().name, "Broken");
        let _ = std::fs::remove_dir_all(&fx.dir);
    }

    #[test]
    fn no_op_toggle_is_skipped_without_a_backup() {
        let fx = fixture("noop");
        // OneDrive zaten etkin → enable anlamsız.
        let (report, change) = fx.engine.change("enable", "OneDrive", None, false, false);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        let change = change.unwrap();
        assert_eq!(change.result, "skipped");
        assert!(
            change.detail.as_deref().unwrap_or("").contains("already enabled"),
            "{:?}",
            change.detail
        );
        assert!(change.backup.is_none());
        assert!(
            fx.dir
                .join("backups")
                .read_dir()
                .map(|mut d| d.next().is_none())
                .unwrap_or(true),
            "no-op yedek yazmamalı"
        );
        let _ = std::fs::remove_dir_all(&fx.dir);
    }

    #[test]
    fn repeated_rollback_walks_back_through_history() {
        let fx = fixture("walkback");
        // disable (yedek: enabled) → enable (yedek: disabled)
        fx.engine.change("disable", "OneDrive", None, false, false);
        fx.engine.change("enable", "OneDrive", None, false, false);
        assert!(fx.engine.find("OneDrive", Discovery::All).unwrap().enabled);

        // İlk geri alma: en yeni yedeğin öncesi = disabled.
        let (report, change) = fx.engine.rollback(Some("OneDrive"), false, false);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(change.unwrap().result, "ok");
        assert!(!fx.engine.find("OneDrive", Discovery::All).unwrap().enabled);

        // İkinci geri alma geçmişte bir adım daha geriye gitmeli: enabled.
        let (report, change) = fx.engine.rollback(Some("OneDrive"), false, false);
        assert!(report.errors.is_empty(), "{:?}", report.errors);
        assert_eq!(change.unwrap().result, "ok");
        assert!(fx.engine.find("OneDrive", Discovery::All).unwrap().enabled);

        // Üçüncü geri alma: tüketilecek yedek kalmadı.
        let (report, change) = fx.engine.rollback(Some("OneDrive"), false, false);
        assert!(change.is_none());
        assert_eq!(report.errors.len(), 1);
        let _ = std::fs::remove_dir_all(&fx.dir);
    }

    #[test]
    fn unknown_target_is_recorded_as_failed() {
        let fx = fixture("missing");
        let (report, change) = fx.engine.change("disable", "NoSuchThing", None, false, false);
        assert_eq!(report.errors.len(), 1);
        assert!(change.is_none());
        let history = fx.engine.history(1);
        assert_eq!(history[0].result, "failed");
        assert!(history[0].detail.as_deref().unwrap_or("").contains("not found"));
        let _ = std::fs::remove_dir_all(&fx.dir);
    }

    #[test]
    fn export_writes_json_and_csv_with_byte_counts() {
        let fx = fixture("export");
        let items = fx.engine.list_startup(&Filter::default(), false);
        let json_path = fx.dir.join("out.json");
        let bytes = fx.engine.export_items(&items, &json_path, "json").unwrap();
        assert!(bytes > 0);
        assert_eq!(std::fs::metadata(&json_path).unwrap().len(), bytes);
        let csv_path = fx.dir.join("out.csv");
        let bytes = fx.engine.export_items(&items, &csv_path, "csv").unwrap();
        assert!(bytes > 0);
        let csv = std::fs::read_to_string(&csv_path).unwrap();
        assert!(csv.starts_with("id,kind,name,"));
        assert_eq!(csv.lines().count(), items.len() + 1);
        let _ = std::fs::remove_dir_all(&fx.dir);
    }
}
