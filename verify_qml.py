#!/usr/bin/env python3
"""Static checks for the Qt6/QML shell under packaging/qml.

Plays the same role for the GUI that verify_tomls.py plays for cleaners, and
guards the same class of bug: a source file that exists on disk but was never
wired into the build, so it is compiled away and only fails at runtime.

Checks:
  * every qml/**/*.qml is listed in CMakeLists.txt's qt_add_qml_module
    QML_FILES block, exactly once;
  * every entry in QML_FILES points at a file that exists;
  * every image/asset a .qml file loads with `source:` exists on disk and is
    listed in the module's RESOURCES block, exactly once (the same "on disk
    but never wired into the build" bug, one level down);
  * every custom component used by a .qml file (an `Identifier {` whose
    identifier matches a sibling `<Identifier>.qml`) is itself registered;
  * brackets balance in every .qml file (a crude but effective guard against a
    truncated file or a stray brace);
  * every i18n key a .qml file looks up (`i18n.tr("…")` or `I18nText { key: … }`)
    exists in main.cpp's Translator tables for *all* eight languages, and those
    tables stay symmetric. QML renders the raw key when a translation is
    missing, so a typo ships as visible UI text — and nothing else catches it.
"""
import os
import re
import sys

ROOT = os.path.dirname(os.path.abspath(__file__))
QML_DIR = os.path.join(ROOT, "packaging", "qml")
QML_SRC = os.path.join(QML_DIR, "qml")
CMAKE = os.path.join(QML_DIR, "CMakeLists.txt")
MAIN_CPP = os.path.join(QML_DIR, "main.cpp")

# The languages Translator must translate into; its tables are `const char
# *<lang>[][2]` arrays.
LANGS = ["en", "tr", "es", "ru", "fr", "de", "pt", "it"]

# Qt built-ins and enums that look like component names but are not our files.
BUILTIN_TYPES = {
    "Qt", "QtObject", "Component", "Connections", "Timer", "Repeater", "Loader",
    "Item", "Rectangle", "Text", "MouseArea", "Canvas", "Window", "Flickable",
    "Column", "Row", "Grid", "Flow", "ListView", "Image", "BorderImage",
    "Gradient", "GradientStop", "Behavior", "Translate", "Scale", "Rotation",
    "NumberAnimation", "ColorAnimation", "SequentialAnimation",
    "ParallelAnimation", "PauseAnimation", "ScriptAction", "RotationAnimation",
    "PropertyAnimation", "SmoothedAnimation", "SpringAnimation", "FontLoader",
    "ShaderEffect", "OpacityMask", "Layer", "Keys", "Shortcut",
    "SystemTrayIcon", "Menu", "MenuItem", "Dialog", "StackView", "SplitView",
    "ScrollView", "TextInput", "TextEdit", "TapHandler", "DragHandler",
    "HoverHandler", "WheelHandler", "State", "Transition", "PropertyChanges",
}
BUILTIN_PREFIXES = (
    "Easing", "Font", "Text", "Animation", "Image", "MouseArea", "Qt",
    "Screen", "StandardPaths", "Application", "Cursor", "Clipboard", "Window",
)

errors = []


def err(msg):
    errors.append(msg)


def strip_comments_and_strings(text):
    """Return the code with comments and string contents blanked out."""
    out = []
    i, n = 0, len(text)
    line_comment = False
    block_comment = False
    quote = None
    while i < n:
        c = text[i]
        nxt = text[i + 1] if i + 1 < n else ""
        if c == "\n":
            line_comment = False
            out.append(c)
            i += 1
            continue
        if line_comment:
            out.append(" ")
            i += 1
            continue
        if block_comment:
            if c == "*" and nxt == "/":
                block_comment = False
                out.append("  ")
                i += 2
                continue
            out.append(" " if c != "\n" else c)
            i += 1
            continue
        if quote:
            if c == "\\":
                out.append("  ")
                i += 2
                continue
            if c == quote:
                quote = None
            out.append(" " if c != "\n" else c)
            i += 1
            continue
        if c == "/" and nxt == "/":
            line_comment = True
            out.append("  ")
            i += 2
            continue
        if c == "/" and nxt == "*":
            block_comment = True
            out.append("  ")
            i += 2
            continue
        if c in ('"', "'", "`"):
            quote = c
            out.append(" ")
            i += 1
            continue
        out.append(c)
        i += 1
    return "".join(out)


def check_balance(name, code):
    pairs = {")": "(", "]": "[", "}": "{"}
    stack = []
    line = 1
    for c in code:
        if c == "\n":
            line += 1
        elif c in "([{":
            stack.append((c, line))
        elif c in ")]}":
            if not stack or stack[-1][0] != pairs[c]:
                err(f"{name}: unbalanced '{c}' on line {line}")
                return
            stack.pop()
    if stack:
        ch, ln = stack[-1]
        err(f"{name}: unclosed '{ch}' opened on line {ln}")


def rel(path):
    """Path relative to qml/, always with forward slashes."""
    return os.path.relpath(path, QML_SRC).replace(os.sep, "/")


def collect_qml_files():
    out = []
    for base, dirs, files in os.walk(QML_SRC):
        dirs[:] = [d for d in dirs if d != "build"]
        for f in files:
            if f.endswith(".qml"):
                out.append(rel(os.path.join(base, f)))
    return sorted(out)


def parse_cmake_registrations():
    if not os.path.exists(CMAKE):
        err(f"cannot read {CMAKE}")
        return []
    with open(CMAKE, encoding="utf-8") as fh:
        text = fh.read()
    match = re.search(r"QML_FILES(.*?)\n\s*\)", text, re.S)
    if not match:
        err("CMakeLists.txt: could not find the QML_FILES block")
        return []
    found = re.findall(r"^\s*qml/([A-Za-z0-9_./-]+\.qml)\s*$", match.group(1), re.M)
    return [f.replace("\\", "/") for f in found]


def parse_cmake_resources():
    """Entries of the module's RESOURCES block, relative to qml/."""
    if not os.path.exists(CMAKE):
        return []
    with open(CMAKE, encoding="utf-8") as fh:
        text = fh.read()
    match = re.search(r"\n\s*RESOURCES(.*?)\n\s*\)", text, re.S)
    if not match:
        return []
    found = re.findall(r"^\s*qml/([A-Za-z0-9_./ -]+)\s*$", match.group(1), re.M)
    return [f.replace("\\", "/") for f in found]


def collect_asset_sources(raw):
    """Relative `source:` paths a .qml file loads from its own directory."""
    out = []
    for value in re.findall(r"source\s*:\s*[\"']([^\"']+)[\"']", raw):
        if value.startswith(("qrc:", "http:", "https:", "file:", ":/")):
            continue
        if "{" in value or "}" in value:
            continue  # a bound expression, not a literal path
        out.append(value)
    return out


def parse_translator_tables():
    """lang -> {key: value} from main.cpp's `const char *<lang>[][2]` tables."""
    if not os.path.exists(MAIN_CPP):
        err(f"cannot read {MAIN_CPP}")
        return {}
    with open(MAIN_CPP, encoding="utf-8") as fh:
        text = fh.read()
    tables = {}
    for lang in LANGS:
        match = re.search(
            r"const char \*%s\[\]\[2\]\s*=\s*\{(.*?)\n\s*\};" % lang, text, re.S
        )
        if not match:
            err(f"main.cpp: could not find the '{lang}' translation table")
            continue
        tables[lang] = dict(
            re.findall(
                r'\{\s*"((?:[^"\\]|\\.)*)"\s*,\s*"((?:[^"\\]|\\.)*)"\s*\}',
                match.group(1),
            )
        )
    return tables


def collect_i18n_keys():
    """(key, qml-file) for every i18n lookup in the .qml tree."""
    out = []
    for name in collect_qml_files():
        with open(os.path.join(QML_SRC, name), encoding="utf-8") as fh:
            raw = fh.read()
        for key in re.findall(r'i18n\.tr\(\s*"([^"]+)"\s*\)', raw):
            out.append((key, name))
        # Scoped to the component, so an unrelated JS object literal that
        # happens to carry a `key:` field is not read as an i18n lookup.
        for key in re.findall(
            r'I18nText\s*\{[^{}]*?\bkey\s*:\s*"([^"]+)"', raw, re.S
        ):
            out.append((key, name))
    return out


def check_i18n(tables):
    """Every looked-up key exists in all 8 languages; tables stay symmetric."""
    used = collect_i18n_keys()
    if not used:
        err("no i18n lookups found in any .qml file")
        return set()
    for key, name in sorted(set(used)):
        for lang in LANGS:
            if lang in tables and key not in tables[lang]:
                err(
                    f"qml/{name} looks up '{key}' but main.cpp has no "
                    f"{lang} translation — the raw key would render in the UI"
                )
    en = tables.get("en", {})
    for lang in LANGS:
        if lang == "en" or lang not in tables:
            continue
        for key in sorted(en):
            if key not in tables[lang]:
                err(f"main.cpp: '{key}' is translated in en but missing in {lang}")
        for key in sorted(tables[lang]):
            if key not in en:
                err(f"main.cpp: '{key}' is translated in {lang} but not in en")
    return {key for key, _ in used}


def parse_sidebar_routes():
    """Route ids declared by Sidebar.qml's nav arrays."""
    path = os.path.join(QML_SRC, "Sidebar.qml")
    if not os.path.exists(path):
        err("cannot read qml/Sidebar.qml")
        return []
    with open(path, encoding="utf-8") as fh:
        text = fh.read()
    return re.findall(r'\{\s*name:\s*"([^"]+)"', text)


def parse_main_routes():
    """(loaded, titled, subtitled, fallback_route) from Main.qml."""
    path = os.path.join(QML_SRC, "Main.qml")
    if not os.path.exists(path):
        err("cannot read qml/Main.qml")
        return {}, set(), set(), None
    with open(path, encoding="utf-8") as fh:
        text = fh.read()
    loaded = dict(
        re.findall(r'activeScreen === "([^"]+)"\)\s*loader\.setSource\("([^"]+)"', text)
    )
    titled = set(re.findall(r'name === "([^"]+)"\)\s*return i18n\.tr\("t\.', text))
    subtitled = set(re.findall(r'name === "([^"]+)"\)\s*return i18n\.tr\("s\.', text))
    # The last screen is reached through the `else` arm rather than a branch.
    fallback = re.search(r'else\s+loader\.setSource\("([^"]+)"', text)
    fallback_route = (
        os.path.splitext(os.path.basename(fallback.group(1)))[0] if fallback else None
    )
    return loaded, titled, subtitled, fallback_route


def collect_navigate_targets():
    """route -> {qml files} for every navigate("X") call."""
    out = {}
    for name in collect_qml_files():
        with open(os.path.join(QML_SRC, name), encoding="utf-8") as fh:
            raw = fh.read()
        # Skip the signal declaration itself (`signal navigate(string name)`).
        for target in re.findall(r'(?<!signal )\bnavigate\(\s*"([^"]+)"\s*\)', raw):
            out.setdefault(target, set()).add(name)
    return out


def check_routes():
    """Every nav route must load, be titled, be subtitled, and be reachable.

    A route Main.qml does not handle is a dead click; a route with no title or
    subtitle silently renders the *fallback* screen's copy instead.
    """
    sidebar = parse_sidebar_routes()
    loaded, titled, subtitled, fallback = parse_main_routes()
    if not sidebar:
        err("Sidebar.qml declares no routes")
        return 0
    for route in sidebar:
        if route not in loaded and route != fallback:
            err(f"Sidebar route '{route}' has no loadScreen() branch in Main.qml")
        if route not in titled and route != fallback:
            err(f"Sidebar route '{route}' has no screenTitle branch in Main.qml")
        if route not in subtitled and route != fallback:
            err(f"Sidebar route '{route}' has no screenSubtitle branch in Main.qml")
    for route, screen in sorted(loaded.items()):
        if route not in sidebar:
            err(
                f"Main.qml loads '{screen}' for route '{route}', which no nav "
                "entry reaches"
            )
        if not os.path.exists(os.path.join(QML_SRC, screen)):
            err(f"Main.qml route '{route}' loads missing qml/{screen}")
    known = set(loaded) | set(sidebar)
    for target, files in sorted(collect_navigate_targets().items()):
        if target not in known:
            err(
                'navigate("%s") in %s targets an unknown route'
                % (target, ", ".join(sorted(files)))
            )
    return len(sidebar)


def check_bridge_wiring(on_disk):
    """`sys.<prop>` in QML must be a Q_PROPERTY on SysInfo and `sweep.<m>(`
    must be a Q_INVOKABLE on the bridge.

    QML reads an unknown context property as `undefined` without failing, so
    a typo (or a property that was never declared, like `sys.swapTotal` used
    to be) ships as a silently dead widget — and nothing else catches it.
    """
    with open(MAIN_CPP, encoding="utf-8") as fh:
        cpp = fh.read()
    props = set(re.findall(r"Q_PROPERTY\(\S+ (\w+) READ", cpp))
    invokables = set(re.findall(r"Q_INVOKABLE \S+ (\w+)\s*\(", cpp))
    hits = 0
    for name in on_disk:
        with open(os.path.join(QML_SRC, name), encoding="utf-8") as fh:
            code = strip_comments_and_strings(fh.read())
        for prop in sorted(set(re.findall(r"\bsys\.(\w+)", code))):
            hits += 1
            if prop not in props:
                err(f"qml/{name} reads `sys.{prop}` but SysInfo has no such Q_PROPERTY")
        for method in sorted(set(re.findall(r"\bsweep\.(\w+)\s*\(", code))):
            hits += 1
            if method not in invokables:
                err(f"qml/{name} calls `sweep.{method}()` but the bridge has no such Q_INVOKABLE")
    return hits


WINDOW_BUILTINS = {
    # Window/Item built-ins that `root.X = ...` may legally target.
    "width", "height", "x", "y", "z", "opacity", "visible", "enabled",
    "flags", "title", "visibility", "color",
    "minimumWidth", "minimumHeight", "maximumWidth", "maximumHeight",
}

CONTEXT_OBJECTS = {"settings", "sweep", "sys", "i18n"}


def check_root_props(on_disk):
    """`root.X = ...` must name a declared property, function, or id.

    QML aborts the whole JS function on assignment to an undeclared
    property, so one typo (or a forgotten `property bool working`) silently
    kills the feature — CleanerScreen shipped an empty list this way because
    `reload()` died before `sweep.listCleaners()` ran. Only `root.*` is
    covered (the proven shape); other receivers are out of scope.
    """
    hits = 0
    for name in on_disk:
        with open(os.path.join(QML_SRC, name), encoding="utf-8") as fh:
            raw = fh.read()
        code = strip_comments_and_strings(raw)
        declared = (
            set(re.findall(r"property\s+\S+\s+(\w+)", code))
            | set(re.findall(r"function\s+(\w+)\s*\(", code))
            | set(re.findall(r"\bid\s*:\s*(\w+)", code))
            | WINDOW_BUILTINS
            | CONTEXT_OBJECTS
        )
        for target in sorted(set(re.findall(r"\broot\.(\w+)\s*=", code))):
            hits += 1
            if target not in declared:
                err(
                    f"qml/{name} assigns `root.{target}` but declares no such "
                    "property/function/id (the handler aborts at runtime)"
                )
    return hits


def main():
    on_disk = collect_qml_files()
    registered = parse_cmake_registrations()
    resources = parse_cmake_resources()

    if not on_disk:
        err("no .qml files found under packaging/qml/qml")

    for name in registered:
        if not os.path.exists(os.path.join(QML_SRC, name)):
            err(f"CMakeLists.txt: QML_FILES references missing qml/{name}")

    counts = {}
    for name in registered:
        counts[name] = counts.get(name, 0) + 1
    for name, n in sorted(counts.items()):
        if n > 1:
            err(f"CMakeLists.txt: qml/{name} is listed {n} times")

    registered_set = set(registered)
    for name in on_disk:
        if name not in registered_set:
            err(
                f"qml/{name} is NOT in CMakeLists.txt QML_FILES — it would be "
                "compiled away and unreachable at runtime"
            )

    # ---- assets ------------------------------------------------------------
    resource_counts = {}
    for name in resources:
        resource_counts[name] = resource_counts.get(name, 0) + 1
        if not os.path.exists(os.path.join(QML_SRC, name)):
            err(f"CMakeLists.txt: RESOURCES references missing qml/{name}")
    for name, n in sorted(resource_counts.items()):
        if n > 1:
            err(f"CMakeLists.txt: qml/{name} is listed in RESOURCES {n} times")
    resource_set = set(resources)

    basenames = {os.path.basename(n)[:-4]: n for n in on_disk}

    for name in on_disk:
        with open(os.path.join(QML_SRC, name), encoding="utf-8") as fh:
            raw = fh.read()
        code = strip_comments_and_strings(raw)
        check_balance(name, code)
        for ref in sorted(set(re.findall(r"\b([A-Z][A-Za-z0-9_]*)\s*\{", code))):
            if ref in BUILTIN_TYPES or ref.startswith(BUILTIN_PREFIXES):
                continue
            target = basenames.get(ref)
            if target is None:
                continue  # a JS object literal or an unresolvable type
            if target not in registered_set:
                err(f"qml/{name} uses `{ref}` but qml/{target} is not registered")

        # Every asset a component loads must exist and be registered.
        here = os.path.dirname(name)
        for value in collect_asset_sources(raw):
            rel_path = os.path.normpath(os.path.join(here, value)).replace(os.sep, "/")
            disk_path = os.path.join(QML_SRC, rel_path)
            if not os.path.exists(disk_path):
                err(f"qml/{name} loads '{value}' but qml/{rel_path} does not exist")
            elif rel_path not in resource_set:
                err(
                    f"qml/{name} loads '{value}' but qml/{rel_path} is NOT in the "
                    "CMakeLists.txt RESOURCES block — it would not ship"
                )

    i18n_keys = check_i18n(parse_translator_tables())
    routes = check_routes()
    bridge_refs = check_bridge_wiring(on_disk)
    root_writes = check_root_props(on_disk)

    print("=== Sweep QML validation (registration + routes + i18n) ===")
    print(f"qml files on disk  : {len(on_disk)}")
    print(f"registered in CMake: {len(registered)}")
    print(f"assets registered  : {len(resources)}")
    print(f"nav routes wired   : {routes}")
    print(f"bridge refs checked: {bridge_refs}")
    print(f"root props checked : {root_writes}")
    print(f"i18n keys covered  : {len(i18n_keys)} across {len(LANGS)} languages")
    if errors:
        print(f"\nERRORS ({len(errors)}):")
        for e in errors:
            print(f"  - {e}")
        return 1
    print("\nEvery .qml file is registered exactly once, every custom component")
    print("resolves to a registered file, every loaded asset is shipped, every nav")
    print("route loads and is titled, brackets balance, and every i18n key is")
    print("translated in all 8 languages. Every sys.* read resolves to a")
    print("SysInfo Q_PROPERTY and every sweep.* call to a bridge Q_INVOKABLE.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
