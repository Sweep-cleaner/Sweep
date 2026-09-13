import QtQuick
// Bellek ekranı: canlı kullanım grafiği + güvenli/agresif optimizasyon.
//
// Veri kaynağı `sys` (main.cpp'deki SysInfo) ve `sweep` köprüsüdür. Optimizasyon
// eşzamansız çalışır (`sweep.memopt()` / `sweep.memoptAggressive()`) ve sonuç
// `onMemoptReady` sinyaliyle JSON olarak döner; root yetkisi gerekiyorsa
// (Linux'ta drop_caches, Windows'ta standby purge) şifre paneli açılır.
Item {
    id: root
    property var theme
    property var manager
    property bool working: false
    property bool aggressive: false
    property bool needsSudo: false
    property string pw: ""
    property bool pwVisible: false
    property string status: ""
    property string lastFreed: ""
    property var samples: []
    signal navigate(string name)

    function human(n) {
        var u = ["B", "KB", "MB", "GB", "TB"], i = 0;
        var v = Number(n);
        while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
        return v.toFixed(1) + " " + u[i];
    }
    function memPct() { return sys.memTotal > 0 ? sys.memUsed / sys.memTotal : 0; }
    function swapPct() { return sys.swapTotal > 0 ? sys.swapUsed / sys.swapTotal : 0; }
    function optimize() {
        if (root.working) return;
        root.working = true;
        root.needsSudo = false;
        root.status = i18n.tr("g.working");
        if (root.aggressive) sweep.memoptAggressive();
        else sweep.memopt();
    }
    function preview() {
        if (root.working) return;
        root.working = true;
        root.status = i18n.tr("g.dry_run");
        var json = sweep.memoptDryRun();
        root.working = false;
        root.applyReport(json, true);
    }
    function retryWithSudo() {
        root.working = true;
        root.needsSudo = false;
        root.status = i18n.tr("g.working");
        if (root.aggressive) sweep.memoptAggressiveWithSudo(root.pw);
        else sweep.memoptWithSudo(root.pw);
    }
    function applyReport(json, isPreview) {
        try {
            var r = JSON.parse(json);
            if (!r || !Array.isArray(r.entries)) throw new Error("bad report");
            var errs = r.failures || [];
            if (Array.isArray(errs) && errs.length > 0) {
                var msg = String(errs[0].message || "failed");
                if (msg.indexOf("root") >= 0 && sweep.needsElevated()) {
                    root.needsSudo = true;
                    root.status = i18n.tr("g.needs_root");
                    root.pw = "";
                } else {
                    root.status = msg.slice(0, 160);
                    root.lastFreed = "";
                }
                return;
            }
            var freed = Number(r.bytes_affected !== undefined ? r.bytes_affected : r.reclaimed_bytes || 0);
            root.lastFreed = isPreview ? "" : human(freed);
            var head = r.entries.length > 0 ? String(r.entries[0].label) : "";
            root.status = (isPreview ? i18n.tr("g.dry_run") + ": " : "") + head.slice(0, 160);
        } catch (e) { root.status = "unreadable result"; }
    }

    Timer {
        interval: 1000
        running: true
        repeat: true
        onTriggered: {
            var next = root.samples.slice();
            next.push(root.memPct());
            while (next.length > 60) next.shift();
            root.samples = next;
            wave.requestPaint();
        }
    }
    Connections {
        target: sweep
        function onMemoptReady(json) {
            root.working = false;
            root.applyReport(json, false);
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
            // Temizlik motoru yoksa (Windows'ta exe yalniz calistiysa)
            // ekran sessizce bos kalmasin: sebebi yaz.
            Rectangle {
                visible: typeof sweep.sweepBinOk === "function" ? !sweep.sweepBinOk() : false
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: warnText.implicitHeight + 28
                radius: 12
                color: Qt.rgba(0.85, 0.55, 0.15, 0.14)
                border.width: 1
                border.color: Qt.rgba(0.85, 0.55, 0.15, 0.55)
                Text {
                    id: warnText
                    anchors.fill: parent
                    anchors.margins: 14
                    verticalAlignment: Text.AlignVCenter
                    wrapMode: Text.WordWrap
                    text: "⚠  " + i18n.tr("g.no_engine")
                    font.pixelSize: 13
                    color: theme.c.text
                }
            }
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: 232
                enterDelay: 0
                Row {
                    anchors.fill: parent
                    anchors.margins: 20
                    spacing: 24
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 8
                        width: 250
                        I18nText { key: "t.memory"; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.muted }
                        Text { text: i18n.tr("g.optimize"); font.pixelSize: 24; font.weight: Font.Black; color: theme.c.text }
                        I18nText { key: "s.memory"; font.pixelSize: 12; color: theme.c.muted; width: parent.width; wrapMode: Text.WordWrap }
                        NeonCheckBox {
                            theme: root.theme; manager: root.manager
                            text: i18n.tr("g.optimize_aggressive")
                            font.pixelSize: 12
                            checked: root.aggressive
                            onToggled: root.aggressive = checked
                        }
                        Row {
                            spacing: 10
                            NeonButton {
                                theme: root.theme; manager: root.manager
                                label: i18n.tr("g.optimize")
                                primary: true; width: 150; busy: root.working
                                onClicked: root.optimize()
                            }
                            NeonButton {
                                theme: root.theme; manager: root.manager
                                label: i18n.tr("g.dry_run"); width: 140
                                onClicked: root.preview()
                            }
                        }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 4
                        ProgressRing {
                            theme: root.theme; width: 132; height: 132
                            value: root.memPct()
                            text: sys.memTotal > 0 ? Math.round(root.memPct() * 100) + "%" : "--"
                            sub: i18n.tr("g.memory")
                            thickness: 11
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            text: human(sys.memUsed) + " / " + human(sys.memTotal)
                            font.pixelSize: 12; color: theme.c.muted
                        }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 10
                        width: parent.width - 480
                        Row {
                            spacing: 16
                            Column {
                                spacing: 2
                                I18nText { key: "g.swap"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted }
                                Text {
                                    text: sys.swapTotal > 0 ? Math.round(root.swapPct() * 100) + "%" : "--"
                                    font.pixelSize: 16; font.weight: Font.Bold; color: theme.c.text
                                }
                            }
                            Column {
                                spacing: 2
                                I18nText { key: "g.temperature"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted }
                                Text { text: "--"; font.pixelSize: 16; font.weight: Font.Bold; color: theme.c.text }
                            }
                        }
                        // canlı bellek kullanım dalgası (son 60 saniye)
                        Rectangle {
                            width: parent.width; height: 74; radius: 10
                            color: Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.7)
                            border.width: 1; border.color: theme.c.edge
                            Canvas {
                                id: wave
                                anchors.fill: parent
                                anchors.margins: 8
                                onPaint: {
                                    var ctx = getContext("2d");
                                    ctx.clearRect(0, 0, width, height);
                                    var s = root.samples;
                                    if (s.length < 2) return;
                                    var step = width / Math.max(1, s.length - 1);
                                    ctx.beginPath();
                                    for (var i = 0; i < s.length; i++) {
                                        var x = i * step;
                                        var y = height - Math.min(1, s[i]) * height;
                                        if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
                                    }
                                    ctx.strokeStyle = theme.c.accent;
                                    ctx.lineWidth = 2;
                                    ctx.lineJoin = "round";
                                    ctx.stroke();
                                }
                            }
                        }
                        Text {
                            text: root.status
                            font.pixelSize: 13; color: theme.c.text
                            width: parent.width; wrapMode: Text.WordWrap
                        }
                        Text {
                            visible: root.lastFreed !== ""
                            text: "✦ ~" + root.lastFreed + " " + i18n.tr("reclaimed")
                            font.pixelSize: 13; font.weight: Font.Bold; color: theme.c.success
                        }
                    }
                }
            }
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: 120
                enterDelay: 60
                Row {
                    anchors.fill: parent
                    anchors.margins: 20
                    spacing: 16
                    NeonCheckBox {
                        theme: root.theme; manager: root.manager
                        text: i18n.tr("g.automem")
                        font.pixelSize: 13
                        anchors.verticalCenter: parent.verticalCenter
                        checked: settings.autoMem
                        onToggled: settings.autoMem = checked
                    }
                    Text {
                        width: parent.width - 260
                        anchors.verticalCenter: parent.verticalCenter
                        wrapMode: Text.WordWrap
                        text: i18n.tr("g.automem_desc")
                        font.pixelSize: 12; color: theme.c.muted
                    }
                }
            }
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: root.needsSudo ? 132 : 0
                visible: root.needsSudo
                opacity: root.needsSudo ? 1 : 0
                Behavior on height { NumberAnimation { duration: 220 } }
                Behavior on opacity { NumberAnimation { duration: 220 } }
                Column {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 10
                    I18nText { key: "g.needs_root"; font.pixelSize: 13; font.weight: Font.Bold; color: theme.c.text }
                    Rectangle {
                        width: parent.width; height: 34; radius: 8
                        color: Qt.rgba(theme.c.edge.r, theme.c.edge.g, theme.c.edge.b, 0.4)
                        border.width: 1; border.color: theme.c.edge
                        TextInput {
                            anchors.fill: parent
                            anchors.margins: 8
                            font.pixelSize: 14
                            color: theme.c.text
                            echoMode: root.pwVisible ? TextInput.Normal : TextInput.Password
                            onTextChanged: root.pw = text
                            Keys.onEnterPressed: root.retryWithSudo()
                            Keys.onReturnPressed: root.retryWithSudo()
                        }
                    }
                    Row {
                        spacing: 12
                        NeonCheckBox {
                            theme: root.theme; manager: root.manager
                            text: root.pwVisible ? "•••" : "abc"
                            font.pixelSize: 11
                            checked: root.pwVisible
                            onToggled: root.pwVisible = checked
                        }
                        NeonButton {
                            theme: root.theme; manager: root.manager
                            label: i18n.tr("g.apply"); primary: root.pw !== ""
                            width: 110; busy: root.working
                            onClicked: root.retryWithSudo()
                        }
                    }
                }
            }
            Item { width: 1; height: 22 }
        }
    }
}
