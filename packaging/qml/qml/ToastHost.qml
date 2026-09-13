import QtQuick
// Stacked toast notifications, bottom-right. Usage: toastHost.show("msg", "ok|warn|bad|info")
Item {
    id: root
    property var theme
    property var manager
    width: 340
    function show(msg, level) {
        if (model.count >= 4) model.remove(0);
        model.append({ msg: String(msg).slice(0, 220), level: level || "info" });
    }
    function showOk(m) { show(m, "ok"); }
    function showWarn(m) { show(m, "warn"); }
    function showBad(m) { show(m, "bad"); }
    ListModel { id: model }
    Column {
        anchors.bottom: parent.bottom
        anchors.right: parent.right
        spacing: 10
        Repeater {
            model: model
            delegate: Rectangle {
                id: card
                width: 320
                height: Math.max(52, label.paintedHeight + 26)
                radius: 12
                color: theme ? Qt.rgba(theme.c.panel2.r, theme.c.panel2.g, theme.c.panel2.b, 0.95) : "#181824"
                border.width: 1
                border.color: model.level === "bad" ? theme.c.danger : model.level === "warn" ? theme.c.warning : model.level === "ok" ? theme.c.success : theme.c.edge
                opacity: 0
                transform: Translate { id: tr; x: 48 }
                Timer { id: kill; interval: 3400; running: true; repeat: false; onTriggered: { card.opacity = 0; tr.x = 48; bye.start(); } }
                Timer { id: bye; interval: 260; running: false; repeat: false; onTriggered: model.remove(index) }
                Component.onCompleted: { card.opacity = 1; tr.x = 0; }
                Behavior on opacity { NumberAnimation { duration: 220; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] } }
                Behavior on x { enabled: false }
                Row {
                    anchors.fill: parent
                    anchors.margins: 12
                    spacing: 10
                    Rectangle {
                        width: 8; height: 8; radius: 4
                        anchors.verticalCenter: parent.verticalCenter
                        color: model.level === "bad" ? theme.c.danger : model.level === "warn" ? theme.c.warning : model.level === "ok" ? theme.c.success : theme.c.accent
                    }
                    Text {
                        id: label
                        width: parent.width - 40
                        text: model.msg
                        font.pixelSize: 13
                        color: theme.c.text
                        wrapMode: Text.Wrap
                        anchors.verticalCenter: parent.verticalCenter
                    }
                }
                MouseArea { anchors.fill: parent; onClicked: model.remove(index) }
            }
        }
    }
}
