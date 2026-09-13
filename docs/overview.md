# Documentation implementation overview

## What was added

Created a Rust-subproject documentation set for `Sweep` / `sweep`, based
on the existing BleachBit source concepts and the implemented Rust code:

- `README.md` — user-facing overview, safety warning, CLI examples, selectors,
  global flags, configuration, platform scope, and project links.
- `CONTRIBUTING.md` — contributor workflow, cleaner authoring rules, safety
  boundaries, importer provenance, and static validation checklist.
- `docs/cleaner-format.md` — native TOML schema, examples, search modes,
  filters, providers, trust model, and troubleshooting.
- `docs/compatibility.md` — source-derived cleaner/platform matrix, provider
  availability caveats, import boundaries, and storage/shredding limitations.
- `THIRD_PARTY.md` — GPL-3.0-or-later notice, BleachBit-inspired provenance,
  upstream resource links, dependency categories, and packaging guidance.

The original repository root `README.md` was intentionally left unchanged.

## Key documentation decisions

- Described `Sweep` as an independent BleachBit-inspired implementation,
  not an official replacement or a claim of complete compatibility.
- Marked `clean`, `deepscan` without `--preview`, `--shred`, `winreg`, external
  processes, package-manager actions, and `wipe` as high-impact operations.
- Documented secure deletion and free-space wiping as best effort, without
  promising recovery prevention on SSDs, snapshots, backups, journals, or
  copy-on-write filesystems.
- Preserved the external-definition trust boundary: `process` and `winreg`
  actions are removed unless `--trust-external` is explicitly used.
- Called out the configuration default-path ambiguity and recommended
  `--config PATH` when the exact location matters.

## Validation performed

- Parsed and semantically checked all 109 embedded TOML cleaners with the
  existing `verify_tomls.py` validator.
- Confirmed all five planned documentation files exist and all 11 local
  Markdown links resolve.
- Checked all documentation for trailing whitespace and conflict markers.
- Cross-checked package metadata: README path, `sweep` binary name, and
  `GPL-3.0-or-later` license.
- No Rust compilation command was run.
