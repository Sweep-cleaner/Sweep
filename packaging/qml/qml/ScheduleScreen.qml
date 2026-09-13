import QtQuick

Item {
    id: root
    property var theme
    property var manager
    property int langTick: 0
    property bool working: false
    property int hour: 3
    property string selection: ""
    property string status: ""
    property string memStatus: ""
    property string backend: ""
    property var elevArgs: []
    property string elevKind: ""
    signal navigate(string name)

    function tr(key) { return root.langTick >= 0 ? i18n.tr(key) : ""; }
    Connections {
        target: i18n
        function onLangChanged() { root.langTick++; }
    }
    Component.onCompleted: { root.backend = sweep.schedBackend(); refresh(); }
    function refresh() {
        if (root.working) return;
        root.working = true;
        root.status = sweep.schedStatus();
        root.memStatus = sweep.schedMemoptStatus();
        root.working = false;
    }
    // Windows'ta gorev kurmak/silmek yonetici ister; yetkisiz kosu
    // "Access is denied" ile doner. MemoryScreen'deki deseni izle: ham
    // metni koru, sonuna cozumu ekle (g.needs_root 8 dilde hazir).
    // Ayrica basarisiz isin argumanlari saklanir; "Yonetici olarak dene"
    // dugmesi ayni isi UAC ile kosar, sonuc schedElevatedReady ile doner.
    function elevDenied(msg) {
        var m = String(msg || "").toLowerCase();
        return m.indexOf("denied") >= 0 || m.indexOf("permission") >= 0
            || m.indexOf("erişim") >= 0 || m.indexOf("engellendi") >= 0
            || m.indexOf("0x80070005") >= 0;
    }
    function privilegeHint(msg) {
        if (root.elevDenied(msg) && sweep.needsElevated()) return String(msg) + " — " + i18n.tr("g.needs_root");
        return msg;
    }
    function armElev(out, kind, args) {
        if (root.elevDenied(out) && sweep.needsElevated()) { root.elevKind = kind; root.elevArgs = args; }
        else { root.elevKind = ""; root.elevArgs = []; }
        return root.privilegeHint(out);
    }
    function retryElevated() {
        if (root.elevArgs.length === 0 || root.working || sweep.busy()) return;
        root.working = true;
        sweep.runElevated(root.elevArgs);
    }
    function memApply() {
        if (root.working) return;
        root.working = true;
        var mh = Math.round(slider.value);
        root.memStatus = root.armElev(sweep.schedMemoptEnable(mh), "mem", ["schedule", "--enable", "--memopt", "--hour", String(mh)]);
        root.working = false;
    }
    function memRemove() {
        if (root.working) return;
        root.working = true;
        root.memStatus = root.armElev(sweep.schedMemoptDisable(), "mem", ["schedule", "--disable", "--memopt"]);
        root.working = false;
    }
    function apply() {
        if (root.working) return;
        root.working = true;
        var h = Math.round(slider.value);
        var parts = [];
        var raw = root.selection.split(/\s+/);
        for (var i = 0; i < raw.length; i++) if (raw[i] !== "") parts.push(raw[i]);
        root.status = root.armElev(sweep.schedEnable(h, root.selection), "sched", ["schedule", "--enable", "--hour", String(h)].concat(parts));
        root.working = false;
    }
    function remove() {
        if (root.working) return;
        root.working = true;
        root.status = root.armElev(sweep.schedDisable(), "sched", ["schedule", "--disable"]);
        root.working = false;
    }
    Connections {
        target: sweep
        ignoreUnknownSignals: true
        function onSchedElevatedReady(json) {
            var kind = root.elevKind;
            root.elevKind = "";
            root.elevArgs = [];
            root.working = false;
            var s = String(json);
            try { var d = JSON.parse(json); if (d && d.schedule) s = String(d.schedule); } catch (e) {}
            if (kind === "mem") root.memStatus = s.slice(0, 200);
            else root.status = s.slice(0, 200);
            root.refresh();
        }
        function onFailed(msg) { root.working = false; }
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
                height: 250
                enterDelay: 0
                Row {
                    anchors.fill: parent
                    anchors.margins: 20
                    spacing: 26
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 6
                        width: 260
                        Text { text: tr("sched.eyebrow"); font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.muted }
                        Text { text: tr("sched.title"); font.pixelSize: 24; font.weight: Font.Black; color: theme.c.text }
                        Text { text: tr("sched.sub"); font.pixelSize: 12; color: theme.c.muted }
                        Row {
                            spacing: 10
                            Text { text: tr("sched.backend") + ":"; font.pixelSize: 12; font.weight: Font.DemiBold; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                            Text { text: root.backend; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.accent; anchors.verticalCenter: parent.verticalCenter }
                        }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 10
                        width: 300
                        Text { text: tr("sched.hour") + ": " + String(Math.round(slider.value)).padStart(2, "0") + ":00"; font.pixelSize: 14; font.weight: Font.Bold; color: theme.c.text }
                        NeonSlider {
                            id: slider
                            theme: root.theme; manager: root.manager
                            from: 0; to: 23; value: root.hour
                            width: parent.width
                            onMoved: function(v) { root.hour = Math.round(v); }
                        }
                        Text { text: tr("sched.selection"); font.pixelSize: 12; font.weight: Font.DemiBold; color: theme.c.muted }
                        Rectangle {
                            width: parent.width; height: 36; radius: 8
                            color: Qt.rgba(theme.c.edge.r, theme.c.edge.g, theme.c.edge.b, 0.4)
                            border.width: 1; border.color: theme.c.edge
                            TextInput {
                                anchors.fill: parent
                                anchors.margins: 8
                                font.pixelSize: 13
                                color: theme.c.text
                                text: root.selection
                                onTextChanged: root.selection = text
                            }
                        }
                        Text { text: tr("sched.hint"); font.pixelSize: 11; color: theme.c.muted; wrapMode: Text.WordWrap; width: parent.width }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 6
                        width: parent.width - 640
                        Text { text: tr("sched.status").toUpperCase(); font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted }
                        Text { text: root.status; font.pixelSize: 13; color: theme.c.text; wrapMode: Text.WordWrap; width: parent.width }
                        Row {
                            spacing: 10
                            NeonButton { theme: root.theme; manager: root.manager; label: root.working ? tr("sched.working") : tr("sched.enable"); primary: true; width: 150; busy: root.working; onClicked: apply() }
                            NeonButton { theme: root.theme; manager: root.manager; label: tr("sched.disable"); width: 110; onClicked: remove() }
                            NeonButton { theme: root.theme; manager: root.manager; label: tr("sched.refresh"); width: 110; onClicked: refresh() }
                            NeonButton { theme: root.theme; manager: root.manager; label: i18n.tr("g.elev_retry"); width: 170; visible: root.elevKind === "sched"; onClicked: retryElevated() }
                        }
                    }
                }
            }
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: 150
                enterDelay: 60
                Row {
                    anchors.fill: parent
                    anchors.margins: 20
                    spacing: 26
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 6
                        width: 260
                        Text { text: tr("sched.memopt"); font.pixelSize: 14; font.weight: Font.Bold; color: theme.c.text }
                        Text { text: root.memStatus; font.pixelSize: 12; color: theme.c.muted; wrapMode: Text.WordWrap; width: parent.width }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 10
                        width: parent.width - 340
                        Row {
                            spacing: 10
                            NeonButton { theme: root.theme; manager: root.manager; label: root.working ? tr("sched.working") : tr("sched.enable"); primary: true; width: 150; busy: root.working; onClicked: memApply() }
                            NeonButton { theme: root.theme; manager: root.manager; label: tr("sched.disable"); width: 110; onClicked: memRemove() }
                            NeonButton { theme: root.theme; manager: root.manager; label: i18n.tr("g.elev_retry"); width: 170; visible: root.elevKind === "mem"; onClicked: retryElevated() }
                        }
                    }
                }
            }
            Item { width: 1; height: 22 }
        }
    }
}
