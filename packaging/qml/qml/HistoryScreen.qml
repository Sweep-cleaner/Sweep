import QtQuick
Item {
    id: root
    property var theme
    property var manager
    property var tops: []
    property var runs: []
    property real totalBytes: 0
    property real totalFiles: 0
    property string status: "Yükleniyor…"
    property bool loading: true
    property bool runsBusy: false
    property real shownBytes: 0
    signal navigate(string name)
    function human(n) {
        var u = ["B", "KB", "MB", "GB", "TB"], i = 0;
        var v = Number(n);
        while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
        return v.toFixed(1) + " " + u[i];
    }
    function reload() { root.loading = true; sweep.history(); }
    function restoreRun(id) { root.runsBusy = true; sweep.undoRun(id); }
    onTotalBytesChanged: { countAnim.to = root.totalBytes; countAnim.start(); }
    NumberAnimation { id: countAnim; target: root; property: "shownBytes"; duration: 700; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] }
    Connections {
        target: sweep
        function onHistoryReady(json) {
            root.loading = false;
            try {
                var r = JSON.parse(json);
                if (!r || !Array.isArray(r.entries)) throw new Error("bad report");
                var by = {}, i;
                for (i = 0; i < r.entries.length; i++) {
                    var e = r.entries[i];
                    if (!e || typeof e.cleaner !== "string") continue;
                    by[e.cleaner] = (by[e.cleaner] || 0) + Number(e.reclaimed || 0);
                }
                var arr = Object.keys(by).map(function(k) { return { label: k, bytes: by[k] }; });
                arr.sort(function(a, b) { return b.bytes - a.bytes; });
                root.tops = arr.slice(0, 8);
                root.totalBytes = Number(r.reclaimed_bytes || 0);
                root.totalFiles = Number(r.files_removed || 0);
                root.status = r.entries.length === 0 ? "Henüz kayıt yok — ilk temizliği yapın." : "Son 30 gün";
            } catch (e) { root.status = "geçmiş okunamadı"; }
            sweep.undoList();
        }
        function onUndoListReady(json) {
            try {
                var r = JSON.parse(json);
                if (!r || !Array.isArray(r.entries)) throw new Error("bad runs");
                var out = [];
                for (var i = 0; i < r.entries.length; i++) {
                    var e = r.entries[i];
                    if (!e || e.cleaner !== "undo" || e.option === "runs") continue;
                    out.push({ id: String(e.option || ""), label: String(e.label || e.option || ""), bytes: Number(e.reclaimed || 0) });
                }
                root.runs = out;
            } catch (e) { root.runs = []; }
        }
        function onUndoReady(json) {
            root.runsBusy = false;
            try {
                var r = JSON.parse(json);
                if (!r || !Array.isArray(r.entries)) throw new Error("bad report");
                var errs = (r.errors && r.errors.length) || r.errors || 0;
                if (Number(errs) > 0 && Array.isArray(r.failures) && r.failures.length > 0 && r.failures[0].message) {
                    root.status = String(r.failures[0].message).slice(0, 160);
                } else {
                    var n = r.entries.filter(function(x) { return x && x.option === "restore"; }).length;
                    root.status = "restored: " + n + " files";
                }
            } catch (e) { root.status = "undo failed"; }
            reload();
        }
        function onFailed(msg) { root.loading = false; root.runsBusy = false; root.status = String(msg).slice(0, 160); }
    }
    Component.onCompleted: reload()
    Flickable {
        anchors.fill: parent
        contentWidth: width
        contentHeight: col.implicitHeight + 44
        clip: true
        Column {
            id: col
            width: parent.width
            spacing: 16 // hero
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: 168
                enterDelay: 0
                Row {
                    anchors.fill: parent
                    anchors.margins: 20
                    spacing: 18
                    Rectangle {
                        width: 96; height: 96; radius: 24
                        anchors.verticalCenter: parent.verticalCenter
                        color: Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.15)
                        border.width: 1; border.color: Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.5)
                        Text { anchors.centerIn: parent; text: "🕘"; font.pixelSize: 44 }
                        RotationAnimation on rotation {
                            running: root.loading
                            loops: Animation.Infinite
                            from: 0; to: 360; duration: 1600
                        }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 4
                        width: parent.width - 380
                        Row {
                            spacing: 8
                            Rectangle {
                                width: 8; height: 8; radius: 4
                                anchors.verticalCenter: parent.verticalCenter
                                color: root.loading ? theme.c.warning : theme.c.success
                            }
                            Text { text: "TOPLAM TEMİZLENEN"; font.pixelSize: 12; font.weight: Font.DemiBold; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                        }
                        Text { text: human(root.shownBytes); font.pixelSize: 40; font.weight: Font.Black; color: theme.c.text }
                        Text { text: root.loading ? "Kayıtlar okunuyor…" : (root.status + "  ·  " + root.totalFiles + " dosya"); font.pixelSize: 13; color: theme.c.muted }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 10
                        width: 200
                        NeonButton { theme: root.theme; manager: root.manager; label: "Yenile"; width: 200; busy: root.loading; progress: root.loading ? 0.4 : 0; onClicked: reload() }
                        NeonButton { theme: root.theme; manager: root.manager; label: "Temizleyiciye Git"; width: 200; onClicked: root.navigate("Cleaner") }
                    }
                }
            }
            // section
            Row {
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 10
                Text { text: "EN ÇOK TEMİZLEYENLER"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                Rectangle { width: parent.width - 240; height: 1; anchors.verticalCenter: parent.verticalCenter; color: theme.c.edge; opacity: 0.6 }
            }
            // top list
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: Math.max(120, root.tops.length * 46 + 36)
                enterDelay: 100
                Column {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 6
                    Repeater {
                        model: root.tops
                        delegate: Row {
                            width: parent.width
                            height: 40
                            spacing: 12
                            Rectangle {
                                width: 30; height: 30; radius: 9
                                anchors.verticalCenter: parent.verticalCenter
                                color: index < 3 ? theme.c.accent : Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.9)
                                border.width: 1; border.color: theme.c.edge
                                Text { anchors.centerIn: parent; text: ["🥇", "🥈", "🥉"][index] || String(index + 1); font.pixelSize: 14; color: theme.c.accentText }
                            }
                            Column {
                                width: parent.width - 170
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 4
                                Text { text: modelData.label; font.pixelSize: 13; font.weight: Font.DemiBold; color: theme.c.text; elide: Text.ElideRight; width: parent.width }
                                Rectangle {
                                    width: parent.width; height: 6; radius: 3
                                    color: theme.c.edge
                                    Rectangle {
                                        width: parent.width * (root.tops.length > 0 ? Number(modelData.bytes) / Math.max(1, Number(root.tops[0].bytes)) : 0)
                                        height: parent.height; radius: 3
                                        color: theme.c.accent
                                        Behavior on width { NumberAnimation { duration: 500; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
                                    }
                                }
                            }
                            Text { text: human(modelData.bytes); font.pixelSize: 13; font.weight: Font.Bold; color: theme.c.text; anchors.verticalCenter: parent.verticalCenter; width: 110; horizontalAlignment: Text.AlignRight }
                        }
                    }
                    Column {
                        width: parent.width
                        spacing: 6
                        visible: root.tops.length === 0 && !root.loading
                        Text { anchors.horizontalCenter: parent.horizontalCenter; text: "📭"; font.pixelSize: 34 }
                        Text { anchors.horizontalCenter: parent.horizontalCenter; text: "Kayıt bulunamadı — önce bir temizlik yapın."; font.pixelSize: 13; color: theme.c.muted }
                    }
                }
            }
            // runs (selective restore)
            Row {
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 10
                Text { text: i18n.tr("u.runs_title").toUpperCase(); font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                Rectangle { width: parent.width - 240; height: 1; anchors.verticalCenter: parent.verticalCenter; color: theme.c.edge; opacity: 0.6 }
            }
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: Math.max(120, root.runs.length * 52 + 36)
                enterDelay: 200
                Column {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 6
                    Repeater {
                        model: root.runs
                        delegate: Row {
                            width: parent.width
                            height: 46
                            spacing: 12
                            Column {
                                width: parent.width - 220
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 4
                                Text { text: modelData.label; font.pixelSize: 12; font.weight: Font.DemiBold; color: theme.c.text; elide: Text.ElideRight; width: parent.width }
                                Text { text: human(modelData.bytes); font.pixelSize: 11; color: theme.c.muted }
                            }
                            NeonButton { theme: root.theme; manager: root.manager; label: i18n.tr("u.restore"); width: 120; anchors.verticalCenter: parent.verticalCenter; busy: root.runsBusy; onClicked: restoreRun(modelData.id) }
                        }
                    }
                    Column {
                        width: parent.width
                        spacing: 6
                        visible: root.runs.length === 0
                        Text { anchors.horizontalCenter: parent.horizontalCenter; text: "📭"; font.pixelSize: 34 }
                        Text { anchors.horizontalCenter: parent.horizontalCenter; text: i18n.tr("u.no_runs"); font.pixelSize: 13; color: theme.c.muted }
                    }
                }
            }
            Item { width: 1; height: 22 }
        }
    }
}
