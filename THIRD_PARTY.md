# Licensing and third-party provenance

This file records this project's licensing and provenance
boundaries. It is project documentation, not legal advice. Distributors and
packagers should perform their own license, copyright, attribution, and notice
review using the exact source and resolved dependency metadata they ship.

## Sweep license

The `sweep` package in `Cargo.toml` is licensed under the GNU General Public
License, version 3 or any later version:

**GPL-3.0-or-later**

The repository license text is available at [COPYING](COPYING).

The Rust implementation is inspired by BleachBit's functionality and
cleaner-definition model. It is an independent implementation, not the
official BleachBit application, and this project does not imply endorsement,
affiliation, or drop-in compatibility.

## BleachBit-inspired concepts and formats

Sweep draws on the publicly visible concepts of cleaner IDs, options,
declarative actions, path search modes, preview/report workflows, platform
filters, and safety guards.

Keep these categories distinct when distributing or extending the project:

1. **Rust implementation:** the code under `src/`, written for this
   project and covered by the package's declared license unless a file says
   otherwise.
2. **Native TOML definitions:** the 45 files under `cleaners/`,
   embedded into the binary by the Rust source. Review their file-level notices
   and history before copying them to another project.
3. **BleachBit CleanerML XML:** an optional external format loaded by the
   importer. Imported definitions retain their upstream provenance and should
   not be silently represented as original Sweep content.
4. **winapp2.ini:** an optional external Windows-oriented definition source.
   Preserve upstream notices and review the provenance of any sections copied
   into a distribution.

The original repository documentation links to these related resources:

- [BleachBit CleanerML repository](https://github.com/bleachbit/cleanerml)
- [BleachBit miscellaneous repository](https://github.com/bleachbit/bleachbit-misc)
- [winapp2.ini repository](https://github.com/bleachbit/winapp2.ini)

Those links identify upstream resources; they are not a substitute for
checking the license and notices of the exact definitions or files imported.

When adapting external definitions:

- retain copyright and license notices;
- record the upstream source and revision where practical;
- review path breadth, filters, OS conditions, and privileged providers;
- inspect upstream changes before refreshing imported data; and
- do not treat a definition's availability as evidence that its target path is
  safe on a particular machine.

## Direct Rust dependencies

`Cargo.toml` declares the following direct dependency groups. This is a
functional inventory, not a generated license report:

| Group | Crates |
| --- | --- |
| Error handling | `thiserror` |
| Logging | `log`, `env_logger` |
| Command line | `clap` |
| Serialization and configuration | `serde`, `serde_json`, `toml` |
| CleanerML parsing | `quick-xml` |
| Matching, traversal, and command parsing | `regex`, `globset`, `walkdir`, `shell-words` |
| Concurrency and CPU information | `rayon`, `num_cpus` |
| Databases | `rusqlite` with the `bundled` feature |
| Platform and miscellaneous helpers | `dirs`, `rand`, `sysinfo`, `indicatif` |
| Unix target dependency | `libc` |
| Windows target dependency | `winreg` |

The exact licenses, copyright notices, feature selections, and transitive
obligations must be verified from the resolved dependency metadata used for a
release. Do not infer a crate's license from its name, and do not describe this
short table as a complete `cargo about`-style notice inventory without checking
the lockfile and generated metadata.

When packaging, produce the appropriate third-party notices for the exact
version graph and retain the license texts required by those dependencies.

## Data and definition provenance

A cleaner definition is executable policy: it identifies paths and can select
operations such as deletion, database editing, package-manager commands,
external processes, or registry changes. Provenance review must therefore
cover both copyright/licensing and operational safety.

For external definitions, check:

- who published the definition and where it was obtained;
- whether it was modified after publication;
- whether its paths are still correct for the intended application version;
- whether it uses `process`, `winreg`, package-manager, or free-space actions;
- whether it contains broad globs or system roots; and
- whether its copyright and license notices are preserved.

Sweep loads external definitions as untrusted by default and removes
`process` and `winreg` actions. The `--trust-external` option is an explicit
security boundary, not a provenance or legal approval.

## Reporting a provenance change

When adding or replacing imported content, update the relevant documentation
with the upstream source and preserve notices in the distributed tree. When a
new dependency is added, update the dependency inventory and generate the
release's exact license/notice report from resolved metadata. When a native
cleaner is independently authored, document that it is native Sweep
content rather than implying that it came from BleachBit.
