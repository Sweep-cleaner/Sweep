import QtQuick
import "themes" as Themes
QtObject {
    id: root
    property string current: "Midnight Blue"
    readonly property var order: ["Midnight Blue", "Gruvbox Dark", "Catppuccin Mocha", "Nord", "Dracula", "Tokyo Night", "One Dark", "Solarized Dark", "Monokai Pro", "Everforest", "Rose Pine"]

    // ---- appearance (new) ----
    // Window-level transparency (bound to Window.opacity by Main.qml)
    property real windowOpacity: 1.0
    // Panel translucency: 0.25 (very glassy) .. 1.0 (solid)
    property real glass: 0.82
    // Decorative background effects on/off
    property bool blur: true
    // Motion scale: 0 = off (reduced motion), 1 = normal, up to 2 = lush
    property real animScale: 1.0
    // Corner radius for cards / buttons
    property real radius: 16
    // Empty = use theme accent, otherwise "#rrggbb" override
    property string accentOverride: ""

    readonly property int animMs: animScale <= 0.01 ? 1 : Math.round(220 * animScale)
    readonly property int animFast: animScale <= 0.01 ? 1 : Math.round(140 * animScale)
    readonly property int animSlow: animScale <= 0.01 ? 1 : Math.round(420 * animScale)
    readonly property bool motionOff: animScale <= 0.01
    readonly property color accentEff: accentOverride !== "" ? accentOverride : c.accent
    readonly property color accent2Eff: c.accent2

    property Themes.MidnightBlue midnight: Themes.MidnightBlue {}
    property Themes.GruvboxDark gruvbox: Themes.GruvboxDark {}
    property Themes.CatppuccinMocha catppuccin: Themes.CatppuccinMocha {}
    property Themes.Nord nord: Themes.Nord {}
    property Themes.Dracula dracula: Themes.Dracula {}
    property Themes.TokyoNight tokyo: Themes.TokyoNight {}
    property Themes.OneDark onedark: Themes.OneDark {}
    property Themes.SolarizedDark solarized: Themes.SolarizedDark {}
    property Themes.MonokaiPro monokai: Themes.MonokaiPro {}
    property Themes.Everforest everforest: Themes.Everforest {}
    property Themes.RosePine rosepine: Themes.RosePine {}
    readonly property var c: current === "Midnight Blue" ? midnight : current === "Gruvbox Dark" ? gruvbox : current === "Catppuccin Mocha" ? catppuccin : current === "Nord" ? nord : current === "Dracula" ? dracula : current === "One Dark" ? onedark : current === "Solarized Dark" ? solarized : current === "Monokai Pro" ? monokai : current === "Everforest" ? everforest : current === "Rose Pine" ? rosepine : tokyo
    function names() { return order; }
    function set(name) {
        if (order.indexOf(name) >= 0) current = name;
    }
    function accentFor(name) {
        if (name === "Midnight Blue") return midnight.accent;
        if (name === "Gruvbox Dark") return gruvbox.accent;
        if (name === "Catppuccin Mocha") return catppuccin.accent;
        if (name === "Nord") return nord.accent;
        if (name === "Dracula") return dracula.accent;
        if (name === "One Dark") return onedark.accent;
        if (name === "Solarized Dark") return solarized.accent;
        if (name === "Monokai Pro") return monokai.accent;
        if (name === "Everforest") return everforest.accent;
        if (name === "Rose Pine") return rosepine.accent;
        return tokyo.accent;
    }
    // ---- helpers (new) ----
    // Panel color with glass translucency applied.
    function panel(a) {
        var base = (a === undefined) ? c.panel : a;
        var g = blur ? glass : 1.0;
        return Qt.rgba(base.r, base.g, base.b, g);
    }
    function panel2() {
        var base = c.panel2;
        var g = blur ? Math.min(1.0, glass + 0.06) : 1.0;
        return Qt.rgba(base.r, base.g, base.b, g);
    }
    function withAlpha(col, alpha) {
        return Qt.rgba(col.r, col.g, col.b, alpha);
    }
    function setGlass(v) { glass = Math.min(1.0, Math.max(0.25, v)); }
    function setWindowOpacity(v) { windowOpacity = Math.min(1.0, Math.max(0.35, v)); }
    function setAnimScale(v) { animScale = Math.min(2.0, Math.max(0.0, v)); }
    function toggleBlur() { blur = !blur; }
}
