import QtQuick
Item {
    id: root
    property var theme
    property ThemeManager manager
    property real uiOpacity: 1
    property string language: settings ? settings.language : "tr"
    signal opacityChanged(real v)
    onLanguageChanged: { if (settings) settings.language = root.language; }
    property bool themeOpen: false
    property bool langOpen: false
    property var languages: ["en", "tr", "de", "fr", "es", "it", "pt", "ru"]
    function effR() { return (manager && manager.radius !== undefined) ? manager.radius : 16; }
    onUiOpacityChanged: { if (manager && Math.abs(manager.windowOpacity - uiOpacity) > 0.001) manager.windowOpacity = uiOpacity; }
    Flickable {
        anchors.fill: parent
        contentWidth: width
        contentHeight: col.implicitHeight + 44
        clip: true
        Column {
            id: col
            width: parent.width
            spacing: 16
            Text { text: "Settings"; font.pixelSize: 26; font.weight: Font.Bold; color: theme.c.text }
            // ---- appearance ----
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: 348
                enterDelay: 0
                Column {
                    anchors.fill: parent
                    anchors.margins: 18
                    spacing: 12
                    Text { text: "◑  THEME  ·  " + manager.current; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.muted }
                    Grid {
                        id: themeGrid
                        width: parent.width
                        columns: 5
                        spacing: 10
                        Repeater {
                            model: manager.names()
                            delegate: Rectangle {
                                width: (themeGrid.width - 40) / 5
                                height: 86
                                radius: 12
                                color: Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.9)
                                border.width: modelData === manager.current ? 2 : 1
                                border.color: modelData === manager.current ? theme.c.accent : theme.c.edge
                                scale: cellMouse.containsMouse ? 1.03 : 1.0
                                Behavior on scale { NumberAnimation { duration: 140 } }
                                Behavior on border.color { ColorAnimation { duration: 150 } }
                                Column {
                                    anchors.fill: parent
                                    anchors.margins: 10
                                    spacing: 6
                                    Row {
                                        spacing: 5
                                        Rectangle { width: 22; height: 22; radius: 6; color: manager.accentFor(modelData) }
                                        Column {
                                            spacing: 3
                                            anchors.verticalCenter: parent.verticalCenter
                                            Rectangle { width: 52; height: 5; radius: 3; color: theme.c.edge }
                                            Rectangle { width: 36; height: 5; radius: 3; color: theme.c.edge; opacity: 0.7 }
                                        }
                                    }
                                    Text { text: modelData; font.pixelSize: 11; font.weight: modelData === manager.current ? Font.Bold : Font.Medium; color: modelData === manager.current ? theme.c.accent : theme.c.text; elide: Text.ElideRight; width: parent.width - 4 }
                                }
                                MouseArea { id: cellMouse; anchors.fill: parent; hoverEnabled: true; onClicked: manager.set(modelData) }
                            }
                        }
                    }
                    Text { text: "✦  Accent"; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.muted }
                    Row {
                        spacing: 8
                        Rectangle {
                            width: 30; height: 30; radius: 15
                            color: "transparent"
                            border.width: manager.accentOverride === "" ? 2 : 1
                            border.color: manager.accentOverride === "" ? theme.c.accent : theme.c.edge
                            Text { anchors.centerIn: parent; text: "A"; font.pixelSize: 13; font.weight: Font.Bold; color: theme.c.text }
                            MouseArea { anchors.fill: parent; onClicked: manager.accentOverride = "" }
                        }
                        Repeater {
                            model: manager.names()
                            delegate: Rectangle {
                                width: 30; height: 30; radius: 15
                                color: manager.accentFor(modelData)
                                border.width: manager.accentOverride === manager.accentFor(modelData) ? 2 : 0
                                border.color: theme.c.text
                                opacity: am.containsMouse ? 1.0 : 0.85
                                MouseArea { id: am; anchors.fill: parent; hoverEnabled: true; onClicked: manager.accentOverride = manager.accentFor(modelData) }
                            }
                        }
                    }
                }
            }
            // ---- transparency ----
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: 252
                enterDelay: 80
                Column {
                    anchors.fill: parent
                    anchors.margins: 18
                    spacing: 10
                    Text { text: "◍  TRANSPARENCY & BLUR"; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.muted }
                    Row {
                        width: parent.width
                        spacing: 14
                        Column {
                            width: parent.width - 220
                            spacing: 8
                            Text { text: "Window opacity  " + Math.round(manager.windowOpacity * 100) + "%"; font.pixelSize: 13; color: theme.c.text }
                            NeonSlider { theme: root.theme; manager: root.manager; width: parent.width; from: 0.35; to: 1; value: manager.windowOpacity; onMoved: function(v) { manager.setWindowOpacity(v); root.uiOpacity = manager.windowOpacity; root.opacityChanged(manager.windowOpacity); } }
                            Text { text: "Panel glass  " + Math.round(manager.glass * 100) + "%"; font.pixelSize: 13; color: theme.c.text }
                            NeonSlider { theme: root.theme; manager: root.manager; width: parent.width; from: 0.25; to: 1; value: manager.glass; onMoved: function(v) { manager.setGlass(v); } }
                        }
                        // live glass preview
                        Rectangle {
                            width: 180; height: 130; radius: 14
                            color: Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.35)
                            Rectangle {
                                anchors.centerIn: parent
                                width: 130; height: 70; radius: 12
                                color: Qt.rgba(theme.c.panel.r, theme.c.panel.g, theme.c.panel.b, manager.blur ? manager.glass : 1.0)
                                border.width: 1; border.color: theme.c.edge
                                Text { anchors.centerIn: parent; text: Math.round((manager.blur ? manager.glass : 1.0) * 100) + "%"; font.pixelSize: 16; font.weight: Font.Bold; color: theme.c.text }
                            }
                        }
                    }
                    Row {
                        spacing: 10
                        Rectangle {
                            width: 52; height: 28; radius: 14
                            color: manager.blur ? theme.c.accent : theme.c.edge
                            Behavior on color { ColorAnimation { duration: 150 } }
                            Rectangle {
                                width: 22; height: 22; radius: 11
                                anchors.verticalCenter: parent.verticalCenter
                                x: manager.blur ? 27 : 3
                                color: theme.c.accentText
                                Behavior on x { NumberAnimation { duration: 150; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
                            }
                            MouseArea { anchors.fill: parent; onClicked: manager.toggleBlur() }
                        }
                        Text { text: "Background blur & aurora effects"; font.pixelSize: 13; color: theme.c.text; anchors.verticalCenter: parent.verticalCenter }
                    }
                }
            }
            // ---- motion ----
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: 168
                enterDelay: 140
                Column {
                    anchors.fill: parent
                    anchors.margins: 18
                    spacing: 10
                    Text { text: "➤  MOTION"; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.muted }
                    Text { text: manager.motionOff ? "Animations off (reduced motion)" : "Animation speed  " + manager.animScale.toFixed(1) + "×"; font.pixelSize: 13; color: theme.c.text }
                    NeonSlider { theme: root.theme; manager: root.manager; width: 340; from: 0; to: 2; value: manager.animScale; onMoved: function(v) { manager.setAnimScale(v); } }
                    Row {
                        spacing: 8
                        Repeater {
                            model: [{ l: "Off", v: 0 }, { l: "Calm", v: 0.6 }, { l: "Normal", v: 1.0 }, { l: "Lush", v: 1.6 }]
                            delegate: Rectangle {
                                width: 76; height: 30; radius: 9
                                color: Math.abs(manager.animScale - modelData.v) < 0.25 ? theme.c.accent : Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.9)
                                border.width: 1; border.color: theme.c.edge
                                Text { anchors.centerIn: parent; text: modelData.l; font.pixelSize: 12; font.weight: Font.DemiBold; color: Math.abs(manager.animScale - modelData.v) < 0.25 ? theme.c.accentText : theme.c.text }
                                MouseArea { anchors.fill: parent; onClicked: manager.setAnimScale(modelData.v) }
                            }
                        }
                    }
                }
            }
            // ---- language ----
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: 150
                enterDelay: 200
                Column {
                    anchors.fill: parent
                    anchors.margins: 18
                    spacing: 10
                    Text { text: "◐  LANGUAGE  ·  " + root.language; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.muted }
                    Flow {
                        width: parent.width
                        spacing: 8
                        Repeater {
                            model: root.languages
                            delegate: Rectangle {
                                width: 64; height: 32; radius: 10
                                color: modelData === root.language ? theme.c.accent : Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.9)
                                border.width: 1; border.color: modelData === root.language ? theme.c.accent : theme.c.edge
                                Text { anchors.centerIn: parent; text: modelData.toUpperCase(); font.pixelSize: 12; font.weight: Font.Bold; color: modelData === root.language ? theme.c.accentText : theme.c.text }
                                MouseArea { anchors.fill: parent; onClicked: root.language = modelData }
                            }
                        }
                    }
                }
            }
            // ---- tray ----
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: 130
                enterDelay: 230
                Column {
                    anchors.fill: parent
                    anchors.margins: 18
                    spacing: 10
                    Text { text: "◉  TRAY"; font.pixelSize: 12; font.weight: Font.Bold; color: theme.c.muted }
                    Row {
                        spacing: 10
                        Rectangle {
                            width: 52; height: 28; radius: 14
                            color: settings.minimizeToTray ? theme.c.accent : theme.c.edge
                            Behavior on color { ColorAnimation { duration: 150 } }
                            Rectangle {
                                width: 22; height: 22; radius: 11
                                anchors.verticalCenter: parent.verticalCenter
                                x: settings.minimizeToTray ? 27 : 3
                                color: theme.c.accentText
                                Behavior on x { NumberAnimation { duration: 150; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
                            }
                            MouseArea { anchors.fill: parent; onClicked: settings.minimizeToTray = !settings.minimizeToTray }
                        }
                        Column {
                            spacing: 2
                            anchors.verticalCenter: parent.verticalCenter
                            Text { text: "Minimize to tray"; font.pixelSize: 13; color: theme.c.text }
                            Text { text: "Closing the window keeps Sweep in the system tray"; font.pixelSize: 11; color: theme.c.muted }
                        }
                    }
                }
            }
            // ---- about ----
            GlassCard {
                theme: root.theme; manager: root.manager
                width: parent.width - 44
                anchors.horizontalCenter: parent.horizontalCenter
                height: 118
                enterDelay: 260
                Row {
                    anchors.fill: parent
                    anchors.margins: 18
                    spacing: 14
                    Image {
                        width: 52; height: 52
                        anchors.verticalCenter: parent.verticalCenter
                        source: "assets/sweep-logo.png"
                        sourceSize.width: 104
                        sourceSize.height: 104
                        smooth: true
                        mipmap: true
                        fillMode: Image.PreserveAspectFit
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 3
                        width: parent.width - 90
                        Text { text: "Sweep  ·  system cleaner"; font.pixelSize: 15; font.weight: Font.Bold; color: theme.c.text }
                        Text { text: manager.names().length + " themes  ·  live sys stats  ·  offline & private"; font.pixelSize: 12; color: theme.c.muted }
                        Text { text: "Tema: " + manager.current + "  ·  Animasyon: " + (manager.motionOff ? "kapalı" : manager.animScale.toFixed(1) + "×"); font.pixelSize: 12; color: theme.c.muted }
                    }
                }
            }
            Item { width: 1; height: 22 }
        }
    }
}
