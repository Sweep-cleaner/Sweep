import QtQuick
Rectangle {
    id: root
    property var theme
    property var manager
    property string text: ""
    property string level: "warn"
    width: badgeText.width + 30
    height: 22
    radius: 11
    opacity: 0 // `ok` was added for the disk-outside screens (startup impact, SMART,
    // health), which need a green badge alongside the existing warn/danger.
    property color dot: level === "danger" ? theme.c.danger : level === "ok" ? theme.c.success : theme.c.warning
    color: Qt.rgba(dot.r, dot.g, dot.b, 0.16)
    border.width: 1
    border.color: Qt.rgba(dot.r, dot.g, dot.b, 0.55)
    Component.onCompleted: fade.start()
    NumberAnimation {
        id: fade
        target: root
        property: "opacity"
        to: 1
        duration: 220
        easing.type: Easing.BezierSpline
        easing.bezierCurve: [0.4, 0, 0.2, 1]
    }
    Row {
        anchors.centerIn: parent
        spacing: 6
        Rectangle { width: 6; height: 6; radius: 3; anchors.verticalCenter: parent.verticalCenter; color: root.dot }
        Text {
            id: badgeText
            text: root.text
            font.pixelSize: 11
            font.weight: Font.Bold
            color: theme.c.text
            anchors.verticalCenter: parent.verticalCenter
        }
    }
}
