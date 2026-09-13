import QtQuick
// Calm background: a near-flat base, a very faint grid and three slowly
// drifting accent glows. Kept deliberately low-contrast so the content, not
// the wallpaper, carries the hierarchy.
Item {
    id: root
    property var theme
    property var manager
    property bool animate: true
    property real intensity: 1.0

    function fx() { return (!manager || manager.blur === undefined || manager.blur) ? 1.0 : 0.25; }
    function spd() { return (manager && manager.motionOff) ? 0 : 1; }

    Rectangle {
        anchors.fill: parent
        color: theme ? theme.c.bg : "black"
        Behavior on color { ColorAnimation { duration: 600 } }
    }
    // faint grid
    Canvas {
        id: grid
        anchors.fill: parent
        opacity: 0.03
        onPaint: {
            var ctx = getContext("2d");
            ctx.clearRect(0, 0, width, height);
            ctx.strokeStyle = theme ? theme.c.text : "#ffffff";
            ctx.lineWidth = 1;
            var step = 48, x;
            ctx.beginPath();
            for (x = 0; x < width; x += step) { ctx.moveTo(x, 0); ctx.lineTo(x, height); }
            for (x = 0; x < height; x += step) { ctx.moveTo(0, x); ctx.lineTo(width, x); }
            ctx.stroke();
        }
        Component.onCompleted: requestPaint()
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
    }
    Rectangle {
        id: blobA
        width: 620; height: 620; radius: 310
        color: theme ? theme.c.accent : "#7AA2F7"
        opacity: 0.10 * root.intensity * root.fx()
        Behavior on color { ColorAnimation { duration: 600 } }
        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.rgba(blobA.color.r, blobA.color.g, blobA.color.b, 0.5) }
            GradientStop { position: 1.0; color: "transparent" }
        }
        x: -180; y: -220
    }
    Rectangle {
        id: blobB
        width: 520; height: 520; radius: 260
        color: theme ? theme.c.accent2 : "#BB9AF7"
        opacity: 0.06 * root.intensity * root.fx()
        Behavior on color { ColorAnimation { duration: 600 } }
        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.rgba(blobB.color.r, blobB.color.g, blobB.color.b, 0.5) }
            GradientStop { position: 1.0; color: "transparent" }
        }
        x: parent.width - 300; y: -160
    }
    Rectangle {
        id: blobC
        width: 680; height: 420; radius: 210
        color: theme ? theme.c.success : "#9ECE6A"
        opacity: 0.04 * root.intensity * root.fx()
        Behavior on color { ColorAnimation { duration: 600 } }
        gradient: Gradient {
            GradientStop { position: 0.0; color: Qt.rgba(blobC.color.r, blobC.color.g, blobC.color.b, 0.5) }
            GradientStop { position: 1.0; color: "transparent" }
        }
        x: parent.width * 0.32; y: parent.height - 240
    }
    // vignette
    Rectangle {
        anchors.fill: parent
        gradient: Gradient {
            GradientStop { position: 0.0; color: "transparent" }
            GradientStop { position: 1.0; color: Qt.rgba(0, 0, 0, 0.32) }
        }
    }
    // drift animations (paused when reduced motion)
    SequentialAnimation {
        running: root.animate && root.spd() > 0
        loops: Animation.Infinite
        NumberAnimation { target: blobA; property: "x"; from: -180; to: -80; duration: 14000; easing.type: Easing.InOutSine }
        NumberAnimation { target: blobA; property: "x"; from: -80; to: -180; duration: 14000; easing.type: Easing.InOutSine }
    }
    SequentialAnimation {
        running: root.animate && root.spd() > 0
        loops: Animation.Infinite
        NumberAnimation { target: blobB; property: "y"; from: -160; to: -70; duration: 17000; easing.type: Easing.InOutSine }
        NumberAnimation { target: blobB; property: "y"; from: -70; to: -160; duration: 17000; easing.type: Easing.InOutSine }
    }
    SequentialAnimation {
        running: root.animate && root.spd() > 0
        loops: Animation.Infinite
        NumberAnimation { target: blobC; property: "x"; from: 360; to: 460; duration: 20000; easing.type: Easing.InOutSine }
        NumberAnimation { target: blobC; property: "x"; from: 460; to: 360; duration: 20000; easing.type: Easing.InOutSine }
    }
}
