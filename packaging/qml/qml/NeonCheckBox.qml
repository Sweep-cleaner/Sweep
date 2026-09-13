import QtQuick
Item {
    id: root
    property var theme
    property var manager
    property bool checked: false
    property string text: ""
    signal toggled(bool on)
    height: 30
    width: parent ? parent.width : 220
    property real drawProgress: checked ? 1 : 0
    Behavior on drawProgress { NumberAnimation { duration: 180; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
    scale: pressArea.pressed ? 0.98 : 1.0
    Behavior on scale { NumberAnimation { duration: 110 } }
    Row {
        anchors.verticalCenter: parent.verticalCenter
        spacing: 11
        Rectangle {
                id: box
                width: 20
                height: 20
                radius: 7
                anchors.verticalCenter: parent.verticalCenter
                // işaretlenince küçük bir spring pop
                SequentialAnimation {
                    id: popAnim
                    NumberAnimation { target: box; property: "scale"; to: 1.25; duration: 110; easing.type: Easing.OutQuad }
                    NumberAnimation { target: box; property: "scale"; to: 1.0; duration: 250; easing.type: Easing.OutBack }
                }
            color: root.checked ? theme.c.accent : Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.7)
            border.width: 1.5
            border.color: root.checked ? theme.c.accent : (hoverBox ? theme.c.accent : theme.c.muted)
            property bool hoverBox: pressArea.containsMouse
            Behavior on color { ColorAnimation { duration: 150; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
            Behavior on border.color { ColorAnimation { duration: 150 } }
            Canvas {
                id: tick
                anchors.fill: parent
                anchors.margins: 3
                onPaint: {
                    var ctx = getContext("2d");
                    ctx.clearRect(0, 0, width, height);
                    ctx.strokeStyle = theme.c.accentText;
                    ctx.lineWidth = 2.4;
                    ctx.lineCap = "round";
                    ctx.lineJoin = "round";
                    var p = root.drawProgress;
                    if (p <= 0.01) return;
                    ctx.beginPath();
                    ctx.moveTo(width * 0.18, height * 0.52);
                    ctx.lineTo(width * 0.44, height * 0.74);
                    ctx.stroke();
                    if (p > 0.45) {
                        ctx.beginPath();
                        ctx.moveTo(width * 0.44, height * 0.74);
                        var t = (p - 0.45) / 0.55;
                        ctx.lineTo(width * 0.44 + (width * 0.82 - width * 0.44) * t, height * 0.74 + (height * 0.24 - height * 0.74) * t);
                        ctx.stroke();
                    }
                }
            }
        }
        Text {
            text: root.text
            font.pixelSize: 14
            color: pressArea.containsMouse ? theme.c.text : theme.c.text
            anchors.verticalCenter: parent.verticalCenter
            elide: Text.ElideRight
            width: root.width - 44
            opacity: root.checked ? 1.0 : 0.88
        }
    }
    onDrawProgressChanged: tick.requestPaint()
    onCheckedChanged: { if (root.checked) popAnim.restart(); tick.requestPaint(); }
    MouseArea {
        id: pressArea
        anchors.fill: parent
        hoverEnabled: true
        onClicked: { root.checked = !root.checked; root.toggled(root.checked); }
    }
}
