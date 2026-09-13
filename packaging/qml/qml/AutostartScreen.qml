import QtQuick
// Başlangıç ve Zamanlanmış Görevler ekranı (tek birleşik liste).
//
// Veri iki CLI çağrısından gelir ve **bir kez** ayrıştırılır:
//   * `sweep.autostartList(json)` → `startup list` (başlangıç girdileri +
//     oturum açma/başlatma tetikleyicili görevler),
//   * `sweep.tasksList(json)`     → `tasks list` (tüm zamanlanmış görevler).
// İkisi id'ye göre birleştirilir (oturum açma görevleri iki listede de
// görünür). Filtre/sıralama tek bir JS geçişinde yapılır; hiçbir iş
// delegate içinde koşmaz, JSON delegate başına ayrıştırılmaz.
//
// Değiştirici işlemler (`enable|disable|remove|edit`) tek bir köprü
// çağrısıyla `startup <op> <id>` olarak koşar; geri alma `startup rollback`,
// denetim defteri `startup history` üzerinden okunur. QML hiçbir zaman ham
// komut satırı kurmaz — filtre ve eylem JSON olarak köprüye verilir.
Item {
    id: root
    property var theme
    property var manager
    signal navigate(string name)
    // Ana pencere toast kuyruğunu bu sinyalle besler (Main.qml bağlar).
    signal toast(string msg, string level)

    // ---- durum ---------------------------------------------------------
    property var items: []          // birleşik ham liste
    property var rows: []           // filtrelenmiş + sıralanmış görünüm
    property var selected: null
    property string selectedId: ""
    property var checked: ({})      // id -> true (toplu seçim)
    property int checkedCount: 0
    property string search: ""
    property string fKind: "all"
    property string fScope: "all"
    property string fCategory: "all"
    property string fRisk: "all"
    property string fState: "all"
    property string sortKey: "name"
    property int tab: 0             // 0 = liste, 1 = geçmiş
    property bool loading: false
    property bool historyBusy: false
    property var historyRows: []
    property string status: ""
    property string errorText: ""
    property bool confirmOpen: false
    property var pendingAction: null
    property bool ackCritical: false
    property bool editOpen: false
    property var editItem: null
    property string editText: ""
    property var elevRetry: null   // {rollback,action,id,command,last} ya da null

    // ---- filtre/sıralama seçenekleri -----------------------------------
    // `k` bir i18n sözlük anahtarıdır; etiketler `label()` ile çözülür.
    readonly property var kindChips: [
        { v: "all", k: "a.kind_all" },
        { v: "startup", k: "a.kind_startup" },
        { v: "task", k: "a.kind_task" }
    ]
    readonly property var scopeChips: [
        { v: "all", k: "a.all" },
        { v: "user", k: "a.scope_user" },
        { v: "system", k: "a.scope_system" }
    ]
    readonly property var categoryChips: [
        { v: "all", k: "a.all" },
        { v: "os", k: "a.category_os" },
        { v: "security", k: "a.category_security" },
        { v: "driver", k: "a.category_driver" },
        { v: "updater", k: "a.category_updater" },
        { v: "third-party", k: "a.category_third_party" },
        { v: "unknown", k: "a.category_unknown" }
    ]
    readonly property var riskChips: [
        { v: "all", k: "a.all" },
        { v: "low", k: "a.risk_low" },
        { v: "medium", k: "a.risk_medium" },
        { v: "high", k: "a.risk_high" },
        { v: "critical", k: "a.risk_critical" }
    ]
    readonly property var stateChips: [
        { v: "all", k: "a.all" },
        { v: "on", k: "a.state_on" },
        { v: "off", k: "a.state_off" }
    ]
    readonly property var sortChips: [
        { v: "name", k: "a.sort_name" },
        { v: "source", k: "a.sort_source" },
        { v: "impact", k: "a.sort_impact" }
    ]

    // ---- etiket çözümleyicileri ----------------------------------------
    function label(key) { return i18n.tr(key); }
    function tx(v) {
        if (v === undefined || v === null || v === "") return "—";
        return String(v);
    }
    function riskLabel(r) { return i18n.tr("a.risk_" + String(r || "low")); }
    function catLabel(c) { return i18n.tr("a.category_" + String(c || "unknown").replace(/-/g, "_")); }
    function triggerLabel(t) { return i18n.tr("a.trigger_" + String(t || "unknown")); }
    function stateLabel(on) { return i18n.tr(on ? "a.state_on" : "a.state_off"); }
    function impactLabel(it) {
        if (!it || !it.impact) return "";
        return i18n.tr("g.impact_" + String(it.impact.level || "low"));
    }
    function riskLevel(r) {
        if (r === "critical" || r === "high") return "danger";
        if (r === "medium") return "warn";
        return "ok";
    }
    function impactLevel(it) {
        if (!it || !it.impact) return "ok";
        if (it.impact.level === "high") return "danger";
        if (it.impact.level === "medium") return "warn";
        return "ok";
    }
    function capLabel(on) { return i18n.tr(on ? "a.yes" : "a.no"); }
    function resultLevel(r) {
        if (r === "ok") return "ok";
        if (r === "failed") return "danger";
        return "warn";
    }

    // ---- veri yükleme ---------------------------------------------------
    function filterJson() {
        return JSON.stringify({
            kind: root.fKind === "all" ? "" : root.fKind,
            scope: root.fScope === "all" ? "" : root.fScope,
            category: root.fCategory === "all" ? "" : root.fCategory,
            risk: root.fRisk === "all" ? "" : root.fRisk,
            state: root.fState === "on" ? "enabled" : (root.fState === "off" ? "disabled" : "all"),
            search: root.search,
            sort: root.sortKey
        });
    }
    function parseEnvelope(text, errs) {
        if (text === undefined || text === null || text === "") return [];
        try {
            var d = JSON.parse(text);
            if (d && d.report && d.report.errors && d.report.errors.length > 0) {
                for (var i = 0; i < d.report.errors.length; i++)
                    errs.push(String(d.report.errors[i].message));
            }
            return (d && Array.isArray(d.entries)) ? d.entries : [];
        } catch (e) {
            // CLI hatası düz metin döner ("error: …" / "timeout"): yutma, göster.
            errs.push(String(text).slice(0, 200));
            return [];
        }
    }
    function refresh() {
        // Senkron CLI çağrısı UI iş parçacığını meşgul eder; yükleme durumunun
        // en az bir kare görünmesi için çağrı bir sonraki kareye ertelenir.
        root.loading = true;
        root.errorText = "";
        root.status = i18n.tr("g.working");
        reloadTimer.restart();
    }
    function doReload() {
        var q = root.filterJson();
        var errs = [];
        var merged = [];
        if (root.fKind !== "task") {
            var a = root.parseEnvelope(sweep.autostartList(q), errs);
            for (var i = 0; i < a.length; i++) merged.push(a[i]);
        }
        if (root.fKind !== "startup") {
            var t = root.parseEnvelope(sweep.tasksList(q), errs);
            for (var j = 0; j < t.length; j++) merged.push(t[j]);
        }
        var seen = ({});
        var unique = [];
        for (var m = 0; m < merged.length; m++) {
            var it = merged[m];
            if (!it || !it.id || seen[it.id]) continue;
            seen[it.id] = true;
            unique.push(it);
        }
        root.items = unique;
        root.loading = false;
        root.errorText = errs.length > 0 ? errs.join(" · ") : "";
        root.applyFilter();
        if (root.selectedId !== "") {
            var still = null;
            for (var s = 0; s < unique.length; s++) {
                if (unique[s].id === root.selectedId) { still = unique[s]; break; }
            }
            root.selected = still;
            if (!still) root.selectedId = "";
        }
        root.status = root.rows.length + " / " + unique.length + " " + i18n.tr("a.status_items");
    }

    // ---- filtre + sıralama (tek geçiş) ---------------------------------
    function applyFilter() {
        var q = root.search.trim().toLowerCase();
        var out = [];
        for (var i = 0; i < root.items.length; i++) {
            var it = root.items[i];
            if (root.fKind !== "all" && it.kind !== root.fKind) continue;
            if (root.fScope !== "all" && it.scope !== root.fScope) continue;
            if (root.fCategory !== "all" && it.category !== root.fCategory) continue;
            if (root.fRisk !== "all" && it.risk !== root.fRisk) continue;
            if (root.fState === "on" && !it.enabled) continue;
            if (root.fState === "off" && it.enabled) continue;
            if (q !== "") {
                var hay = ((it.name || "") + " " + (it.command || "") + " " +
                           (it.location || "") + " " + (it.source || "")).toLowerCase();
                if (hay.indexOf(q) < 0) continue;
            }
            out.push(it);
        }
        var key = root.sortKey;
        out.sort(function(a, b) {
            if (key === "source") {
                var sa = String(a.source || ""), sb = String(b.source || "");
                if (sa !== sb) return sa < sb ? -1 : 1;
            } else if (key === "impact") {
                var ia = a.impact ? Number(a.impact.estimated_ms || 0) : -1;
                var ib = b.impact ? Number(b.impact.estimated_ms || 0) : -1;
                if (ia !== ib) return ib - ia;
            }
            var na = String(a.name || "").toLowerCase();
            var nb = String(b.name || "").toLowerCase();
            return na < nb ? -1 : (na > nb ? 1 : 0);
        });
        root.rows = out;
        root.checkedCount = root.countChecked();
    }

    // ---- seçim ----------------------------------------------------------
    function selectItem(it) { root.selected = it; root.selectedId = it ? it.id : ""; }
    function isChecked(id) { return root.checked[id] === true; }
    function countChecked() {
        var n = 0;
        for (var k in root.checked) { if (root.checked[k]) n++; }
        return n;
    }
    function toggleCheck(id, on) {
        var c = ({});
        for (var k in root.checked) c[k] = root.checked[k];
        c[id] = on;
        root.checked = c;
        root.checkedCount = root.countChecked();
    }
    function setAllChecked(on) {
        var c = ({});
        if (on) {
            for (var i = 0; i < root.rows.length; i++) c[root.rows[i].id] = true;
        }
        root.checked = c;
        root.checkedCount = root.countChecked();
    }
    function checkedIds() {
        var out = [];
        for (var k in root.checked) { if (root.checked[k]) out.push(k); }
        return out;
    }

    // ---- eylemler -------------------------------------------------------
    function readResult(json) {
        try {
            var d = JSON.parse(json);
            if (d && d.exported) return { ok: true, warn: false, msg: String(d.exported.path || "") };
            if (d && d.action) {
                var r = String(d.action.result || "");
                return {
                    ok: r === "ok",
                    warn: r === "unverified",
                    msg: String(d.action.op || "") + " " + String(d.action.name || "") + " — " + r
                };
            }
            return { ok: true, warn: false, msg: String(json).slice(0, 160) };
        } catch (e) {
            return { ok: false, warn: false, msg: String(json).slice(0, 200) };
        }
    }
    // Sistem girdilerinde yetkisiz islem "Access is denied" ile doner
    // (ScheduleScreen ile ayni desen): ham sonucu koru, cozumu ekle.
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
    function doAction(action, id, command) {
        var payload = JSON.stringify({ action: action, id: id, command: command || "" });
        return root.readResult(sweep.autostartAct(payload));
    }
    function rollbackNow(id, last) {
        return root.readResult(sweep.autostartRollback(id, last));
    }
    function finishOne(res, name, retry) {
        if (res.ok) { root.elevRetry = null; root.toast(name + " ✓", "ok"); }
        else if (res.warn) { root.elevRetry = null; root.toast(name + " ⚠", "warn"); root.errorText = res.msg; }
        else {
            root.toast(name + " ✕", "bad");
            root.errorText = root.privilegeHint(res.msg);
            root.elevRetry = (retry && root.elevDenied(res.msg) && sweep.needsElevated()) ? retry : null;
        }
        root.refresh();
    }
    function runBulk(action) {
        var ids = root.checkedIds();
        var ok = 0, fail = 0, lastErr = "";
        for (var i = 0; i < ids.length; i++) {
            var r = root.doAction(action, ids[i], "");
            if (r.ok) ok++;
            else { fail++; lastErr = r.msg; }
        }
        root.setAllChecked(false);
        if (fail > 0) { root.errorText = root.privilegeHint(lastErr); root.toast(fail + " ✕", "bad"); }
        else root.toast(ok + " ✓", "ok");
        root.refresh();
    }
    // Onay penceresini tek yerden aç: risk onayı ve kutu durumu her seferinde
    // sıfırlanır (kutu örnekte kalıcıdır, diyalog yeniden yaratılmaz).
    function armConfirm(p) {
        root.pendingAction = p;
        root.ackCritical = false;
        ackBox.checked = false;
        root.confirmOpen = true;
    }
    function requestAction(it, action) {
        root.armConfirm({ kind: "one", action: action, id: it.id, name: it.name,
                          risk: it.risk, command: "", last: false });
    }
    function requestBulk(action) {
        root.armConfirm({ kind: "bulk", action: action, id: "",
                          name: root.checkedCount + " " + i18n.tr("a.selected"),
                          risk: "", command: "", last: false });
    }
    function requestRollback(id, name) {
        root.armConfirm({ kind: "rollback", action: "rollback", id: id, name: name,
                          risk: "", command: "", last: false });
    }
    function requestRollbackLast() {
        root.armConfirm({ kind: "rollback", action: "rollback", id: "",
                          name: i18n.tr("a.rollback_last"), risk: "", command: "", last: true });
    }
    // Tekil islem UAC ile tekrar kosulabilir (toplu islemde N tane UAC
    // cikmamasi icin bulk disinda tutulur).
    function elevArgsFor(r) {
        if (!r) return [];
        if (r.rollback) {
            var a = ["startup", "rollback"];
            if (r.id !== "") a.push(r.id);
            if (r.last) a.push("--last");
            return a;
        }
        var b = ["startup", r.action, r.id];
        if (r.action === "edit" && r.command !== "") { b.push("--command"); b.push(r.command); }
        return b;
    }
    function retryElevated() {
        if (!root.elevRetry || root.loading || sweep.busy()) return;
        root.loading = true;
        sweep.runElevated(root.elevArgsFor(root.elevRetry));
    }
    function confirmPending() {
        var p = root.pendingAction;
        if (!p) return;
        root.confirmOpen = false;
        root.pendingAction = null;
        if (p.kind === "bulk") {
            root.elevRetry = null;
            root.runBulk(p.action);
        } else if (p.kind === "rollback") {
            root.finishOne(root.rollbackNow(p.id, p.last), p.name, { rollback: true, id: p.id, last: p.last });
        } else {
            root.finishOne(root.doAction(p.action, p.id, p.command), p.name, { rollback: false, action: p.action, id: p.id, command: p.command });
        }
    }
    function openEdit(it) {
        if (!it || !it.editable) return;
        root.editItem = it;
        root.editText = it.command || "";
        root.editOpen = true;
    }
    function saveEdit() {
        root.editOpen = false;
        var it = root.editItem;
        if (!it) return;
        root.armConfirm({ kind: "one", action: "edit", id: it.id, name: it.name,
                          risk: it.risk, command: root.editText, last: false });
    }

    // ---- denetim geçmişi ------------------------------------------------
    function loadHistory() {
        root.historyBusy = true;
        var json = sweep.autostartHistory(100);
        try {
            var d = JSON.parse(json);
            var list = (d && Array.isArray(d.entries)) ? d.entries : [];
            list.reverse(); // defter eskiden yeniye; ekranda en yeni üstte
            root.historyRows = list;
            root.errorText = "";
        } catch (e) {
            root.historyRows = [];
            root.errorText = String(json).slice(0, 200);
        }
        root.historyBusy = false;
    }
    function exportHistory(fmt) {
        var stamp = new Date().toISOString().replace(/[:.]/g, "-");
        var name = "sweep-autostart-history-" + stamp + "." + fmt;
        var res = root.readResult(sweep.autostartExport(name, fmt));
        if (res.ok) root.toast(i18n.tr("a.exported") + ": " + res.msg, "ok");
        else { root.errorText = res.msg; root.toast(i18n.tr("a.error") + ": " + res.msg, "bad"); }
    }

    Timer {
        id: reloadTimer
        interval: 40
        repeat: false
        onTriggered: root.doReload()
    }
    Connections {
        target: sweep
        ignoreUnknownSignals: true
        function onStartupElevatedReady(json) {
            root.elevRetry = null;
            root.loading = false;
            var res = root.readResult(json);
            if (res.ok) root.toast(i18n.tr("g.elev_retry") + " ✓", "ok");
            else if (res.warn) { root.toast("⚠", "warn"); root.errorText = res.msg; }
            else { root.toast("✕", "bad"); root.errorText = res.msg; }
            root.refresh();
        }
        function onFailed(msg) { root.loading = false; }
    }
    Component.onCompleted: root.refresh()

    // ---- yerleşim -------------------------------------------------------
    Item {
        anchors.fill: parent
        anchors.margins: 22

        Row {
            id: header
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            height: 46
            spacing: 10
            Column {
                width: parent.width - 370
                spacing: 2
                I18nText { key: "a.eyebrow"; font.pixelSize: 11; font.weight: Font.Bold; font.letterSpacing: 0.8; color: theme.c.muted }
                I18nText { key: "s.autostart"; font.pixelSize: 18; font.weight: Font.Black; color: theme.c.text; elide: Text.ElideRight; width: parent.width }
            }
            NeonButton {
                theme: root.theme; manager: root.manager
                width: 100; label: i18n.tr("g.refresh")
                anchors.verticalCenter: parent.verticalCenter
                onClicked: root.refresh()
            }
            NeonButton {
                theme: root.theme; manager: root.manager
                width: 130; primary: root.tab === 1
                label: i18n.tr("a.tab_history")
                anchors.verticalCenter: parent.verticalCenter
                onClicked: { root.tab = 1; root.loadHistory(); }
            }
            NeonButton {
                theme: root.theme; manager: root.manager
                width: 90; primary: root.tab === 0
                label: i18n.tr("a.tab_list")
                anchors.verticalCenter: parent.verticalCenter
                onClicked: root.tab = 0
            }
        }

        Rectangle {
            id: errBanner
            anchors.top: header.bottom
            anchors.topMargin: root.errorText !== "" ? 8 : 0
            anchors.left: parent.left
            anchors.right: parent.right
            height: root.errorText !== "" ? (root.elevRetry !== null ? 52 : 34) : 0
            visible: root.errorText !== ""
            radius: 10
            color: Qt.rgba(theme.c.danger.r, theme.c.danger.g, theme.c.danger.b, 0.14)
            border.width: 1
            border.color: Qt.rgba(theme.c.danger.r, theme.c.danger.g, theme.c.danger.b, 0.5)
            Row {
                anchors.fill: parent
                anchors.margins: 8
                spacing: 8
                I18nText { key: "a.error"; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.danger; anchors.verticalCenter: parent.verticalCenter }
                Text {
                    text: root.errorText
                    font.pixelSize: 12; color: theme.c.text
                    elide: Text.ElideRight
                    width: root.elevRetry !== null ? parent.width - 250 : parent.width - 70
                    anchors.verticalCenter: parent.verticalCenter
                }
                NeonButton {
                    theme: root.theme; manager: root.manager
                    width: 170; label: i18n.tr("g.elev_retry")
                    visible: root.elevRetry !== null
                    anchors.verticalCenter: parent.verticalCenter
                    onClicked: root.retryElevated()
                }
            }
        }

        Text {
            id: statusLine
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            height: 16
            text: root.status
            font.pixelSize: 11
            color: theme.c.muted
            elide: Text.ElideRight
        }

        Item {
            id: body
            anchors.top: errBanner.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: statusLine.top
            anchors.bottomMargin: 6

            // ---------------- liste sekmesi ----------------
            Item {
                id: listPane
                anchors.fill: parent
                visible: root.tab === 0

                GlassCard {
                    id: filterCard
                    theme: root.theme; manager: root.manager
                    anchors.top: parent.top
                    anchors.left: parent.left
                    anchors.right: parent.right
                    height: 104
                    hoverable: false
                    enterDelay: 30
                    Column {
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 8
                        Row {
                            width: parent.width
                            height: 32
                            spacing: 8
                            Rectangle {
                                width: 200; height: 32; radius: 9
                                color: Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.8)
                                border.width: 1; border.color: theme.c.edge
                                TextInput {
                                    id: searchInput
                                    anchors.fill: parent
                                    anchors.margins: 7
                                    font.pixelSize: 12
                                    color: theme.c.text
                                    clip: true
                                    onTextChanged: { root.search = text; root.applyFilter(); }
                                }
                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    anchors.left: parent.left
                                    anchors.leftMargin: 7
                                    visible: root.search === ""
                                    text: i18n.tr("a.search")
                                    font.pixelSize: 12
                                    color: theme.c.muted
                                }
                            }
                            NeonButton {
                                theme: root.theme; manager: root.manager
                                width: 100; height: 30; label: i18n.tr("a.select_all")
                                onClicked: root.setAllChecked(true)
                            }
                            NeonButton {
                                theme: root.theme; manager: root.manager
                                width: 84; height: 30; label: i18n.tr("a.clear_selection")
                                onClicked: root.setAllChecked(false)
                            }
                            NeonButton {
                                theme: root.theme; manager: root.manager
                                width: 90; height: 30; primary: true
                                label: i18n.tr("g.open")
                                // NeonButton'ın iç MouseArea'si `enabled`i devralmaz:
                                // kilit onClicked içinde elle uygulanır.
                                opacity: root.checkedCount > 0 ? 1 : 0.45
                                onClicked: { if (root.checkedCount > 0) root.requestBulk("enable"); }
                            }
                            NeonButton {
                                theme: root.theme; manager: root.manager
                                width: 100; height: 30
                                label: i18n.tr("g.close")
                                opacity: root.checkedCount > 0 ? 1 : 0.45
                                onClicked: { if (root.checkedCount > 0) root.requestBulk("disable"); }
                            }
                            Text {
                                text: root.checkedCount > 0 ? (root.checkedCount + " " + i18n.tr("a.selected")) : ""
                                font.pixelSize: 11
                                color: theme.c.muted
                                anchors.verticalCenter: parent.verticalCenter
                            }
                        }
                        Row {
                            width: parent.width
                            height: 40
                            spacing: 14
                            // Yatay taşma: Sidebar'daki Flickable deseninin aynısı.
                            Flickable {
                                width: parent.width - 0
                                height: 40
                                contentWidth: chipRow.implicitWidth
                                contentHeight: height
                                clip: true
                                flickableDirection: Flickable.HorizontalFlick
                                Row {
                                    id: chipRow
                                    height: parent.height
                                    spacing: 16
                                    Row {
                                        spacing: 6
                                        I18nText { key: "a.kind"; font.pixelSize: 10; font.weight: Font.Bold; font.letterSpacing: 0.6; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                                        Repeater {
                                            model: root.kindChips
                                            delegate: Rectangle {
                                                width: chipLabelK.implicitWidth + 18; height: 24; radius: 12
                                                anchors.verticalCenter: parent.verticalCenter
                                                color: root.fKind === modelData.v ? Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.22) : Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.75)
                                                border.width: 1
                                                border.color: root.fKind === modelData.v ? theme.c.accent : theme.c.edge
                                                Text { id: chipLabelK; anchors.centerIn: parent; text: root.label(modelData.k); font.pixelSize: 11; color: root.fKind === modelData.v ? theme.c.text : theme.c.muted }
                                                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { root.fKind = modelData.v; root.applyFilter(); } }
                                            }
                                        }
                                    }
                                    Row {
                                        spacing: 6
                                        I18nText { key: "a.scope"; font.pixelSize: 10; font.weight: Font.Bold; font.letterSpacing: 0.6; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                                        Repeater {
                                            model: root.scopeChips
                                            delegate: Rectangle {
                                                width: chipLabelS.implicitWidth + 18; height: 24; radius: 12
                                                anchors.verticalCenter: parent.verticalCenter
                                                color: root.fScope === modelData.v ? Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.22) : Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.75)
                                                border.width: 1
                                                border.color: root.fScope === modelData.v ? theme.c.accent : theme.c.edge
                                                Text { id: chipLabelS; anchors.centerIn: parent; text: root.label(modelData.k); font.pixelSize: 11; color: root.fScope === modelData.v ? theme.c.text : theme.c.muted }
                                                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { root.fScope = modelData.v; root.applyFilter(); } }
                                            }
                                        }
                                    }
                                    Row {
                                        spacing: 6
                                        I18nText { key: "a.category"; font.pixelSize: 10; font.weight: Font.Bold; font.letterSpacing: 0.6; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                                        Repeater {
                                            model: root.categoryChips
                                            delegate: Rectangle {
                                                width: chipLabelC.implicitWidth + 18; height: 24; radius: 12
                                                anchors.verticalCenter: parent.verticalCenter
                                                color: root.fCategory === modelData.v ? Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.22) : Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.75)
                                                border.width: 1
                                                border.color: root.fCategory === modelData.v ? theme.c.accent : theme.c.edge
                                                Text { id: chipLabelC; anchors.centerIn: parent; text: root.label(modelData.k); font.pixelSize: 11; color: root.fCategory === modelData.v ? theme.c.text : theme.c.muted }
                                                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { root.fCategory = modelData.v; root.applyFilter(); } }
                                            }
                                        }
                                    }
                                    Row {
                                        spacing: 6
                                        I18nText { key: "a.risk"; font.pixelSize: 10; font.weight: Font.Bold; font.letterSpacing: 0.6; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                                        Repeater {
                                            model: root.riskChips
                                            delegate: Rectangle {
                                                width: chipLabelR.implicitWidth + 18; height: 24; radius: 12
                                                anchors.verticalCenter: parent.verticalCenter
                                                color: root.fRisk === modelData.v ? Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.22) : Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.75)
                                                border.width: 1
                                                border.color: root.fRisk === modelData.v ? theme.c.accent : theme.c.edge
                                                Text { id: chipLabelR; anchors.centerIn: parent; text: root.label(modelData.k); font.pixelSize: 11; color: root.fRisk === modelData.v ? theme.c.text : theme.c.muted }
                                                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { root.fRisk = modelData.v; root.applyFilter(); } }
                                            }
                                        }
                                    }
                                    Row {
                                        spacing: 6
                                        I18nText { key: "a.state"; font.pixelSize: 10; font.weight: Font.Bold; font.letterSpacing: 0.6; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                                        Repeater {
                                            model: root.stateChips
                                            delegate: Rectangle {
                                                width: chipLabelSt.implicitWidth + 18; height: 24; radius: 12
                                                anchors.verticalCenter: parent.verticalCenter
                                                color: root.fState === modelData.v ? Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.22) : Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.75)
                                                border.width: 1
                                                border.color: root.fState === modelData.v ? theme.c.accent : theme.c.edge
                                                Text { id: chipLabelSt; anchors.centerIn: parent; text: root.label(modelData.k); font.pixelSize: 11; color: root.fState === modelData.v ? theme.c.text : theme.c.muted }
                                                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { root.fState = modelData.v; root.applyFilter(); } }
                                            }
                                        }
                                    }
                                    Row {
                                        spacing: 6
                                        I18nText { key: "a.sort"; font.pixelSize: 10; font.weight: Font.Bold; font.letterSpacing: 0.6; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
                                        Repeater {
                                            model: root.sortChips
                                            delegate: Rectangle {
                                                width: chipLabelSo.implicitWidth + 18; height: 24; radius: 12
                                                anchors.verticalCenter: parent.verticalCenter
                                                color: root.sortKey === modelData.v ? Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.22) : Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.75)
                                                border.width: 1
                                                border.color: root.sortKey === modelData.v ? theme.c.accent : theme.c.edge
                                                Text { id: chipLabelSo; anchors.centerIn: parent; text: root.label(modelData.k); font.pixelSize: 11; color: root.sortKey === modelData.v ? theme.c.text : theme.c.muted }
                                                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: { root.sortKey = modelData.v; root.applyFilter(); } }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                Row {
                    anchors.top: filterCard.bottom
                    anchors.topMargin: 10
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    spacing: 12

                    GlassCard {
                        id: listCard
                        theme: root.theme; manager: root.manager
                        width: parent.width - 312
                        height: parent.height
                        hoverable: false
                        enterDelay: 60
                        ListView {
                            id: itemsList
                            anchors.fill: parent
                            anchors.margins: 10
                            clip: true
                            model: root.rows
                            spacing: 6
                            cacheBuffer: 800
                            reuseItems: true
                            delegate: Rectangle {
                                width: ListView.view.width
                                height: 64
                                radius: 11
                                color: modelData.id === root.selectedId
                                       ? Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.14)
                                       : Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.55)
                                border.width: 1
                                border.color: modelData.id === root.selectedId
                                              ? Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.55)
                                              : theme.c.edge
                                MouseArea {
                                    anchors.fill: parent
                                    onClicked: root.selectItem(modelData)
                                }
                                Row {
                                    anchors.fill: parent
                                    anchors.margins: 10
                                    spacing: 8
                                    NeonCheckBox {
                                        theme: root.theme; manager: root.manager
                                        // NeonCheckBox metin sütununu `width - 44`
                                        // yapar; metin boşken negatif genişlik
                                        // uyarısını önlemek için 44 px.
                                        width: 44; text: ""
                                        anchors.verticalCenter: parent.verticalCenter
                                        checked: root.isChecked(modelData.id)
                                        onToggled: root.toggleCheck(modelData.id, on)
                                    }
                                    Column {
                                        width: parent.width - 152
                                        spacing: 4
                                        anchors.verticalCenter: parent.verticalCenter
                                        Text {
                                            text: modelData.name
                                            font.pixelSize: 13; font.weight: Font.DemiBold
                                            color: theme.c.text
                                            elide: Text.ElideRight
                                            width: parent.width
                                        }
                                        Row {
                                            width: parent.width
                                            height: 22
                                            spacing: 6
                                            clip: true
                                            Rectangle {
                                                height: 20; radius: 10
                                                width: catText.implicitWidth + 16
                                                anchors.verticalCenter: parent.verticalCenter
                                                color: Qt.rgba(theme.c.edge.r, theme.c.edge.g, theme.c.edge.b, 0.5)
                                                border.width: 1; border.color: theme.c.edge
                                                Text { id: catText; anchors.centerIn: parent; text: root.catLabel(modelData.category); font.pixelSize: 10; color: theme.c.muted }
                                            }
                                            NeonBadge {
                                                theme: root.theme; manager: root.manager
                                                level: root.riskLevel(modelData.risk)
                                                text: root.riskLabel(modelData.risk)
                                                anchors.verticalCenter: parent.verticalCenter
                                            }
                                            NeonBadge {
                                                theme: root.theme; manager: root.manager
                                                level: modelData.enabled ? "ok" : "warn"
                                                text: root.stateLabel(modelData.enabled)
                                                anchors.verticalCenter: parent.verticalCenter
                                            }
                                            NeonBadge {
                                                visible: modelData.impact !== undefined
                                                theme: root.theme; manager: root.manager
                                                level: root.impactLevel(modelData)
                                                text: root.impactLabel(modelData)
                                                anchors.verticalCenter: parent.verticalCenter
                                            }
                                        }
                                    }
                                    Column {
                                        width: 92
                                        spacing: 2
                                        anchors.verticalCenter: parent.verticalCenter
                                        Text {
                                            text: modelData.scope === "system" ? i18n.tr("a.scope_system") : i18n.tr("a.scope_user")
                                            font.pixelSize: 10; color: theme.c.muted
                                            elide: Text.ElideRight; width: parent.width
                                            horizontalAlignment: Text.AlignRight
                                        }
                                        Text {
                                            // Kaynak her satırda; görevlerde tetikleyici de eklenir.
                                            text: String(modelData.source)
                                                  + (modelData.kind === "task" ? " · " + root.triggerLabel(modelData.trigger) : "")
                                            font.pixelSize: 10; color: theme.c.muted
                                            elide: Text.ElideRight; width: parent.width
                                            horizontalAlignment: Text.AlignRight
                                        }
                                        Text {
                                            text: root.tx(modelData.last_run)
                                            font.pixelSize: 10; color: theme.c.muted
                                            elide: Text.ElideRight; width: parent.width
                                            horizontalAlignment: Text.AlignRight
                                        }
                                    }
                                }
                            }
                        }
                        // yükleme durumu
                        Rectangle {
                            anchors.fill: parent
                            visible: root.loading
                            radius: parent.radius
                            color: Qt.rgba(theme.c.bg.r, theme.c.bg.g, theme.c.bg.b, 0.72)
                            Column {
                                anchors.centerIn: parent
                                spacing: 10
                                ProgressRing {
                                    theme: root.theme
                                    width: 62; height: 62
                                    value: 0.72
                                    thickness: 7
                                    anchors.horizontalCenter: parent.horizontalCenter
                                }
                                I18nText { key: "g.working"; font.pixelSize: 12; color: theme.c.muted; anchors.horizontalCenter: parent.horizontalCenter }
                            }
                        }
                        // boş durum
                        Column {
                            anchors.centerIn: parent
                            spacing: 8
                            visible: !root.loading && root.rows.length === 0 && root.errorText === ""
                            width: parent.width - 40
                            I18nText { key: "a.empty"; font.pixelSize: 15; font.weight: Font.DemiBold; color: theme.c.text; anchors.horizontalCenter: parent.horizontalCenter }
                            I18nText { key: "a.empty_hint"; font.pixelSize: 12; color: theme.c.muted; anchors.horizontalCenter: parent.horizontalCenter }
                        }
                    }

                    GlassCard {
                        id: detailCard
                        theme: root.theme; manager: root.manager
                        width: 300
                        height: parent.height
                        hoverable: false
                        enterDelay: 80
                        Flickable {
                            anchors.fill: parent
                            anchors.margins: 14
                            contentWidth: width
                            contentHeight: detailCol.implicitHeight
                            clip: true
                            Column {
                                id: detailCol
                                width: parent.width
                                spacing: 9
                                I18nText { key: "a.detail"; font.pixelSize: 10; font.weight: Font.Bold; font.letterSpacing: 0.8; color: theme.c.muted }
                                Text {
                                    visible: root.selected === null
                                    text: i18n.tr("a.no_selection")
                                    font.pixelSize: 12; color: theme.c.muted
                                    wrapMode: Text.WordWrap; width: parent.width
                                }
                                Text {
                                    visible: root.selected !== null
                                    text: root.selected ? root.selected.name : ""
                                    font.pixelSize: 15; font.weight: Font.DemiBold
                                    color: theme.c.text
                                    wrapMode: Text.WordWrap; width: parent.width
                                }
                                Row {
                                    visible: root.selected !== null
                                    spacing: 6
                                    NeonBadge {
                                        theme: root.theme; manager: root.manager
                                        level: root.selected ? root.riskLevel(root.selected.risk) : "ok"
                                        text: root.selected ? root.riskLabel(root.selected.risk) : ""
                                    }
                                    NeonBadge {
                                        theme: root.theme; manager: root.manager
                                        level: (root.selected && root.selected.enabled) ? "ok" : "warn"
                                        text: root.selected ? root.stateLabel(root.selected.enabled) : ""
                                    }
                                }
                                // alan çiftleri
                                Column {
                                    visible: root.selected !== null
                                    width: parent.width
                                    spacing: 7
                                    Text { width: parent.width; text: i18n.tr("a.command"); font.pixelSize: 10; font.weight: Font.Bold; color: theme.c.muted }
                                    Text { width: parent.width; text: root.selected ? root.tx(root.selected.command) : ""; font.pixelSize: 11; color: theme.c.text; wrapMode: Text.WrapAnywhere }
                                    Text { width: parent.width; text: i18n.tr("a.location"); font.pixelSize: 10; font.weight: Font.Bold; color: theme.c.muted }
                                    Text { width: parent.width; text: root.selected ? root.tx(root.selected.location) : ""; font.pixelSize: 11; color: theme.c.text; wrapMode: Text.WrapAnywhere }
                                    Text { width: parent.width; text: i18n.tr("a.trigger"); font.pixelSize: 10; font.weight: Font.Bold; color: theme.c.muted }
                                    Text { width: parent.width; text: root.selected ? (root.selected.kind === "task" ? root.triggerLabel(root.selected.trigger) : root.selected.source) : ""; font.pixelSize: 11; color: theme.c.text; wrapMode: Text.WordWrap }
                                    Text { width: parent.width; text: i18n.tr("a.author"); font.pixelSize: 10; font.weight: Font.Bold; color: theme.c.muted }
                                    Text { width: parent.width; text: root.selected ? root.tx(root.selected.author) : ""; font.pixelSize: 11; color: theme.c.text; wrapMode: Text.WordWrap }
                                    Text { width: parent.width; text: i18n.tr("a.last_run"); font.pixelSize: 10; font.weight: Font.Bold; color: theme.c.muted }
                                    Text { width: parent.width; text: root.selected ? (root.tx(root.selected.last_run) + " / " + root.tx(root.selected.last_result)) : ""; font.pixelSize: 11; color: theme.c.text; wrapMode: Text.WordWrap }
                                    Text { width: parent.width; text: i18n.tr("a.reason"); font.pixelSize: 10; font.weight: Font.Bold; color: theme.c.muted }
                                    Text { width: parent.width; text: root.selected && root.selected.impact ? root.tx(root.selected.impact.reason) : "—"; font.pixelSize: 11; color: theme.c.text; wrapMode: Text.WordWrap }
                                    Text { width: parent.width; text: i18n.tr("a.notes"); font.pixelSize: 10; font.weight: Font.Bold; color: theme.c.muted }
                                    Text { width: parent.width; text: root.selected ? root.tx(root.selected.notes) : ""; font.pixelSize: 11; color: theme.c.text; wrapMode: Text.WordWrap }
                                }
                                I18nText { visible: root.selected !== null; key: "a.capabilities"; font.pixelSize: 10; font.weight: Font.Bold; font.letterSpacing: 0.8; color: theme.c.muted }
                                Column {
                                    visible: root.selected !== null
                                    width: parent.width
                                    spacing: 3
                                    Row {
                                        spacing: 6
                                        I18nText { key: "a.reversible"; font.pixelSize: 11; color: theme.c.muted; width: 150 }
                                        Text { text: root.selected ? root.capLabel(root.selected.reversible) : ""; font.pixelSize: 11; font.weight: Font.DemiBold; color: theme.c.text }
                                    }
                                    Row {
                                        spacing: 6
                                        I18nText { key: "a.editable"; font.pixelSize: 11; color: theme.c.muted; width: 150 }
                                        Text { text: root.selected ? root.capLabel(root.selected.editable) : ""; font.pixelSize: 11; font.weight: Font.DemiBold; color: theme.c.text }
                                    }
                                    Row {
                                        spacing: 6
                                        I18nText { key: "a.removable"; font.pixelSize: 11; color: theme.c.muted; width: 150 }
                                        Text { text: root.selected ? root.capLabel(root.selected.removable) : ""; font.pixelSize: 11; font.weight: Font.DemiBold; color: theme.c.text }
                                    }
                                }
                                Flow {
                                    visible: root.selected !== null
                                    width: parent.width
                                    spacing: 7
                                    NeonButton {
                                        theme: root.theme; manager: root.manager
                                        width: 118; primary: root.selected ? !root.selected.enabled : false
                                        label: root.selected && root.selected.enabled ? i18n.tr("g.close") : i18n.tr("g.open")
                                        visible: root.selected ? root.selected.reversible : false
                                        onClicked: { if (root.selected) root.requestAction(root.selected, root.selected.enabled ? "disable" : "enable"); }
                                    }
                                    NeonButton {
                                        theme: root.theme; manager: root.manager
                                        width: 96
                                        label: i18n.tr("a.edit")
                                        visible: root.selected ? root.selected.editable : false
                                        onClicked: root.openEdit(root.selected)
                                    }
                                    NeonButton {
                                        theme: root.theme; manager: root.manager
                                        width: 96; danger: true
                                        label: i18n.tr("a.remove")
                                        visible: root.selected ? root.selected.removable : false
                                        onClicked: root.requestAction(root.selected, "remove")
                                    }
                                    NeonButton {
                                        theme: root.theme; manager: root.manager
                                        width: 110
                                        label: i18n.tr("a.rollback")
                                        visible: root.selected !== null
                                        onClicked: { if (root.selected) root.requestRollback(root.selected.id, root.selected.name); }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ---------------- geçmiş sekmesi ----------------
            Item {
                id: historyPane
                anchors.fill: parent
                visible: root.tab === 1
                GlassCard {
                    theme: root.theme; manager: root.manager
                    anchors.fill: parent
                    hoverable: false
                    enterDelay: 40
                    Column {
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 10
                        Row {
                            width: parent.width
                            height: 36
                            spacing: 8
                            I18nText { key: "a.tab_history"; font.pixelSize: 14; font.weight: Font.DemiBold; color: theme.c.text; anchors.verticalCenter: parent.verticalCenter }
                            Text {
                                text: root.historyRows.length + " " + i18n.tr("a.status_items")
                                font.pixelSize: 11; color: theme.c.muted
                                anchors.verticalCenter: parent.verticalCenter
                            }
                            NeonButton {
                                theme: root.theme; manager: root.manager
                                width: 120; height: 32; label: i18n.tr("a.export_json")
                                onClicked: root.exportHistory("json")
                            }
                            NeonButton {
                                theme: root.theme; manager: root.manager
                                width: 120; height: 32; label: i18n.tr("a.export_csv")
                                onClicked: root.exportHistory("csv")
                            }
                            NeonButton {
                                theme: root.theme; manager: root.manager
                                width: 170; height: 32; label: i18n.tr("a.rollback_last")
                                onClicked: root.requestRollbackLast()
                            }
                        }
                        ListView {
                            width: parent.width
                            height: parent.height - 46
                            clip: true
                            model: root.historyRows
                            spacing: 6
                            cacheBuffer: 600
                            reuseItems: true
                            delegate: Rectangle {
                                width: ListView.view.width
                                height: 58
                                radius: 10
                                color: Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.55)
                                border.width: 1; border.color: theme.c.edge
                                Row {
                                    anchors.fill: parent
                                    anchors.margins: 9
                                    spacing: 10
                                    Column {
                                        width: parent.width - 150
                                        spacing: 3
                                        anchors.verticalCenter: parent.verticalCenter
                                        Row {
                                            spacing: 8
                                            Text { text: modelData.op; font.pixelSize: 12; font.weight: Font.DemiBold; color: theme.c.text }
                                            Text { text: modelData.name; font.pixelSize: 12; color: theme.c.text; elide: Text.ElideRight; width: 220 }
                                            NeonBadge {
                                                theme: root.theme; manager: root.manager
                                                level: root.resultLevel(modelData.result)
                                                text: modelData.result
                                            }
                                        }
                                        Text {
                                            text: root.tx(modelData.ts) + "  " + root.tx(modelData.detail)
                                            font.pixelSize: 10; color: theme.c.muted
                                            elide: Text.ElideRight
                                            width: parent.width
                                        }
                                    }
                                    Column {
                                        width: 120
                                        spacing: 2
                                        anchors.verticalCenter: parent.verticalCenter
                                        Text { text: i18n.tr("a.backup"); font.pixelSize: 9; font.weight: Font.Bold; color: theme.c.muted }
                                        Text { text: root.tx(modelData.backup); font.pixelSize: 10; color: theme.c.text; elide: Text.ElideMiddle; width: parent.width }
                                    }
                                    NeonButton {
                                        theme: root.theme; manager: root.manager
                                        width: 90
                                        label: i18n.tr("a.rollback")
                                        visible: modelData.backup !== null && modelData.backup !== undefined
                                        anchors.verticalCenter: parent.verticalCenter
                                        onClicked: root.requestRollback(modelData.id, modelData.name)
                                    }
                                }
                            }
                        }
                        Text {
                            visible: !root.historyBusy && root.historyRows.length === 0
                            text: i18n.tr("a.history_empty")
                            font.pixelSize: 12; color: theme.c.muted
                            anchors.horizontalCenter: parent.horizontalCenter
                        }
                    }
                }
            }
        }
    }

    // ---------------- onay katmanı ----------------
    Item {
        anchors.fill: parent
        visible: root.confirmOpen
        z: 50
        Rectangle {
            anchors.fill: parent
            color: Qt.rgba(0, 0, 0, 0.45)
            MouseArea { anchors.fill: parent } // arkaya tıklama yutulur
        }
        Rectangle {
            width: 440
            height: confirmCol.implicitHeight + 36
            anchors.centerIn: parent
            radius: 16
            color: Qt.rgba(theme.c.panel.r, theme.c.panel.g, theme.c.panel.b, 0.98)
            border.width: 1
            border.color: (root.pendingAction && root.pendingAction.risk === "critical") ? theme.c.danger : theme.c.edge
            Column {
                id: confirmCol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 18
                spacing: 10
                I18nText { key: "a.confirm_title"; font.pixelSize: 15; font.weight: Font.Bold; color: theme.c.text }
                Text {
                    text: root.pendingAction ? root.pendingAction.name : ""
                    font.pixelSize: 13; font.weight: Font.DemiBold; color: theme.c.text
                    wrapMode: Text.WordWrap; width: parent.width
                }
                I18nText { key: "a.confirm_body"; font.pixelSize: 12; color: theme.c.muted; wrapMode: Text.WordWrap; width: parent.width }
                Rectangle {
                    width: parent.width
                    height: critCol.implicitHeight + 18
                    radius: 10
                    visible: root.pendingAction !== null && root.pendingAction.risk === "critical"
                    color: Qt.rgba(theme.c.danger.r, theme.c.danger.g, theme.c.danger.b, 0.16)
                    border.width: 1
                    border.color: Qt.rgba(theme.c.danger.r, theme.c.danger.g, theme.c.danger.b, 0.6)
                    Column {
                        id: critCol
                        anchors.left: parent.left
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.margins: 9
                        spacing: 6
                        I18nText { key: "a.confirm_critical"; font.pixelSize: 12; color: theme.c.danger; wrapMode: Text.WordWrap; width: parent.width }
                        NeonCheckBox {
                            id: ackBox
                            theme: root.theme; manager: root.manager
                            width: parent.width
                            text: i18n.tr("a.confirm_ack")
                            onToggled: root.ackCritical = on
                        }
                    }
                }
                Row {
                    anchors.right: parent.right
                    spacing: 8
                    NeonButton {
                        theme: root.theme; manager: root.manager
                        width: 100; label: i18n.tr("a.cancel")
                        onClicked: { root.confirmOpen = false; root.pendingAction = null; }
                    }
                    NeonButton {
                        theme: root.theme; manager: root.manager
                        width: 120
                        danger: true
                        primary: !(root.pendingAction && root.pendingAction.risk === "critical")
                        // Kritik öğede onay, kutuyu işaretleyene kadar kilitli.
                        // (NeonButton'ın iç MouseArea'si `enabled`i devralmaz.)
                        readonly property bool locked: (root.pendingAction !== null
                                                        && root.pendingAction.risk === "critical"
                                                        && !root.ackCritical)
                        opacity: locked ? 0.4 : 1
                        label: i18n.tr("a.confirm")
                        onClicked: { if (!locked) root.confirmPending(); }
                    }
                }
            }
        }
    }

    // ---------------- komut düzenleme katmanı ----------------
    Item {
        anchors.fill: parent
        visible: root.editOpen
        z: 60
        Rectangle {
            anchors.fill: parent
            color: Qt.rgba(0, 0, 0, 0.45)
            MouseArea { anchors.fill: parent }
        }
        Rectangle {
            width: 520
            height: editCol.implicitHeight + 36
            anchors.centerIn: parent
            radius: 16
            color: Qt.rgba(theme.c.panel.r, theme.c.panel.g, theme.c.panel.b, 0.98)
            border.width: 1
            border.color: theme.c.edge
            Column {
                id: editCol
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.margins: 18
                spacing: 10
                I18nText { key: "a.edit_title"; font.pixelSize: 15; font.weight: Font.Bold; color: theme.c.text }
                I18nText { key: "a.edit_label"; font.pixelSize: 11; color: theme.c.muted }
                Rectangle {
                    width: parent.width
                    height: 38
                    radius: 9
                    color: Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.9)
                    border.width: 1; border.color: theme.c.edge
                    TextInput {
                        id: editInput
                        anchors.fill: parent
                        anchors.margins: 8
                        font.pixelSize: 12
                        color: theme.c.text
                        clip: true
                        text: root.editText
                        onTextChanged: root.editText = text
                    }
                }
                Row {
                    anchors.right: parent.right
                    spacing: 8
                    NeonButton {
                        theme: root.theme; manager: root.manager
                        width: 100; label: i18n.tr("a.cancel")
                        onClicked: root.editOpen = false
                    }
                    NeonButton {
                        theme: root.theme; manager: root.manager
                        width: 120; primary: true
                        label: i18n.tr("a.save")
                        onClicked: root.saveEdit()
                    }
                }
            }
        }
    }
}
