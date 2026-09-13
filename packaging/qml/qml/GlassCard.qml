import QtQuick
// Surface panel: entrance animation, a quiet hover response and a 1px top
// hairline. Deliberately understated — no gloss band, no glow, no accent bar.
Rectangle {
    id: root
    property var theme
    property var manager
    property bool hoverable: true
    property int enterDelay: 0
    property bool entered: false
    default property alias body: inner.children

    function effRadius() { return (manager && manager.radius !== undefined) ? manager.radius : 16; }
    function animMs() { return (manager && manager.animMs !== undefined) ? manager.animMs : 220; }
    function glassOn() { return (!manager || manager.blur === undefined) ? true : manager.blur; }
    function glassA() { return (manager && manager.glass !== undefined) ? manager.glass : 0.82; }
    function hoverBorder() {
        if (!theme) return "#333344";
        if (!(hovered && hoverable)) return theme.c.edge;
        return Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.5);
    }

    radius: effRadius()
    color: theme ? Qt.rgba(theme.c.panel.r, theme.c.panel.g, theme.c.panel.b, glassOn() ? glassA() : 1.0) : "#101018"
    border.width: 1
    border.color: hoverBorder()
    opacity: entered ? 1 : 0

    property bool hovered: hoverArea.containsMouse
    property real lift: (hovered && hoverable) ? -1 : 0
    scale: (hovered && hoverable) ? 1.006 : 1.0
    Behavior on scale { NumberAnimation { duration: 200; easing.type: Easing.OutQuad } }
    transform: Translate { y: root.lift + (root.entered ? 0 : 10) }
    Behavior on lift { NumberAnimation { duration: 160; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
    Behavior on opacity { NumberAnimation { duration: 240; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
    Behavior on border.color { ColorAnimation { duration: 160 } }

    // a single 1px top hairline instead of a gloss band
    Rectangle {
        anchors.top: parent.top
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.margins: 1
        height: 1
        radius: root.radius - 1
        opacity: 0.05
        color: "white"
    }

    Item {
        id: inner
        anchors.fill: parent
    }
    MouseArea {
        id: hoverArea
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.NoButton
    }
    Timer {
        interval: root.enterDelay
        running: true
        repeat: false
        onTriggered: root.entered = true
    }
}
