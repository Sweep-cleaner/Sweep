import QtQuick
Item {
    id: root
    property var theme
    property var manager
    property real from: 0
    property real to: 100
    property real value: 50
    signal moved(real v)
    // Vurus alani: Ayarlar Flickable icinde ince slider tutturmak zordu;
    // gorsel ayni kalir, yalniz tiklanabilir yukseklik buyur.
    height: 40
    width: 220
    property real frac: (root.value - root.from) / Math.max(0.0001, root.to - root.from)
    Rectangle {
        id: track
        height: dragArea.pressed || dragArea.containsMouse ? 7 : 5
        radius: 4
        anchors.verticalCenter: parent.verticalCenter
        anchors.left: parent.left
        anchors.right: parent.right
        color: theme.c.edge
        opacity: 0.75
        Behavior on height { NumberAnimation { duration: 120 } }
        Rectangle {
            width: Math.min(track.width, Math.max(0, track.width * root.frac))
            height: parent.height
            radius: 4
            color: theme.c.accent
        }
        // glow tip
        Rectangle {
            width: 26; height: parent.height + 8
            radius: 6
            x: Math.min(track.width - 26, Math.max(0, track.width * root.frac - 13))
            y: -4
            color: theme.c.accent
            opacity: 0.25
        }
    }
    Rectangle {
        id: knob
        width: dragArea.pressed ? 22 : 19
        height: dragArea.pressed ? 22 : 19
        radius: width / 2
        anchors.verticalCenter: parent.verticalCenter
        x: Math.min(track.width - width / 2, Math.max(-width / 2, track.width * root.frac - width / 2))
        color: dragArea.pressed ? Qt.lighter(theme.c.accent, 1.2) : theme.c.accent
        border.width: 2
        border.color: theme.c.accentText
        Behavior on width { NumberAnimation { duration: 110 } }
        Behavior on height { NumberAnimation { duration: 110 } }
    }
    MouseArea {
        id: dragArea
        anchors.fill: parent
        hoverEnabled: true
        preventStealing: true
        onPressed: function(m) { update(m.x); }
        onPositionChanged: function(m) { if (pressed) update(m.x); }
        function update(px) {
            var t = Math.min(track.width, Math.max(0, px));
            root.value = root.from + t / Math.max(1, track.width) * (root.to - root.from);
            root.moved(root.value);
        }
    }
}
