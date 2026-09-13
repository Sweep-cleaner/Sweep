import QtQuick

// Disk ekrani: birimler + klasor drill-down (salt okunur).
//
// Veri: `sweep.volumes()` (sync, QStorageInfo) ve `sweep.du(yol)` (async,
// `onDuReady` JSON doner). Olcum motoru `sweep du` hicbir seyi silmez;
// bu ekran yalniz gosterir. Klasore cift tikla girilir, `‹` ile yukari.
Item {
    id: root
    property var theme
    property var manager
    property string cwd: ""
    property var crumbs: []
    property var entries: []
    property var volumes: []
    property bool working: false
    property string status: ""
    property string pendingPath: ""
    signal navigate(string name)

    function human(n) {
        var u = ["B", "KB", "MB", "GB", "TB"], i = 0;
        var v = Number(n);
        while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
        return v.toFixed(1) + " " + u[i];
    }
    function baseName(p) {
        var s = String(p).replace(/\\/g, "/");
        if (s.length > 1 && s.charAt(s.length - 1) === "/") s = s.slice(0, -1);
        var i = s.lastIndexOf("/");
        return i >= 0 ? s.slice(i + 1) : s;
    }
    function maxBytes() { return root.entries.length > 0 ? Number(root.entries[0].bytes) || 1 : 1; }
    Component.onCompleted: loadVolumes()
    function loadVolumes() {
        if (root.working) return;
        try {
            var arr = JSON.parse(sweep.volumes());
            root.volumes = Array.isArray(arr) ? arr : [];
            root.cwd = "";
            root.entries = [];
            root.crumbs = [];
            root.status = root.volumes.length + " " + i18n.tr("disk.volumes");
        } catch (e) { root.status = "unreadable result"; }
    }
    function openDir(path) {
        if (root.working) return;
        root.working = true;
        root.pendingPath = String(path);
        root.status = i18n.tr("g.working");
        sweep.du(root.pendingPath);
    }
    function applyDu(json) {
        try {
            var r = JSON.parse(json);
            if (!r || !Array.isArray(r.entries)) throw new Error("bad report");
            var arr = [];
            for (var i = 0; i < r.entries.length; i++) {
                var e = r.entries[i];
                if (!e || !e.path) continue; // ozet satirini atla
                arr.push({
                    path: String(e.path),
                    name: baseName(e.path),
                    bytes: Number(e.reclaimed || 0),
                    isDir: String(e.option || "") === "dir"
                });
            }
            root.entries = arr;
            root.cwd = root.pendingPath;
            buildCrumbs(root.cwd);
            root.status = arr.length === 0 ? i18n.tr("disk.empty") : root.cwd;
        } catch (e) { root.status = "unreadable result"; }
    }
    function buildCrumbs(path) {
        var s = String(path).replace(/\\/g, "/");
        var unc = s.slice(0, 2) === "//";
        var parts = s.split("/").filter(function(p) { return p !== ""; });
        var out = [];
        var acc = null; // null = henuz kok yazilmadi
        var i = 0;
        if (unc && parts.length >= 2) {
            // Ag paylasimi: ilk iki parca kok ("//sunucu/paylasim").
            acc = "//" + parts[0] + "/" + parts[1];
            out.push({ label: parts[0] + "/" + parts[1], path: acc });
            i = 2;
        }
        for (; i < parts.length; i++) {
            var seg;
            if (acc === null) {
                if (/^[A-Za-z]:$/.test(parts[i])) { seg = parts[i] + "/"; } // "C:/"
                else { seg = "/" + parts[i]; } // POSIX kok
                acc = seg;
            } else {
                seg = (acc.charAt(acc.length - 1) === "/") ? acc + parts[i] : acc + "/" + parts[i];
                acc = seg;
            }
            out.push({ label: parts[i], path: acc });
        }
        root.crumbs = out;
    }
    function goUp() {
        if (root.working || root.cwd === "") return;
        var s = root.cwd.replace(/\\/g, "/");
        if (s.length > 1 && s.charAt(s.length - 1) === "/") s = s.slice(0, -1);
        var i = s.lastIndexOf("/");
        if (i <= 0) { loadVolumes(); return; }
        // "C:" tek basina kalirsa surucu koku ac.
        var parent = s.slice(0, i);
        if (/^[A-Za-z]:$/.test(parent)) parent += "/";
        openDir(parent);
    }
    Connections {
        target: sweep
        function onDuReady(json) {
            if (!root.working) return;
            root.working = false;
            root.applyDu(json);
        }
        function onFailed(msg) { root.working = false; root.status = String(msg).slice(0, 160); }
    }

    Flickable {
        anchors.fill: parent
        contentWidth: width
        contentHeight: col.implicitHeight + 44
        clip: true
        Column {
            id: col
            width: parent.width
            spacing: 16
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
                        Text { anchors.centerIn: parent; text: "◍"; font.pixelSize: 44; color: theme.c.accent }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 4
                        width: parent.width - 380
                        Text { text: i18n.tr("t.disk").toUpperCase(); font.pixelSize: 12; font.weight: Font.DemiBold; color: theme.c.muted }
                        Text { text: root.cwd === "" ? i18n.tr("disk.volumes") : baseName(root.cwd); font.pixelSize: 26; font.weight: Font.Black; color: theme.c.text; elide: Text.ElideMiddle; width: parent.width }
                        Text { text: root.status; font.pixelSize: 13; color: theme.c.muted; elide: Text.ElideMiddle; width: parent.width }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 10
                        width: 200
                        NeonButton { theme: root.theme; manager: root.manager; label: i18n.tr("disk.volumes"); width: 200; busy: false; onClicked: loadVolumes() }
                        NeonButton { theme: root.theme; manager: root.manager; label: root.working ? i18n.tr("g.working") : i18n.tr("disk.cancel"); width: 200; busy: root.working; onClicked: if (root.working) sweep.cancel() }
                    }
                }
            }
            // birimler
            GlassCard {
                visible: root.cwd === ""
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: Math.max(120, root.volumes.length * 56 + 36)
                enterDelay: 80
                Column {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 8
                    Repeater {
                        model: root.volumes
                        delegate: Rectangle {
                            width: parent.width
                            height: 48
                            radius: 10
                            color: volMouse.containsMouse ? Qt.rgba(theme.c.edge.r, theme.c.edge.g, theme.c.edge.b, 0.4) : "transparent"
                            Column {
                                anchors.fill: parent
                                anchors.margins: 6
                                spacing: 5
                                Row {
                                    width: parent.width
                                    spacing: 10
                                    Text { text: String(modelData.name || modelData.root); font.pixelSize: 13; font.weight: Font.DemiBold; color: theme.c.text; anchors.verticalCenter: parent.verticalCenter; width: parent.width - 220; elide: Text.ElideRight }
                                    Text { text: human(Number(modelData.total) - Number(modelData.free)) + " / " + human(Number(modelData.total)); font.pixelSize: 12; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter; width: 210; horizontalAlignment: Text.AlignRight }
                                }
                                Rectangle {
                                    width: parent.width; height: 6; radius: 3
                                    color: theme.c.edge
                                    Rectangle {
                                        width: parent.width * (Number(modelData.total) > 0 ? Math.min(1, (Number(modelData.total) - Number(modelData.free)) / Number(modelData.total)) : 0)
                                        height: parent.height; radius: 3
                                        color: theme.c.accent
                                    }
                                }
                            }
                            MouseArea { id: volMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: openDir(String(modelData.root)) }
                        }
                    }
                }
            }
            // klasor icerigi
            GlassCard {
                visible: root.cwd !== ""
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: Math.max(160, root.crumbs.length > 0 ? 52 : 12, root.entries.length * 40 + 100)
                enterDelay: 80
                Column {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 4
                    Row {
                        width: parent.width
                        spacing: 6
                        NeonButton { theme: root.theme; manager: root.manager; label: "‹ " + i18n.tr("disk.up"); width: 110; busy: false; onClicked: goUp() }
                        Flickable {
                            width: parent.width - 122
                            height: 36
                            contentWidth: crumbRow.implicitWidth
                            clip: true
                            Row {
                                id: crumbRow
                                spacing: 2
                                height: 36
                                Repeater {
                                    model: root.crumbs
                                    delegate: Row {
                                        spacing: 2
                                        height: 36
                                        Text { text: "/"; font.pixelSize: 13; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter; visible: index > 0 }
                                        Text {
                                            text: String(modelData.label); font.pixelSize: 13
                                            font.weight: index === root.crumbs.length - 1 ? Font.Bold : Font.Normal
                                            color: index === root.crumbs.length - 1 ? theme.c.text : theme.c.accent
                                            anchors.verticalCenter: parent.verticalCenter
                                            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: openDir(String(modelData.path)) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Repeater {
                        model: root.entries
                        delegate: Rectangle {
                            width: parent.width
                            height: 36
                            radius: 8
                            color: rowMouse.containsMouse ? Qt.rgba(theme.c.edge.r, theme.c.edge.g, theme.c.edge.b, 0.35) : "transparent"
                            Row {
                                anchors.fill: parent
                                anchors.leftMargin: 8
                                anchors.rightMargin: 8
                                spacing: 12
                                Text { text: human(modelData.bytes); font.pixelSize: 13; font.weight: Font.Bold; color: theme.c.accent; anchors.verticalCenter: parent.verticalCenter; width: 90; horizontalAlignment: Text.AlignRight }
                                Rectangle {
                                    width: 90; height: 6; radius: 3
                                    anchors.verticalCenter: parent.verticalCenter
                                    color: theme.c.edge
                                    Rectangle { width: parent.width * Math.min(1, Number(modelData.bytes) / root.maxBytes()); height: parent.height; radius: 3; color: modelData.isDir ? theme.c.accent : theme.c.muted }
                                }
                                Text { text: (modelData.isDir ? "› " : "") + String(modelData.name); font.pixelSize: 12; color: theme.c.text; elide: Text.ElideMiddle; width: parent.width - 214; anchors.verticalCenter: parent.verticalCenter }
                            }
                            MouseArea { id: rowMouse; anchors.fill: parent; hoverEnabled: true; onDoubleClicked: if (modelData.isDir) openDir(String(modelData.path)) }
                        }
                    }
                    Text {
                        visible: root.entries.length === 0 && !root.working
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: i18n.tr("disk.empty")
                        font.pixelSize: 13; color: theme.c.muted
                    }
                }
            }
            Item { width: 1; height: 22 }
        }
    }
}
