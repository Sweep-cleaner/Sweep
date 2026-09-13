import QtQuick

Item {
    id: root
    property var theme
    property var manager
    property bool working: false
    property string status: "Kullanılmayan sayfa önbelleğini bırakır."
    property string lastFreed: ""
    property bool needsSudo: false
    property string pw: ""
    property bool pwVisible: false
    signal navigate(string name)

    function human(n) {
        var u = ["B", "KB", "MB", "GB", "TB"], i = 0;
        var v = Number(n);
        while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
        return v.toFixed(1) + " " + u[i];
    }
    function optimize() {
        if (root.working) return;
        root.working = true;
        root.needsSudo = false;
        root.status = "Bellek optimize ediliyor…";
        sweep.memopt();
    }
    function retryWithSudo() {
        root.working = true;
        root.needsSudo = false;
        root.status = "Root şifresiyle yeniden deneniyor…";
        sweep.memoptWithSudo(root.pw);
    }
    Connections {
        target: sweep
        function onMemoptReady(json) {
            root.working = false;
            try {
                var r = JSON.parse(json);
                if (!r || !Array.isArray(r.entries)) throw new Error("bad report");
                var errs = r.failures || [];
                if (Array.isArray(errs) && errs.length > 0) {
                    var msg = String(errs[0].message || "başarısız");
                    if (msg.indexOf("needs root") >= 0 && sweep.needsElevated()) {
                        root.needsSudo = true;
                        root.status = "Root şifresi gerekli — bellek optimizasyonu için şifre girin.";
                        root.pw = "";
                    } else {
                        root.status = msg.slice(0, 160);
                        root.lastFreed = "";
                    }
                } else {
                    var freed = Number(r.reclaimed_bytes || 0);
                    root.lastFreed = human(freed);
                    root.status = "Optimize edildi — ~" + human(freed) + " geri kazanıldı.";
                }
            } catch (e) { root.status = "sonuç okunamadı"; }
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
                height: 210
                enterDelay: 0
                Row {
                    anchors.fill: parent
                    anchors.margins: 20
                    spacing: 26
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 6
                        width: 220
                        Text { text: "🧠  BELLEK OPTİMİZASYONU"; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.muted }
                        Text { text: "RAM'i rahatlat"; font.pixelSize: 24; font.weight: Font.Black; color: theme.c.text }
                        Text { text: "root ister · güvenli seviye (pagecache)"; font.pixelSize: 12; color: theme.c.muted }
                        NeonButton { theme: root.theme; manager: root.manager; label: root.working ? "Çalışıyor…" : "Optimize et"; primary: true; width: 200; busy: root.working; onClicked: optimize() }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 6
                        ProgressRing { theme: root.theme; width: 130; height: 130; value: sys.memTotal > 0 ? sys.memUsed / sys.memTotal : 0; text: sys.memTotal > 0 ? Math.round(sys.memUsed / sys.memTotal * 100) + "%" : "--"; sub: "RAM"; thickness: 11 }
                        Text { anchors.horizontalCenter: parent.horizontalCenter; text: human(sys.memUsed) + " / " + human(sys.memTotal); font.pixelSize: 12; color: theme.c.muted }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 6
                        width: parent.width - 420
                        Text { text: "DURUM"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted }
                        Text { text: root.status; font.pixelSize: 14; color: theme.c.text; wrapMode: Text.WordWrap; width: parent.width }
                        Text { visible: root.lastFreed !== ""; text: "✦ ~" + root.lastFreed + " kazanıldı"; font.pixelSize: 14; font.weight: Font.Bold; color: theme.c.success }
                    }
                }
            }
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: root.needsSudo ? 136 : 0
                visible: root.needsSudo
                opacity: root.needsSudo ? 1 : 0
                Behavior on height { NumberAnimation { duration: 220 } }
                Behavior on opacity { NumberAnimation { duration: 220 } }
                Column {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 12
                    Text { text: "🔓 Root şifresi gerekli"; font.pixelSize: 13; font.weight: Font.Bold; color: theme.c.text }
                    Text { text: "Bellek optimizasyonu (drop_caches) root gerektirir."; font.pixelSize: 11; color: theme.c.muted; width: parent.width; wrapMode: Text.WordWrap }
                    Rectangle {
                        width: parent.width; height: 36; radius: 8
                        color: Qt.rgba(theme.c.edge.r, theme.c.edge.g, theme.c.edge.b, 0.4)
                        border.width: 1; border.color: theme.c.edge
                        TextInput {
                            id: pwInput
                            anchors.fill: parent
                            anchors.margins: 8
                            font.pixelSize: 14
                            color: theme.c.text
                            echoMode: root.pwVisible ? TextInput.Normal : TextInput.Password
                            onTextChanged: root.pw = text
                            focus: true
                            Keys.onEnterPressed: root.retryWithSudo()
                            Keys.onReturnPressed: root.retryWithSudo()
                        }
                    }
                    Row {
                        spacing: 12
                        NeonCheckBox { theme: root.theme; manager: root.manager; text: root.pwVisible ? "Gizle" : "Göster"; font.pixelSize: 11; checked: root.pwVisible; onToggled: root.pwVisible = checked }
                        NeonButton { theme: root.theme; manager: root.manager; label: "Uygula"; primary: root.pw !== ""; width: 100; busy: root.working; onClicked: root.retryWithSudo() }
                    }
                }
            }
            Item { width: 1; height: 22 }
        }
    }
}
