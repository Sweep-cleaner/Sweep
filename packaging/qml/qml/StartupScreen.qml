import QtQuick
// Başlangıç yöneticisi: her girdi için etki rozeti (kırmızı/sarı/yeşil) ve
// geri alınabilir aç/kapat anahtarı.
//
// Girdiler `sweep.startupList(impact)` ile yapısal JSON olarak gelir
// ({"report":…, "entries":[{name, path, system, enabled, command, impact}]}).
// Kapatma hiçbir şeyi silmez: Windows'ta değer `sweep-disabled` alt anahtarına
// taşınır, Linux'ta `Hidden=true` yazılır, macOS'ta plist `.disabled` olur.
Item {
    id: root
    property var theme
    property var manager
    property var entries: []
    property bool showImpact: true
    property string filter: ""
    property string status: ""
    property bool busy: false
    signal navigate(string name)

    function load() {
        root.busy = true;
        var json = sweep.startupList(root.showImpact);
        root.busy = false;
        root.apply(json);
    }
    function apply(json) {
        try {
            var data = JSON.parse(json);
            var list = (data && Array.isArray(data.entries)) ? data.entries : [];
            root.entries = list;
            root.status = list.length + " / " + list.length;
        } catch (e) {
            root.status = String(json).slice(0, 160);
            root.entries = [];
        }
    }
    function visibleEntries() {
        var needle = root.filter.trim().toLowerCase();
        if (needle === "") return root.entries;
        return root.entries.filter(function(e) {
            var hay = ((e.name || "") + " " + (e.path || "") + " " + (e.command || "")).toLowerCase();
            return hay.indexOf(needle) >= 0;
        });
    }
    function levelOf(e) {
        if (!e || !e.impact) return "ok";
        if (e.impact.level === "high") return "danger";
        if (e.impact.level === "medium") return "warn";
        return "ok";
    }
    function impactLabel(e) {
        if (!e || !e.impact) return "";
        if (e.impact.level === "high") return i18n.tr("g.impact_high");
        if (e.impact.level === "medium") return i18n.tr("g.impact_medium");
        return i18n.tr("g.impact_low");
    }
    function toggle(e) {
        var json = sweep.startupSet(e.name, !e.enabled);
        root.status = String(json).slice(0, 160);
        root.load();
    }
    Component.onCompleted: load()

    Column {
        anchors.fill: parent
        anchors.margins: 22
        spacing: 14
        Row {
            width: parent.width
            spacing: 12
            Column {
                spacing: 2
                width: parent.width - 320
                I18nText { key: "t.startup_mgr"; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.muted }
                I18nText { key: "s.startup_mgr"; font.pixelSize: 20; font.weight: Font.Black; color: theme.c.text }
                Text { text: root.status; font.pixelSize: 11; color: theme.c.muted; elide: Text.ElideRight; width: parent.width }
            }
            Rectangle {
                width: 220; height: 34; radius: 10
                anchors.verticalCenter: parent.verticalCenter
                color: Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.8)
                border.width: 1; border.color: theme.c.edge
                TextInput {
                    anchors.fill: parent
                    anchors.margins: 8
                    font.pixelSize: 13
                    color: theme.c.text
                    clip: true
                    onTextChanged: root.filter = text
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.left: parent.left
                    anchors.leftMargin: 8
                    visible: root.filter === ""
                    text: i18n.tr("g.search_startup")
                    font.pixelSize: 13
                    color: theme.c.muted
                }
            }
            NeonButton {
                theme: root.theme; manager: root.manager
                label: i18n.tr("g.refresh"); width: 100
                anchors.verticalCenter: parent.verticalCenter
                onClicked: root.load()
            }
        }
        GlassCard {
            theme: root.theme; manager: root.manager
            width: parent.width
            height: parent.height - 62
            enterDelay: 40
            ListView {
                anchors.fill: parent
                anchors.margins: 10
                clip: true
                model: root.visibleEntries()
                spacing: 8
                delegate: Rectangle {
                    width: ListView.view.width
                    height: 62
                    radius: 11
                    color: Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.55)
                    border.width: 1
                    border.color: theme.c.edge
                    Row {
                        anchors.fill: parent
                        anchors.margins: 10
                        spacing: 12
                        Column {
                            width: parent.width - 260
                            spacing: 2
                            anchors.verticalCenter: parent.verticalCenter
                            Text {
                                text: modelData.name
                                font.pixelSize: 14; font.weight: Font.DemiBold
                                color: theme.c.text
                                elide: Text.ElideRight
                                width: parent.width
                            }
                            Text {
                                text: (modelData.system ? "system · " : "") + String(modelData.path)
                                font.pixelSize: 11; color: theme.c.muted
                                elide: Text.ElideMiddle
                                width: parent.width
                            }
                        }
                        NeonBadge {
                            visible: root.showImpact && modelData.impact !== undefined
                            theme: root.theme; manager: root.manager
                            level: root.levelOf(modelData)
                            text: root.impactLabel(modelData)
                            anchors.verticalCenter: parent.verticalCenter
                        }
                        NeonButton {
                            theme: root.theme; manager: root.manager
                            width: 110
                            primary: !modelData.enabled
                            label: modelData.enabled ? i18n.tr("g.close") : i18n.tr("g.open")
                            anchors.verticalCenter: parent.verticalCenter
                            onClicked: root.toggle(modelData)
                        }
                    }
                }
            }
        }
    }
}
