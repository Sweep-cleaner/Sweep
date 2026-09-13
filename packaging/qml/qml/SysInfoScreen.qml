import QtQuick
// Sistem sağlığı: CPU / RAM / disk / sıcaklık kartları, SMART durumu ve uptime.
//
// `sweep.sysinfoDump()` tam künyeyi ({"report":…, "sysinfo":{…}}) döndürür;
// sağlık seviyesi yeşil/sarı/kırmızı olarak rozetlenir ve gerekçeler listelenir.
Item {
    id: root
    property var theme
    property var manager
    property var info: ({})
    property string status: ""
    property bool busy: false
    signal navigate(string name)

    function human(n) {
        var u = ["B", "KB", "MB", "GB", "TB"], i = 0;
        var v = Number(n);
        while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
        return v.toFixed(1) + " " + u[i];
    }
    function pct(part, whole) { return whole > 0 ? Math.round(part / whole * 100) : 0; }
    function load() {
        root.busy = true;
        var json = sweep.sysinfoDump();
        root.busy = false;
        try {
            var data = JSON.parse(json);
            root.info = (data && data.sysinfo) ? data.sysinfo : ({});
            root.status = "";
        } catch (e) {
            root.info = ({});
            root.status = String(json).slice(0, 160);
        }
    }
    function healthLevel() {
        var lvl = root.info && root.info.health ? root.info.health.level : "green";
        if (lvl === "red") return "danger";
        if (lvl === "yellow") return "warn";
        return "ok";
    }
    function healthLabel() {
        var lvl = root.info && root.info.health ? root.info.health.level : "green";
        if (lvl === "red") return i18n.tr("g.health_red");
        if (lvl === "yellow") return i18n.tr("g.health_yellow");
        return i18n.tr("g.health_green");
    }
    function uptime() {
        var s = Number(root.info.uptime_secs || 0);
        var d = Math.floor(s / 86400), h = Math.floor((s % 86400) / 3600), m = Math.floor((s % 3600) / 60);
        if (d > 0) return d + "d " + h + "h " + m + "m";
        if (h > 0) return h + "h " + m + "m";
        return m + "m";
    }
    function smartText() {
        var list = (root.info && root.info.smart) ? root.info.smart : [];
        if (list.length === 0) return "--";
        var worst = "ok";
        list.forEach(function(s) {
            if (s.status === "critical") worst = "danger";
            else if (s.status === "warning" && worst !== "danger") worst = "warn";
            else if (s.status === "unknown" && worst === "ok") worst = "warn";
        });
        return worst === "danger" ? i18n.tr("g.health_red")
             : worst === "warn" ? i18n.tr("g.health_yellow")
             : i18n.tr("g.smart");
    }
    Component.onCompleted: load()

    Flickable {
        anchors.fill: parent
        contentWidth: width
        contentHeight: col.implicitHeight + 44
        clip: true
        Column {
            id: col
            width: parent.width
            spacing: 16
            Row {
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 12
                Column {
                    width: parent.width - 260
                    spacing: 2
                    I18nText { key: "t.sysinfo"; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.muted }
                    I18nText { key: "s.sysinfo"; font.pixelSize: 20; font.weight: Font.Black; color: theme.c.text }
                    Text {
                        text: String(root.info.os || "") + " " + String(root.info.os_version || "")
                              + " · " + String(root.info.host || "")
                        font.pixelSize: 11; color: theme.c.muted
                        elide: Text.ElideRight; width: parent.width
                    }
                }
                NeonBadge {
                    theme: root.theme; manager: root.manager
                    level: root.healthLevel()
                    text: root.healthLabel()
                    anchors.verticalCenter: parent.verticalCenter
                }
                NeonButton {
                    theme: root.theme; manager: root.manager
                    label: i18n.tr("g.refresh"); width: 100
                    anchors.verticalCenter: parent.verticalCenter
                    busy: root.busy
                    onClicked: root.load()
                }
            }
            Row {
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 14
                GlassCard {
                    theme: root.theme; manager: root.manager
                    width: (parent.width - 28) / 3
                    height: 150
                    enterDelay: 0
                    Column {
                        anchors.fill: parent
                        anchors.margins: 16
                        spacing: 6
                        I18nText { key: "g.cpu"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted }
                        Text {
                            text: String(root.info.cpu ? root.info.cpu.model : "--")
                            font.pixelSize: 13; font.weight: Font.DemiBold; color: theme.c.text
                            width: parent.width; elide: Text.ElideRight
                        }
                        Text {
                            text: (root.info.cpu ? root.info.cpu.cores : 0) + " " + i18n.tr("g.cores")
                                  + " / " + (root.info.cpu ? root.info.cpu.threads : 0)
                                  + " · " + (root.info.cpu ? root.info.cpu.frequency_mhz : 0) + " MHz"
                            font.pixelSize: 12; color: theme.c.muted
                        }
                        I18nText { key: "g.uptime"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted }
                        Text { text: root.uptime(); font.pixelSize: 16; font.weight: Font.Bold; color: theme.c.text }
                    }
                }
                GlassCard {
                    theme: root.theme; manager: root.manager
                    width: (parent.width - 28) / 3
                    height: 150
                    enterDelay: 60
                    Column {
                        anchors.fill: parent
                        anchors.margins: 16
                        spacing: 6
                        I18nText { key: "g.memory"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted }
                        Text {
                            text: root.info.memory
                                  ? root.pct(root.info.memory.used, root.info.memory.total) + "%"
                                  : "--"
                            font.pixelSize: 26; font.weight: Font.Black; color: theme.c.text
                        }
                        Text {
                            text: root.info.memory
                                  ? root.human(root.info.memory.used) + " / " + root.human(root.info.memory.total)
                                  : "--"
                            font.pixelSize: 12; color: theme.c.muted
                        }
                        I18nText { key: "g.swap"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted }
                        Text {
                            text: root.info.memory
                                  ? root.human(root.info.memory.swap_used) + " / " + root.human(root.info.memory.swap_total)
                                  : "--"
                            font.pixelSize: 13; font.weight: Font.DemiBold; color: theme.c.text
                        }
                    }
                }
                GlassCard {
                    theme: root.theme; manager: root.manager
                    width: (parent.width - 28) / 3
                    height: 150
                    enterDelay: 120
                    Column {
                        anchors.fill: parent
                        anchors.margins: 16
                        spacing: 6
                        I18nText { key: "g.temperature"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted }
                        Text {
                            text: root.info.thermal_c !== undefined && root.info.thermal_c !== null
                                  ? Number(root.info.thermal_c).toFixed(1) + "°C"
                                  : "--"
                            font.pixelSize: 26; font.weight: Font.Black; color: theme.c.text
                        }
                        I18nText { key: "g.smart"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted }
                        NeonBadge {
                            theme: root.theme; manager: root.manager
                            level: root.healthLevel() === "danger" ? "danger" : "ok"
                            text: root.smartText()
                        }
                    }
                }
            }
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: notesCol.implicitHeight + 34
                enterDelay: 180
                Column {
                    id: notesCol
                    anchors.fill: parent
                    anchors.margins: 16
                    spacing: 6
                    I18nText { key: "g.smart"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted }
                    Repeater {
                        model: (root.info && root.info.health && root.info.health.notes) ? root.info.health.notes : []
                        delegate: Text {
                            text: "• " + String(modelData)
                            font.pixelSize: 13; color: theme.c.text
                            width: notesCol.width; wrapMode: Text.WordWrap
                        }
                    }
                    Text {
                        visible: root.status !== ""
                        text: root.status
                        font.pixelSize: 12; color: theme.c.muted
                        width: notesCol.width; wrapMode: Text.WordWrap
                    }
                }
            }
            Item { width: 1; height: 22 }
        }
    }
}
