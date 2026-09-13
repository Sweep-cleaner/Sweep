//! The cleaning engine: turns a selection of `cleaner.option` pairs into a
//! [`Report`].
//!
//! Design points, compared with BleachBit's `Worker`:
//!
//! * **parallel by option** — each `(cleaner, option)` runs on its own rayon
//!   task, so a slow Firefox cache walk does not block the apt cache walk;
//! * **one append-only report** instead of a dict mutated in place, which
//!   makes the parallel merge a plain `extend`;
//! * **running-app gate** — a cleaner whose target is live is skipped (unless
//!   `--force`) and the reason is recorded, not silently swallowed;
//! * **deep scan** is folded into the same pass so a cleaner can mix ordinary
//!   and deep actions.

use rayon::prelude::*;

use crate::action::context::RunContext;
use crate::core::error::{Error, Result};
use crate::core::report::Report;
use crate::definition::model::SearchKind;
use crate::definition::CleanerRegistry;

/// One concrete `cleaner.option` the user asked to run.
#[derive(Debug, Clone)]
pub struct SelectedOption {
    /// Cleaner id.
    pub cleaner: String,
    /// Option id.
    pub option: String,
}

/// Resolve user selectors into concrete `(cleaner, option)` pairs.
///
/// A selector is one of:
/// * `*` / `all` — every active option of every cleaner that applies here;
/// * `cleaner` — every option of that cleaner (case-insensitive);
/// * `cleaner.option` — exactly that option (cleaner part case-insensitive,
///   must be active on this platform);
/// * a glob (`fire*`, `*fox`) — every option of every matching cleaner.
///
/// Exclusions use the same language. An empty result is an error.
pub fn resolve(
    registry: &CleanerRegistry,
    selectors: &[String],
    all: bool,
    exclude: &[String],
) -> Result<Vec<SelectedOption>> {
    let mut out = Vec::new();

    for selector in selectors {
        if selector == "*" || selector.eq_ignore_ascii_case("all") {
            for cleaner in registry.all() {
                for option in cleaner.def.active_options() {
                    out.push(SelectedOption {
                        cleaner: cleaner.def.id.clone(),
                        option: option.id.clone(),
                    });
                }
            }
            continue;
        }

        if let Some((cleaner_id, option_id)) = selector.split_once('.') {
            let cleaner = lookup_cleaner(registry, cleaner_id)
                .ok_or_else(|| Error::UnknownCleaner(cleaner_id.to_string()))?;
            let canonical = cleaner.def.id.clone();
            match cleaner.def.option(option_id) {
                Some(opt) if opt.os.matches() => out.push(SelectedOption {
                    cleaner: canonical,
                    option: option_id.to_string(),
                }),
                _ => {
                    return Err(Error::UnknownOption {
                        cleaner: cleaner_id.to_string(),
                        option: option_id.to_string(),
                    })
                }
            }
            continue;
        }

        // Bare cleaner id (case-insensitive) or glob.
        let ids = lookup_cleaners(registry, selector);
        if ids.is_empty() {
            return Err(Error::UnknownCleaner(selector.to_string()));
        }
        for id in ids {
            let cleaner = registry
                .get(&id)
                .expect("matching selector resolves to a known cleaner");
            for option in cleaner.def.active_options() {
                out.push(SelectedOption {
                    cleaner: id.clone(),
                    option: option.id.clone(),
                });
            }
        }
    }

    // `--all` with no selectors.
    if all && out.is_empty() {
        for cleaner in registry.all() {
            for option in cleaner.def.active_options() {
                out.push(SelectedOption {
                    cleaner: cleaner.def.id.clone(),
                    option: option.id.clone(),
                });
            }
        }
    }

    // Apply exclusions: same selector language as inclusion
    // (exact `cleaner`, exact `cleaner.option`, or glob).
    if !exclude.is_empty() {
        let mut excluded: std::collections::HashSet<(String, String)> =
            std::collections::HashSet::new();
        for pattern in exclude {
            if let Some((cleaner_id, option_pat)) = pattern.split_once('.') {
                for id in lookup_cleaners(registry, cleaner_id) {
                    if let Some(cleaner) = registry.get(&id) {
                        if option_pat.contains(['*', '?', '[', ']']) {
                            if let Ok(matcher) = globset::GlobBuilder::new(option_pat)
                                .case_insensitive(true)
                                .literal_separator(true)
                                .build()
                            {
                                let matcher = matcher.compile_matcher();
                                for option in cleaner.def.active_options() {
                                    if matcher.is_match(&option.id) {
                                        excluded.insert((id.clone(), option.id.clone()));
                                    }
                                }
                            }
                        } else {
                            excluded.insert((id.clone(), option_pat.to_string()));
                        }
                    }
                }
            } else {
                for id in lookup_cleaners(registry, pattern) {
                    if let Some(cleaner) = registry.get(&id) {
                        for option in cleaner.def.active_options() {
                            excluded.insert((id.clone(), option.id.clone()));
                        }
                    }
                }
            }
        }
        out.retain(|item| !excluded.contains(&(item.cleaner.clone(), item.option.clone())));
    }

    // De-duplicate while preserving order.
    let mut seen = std::collections::HashSet::new();
    out.retain(|item| seen.insert((item.cleaner.clone(), item.option.clone())));

    if out.is_empty() {
        return Err(Error::msg("nothing selected"));
    }

    Ok(out)
}

/// Case-insensitive cleaner lookup: exact id first (any case), then glob.
fn lookup_cleaner<'a>(
    registry: &'a CleanerRegistry,
    id: &str,
) -> Option<&'a crate::definition::model::LoadedCleaner> {
    if let Some(found) = registry.get(id) {
        return Some(found);
    }
    let lower = id.to_lowercase();
    registry.all().find(|c| c.def.id.to_lowercase() == lower)
}

/// All cleaner ids matching a bare selector (exact, case-insensitive, or glob).
fn lookup_cleaners(registry: &CleanerRegistry, selector: &str) -> Vec<String> {
    if let Some(found) = lookup_cleaner(registry, selector) {
        return vec![found.def.id.clone()];
    }
    registry.matching(selector)
}

/// Run a selection, returning the merged [`Report`].
///
/// A real (non-preview) run first appends a `run` marker to the operation
/// log so `undo --last` / `--run` can group the deletions that follow it.
pub fn run(
    registry: &CleanerRegistry,
    selection: &[SelectedOption],
    ctx: &RunContext,
    force: bool,
) -> Report {
    if !ctx.dry_run {
        let summary = selection
            .iter()
            .map(|s| format!("{}.{}", s.cleaner, s.option))
            .collect::<Vec<_>>()
            .join(", ");
        crate::deep::safety::log_run_start(ctx, &summary);
    }
    let watch = crate::core::report::Stopwatch::start();
    let items: Vec<(String, String)> = selection
        .iter()
        .map(|s| (s.cleaner.clone(), s.option.clone()))
        .collect();

    let reports: Vec<Report> = items
        .par_iter()
        .map(|(cleaner_id, option_id)| run_one(registry, cleaner_id, option_id, ctx, force))
        .collect();

    let mut merged = Report::new();
    for report in reports {
        merged.merge(report);
    }
    merged.finish(watch);
    merged
}

/// Run a single `(cleaner, option)`.
fn run_one(
    registry: &CleanerRegistry,
    cleaner_id: &str,
    option_id: &str,
    ctx: &RunContext,
    force: bool,
) -> Report {
    let mut report = Report::new();

    let Some(loaded) = registry.get(cleaner_id) else {
        report.fail(cleaner_id, option_id, "cleaner not found");
        return report;
    };
    let cleaner = &loaded.def;

    if cleaner.is_running() && !force {
        let reason = cleaner
            .running_reason()
            .unwrap_or_else(|| "target application is running".to_string());
        report.fail(
            cleaner_id,
            option_id,
            format!("skipped: {reason} (use --force to override)"),
        );
        return report;
    }

    let Some(option) = cleaner.option(option_id) else {
        report.fail(cleaner_id, option_id, "option not found");
        return report;
    };

    let vars = cleaner.resolve_vars();

    // Run every ordinary action.
    for action in &option.actions {
        if action.search == SearchKind::Deep {
            continue; // handled below in one pass
        }
        let sub = crate::action::run(action, &vars, ctx, cleaner_id, option_id);
        report.merge(sub);
    }

    // Collect deep-scan roots from this option and run them together.
    let deep_roots: Vec<std::path::PathBuf> = option
        .actions
        .iter()
        .filter(|a| a.search == SearchKind::Deep)
        .flat_map(|a| crate::definition::model::expand_action_paths(a, &vars))
        .collect();
    if !deep_roots.is_empty() {
        let rules =
            crate::engine::deepscan::compile_rules(&crate::engine::deepscan::default_patterns());
        let sub = crate::engine::deepscan::scan(&deep_roots, &rules, ctx);
        report.merge(sub);
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::model::{CleanerDef, OptionDef, OsFilter, Trust};
    use std::path::PathBuf;

    fn option(id: &str) -> OptionDef {
        OptionDef {
            id: id.to_string(),
            label: format!("{id} label"),
            description: String::new(),
            warning: None,
            actions: Vec::new(),
            os: OsFilter::any(),
        }
    }

    /// Two cleaners: `alpha` (cache, logs) and `beta` (cache).
    fn registry() -> CleanerRegistry {
        let mut reg = CleanerRegistry::new();
        let mut alpha = CleanerDef::new("alpha", "Alpha");
        alpha.description = "alpha desc".into();
        alpha.options = vec![option("cache"), option("logs")];
        reg.register(alpha, Trust::Trusted, PathBuf::from("alpha.toml"));
        let mut beta = CleanerDef::new("beta", "Beta");
        beta.options = vec![option("cache")];
        reg.register(beta, Trust::Trusted, PathBuf::from("beta.toml"));
        reg
    }

    fn keys(selection: &[SelectedOption]) -> Vec<String> {
        selection
            .iter()
            .map(|s| format!("{}.{}", s.cleaner, s.option))
            .collect()
    }

    fn resolve_ok(selectors: &[&str], all: bool, exclude: &[&str]) -> Vec<String> {
        let reg = registry();
        let owned: Vec<String> = selectors.iter().map(|s| s.to_string()).collect();
        let exc: Vec<String> = exclude.iter().map(|s| s.to_string()).collect();
        keys(&resolve(&reg, &owned, all, &exc).unwrap())
    }

    #[test]
    fn selector_forms() {
        assert_eq!(
            resolve_ok(&["alpha"], false, &[]),
            ["alpha.cache", "alpha.logs"]
        );
        assert_eq!(resolve_ok(&["alpha.cache"], false, &[]), ["alpha.cache"]);
        // Cleaner ids are case-insensitive.
        assert_eq!(resolve_ok(&["ALPHA"], false, &[]).len(), 2);
        // A bare glob matches cleaner ids.
        assert_eq!(
            resolve_ok(&["alp*"], false, &[]),
            ["alpha.cache", "alpha.logs"]
        );
        // `*` / `all` means everything.
        assert_eq!(resolve_ok(&["*"], false, &[]).len(), 3);
        assert_eq!(resolve_ok(&["all"], false, &[]).len(), 3);
        // `--all` with no selectors is the same thing.
        assert_eq!(resolve_ok(&[], true, &[]).len(), 3);
    }

    #[test]
    fn exclusions_use_the_same_language() {
        let all = resolve_ok(&["*"], false, &["alpha.cache"]);
        assert!(!all.contains(&"alpha.cache".to_string()));
        assert_eq!(all.len(), 2);
        // Excluding a whole cleaner drops every one of its options.
        assert_eq!(
            resolve_ok(&["*"], false, &["beta"]),
            ["alpha.cache", "alpha.logs"]
        );
        // Option globs work inside an exclusion too.
        assert!(!resolve_ok(&["*"], false, &["alpha.l*"]).contains(&"alpha.logs".to_string()));
        // An exclusion that matches nothing is not an error.
        assert_eq!(resolve_ok(&["*"], false, &["nope"]).len(), 3);
    }

    #[test]
    fn duplicates_collapse() {
        // `alpha` already expands to alpha.cache; naming it again adds nothing.
        assert_eq!(
            resolve_ok(&["alpha", "alpha.cache"], false, &[]),
            ["alpha.cache", "alpha.logs"]
        );
    }

    #[test]
    fn bad_selectors_are_errors() {
        let reg = registry();
        assert!(resolve(&reg, &["nope".into()], false, &[]).is_err());
        assert!(resolve(&reg, &["alpha.nope".into()], false, &[]).is_err());
        assert!(resolve(&reg, &["nope.nope".into()], false, &[]).is_err());
        // Nothing selected at all is an error rather than a silent no-op.
        assert!(resolve(&reg, &[], false, &[]).is_err());
    }
}
