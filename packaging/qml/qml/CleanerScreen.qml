import QtQuick
import QtQml.Models
Item {
    id: root
    property var theme
    property var manager
    property var items: []
    property var managers: []
    property bool onlyInstalled: true
    property string filter: ""
    property string status: "Select cleaners, then Preview."
    property string result: ""
    property bool armed: false
    property bool working: false
    property real cleanProgress: 0
    property bool undoAvailable: false
    property bool previewDone: false
    property bool backupDialog: false
    property bool dangerAck: false
    // Ekrana girince listeyi cek: cagri yoksa liste hic dolmaz (bos ekran).
    Component.onCompleted: reload()
    function selectedCount() { return root.items.filter(function(x) { return x.checked; }).length; }
    function selectedBytes() { var t = 0, i; for (i = 0; i < root.items.length; i++) if (root.items[i].checked) t += Number(root.items[i].bytes || 0); return t; }
    function human(n) {
        var u = ["B", "KB", "MB", "GB", "TB"], i = 0;
        var v = Number(n);
        while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
        return v.toFixed(1) + " " + u[i];
    }
    function reload() {
        root.working = true;
        sweep.listCleaners();
    }
    function selected() {
        return root.items.filter(function(x) { return x.checked; }).map(function(x) { return x.key; });
    }
    function setAll(on) {
        var cp = root.items.slice();
        var i;
        for (i = 0; i < cp.length; i++) cp[i].checked = on;
        root.items = cp;
        groupModel.clear();
        buildGroups();
        markPreviewStale();
    }
    function buildGroups() {
        var groups = {}, i;
        var src = filtered();
        for (i = 0; i < src.length; i++) {
            var g = src[i].group;
            if (!groups[g]) groups[g] = [];
            groups[g].push(src[i]);
        }
        var keys = Object.keys(groups).sort();
        for (i = 0; i < keys.length; i++) {
            var arr = groups[keys[i]];
            var j;
            for (j = 0; j < arr.length; j++) groupModel.append({ group: keys[i], key: arr[j].key, label: arr[j].label, checked: arr[j].checked, warn: arr[j].warn, bytes: arr[j].bytes });
        }
    }
    function hiddenCleaners() {
        if (!root.onlyInstalled || root.managers.length === 0) return {};
        var h = {}, i;
        for (i = 0; i < root.managers.length; i++) {
            var m = root.managers[i];
            if (!m.found && m.cleaner) h[m.cleaner] = true;
        }
        return h;
    }
    function filtered() {
        var f = root.filter.toLowerCase();
        var h = hiddenCleaners();
        return root.items.filter(function(x) {
            var id = String(x.key).split(".")[0];
            if (h[id]) return false;
            return f === "" || x.label.toLowerCase().indexOf(f) >= 0;
        });
    }
    function preview() {
        var sel = selected();
        if (sel.length === 0) { root.status = "Nothing selected."; return; }
        root.working = true;
        root.cleanProgress = 0.15;
        sweep.preview(sel);
    }
    function hasWarn() { return root.items.some(function(x) { return x.checked && x.warn !== ""; }); }
    function hasDanger() { return root.items.some(function(x) { return x.checked && x.warn === "danger"; }); }
    function markPreviewStale() { root.previewDone = false; root.dangerAck = false; root.armed = false; }
    function clean() {
        var sel = selected();
        if (sel.length === 0) { root.status = "Nothing selected."; return; }
        // Risk kademesi: danger → yedek önerisi, warn → önce önizleme.
        if (hasDanger() && !root.dangerAck) { root.backupDialog = true; return; }
        if (hasWarn() && !root.previewDone) { root.status = i18n.tr("u.review_first"); preview(); return; }
        if (!root.armed) { root.armed = true; root.status = "Press Clean again to confirm."; return; }
        doClean(sel);
    }
    function doClean(sel) {
        root.armed = false;
        root.dangerAck = false;
        root.previewDone = false;
        root.undoAvailable = false;
        root.working = true;
        root.cleanProgress = 0.3;
        if (sweep.needsElevated()) {
            // Windows'ta yükseltilmiş yönetici gerekliyse ayrı bir UAC süreci
            // çalışır; sonuç normal cleanReady sinyaliyle döner.
            var args = ["clean", "--yes", "--summary", "--backup-dir", sweep.backupDir()].concat(sel);
            sweep.runElevated(args);
        } else {
            sweep.clean(sel);
        }
    }
    function undoLastClean() {
        root.working = true;
        sweep.undoLast();
    }
    Connections {
        target: sweep
        function onListReady(json) {
            try {
                var data = JSON.parse(json);
                if (data === null || typeof data !== "object" || Array.isArray(data)) throw new Error("bad list");
                var out = [];
                var pushOpt = function(id, c) {
                    if (!c || !Array.isArray(c.options)) return;
                    c.options.forEach(function(o) {
                        if (!o || typeof o.id !== "string") return;
                        var warn = (o.warning || "") !== "" ? "warn" : ((id === "system" || (o.id || "").indexOf("password") >= 0) ? "danger" : "");
                        out.push({ group: c.name || id, key: id + "." + o.id, label: o.label || o.id, checked: false, warn: warn, bytes: 0 });
                    });
                };
                Object.keys(data).sort().forEach(function(id) { pushOpt(id, data[id]); });
                root.items = out;
                groupModel.clear();
                buildGroups();
                root.status = out.length + " options loaded.";
            } catch (e) { root.status = "failed to load cleaners"; }
            root.working = false;
            sweep.diagnostics();
        }
        function onPreviewReady(json) {
            try {
                var r = JSON.parse(json);
                if (!r || !Array.isArray(r.entries)) throw new Error("bad report");
                var by = {}, i;
                // `--summary` reports exact per-option totals plus a sampled
                // entry list, so prefer `by_option`; summing entries keeps the
                // screen working against an engine that does not send it.
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
                var cp = root.items.slice();
                for (i = 0; i < cp.length; i++) cp[i].bytes = by[cp[i].key] || 0;
                root.items = cp;
                groupModel.clear();
                buildGroups();
                var lines = r.entries.slice(0, 80).map(function(x) { return ((x && (x.path || x.label)) || "") + " (" + human(Number((x && x.reclaimed) || 0)) + ")"; });
                root.result = lines.join("\n");
                root.status = Number(r.files_removed || 0) + " files  ·  " + human(Number(r.reclaimed_bytes || 0)) + "  ·  " + Number((r.errors && r.errors.length) || r.errors || 0) + " errors";
                root.previewDone = true;
            } catch (e) { root.status = "preview failed"; }
            root.working = false;
        }
        function onCleanReady(json) {
            try {
                var r = JSON.parse(json);
                if (!r || !Array.isArray(r.entries)) throw new Error("bad report");
                root.status = "cleaned: " + Number(r.files_removed || 0) + " files  ·  " + human(Number(r.reclaimed_bytes || 0));
                root.result = "";
                if (Number(r.files_removed || 0) > 0) {
                    root.undoAvailable = true;
                    root.status += "  ·  " + i18n.tr("u.undo_available");
                }
            } catch (e) { root.status = "clean failed"; }
            root.working = false;
        }
        function onUndoReady(json) {
            try {
                var r = JSON.parse(json);
                if (!r || !Array.isArray(r.entries)) throw new Error("bad report");
                var errs = (r.errors && r.errors.length) || r.errors || 0;
                if (Number(errs) > 0 && Array.isArray(r.failures) && r.failures.length > 0 && r.failures[0].message) {
                    root.status = String(r.failures[0].message).slice(0, 220);
                } else {
                    var n = r.entries.filter(function(x) { return x && x.option === "restore"; }).length;
                    root.status = "restored: " + n + " files";
                    if (Number(errs) === 0) root.undoAvailable = false;
                }
            } catch (e) { root.status = "undo failed"; }
            root.working = false;
        }
        function onProgressLine(line) { root.status = String(line).slice(0, 160); }
        function onFailed(msg) { root.status = String(msg).slice(0, 220); root.working = false; }
        function onDiagnosticsReady(json) {
            try {
                var r = JSON.parse(json);
                root.managers = (r && r.managers) || [];
                groupModel.clear();
                buildGroups();
            } catch (e) {}
        }
    }
    ListModel { id: groupModel }
    Connections {
        target: sweep
        function onProgressChanged() { root.cleanProgress = sweep.progress; }
    }
    Column {
        anchors.fill: parent
        anchors.margins: 22
        spacing: 12 // header
        Row {
            width: parent.width
            spacing: 12
            Column {
                spacing: 2
                width: parent.width - 380
                Text { text: "Cleaner"; font.pixelSize: 26; font.weight: Font.Bold; color: theme.c.text }
                Text { text: selectedCount() + " selected  ·  " + root.items.length + " options" + (selectedBytes() > 0 ? "  ·  ≈" + human(selectedBytes()) : ""); font.pixelSize: 12; color: theme.c.muted }
            }
            Rectangle {
                width: 360
                height: 42
                radius: 12
                anchors.verticalCenter: parent.verticalCenter
                color: Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.9)
                border.width: 1
                border.color: search.activeFocus ? theme.c.accent : theme.c.edge
                TextInput {
                    id: search
                    anchors.fill: parent
                    anchors.leftMargin: 14
                    anchors.rightMargin: 34
                    verticalAlignment: TextInput.AlignVCenter
                    font.pixelSize: 14
                    color: theme.c.text
                    onTextChanged: { root.filter = text; groupModel.clear(); buildGroups(); }
                    Text {
                        anchors.fill: parent
                        verticalAlignment: Text.AlignVCenter
                        text: "Search cleaners…"
                        font.pixelSize: 14
                        color: theme.c.muted
                        visible: !search.text && !search.activeFocus
                    }
                }
                Text {
                    x: 330; anchors.verticalCenter: parent.verticalCenter
                    text: "✕"; font.pixelSize: 14; color: theme.c.muted
                    visible: search.text !== ""
                    MouseArea { anchors.fill: parent; anchors.margins: -8; onClicked: search.text = "" }
                }
            }
        }
        Row {
            width: parent.width
            spacing: 10
            Text { text: "Sadece kurulu yöneticiler"; font.pixelSize: 12; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
            Rectangle {
                width: 44; height: 24; radius: 12
                anchors.verticalCenter: parent.verticalCenter
                color: root.onlyInstalled ? theme.c.accent : theme.c.edge
                Rectangle {
                    width: 18; height: 18; radius: 9
                    anchors.verticalCenter: parent.verticalCenter
                    x: root.onlyInstalled ? 23 : 3
                    color: theme.c.accentText
                    Behavior on x { NumberAnimation { duration: 140 } }
                }
                MouseArea {
                    anchors.fill: parent
                    onClicked: { root.onlyInstalled = !root.onlyInstalled; groupModel.clear(); buildGroups(); }
                }
            }
            Text { text: root.onlyInstalled ? "açık" : "kapalı"; font.pixelSize: 12; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
        }
        // busy bar
        Rectangle {
            width: parent.width
            height: root.working ? 6 : 0
            radius: 3
            visible: root.working
            color: theme.c.edge
            Behavior on height { NumberAnimation { duration: 160 } }
            Rectangle {
                id: busyScan
                width: 140; height: parent.height; radius: 3
                color: theme.c.accent
                SequentialAnimation {
                    running: root.working
                    loops: Animation.Infinite
                    NumberAnimation { target: busyScan; property: "x"; from: 0; to: Math.max(0, root.width - 400); duration: 1000; easing.type: Easing.InOutSine }
                    NumberAnimation { target: busyScan; property: "x"; from: Math.max(0, root.width - 400); to: 0; duration: 1000; easing.type: Easing.InOutSine }
                }
            }
        }
        // list card
        GlassCard {
            theme: root.theme; manager: root.manager
            width: parent.width
            height: parent.height - 330
            enterDelay: 60
            ListView {
                id: listView
                anchors.fill: parent
                anchors.margins: 10
                clip: true
                cacheBuffer: 1200
                reuseItems: true
                model: DelegateModel {
                    model: groupModel
                    delegate: Column {
                        width: listView.width - 20
                        Rectangle {
                            width: parent.width
                            height: 30
                            color: "transparent"
                            Text { x: 6; anchors.verticalCenter: parent.verticalCenter; text: group; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.accent }
                        }
                        Rectangle {
                            width: parent.width
                            height: 40
                            radius: 10
                            color: rowMouse.containsMouse ? Qt.rgba(theme.c.edge.r, theme.c.edge.g, theme.c.edge.b, 0.5) : (model.checked ? Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.10) : "transparent")
                            border.width: model.checked ? 1 : 0
                            border.color: Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.4)
                            Behavior on color { ColorAnimation { duration: 140 } }
                            Row {
                                anchors.fill: parent
                                anchors.leftMargin: 8
                                anchors.rightMargin: 8
                                spacing: 10
                                NeonCheckBox {
                                    theme: root.theme
                                    manager: root.manager
                                    width: parent.width - 230
                                    anchors.verticalCenter: parent.verticalCenter
                                    text: label
                                    checked: model.checked
                                    onToggled: function(on) {
                                        model.checked = on;
                                        var i;
                                        for (i = 0; i < root.items.length; i++) {
                                            if (root.items[i].key === model.key) { root.items[i].checked = on; break; }
                                        }
                                        markPreviewStale();
                                    }
                                }
                                Text {
                                    text: human(bytes)
                                    font.pixelSize: 12
                                    font.weight: Font.DemiBold
                                    color: Number(bytes) > 0 ? theme.c.success : theme.c.muted
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 90
                                    horizontalAlignment: Text.AlignRight
                                    visible: Number(bytes) > 0
                                }
                                Loader {
                                    anchors.verticalCenter: parent.verticalCenter
                                    active: warn !== ""
                                    sourceComponent: badgeComp
                                    property string bText: warn === "danger" ? "careful" : "warn"
                                    property string bLevel: warn
                                    onLoaded: { item.text = bText; item.level = bLevel; }
                                }
                            }
                            MouseArea {
                                id: rowMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                acceptedButtons: Qt.NoButton
                            }
                        }
                    }
                }
                section.property: "group"
                section.delegate: Rectangle {
                    width: listView.width - 20
                    height: 32
                    color: theme.c.panel2
                    border.width: 1
                    border.color: theme.c.edge
                    Text { anchors.verticalCenter: parent.verticalCenter; x: 12; text: section; font.pixelSize: 13; font.weight: Font.Bold; color: theme.c.accent }
                }
            }
            // empty state
            Column {
                anchors.centerIn: parent
                spacing: 8
                visible: groupModel.count === 0 && !root.working
                Text { anchors.horizontalCenter: parent.horizontalCenter; text: "○"; font.pixelSize: 34; color: theme.c.muted }
                Text { anchors.horizontalCenter: parent.horizontalCenter; text: root.filter === "" ? "No cleaners found." : "No match for \"" + root.filter + "\""; font.pixelSize: 14; color: theme.c.text }
                NeonButton {
                    anchors.horizontalCenter: parent.horizontalCenter
                    theme: root.theme; manager: root.manager; label: "Clear search"; width: 140
                    visible: root.filter !== ""
                    onClicked: search.text = ""
                }
            }
        }
        // floating action bar
        GlassCard {
            theme: root.theme; manager: root.manager
            width: parent.width
            height: 132
            enterDelay: 120
            Row {
                anchors.fill: parent
                anchors.margins: 14
                spacing: 10
                Column {
                    width: parent.width - 560
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 4
                    Text { text: root.status; font.pixelSize: 13; font.weight: Font.DemiBold; color: theme.c.text; elide: Text.ElideRight; width: parent.width }
                    Rectangle {
                        width: parent.width; height: 7; radius: 4
                        color: theme.c.edge
                        visible: root.working || root.cleanProgress > 0.01
                        Rectangle {
                            width: parent.width * Math.min(1, Math.max(0, root.cleanProgress))
                            height: parent.height; radius: 4
                            color: theme.c.accent
                            Behavior on width { NumberAnimation { duration: 300 } }
                        }
                    }
                    Text {
                        width: parent.width
                        height: 40
                        text: root.result
                        font.pixelSize: 11
                        font.family: "monospace"
                        color: theme.c.muted
                        elide: Text.ElideRight
                        clip: true
                    }
                }
                Column { width: 10; height: 1; spacing: 1 }
                Column {
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 8
                    Row {
                        spacing: 8
                        NeonButton { theme: root.theme; manager: root.manager; label: "Select all"; width: 110; onClicked: setAll(true) }
                        NeonButton { theme: root.theme; manager: root.manager; label: "None"; width: 90; onClicked: setAll(false) }
                        NeonButton { theme: root.theme; manager: root.manager; label: "Preview"; primary: true; width: 130; busy: root.working; progress: root.cleanProgress; onClicked: preview() }
                    }
                    Row {
                        spacing: 8
                        NeonButton { theme: root.theme; manager: root.manager; label: root.armed ? "Sure?" : "Clean"; danger: true; armed: root.armed; width: 130; busy: root.working; progress: root.cleanProgress; onClicked: clean() }
                        NeonButton { theme: root.theme; manager: root.manager; label: "Cancel"; width: 110; onClicked: sweep.cancel() }
                        NeonButton { theme: root.theme; manager: root.manager; label: "Reload"; width: 90; onClicked: reload() }
                        NeonButton { theme: root.theme; manager: root.manager; label: i18n.tr("u.undo"); width: 110; visible: root.undoAvailable && !root.working; onClicked: undoLastClean() }
                    }
                }
            }
        }
    }
    Component {
        id: badgeComp
        NeonBadge { theme: root.theme; manager: root.manager; text: bText; level: bLevel }
    }
    // Deep/System öncesi yedek önerisi: teknik ayrıntı yok, iki net seçenek.
    Rectangle {
        anchors.fill: parent
        visible: root.backupDialog
        color: "#80000000"
        z: 50
        MouseArea { anchors.fill: parent }
        GlassCard {
            theme: root.theme; manager: root.manager
            width: 440
            height: 250
            anchors.centerIn: parent
            Column {
                anchors.fill: parent
                anchors.margins: 22
                spacing: 12
                Text { text: i18n.tr("u.backup_title"); font.pixelSize: 18; font.weight: Font.Bold; color: theme.c.text; wrapMode: Text.WordWrap; width: parent.width }
                Text { text: i18n.tr("u.backup_body"); font.pixelSize: 13; color: theme.c.muted; wrapMode: Text.WordWrap; width: parent.width }
                Text { text: "Sweep cleans your system. You stay in control."; font.pixelSize: 12; font.italic: true; color: theme.c.muted; wrapMode: Text.WordWrap; width: parent.width }
                Row {
                    spacing: 10
                    NeonButton {
                        theme: root.theme; manager: root.manager; label: i18n.tr("u.create_backup"); primary: true; width: 190
                        onClicked: {
                            root.backupDialog = false;
                            if (!sweep.openSystemBackup()) root.status = i18n.tr("u.backup_open_failed");
                        }
                    }
                    NeonButton {
                        theme: root.theme; manager: root.manager; label: i18n.tr("u.continue"); width: 150
                        onClicked: { root.backupDialog = false; root.dangerAck = true; clean(); }
                    }
                }
            }
        }
    }
}
