# Contributing to Sweep

Thanks for helping with Sweep — a fast, safe, cross-platform system cleaner
written in Rust (GPL-3.0-or-later). PRs and issue reports are welcome.

Sweep deletes files, edits structured data, runs system services and external
commands, touches the registry on Windows, and can overwrite or wipe storage.
Every change that affects paths or actions deserves a **safety review**, not
only a style review.

> Kısa Türkçe özet: Önce `preview` ile disposable (geçici) klasörlerde test
> edin, `python verify_tomls.py` + `cargo fmt/clippy/test` çalıştırın,
> `CHANGELOG.md` güncelleyin ve PR şablonundaki güvenlik kutularını
> işaretleyin. Güvenlik açığını herkese açık issue olarak **açmayın** —
> [SECURITY.md](SECURITY.md) adresine bakın.

## Quick start

```sh
git clone https://github.com/sweep-cleaner/sweep
cd sweep

# Static check for the 45 embedded cleaner definitions (no Rust needed):
python3 verify_tomls.py

# Full Rust checks:
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
```

Prerequisites: Rust 1.88+ (see `rust-version` in `Cargo.toml`;
`rust-toolchain.toml` pins the development toolchain), Python 3.11+ (for
`tomllib` used by `verify_tomls.py`). Platform helpers (package managers,
`fc-cache`, journal tools, …) are only needed when testing the providers that
use them.

## Project layout

```text
.
├── cleaners/       45 built-in native TOML cleaner definitions
├── src/
│   ├── action/     Concrete action providers and dispatch
│   ├── cli/        clap command-line interface
│   ├── config/     Persisted TOML configuration
│   ├── core/       Errors, paths, reports, and safety guards
│   ├── definition/ Native TOML, CleanerML, and winapp2 loaders
│   ├── engine/     Selection, workers, progress, and deep scan
│   ├── fsutil/     Traversal, sizes, deletion, and free-space helpers
│   ├── platform/   Windows, Linux, macOS, and Unix implementations
│   └── shred/      Overwrite and secure-deletion primitives
├── docs/           User and cleaner-authoring documentation
└── verify_tomls.py Static validation for embedded cleaner definitions
```

The library crate is named `sweep`; the binaries are `sweep` (CLI) and
`sweep-gui` (optional, `--features gui`).

## Recommended workflow

1. Read the relevant source path before changing a cleaner or provider.
2. For a cleaner change, create a narrowly scoped disposable fixture tree in a
   temporary location. Do not point a new definition at a real home directory
   until its preview output has been inspected.
3. Add or update the native TOML definition under `cleaners/`.
4. Run the static definition validator from the project root:

   ```sh
   python3 verify_tomls.py
   ```

   This parses the 45 embedded TOMLs and checks their schema tokens, provider
   names, and dispatcher reachability. It does not compile Rust.
5. Review the diff and all path expansions, filters, OS conditions, and warning
   text.
6. Use preview mode against disposable fixtures and inspect every reported
   entry before attempting a narrowly selected clean.
7. Exercise failure cases deliberately: missing paths, permissions, running
   applications, links/junctions, malformed definitions, and interrupted
   operations.
8. Update the relevant documentation whenever CLI flags, providers, config
   fields, cleaner schema, platform behavior, or provenance changes.
9. Update [CHANGELOG.md](CHANGELOG.md) under `[Unreleased]`.

Never use real personal data as a test fixture. Keep backups of any data that
could be reached by a new action.

## Cleaner authoring rules

A native definition should:

- use a stable, lowercase `id` and descriptive `name`;
- give every option a stable `id`, readable `label`, and useful description;
- specify `os` filters whenever an option, variable, path, or action is
  platform-specific;
- specify `search` explicitly, even though the parser can infer `glob` for a
  wildcard path and `file` otherwise;
- use the narrowest possible path and avoid broad roots such as a whole home
  directory or system volume;
- use `type = "f"` or `type = "d"` where matching only files or directories is
  intended;
- use age, size, depth, and regular-expression filters to reduce accidental
  matches;
- add an option `warning` when cleaning history, cookies, sessions, logs, or
  other data users may want to retain; and
- use a command present in `src/action/provider.rs` and reachable through
  `src/action/mod.rs`.

Variables use `$$name$$` placeholders and can expand to several paths. Keep
variable globs specific. A variable that expands to no paths should be treated
as a normal no-op, not as a reason to broaden the path.

See [docs/cleaner-format.md](docs/cleaner-format.md) for the complete field
reference and provider groups.

## Safety and security rules

Do not weaken a safety boundary simply to make a cleaner pass a fixture. In
particular, preserve:

- `Guard` checks, built-in protected roots, and the user keep list;
- link, junction, reparse-point, mount-point, and traversal checks;
- dry-run/preview behavior and report generation;
- running-application checks unless a deliberate, documented override is
  being designed;
- the external-definition trust filter;
- XML internal-subset DTD rejection; and
- restrictions on external actions.

Treat the following as privileged or high-impact even when they are expressed
as a single cleaner action:

- `process` and `--trust-external`;
- `winreg` and registry paths;
- package-manager commands such as `apt.*`, `dnf.clean`, `yum.clean`,
  `pacman.clean`, `zypper.clean`, and `brew.cleanup`;
- `system.free_disk_space`, `system.trim`, and `sweep wipe`;
- secure overwrite and shredding; and
- system cache, journal, DNS, clipboard, and recycle-bin operations.

Definitions loaded from user-writable locations are untrusted by default.
Untrusted definitions lose `process` and `winreg` actions. Parsing successfully
is not evidence that a definition is safe; review its provenance, commands,
paths, filters, and platform conditions.

Secure overwrite is best effort. It is not a guarantee for SSD/NVMe media,
copy-on-write or journaled filesystems, snapshots, backups, filesystem
replication, or device remapping.

Never report a security vulnerability as a public issue — see
[SECURITY.md](SECURITY.md).

## Importers and provenance

Three definition front-ends feed the same internal cleaner model:

- native TOML, used by the built-in definitions and user-authored drop-ins;
- BleachBit CleanerML XML, enabled explicitly with `--bleachbit`; and
- `winapp2.ini`, enabled through the dedicated `--winapp2` loading path or a
  definition directory.

Preserve upstream notices when adapting external CleanerML or winapp2 content.
Review changes from upstream sources before importing them, and do not infer
that a path is safe merely because it appears in an upstream definition.

See [THIRD_PARTY.md](THIRD_PARTY.md) for licensing and provenance boundaries.

## Pull requests

- Keep PRs narrowly scoped; one cleaner or one behavior per PR.
- Fill in the PR template, including the safety checklist for any change that
  touches deletion paths.
- Keep `verify_tomls.py` green and update `CHANGELOG.md`.
- Be ready to show `preview` output against a disposable fixture for
  cleaner changes.

## Validation checklist

Before submitting a change, review:

- [ ] `python3 verify_tomls.py` succeeds for cleaner-definition changes.
- [ ] `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`,
      and `cargo test --locked` succeed for Rust changes.
- [ ] Every documented CLI command and global flag matches `src/cli/mod.rs`.
- [ ] Every documented provider name matches `src/action/provider.rs` and its
      dispatch route.
- [ ] TOML field names and accepted tokens match `src/definition/native.rs`
      and `src/definition/model.rs`.
- [ ] Markdown links, headings, tables, and fenced code blocks are readable.
- [ ] Destructive examples are explicit and narrowly scoped.
- [ ] Disposable fixtures, not real personal data, were used for manual review.
- [ ] `git diff --check` has been inspected when working in a Git checkout.
- [ ] Platform-specific behavior and permission requirements are documented.
- [ ] Upstream notices and provenance are preserved for imported definitions.
- [ ] `CHANGELOG.md` has an entry under `[Unreleased]`.

## License

By contributing, you agree that your contributions are licensed under the
project's [GPL-3.0-or-later](COPYING) license. Preserve existing copyright and
license notices when adapting external content.
