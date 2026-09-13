# Native cleaner format

This document describes Sweep's native TOML cleaner format. It is the
authoritative authoring guide for files loaded by the native parser in
`src/definition/native.rs`.

## One normalized model

Sweep accepts three cleaner-definition front-ends:

1. native TOML, used by built-in definitions and native drop-ins;
2. BleachBit CleanerML XML, loaded explicitly with `--bleachbit`; and
3. `winapp2.ini`, loaded through the dedicated `--winapp2` path or a definition
   directory.

All front-ends are normalized into the same model:

- `CleanerDef`: identity, description, OS filter, running checks, variables,
  and options;
- `OptionDef`: one selectable operation with a label, description, warning,
  OS filter, and actions; and
- `ActionDef`: a provider command, path/search strategy, filters, and
  command-specific parameters.

Users select normalized options with selectors such as
`firefox.history`. The native TOML form is preferred for new Sweep
cleaners because it is typed, compact, comment-friendly, and avoids XML entity
escaping for Windows paths.

## Minimal example

The following example is intentionally scoped to a disposable fixture. Do not
copy it unchanged into a cleaner intended for real personal data.

```toml
id = "demo_fixture"
name = "Demo fixture"
description = "Removes old log files from a disposable test directory"
os = "unix"

[[running]]
type = "path"
value = "/tmp/demo-fixture/app.lock"
os = "unix"

[[var]]
name = "fixture"

[[var.value]]
value = "/tmp/demo-fixture"
search = "literal"
os = "unix"

[[option]]
id = "old_logs"
label = "Old logs"
description = "Remove old log files from the disposable fixture"
warning = "Only use this option with a disposable test directory."
os = "unix"

[[option.action]]
command = "delete"
search = "glob"
path = "$$fixture$$/*.log"
os = "unix"
max_age_days = 7
type = "f"
```

An action may use a literal path, a glob, a recursive walk, or a deep-scan
rule. Keep paths narrow and specify `search` explicitly even though the parser
can infer `glob` for a wildcard path and `file` otherwise.

## Cleaner fields

### Top-level cleaner

```toml
id = "stable_identifier"
name = "Display name"
description = "What the cleaner removes"
os = "windows" # optional; empty means all platforms
```

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | string, required | Stable selector and registry identifier. |
| `name` | string, required | Human-readable cleaner name. |
| `description` | string, optional | Longer explanation shown by listing/diagnostics. |
| `os` | string, optional | `windows`, `linux`, `macos`, `unix`, `bsd`, or empty for any platform. |

Use stable IDs. Renaming an ID changes selectors and can break user scripts or
saved selections.

### Running checks

Add one or more `[[running]]` tables when an option should be skipped while an
application or lock path is active:

```toml
[[running]]
type = "exe"
value = "firefox"
os = "unix"
same_user = true

[[running]]
type = "path"
value = "C:\\ProgramData\\Example\\app.lock"
os = "windows"
```

| Field | Type | Meaning |
| --- | --- | --- |
| `type` | string, required | `exe`, `path`, or `pathname`. |
| `value` | string, required | Executable name or path pattern. |
| `os` | string, optional | Platform filter. |
| `same_user` | boolean, optional | For executable checks, limit the match to the current user. |

Close target applications before cleaning. `--force` can bypass the running-
application refusal and should be treated as an expert-only override.

### Variables and path expansion

Variables are declared with `[[var]]` and one or more `[[var.value]]` tables:

```toml
[[var]]
name = "profile"

[[var.value]]
os = "unix"
search = "glob"
value = "~/.config/example/*"

[[var.value]]
os = "windows"
search = "glob"
value = '$LOCALAPPDATA\\Example\\Profiles\\*'
```

Supported variable-search values are:

- `literal` or an empty value: use the value as a literal path;
- `glob`: expand matching filesystem paths; and
- `winreg` or `registry`: read a Windows registry-derived value where the
  platform implementation supports it.

Use a variable in an action as `$$profile$$/Cache`. Each variable can expand
to multiple paths, and variable definitions are filtered by their `os` field
before expansion. For registry-derived values, `name` identifies the registry
value when required by the definition.

Avoid variables that can expand to an entire home directory, system volume, or
other broad root. A definition should be specific about both the base path and
the child it intends to clean.

### Options

Each selectable operation is an `[[option]]` table:

```toml
[[option]]
id = "cache"
label = "Cache"
description = "Delete the application's disposable cache"
warning = "The next start may be slower."
os = "unix"
```

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | string, required | Stable option selector suffix. |
| `label` | string, required | Short user-facing label. |
| `description` | string, optional | Explanation of the operation. |
| `warning` | string, optional | Extra caution shown for privacy/destructive effects. |
| `os` | string, optional | Platform filter. |
| `[[option.action]]` | table, repeatable | One or more actions executed for the option. |

One option can combine several actions, for example deleting a cache directory
and vacuuming a related SQLite database.

## Action fields

The common action shape is:

```toml
[[option.action]]
command = "delete"
search = "walk.all"
path = "$$profile$$/Cache"
os = "unix"
type = "d"
max_depth = 4
```

### Search strategies

`search` accepts:

| Value | Meaning |
| --- | --- |
| `file` | Treat `path` as one file/path target. |
| `glob` | Expand wildcard matches. |
| `walk.all` | Recursively visit files and directories. |
| `walk.files` | Recursively visit files only. |
| `walk.top` | Visit the immediate children of a directory. |
| `deep` | Defer matching to the deep-scan engine. |

If `search` is omitted, the parser uses `glob` when `path` contains a
wildcard and `file` otherwise. This inference is convenient but explicit
`search` values are easier to review.

### Matching and safety filters

These optional fields narrow the candidates selected by an action:

| Field | Meaning |
| --- | --- |
| `regex` | Include names matching a regular expression. |
| `nregex` | Exclude names matching a regular expression. |
| `wholeregex` | Include full paths matching a regular expression. |
| `nwholeregex` | Exclude full paths matching a regular expression. |
| `type` | `f`/`file` for files or `d`/`dir`/`directory` for directories. |
| `max_age_days` | Ignore newer entries; select entries at least this old. |
| `min_age_days` | Ignore older entries; select entries no older than this. |
| `min_size` | Minimum size in bytes or a size string such as `1MB`. |
| `max_size` | Maximum size in bytes or a size string such as `100MB`. |
| `max_depth` | Maximum traversal depth. |
| `include_dirs` | Whether directory entries are included where applicable. |

Size values must be non-negative integers or strings accepted by the size
parser, such as `1KB`, `10MB`, or `2GB`. Use filters as a defense in depth;
they do not replace a narrow base path.

### OS filters

`os` may be placed at the cleaner, running-check, variable value, option, or
action level. The accepted platform labels are `windows`, `linux`, `macos`,
`unix`, and `bsd`. An empty value means no platform restriction. The most
specific applicable filter should be used where a path or operation differs
by operating system.

## Action providers

Provider names are defined authoritatively in
`src/action/provider.rs`; `sweep providers` shows the entries available on the
current platform.

### Filesystem

| Provider | Behavior |
| --- | --- |
| `delete` | Delete files or directories selected by the path/search/filter fields. |
| `shred` | Overwrite and then delete, independent of the global shred setting. |
| `truncate` | Empty a file while keeping the file entry. |

### Structured files

| Provider | Behavior |
| --- | --- |
| `ini` | Remove an INI section or parameter using command-specific fields. |
| `json` | Remove a JSON key using command-specific fields. |

### SQLite and browser data

| Provider group | Behavior |
| --- | --- |
| `sqlite.vacuum` | Vacuum a SQLite database and reclaim free pages where possible. |
| `chrome.*` | Chromium history, keywords, favicons, database tracker, and cookies. |
| `mozilla.*` | Firefox/Mozilla history, URL history, database cleanup, and profile vacuum. |
| `cookies` | Cookie cleanup for supported browser database schemas. |

These actions edit databases rather than simply unlinking a path. Close the
browser first, retain backups, and expect database schema/version differences
to produce warnings or partial results.

### System and package-manager providers

The `system.*` group includes clipboard, DNS, memory, trash, rotated logs,
localizations, temporary files, free-space operations, journald, font caches,
recycle bin, Prefetch, WER, Windows Update, Delivery Optimization, Explorer
caches, thumbnail databases, Microsoft Store cache, DirectX shader cache,
crash dumps, memory dumps, and TRIM-related operations. Exact availability is
platform-filtered.

Package-manager providers are:

```text
apt.clean
apt.autoclean
apt.autoremove
dnf.clean
yum.clean
pacman.clean
zypper.clean
brew.cleanup
```

They require the corresponding executable and may require administrative
privileges. Their effects are broader than deleting an application cache.

### Privileged providers

| Provider | Trust requirement | Behavior |
| --- | --- | --- |
| `process` | Trusted definition | Run an external command. |
| `winreg` | Trusted definition and Windows | Delete a registry key or value. |

External definitions are untrusted by default, so these actions are removed.
`--trust-external` enables them for a run; use it only after reviewing the
source and command/path arguments.

## Import and validation

`sweep import PATH` validates a native `.toml`, a CleanerML `.xml`, a
`winapp2.ini`, or a whole definition directory without running any of their
actions. Pass a directory to load every definition inside it, or a single file
to validate just that one.

Nothing is executed: the file is parsed, normalized into the internal model and
run through `CleanerDef::validate()`, so unknown action commands, duplicate
option ids and missing labels are reported as warnings. `winapp2.ini` sections
that Sweep cannot translate at all simply yield no rules, which is reported as
`no recognizable rules found` rather than an error.

The importers normalize external definitions into the same model and collect
registry warnings for unknown commands or unusable options. CleanerML parsing
rejects internal-subset DTDs, and untrusted definitions lose `process` and
`winreg` actions. These checks reduce risk but do not establish that a
third-party definition is safe.

For built-in native TOMLs, run the static checker from the project directory:

```sh
python verify_tomls.py
```

## Troubleshooting

### Unknown provider

Check spelling and compare the command with `src/action/provider.rs`. Run
`sweep providers` to see current platform availability. A command can be
registered but unavailable on the current operating system.

### Malformed TOML or missing fields

Use `sweep import path/to/cleaner.toml`. Check the required `id` and `name`,
then verify the table spelling: `[[running]]`, `[[var]]`, `[[var.value]]`,
`[[option]]`, and `[[option.action]]`.

### Option is not listed

The cleaner, option, or action may be filtered by `os`; the provider may be
unavailable; or all actions may have been removed by the untrusted-definition
trust filter. Inspect the import warnings and run `sweep diagnostics`.

### Cleaner is skipped

Close the target application or inspect its running check. Do not use `--force`
unless you understand the risk of cleaning files owned by a running process.

### No paths are found

Inspect variable expansion, environment variables, OS filters, glob spelling,
and path separators. Use a disposable fixture and preview the exact selector.
Do not broaden a path as a first troubleshooting step.

### Operation fails part-way through

Treat per-entry errors as useful report data. Check permissions, locks,
read-only attributes, mount points, and external helper availability. Re-run a
narrow preview after the filesystem state has changed; do not assume a preview
is still current.
