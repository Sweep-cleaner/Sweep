//! Filesystem-backed actions: `delete`, `shred`, `truncate`.
//!
//! This is the hot path. `scan_paths` produces candidates in contents-first
//! order (children before their parent) so a directory is only `rmdir`ed once
//! it is already empty, and the per-candidate work is kept allocation-light.

use std::path::PathBuf;

use crate::action::context::RunContext;
use crate::action::filter::ActionFilter;
use crate::core::error::Error;
use crate::core::report::{Entry, EntryKind, Report};
use crate::definition::model::{ActionDef, SearchKind, VarSet};
use crate::fsutil::delete::{delete, truncate_file, DeleteOptions};
use crate::fsutil::size::size_of;
use crate::fsutil::walk::{glob_paths, scan_paths, ScanOptions};

/// Run one filesystem action (delete / shred / truncate).
pub fn run(
    def: &ActionDef,
    vars: &VarSet,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
) -> Report {
    let mut report = Report::new();
    let filter = match ActionFilter::compile(def) {
        Ok(filter) => filter,
        Err(err) => {
            report.fail(cleaner, option, err.to_string());
            return report;
        }
    };

    let wants_shred = ctx.global_shred || def.command == "shred";

    for root in crate::definition::model::expand_action_paths(def, vars) {
        let roots = collect_roots(&root, def, &filter);
        for path in roots {
            if ctx.cancelled() {
                report.aborted = true;
                return report;
            }
            run_one(
                &path,
                def,
                wants_shred,
                &filter,
                ctx,
                cleaner,
                option,
                &mut report,
            );
        }
    }

    report
}

/// Expand a single declared root into every concrete path it matches.
fn collect_roots(root: &std::path::Path, def: &ActionDef, filter: &ActionFilter) -> Vec<PathBuf> {
    match def.search {
        SearchKind::File => {
            if root.exists() {
                vec![root.to_path_buf()]
            } else {
                Vec::new()
            }
        }
        SearchKind::Glob => match glob_paths(&root.to_string_lossy()) {
            Ok(paths) => paths
                .into_iter()
                .filter(|p| filter.accepts_name(p, p.is_dir()))
                .collect(),
            Err(err) => {
                log::debug!("glob failed for {}: {}", root.display(), err);
                Vec::new()
            }
        },
        SearchKind::WalkFiles => {
            let options = ScanOptions::files().with_same_filesystem(true);
            let options = apply_depth(options, def);
            scan_paths(root, &options)
                .into_iter()
                .filter(|p| filter.accepts(p))
                .collect()
        }
        SearchKind::WalkAll | SearchKind::WalkTop => {
            let mut options = ScanOptions::all().with_same_filesystem(true);
            options = apply_depth(options, def);
            let mut paths: Vec<PathBuf> = scan_paths(root, &options)
                .into_iter()
                .filter(|p| filter.accepts(p))
                .collect();
            if def.search == SearchKind::WalkTop {
                paths.push(root.to_path_buf());
            }
            paths
        }
        SearchKind::Deep => {
            // `search="deep"` is delegated to the deep-scan engine, which
            // handles the root-level globbing itself.
            Vec::new()
        }
    }
}

fn apply_depth(options: ScanOptions, def: &ActionDef) -> ScanOptions {
    match def.max_depth {
        Some(depth) => options.with_max_depth(depth),
        None => options,
    }
}

/// Act on one concrete path, recording what happened.
#[allow(clippy::too_many_arguments)]
fn run_one(
    path: &std::path::Path,
    def: &ActionDef,
    wants_shred: bool,
    filter: &ActionFilter,
    ctx: &RunContext,
    cleaner: &str,
    option: &str,
    report: &mut Report,
) {
    // Cheap name filter already applied during collection; the metadata filter
    // may not have been (glob results are name-filtered only).
    if !filter.accepts_name(path, path.is_dir()) {
        return;
    }
    if !filter.accepts(path) {
        return;
    }

    // The one gate that protects the user's data.
    match ctx.guard.check(path, def.search == SearchKind::File) {
        Ok(()) => {}
        Err(Error::Aborted) => {
            report.push(Entry::skipped(cleaner, option, path));
            return;
        }
        Err(Error::Link(_)) => {
            // Never follow a link; the link *name* may still be removed.
            if !path.is_symlink() {
                report.push(Entry::skipped(cleaner, option, path));
                return;
            }
        }
        Err(Error::ProtectedPath(_)) => {
            report.push(Entry::skipped(cleaner, option, path));
            return;
        }
        Err(other) => {
            report.fail(cleaner, option, format!("{}: {}", path.display(), other));
            return;
        }
    }

    // Son güvenlik doğrulaması: yukarıdaki Guard kararı güven bilgisidir,
    // silme öncesi protected-path / symlink-hedef / kritik-dizin / yetki
    // denetimleri yine zorunlu olarak çalışır (`guarded_delete` ile aynı
    // katman). Bağ adının kendisi silinirken Guard kararı yeterlidir:
    // `delete` bağı asla izlemez, yalnızca adı kaldırır.
    if !ctx.dry_run && !path.is_symlink() {
        if let Err(reason) = crate::deep::safety::check_path(path, ctx) {
            log::warn!("skipped {}: {reason}", path.display());
            report.push(Entry::skipped(cleaner, option, path));
            return;
        }
    }

    // A directory contributes no bytes of its own: the walk is `contents_first`,
    // so every file underneath it is reported (and counted) separately. Sizing
    // the directory recursively here used to mean a full re-walk of every
    // subtree — O(files × depth) extra stats on a cache tree, which is what made
    // `preview --all` / `clean --all` appear to hang — and for a *non-empty*
    // directory the number was thrown away anyway: `delete` uses `remove_dir`
    // (never `remove_dir_all`), so a non-empty directory is only ever skipped.
    let (reclaimed, preview_bytes) = if path.is_dir() {
        (0, 0)
    } else {
        let bytes = size_of(path).unwrap_or(0);
        (bytes, bytes)
    };

    // Yedek + denetim kaydı yalnızca gerçek dosyalarda tutulur: dizin ve bağ
    // adları yedeksizdir; `clean` diye loglansalar geri alma kapsamını
    // kirletirlerdi (dizinler geri yüklemede yeniden açılır).
    let auditable = !ctx.dry_run && path.is_file() && !path.is_symlink();
    match def.command.as_str() {
        "truncate" => {
            if ctx.dry_run {
                report.push(Entry::new(
                    EntryKind::Truncate,
                    cleaner,
                    option,
                    EntryKind::Truncate.label(),
                    Some(path),
                    preview_bytes,
                ));
                return;
            }
            if auditable {
                crate::deep::safety::maybe_backup(ctx, path);
            }
            match truncate_file(path) {
                Ok(()) => {
                    if auditable {
                        crate::deep::safety::log_operation(
                            ctx, cleaner, option, path, reclaimed, false,
                        );
                    }
                    report.push(Entry::new(
                        EntryKind::Truncate,
                        cleaner,
                        option,
                        EntryKind::Truncate.label(),
                        Some(path),
                        0,
                    ))
                }
                Err(err) => report.fail(cleaner, option, err.to_string()),
            }
        }
        "shred" | "delete" => {
            let kind = if wants_shred {
                EntryKind::Shred
            } else {
                EntryKind::Delete
            };
            if ctx.dry_run {
                report.push(Entry::new(
                    kind,
                    cleaner,
                    option,
                    kind.label(),
                    Some(path),
                    preview_bytes,
                ));
                return;
            }
            let options = if wants_shred {
                DeleteOptions::shred(ctx.shred_spec)
            } else {
                DeleteOptions::simple()
            };
            if auditable {
                crate::deep::safety::maybe_backup(ctx, path);
            }
            match delete(path, options) {
                Ok(true) => {
                    if auditable {
                        crate::deep::safety::log_operation(
                            ctx, cleaner, option, path, reclaimed, false,
                        );
                    }
                    report.push(Entry::new(
                        kind,
                        cleaner,
                        option,
                        kind.label(),
                        Some(path),
                        reclaimed,
                    ))
                }
                Ok(false) => { /* not removed (e.g. mount point) */ }
                Err(err) if err.is_benign() => {
                    log::debug!("benign delete failure on {}: {}", path.display(), err);
                }
                Err(err) => report.fail(cleaner, option, err.to_string()),
            }
        }
        other => {
            report.fail(
                cleaner,
                option,
                format!("file action got non-file command '{other}'"),
            );
        }
    }
}
