import QtQuick

Item {
    id: root
    property var theme
    property var manager
    property string mode: "big"
    property bool working: false
    property string status: "Tara: ev dizinindeki en büyük dosyalar."
    property var results: []
    property real totalBytes: 0
    signal navigate(string name)

    function human(n) {
        var u = ["B", "KB", "MB", "GB", "TB"], i = 0;
        var v = Number(n);
        while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
        return v.toFixed(1) + " " + u[i];
    }
    function scanBig() {
        if (root.working) return;
        root.mode = "big"; root.working = true;
        root.status = "Büyük dosyalar taranıyor…";
        sweep.bigfiles(30);
    }
    function scanDupes() {
        if (root.working) return;
        root.mode = "dupes"; root.working = true;
        root.status = "Kopya dosyalar taranıyor…";
        sweep.dupes();
    }
    function handleReport(json) {
        root.working = false;
        try {
            var r = JSON.parse(json);
            if (!r || !Array.isArray(r.entries)) throw new Error("bad report");
            var arr = r.entries.slice(0, 30).map(function(e) {
                return { text: String((e && (e.path || e.label)) || ""), bytes: Number((e && e.reclaimed) || 0) };
            });
            root.results = arr;
            root.totalBytes = Number(r.reclaimed_bytes || 0);
            var errs = Number((r.errors && r.errors.length) || 0);
            root.status = arr.length + " bulgu  ·  " + human(root.totalBytes) + (errs > 0 ? "  ·  " + errs + " hata" : "");
        } catch (e) { root.status = "tarama okunamadı"; }
    }
    Connections {
        target: sweep
        function onBigfilesReady(json) { if (root.mode === "big") handleReport(json); else root.working = false; }
        function onDupesReady(json) { if (root.mode === "dupes") handleReport(json); else root.working = false; }
        function onFailed(msg) { root.working = false; root.status = String(msg).slice(0, 160); }
    }
    Component.onCompleted: scanBig()

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
                        Text { anchors.centerIn: parent; text: "📁"; font.pixelSize: 44 }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 4
                        width: parent.width - 380
                        Text { text: "GENİŞ DOSYA TARAMASI"; font.pixelSize: 12; font.weight: Font.DemiBold; color: theme.c.muted }
                        Text { text: root.mode === "big" ? human(root.totalBytes) + " büyük dosya" : root.results.length + " kopya grup"; font.pixelSize: 32; font.weight: Font.Black; color: theme.c.text }
                        Text { text: root.status; font.pixelSize: 13; color: theme.c.muted; elide: Text.ElideRight; width: parent.width }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 10
                        width: 200
                        NeonButton { theme: root.theme; manager: root.manager; label: "Büyük dosyalar"; primary: root.mode === "big"; width: 200; busy: root.working && root.mode === "big"; onClicked: scanBig() }
                        NeonButton { theme: root.theme; manager: root.manager; label: "Kopyalar"; primary: root.mode === "dupes"; width: 200; busy: root.working && root.mode === "dupes"; onClicked: scanDupes() }
                    }
                }
            }
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: Math.max(120, root.results.length * 40 + 36)
                enterDelay: 100
                Column {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 4
                    Repeater {
                        model: root.results
                        delegate: Row {
                            width: parent.width
                            height: 36
                            spacing: 12
                            Text { text: human(modelData.bytes); font.pixelSize: 13; font.weight: Font.Bold; color: theme.c.accent; anchors.verticalCenter: parent.verticalCenter; width: 90; horizontalAlignment: Text.AlignRight }
                            Text { text: modelData.text; font.pixelSize: 12; color: theme.c.text; elide: Text.ElideMiddle; width: parent.width - 102; anchors.verticalCenter: parent.verticalCenter }
                        }
                    }
                    Text {
                        visible: root.results.length === 0 && !root.working
                        anchors.horizontalCenter: parent.horizontalCenter
                        text: "Bulgu yok — farklı bir kök dizinle CLI'dan tarayın.";
                        font.pixelSize: 13; color: theme.c.muted
                    }
                }
            }
            Item { width: 1; height: 22 }
        }
    }
}
