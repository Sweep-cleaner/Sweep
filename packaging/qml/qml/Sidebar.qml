import QtQuick
Rectangle {
    id: root
    property var theme
    property var manager
    property string screen: "Dashboard"
    property real uiOpacity: 1
    property bool pinned: true
    property int langTick: 0
    signal navigate(string name)
    // `name` is the route id used by navigate(); `title`/`desc` are the display
    // copy. Glyphs are monochrome geometric shapes, so they inherit the text
    // colour instead of pulling in a colour-emoji font.
    // `tkey`/`dkey` are i18n dictionary keys; `title`/`desc` remain as fallbacks.
    readonly property var menuItems: [
        { name: "Dashboard", title: "Panel", desc: "Sistem durumu", tkey: "t.dashboard", dkey: "s.dashboard", glyph: "▦" },
        { name: "Cleaner", title: "Temizleyici", desc: "Alan temizle", tkey: "t.cleaner", dkey: "s.cleaner", glyph: "◈" },
        { name: "Files", title: "Dosyalar", desc: "Büyük & kopya dosyalar", tkey: "t.files", dkey: "s.files", glyph: "▤" },
        { name: "Disk", title: "Disk", desc: "Klasör boyutları", tkey: "t.disk", dkey: "s.disk", glyph: "◍" }
    ]
    readonly property var sysItems: [
        { name: "History", title: "Geçmiş", desc: "Temizlik geçmişi", tkey: "t.history", dkey: "s.history", glyph: "◔" },
        { name: "System", title: "Sistem", desc: "Donanım nabzı", tkey: "t.system", dkey: "s.system", glyph: "▣" },
        { name: "Optimize", title: "Bellek", desc: "Bellek optimizasyonu", tkey: "t.memory", dkey: "s.memory", glyph: "◎" },
        { name: "Autostart", title: "Başlangıç ve Görevler", desc: "Otomatik başlayan uygulamalar", tkey: "t.autostart_nav", dkey: "s.autostart_nav", glyph: "⧗" },
        { name: "SysInfo", title: "Sistem Sağlığı", desc: "CPU, disk, SMART", tkey: "t.sysinfo", dkey: "s.sysinfo", glyph: "◫" },
        { name: "Network", title: "Ağ", desc: "DNS ve önbellekler", tkey: "t.network", dkey: "s.network", glyph: "⇄" },
        { name: "Privacy", title: "Gizlilik", desc: "Telemetri ve izler", tkey: "t.privacy", dkey: "s.privacy", glyph: "◉" },
        { name: "Schedule", title: "Zamanlama", desc: "Otomatik temizlik", tkey: "t.schedule", dkey: "s.schedule", glyph: "◷" },
        { name: "Settings", title: "Ayarlar", desc: "Tema & ayarlar", tkey: "t.settings", dkey: "s.settings", glyph: "⚙" }
    ]
    Connections {
        target: i18n
        function onLangChanged() { root.langTick++; }
    }
    function human(n) {
        var u = ["B", "KB", "MB", "GB", "TB"], i = 0;
        var v = Number(n);
        while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
        return v.toFixed(1) + " " + u[i];
    }
    width: 232
    color: theme ? Qt.rgba(theme.c.panel.r, theme.c.panel.g, theme.c.panel.b, 0.9) : "#101018"
    Rectangle { width: 1; height: parent.height; anchors.right: parent.right; color: theme.c.edge; opacity: 0.7 }
    // Girdi listesi kaydırılabilir: disk-dışı ekranlarla birlikte menü öğesi
    // sayısı 12'ye çıktı ve 660 px'lik en küçük pencere yüksekliğine sığmıyor.
    // Alt tema kartı için 84 px boşluk bırakılır.
    Flickable {
        anchors.fill: parent
        anchors.leftMargin: 14
        anchors.rightMargin: 14
        anchors.topMargin: 14
        anchors.bottomMargin: 84
        contentWidth: width
        contentHeight: navColumn.implicitHeight
        clip: true
        Column {
            id: navColumn
            width: parent.width
            spacing: 8 // logo
        Row {
            id: logoRow
            spacing: 11
            width: parent.width
            height: 46
            opacity: 0
            transform: Translate { id: logoSlide; x: -14 }
            Component.onCompleted: logoIn.start()
            SequentialAnimation {
                id: logoIn
                PauseAnimation { duration: 40 }
                ParallelAnimation {
                    NumberAnimation { target: logoSlide; property: "x"; to: 0; duration: 320; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] }
                    NumberAnimation { target: logoRow; property: "opacity"; to: 1; duration: 320 }
                }
            }
            Image {
                id: logoTile
                width: 40; height: 40
                anchors.verticalCenter: parent.verticalCenter
                // `sourceSize` decodes the 512 px master down to what a HiDPI
                // tile actually needs, so the sidebar never holds a large
                // texture in memory.
                source: "assets/sweep-logo.png"
                sourceSize.width: 80
                sourceSize.height: 80
                smooth: true
                mipmap: true
                fillMode: Image.PreserveAspectFit
                MouseArea {
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: root.navigate("Dashboard")
                }
            }
            Column {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 1
                width: parent.width - 56
                Text { text: "Sweep"; font.pixelSize: 17; font.weight: Font.Bold; color: theme.c.text }
                I18nText { key: "side.subtitle"; font.pixelSize: 9; font.weight: Font.DemiBold; font.letterSpacing: 0.8; color: theme.c.muted }
            }
        }
        Rectangle { width: parent.width; height: 1; color: theme.c.edge; opacity: 0.55 }
        I18nText { key: "side.menu"; font.pixelSize: 10; font.weight: Font.Bold; font.letterSpacing: 0.9; color: theme.c.muted; leftPadding: 6 }
        Repeater {
            model: root.menuItems
            delegate: NavItem {
                theme: root.theme
                itemName: root.langTick >= 0 ? i18n.tr(modelData.tkey) : ""
                itemDesc: root.langTick >= 0 ? i18n.tr(modelData.dkey) : ""
                glyph: modelData.glyph
                active: modelData.name === root.screen
                delay: index * 70
                onGo: root.navigate(modelData.name)
            }
        }
        I18nText { key: "side.analysis"; font.pixelSize: 10; font.weight: Font.Bold; font.letterSpacing: 0.9; color: theme.c.muted; leftPadding: 6 }
        Repeater {
            model: root.sysItems
            delegate: NavItem {
                theme: root.theme
                itemName: root.langTick >= 0 ? i18n.tr(modelData.tkey) : ""
                itemDesc: root.langTick >= 0 ? i18n.tr(modelData.dkey) : ""
                glyph: modelData.glyph
                active: modelData.name === root.screen
                delay: 140 + index * 70
                onGo: root.navigate(modelData.name)
            }
        }
        I18nText { key: "side.storage"; font.pixelSize: 10; font.weight: Font.Bold; font.letterSpacing: 0.9; color: theme.c.muted; leftPadding: 6 }
        // disk mini card
        Rectangle {
            width: parent.width
            height: 92
            radius: 12
            color: Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.9)
            border.width: 1; border.color: theme.c.edge
            Column {
                anchors.fill: parent
                anchors.margins: 12
                spacing: 7
                Row {
                    width: parent.width
                    I18nText { key: "side.diskUsage"; font.pixelSize: 12; font.weight: Font.DemiBold; color: theme.c.text; width: parent.width - 48 }
                    Text { text: sys.diskTotal > 0 ? Math.round(sys.diskUsed / sys.diskTotal * 100) + "%" : "--"; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.accent; horizontalAlignment: Text.AlignRight; width: 48 }
                }
                Rectangle {
                    width: parent.width; height: 6; radius: 3
                    color: theme.c.edge
                    Rectangle {
                        width: parent.width * (sys.diskTotal > 0 ? Math.min(1, sys.diskUsed / sys.diskTotal) : 0)
                        height: parent.height; radius: 3
                        color: theme.c.accent
                        Behavior on width { NumberAnimation { duration: 500; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
                    }
                }
                Text { text: human(sys.diskUsed) + " / " + human(sys.diskTotal) + " " + (root.langTick >= 0 ? i18n.tr("side.full") : ""); font.pixelSize: 11; color: theme.c.muted }
            }
        }
            Item { width: 1; height: 1 }
        }
    }
    // bottom theme card
    Rectangle {
        anchors.bottom: parent.bottom
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: 14
        height: 60
        radius: 12
        scale: themeMouse.pressed ? 0.98 : 1.0
        Behavior on scale { NumberAnimation { duration: 120 } }
        color: Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.9)
        border.width: 1
        border.color: themeMouse.containsMouse ? Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.5) : theme.c.edge
        Behavior on border.color { ColorAnimation { duration: 150 } }
        Row {
            anchors.fill: parent
            anchors.margins: 10
            spacing: 10
            Rectangle {
                width: 32; height: 32; radius: 9
                anchors.verticalCenter: parent.verticalCenter
                color: Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.16)
                border.width: 1
                border.color: Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.45)
                Text { anchors.centerIn: parent; text: "◐"; font.pixelSize: 15; color: theme.c.accent }
            }
            Column {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 1
                width: parent.width - 84
                Text { text: manager ? manager.current : ""; font.pixelSize: 12; font.weight: Font.DemiBold; color: theme.c.text; elide: Text.ElideRight; width: parent.width }
                I18nText { key: "side.changeTheme"; font.pixelSize: 10; color: theme.c.muted }
            }
            Text { text: "›"; font.pixelSize: 18; color: theme.c.muted; anchors.verticalCenter: parent.verticalCenter }
        }
        MouseArea { id: themeMouse; anchors.fill: parent; hoverEnabled: true; onClicked: root.navigate("Settings") }
    }
}
