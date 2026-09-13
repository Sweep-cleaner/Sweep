#!/usr/bin/env bash
# Headless smoke test for the Qt6 GUI — run this from WSL (or any Linux box).
#
# The GUI normally needs a display. With QT_QPA_PLATFORM=offscreen Qt renders
# into a buffer instead, so every route can be loaded and any QML error is
# printed on stderr without a window ever appearing.
#
# Usage (from WSL):
#   bash packaging/qml/smoke-wsl.sh            # build, then check every route
#   bash packaging/qml/smoke-wsl.sh --no-build # skip the build
#
# Requires: Qt6 dev packages (qmake/cmake), and the SWEEP_SCREEN hook in
# packaging/qml/main.cpp.

set -uo pipefail

REPO="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO" || exit 1

BUILD_DIR="${BUILD_DIR:-packaging/qml/build}"
BIN="$BUILD_DIR/sweep-qml"
# Keep in sync with Sidebar.qml (verify_qml.py's route guard enforces this).
ROUTES=(Dashboard Cleaner Files History System Optimize StartupMgr SysInfo Network Privacy Schedule Settings)

if [[ "${1:-}" != "--no-build" ]]; then
    echo "== building =="
    cmake --build "$BUILD_DIR" -j"$(nproc)" || {
        echo "BUILD FAILED — is Qt6 installed and is $BUILD_DIR configured?"
        echo "Configure once with:"
        echo "  cmake -B packaging/qml/build -S packaging/qml -DCMAKE_BUILD_TYPE=Release"
        exit 1
    }
fi

if [[ ! -x "$BIN" ]]; then
    echo "No $BIN — build first (or pass a different BUILD_DIR=...)." >&2
    exit 1
fi

# Force QML/JS errors to the console instead of swallowing them.
export QT_LOGGING_RULES="qt.qml.binding.removal.info=true;default.debug=true"
export QSG_RENDER_LOOP=basic

# Which platform plugins to try. offscreen alone is NOT enough: it never
# creates a native window, so it hides crashes in the display-only paths
# (system tray, DWM effects, scene graph). Add the real plugin too.
#   Windows : SWEEP_SMOKE_PLATFORMS="offscreen windows"
#   Linux   : SWEEP_SMOKE_PLATFORMS="offscreen xcb"   (or wayland)
PLATFORMS="${SWEEP_SMOKE_PLATFORMS:-offscreen}"
# 128+signal: 139=SIGSEGV, 134=SIGABRT, 136=SIGFPE.
CRASH_SIGNALS=(139 134 136)

failures=0
for platform in $PLATFORMS; do
    export QT_QPA_PLATFORM="$platform"
    echo "-- platform: $platform"
    for route in "${ROUTES[@]}"; do
        out="$(SWEEP_SCREEN="$route" timeout 12 "$BIN" 2>&1 >/dev/null)"
        code=$?
        crashed=no
        for sig in "${CRASH_SIGNALS[@]}"; do
            [[ $code -eq $sig ]] && crashed=yes
        done
        # 124 == timeout, which is the *good* case: the window stayed up.
        if [[ $crashed == yes ]]; then
            echo "FAIL  $route (exit $code — crash signal)"
            sed -n '1,12p' <<<"$out" | sed 's/^/        /'
            failures=$((failures + 1))
        elif grep -qiE "is not a type|cannot read property|is undefined|ReferenceError|TypeError|QQmlApplicationEngine failed|file:///.*:[0-9]+:[0-9]+:" <<<"$out"; then
            echo "FAIL  $route"
            sed -n '1,12p' <<<"$out" | sed 's/^/        /'
            failures=$((failures + 1))
        elif [[ $code -eq 124 || $code -eq 0 ]]; then
            echo "ok    $route"
        else
            echo "FAIL  $route (exit $code)"
            sed -n '1,12p' <<<"$out" | sed 's/^/        /'
            failures=$((failures + 1))
        fi
    done
done

echo
if [[ $failures -eq 0 ]]; then
    echo "ALL ${#ROUTES[@]} ROUTES LOADED CLEANLY"
    echo "This is still a headless check: it proves the QML parses, binds and"
    echo "instantiates. It does NOT prove the layout looks right — click"
    echo "through the GUI once with a display to close G7 properly."
    exit 0
fi
echo "$failures route(s) reported errors."
exit 1
