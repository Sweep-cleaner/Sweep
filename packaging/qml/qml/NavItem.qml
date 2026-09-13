import QtQuick
// Sidebar navigation item: staggered entrance, a small hover nudge and a flat
// monochrome icon tile. No pulsing glow — the active state is carried by the
// tint, the border and the rail marker on the sidebar edge.
Rectangle {
    id: navCard
    property var theme
    property string itemName: ""
    property string itemDesc: ""
    property string glyph: ""
    property bool active: false
    property int delay: 0
    signal go
    width: 204
    height: 54
    radius: 12
    opacity: 0
    transform: Translate { id: slide; x: -14 }
    property bool hovered: itemMouse.containsMouse
    x: hovered ? 2 : 0
    Behavior on x { NumberAnimation { duration: 160; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
    scale: itemMouse.pressed ? 0.98 : 1.0
    Behavior on scale { NumberAnimation { duration: 110 } }
    // aktif olunca küçük bir spring pop
    SequentialAnimation {
        id: activePop
        NumberAnimation { target: navCard; property: "scale"; to: 1.035; duration: 130; easing.type: Easing.OutQuad }
        NumberAnimation { target: navCard; property: "scale"; to: 1.0; duration: 280; easing.type: Easing.OutBack }
    }
    onActiveChanged: { if (navCard.active) activePop.restart(); }
    color: active ? Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.13) : (hovered ? Qt.rgba(theme.c.edge.r, theme.c.edge.g, theme.c.edge.b, 0.4) : "transparent")
    border.width: active ? 1 : 0
    border.color: Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.4)
    Behavior on color { ColorAnimation { duration: 160 } }
    // rail marker on the sidebar edge
    Rectangle {
        width: 3; height: 22; radius: 2
        anchors.left: parent.left; anchors.leftMargin: -14
        anchors.verticalCenter: parent.verticalCenter
        color: theme.c.accent
        visible: navCard.active
    }
    Row {
        anchors.fill: parent
        anchors.leftMargin: 10
        anchors.rightMargin: 10
        spacing: 11
        Rectangle {
            width: 32; height: 32; radius: 9
            anchors.verticalCenter: parent.verticalCenter
            color: navCard.active ? Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.18) : Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.85)
            border.width: 1
            border.color: navCard.active ? Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.45) : theme.c.edge
            Text {
                anchors.centerIn: parent
                text: navCard.glyph
                // 16px Windows yedek fontlarinda (Segoe UI Symbol) kucuk /
                // eksik gorunuyordu; karo 32px oldugu icin 19px guvenli.
                font.pixelSize: 19
                color: navCard.active ? theme.c.accent : theme.c.muted
            }
        }
        Column {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 1
            width: parent.width - 60
            Text { text: navCard.itemName; font.pixelSize: 13; font.weight: navCard.active ? Font.DemiBold : Font.Normal; color: theme.c.text; elide: Text.ElideRight; width: parent.width }
            Text { text: navCard.itemDesc; font.pixelSize: 10; color: navCard.active ? theme.c.accent : theme.c.muted; elide: Text.ElideRight; width: parent.width }
        }
    }
    MouseArea { id: itemMouse; anchors.fill: parent; hoverEnabled: true; onClicked: navCard.go() }
    Timer { interval: navCard.delay; running: true; repeat: false; onTriggered: enterAnim.start() }
    ParallelAnimation {
        id: enterAnim
        NumberAnimation { target: slide; property: "x"; to: 0; duration: 300; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] }
        NumberAnimation { target: navCard; property: "opacity"; to: 1; duration: 300 }
    }
}
