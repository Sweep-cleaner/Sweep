#!/usr/bin/env python3
"""Schema + semantic validation for embedded Sweep cleaners/*.toml.

This does NOT compile Rust. It uses tomllib to parse each TOML and mirrors the
exact rules that src/definition/native.rs::parse_native and
src/definition/model.rs::CleanerDef::validate apply at load time, so we can
predict whether `load_builtin` (which asserts warnings().is_empty()) will pass
for every embedded definition.

Mirrors:
  * NativeDef fields (id/name required; os: String; [[running]] type/value;
    [[var]] + [[var.value]]; [[option]] id/label + [[option.action]]).
  * parse_native semantic checks: running.type in {exe,path,pathname};
    var.value.search in {literal,glob,winreg,registry,""}; option.action.type
    in {f,file,d,dir,directory,""}; option.action.search via SearchKind::parse.
  * ByteSize parsing (int or "<n>KB/MB/GB/TB").
  * validate(): every action.command must exist in the provider registry.
  * rooted paths: every non-empty, filesystem action path must be absolute
    (start with ~, /, %, $, *, a Windows drive such as C:\\, or a backslash),
    mirroring the runtime guard in definition/model.rs::expand_action_paths.
  * registry coverage: every cleaners/*.toml is referenced by exactly one
    include_str! in src/definition/builtin.rs, and no registry entry is
    duplicated or dangling. An unregistered file is compiled away and would be
    silently unreachable at runtime.
"""
import argparse
import glob
import os
import re
import sys
import tomllib

ROOT = os.path.dirname(os.path.abspath(__file__))

parser = argparse.ArgumentParser(description="Validate Sweep native cleaner TOMLs")
parser.add_argument(
    "--dir",
    action="append",
    default=[],
    metavar="DIR",
    help="extra directory of *.toml cleaners to validate (repeatable); "
    "default scans the embedded cleaners/ directory only",
)
ARGS = parser.parse_args()

# Authoritative provider command names (src/action/provider.rs PROVIDERS).
KNOWN_COMMANDS = {
    "delete", "shred", "truncate", "ini", "json", "sqlite.vacuum",
    "chrome.history", "chrome.keywords", "chrome.autofill", "chrome.favicons", "chrome.databases_db",
    "chrome.cookies", "mozilla.history", "mozilla.url_history", "mozilla.databases",
    "mozilla.vacuum", "cookies", "winreg", "process",
    "system.clipboard", "system.dns", "system.memory", "system.trash",
    "system.rotated_logs", "system.localizations", "system.recent_documents",
    "system.custom", "system.tmp", "system.free_disk_space", "system.journald",
    "system.font_cache", "system.recycle_bin", "system.prefetch", "system.wer",
    "system.windows_update_cache", "system.delivery_optimization",
    "system.explorer_caches", "system.trim",
    "system.thumbcache", "system.store_cache", "system.d3d_shader_cache",
    "system.crash_dumps", "system.memory_dumps",
    "apt.clean", "apt.autoclean", "apt.autoremove",
    "dnf.clean", "yum.clean", "pacman.clean", "zypper.clean", "brew.cleanup",
    "system.apt_cache", "system.dnf_cache", "system.pacman_cache",
    "system.old_kernels", "system.journal_vacuum", "system.user_caches",
    "system.temp_deep", "system.orphans", "system.snap", "system.flatpak",
    "system.nix_gc", "system.docker_volumes", "system.journal_user",
    "system.tm_thin",
    "system.docker", "system.kde_cache", "system.kde_pim", "system.baloo",
    "system.gnome_cache", "system.zeitgeist", "system.tracker",
    "system.apt_lists", "system.zypp_cache", "system.crash_reports",
    "system.coredumps", "system.dev_npm", "system.dev_pip",
    "system.dev_cargo", "system.dev_gradle", "system.dev_go",
}

# Mirror of action/mod.rs::run dispatch routing.
def dispatchable(cmd: str) -> bool:
    if cmd in ("delete", "shred", "truncate", "ini", "json"):
        return True
    if cmd.startswith("sqlite."):
        return True
    if cmd.startswith("chrome.") or cmd.startswith("mozilla.") or cmd == "cookies":
        return True
    if cmd.startswith("system."):
        return True
    if cmd in ("process", "winreg"):
        return True
    if cmd in (
        "apt.clean", "apt.autoclean", "apt.autoremove", "dnf.clean", "yum.clean",
        "pacman.clean", "zypper.clean", "brew.cleanup",
    ):
        return True
    return False

VALID_RUNNING_TYPES = {"exe", "path", "pathname"}
VALID_VAR_SEARCH = {"", "literal", "glob", "winreg", "registry"}
VALID_OBJ_TYPE = {"", "f", "file", "d", "dir", "directory"}
VALID_SEARCH = {"", "file", "glob", "walk.all", "walk.files", "walk.top", "deep"}

# Providers whose `path` is not a filesystem location (registry key, command
# line). They are exempt from the rooted-path rule below.
NON_FS_COMMANDS = {"winreg", "process"}


def path_is_rooted(path: str) -> bool:
    """Mirror of core::path::looks_absolute plus the runtime `$$var$$` form.

    A relative path would be resolved against the process working directory, so
    the same cleaner would delete different things depending on where `sweep`
    started. Every filesystem action path must therefore be one of:
      * an absolute POSIX path (`/...`) or Windows drive (`C:\\...`, `C:/...`,
        i.e. `X:\\`);
      * a home (`~`) or environment (`$VAR`, `%VAR%`) reference;
      * a `$$var$$` placeholder (resolved later by the VarSet);
      * a glob / wildcard (`*`).
    """
    if not path:
        return True
    if path[0] in ('~', '/', '\\', '$', '%', '*'):
        return True
    return len(path) >= 3 and path[1] == ':' and path[2] in ('\\', '/')

errors = []


def err(path, msg):
    errors.append(f"[{path}] {msg}")


def human_to_bytes(text):
    """Approximate src/fsutil/size.rs::human_to_bytes."""
    text = text.strip()
    if not text:
        return None
    mult = {"b": 1, "kb": 1024, "mb": 1024**2, "gb": 1024**3, "tb": 1024**4}
    low = text.lower()
    for suf, m in mult.items():
        if low.endswith(suf):
            num = low[: -len(suf)].strip()
            return int(float(num) * m)
    # bare integer = bytes
    return int(float(text))


def check_size(name, where, raw):
    if raw is None:
        return
    if isinstance(raw, int):
        if raw < 0:
            err(name, f"{where}: negative size {raw}")
        return
    try:
        human_to_bytes(str(raw).strip())
    except Exception:
        err(name, f"{where}: cannot parse size string '{raw}'")


# Collect the TOML files to validate. By default we scan the embedded
# `cleaners/` directory; `--dir` adds extra directories (e.g. the community hub
# rules in ../hub) which share the same native schema.
def collect_files():
    dirs = [os.path.join(ROOT, "cleaners")]
    for d in ARGS.dir:
        dirs.append(d)
    out = []
    for d in dirs:
        out.extend(sorted(glob.glob(os.path.join(d, "*.toml"))))
    return out


files = collect_files()

for toml_path in files:
    name = os.path.basename(toml_path)
    # The community hub ships an `index.toml` whose schema ([[cleaner]]) is not a
    # Sweep cleaner; it is validated by the hub installer itself, not here.
    if name == "index.toml":
        continue
    try:
        with open(toml_path, "rb") as fh:
            data = tomllib.load(fh)
    except Exception as e:
        err(name, f"TOML syntax error: {e}")
        continue

    cid = data.get("id")
    if not isinstance(cid, str) or not cid:
        err(name, "missing or non-string top-level `id`")
    cname = data.get("name")
    if not isinstance(cname, str) or not cname:
        err(name, "missing or non-string top-level `name`")

    # running
    for i, r in enumerate(data.get("running", []) or []):
        if not isinstance(r, dict):
            err(name, f"running[{i}] not a table")
            continue
        rt = r.get("type")
        if rt not in VALID_RUNNING_TYPES:
            err(name, f"running[{i}] type '{rt}' not in {sorted(VALID_RUNNING_TYPES)}")
        if "value" not in r:
            err(name, f"running[{i}] missing `value`")

    # vars -- NOTE: the Rust schema key is `var` (singular), see
    # src/definition/native.rs::NativeDef. Checking "vars" silently skipped
    # every [[var]] block in all 45 cleaners.
    if "vars" in data:
        err(name, "unknown top-level key `vars` (the schema key is `var`)")
    for i, v in enumerate(data.get("var", []) or []):
        if not isinstance(v, dict) or "name" not in v:
            err(name, f"vars[{i}] missing `name`")
            continue
        for j, val in enumerate(v.get("value", []) or []):
            vs = (val.get("search") if isinstance(val, dict) else None) or ""
            if vs not in VALID_VAR_SEARCH:
                err(name, f"vars[{i}].value[{j}] search '{vs}' not in {sorted(VALID_VAR_SEARCH)}")

    # options
    options = data.get("option")
    if not isinstance(options, list) or not options:
        err(name, "missing or non-list `option`")
    else:
        for i, opt in enumerate(options):
            if not isinstance(opt, dict):
                err(name, f"option[{i}] not a table")
                continue
            oid = opt.get("id")
            if not isinstance(oid, str) or not oid:
                err(name, f"option[{i}] missing `id`")
            olabel = opt.get("label")
            if not isinstance(olabel, str) or not olabel:
                err(name, f"option[{i}] missing `label`")
            acts = opt.get("action")
            if not isinstance(acts, list) or not acts:
                err(name, f"option[{i}] missing/empty `action`")
                continue
            for j, a in enumerate(acts):
                if not isinstance(a, dict):
                    err(name, f"option[{i}].action[{j}] not a table")
                    continue
                cmd = a.get("command")
                if not isinstance(cmd, str) or not cmd:
                    err(name, f"option[{i}].action[{j}] missing `command`")
                elif cmd not in KNOWN_COMMANDS:
                    err(name, f"option[{i}].action[{j}] command '{cmd}' NOT in provider registry")
                elif not dispatchable(cmd):
                    err(name, f"option[{i}].action[{j}] command '{cmd}' is in registry but NOT routed by action/mod.rs::run")

                s = a.get("search", "")
                if s not in VALID_SEARCH:
                    err(name, f"option[{i}].action[{j}] search '{s}' not a valid SearchKind")
                ot = a.get("type", "")
                if ot not in VALID_OBJ_TYPE:
                    err(name, f"option[{i}].action[{j}] type '{ot}' not a valid ObjectType")

                # Filesystem paths must be absolute; a relative one would be
                # resolved against the process CWD (see model.rs::expand_action_paths).
                p = a.get("path")
                if (
                    cmd not in NON_FS_COMMANDS
                    and isinstance(p, str)
                    and p.strip()
                    and not path_is_rooted(p.strip())
                ):
                    err(
                        name,
                        f"option[{i}].action[{j}] path '{p}' is not rooted "
                        "(cleaner paths must be absolute: start with ~ / % $ * or a drive letter)",
                    )
                check_size(name, f"option[{i}].action[{j}].min_size", a.get("min_size"))
                check_size(name, f"option[{i}].action[{j}].max_size", a.get("max_size"))

# Global: every registered provider command must be reachable by the dispatcher,
# otherwise a user-written cleaner using it would silently do nothing.
for cmd in sorted(KNOWN_COMMANDS):
    if not dispatchable(cmd):
        err("<registry>", f"provider command '{cmd}' has no dispatch route in action/mod.rs::run")

# Global: the embedded set must equal the files on disk. A cleaner that exists in
# `cleaners/` but is missing from `BUILTIN_CLEANERS` is compiled away entirely, so
# `sweep <id>` fails with "unknown cleaner" while the file still looks valid here
# (this actually happened: 109 files, only 103 registered). The reverse — a
# registry entry pointing at a deleted file — breaks the build, so it is reported
# too. `include_str!` is the single source of truth for what is embedded.
BUILTIN_RS = os.path.join(ROOT, "src", "definition", "builtin.rs")
if os.path.exists(BUILTIN_RS):
    with open(BUILTIN_RS, encoding="utf-8") as fh:
        builtin_text = fh.read()
    registered = re.findall(
        r'include_str!\("\.\./\.\./cleaners/([A-Za-z0-9_.-]+\.toml)"\)', builtin_text
    )
    counts = {}
    for name in registered:
        counts[name] = counts.get(name, 0) + 1
    for name, n in sorted(counts.items()):
        if n > 1:
            err("builtin.rs", f"cleaner '{name}' is registered {n} times")

    on_disk = {
        os.path.basename(p)
        for p in glob.glob(os.path.join(ROOT, "cleaners", "*.toml"))
    }
    for name in sorted(on_disk - set(registered)):
        err(
            "builtin.rs",
            f"cleaners/{name} is NOT in BUILTIN_CLEANERS — it would be "
            "unreachable at runtime (add an include_str! entry)",
        )
    for name in sorted(set(registered) - on_disk):
        err("builtin.rs", f"BUILTIN_CLEANERS references missing cleaners/{name}")
else:
    err("builtin.rs", f"cannot read {BUILTIN_RS} to cross-check the registry")

print("=== Sweep TOML validation (mirrors parse_native + validate) ===")
print(f"files scanned: {len(files)}")
if errors:
    # group by file for readability
    by_file = {}
    for e in errors:
        f = e.split("]")[0].strip("[")
        by_file.setdefault(f, []).append(e)
    print(f"\nERRORS ({len(errors)}):")
    for f in sorted(by_file):
        print(f"  {f}:")
        for e in by_file[f]:
            print(f"    - {e.split(']', 1)[1].strip()}")
    sys.exit(1)
else:
    print(f"\nAll {len(files)} TOMLs: valid syntax, valid NativeDef schema,")
    print("valid search/type tokens, and every action.command is a registered provider.")
    print("Every cleaners/*.toml is wired into BUILTIN_CLEANERS exactly once.")
    print("=> load_builtin's warnings().is_empty() assertion should hold.")
    sys.exit(0)
