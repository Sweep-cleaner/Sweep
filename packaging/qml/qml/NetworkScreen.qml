import QtQuick
// Ağ temizliği: DNS çözümleyici önbelleği ve paket yöneticisi önbellekleri.
//
// `sweep.networkFlush()` DNS'i boşaltır (geri dönüşsüz silme yok).
// `sweep.networkClean(dryRun)` npm/pip/yarn/cargo/gem önbelleklerini, git
// depolarındaki `git gc --auto` ve `docker network prune` işlerini yürütür.
// Önce "Deneme" ile ne olacağı görülür, sonra uygulanır.
Item {
    id: root
    property var theme
    property var manager
    // `id` is the tool identifier, `label` is either an i18n key (prefixed
    // "g.") or a product name shown verbatim — keeping them separate stops the
    // two from being confused by key-coverage tooling.
    property var targets: [
        { id: "dns", label: "g.flush_dns", on: true },
        { id: "npm", label: "npm", on: true },
        { id: "pip", label: "pip", on: true },
        { id: "yarn", label: "yarn", on: true },
        { id: "cargo", label: "cargo", on: true },
        { id: "gem", label: "gem", on: true },
        { id: "git", label: "git gc", on: true },
        { id: "docker", label: "docker", on: true }
    ]
    property string status: ""
    property string lastFreed: ""
    property bool busy: false
    signal navigate(string name)

    function human(n) {
        var u = ["B", "KB", "MB", "GB", "TB"], i = 0;
        var v = Number(n);
        while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
        return v.toFixed(1) + " " + u[i];
    }
    function summarise(json, isPreview) {
        try {
            var r = JSON.parse(json);
            if (!r || !Array.isArray(r.entries)) throw new Error("bad report");
            var errs = r.failures || [];
            var freed = Number(r.bytes_affected !== undefined ? r.bytes_affected : r.reclaimed_bytes || 0);
            root.lastFreed = isPreview ? "" : root.human(freed);
            var first = r.entries.length > 0 ? String(r.entries[0].label) : "";
            root.status = (isPreview ? i18n.tr("g.dry_run") + ": " : "") + first.slice(0, 160);
            if (Array.isArray(errs) && errs.length > 0) {
                root.status = String(errs[0].message || "failed").slice(0, 160);
            }
        } catch (e) {
            root.status = String(json).slice(0, 160);
        }
    }
    function flushDns() {
        if (root.busy) return;
        root.busy = true;
        root.status = i18n.tr("g.working");
        var json = sweep.networkFlush();
        root.busy = false;
        root.summarise(json, false);
    }
    function clean(dryRun) {
        if (root.busy) return;
        root.busy = true;
        root.status = i18n.tr("g.working");
        var json = sweep.networkClean(dryRun);
        root.busy = false;
        root.summarise(json, dryRun);
    }

    Column {
        anchors.fill: parent
        anchors.margins: 22
        spacing: 16
        Column {
            width: parent.width
            spacing: 2
            I18nText { key: "t.network"; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.muted }
            I18nText { key: "s.network"; font.pixelSize: 20; font.weight: Font.Black; color: theme.c.text }
            Text {
                text: root.status
                font.pixelSize: 12; color: theme.c.muted
                width: parent.width; elide: Text.ElideRight
            }
        }
        Row {
            width: parent.width
            spacing: 14
            GlassCard {
                theme: root.theme; manager: root.manager
                width: (parent.width - 14) / 2
                height: 190
                enterDelay: 0
                Column {
                    anchors.fill: parent
                    anchors.margins: 18
                    spacing: 10
                    I18nText { key: "g.flush_dns"; font.pixelSize: 15; font.weight: Font.Bold; color: theme.c.text }
                    Text {
                        text: "resolvectl / ipconfig /flushdns / dscacheutil"
                        font.pixelSize: 11; color: theme.c.muted
                        width: parent.width; wrapMode: Text.WordWrap
                    }
                    NeonButton {
                        theme: root.theme; manager: root.manager
                        label: i18n.tr("g.flush_dns"); primary: true; width: 180
                        busy: root.busy
                        onClicked: root.flushDns()
                    }
                }
            }
            GlassCard {
                theme: root.theme; manager: root.manager
                width: (parent.width - 14) / 2
                height: 190
                enterDelay: 70
                Column {
                    anchors.fill: parent
                    anchors.margins: 18
                    spacing: 10
                    I18nText { key: "g.clean_caches"; font.pixelSize: 15; font.weight: Font.Bold; color: theme.c.text }
                    Text {
                        text: "npm · pip · yarn · cargo · gem · git gc · docker"
                        font.pixelSize: 11; color: theme.c.muted
                        width: parent.width; wrapMode: Text.WordWrap
                    }
                    Row {
                        spacing: 10
                        NeonButton {
                            theme: root.theme; manager: root.manager
                            label: i18n.tr("g.dry_run"); width: 130
                            onClicked: root.clean(true)
                        }
                        NeonButton {
                            theme: root.theme; manager: root.manager
                            label: i18n.tr("g.clean_caches"); primary: true; width: 170
                            busy: root.busy
                            onClicked: root.clean(false)
                        }
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
            width: parent.width
            height: parent.height - 300
            enterDelay: 140
            Grid {
                anchors.fill: parent
                anchors.margins: 14
                columns: 2
                columnSpacing: 20
                rowSpacing: 4
                Repeater {
                    model: root.targets
                    delegate: NeonCheckBox {
                        theme: root.theme; manager: root.manager
                        width: (parent.width - 20) / 2
                        text: modelData.label.indexOf("g.") === 0 ? i18n.tr(modelData.label) : modelData.label
                        checked: modelData.on
                    }
                }
            }
        }
    }
}
