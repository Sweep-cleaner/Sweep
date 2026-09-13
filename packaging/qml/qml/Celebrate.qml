import QtQuick
// Cleanup celebration: a short burst of physics-based confetti drawn on a
// Canvas. No external assets, no shaders — just particles, gravity and alpha
// fade. `burst()` is called by screens when a clean finishes successfully.
Item {
    id: root
    property var theme
    property bool active: false
    property var particles: []
    property real age: 0
    function colors() {
        return theme
            ? [theme.c.accent, theme.c.accent2, theme.c.success, theme.c.warning]
            : ["#7AA2F7", "#BB9AF7", "#9ECE6A", "#E0AF68"];
    }
    function spawn() {
        var ps = [];
        var cols = colors();
        var i;
        for (i = 0; i < 84; i++) {
            ps.push({
                x: width * 0.5,
                y: height * 0.42,
                vx: (Math.random() - 0.5) * 940,
                vy: -Math.random() * 640 - 140,
                g: 640,
                life: 0.65 + Math.random() * 1.05,
                size: 2.5 + Math.random() * 4.5,
                color: cols[i % cols.length]
            });
        }
        root.particles = ps;
        root.age = 0;
    }
    function burst() {
        if (root.active) return;
        root.active = true;
        spawn();
        canvas.requestPaint();
        tick.start();
    }
    Canvas {
        id: canvas
        anchors.fill: parent
        visible: root.active
        onPaint: {
            var ctx = getContext("2d");
            ctx.clearRect(0, 0, width, height);
            var ps = root.particles;
            var i;
            for (i = 0; i < ps.length; i++) {
                var p = ps[i];
                var a = Math.max(0, 1 - root.age / p.life);
                if (a <= 0) continue;
                ctx.globalAlpha = a;
                ctx.fillStyle = p.color;
                ctx.beginPath();
                ctx.arc(p.x, p.y, p.size, 0, Math.PI * 2);
                ctx.fill();
            }
            ctx.globalAlpha = 1;
        }
    }
    Timer {
        id: tick
        interval: 16
        repeat: true
        onTriggered: {
            root.age += 0.016;
            var ps = root.particles;
            var dt = 0.016;
            var i;
            for (i = 0; i < ps.length; i++) {
                ps[i].vy += ps[i].g * dt;
                ps[i].x += ps[i].vx * dt;
                ps[i].y += ps[i].vy * dt;
            }
            canvas.requestPaint();
            if (root.age > 1.7) {
                tick.stop();
                root.active = false;
            }
        }
    }
}