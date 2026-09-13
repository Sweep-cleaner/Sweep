import QtQuick
// Compact metric tile: an uppercase label, a value, and either a sparkline or
// a progress bar. Set `progress` >= 0 to switch to the bar — used for metrics
// that have no history yet (a one-point sparkline renders as a dead flat line).
Rectangle {
    id: root
    property var theme
    property var manager
    property string title: ""
    property string value: ""
    property var spark: []
    property real progress: -1
    default property alias body: inner.children
    function effR() { return (manager && manager.radius !== undefined) ? manager.radius : 14; }
    function hoverBorder() {
        if (!theme) return "#333344";
        if (!hoverArea.containsMouse) return theme.c.edge;
        return Qt.rgba(theme.c.accent.r, theme.c.accent.g, theme.c.accent.b, 0.5);
    }
    radius: effR()
    color: theme ? Qt.rgba(theme.c.panel.r, theme.c.panel.g, theme.c.panel.b, (manager && manager.glass !== undefined) ? manager.glass : 0.82) : "#14141c"
    border.width: 1
    border.color: hoverBorder()
    Behavior on border.color { ColorAnimation { duration: 160 } }
    scale: hoverArea.containsMouse ? 1.012 : 1.0
    Behavior on scale { NumberAnimation { duration: 200; easing.type: Easing.OutQuad } }
    transform: Translate { id: lift; y: hoverArea.containsMouse ? -2 : 0; Behavior on y { NumberAnimation { duration: 160; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } } }
    Column {
        id: inner
        anchors.fill: parent
        anchors.margins: 14
        spacing: 6
        Text { text: root.title; font.pixelSize: 11; font.weight: Font.DemiBold; font.letterSpacing: 0.6; color: theme.c.muted }
        Text { text: root.value; font.pixelSize: 21; font.weight: Font.DemiBold; color: theme.c.text }
        ChartComponents {
            visible: root.progress < 0
            width: parent.width
            height: 34
            mode: "spark"
            values: root.spark
            lineColor: theme.c.accent
            edgeColor: theme.c.edge
        }
        Item {
            visible: root.progress >= 0
            width: parent.width
            height: 34
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width
                height: 6
                radius: 3
                color: theme.c.edge
                Rectangle {
                    width: parent.width * Math.min(1, Math.max(0, root.progress))
                    height: parent.height
                    radius: 3
                    color: theme.c.accent
                    Behavior on width { NumberAnimation { duration: 500; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
                }
            }
        }
    }
    MouseArea { id: hoverArea; anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.NoButton }
}
