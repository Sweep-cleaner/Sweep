#!/bin/bash
set -u
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
QML="$ROOT/packaging/qml"
IMG="localhost/sweep:latest"
PASS=0
FAIL=0
ok() { PASS=$((PASS+1)); echo "PASS: $1"; }
bad() { FAIL=$((FAIL+1)); echo "FAIL: $1"; }
[ -f "$QML/main.cpp" ] && ok "main.cpp exists" || bad "main.cpp missing"
[ -f "$QML/qml/Main.qml" ] && ok "Main.qml exists" || bad "Main.qml missing"
grep -q "terminate()" "$QML/main.cpp" && ok "cancel uses SIGTERM" || bad "cancel SIGTERM missing"
grep -q '"--quiet"' "$QML/main.cpp" && ok "quiet flag present" || bad "quiet flag missing"
grep -q '"--all"' "$QML/main.cpp" && ok "empty-selection --all fallback" || bad "--all fallback missing"
grep -q '"--json"' "$QML/main.cpp" && ok "json flag present" || bad "json flag missing"
grep -q "progressLine" "$QML/main.cpp" && ok "stderr progress stream" || bad "progress stream missing"
grep -q "QDateTime" "$QML/main.cpp" && ok "QDateTime include" || bad "QDateTime include missing"
grep -q "WebEngine\|Multimedia\|Charts" "$QML/CMakeLists.txt" && bad "heavy modules present" || ok "no heavy Qt modules"
if command -v qmllint >/dev/null 2>&1; then
  if qmllint "$QML/qml/Main.qml" "$QML/qml/Sidebar.qml" "$QML/qml/Dashboard.qml" "$QML/qml/CleanerScreen.qml" "$QML/qml/Settings.qml" "$QML/qml/ThemeManager.qml" "$QML/qml/ChartComponents.qml" "$QML/qml/"Neon*.qml "$QML/qml/themes/"*.qml >/dev/null 2>&1; then ok "qmllint clean"; else bad "qmllint errors"; fi
else
  bad "qmllint not found"
fi
if command -v podman >/dev/null 2>&1; then
  LIST="$(podman run --rm "$IMG" --json list 2>/dev/null)"
  echo "$LIST" | grep -q '"options"' && ok "list JSON has options" || bad "list JSON shape"
  echo "$LIST" | python3 -c "import json,sys; d=json.load(sys.stdin); assert isinstance(d, dict); k=list(d)[0]; assert isinstance(d[k]['options'], list); assert isinstance(d[k]['options'][0]['id'], str)" 2>/dev/null && ok "list JSON parses" || bad "list JSON parse"
  PREV="$(podman run --rm "$IMG" --json preview --all 2>/dev/null | head -c 200000)"
  echo "$PREV" | python3 -c "import json,sys; r=json.load(sys.stdin); assert isinstance(r['entries'], list); e=r['entries'][0]; assert {'cleaner','option','reclaimed','kind'} <= set(e); assert isinstance(r.get('reclaimed_bytes',0), int)" 2>/dev/null && ok "preview --all schema" || bad "preview schema"
  EMPTY="$(podman run --rm "$IMG" --json preview 2>/dev/null)"
  echo "$EMPTY" | grep -q '"error"' && ok "empty preview returns error object" || bad "empty preview shape"
  UNKNOWN="$(podman run --rm "$IMG" --json preview doesnotexistplz 2>/dev/null)"
  echo "$UNKNOWN" | grep -q '"error"' && ok "unknown selector returns error object" || bad "unknown selector shape"
else
  bad "podman not found"
fi
if [ -d /tmp/sweep-qml-check ]; then
  if cmake --build /tmp/sweep-qml-check -j"$(nproc)" >/dev/null 2>&1; then ok "qml binary builds"; else bad "qml build failed"; fi
  [ -x /tmp/sweep-qml-check/sweep-qml ] && ok "binary exists" || bad "binary missing"
  if ldd /tmp/sweep-qml-check/sweep-qml 2>/dev/null | grep -qiE "WebEngine|Multimedia"; then bad "binary links heavy modules"; else ok "binary has no heavy links"; fi
else
  bad "no cmake build dir"
fi
echo "RESULT pass=$PASS fail=$FAIL"
[ "$FAIL" -eq 0 ]
