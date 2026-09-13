import QtQuick
import QtQuick.Window
Window {
    id: root
    width: 1140
    height: 720
    minimumWidth: 1000
    minimumHeight: 620
    visible: true
    title: "Sweep"
    // Gri OS baslik cubugu kaldirildi: cercevesiz pencere + asagidaki ozel
    // baslik cubugu (surukle / simge / buyut / kapat). Kenar-surukleme ve
    // Aero-snap yerine sag-alt kose tutamaci vardir.
    flags: Qt.FramelessWindowHint | Qt.Window
    // Pencere her zaman saydamdir; arka plani asagidaki yuvarlak cerceve
    // (chrome) cizer. Boylece koseler masaustune yuvarlak acilir ve Mica
    // DWM'den sizar.
    color: "transparent"
    opacity: themeMgr.windowOpacity * root.exitFade
    // Kapanis kararmasi (tepsiye iniste degil, gercek cikista oynar).
    property real exitFade: 1
    property string activeScreen: "Dashboard"
    property alias uiOpacity: themeMgr.windowOpacity
    property int langTick: 0
    ThemeManager { id: themeMgr }
    property var theme: themeMgr
    function screenTitle(name) {
        if (name === "Dashboard") return i18n.tr("t.dashboard");
        if (name === "Cleaner") return i18n.tr("t.cleaner");
        if (name === "History") return i18n.tr("t.history");
        if (name === "System") return i18n.tr("t.system");
        if (name === "Files") return i18n.tr("t.files");
        if (name === "Disk") return i18n.tr("t.disk");
        if (name === "Optimize") return i18n.tr("t.memory");
        if (name === "Autostart") return i18n.tr("t.autostart");
        if (name === "SysInfo") return i18n.tr("t.sysinfo");
        if (name === "Network") return i18n.tr("t.network");
        if (name === "Privacy") return i18n.tr("t.privacy");
        if (name === "Schedule") return i18n.tr("t.schedule");
        return i18n.tr("t.settings");
    }
    function screenSubtitle(name) {
        if (name === "Dashboard") return i18n.tr("s.dashboard");
        if (name === "Cleaner") return i18n.tr("s.cleaner");
        if (name === "History") return i18n.tr("s.history");
        if (name === "System") return i18n.tr("s.system");
        if (name === "Files") return i18n.tr("s.files");
        if (name === "Disk") return i18n.tr("s.disk");
        if (name === "Optimize") return i18n.tr("s.memory");
        if (name === "Autostart") return i18n.tr("s.autostart");
        if (name === "SysInfo") return i18n.tr("s.sysinfo");
        if (name === "Network") return i18n.tr("s.network");
        if (name === "Privacy") return i18n.tr("s.privacy");
        if (name === "Schedule") return i18n.tr("s.schedule");
        return i18n.tr("s.settings");
    }
    function show(name) {
        if (root.activeScreen === name && loader.status === Loader.Ready) return;
        root.activeScreen = name;
        exitAnim.start();
    }
    // Karsilama turu: 4 durak, basliklar mevcut t.*/s.* anahtarlarindan.
    property bool welcomeOpen: false
    property bool tourActive: false
    property int tourStep: 0
    property var tourStops: [
        { s: "Dashboard", t: "t.dashboard", d: "s.dashboard" },
        { s: "Cleaner", t: "t.cleaner", d: "s.cleaner" },
        { s: "Autostart", t: "t.autostart", d: "s.autostart" },
        { s: "Schedule", t: "t.schedule", d: "s.schedule" }
    ]
    function startTour() { root.tourActive = true; root.tourStep = 0; show(root.tourStops[0].s); }
    function tourNext() {
        if (root.tourStep < root.tourStops.length - 1) { root.tourStep++; show(root.tourStops[root.tourStep].s); }
        else endTour();
    }
    function tourBack() { if (root.tourStep > 0) { root.tourStep--; show(root.tourStops[root.tourStep].s); } }
    function endTour() { root.tourActive = false; root.welcomeOpen = false; settings.welcomed = true; }
    function loadScreen() {
        if (root.activeScreen === "Dashboard") loader.setSource("Dashboard.qml", { theme: themeMgr, manager: themeMgr });
        else if (root.activeScreen === "Cleaner") loader.setSource("CleanerScreen.qml", { theme: themeMgr, manager: themeMgr });
        else if (root.activeScreen === "History") loader.setSource("HistoryScreen.qml", { theme: themeMgr, manager: themeMgr });
        else if (root.activeScreen === "System") loader.setSource("SystemScreen.qml", { theme: themeMgr, manager: themeMgr });
        else if (root.activeScreen === "Files") loader.setSource("FilesScreen.qml", { theme: themeMgr, manager: themeMgr });
        else if (root.activeScreen === "Disk") loader.setSource("DiskScreen.qml", { theme: themeMgr, manager: themeMgr });
        else if (root.activeScreen === "Optimize") loader.setSource("MemoryScreen.qml", { theme: themeMgr, manager: themeMgr });
        else if (root.activeScreen === "Autostart") loader.setSource("AutostartScreen.qml", { theme: themeMgr, manager: themeMgr });
        else if (root.activeScreen === "SysInfo") loader.setSource("SysInfoScreen.qml", { theme: themeMgr, manager: themeMgr });
        else if (root.activeScreen === "Network") loader.setSource("NetworkScreen.qml", { theme: themeMgr, manager: themeMgr });
        else if (root.activeScreen === "Privacy") loader.setSource("PrivacyScreen.qml", { theme: themeMgr, manager: themeMgr });
        else if (root.activeScreen === "Schedule") loader.setSource("ScheduleScreen.qml", { theme: themeMgr, manager: themeMgr });
        else loader.setSource("Settings.qml", { theme: themeMgr, manager: themeMgr, uiOpacity: themeMgr.windowOpacity });
    }
    function toast(msg, level) { toastHost.show(msg, level); }
    // Buyut / geri yukle (Qt normal geometriyi kendisi saklar).
    function winMax() {
        if (root.visibility === Window.Maximized) root.visibility = Window.Windowed;
        else root.visibility = Window.Maximized;
    }
    // Otomatik bellek bakimi (A modu): hangi ekran acik olursa olsun calisir.
    // 5 dk'da bir, zaten 1Hz'de akan sys verisine bakar (ek olcum yok);
    // RAM %80'i asarsa ve kopru bossa guvenli memopt kosar. 15 dk soguma;
    // zaman damgasi bastan yazilir ki basarisiz kosu bile spam uretmesin.
    // Uygulama kapaliyken calismaz — onun icin Zamanlama'daki bellek gorevi var.
    property double autoLastMem: 0
    Timer {
        id: autoMemTimer
        interval: 300000
        running: true
        repeat: true
        onTriggered: {
            if (!settings.autoMem) return;
            if (sweep.busy()) return;
            if (typeof sweep.sweepBinOk === "function" && !sweep.sweepBinOk()) return;
            if (Date.now() - root.autoLastMem < 900000) return;
            if (!(sys.memTotal > 0) || sys.memUsed / sys.memTotal < 0.8) return;
            root.autoLastMem = Date.now();
            sweep.memopt();
        }
    }
    // Kalıcı ayarlar: açılışta QSettings'ten ThemeManager'a yükle.
    Component.onCompleted: {
        themeMgr.current = settings.theme;
        themeMgr.accentOverride = settings.accent;
        themeMgr.windowOpacity = settings.windowOpacity;
        themeMgr.glass = settings.glass;
        themeMgr.blur = settings.blur;
        themeMgr.animScale = settings.animScale;
        loadScreen();
        if (!settings.welcomed) root.welcomeOpen = true;
    }
    // Her değişiklik anında diske yaz (QSettings).
    Connections {
        target: themeMgr
        function onCurrentChanged() { settings.theme = themeMgr.current; }
        function onAccentOverrideChanged() { settings.accent = themeMgr.accentOverride; }
        function onWindowOpacityChanged() { settings.windowOpacity = themeMgr.windowOpacity; }
        function onGlassChanged() { settings.glass = themeMgr.glass; }
        function onBlurChanged() { settings.blur = themeMgr.blur; }
        function onAnimScaleChanged() { settings.animScale = themeMgr.animScale; }
    }
    Connections {
        target: settings
        function onLanguageChanged() { i18n.lang = settings.language; }
    }
    Connections {
        target: i18n
        function onLangChanged() { root.langTick++; }
    }
    // Kapat düğmesi → tepsiye küçült (Ayarlar'da kapatılabilir).
    // Her iki yolda da kisa kararma oynar: tepside gizlenip geri doner,
    // gercek cikista bayrak konup kapatilir.
    property bool exitToTray: false
    onClosing: function(close) {
        if (settings && settings.minimizeToTray && !settings.quitting) {
            close.accepted = false;
            root.exitToTray = true;
            closeAnim.start();
            return;
        }
        if (settings && !settings.quitting) {
            close.accepted = false;
            root.exitToTray = false;
            closeAnim.start();
        }
    }
    NumberAnimation {
        id: closeAnim
        target: root
        property: "exitFade"
        to: 0
        duration: 220
        easing.type: Easing.OutQuad
        onStopped: {
            if (root.exitToTray) {
                root.exitFade = 1;
                sweep.hideWindow();
            } else {
                settings.quitting = true;
                root.exitFade = 1;
                root.close();
            }
        }
    }
    SequentialAnimation {
        id: exitAnim
        ParallelAnimation {
            NumberAnimation { target: loader; property: "opacity"; to: 0; duration: themeMgr.motionOff ? 1 : Math.max(1, Math.round(130 * themeMgr.animScale)); easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] }
            NumberAnimation { target: loader; property: "x"; to: -18; duration: themeMgr.motionOff ? 1 : Math.max(1, Math.round(210 * themeMgr.animScale)); easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] }
            NumberAnimation { target: loader; property: "scale"; to: 0.985; duration: themeMgr.motionOff ? 1 : Math.max(1, Math.round(210 * themeMgr.animScale)); easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] }
        }
        ScriptAction { script: loadScreen(); }
    }
    NumberAnimation {
        id: enterAnim
        target: loader
        property: "opacity"
        from: 0
        to: 1
        duration: themeMgr.motionOff ? 1 : Math.max(1, Math.round(260 * themeMgr.animScale))
        easing.type: Easing.BezierSpline
        easing.bezierCurve: [0.4, 0, 0.2, 1]
    }
    NumberAnimation {
        id: slideAnim
        target: loader
        property: "x"
        from: 26
        to: 0
        duration: themeMgr.motionOff ? 1 : Math.max(1, Math.round(300 * themeMgr.animScale))
        easing.type: Easing.BezierSpline
        easing.bezierCurve: [0.4, 0, 0.2, 1]
    }
    NumberAnimation {
        id: zoomAnim
        target: loader
        property: "scale"
        from: 0.97
        to: 1
        duration: themeMgr.motionOff ? 1 : Math.max(1, Math.round(320 * themeMgr.animScale))
        easing.type: Easing.BezierSpline
        easing.bezierCurve: [0.4, 0, 0.2, 1]
    }
    Connections {
        target: loader.item
        ignoreUnknownSignals: true
        function onOpacityChanged(v) { themeMgr.windowOpacity = v; }
        function onNavigate(name) { show(name); }
        // Ekranlar kendi toast'larını yayar (AutostartScreen); metin `i18n.tr`
        // ile ekranın içinde çözülür, burada yalnız kuyruğa verilir.
        function onToast(msg, level) { toastHost.show(msg, level); }
    }
    // Yuvarlak pencere cercevesi: tum icerik bu kirpilir kutunun icindedir.
    // Buyutulmusken kose sifirlanir ki ekran kenarlarinda bosluk kalmasin.
    Rectangle {
        id: chrome
        anchors.fill: parent
        radius: root.visibility === Window.Maximized ? 0 : 12
        clip: true
        color: (sys && sys.winMica) ? Qt.rgba(themeMgr.c.bg.r, themeMgr.c.bg.g, themeMgr.c.bg.b, 0.72) : themeMgr.c.bg
    AuroraBackground {
        anchors.fill: parent
        theme: themeMgr
        manager: themeMgr
    }
    Column {
        anchors.fill: parent
        // ---- ozel baslik cubugu (OS cercevesi yok) ----
        Rectangle {
            id: titleBar
            width: parent.width
            height: 42
            color: Qt.rgba(themeMgr.c.panel.r, themeMgr.c.panel.g, themeMgr.c.panel.b, 0.65)
            Rectangle { width: parent.width; height: 1; anchors.bottom: parent.bottom; color: themeMgr.c.edge; opacity: 0.6 }
            property point dragPos: Qt.point(0, 0)
            Row {
                anchors.left: parent.left
                anchors.leftMargin: 14
                anchors.verticalCenter: parent.verticalCenter
                spacing: 9
                Image {
                    width: 22; height: 22
                    anchors.verticalCenter: parent.verticalCenter
                    source: "assets/sweep-logo.png"
                    sourceSize.width: 44
                    sourceSize.height: 44
                    smooth: true
                    mipmap: true
                    fillMode: Image.PreserveAspectFit
                }
                Text { text: "Sweep"; font.pixelSize: 13; font.weight: Font.DemiBold; color: themeMgr.c.muted; anchors.verticalCenter: parent.verticalCenter }
            }
            MouseArea {
                id: sysMove
                anchors.fill: parent
                anchors.rightMargin: 138
                onPressed: function(m) { titleBar.dragPos = Qt.point(m.x, m.y); }
                onPositionChanged: function(m) {
                    if (!pressed || root.visibility !== Window.Windowed) return;
                    root.x += m.x - titleBar.dragPos.x;
                    root.y += m.y - titleBar.dragPos.y;
                }
                onDoubleClicked: root.winMax()
            }
            Row {
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                spacing: 0
                Rectangle {
                    id: winMin
                    width: 46; height: 32
                    color: minHover.containsMouse ? Qt.rgba(themeMgr.c.edge.r, themeMgr.c.edge.g, themeMgr.c.edge.b, 0.7) : "transparent"
                    Text { anchors.centerIn: parent; text: "—"; font.pixelSize: 12; color: themeMgr.c.text }
                    MouseArea { id: minHover; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.showMinimized() }
                }
                Rectangle {
                    id: winMax
                    width: 46; height: 32
                    color: maxHover.containsMouse ? Qt.rgba(themeMgr.c.edge.r, themeMgr.c.edge.g, themeMgr.c.edge.b, 0.7) : "transparent"
                    Text { anchors.centerIn: parent; text: root.visibility === Window.Maximized ? "❐" : "▢"; font.pixelSize: 11; color: themeMgr.c.text }
                    MouseArea { id: maxHover; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.winMax() }
                }
                Rectangle {
                    id: winClose
                    width: 46; height: 32
                    color: closeHover.containsMouse ? "#E81123" : "transparent"
                    Text { anchors.centerIn: parent; text: "✕"; font.pixelSize: 12; color: closeHover.containsMouse ? "white" : themeMgr.c.text }
                    MouseArea { id: closeHover; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.close() }
                }
            }
        }
        Row {
        width: parent.width
        height: parent.height - titleBar.height
        Sidebar {
            id: bar
            height: parent.height
            theme: themeMgr
            manager: themeMgr
            screen: root.activeScreen
            uiOpacity: themeMgr.windowOpacity
            onNavigate: function(name) { show(name); }
        }
        Column {
            width: parent.width - bar.width
            height: parent.height
            Behavior on width { NumberAnimation { duration: 220; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
            // top bar: page title on the left, live status on the right
            Rectangle {
                width: parent.width
                height: 58
                color: Qt.rgba(themeMgr.c.panel.r, themeMgr.c.panel.g, themeMgr.c.panel.b, 0.5)
                Rectangle { width: parent.width; height: 1; anchors.bottom: parent.bottom; color: themeMgr.c.edge; opacity: 0.6 }
                Column {
                    anchors.left: parent.left
                    anchors.leftMargin: 24
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 2
                    Text { text: root.langTick >= 0 ? root.screenTitle(root.activeScreen) : ""; font.pixelSize: 18; font.weight: Font.DemiBold; color: themeMgr.c.text }
                    Text { text: root.langTick >= 0 ? root.screenSubtitle(root.activeScreen) : ""; font.pixelSize: 12; color: themeMgr.c.muted }
                }
                Row {
                    anchors.right: parent.right
                    anchors.rightMargin: 20
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 0
                    Rectangle {
                        width: statusRow.width + 26
                        height: 32
                        radius: 16
                        color: Qt.rgba(themeMgr.c.panel2.r, themeMgr.c.panel2.g, themeMgr.c.panel2.b, 0.8)
                        border.width: 1
                        border.color: themeMgr.c.edge
                        Row {
                            id: statusRow
                            anchors.centerIn: parent
                            spacing: 12
                            Row {
                                spacing: 7
                                anchors.verticalCenter: parent.verticalCenter
                                Rectangle {
                                    width: 7; height: 7; radius: 4
                                    anchors.verticalCenter: parent.verticalCenter
                                    color: themeMgr.c.accent
                                }
                                Text { text: "CPU"; font.pixelSize: 11; font.weight: Font.DemiBold; font.letterSpacing: 0.5; color: themeMgr.c.muted; anchors.verticalCenter: parent.verticalCenter }
                                Text { text: sys.cpuPct.toFixed(0) + "%"; font.pixelSize: 12; font.weight: Font.Bold; color: themeMgr.c.text; anchors.verticalCenter: parent.verticalCenter }
                            }
                            Rectangle { width: 1; height: 14; color: themeMgr.c.edge; anchors.verticalCenter: parent.verticalCenter }
                            Row {
                                spacing: 7
                                anchors.verticalCenter: parent.verticalCenter
                                Text { text: "DİSK"; font.pixelSize: 11; font.weight: Font.DemiBold; font.letterSpacing: 0.5; color: themeMgr.c.muted; anchors.verticalCenter: parent.verticalCenter }
                                Text { text: sys.diskTotal > 0 ? Math.round(sys.diskUsed / sys.diskTotal * 100) + "%" : "--"; font.pixelSize: 12; font.weight: Font.Bold; color: themeMgr.c.text; anchors.verticalCenter: parent.verticalCenter }
                            }
                        }
                    }
                }
            }
            Item {
                width: parent.width
                height: parent.height - 58
                Loader {
                    id: loader
                    anchors.fill: parent
                    transformOrigin: Item.Center
                    onLoaded: { enterAnim.start(); slideAnim.start(); zoomAnim.start(); }
                }
            }
        }
        }
    }
        // sag-alt kose tutamaci: cerceve icinde kalir, kose egrisine kirpilir
        Rectangle {
            width: 22; height: 22
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            color: "transparent"
            visible: root.visibility === Window.Windowed
            Text { anchors.centerIn: parent; text: "◢"; font.pixelSize: 12; color: themeMgr.c.muted; opacity: 0.7 }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.SizeFDiagCursor
                hoverEnabled: true
                property point prev: Qt.point(0, 0)
                onPressed: function(m) { prev = Qt.point(m.x, m.y); }
                onPositionChanged: function(m) {
                    if (!pressed || root.visibility !== Window.Windowed) return;
                    root.width = Math.max(root.minimumWidth, root.width + (m.x - prev.x));
                    root.height = Math.max(root.minimumHeight, root.height + (m.y - prev.y));
                    prev = Qt.point(m.x, m.y);
                }
            }
        }
    }
    // Ilk-acilis karsilamasi ve tur (ayarlarda kalici welcomed bayragi).
    WelcomeScreen {
        anchors.fill: parent
        theme: themeMgr
        manager: themeMgr
        visible: root.welcomeOpen
        mode: root.tourActive ? "tour" : "welcome"
        tourStep: root.tourStep
        tourTotal: root.tourStops.length
        winMax: root.visibility === Window.Maximized
        stepTkey: root.tourActive ? root.tourStops[root.tourStep].t : ""
        stepDkey: root.tourActive ? root.tourStops[root.tourStep].d : ""
        onTour: startTour()
        onSkip: endTour()
        onNext: tourNext()
        onBack: tourBack()
    }
    ToastHost {
        id: toastHost
        theme: themeMgr
        manager: themeMgr
        anchors.fill: parent
        anchors.margins: 18
    }
    Connections {
        target: sweep
        ignoreUnknownSignals: true
        function onFailed(msg) { toastHost.show(String(msg).slice(0, 160), "bad"); }
    }
}