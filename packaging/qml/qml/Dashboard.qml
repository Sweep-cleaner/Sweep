import QtQuick
Item {
    id: root
    property var theme
    property var manager
    property string lastSummary: "Henüz tarama yapılmadı"
    property real lastBytes: 0
    property real lastFiles: 0
    property real lastErrors: 0
    property var donut: []
    property var cpuHist: []
    property var memHist: []
    property string hoverTip: ""
    property bool previewing: false
    property int langTick: 0
    signal navigate(string name)
    property real shownBytes: 0
    signal requestPreview
    Connections {
        target: i18n
        function onLangChanged() { root.langTick++; }
    }
    function previewAll() {
        root.previewing = true;
        sweep.preview(["--all"]);
    }
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
    function enterMs() { return (manager && manager.animMs !== undefined) ? manager.animMs : 220; }
    onLastBytesChanged: countAnim.start()
    NumberAnimation { id: countAnim; target: root; property: "shownBytes"; to: root.lastBytes; duration: 700; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] }
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
        function onFailed(msg) { root.lastSummary = "Hata: " + msg; root.previewing = false; }
        function onPreviewReady(json) {
            root.previewing = false;
            try {
                var r = JSON.parse(json);
                if (!r || !Array.isArray(r.entries)) throw new Error("bad report");
                var by = {}, i;
                // `--summary` sends exact per-option totals (`by_option`) and only
                // samples `entries`, so the donut must come from the totals.
                if (Array.isArray(r.by_option)) {
                    for (i = 0; i < r.by_option.length; i++) {
                        var o = r.by_option[i];
                        if (!o || typeof o.cleaner !== "string") continue;
                        by[o.cleaner + "." + (o.option || "")] = Number(o.bytes || 0);
                    }
                } else {
                    for (i = 0; i < r.entries.length; i++) {
                        var e = r.entries[i];
                        if (!e || typeof e.cleaner !== "string") continue;
                        var k = e.cleaner + "." + (e.option || "");
                        by[k] = (by[k] || 0) + Number(e.reclaimed || 0);
                    }
                }
                var arr = Object.keys(by).map(function(k) { return { label: k, bytes: by[k] }; });
                arr.sort(function(a, b) { return b.bytes - a.bytes; });
                root.donut = arr.slice(0, 6);
                // Exact even when `entries` is sampled, unlike summing the sample.
                root.lastBytes = Number(r.reclaimed_bytes || 0);
                root.lastFiles = Number(r.files_removed || 0);
                root.lastErrors = Number(r.errors || 0);
                root.lastSummary = arr.length + " kategori";
                donutChart.requestPaint();
                if (Number(r.files_removed || 0) > 0) celebrate.burst();
            } catch (err) { root.lastSummary = "Tarama başarısız"; }
        }
    }
    Flickable {
        anchors.fill: parent
        contentWidth: width
        contentHeight: col.implicitHeight + 44
        clip: true
        Column {
            id: col
            width: parent.width
            spacing: 16 // ---- hero ----
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: 158
                enterDelay: 0
                Row {
                    anchors.fill: parent
                    anchors.margins: 20
                    spacing: 20
                    ProgressRing {
                        theme: root.theme
                        width: 118; height: 118
                        anchors.verticalCenter: parent.verticalCenter
                        value: sys.diskTotal > 0 ? sys.diskUsed / sys.diskTotal : 0
                        text: sys.diskTotal > 0 ? Math.round(sys.diskUsed / sys.diskTotal * 100) + "%" : "--"
                        sub: root.langTick >= 0 ? i18n.tr("d.diskFull") : ""
                        thickness: 10
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 7
                        width: parent.width - 366
                        Row {
                            spacing: 8
                            Rectangle {
                                id: liveDot
                                width: 7; height: 7; radius: 4
                                anchors.verticalCenter: parent.verticalCenter
                                color: root.previewing ? theme.c.warning : theme.c.success
                                SequentialAnimation {
                                    running: true; loops: Animation.Infinite
                                    NumberAnimation { target: liveDot; property: "opacity"; from: 1; to: 0.35; duration: 1100; easing.type: Easing.InOutSine }
                                    NumberAnimation { target: liveDot; property: "opacity"; from: 0.35; to: 1; duration: 1100; easing.type: Easing.InOutSine }
                                }
                            }
                            I18nText { key: "d.reclaim"; font.pixelSize: 11; font.weight: Font.Bold; font.letterSpacing: 0.9; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                            Rectangle {
                                anchors.verticalCenter: parent.verticalCenter
                                width: statusText.width + 20; height: 19; radius: 9
                                color: Qt.rgba(theme.c.edge.r, theme.c.edge.g, theme.c.edge.b, 0.6)
                                border.width: 1
                                border.color: root.previewing ? Qt.rgba(theme.c.warning.r, theme.c.warning.g, theme.c.warning.b, 0.6) : theme.c.edge
                                Text {
                                    id: statusText
                                    anchors.centerIn: parent
                                    text: root.langTick >= 0 ? (root.previewing ? i18n.tr("d.scanning") : i18n.tr("d.ready")) : ""
                                    font.pixelSize: 9
                                    font.weight: Font.Bold
                                    font.letterSpacing: 0.7
                                    color: root.previewing ? theme.c.warning : theme.c.muted
                                }
                            }
                        }
                        Text { text: human(root.shownBytes); font.pixelSize: 34; font.weight: Font.DemiBold; color: theme.c.text }
                        Text {
                            text: root.langTick >= 0 ? (root.previewing ? i18n.tr("d.analyzing") : (root.lastSummary + "  ·  " + root.lastFiles + " dosya")) : ""
                            font.pixelSize: 12
                            color: theme.c.muted
                        }
                        Row {
                            spacing: 16
                            topPadding: 4
                            Repeater {
                                model: [
                                    { k: "KATEGORİ", v: root.donut.length },
                                    { k: "DOSYA", v: root.lastFiles },
                                    { k: "HATA", v: root.lastErrors }
                                ]
                                delegate: Row {
                                    spacing: 16
                                    Rectangle {
                                        visible: index > 0
                                        width: 1; height: 22
                                        anchors.verticalCenter: parent.verticalCenter
                                        color: theme.c.edge
                                    }
                                    Column {
                                        spacing: 0
                                        anchors.verticalCenter: parent.verticalCenter
                                        Text { text: modelData.k; font.pixelSize: 9; font.weight: Font.Bold; font.letterSpacing: 0.7; color: theme.c.muted }
                                        Text { text: String(modelData.v); font.pixelSize: 14; font.weight: Font.DemiBold; color: theme.c.text }
                                    }
                                }
                            }
                        }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 10
                        width: 210
                        NeonButton { theme: root.theme; manager: root.manager; label: root.langTick >= 0 ? (root.previewing ? i18n.tr("d.scanning2") : i18n.tr("d.scanAll")) : ""; primary: true; width: 210; busy: root.previewing; progress: root.previewing ? 0.4 : 0; onClicked: previewAll() }
                        NeonButton { theme: root.theme; manager: root.manager; label: root.langTick >= 0 ? i18n.tr("d.openCleaner") : ""; width: 210; onClicked: root.navigate("Cleaner") }
                    }
                }
            }
            // ---- quick actions: one card, three segments ----
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: 78
                enterDelay: 60
                Row {
                    anchors.fill: parent
                    Repeater {
                        model: [
                            { glyph: "▶", title: "Hızlı tara", sub: "Tümünü önizle", tkey: "d.quickScan", dkey: "d.quickScanSub", go: "preview" },
                            { glyph: "◈", title: "Temizleyici", sub: "Seç ve temizle", tkey: "d.cleaner", dkey: "d.cleanerSub", go: "Cleaner" },
                            { glyph: "◐", title: "Görünüm", sub: "Tema ve saydamlık", tkey: "d.appearance", dkey: "d.appearanceSub", go: "Settings" }
                        ]
                        delegate: Item {
                            width: parent.width / 3
                            height: parent.height
                            Row {
                                anchors.centerIn: parent
                                spacing: 11
                                Text {
                                    text: modelData.glyph
                                    font.pixelSize: 16
                                    color: segMouse.containsMouse ? theme.c.accent : theme.c.muted
                                    anchors.verticalCenter: parent.verticalCenter
                                    Behavior on color { ColorAnimation { duration: 150 } }
                                }
                                Column {
                                    anchors.verticalCenter: parent.verticalCenter
                                    spacing: 1
                                    Text { text: root.langTick >= 0 ? i18n.tr(modelData.tkey) : ""; font.pixelSize: 13; font.weight: Font.DemiBold; color: theme.c.text }
                                    Text { text: root.langTick >= 0 ? i18n.tr(modelData.dkey) : ""; font.pixelSize: 11; color: theme.c.muted }
                                }
                            }
                            Rectangle {
                                visible: index > 0
                                width: 1
                                height: parent.height - 28
                                anchors.left: parent.left
                                anchors.verticalCenter: parent.verticalCenter
                                color: theme.c.edge
                                opacity: 0.7
                            }
                            Rectangle {
                                anchors.bottom: parent.bottom
                                anchors.horizontalCenter: parent.horizontalCenter
                                width: parent.width * 0.45
                                height: 2
                                radius: 1
                                color: theme.c.accent
                                opacity: segMouse.containsMouse ? 1 : 0
                                Behavior on opacity { NumberAnimation { duration: 160 } }
                            }
                            MouseArea {
                                id: segMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                onClicked: { if (modelData.go === "preview") previewAll(); else root.navigate(modelData.go); }
                            }
                        }
                    }
                }
            }
            // ---- section: system ----
            Row {
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 12
                I18nText { key: "d.system"; font.pixelSize: 11; font.weight: Font.Bold; font.letterSpacing: 1.0; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                Rectangle { width: parent.width - 150; height: 1; anchors.verticalCenter: parent.verticalCenter; color: theme.c.edge; opacity: 0.6 }
                Row {
                    spacing: 6
                    anchors.verticalCenter: parent.verticalCenter
                    Rectangle { width: 6; height: 6; radius: 3; color: theme.c.success; anchors.verticalCenter: parent.verticalCenter }
                    I18nText { key: "d.live"; font.pixelSize: 10; font.weight: Font.Bold; font.letterSpacing: 0.8; color: theme.c.success; anchors.verticalCenter: parent.verticalCenter }
                }
            }
            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 14
                NeonCard {
                    theme: root.theme; manager: root.manager
                    width: (col.width - 44 - 28) / 3
                    height: 126
                    title: "CPU"
                    value: sys.cpuPct.toFixed(1) + " %"
                    spark: root.cpuHist
                }
                NeonCard {
                    theme: root.theme; manager: root.manager
                    width: (col.width - 44 - 28) / 3
                    height: 126
                    title: "RAM"
                    value: human(sys.memUsed) + " / " + human(sys.memTotal)
                    spark: root.memHist
                }
                NeonCard {
                    theme: root.theme; manager: root.manager
                    width: (col.width - 44 - 28) / 3
                    height: 126
                    title: "DİSK"
                    value: human(sys.diskUsed) + " / " + human(sys.diskTotal)
                    progress: sys.diskTotal > 0 ? sys.diskUsed / sys.diskTotal : 0
                }
            }
            // ---- section: charts ----
            Row {
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 12
                I18nText { key: "d.charts"; font.pixelSize: 11; font.weight: Font.Bold; font.letterSpacing: 1.0; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                Rectangle { width: parent.width - 120; height: 1; anchors.verticalCenter: parent.verticalCenter; color: theme.c.edge; opacity: 0.6 }
            }
            Row {
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 14
                GlassCard {
                    id: donutCard
                    theme: root.theme; manager: root.manager
                    width: (col.width - 44 - 14) / 2
                    height: 264
                    enterDelay: 120
                    I18nText { x: 18; y: 16; key: "d.distribution"; font.pixelSize: 13; font.weight: Font.DemiBold; color: theme.c.muted }
                    Text {
                        x: 18; y: 34
                        text: root.hoverTip
                        font.pixelSize: 12
                        color: theme.c.accent
                        opacity: root.hoverTip === "" ? 0 : 1
                        Behavior on opacity { NumberAnimation { duration: 150 } }
                    }
                    ChartComponents {
                        id: donutChart
                        anchors.fill: parent
                        anchors.margins: 14
                        anchors.topMargin: 52
                        mode: "donut"
                        segments: root.donut
                        chartPalette: theme.c.chart
                        lineColor: theme.c.accent
                        edgeColor: theme.c.edge
                        textColor: theme.c.text
                    }
                    // empty state, drawn beside the quiet ring track
                    Column {
                        visible: root.donut.length === 0
                        anchors.left: parent.left
                        anchors.leftMargin: 190
                        anchors.right: parent.right
                        anchors.rightMargin: 20
                        anchors.verticalCenter: parent.verticalCenter
                        anchors.verticalCenterOffset: 16
                        spacing: 8
                        I18nText { key: "d.noData"; font.pixelSize: 13; font.weight: Font.DemiBold; color: theme.c.text }
                        I18nText {
                            key: "d.noDataSub"
                            font.pixelSize: 11
                            color: theme.c.muted
                            wrapMode: Text.WordWrap
                            width: parent.width
                        }
                        NeonButton { theme: root.theme; manager: root.manager; label: root.langTick >= 0 ? i18n.tr("d.scanAll") : ""; width: 132; height: 32; onClicked: previewAll() }
                    }
                    // legend (only with data)
                    Column {
                        visible: root.donut.length > 0
                        x: 200
                        y: 60
                        width: parent.width - 220
                        spacing: 7
                        Repeater {
                            model: root.donut
                            delegate: Row {
                                spacing: 8
                                Rectangle { width: 8; height: 8; radius: 2; anchors.verticalCenter: parent.verticalCenter; color: theme.c.chart[index % theme.c.chart.length] }
                                Text {
                                    text: modelData.label
                                    font.pixelSize: 11
                                    color: theme.c.text
                                    width: parent.width - 100
                                    elide: Text.ElideRight
                                    anchors.verticalCenter: parent.verticalCenter
                                }
                                Text { text: human(modelData.bytes); font.pixelSize: 11; font.weight: Font.DemiBold; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                            }
                        }
                    }
                    MouseArea {
                        anchors.fill: parent
                        anchors.topMargin: 52
                        hoverEnabled: true
                        onPositionChanged: function(m) {
                            if (root.donut.length === 0) return;
                            var tot = 0, i;
                            for (i = 0; i < root.donut.length; i++) tot += Number(root.donut[i].bytes);
                            var frac = m.x / width;
                            var acc = 0;
                            for (i = 0; i < root.donut.length; i++) {
                                acc += Number(root.donut[i].bytes) / Math.max(1, tot);
                                if (frac <= acc) {
                                    donutChart.hoverIndex = i;
                                    donutChart.requestPaint();
                                    root.hoverTip = root.donut[i].label + "  " + human(root.donut[i].bytes);
                                    return;
                                }
                            }
                        }
                        onExited: { donutChart.hoverIndex = -1; donutChart.requestPaint(); root.hoverTip = ""; }
                    }
                }
                GlassCard {
                    theme: root.theme; manager: root.manager
                    width: (col.width - 44 - 14) / 2
                    height: 264
                    enterDelay: 180
                    Column {
                        anchors.fill: parent
                        anchors.margins: 18
                        spacing: 10
                        Row {
                            width: parent.width
                            I18nText { key: "d.wave"; font.pixelSize: 13; font.weight: Font.DemiBold; color: theme.c.muted }
                            Item { width: parent.width - 190; height: 1 }
                            Text { text: sys.cpuPct.toFixed(0) + "% CPU"; font.pixelSize: 12; font.weight: Font.DemiBold; color: theme.c.text }
                        }
                        ChartComponents {
                            id: waveChart
                            width: parent.width
                            height: 104
                            mode: "wave"
                            values: root.cpuHist
                            lineColor: theme.c.accent
                            edgeColor: theme.c.edge
                        }
                        Row {
                            width: parent.width
                            spacing: 10
                            Text { text: "I/O"; font.pixelSize: 11; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                            Rectangle {
                                width: parent.width - 118
                                height: 6
                                radius: 3
                                color: theme.c.edge
                                anchors.verticalCenter: parent.verticalCenter
                                Rectangle {
                                    id: ioBar
                                    property real target: 0
                                    width: Math.min(parent.width, target / 50000000 * parent.width)
                                    height: parent.height
                                    radius: 3
                                    color: theme.c.success
                                    Behavior on width { NumberAnimation { duration: 400; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
                                }
                            }
                            Text { text: human(ioBar.target) + "/s"; font.pixelSize: 11; color: theme.c.text; anchors.verticalCenter: parent.verticalCenter }
                        }
                        Rectangle { width: parent.width; height: 1; color: theme.c.edge; opacity: 0.5 }
                        Text { text: "RAM " + human(sys.memUsed) + " / " + human(sys.memTotal); font.pixelSize: 11; color: theme.c.muted }
                        Text {
                            text: (root.langTick >= 0 ? i18n.tr("d.lastReport") : "") + " " + root.lastSummary + "  ·  " + human(root.lastBytes) + "  ·  " + root.lastFiles + " dosya"
                            font.pixelSize: 11
                            color: theme.c.muted
                            elide: Text.ElideRight
                            width: parent.width
                        }
                    }
                }
            }
            Item { width: 1; height: 22 }
        }
    }
    // temizlik sonrası kutlama (her şeyin üstünde)
    Celebrate {
        id: celebrate
        theme: root.theme
        anchors.fill: parent
    }
}
