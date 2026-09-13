import QtQuick
// Ilk acilis karsilamasi ve 4 duraklik tur.
// mode "welcome": opak tam sahne; mode "tour": yalniz alt kart, gerisi tiklanabilir.
Item {
    id: root
    property var theme
    property var manager
    property string mode: "welcome"
    property int tourStep: 0
    property int tourTotal: 4
    property string stepTkey: ""
    property string stepDkey: ""
    property bool winMax: false
    property bool isLast: tourStep >= tourTotal - 1
    signal tour()
    signal skip()
    signal next()
    signal back()
    property int stage: 0
    function animMs(base) { return themeMgr.motionOff ? 1 : Math.max(1, Math.round(base * themeMgr.animScale)); }
    Component.onCompleted: { if (themeMgr.motionOff) root.stage = 4; }
    Timer {
        interval: 140
        repeat: true
        running: root.visible && root.mode === "welcome" && root.stage < 4 && !themeMgr.motionOff
        onTriggered: root.stage++
    }
    // ---- karsilama sahnesi: logo + yanal icerik ----
    Rectangle {
        anchors.fill: parent
        visible: root.mode === "welcome"
        color: theme.c.bg
        clip: true
        radius: root.winMax ? 0 : 12
        Rectangle {
            id: washA
            width: 380; height: 380; radius: 190; color: theme.c.accent; opacity: 0.06
            x: -150; y: -150
            SequentialAnimation on x {
                loops: Animation.Infinite
                running: root.visible && root.mode === "welcome" && !themeMgr.motionOff
                NumberAnimation { to: -110; duration: 11000; easing.type: Easing.InOutSine }
                NumberAnimation { to: -150; duration: 11000; easing.type: Easing.InOutSine }
            }
        }
        Item {
            x: parent.width - 280; y: parent.height - 280
            width: 460; height: 460
            Rectangle {
                width: 460; height: 460; radius: 230; color: theme.c.accent; opacity: 0.05
                SequentialAnimation on y {
                    loops: Animation.Infinite
                    running: root.visible && root.mode === "welcome" && !themeMgr.motionOff
                    NumberAnimation { to: 24; duration: 13000; easing.type: Easing.InOutSine }
                    NumberAnimation { to: 0; duration: 13000; easing.type: Easing.InOutSine }
                }
            }
        }
        Row {
            anchors.centerIn: parent
            width: Math.min(700, parent.width - 48)
            spacing: 40
            // Logo madalyonu: yavas yukselen-alcalan + cift yonlu yorunge
            Item {
                id: logoWrap
                width: 232; height: 288
                anchors.verticalCenter: parent.verticalCenter
                opacity: root.stage >= 1 ? 1 : 0
                scale: root.stage >= 1 ? 1 : 0.85
                Behavior on opacity { NumberAnimation { duration: root.animMs(400) } }
                Behavior on scale { NumberAnimation { duration: root.animMs(450); easing.type: Easing.OutBack } }
                transform: Translate {
                    id: logoFloat
                    SequentialAnimation on y {
                        loops: Animation.Infinite
                        running: root.visible && root.mode === "welcome" && !themeMgr.motionOff
                        NumberAnimation { to: -9; duration: 2600; easing.type: Easing.InOutSine }
                        NumberAnimation { to: 9; duration: 2600; easing.type: Easing.InOutSine }
                    }
                }
                Item {
                    anchors.centerIn: parent
                    width: 252; height: 252
                    RotationAnimation on rotation { from: 0; to: 360; duration: 9000; loops: Animation.Infinite; running: root.visible && root.mode === "welcome" && !themeMgr.motionOff }
                    Rectangle { width: 9; height: 9; radius: 4.5; color: theme.c.accent; anchors.top: parent.top; anchors.horizontalCenter: parent.horizontalCenter }
                }
                Item {
                    anchors.centerIn: parent
                    width: 278; height: 278
                    RotationAnimation on rotation { from: 360; to: 0; duration: 15000; loops: Animation.Infinite; running: root.visible && root.mode === "welcome" && !themeMgr.motionOff }
                    Rectangle { width: 7; height: 7; radius: 3.5; color: theme.c.muted; anchors.bottom: parent.bottom; anchors.horizontalCenter: parent.horizontalCenter }
                    Rectangle { width: 5; height: 5; radius: 2.5; color: theme.c.accent; opacity: 0.7; anchors.top: parent.top; anchors.horizontalCenter: parent.horizontalCenter; anchors.topMargin: 26 }
                }
                Rectangle {
                    anchors.centerIn: parent
                    width: 216; height: 216; radius: 54
                    color: "#17171a"
                    border.width: 2
                    border.color: Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.55)
                    clip: true
                    Image {
                        anchors.fill: parent
                        source: "assets/sweep-logo.png"
                        sourceSize.width: 432
                        sourceSize.height: 432
                        fillMode: Image.PreserveAspectFit
                        smooth: true
                        mipmap: true
                    }
                }
            }
            Column {
                width: parent.width - 272
                anchors.verticalCenter: parent.verticalCenter
                spacing: 14
                Rectangle {
                    width: 44; height: 5; radius: 2.5; color: theme.c.accent
                    opacity: root.stage >= 2 ? 1 : 0
                    Behavior on opacity { NumberAnimation { duration: root.animMs(300) } }
                }
                I18nText {
                    key: "w.title"
                    font.pixelSize: 34; font.bold: true; color: theme.c.text
                    width: parent.width; wrapMode: Text.WordWrap
                    opacity: root.stage >= 2 ? 1 : 0
                    transform: Translate { x: root.stage >= 2 ? 0 : -18 }
                    Behavior on opacity { NumberAnimation { duration: root.animMs(350) } }
                }
                I18nText {
                    key: "w.sub"
                    font.pixelSize: 15; color: theme.c.muted; wrapMode: Text.WordWrap
                    width: parent.width
                    opacity: root.stage >= 2 ? 1 : 0
                    Behavior on opacity { NumberAnimation { duration: root.animMs(350) } }
                }
                Column {
                    spacing: 10
                    width: parent.width
                    Row {
                        spacing: 12
                        width: parent.width
                        opacity: root.stage >= 3 ? 1 : 0
                        transform: Translate { x: root.stage >= 3 ? 0 : -14 }
                        Behavior on opacity { NumberAnimation { duration: root.animMs(300) } }
                        Item {
                            width: 14; height: 14; anchors.verticalCenter: parent.verticalCenter
                            Rectangle { width: 12; height: 12; radius: 6; anchors.centerIn: parent; color: "transparent"; border.width: 2; border.color: theme.c.accent }
                            Rectangle { width: 4; height: 4; radius: 2; anchors.centerIn: parent; color: theme.c.accent }
                        }
                        I18nText { key: "s.cleaner"; font.pixelSize: 14; color: theme.c.text; width: parent.width - 26; wrapMode: Text.WordWrap }
                    }
                    Row {
                        spacing: 12
                        width: parent.width
                        opacity: root.stage >= 3 ? 1 : 0
                        transform: Translate { x: root.stage >= 3 ? 0 : -14 }
                        Behavior on opacity { NumberAnimation { duration: root.animMs(450) } }
                        Item {
                            width: 14; height: 14; anchors.verticalCenter: parent.verticalCenter
                            Rectangle { width: 12; height: 12; radius: 6; anchors.centerIn: parent; color: "transparent"; border.width: 2; border.color: theme.c.accent }
                            Rectangle { width: 4; height: 4; radius: 2; anchors.centerIn: parent; color: theme.c.accent }
                        }
                        I18nText { key: "s.autostart"; font.pixelSize: 14; color: theme.c.text; width: parent.width - 26; wrapMode: Text.WordWrap }
                    }
                    Row {
                        spacing: 12
                        width: parent.width
                        opacity: root.stage >= 3 ? 1 : 0
                        transform: Translate { x: root.stage >= 3 ? 0 : -14 }
                        Behavior on opacity { NumberAnimation { duration: root.animMs(600) } }
                        Item {
                            width: 14; height: 14; anchors.verticalCenter: parent.verticalCenter
                            Rectangle { width: 12; height: 12; radius: 6; anchors.centerIn: parent; color: "transparent"; border.width: 2; border.color: theme.c.accent }
                            Rectangle { width: 4; height: 4; radius: 2; anchors.centerIn: parent; color: theme.c.accent }
                        }
                        I18nText { key: "s.schedule"; font.pixelSize: 14; color: theme.c.text; width: parent.width - 26; wrapMode: Text.WordWrap }
                    }
                }
                Row {
                    spacing: 12
                    opacity: root.stage >= 4 ? 1 : 0
                    Behavior on opacity { NumberAnimation { duration: root.animMs(350) } }
                    NeonButton { theme: root.theme; manager: root.manager; primary: true; label: i18n.tr("w.tour"); width: 170; onClicked: root.tour() }
                    NeonButton { theme: root.theme; manager: root.manager; label: i18n.tr("w.skip"); width: 130; onClicked: root.skip() }
                }
            }
        }
    }
    // ---- tur karti ----
    Rectangle {
        visible: root.mode === "tour"
        width: Math.min(560, parent.width - 48)
        anchors.bottom: parent.bottom; anchors.bottomMargin: 24
        anchors.horizontalCenter: parent.horizontalCenter
        height: cardCol.height + 32
        radius: 14
        color: theme.c.card
        border.width: 1; border.color: theme.c.muted
        Column {
            id: cardCol
            width: parent.width - 40
            anchors.top: parent.top; anchors.topMargin: 16
            anchors.horizontalCenter: parent.horizontalCenter
            spacing: 8
            Row {
                spacing: 6
                Repeater {
                    model: root.tourTotal
                    Rectangle { width: 8; height: 8; radius: 4; color: index === root.tourStep ? theme.c.accent : theme.c.muted; opacity: index === root.tourStep ? 1 : 0.4 }
                }
                Text { text: (root.tourStep + 1) + " / " + root.tourTotal; font.pixelSize: 12; color: theme.c.muted }
            }
            I18nText { id: stepTitle; key: root.stepTkey; font.pixelSize: 17; font.bold: true; color: theme.c.text }
            I18nText { id: stepBody; key: root.stepDkey; font.pixelSize: 14; color: theme.c.muted; wrapMode: Text.WordWrap; width: cardCol.width }
            Row {
                spacing: 10
                NeonButton { theme: root.theme; manager: root.manager; label: i18n.tr("w.back"); width: 110; visible: root.tourStep > 0; onClicked: root.back() }
                NeonButton { theme: root.theme; manager: root.manager; label: i18n.tr("w.skip"); width: 110; onClicked: root.skip() }
                NeonButton { theme: root.theme; manager: root.manager; label: root.isLast ? i18n.tr("w.done") : i18n.tr("w.next"); width: 130; onClicked: root.next() }
            }
        }
    }
    onStepTkeyChanged: stepTitle.refresh()
    onStepDkeyChanged: stepBody.refresh()
}
