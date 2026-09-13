import QtQuick
Canvas {
    id: root
    property string mode: "wave"
    property var values: []
    property var segments: []
    property var chartPalette: []
    property color lineColor: "#7AA2F7"
    property color edgeColor: "#414868"
    property color textColor: "#C0CAF5"
    property real hoverIndex: -1
    property real thickness: 22
    property var manager
    property real reveal: 1
    renderTarget: Canvas.FramebufferObject
    renderStrategy: Canvas.Cooperative
    onValuesChanged: { revealAnim.restart(); requestPaint(); }
    onSegmentsChanged: { revealAnim.restart(); requestPaint(); }
    NumberAnimation { id: revealAnim; target: root; property: "reveal"; from: 0.25; to: 1; duration: 500; easing.type: Easing.BezierSpline; easing.bezierCurve: [0.4, 0, 0.2, 1] }
    onLineColorChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()
    onRevealChanged: requestPaint()
    function human(n) {
        var u = ["B", "KB", "MB", "GB", "TB"], i = 0;
        var v = Number(n);
        while (v >= 1024 && i < u.length - 1) { v /= 1024; i++; }
        return v.toFixed(1) + u[i];
    }
    // Every branch must fall through to the single ctx.restore() below: an early
    // return after ctx.save() would grow the canvas state stack on each repaint.
    onPaint: {
        var ctx = getContext("2d");
        ctx.clearRect(0, 0, width, height);
        ctx.save();
        ctx.globalAlpha = Math.min(1, Math.max(0.05, root.reveal));
        ctx.lineJoin = "round";
        ctx.lineCap = "round";
        if (root.mode === "wave" || root.mode === "spark") {
            var n = root.values.length;
            ctx.globalAlpha = 0.5;
            ctx.strokeStyle = root.edgeColor;
            ctx.lineWidth = 1;
            ctx.beginPath();
            ctx.moveTo(0, height - 1);
            ctx.lineTo(width, height - 1);
            ctx.stroke();
            ctx.globalAlpha = Math.min(1, Math.max(0.05, root.reveal));
            if (n >= 2) {
                var grad = ctx.createLinearGradient(0, 0, 0, height);
                grad.addColorStop(0, root.lineColor);
                grad.addColorStop(1, root.edgeColor);
                ctx.strokeStyle = grad;
                ctx.lineWidth = root.mode === "spark" ? 1.4 : 1.8;
                ctx.beginPath();
                var i, x, y;
                for (i = 0; i < n; i++) {
                    x = i / Math.max(1, n - 1) * width;
                    y = height - 3 - (Math.min(100, Math.max(0, Number(root.values[i]))) / 100) * (height - 6);
                    if (i === 0) ctx.moveTo(x, y); else ctx.lineTo(x, y);
                }
                ctx.stroke();
                if (root.mode === "wave") {
                    ctx.lineTo(width, height);
                    ctx.lineTo(0, height);
                    ctx.closePath();
                    ctx.globalAlpha = 0.10;
                    ctx.fillStyle = root.lineColor;
                    ctx.fill();
                    ctx.globalAlpha = Math.min(1, Math.max(0.05, root.reveal));
                }
            }
        } else if (root.mode === "donut") {
            var tot = 0, k;
            for (k = 0; k < root.segments.length; k++) tot += Number(root.segments[k].bytes);
            var cx = 86, cy = height / 2, r = Math.min(72, height / 2 - 8), ir = r - root.thickness;
            var mid = (r + ir) / 2, lw = r - ir;
            if (tot <= 0) {
                // No data yet: a quiet track only. The caller supplies the
                // empty-state message so it matches the rest of the UI.
                ctx.globalAlpha = 0.55;
                ctx.strokeStyle = root.edgeColor;
                ctx.lineWidth = lw;
                ctx.beginPath();
                ctx.arc(cx, cy, mid, 0, Math.PI * 2);
                ctx.stroke();
            } else {
                var a = -Math.PI / 2, idx;
                for (idx = 0; idx < root.segments.length; idx++) {
                    var sw = Number(root.segments[idx].bytes) / tot * Math.PI * 2;
                    var col = root.chartPalette.length > 0 ? root.chartPalette[idx % root.chartPalette.length] : root.lineColor;
                    ctx.strokeStyle = col;
                    ctx.lineWidth = (idx === root.hoverIndex) ? lw + 4 : lw;
                    ctx.beginPath();
                    ctx.arc(cx, cy, mid, a + 0.03, a + sw - 0.03);
                    ctx.stroke();
                    a += sw;
                }
                ctx.globalAlpha = 1;
                ctx.fillStyle = root.textColor;
                ctx.font = "700 18px sans-serif";
                ctx.textAlign = "center";
                ctx.fillText(human(tot), cx, cy + 6);
            }
        }
        ctx.restore();
    }
}
