#!/bin/bash
set -u
mkdir -p /app/qml /out
cp -r /src/qml/* /app/qml/
ls /app/qml > /out/files.log
Xvfb :99 -screen 0 1280x800x24 &
sleep 2
export QT_QPA_PLATFORM=xcb
export QSG_INFO=1
export QT_LOGGING_RULES="qt.scenegraph.general=true"
DISPLAY=:99 /app-sweep-qml > /out/app.log 2>&1 &
APP=$!
sleep 14
DISPLAY=:99 scrot /out/shot.png
kill $APP 2>/dev/null
echo "screenshot at /out/shot.png"
