import QtQuick
// Flat, quiet button. Primary fills with the accent; secondary is a surface
// with an edge. `busy` shows a progress fill and locks the pointer.
Rectangle {
    id: root
    property string label: ""
    property var theme
    property var manager
    property bool primary: false
    property bool danger: false
    property bool armed: false
    property bool busy: false
    property real progress: 0
    signal clicked
    height: 38
    function effR() { return (manager && manager.radius !== undefined) ? Math.max(10, manager.radius - 5) : 11; }
    radius: effR()
    property color base: (danger && armed) ? theme.c.danger : primary ? theme.c.accent : Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.9)
    property color hover: (danger && armed) ? Qt.lighter(theme.c.danger, 1.12) : primary ? Qt.lighter(theme.c.accent, 1.08) : theme.c.edge
    color: pressArea.pressed ? Qt.darker(base, 1.25) : pressArea.containsMouse ? hover : base
    scale: pressArea.pressed ? 0.985 : 1.0
    Behavior on color { ColorAnimation { duration: 150; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
    border.width: 1
    border.color: primary ? Qt.rgba(theme.c.accentText.r, theme.c.accentText.g, theme.c.accentText.b, 0.16) : (danger && !armed) ? theme.c.danger : (pressArea.containsMouse ? theme.c.accent : theme.c.edge)
    Behavior on border.color { ColorAnimation { duration: 150 } }
    clip: true
    // Basma noktasından yayılan dalga: cap, o noktadan en uzak koseyi
    // ortecek buyuklukte secilir; boylece nereye basilirsa basilsin dalga
    // butonu tam doldurur, disari tasan kirpilir (clip).
    Rectangle {
        id: ripple
        width: 10
        height: 10
        radius: 5
        color: (root.primary || (root.danger && root.armed)) ? theme.c.accentText : theme.c.accent
        opacity: 0
        scale: 0.1
        visible: opacity > 0.02
        Behavior on opacity { NumberAnimation { duration: 340; easing.type: Easing.OutCubic } }
        Behavior on scale { NumberAnimation { duration: 340; easing.type: Easing.OutCubic } }
    }
    function rippleFrom(px, py) {
        var dx = Math.max(px, root.width - px);
        var dy = Math.max(py, root.height - py);
        var d = Math.ceil(2 * Math.sqrt(dx * dx + dy * dy));
        ripple.width = d;
        ripple.height = d;
        ripple.radius = d / 2;
        ripple.x = px - d / 2;
        ripple.y = py - d / 2;
    }
    SequentialAnimation {
        id: rippleAnim
        ParallelAnimation {
            NumberAnimation { target: ripple; property: "opacity"; to: 0.4; duration: 80 }
            NumberAnimation { target: ripple; property: "scale"; to: 1.0; duration: 320; easing.type: Easing.OutCubic }
        }
        NumberAnimation { target: ripple; property: "opacity"; to: 0; duration: 260; easing.type: Easing.InCubic }
    }
    // bırakınca hafif spring sıçraması
    SequentialAnimation {
        id: springAnim
        NumberAnimation { target: root; property: "scale"; to: 1.03; duration: 90; easing.type: Easing.OutQuad }
        NumberAnimation { target: root; property: "scale"; to: 1.0; duration: 280; easing.type: Easing.OutBack }
    }
    // progress fill (only while busy)
    Rectangle {
        width: parent.width * Math.min(1, Math.max(0, root.progress))
        height: parent.height
        color: theme.c.accentText
        opacity: 0.16
        visible: root.busy
    }
    Text {
        anchors.centerIn: parent
        text: root.busy ? Math.round(root.progress * 100) + "%" : root.label
        font.pixelSize: 13
        font.weight: Font.DemiBold
        color: (root.primary || (root.danger && root.armed)) ? theme.c.accentText : theme.c.text
    }
    MouseArea {
        id: pressArea
        anchors.fill: parent
        hoverEnabled: true
        enabled: !root.busy
        onPressed: function(mouse) {
            rippleFrom(mouse.x, mouse.y);
            rippleAnim.restart();
        }
        onReleased: springAnim.restart()
        onClicked: root.clicked()
    }
}
