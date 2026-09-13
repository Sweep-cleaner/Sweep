import QtQuick
Item {
    id: root
    property var theme
    property var manager
    property var managers: []
    property var cpuHist: []
    property var memHist: []
    property string mgrStatus: ""
    signal navigate(string name)
    function human(n) {
        var u = ["B", "KB", "MB", "GB", "TB"], i = 0;
        var v = Number(n);
        while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
        return v.toFixed(1) + " " + u[i];
    }
    function pushHist(arr, v) {
        var w = arr.concat([v]);
        if (w.length > 60) w = w.slice(w.length - 60);
        return w;
    }
    Timer {
        interval: 1000
        running: true
        repeat: true
        onTriggered: {
            root.cpuHist = pushHist(root.cpuHist, sys.cpuPct);
            var pct = sys.memTotal > 0 ? sys.memUsed / sys.memTotal * 100 : 0;
            root.memHist = pushHist(root.memHist, pct);
            ioBar.target = sys.ioRate;
        }
    }
    Connections {
        target: sweep
        function onDiagnosticsReady(json) {
            try {
                var r = JSON.parse(json);
                root.managers = (r && r.managers) || [];
                root.mgrStatus = root.managers.filter(function(m) { return m.found; }).length + " / " + root.managers.length + " kurulu";
            } catch (e) { root.mgrStatus = "okunamadı"; }
        }
    }
    Component.onCompleted: sweep.diagnostics()
    Flickable {
        anchors.fill: parent
        contentWidth: width
        contentHeight: col.implicitHeight + 44
        clip: true
        Column {
            id: col
            width: parent.width
            spacing: 16 // hero rings
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
                        width: 200
                        Text { text: "⚡  CANLI SİSTEM"; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.muted }
                        Text { text: "Donanım nabzı"; font.pixelSize: 24; font.weight: Font.Black; color: theme.c.text }
                        Text { text: "1 sn aralıkla güncellenir"; font.pixelSize: 12; color: theme.c.muted }
                        NeonButton { theme: root.theme; manager: root.manager; label: "Panoya Dön"; width: 170; onClicked: root.navigate("Dashboard") }
                    }
                    Row {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 22
                        Column {
                            spacing: 6
                            ProgressRing { theme: root.theme; width: 120; height: 120; value: sys.cpuPct / 100; text: sys.cpuPct.toFixed(0) + "%"; sub: "CPU"; thickness: 11 }
                            Text { anchors.horizontalCenter: parent.horizontalCenter; text: "🖥️ işlemci"; font.pixelSize: 12; color: theme.c.muted }
                        }
                        Column {
                            spacing: 6
                            ProgressRing { theme: root.theme; width: 120; height: 120; value: sys.memTotal > 0 ? sys.memUsed / sys.memTotal : 0; text: sys.memTotal > 0 ? Math.round(sys.memUsed / sys.memTotal * 100) + "%" : "--"; sub: "RAM"; thickness: 11 }
                            Text { anchors.horizontalCenter: parent.horizontalCenter; text: "🧠 bellek"; font.pixelSize: 12; color: theme.c.muted }
                        }
                        Column {
                            spacing: 6
                            ProgressRing { theme: root.theme; width: 120; height: 120; value: sys.diskTotal > 0 ? sys.diskUsed / sys.diskTotal : 0; text: sys.diskTotal > 0 ? Math.round(sys.diskUsed / sys.diskTotal * 100) + "%" : "--"; sub: "DİSK"; thickness: 11 }
                            Text { anchors.horizontalCenter: parent.horizontalCenter; text: "💾 disk"; font.pixelSize: 12; color: theme.c.muted }
                        }
                    }
                }
            }
            // section
            Row {
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 10
                Text { text: "DETAYLAR"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                Rectangle { width: parent.width - 170; height: 1; anchors.verticalCenter: parent.verticalCenter; color: theme.c.edge; opacity: 0.6 }
            }
            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 14
                NeonCard {
                    theme: root.theme; manager: root.manager
                    width: (col.width - 44 - 28) / 3
                    height: 132
                    title: "🖥️ CPU"
                    value: sys.cpuPct.toFixed(1) + " %"
                    spark: root.cpuHist
                }
                NeonCard {
                    theme: root.theme; manager: root.manager
                    width: (col.width - 44 - 28) / 3
                    height: 132
                    title: "🧠 RAM"
                    value: human(sys.memUsed) + " / " + human(sys.memTotal)
                    spark: root.memHist
                }
                GlassCard {
                    theme: root.theme; manager: root.manager
                    width: (col.width - 44 - 28) / 3
                    height: 132
                    enterDelay: 120
                    Column {
                        anchors.fill: parent
                        anchors.margins: 14
                        spacing: 4
                        Text { text: "💽 G/Ç"; font.pixelSize: 12; font.weight: Font.Medium; color: theme.c.muted }
                        Text { text: human(ioBar.target) + "/s"; font.pixelSize: 22; font.weight: Font.Bold; color: theme.c.text }
                        Rectangle {
                            width: parent.width; height: 8; radius: 4
                            color: theme.c.edge
                            Rectangle {
                                id: ioBar
                                property real target: 0
                                width: Math.min(parent.width, target / 50000000 * parent.width)
                                height: parent.height; radius: 4
                                color: theme.c.success
                                Behavior on width { NumberAnimation { duration: 400; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
                            }
                        }
                    }
                }
            }
            // per-core CPU (Linux: /proc/stat per-cpu; Windows: tek global bar)
            Row {
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 10
                Text { text: "ÇEKİRDEKLER · " + sys.cpuCount; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                Rectangle { width: parent.width - 220; height: 1; anchors.verticalCenter: parent.verticalCenter; color: theme.c.edge; opacity: 0.6 }
            }
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: sys.cpuCores.length > 0 ? 118 : 86
                enterDelay: 140
                Row {
                    visible: sys.cpuCores.length > 0
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 8
                    Repeater {
                        model: sys.cpuCores
                        delegate: Column {
                            width: (parent.width - 8 * Math.max(1, sys.cpuCores.length - 1)) / Math.max(1, sys.cpuCores.length)
                            height: parent.height
                            spacing: 5
                            Text {
                                anchors.horizontalCenter: parent.horizontalCenter
                                text: index
                                font.pixelSize: 9
                                font.weight: Font.DemiBold
                                color: Number(modelData) > 80 ? theme.c.danger : theme.c.muted
                            }
                            Rectangle {
                                width: parent.width
                                height: parent.height - 22
                                radius: 4
                                color: theme.c.edge
                                Rectangle {
                                    anchors.bottom: parent.bottom
                                    width: parent.width
                                    height: parent.height * Math.min(1, Math.max(0, Number(modelData) / 100))
                                    radius: 4
                                    color: Number(modelData) > 80 ? theme.c.danger : theme.c.accent
                                    Behavior on height { NumberAnimation { duration: 450; easing.type: Easing.OutCubic } }
                                }
                            }
                        }
                    }
                }
                // Windows: çekirdek düzeyi veri yok — dürüst tek bar
                Row {
                    visible: sys.cpuCores.length === 0
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 10
                    Text { text: "CPU"; font.pixelSize: 12; font.weight: Font.DemiBold; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                    Rectangle {
                        width: parent.width - 160
                        height: 8
                        radius: 4
                        anchors.verticalCenter: parent.verticalCenter
                        color: theme.c.edge
                        Rectangle {
                            width: parent.width * Math.min(1, Math.max(0, sys.cpuPct / 100))
                            height: parent.height
                            radius: 4
                            color: sys.cpuPct > 80 ? theme.c.danger : theme.c.accent
                            Behavior on width { NumberAnimation { duration: 450; easing.type: Easing.OutCubic } }
                        }
                    }
                    Text { text: sys.cpuPct.toFixed(0) + "%"; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.text; anchors.verticalCenter: parent.verticalCenter }
                    Text { text: "(çekirdek düzeyi yok — toplam)"; font.pixelSize: 10; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                }
            }
            Row {
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 10
                Text { text: "SİSTEM ÖZELLİKLERİ"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                Rectangle { width: parent.width - 220; height: 1; anchors.verticalCenter: parent.verticalCenter; color: theme.c.edge; opacity: 0.6 }
            }
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: 150
                enterDelay: 160
                Grid {
                    anchors.fill: parent
                    anchors.margins: 16
                    columns: 2
                    rowSpacing: 8
                    columnSpacing: 18
                    Text { text: "İşletim sistemi"; font.pixelSize: 12; color: theme.c.muted; width: 150 }
                    Text { text: sys.osName; font.pixelSize: 13; font.weight: Font.DemiBold; color: theme.c.text; elide: Text.ElideRight; width: parent.width - 200 }
                    Text { text: "Çekirdek"; font.pixelSize: 12; color: theme.c.muted; width: 150 }
                    Text { text: sys.kernel; font.pixelSize: 13; color: theme.c.text }
                    Text { text: "İşlemci"; font.pixelSize: 12; color: theme.c.muted; width: 150 }
                    Text { text: sys.cpuModel; font.pixelSize: 13; color: theme.c.text; elide: Text.ElideRight; width: parent.width - 200 }
                    Text { text: "Makine"; font.pixelSize: 12; color: theme.c.muted; width: 150 }
                    Text { text: sys.hostName; font.pixelSize: 13; color: theme.c.text }
                }
            }
            Row {
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 10
                Text { text: "PAKET YÖNETİCİLERİ · " + root.mgrStatus; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                Rectangle { width: parent.width - 300; height: 1; anchors.verticalCenter: parent.verticalCenter; color: theme.c.edge; opacity: 0.6 }
            }
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: Math.max(80, root.managers.length * 32 + 32)
                enterDelay: 200
                Column {
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 4
                    Repeater {
                        model: root.managers
                        delegate: Row {
                            width: parent.width
                            height: 28
                            spacing: 10
                            Rectangle {
                                width: 22; height: 22; radius: 11
                                anchors.verticalCenter: parent.verticalCenter
                                color: modelData.found ? theme.c.success : Qt.rgba(theme.c.edge.r, theme.c.edge.g, theme.c.edge.b, 0.6)
                                Text { anchors.centerIn: parent; text: modelData.found ? "✓" : "–"; font.pixelSize: 12; font.weight: Font.Bold; color: modelData.found ? "#0a0f0a" : theme.c.muted }
                            }
                            Text { text: modelData.label; font.pixelSize: 13; color: modelData.found ? theme.c.text : theme.c.muted; anchors.verticalCenter: parent.verticalCenter; width: parent.width - 140 }
                            Text { text: modelData.found ? "kurulu" : "yok"; font.pixelSize: 12; color: modelData.found ? theme.c.success : theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                        }
                    }
                }
            }
            Item { width: 1; height: 22 }
        }
    }
}
