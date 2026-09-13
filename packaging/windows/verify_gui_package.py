#!/usr/bin/env python3
"""Verify a staged or installed Sweep GUI package.

This is the acceptance check for the Windows packaging path. It exists because
the shipped installer once contained a sweep-qml.exe linked against the UCRT
while the bundled Qt 6.8.1 win64_mingw DLLs link msvcrt.dll. Two CRTs in one
process made the GUI die with 0xC0000005 inside ntdll before any window was
created - the user saw "nothing opens".

Usage:
    python packaging/windows/verify_gui_package.py "C:\\Program Files\\Sweep"
    python packaging/windows/verify_gui_package.py packaging/dist/windows/stage
"""
from __future__ import annotations

import os
import struct
import sys

UCRT_PREFIXES = ("api-ms-win-crt",)

# Windows DLLs the GUI cannot start without.
REQUIRED_FILES = (
    "sweep-qml.exe",
    "sweep.exe",
    "sweep.ico",
    "Qt6Core.dll",
    "Qt6Gui.dll",
    "Qt6Qml.dll",
    "Qt6Quick.dll",
    os.path.join("platforms", "qwindows.dll"),
    os.path.join("qml", "Main.qml"),
)


def read_pe_imports(path: str) -> tuple[list[str], int]:
    """Return (imported_dll_names, subsystem) for a PE file."""
    with open(path, "rb") as fh:
        data = fh.read()
    if data[:2] != b"MZ":
        raise ValueError(f"{path}: not a PE file (no MZ header)")
    pe_off = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe_off : pe_off + 4] != b"PE\0\0":
        raise ValueError(f"{path}: not a PE file (no PE signature)")
    num_sections = struct.unpack_from("<H", data, pe_off + 6)[0]
    opt_size = struct.unpack_from("<H", data, pe_off + 20)[0]
    opt_off = pe_off + 24
    magic = struct.unpack_from("<H", data, opt_off)[0]
    if magic not in (0x10B, 0x20B):
        raise ValueError(f"{path}: unknown optional header magic 0x{magic:x}")
    subsystem = struct.unpack_from("<H", data, opt_off + 68)[0]
    dd_off = opt_off + (96 if magic == 0x10B else 112)
    import_rva, _import_size = struct.unpack_from("<II", data, dd_off + 8)

    # RVA -> file offset via the section table.
    sections = []
    sec_off = opt_off + opt_size
    for i in range(num_sections):
        base = sec_off + i * 40
        va, raw_size, raw_ptr = struct.unpack_from("<III", data, base + 12)
        sections.append((va, raw_size, raw_ptr))

    def rva_to_off(rva: int) -> int | None:
        for va, raw_size, raw_ptr in sections:
            if va <= rva < va + max(raw_size, 1):
                return raw_ptr + (rva - va)
        return None

    names: list[str] = []
    if import_rva:
        desc = rva_to_off(import_rva)
        if desc is None:
            raise ValueError(f"{path}: import directory RVA is not mapped")
        while True:
            name_rva = struct.unpack_from("<I", data, desc + 12)[0]
            if name_rva == 0:
                break
            off = rva_to_off(name_rva)
            if off is None:
                break
            end = data.index(b"\0", off)
            names.append(data[off:end].decode("ascii", "replace"))
            desc += 20
    return names, subsystem


def check_file(path: str) -> tuple[bool, list[str]]:
    """PE-only check for a single executable (e.g. a fresh build, not yet staged)."""
    lines = [f"Verifying executable: {path}"]
    ok = True
    try:
        imports, subsystem = read_pe_imports(path)
    except ValueError as exc:
        return False, lines + [f"  FAIL  cannot parse: {exc}"]
    if subsystem == 2:
        lines.append("  ok    PE subsystem = 2 (WINDOWS, no console)")
    else:
        ok = False
        lines.append(f"  FAIL  PE subsystem = {subsystem} (expected 2)")
    lower = [n.lower() for n in imports]
    bad = sorted({n for n in lower if n.startswith(UCRT_PREFIXES) or n == "ucrtbase.dll"})
    if bad:
        ok = False
        lines.append(f"  FAIL  links the UCRT: {', '.join(bad)}")
    else:
        lines.append("  ok    does not link the UCRT")
    lines.append("")
    lines.append("RESULT: " + ("PASS - safe against the msvcrt-built Qt runtime" if ok else "FAIL - do not ship this binary"))
    return ok, lines


def check_dir(root: str) -> tuple[bool, list[str]]:
    if os.path.isfile(root):
        return check_file(root)
    lines: list[str] = []
    ok = True

    def fail(msg: str) -> None:
        nonlocal ok
        ok = False
        lines.append(f"  FAIL  {msg}")

    def good(msg: str) -> None:
        lines.append(f"  ok    {msg}")

    lines.append(f"Verifying GUI package: {root}")
    if not os.path.isdir(root):
        return False, [f"  FAIL  directory does not exist: {root}"]

    for rel in REQUIRED_FILES:
        p = os.path.join(root, rel)
        if os.path.isfile(p):
            good(f"present: {rel} ({os.path.getsize(p)} bytes)")
        else:
            fail(f"missing: {rel}")

    # Nested staging leftovers must never reach an installer.
    junk = [d for d in os.listdir(root) if d.startswith("stage.new.") or d.startswith("stage.old.")]
    if junk:
        fail(f"staging leftovers packaged: {', '.join(sorted(junk))}")
    else:
        good("no stage.new.*/stage.old.* leftovers")

    gui = os.path.join(root, "sweep-qml.exe")
    if not os.path.isfile(gui):
        return ok, lines

    try:
        imports, subsystem = read_pe_imports(gui)
    except ValueError as exc:
        return False, lines + [f"  FAIL  cannot parse sweep-qml.exe: {exc}"]

    if subsystem == 2:
        good("sweep-qml.exe PE subsystem = 2 (WINDOWS, no console)")
    else:
        fail(f"sweep-qml.exe PE subsystem = {subsystem} (expected 2; a console window will flash)")

    lower = [n.lower() for n in imports]
    bad = [n for n in lower if n.startswith(UCRT_PREFIXES) or n == "ucrtbase.dll"]
    if bad:
        fail(f"sweep-qml.exe links the UCRT ({', '.join(sorted(set(bad)))}) while Qt links msvcrt.dll")
    else:
        good("sweep-qml.exe does not link the UCRT")

    if "msvcrt.dll" in lower:
        good("sweep-qml.exe links msvcrt.dll")
    else:
        lines.append("  note  sweep-qml.exe does not import msvcrt.dll directly (may come via libstdc++/Qt)")

    qt_core = os.path.join(root, "Qt6Core.dll")
    if os.path.isfile(qt_core):
        try:
            qt_imports, _ = read_pe_imports(qt_core)
            qt_lower = [n.lower() for n in qt_imports]
            if "msvcrt.dll" in qt_lower and not any(
                n.startswith(UCRT_PREFIXES) or n == "ucrtbase.dll" for n in qt_lower
            ):
                good("Qt6Core.dll links msvcrt.dll (legacy MSVCRT build)")
            else:
                fail("Qt6Core.dll is not a plain msvcrt build; the two-CRT check is inconclusive")
        except ValueError as exc:
            fail(f"cannot parse Qt6Core.dll: {exc}")

    lines.append("")
    lines.append("RESULT: " + ("PASS - the GUI package can start" if ok else "FAIL - do not ship this package"))
    return ok, lines


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print(__doc__)
        return 2
    ok, lines = check_dir(argv[1])
    print("\n".join(lines))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
