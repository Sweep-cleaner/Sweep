import QtQuick
// Text that re-reads its copy from the i18n dictionary whenever the language
// changes. Usage: I18nText { key: "d.reclaim" } — all other Text properties
// (font, color, ...) pass through as usual.
Text {
    id: root
    property string key: ""
    property string txt: ""
    function refresh() {
        root.txt = (i18n && typeof i18n.tr === "function") ? i18n.tr(root.key) : root.key;
    }
    text: root.txt
    Component.onCompleted: refresh()
    Connections {
        target: i18n
        function onLangChanged() { root.refresh(); }
    }
}