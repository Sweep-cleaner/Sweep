import QtQuick
// Gizlilik kalkanı: telemetriyi kapatma ve iz temizliği.
//
// `sweep.privacyShield(dryRun)` sistem telemetrisini kapatır (Windows'ta kayıt
// defteri, Linux'ta whoopsie/apport/journal-upload, macOS'ta defaults).
// `sweep.privacyWipe(dryRun)` son kullanılan dosya kayıtlarını siler ve panoyu
// boşaltır. Her ikisi de önce "Deneme" ile gösterilebilir; hiçbir klasör
// özyinelemeli silinmez.
Item {
    id: root
    property var theme
    property var manager
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
            var freed = Number(r.bytes_affected !== undefined ? r.bytes_affected : r.reclaimed_bytes || 0);
            root.lastFreed = isPreview ? "" : root.human(freed);
            var lines = r.entries.map(function(e) { return String(e.label); });
            var head = lines.length > 0 ? lines[0] : "";
            root.status = (isPreview ? i18n.tr("g.dry_run") + ": " : "") + head.slice(0, 150);
            var errs = r.failures || [];
            if (Array.isArray(errs) && errs.length > 0) {
                root.status = String(errs[0].message || "failed").slice(0, 160);
            }
        } catch (e) {
            root.status = String(json).slice(0, 160);
        }
    }
    function run(kind, dryRun) {
        if (root.busy) return;
        root.busy = true;
        root.status = i18n.tr("g.working");
        var json = (kind === "shield") ? sweep.privacyShield(dryRun) : sweep.privacyWipe(dryRun);
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
            I18nText { key: "t.privacy"; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.muted }
            I18nText { key: "s.privacy"; font.pixelSize: 20; font.weight: Font.Black; color: theme.c.text }
            Text {
                text: root.status
                font.pixelSize: 12; color: theme.c.muted
                width: parent.width; elide: Text.ElideRight
            }
            Text {
                visible: root.lastFreed !== ""
                text: "✦ ~" + root.lastFreed + " " + i18n.tr("reclaimed")
                font.pixelSize: 13; font.weight: Font.Bold; color: theme.c.success
            }
        }
        Row {
            width: parent.width
            spacing: 14
            GlassCard {
                theme: root.theme; manager: root.manager
                width: (parent.width - 14) / 2
                height: 210
                enterDelay: 0
                Column {
                    anchors.fill: parent
                    anchors.margins: 18
                    spacing: 10
                    I18nText { key: "g.shield"; font.pixelSize: 15; font.weight: Font.Bold; color: theme.c.text }
                    Text {
                        text: "AllowTelemetry · whoopsie · apport · SubmitDiagInfo"
                        font.pixelSize: 11; color: theme.c.muted
                        width: parent.width; wrapMode: Text.WordWrap
                    }
                    Text {
                        text: i18n.tr("g.needs_root")
                        font.pixelSize: 11; color: theme.c.warning
                        width: parent.width; wrapMode: Text.WordWrap
                    }
                    Row {
                        spacing: 10
                        NeonButton {
                            theme: root.theme; manager: root.manager
                            label: i18n.tr("g.dry_run"); width: 130
                            onClicked: root.run("shield", true)
                        }
                        NeonButton {
                            theme: root.theme; manager: root.manager
                            label: i18n.tr("g.shield"); primary: true; width: 170
                            busy: root.busy
                            onClicked: root.run("shield", false)
                        }
                    }
                }
            }
            GlassCard {
                theme: root.theme; manager: root.manager
                width: (parent.width - 14) / 2
                height: 210
                enterDelay: 70
                Column {
                    anchors.fill: parent
                    anchors.margins: 18
                    spacing: 10
                    I18nText { key: "g.wipe"; font.pixelSize: 15; font.weight: Font.Bold; color: theme.c.text }
                    Row {
                        spacing: 18
                        Column {
                            spacing: 2
                            I18nText { key: "g.recent_files"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted }
                            Text { text: "recently-used.xbel · Recent · recentitems.plist"; font.pixelSize: 11; color: theme.c.muted }
                        }
                        Column {
                            spacing: 2
                            I18nText { key: "g.clipboard"; font.pixelSize: 11; font.weight: Font.Bold; color: theme.c.muted }
                            Text { text: "xclip · clip · pbcopy"; font.pixelSize: 11; color: theme.c.muted }
                        }
                    }
                    Row {
                        spacing: 10
                        NeonButton {
                            theme: root.theme; manager: root.manager
                            label: i18n.tr("g.dry_run"); width: 130
                            onClicked: root.run("wipe", true)
                        }
                        NeonButton {
                            theme: root.theme; manager: root.manager
                            label: i18n.tr("g.wipe"); danger: true; primary: true; width: 190
                            busy: root.busy
                            onClicked: root.run("wipe", false)
                        }
                    }
                }
            }
        }
        GlassCard {
            theme: root.theme; manager: root.manager
            width: parent.width
            height: parent.height - 300
            enterDelay: 140
            Column {
                anchors.fill: parent
                anchors.margins: 18
                spacing: 8
                I18nText { key: "s.privacy"; font.pixelSize: 14; font.weight: Font.Bold; color: theme.c.text }
                Text {
                    text: "• Linux: systemd journal iletimi, whoopsie, apport kapatılır.\n"
                          + "• Windows: AllowTelemetry=0, DisableWebSearch=1 (HKLM için yönetici).\n"
                          + "• macOS: defaults write ile diagnostik gönderimi kapatılır.\n"
                          + "• Hiçbir dosya kalıcı olarak silinmez; son kullanılanlar klasörü korunur."
                    font.pixelSize: 12; color: theme.c.muted
                    width: parent.width; wrapMode: Text.WordWrap
                }
            }
        }
    }
}
