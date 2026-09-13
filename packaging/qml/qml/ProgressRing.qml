import QtQuick
// Animated progress ring (0..1). Center label via `text`.
Canvas {
    id: root
    property var theme
    property real value: 0
    property real shown: 0
    property string text: ""
    property string sub: ""
    property real thickness: 10
    property color trackColor: theme ? theme.c.edge : "#333344"
    property color ringColor: theme ? theme.c.accent : "#7AA2F7"
    property bool glow: true
    width: 120; height: 120
    renderTarget: Canvas.FramebufferObject
    onValueChanged: anim.start()
    onTrackColorChanged: requestPaint()
    onRingColorChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()
    NumberAnimation {
        id: anim
        target: root
        property: "shown"
        to: root.value
        duration: 600
        easing.type: Easing.BezierSpline
        easing.bezierCurve: [0.4, 0, 0.2, 1]
        onFinished: root.requestPaint()
    }
    onShownChanged: requestPaint()
    Component.onCompleted: { shown = 0; anim.to = value; anim.start(); }
    onPaint: {
        var ctx = getContext("2d");
        ctx.clearRect(0, 0, width, height);
        var cx = width / 2, cy = height / 2;
        var r = Math.min(width, height) / 2 - thickness / 2 - 3;
        ctx.lineCap = "round";
        ctx.lineWidth = thickness;
        ctx.strokeStyle = trackColor;
        ctx.globalAlpha = 0.5;
        ctx.beginPath();
        ctx.arc(cx, cy, r, 0, Math.PI * 2);
        ctx.stroke();
        ctx.globalAlpha = 1.0;
        var a0 = -Math.PI / 2;
        var a1 = a0 + Math.max(0.001, Math.min(1, shown)) * Math.PI * 2;
        if (glow) {
            ctx.save();
            ctx.shadowColor = ringColor;
            ctx.shadowBlur = 12;
            ctx.strokeStyle = ringColor;
            ctx.beginPath();
            ctx.arc(cx, cy, r, a0, a1);
            ctx.stroke();
            ctx.restore();
        } else {
            ctx.strokeStyle = ringColor;
            ctx.beginPath();
            ctx.arc(cx, cy, r, a0, a1);
            ctx.stroke();
        }
        ctx.fillStyle = theme ? theme.c.text : "white";
        ctx.textAlign = "center";
        ctx.font = "700 " + Math.round(width * 0.17) + "px sans-serif";
        ctx.fillText(text, cx, cy + 2);
        if (sub !== "") {
            ctx.fillStyle = theme ? theme.c.muted : "grey";
            ctx.font = Math.round(width * 0.09) + "px sans-serif";
            ctx.fillText(sub, cx, cy + Math.round(width * 0.15));
        }
    }
}
