#include <QApplication>
#include <QCoreApplication>
#include <QQmlApplicationEngine>
#include <QMetaObject>
#include <QQmlContext>
#include <QQuickWindow>
#include <QOpenGLContext>
#include <QSGRendererInterface>
#include <QObject>
#include <QDir>
#include <QProcess>
#include <QTimer>
#include <QFile>
#include <QFileInfo>
#include <QFont>
#include <QIcon>
#include <QImage>
#include <QDateTime>
#include <QStandardPaths>
#include <QStorageInfo>
#include <QDesktopServices>
#include <QUrl>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMutexLocker>
#include <QMutex>
#include <QTextStream>
#include <QSysInfo>
#include <QSettings>
#include <QLockFile>
#include <QAbstractNativeEventFilter>
#include <QSystemTrayIcon>
#include <QMenu>
#include <QAction>
#include <QOperatingSystemVersion>
#include <QHash>
#include <QVector>
#include <QVariantList>
#include <cstdlib>

#ifdef Q_OS_WIN
#ifndef NOMINMAX
#define NOMINMAX
#endif
#include <windows.h>
#include <shellapi.h>
#include <dwmapi.h>
#ifdef _MSC_VER
#pragma comment(lib, "dwmapi.lib")
#pragma comment(lib, "shell32.lib")
#endif
#endif

#ifndef Q_OS_WIN
#include <sys/statvfs.h>
#endif

#ifdef Q_OS_WIN
// Tek GUI örneği: ikinci süreç mutex/kilitte durur, gizli tepsi
// penceresine Show mesajı yollar, çıkar. Prop pencere oluşunca set edilir.
static const wchar_t kSweepProp[] = L"org.sweep.Sweep";

static UINT sweepShowMessage()
{
    static const UINT m = RegisterWindowMessageW(L"org.sweep.Sweep.Show");
    return m;
}

static BOOL CALLBACK findSweepHwndProc(HWND hwnd, LPARAM lp)
{
    if (GetPropW(hwnd, kSweepProp)) {
        *reinterpret_cast<HWND *>(lp) = hwnd;
        return FALSE;
    }
    return TRUE;
}

static HWND findSweepHwnd()
{
    HWND found = nullptr;
    EnumWindows(findSweepHwndProc, reinterpret_cast<LPARAM>(&found));
    return found;
}

static void wakeExistingSweep()
{
    const HWND hwnd = findSweepHwnd();
    if (!hwnd)
        return;
    DWORD pid = 0;
    GetWindowThreadProcessId(hwnd, &pid);
    if (pid)
        AllowSetForegroundWindow(pid);
    PostMessageW(hwnd, sweepShowMessage(), 0, 0);
}
#endif

// ponytail: env + göreli yol araması; bulamazsa PATH'teki `sweep`e düşer
static QString resolveSweepBin() {
    if (const char *e = std::getenv("SWEEP_BIN"))
        if (QFileInfo(QString::fromLocal8Bit(e)).isExecutable()) return QString::fromLocal8Bit(e);
    const QString here = QCoreApplication::applicationDirPath();
#ifdef Q_OS_WIN
    // Windows: exe yanındaki sweep.exe, sonra Program Files, sonra PATH.
    if (QFileInfo(here + QStringLiteral("/sweep.exe")).isFile())
        return here + QStringLiteral("/sweep.exe");
    for (const char *pf : {"ProgramFiles", "ProgramFiles(x86)"}) {
        if (const char *base = std::getenv(pf)) {
            const QString c = QString::fromLocal8Bit(base)
                + QStringLiteral("/sweep/sweep.exe");
            if (QFileInfo(c).isFile()) return c;
        }
    }
    return QStringLiteral("sweep.exe"); // PATH'e bırak
#else
    for (const QString &c : {here + QStringLiteral("/sweep"),
                             QStringLiteral("/usr/local/bin/sweep")}) {
        if (QFileInfo(c).isExecutable()) return c;
    }
    return QStringLiteral("sweep"); // PATH'e bırak
#endif
}

// ---------------------------------------------------------------------------
// AppSettings: tema / saydamlık / hareket / dil kalıcılığı (QSettings).
// QML'de `settings` olarak kullanılır; ThemeManager buradan yüklenir ve
// her değişiklik anında diske yazılır.
// ---------------------------------------------------------------------------
class AppSettings : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString theme READ theme WRITE setTheme NOTIFY themeChanged)
    Q_PROPERTY(QString accent READ accent WRITE setAccent NOTIFY accentChanged)
    Q_PROPERTY(double windowOpacity READ windowOpacity WRITE setWindowOpacity NOTIFY windowOpacityChanged)
    Q_PROPERTY(double glass READ glass WRITE setGlass NOTIFY glassChanged)
    Q_PROPERTY(bool blur READ blur WRITE setBlur NOTIFY blurChanged)
    Q_PROPERTY(double animScale READ animScale WRITE setAnimScale NOTIFY animScaleChanged)
    Q_PROPERTY(QString language READ language WRITE setLanguage NOTIFY languageChanged)
    Q_PROPERTY(bool minimizeToTray READ minimizeToTray WRITE setMinimizeToTray NOTIFY minimizeToTrayChanged)
    Q_PROPERTY(bool autoMem READ autoMem WRITE setAutoMem NOTIFY autoMemChanged)
    Q_PROPERTY(bool quitting READ quitting WRITE setQuitting NOTIFY quittingChanged)
    Q_PROPERTY(bool welcomed READ welcomed WRITE setWelcomed NOTIFY welcomedChanged)
public:
    explicit AppSettings(QObject *parent = nullptr)
        : QObject(parent), m_s(QStringLiteral("Sweep"), QStringLiteral("Sweep")) {
        m_theme = m_s.value(QStringLiteral("theme"), QStringLiteral("Midnight Blue")).toString();
        m_accent = m_s.value(QStringLiteral("accent"), QString()).toString();
        m_windowOpacity = m_s.value(QStringLiteral("windowOpacity"), 1.0).toDouble();
        m_glass = m_s.value(QStringLiteral("glass"), 0.82).toDouble();
        m_blur = m_s.value(QStringLiteral("blur"), true).toBool();
        m_animScale = m_s.value(QStringLiteral("animScale"), 1.0).toDouble();
        m_language = m_s.value(QStringLiteral("language"), QStringLiteral("tr")).toString();
        m_minimizeToTray = m_s.value(QStringLiteral("minimizeToTray"), true).toBool();
        m_autoMem = m_s.value(QStringLiteral("autoMem"), false).toBool();
        m_welcomed = m_s.value(QStringLiteral("welcomed"), false).toBool();
    }
    QString theme() const { return m_theme; }
    QString accent() const { return m_accent; }
    double windowOpacity() const { return m_windowOpacity; }
    double glass() const { return m_glass; }
    bool blur() const { return m_blur; }
    double animScale() const { return m_animScale; }
    QString language() const { return m_language; }
    bool minimizeToTray() const { return m_minimizeToTray; }
    bool quitting() const { return m_quitting; }
    bool welcomed() const { return m_welcomed; }
    void setTheme(const QString &v) { if (v == m_theme) return; m_theme = v; m_s.setValue(QStringLiteral("theme"), v); emit themeChanged(); }
    void setAccent(const QString &v) { if (v == m_accent) return; m_accent = v; m_s.setValue(QStringLiteral("accent"), v); emit accentChanged(); }
    void setWindowOpacity(double v) { if (qFuzzyCompare(v, m_windowOpacity)) return; m_windowOpacity = v; m_s.setValue(QStringLiteral("windowOpacity"), v); emit windowOpacityChanged(); }
    void setGlass(double v) { if (qFuzzyCompare(v, m_glass)) return; m_glass = v; m_s.setValue(QStringLiteral("glass"), v); emit glassChanged(); }
    void setBlur(bool v) { if (v == m_blur) return; m_blur = v; m_s.setValue(QStringLiteral("blur"), v); emit blurChanged(); }
    void setAnimScale(double v) { if (qFuzzyCompare(v, m_animScale)) return; m_animScale = v; m_s.setValue(QStringLiteral("animScale"), v); emit animScaleChanged(); }
    void setLanguage(const QString &v) { if (v == m_language) return; m_language = v; m_s.setValue(QStringLiteral("language"), v); emit languageChanged(); }
    void setMinimizeToTray(bool v) { if (v == m_minimizeToTray) return; m_minimizeToTray = v; m_s.setValue(QStringLiteral("minimizeToTray"), v); emit minimizeToTrayChanged(); }
    bool autoMem() const { return m_autoMem; }
    void setAutoMem(bool v) { if (v == m_autoMem) return; m_autoMem = v; m_s.setValue(QStringLiteral("autoMem"), v); emit autoMemChanged(); }
    void setQuitting(bool v) { if (v == m_quitting) return; m_quitting = v; emit quittingChanged(); }
    void setWelcomed(bool v) { if (v == m_welcomed) return; m_welcomed = v; m_s.setValue(QStringLiteral("welcomed"), v); emit welcomedChanged(); }
signals:
    void themeChanged();
    void accentChanged();
    void windowOpacityChanged();
    void glassChanged();
    void blurChanged();
    void animScaleChanged();
    void languageChanged();
    void minimizeToTrayChanged();
    void autoMemChanged();
    void quittingChanged();
    void welcomedChanged();
private:
    QSettings m_s;
    QString m_theme, m_accent, m_language;
    double m_windowOpacity, m_glass, m_animScale;
    bool m_blur, m_minimizeToTray, m_autoMem = false, m_quitting = false, m_welcomed = false;
};

// ---------------------------------------------------------------------------
// Translator: 8 dil sözlüğü (en/tr/es/ru/fr/de/pt/it); QML'de `i18n.tr(key)`.
// Bilinmeyen dil İngilizce'ye düşer (eksik anahtarlar da).
// ---------------------------------------------------------------------------
class Translator : public QObject {
    Q_OBJECT
    Q_PROPERTY(QString lang READ lang WRITE setLang NOTIFY langChanged)
public:
    explicit Translator(QObject *parent = nullptr) : QObject(parent) {
        const char *en[][2] = {
            {"side.subtitle", "SYSTEM CLEANER"},
            {"side.menu", "MENU"},
            {"side.analysis", "ANALYSIS & SETUP"},
            {"side.storage", "STORAGE"},
            {"side.diskUsage", "Disk usage"},
            {"side.full", "full"},
            {"side.theme", "Theme"},
            {"side.changeTheme", "Change theme"},
            {"t.dashboard", "Dashboard"},
            {"t.cleaner", "Cleaner"},
            {"t.history", "History"},
            {"t.system", "System"},
            {"t.files", "Files"},
            {"t.disk", "Disk"},
            {"t.optimize", "Memory"},
            {"t.settings", "Settings"},
            {"s.dashboard", "System status overview"},
            {"s.cleaner", "Reclaim disk space safely"},
            {"s.history", "Past cleaning operations"},
            {"s.system", "Live hardware pulse"},
            {"s.files", "Hunt big and duplicate files"},
            {"s.disk", "Folder sizes at a glance"},
            {"disk.up", "Up"},
            {"disk.volumes", "Volumes"},
            {"disk.empty", "Empty folder"},
            {"disk.cancel", "Stop"},
            {"s.optimize", "Reduce memory pressure"},
            {"s.settings", "Theme, transparency and motion"},
            {"d.reclaim", "RECLAIMABLE SPACE"},
            {"d.ready", "READY"},
            {"d.scanning", "SCANNING"},
            {"d.analyzing", "Analyzing…"},
            {"d.scanAll", "Scan all"},
            {"d.scanning2", "Scanning…"},
            {"d.openCleaner", "Open cleaner"},
            {"d.quickScan", "Quick scan"},
            {"d.quickScanSub", "Preview everything"},
            {"d.cleaner", "Cleaner"},
            {"d.cleanerSub", "Select and clean"},
            {"d.appearance", "Appearance"},
            {"d.appearanceSub", "Theme and transparency"},
            {"d.system", "SYSTEM"},
            {"d.live", "LIVE"},
            {"d.charts", "CHARTS"},
            {"d.distribution", "Disk distribution"},
            {"d.wave", "CPU / RAM wave"},
            {"d.noData", "No data yet"},
            {"d.noDataSub", "Run a scan to see reclaimable space."},
            {"d.lastReport", "Last report:"},
            {"d.diskFull", "disk full"},
            {"t.schedule", "Schedule"},
            {"s.schedule", "Automatic daily cleanup"},
            {"sched.eyebrow", "SCHEDULED CLEANUP"},
            {"sched.title", "Sweep on autopilot"},
            {"sched.sub", "A quiet clean, every day"},
            {"sched.backend", "Backend"},
            {"sched.status", "Status"},
            {"sched.hour", "Hour"},
            {"sched.selection", "Cleaners"},
            {"sched.hint", "e.g. firefox.cache (empty = default)"},
            {"sched.enable", "Enable daily"},
            {"sched.disable", "Disable"},
            {"sched.working", "Working…"},
            {"sched.refresh", "Refresh"},
            {"sched.memopt", "Memory task"},
            {"t.memory", "Memory"},
            {"s.memory", "Live memory usage and optimization"},
            {"t.startup_mgr", "Startup"},
            {"s.startup_mgr", "What launches when you log in"},
            {"t.sysinfo", "System health"},
            {"s.sysinfo", "CPU, disks, SMART and temperature"},
            {"t.network", "Network"},
            {"s.network", "DNS and package caches"},
            {"t.privacy", "Privacy"},
            {"s.privacy", "Telemetry, recent files and clipboard"},
            {"g.optimize", "Optimize"},
            {"g.optimize_aggressive", "Aggressive mode (swap/standby)"},
            {"g.impact_high", "High impact"},
            {"g.impact_medium", "Medium impact"},
            {"g.impact_low", "Low impact"},
            {"g.flush_dns", "Flush DNS"},
            {"g.clean_caches", "Clean package caches"},
            {"g.shield", "Disable telemetry"},
            {"g.wipe", "Wipe history & clipboard"},
            {"g.search_startup", "Search startup entries…"},
            {"g.health_green", "Healthy"},
            {"g.health_yellow", "Warning"},
            {"g.health_red", "Critical"},
            {"g.uptime", "Uptime"},
            {"g.temperature", "Temperature"},
            {"g.dry_run", "Dry run (preview)"},
            {"g.recent_files", "Recent files"},
            {"g.clipboard", "Clipboard"},
            {"g.working", "Working…"},
            {"g.refresh", "Refresh"},
            {"g.close", "Disable"},
            {"g.open", "Enable"},
            {"g.cores", "cores"},
            {"g.cpu", "CPU"},
            {"g.memory", "Memory"},
            {"g.swap", "Swap"},
            {"reclaimed", "reclaimed"},
            {"g.needs_root", "Root/administrator password required"},
            {"g.elev_retry", "Retry as admin"},
            {"w.title", "Welcome to Sweep"},
            {"w.sub", "Your PC, clean and under control."},
            {"w.tour", "Take a tour"},
            {"w.skip", "Skip"},
            {"w.next", "Next"},
            {"w.back", "Back"},
            {"w.done", "Done"},
            {"g.no_engine", "Cleanup engine (sweep) not found — install it next to the app to enable optimization."},
            {"g.apply", "Apply"},
            {"g.automem", "Automatic maintenance"},
            {"g.automem_desc", "Checks every 5 min, trims when RAM passes 80% (safe, no admin)."},
            {"g.smart", "SMART"},
            {"a.all", "All"},
            {"a.author", "Author"},
            {"u.undo_available", "Undo available"},
            {"u.undo", "Undo"},
            {"u.undo_last", "Undo last clean"},
            {"u.restore", "Restore"},
            {"u.runs_title", "Previous cleans"},
            {"u.no_runs", "No previous cleans yet."},
            {"u.backup_title", "Create a backup first?"},
            {"u.backup_body", "We recommend creating a system backup before continuing."},
            {"u.create_backup", "Create Backup"},
            {"u.continue", "Continue"},
            {"u.review_first", "Review the preview, then press Clean again."},
            {"u.backup_open_failed", "Could not open system backup settings. Please back up manually."},
            {"a.backup", "Backup"},
            {"a.cancel", "Cancel"},
            {"a.capabilities", "CAPABILITIES"},
            {"a.category", "CATEGORY"},
            {"a.category_driver", "Driver"},
            {"a.category_os", "OS"},
            {"a.category_security", "Security"},
            {"a.category_third_party", "Third-party"},
            {"a.category_unknown", "Unknown"},
            {"a.category_updater", "Updater"},
            {"a.clear_selection", "Clear"},
            {"a.command", "Command"},
            {"a.confirm", "Confirm"},
            {"a.confirm_ack", "I understand the risk"},
            {"a.confirm_body", "This changes the system configuration. Continue?"},
            {"a.confirm_critical", "Critical risk: disabling or removing this can break startup or security."},
            {"a.confirm_title", "Confirm change"},
            {"a.detail", "DETAIL"},
            {"a.edit", "Edit command"},
            {"a.edit_label", "New command"},
            {"a.edit_title", "Edit command"},
            {"a.editable", "Editable"},
            {"a.empty", "No startup entries found"},
            {"a.empty_hint", "Try loosening the filters."},
            {"a.error", "Error"},
            {"a.export_csv", "Export CSV"},
            {"a.export_json", "Export JSON"},
            {"a.exported", "Exported"},
            {"a.eyebrow", "STARTUP & TASKS"},
            {"a.history_empty", "No audit entries yet"},
            {"a.kind", "KIND"},
            {"a.kind_all", "All"},
            {"a.kind_startup", "Startup"},
            {"a.kind_task", "Scheduled"},
            {"a.last_run", "Last run"},
            {"a.location", "Location"},
            {"a.no", "No"},
            {"a.no_selection", "Select an item to see its details."},
            {"a.notes", "Notes"},
            {"a.reason", "Reason"},
            {"a.removable", "Removable"},
            {"a.remove", "Remove"},
            {"a.reversible", "Reversible"},
            {"a.risk", "RISK"},
            {"a.risk_critical", "Critical"},
            {"a.risk_high", "High"},
            {"a.risk_low", "Low"},
            {"a.risk_medium", "Medium"},
            {"a.rollback", "Roll back"},
            {"a.rollback_last", "Roll back last change"},
            {"a.save", "Save"},
            {"a.scope", "SCOPE"},
            {"a.scope_system", "System"},
            {"a.scope_user", "User"},
            {"a.search", "Search name, command or location…"},
            {"a.select_all", "Select all"},
            {"a.selected", "selected"},
            {"a.sort", "Sort"},
            {"a.sort_impact", "Impact"},
            {"a.sort_name", "Name"},
            {"a.sort_source", "Source"},
            {"a.state", "STATE"},
            {"a.state_off", "Disabled"},
            {"a.state_on", "Enabled"},
            {"a.status_items", "items"},
            {"a.tab_history", "Audit history"},
            {"a.tab_list", "List"},
            {"a.trigger", "Trigger"},
            {"a.trigger_boot", "Boot"},
            {"a.trigger_event", "Event"},
            {"a.trigger_idle", "Idle"},
            {"a.trigger_logon", "Logon"},
            {"a.trigger_schedule", "Schedule"},
            {"a.trigger_unknown", "Unknown"},
            {"a.yes", "Yes"},
            {"s.autostart", "Everything that starts on its own: autostart entries and scheduled tasks"},
            {"t.autostart_nav", "Startup & Tasks"},
            {"s.autostart_nav", "Autostart and scheduled tasks"},
            {"t.autostart", "Startup & Scheduled Tasks"},
        };
        const char *tr[][2] = {
            {"side.subtitle", "SİSTEM TEMİZLEYİCİ"},
            {"side.menu", "MENÜ"},
            {"side.analysis", "ANALİZ & AYAR"},
            {"side.storage", "DEPOLAMA"},
            {"side.diskUsage", "Disk kullanımı"},
            {"side.full", "dolu"},
            {"side.theme", "Tema"},
            {"side.changeTheme", "Temayı değiştir"},
            {"t.dashboard", "Panel"},
            {"t.cleaner", "Temizleyici"},
            {"t.history", "Geçmiş"},
            {"t.system", "Sistem"},
            {"t.files", "Dosyalar"},
            {"t.disk", "Disk"},
            {"t.optimize", "Bellek"},
            {"t.settings", "Ayarlar"},
            {"s.dashboard", "Sistem durumuna genel bakış"},
            {"s.cleaner", "Disk alanını güvenle geri kazan"},
            {"s.history", "Geçmiş temizlik işlemleri"},
            {"s.system", "Canlı donanım nabzı"},
            {"s.files", "Büyük ve yinelenen dosya avı"},
            {"s.disk", "Klasör boyutları bir bakışta"},
            {"disk.up", "Yukarı"},
            {"disk.volumes", "Birimler"},
            {"disk.empty", "Boş klasör"},
            {"disk.cancel", "Durdur"},
            {"s.optimize", "Bellek baskısını azalt"},
            {"s.settings", "Tema, saydamlık ve hareket"},
            {"d.reclaim", "GERİ KAZANILABİLİR ALAN"},
            {"d.ready", "HAZIR"},
            {"d.scanning", "TARANIYOR"},
            {"d.analyzing", "Analiz ediliyor…"},
            {"d.scanAll", "Tümünü tara"},
            {"d.scanning2", "Taranıyor…"},
            {"d.openCleaner", "Temizleyiciyi aç"},
            {"d.quickScan", "Hızlı tara"},
            {"d.quickScanSub", "Tümünü önizle"},
            {"d.cleaner", "Temizleyici"},
            {"d.cleanerSub", "Seç ve temizle"},
            {"d.appearance", "Görünüm"},
            {"d.appearanceSub", "Tema ve saydamlık"},
            {"d.system", "SİSTEM"},
            {"d.live", "CANLI"},
            {"d.charts", "GRAFİKLER"},
            {"d.distribution", "Disk dağılımı"},
            {"d.wave", "CPU / RAM dalgası"},
            {"d.noData", "Henüz veri yok"},
            {"d.noDataSub", "Geri kazanılabilir alanı görmek için bir tarama çalıştırın."},
            {"d.lastReport", "Son rapor:"},
            {"d.diskFull", "disk dolu"},
            {"t.schedule", "Zamanlama"},
            {"s.schedule", "Otomatik günlük temizlik"},
            {"sched.eyebrow", "ZAMANLANMIŞ TEMİZLİK"},
            {"sched.title", "Sweep otopilotta"},
            {"sched.sub", "Her gün sessiz temizlik"},
            {"sched.backend", "Arka uç"},
            {"sched.status", "Durum"},
            {"sched.hour", "Saat"},
            {"sched.selection", "Temizleyiciler"},
            {"sched.hint", "ör. firefox.cache (boş = varsayılan)"},
            {"sched.enable", "Günlük aç"},
            {"sched.disable", "Kapat"},
            {"sched.working", "Çalışıyor…"},
            {"sched.refresh", "Yenile"},
            {"sched.memopt", "Bellek görevi"},
            {"t.memory", "Bellek"},
            {"s.memory", "Canlı bellek kullanımı ve optimizasyon"},
            {"t.startup_mgr", "Başlangıç"},
            {"s.startup_mgr", "Oturum açınca ne başlar"},
            {"t.sysinfo", "Sistem sağlığı"},
            {"s.sysinfo", "CPU, disk, SMART ve sıcaklık"},
            {"t.network", "Ağ"},
            {"s.network", "DNS ve paket önbellekleri"},
            {"t.privacy", "Gizlilik"},
            {"s.privacy", "Telemetri, son dosyalar ve pano"},
            {"g.optimize", "Optimize et"},
            {"g.optimize_aggressive", "Agresif mod (takas/standby)"},
            {"g.impact_high", "Yüksek etki"},
            {"g.impact_medium", "Orta etki"},
            {"g.impact_low", "Düşük etki"},
            {"g.flush_dns", "DNS'i boşalt"},
            {"g.clean_caches", "Paket önbelleklerini temizle"},
            {"g.shield", "Telemetriyi kapat"},
            {"g.wipe", "Geçmişi ve panoyu temizle"},
            {"g.search_startup", "Başlangıç girdisi ara…"},
            {"g.health_green", "Sağlıklı"},
            {"g.health_yellow", "Uyarı"},
            {"g.health_red", "Kritik"},
            {"g.uptime", "Çalışma süresi"},
            {"g.temperature", "Sıcaklık"},
            {"g.dry_run", "Deneme (önizleme)"},
            {"g.recent_files", "Son dosyalar"},
            {"g.clipboard", "Pano"},
            {"g.working", "Çalışıyor…"},
            {"g.refresh", "Yenile"},
            {"g.close", "Kapat"},
            {"g.open", "Aç"},
            {"g.cores", "çekirdek"},
            {"g.cpu", "İŞLEMCİ"},
            {"g.memory", "Bellek"},
            {"g.swap", "Takas"},
            {"reclaimed", "geri kazanıldı"},
            {"g.needs_root", "Root/yönetici şifresi gerekli"},
            {"g.elev_retry", "Yönetici olarak dene"},
            {"w.title", "Sweep'e hoş geldin"},
            {"w.sub", "Bilgisayarın temiz ve kontrol altında."},
            {"w.tour", "Tura katıl"},
            {"w.skip", "Atla"},
            {"w.next", "İleri"},
            {"w.back", "Geri"},
            {"w.done", "Bitti"},
            {"g.no_engine", "Temizlik motoru (sweep) bulunamadı — optimizasyon için uygulamayla aynı klasöre kurun."},
            {"g.apply", "Uygula"},
            {"g.automem", "Otomatik bakım"},
            {"g.automem_desc", "5 dakikada bir bakar, RAM %80'i aşınca kırpar (güvenli, yöneticisiz)."},
            {"g.smart", "SMART"},
            {"a.all", "Tümü"},
            {"a.author", "Yayıncı"},
            {"u.undo_available", "Geri alma hazır"},
            {"u.undo", "Geri Al"},
            {"u.undo_last", "Son temizliği geri al"},
            {"u.restore", "Geri Yükle"},
            {"u.runs_title", "Önceki temizlikler"},
            {"u.no_runs", "Henüz önceki temizlik yok."},
            {"u.backup_title", "Önce yedek oluşturulsun mu?"},
            {"u.backup_body", "Devam etmeden önce sistem yedeği oluşturmanı öneririz."},
            {"u.create_backup", "Yedek Oluştur"},
            {"u.continue", "Devam Et"},
            {"u.review_first", "Önce önizlemeyi incele, sonra Temizle'ye bas."},
            {"u.backup_open_failed", "Sistem yedek ayarları açılamadı. Lütfen manuel yedek al."},
            {"a.backup", "Yedek"},
            {"a.cancel", "Vazgeç"},
            {"a.capabilities", "YETENEKLER"},
            {"a.category", "KATEGORİ"},
            {"a.category_driver", "Sürücü"},
            {"a.category_os", "İşletim sistemi"},
            {"a.category_security", "Güvenlik"},
            {"a.category_third_party", "Üçüncü taraf"},
            {"a.category_unknown", "Bilinmiyor"},
            {"a.category_updater", "Güncelleyici"},
            {"a.clear_selection", "Temizle"},
            {"a.command", "Komut"},
            {"a.confirm", "Onayla"},
            {"a.confirm_ack", "Riski anladım"},
            {"a.confirm_body", "Bu, sistem yapılandırmasını değiştirir. Devam edilsin mi?"},
            {"a.confirm_critical", "Kritik risk: bunu devre dışı bırakmak veya kaldırmak açılışı ya da güvenliği bozabilir."},
            {"a.confirm_title", "Değişikliği onayla"},
            {"a.detail", "AYRINTI"},
            {"a.edit", "Komutu düzenle"},
            {"a.edit_label", "Yeni komut"},
            {"a.edit_title", "Komutu düzenle"},
            {"a.editable", "Düzenlenebilir"},
            {"a.empty", "Başlangıç öğesi bulunamadı"},
            {"a.empty_hint", "Filtreleri gevşetmeyi deneyin."},
            {"a.error", "Hata"},
            {"a.export_csv", "CSV dışa aktar"},
            {"a.export_json", "JSON dışa aktar"},
            {"a.exported", "Dışa aktarıldı"},
            {"a.eyebrow", "BAŞLANGIÇ VE GÖREVLER"},
            {"a.history_empty", "Henüz denetim kaydı yok"},
            {"a.kind", "TÜR"},
            {"a.kind_all", "Tümü"},
            {"a.kind_startup", "Başlangıç"},
            {"a.kind_task", "Zamanlanmış"},
            {"a.last_run", "Son çalışma"},
            {"a.location", "Konum"},
            {"a.no", "Hayır"},
            {"a.no_selection", "Ayrıntılar için bir öğe seçin."},
            {"a.notes", "Notlar"},
            {"a.reason", "Gerekçe"},
            {"a.removable", "Kaldırılabilir"},
            {"a.remove", "Kaldır"},
            {"a.reversible", "Geri alınabilir"},
            {"a.risk", "RİSK"},
            {"a.risk_critical", "Kritik"},
            {"a.risk_high", "Yüksek"},
            {"a.risk_low", "Düşük"},
            {"a.risk_medium", "Orta"},
            {"a.rollback", "Geri al"},
            {"a.rollback_last", "Son değişikliği geri al"},
            {"a.save", "Kaydet"},
            {"a.scope", "KAPSAM"},
            {"a.scope_system", "Sistem"},
            {"a.scope_user", "Kullanıcı"},
            {"a.search", "Ad, komut veya konumda ara…"},
            {"a.select_all", "Tümünü seç"},
            {"a.selected", "seçili"},
            {"a.sort", "Sırala"},
            {"a.sort_impact", "Etki"},
            {"a.sort_name", "Ad"},
            {"a.sort_source", "Kaynak"},
            {"a.state", "DURUM"},
            {"a.state_off", "Devre dışı"},
            {"a.state_on", "Etkin"},
            {"a.status_items", "öğe"},
            {"a.tab_history", "Denetim geçmişi"},
            {"a.tab_list", "Liste"},
            {"a.trigger", "Tetikleyici"},
            {"a.trigger_boot", "Önyükleme"},
            {"a.trigger_event", "Olay"},
            {"a.trigger_idle", "Boşta"},
            {"a.trigger_logon", "Oturum açma"},
            {"a.trigger_schedule", "Zamanlama"},
            {"a.trigger_unknown", "Bilinmiyor"},
            {"a.yes", "Evet"},
            {"s.autostart", "Kendiliğinden başlayan her şey: başlangıç girdileri ve zamanlanmış görevler"},
            {"t.autostart_nav", "Başlangıç ve Görevler"},
            {"s.autostart_nav", "Otomatik başlayan uygulamalar"},
            {"t.autostart", "Başlangıç ve Zamanlanmış Görevler"},
        };
        const char *es[][2] = {
            {"side.subtitle", "LIMPIADOR DEL SISTEMA"},
            {"side.menu", "MENÚ"},
            {"side.analysis", "ANÁLISIS Y AJUSTES"},
            {"side.storage", "ALMACENAMIENTO"},
            {"side.diskUsage", "Uso del disco"},
            {"side.full", "lleno"},
            {"side.theme", "Tema"},
            {"side.changeTheme", "Cambiar tema"},
            {"t.dashboard", "Panel"},
            {"t.cleaner", "Limpiador"},
            {"t.history", "Historial"},
            {"t.system", "Sistema"},
            {"t.files", "Archivos"},
            {"t.disk", "Disco"},
            {"t.optimize", "Memoria"},
            {"t.settings", "Ajustes"},
            {"s.dashboard", "Resumen del estado del sistema"},
            {"s.cleaner", "Recupera espacio en disco con seguridad"},
            {"s.history", "Operaciones de limpieza anteriores"},
            {"s.system", "Pulso del hardware en vivo"},
            {"s.files", "Caza archivos grandes y duplicados"},
            {"s.disk", "Tamaños de carpeta de un vistazo"},
            {"disk.up", "Subir"},
            {"disk.volumes", "Volúmenes"},
            {"disk.empty", "Carpeta vacía"},
            {"disk.cancel", "Detener"},
            {"s.optimize", "Reduce la presión de memoria"},
            {"s.settings", "Tema, transparencia y movimiento"},
            {"d.reclaim", "ESPACIO RECUPERABLE"},
            {"d.ready", "LISTO"},
            {"d.scanning", "ESCANEANDO"},
            {"d.analyzing", "Analizando…"},
            {"d.scanAll", "Escanear todo"},
            {"d.scanning2", "Escaneando…"},
            {"d.openCleaner", "Abrir limpiador"},
            {"d.quickScan", "Escaneo rápido"},
            {"d.quickScanSub", "Vista previa de todo"},
            {"d.cleaner", "Limpiador"},
            {"d.cleanerSub", "Selecciona y limpia"},
            {"d.appearance", "Apariencia"},
            {"d.appearanceSub", "Tema y transparencia"},
            {"d.system", "SISTEMA"},
            {"d.live", "EN VIVO"},
            {"d.charts", "GRÁFICOS"},
            {"d.distribution", "Distribución del disco"},
            {"d.wave", "Onda CPU / RAM"},
            {"d.noData", "Sin datos aún"},
            {"d.noDataSub", "Ejecuta un análisis para ver el espacio recuperable."},
            {"d.lastReport", "Último informe:"},
            {"d.diskFull", "disco lleno"},
            {"t.schedule", "Horario"},
            {"s.schedule", "Limpieza diaria automática"},
            {"sched.eyebrow", "LIMPIEZA PROGRAMADA"},
            {"sched.title", "Sweep en piloto automático"},
            {"sched.sub", "Limpieza silenciosa diaria"},
            {"sched.backend", "Motor"},
            {"sched.status", "Estado"},
            {"sched.hour", "Hora"},
            {"sched.selection", "Limpiadores"},
            {"sched.hint", "p. ej. firefox.cache (vacío = defecto)"},
            {"sched.enable", "Activar diario"},
            {"sched.disable", "Desactivar"},
            {"sched.working", "Trabajando…"},
            {"sched.refresh", "Actualizar"},
            {"sched.memopt", "Tarea de memoria"},
            {"t.memory", "Memoria"},
            {"s.memory", "Uso de memoria en vivo y optimización"},
            {"t.startup_mgr", "Inicio"},
            {"s.startup_mgr", "Qué se inicia al entrar"},
            {"t.sysinfo", "Salud del sistema"},
            {"s.sysinfo", "CPU, discos, SMART y temperatura"},
            {"t.network", "Red"},
            {"s.network", "DNS y cachés de paquetes"},
            {"t.privacy", "Privacidad"},
            {"s.privacy", "Telemetría, archivos recientes y portapapeles"},
            {"g.optimize", "Optimizar"},
            {"g.optimize_aggressive", "Modo agresivo (swap/standby)"},
            {"g.impact_high", "Impacto alto"},
            {"g.impact_medium", "Impacto medio"},
            {"g.impact_low", "Impacto bajo"},
            {"g.flush_dns", "Vaciar DNS"},
            {"g.clean_caches", "Limpiar cachés de paquetes"},
            {"g.shield", "Desactivar telemetría"},
            {"g.wipe", "Borrar historial y portapapeles"},
            {"g.search_startup", "Buscar entradas de inicio…"},
            {"g.health_green", "Saludable"},
            {"g.health_yellow", "Advertencia"},
            {"g.health_red", "Crítico"},
            {"g.uptime", "Tiempo activo"},
            {"g.temperature", "Temperatura"},
            {"g.dry_run", "Simulación (vista previa)"},
            {"g.recent_files", "Archivos recientes"},
            {"g.clipboard", "Portapapeles"},
            {"g.working", "Trabajando…"},
            {"g.refresh", "Actualizar"},
            {"g.close", "Desactivar"},
            {"g.open", "Activar"},
            {"g.cores", "núcleos"},
            {"g.cpu", "CPU"},
            {"g.memory", "Memoria"},
            {"g.swap", "Swap"},
            {"reclaimed", "recuperado"},
            {"g.needs_root", "Se requiere contraseña de root/administrador"},
            {"g.elev_retry", "Reintentar como administrador"},
            {"w.title", "Te damos la bienvenida a Sweep"},
            {"w.sub", "Tu PC, limpio y bajo control."},
            {"w.tour", "Haz un recorrido"},
            {"w.skip", "Omitir"},
            {"w.next", "Siguiente"},
            {"w.back", "Atrás"},
            {"w.done", "Listo"},
            {"g.no_engine", "Motor de limpieza (sweep) no encontrado — instálalo junto a la app para optimizar."},
            {"g.apply", "Aplicar"},
            {"g.automem", "Mantenimiento automático"},
            {"g.automem_desc", "Revisa cada 5 min, optimiza si la RAM supera el 80%."},
            {"g.smart", "SMART"},
            {"a.all", "Todo"},
            {"u.undo_available", "Deshacer disponible"},
            {"u.undo", "Deshacer"},
            {"u.undo_last", "Deshacer la última limpieza"},
            {"u.restore", "Restaurar"},
            {"u.runs_title", "Limpiezas anteriores"},
            {"u.no_runs", "Aún no hay limpiezas anteriores."},
            {"u.backup_title", "¿Crear una copia primero?"},
            {"u.backup_body", "Recomendamos crear una copia de seguridad del sistema antes de continuar."},
            {"u.create_backup", "Crear copia"},
            {"u.continue", "Continuar"},
            {"u.review_first", "Revisa la vista previa y pulsa Limpiar de nuevo."},
            {"u.backup_open_failed", "No se pudo abrir la configuración de copias. Haz una copia manual."},
            {"a.author", "Autor"},
            {"a.backup", "Copia"},
            {"a.cancel", "Cancelar"},
            {"a.capabilities", "CAPACIDADES"},
            {"a.category", "CATEGORÍA"},
            {"a.category_driver", "Controlador"},
            {"a.category_os", "SO"},
            {"a.category_security", "Seguridad"},
            {"a.category_third_party", "Terceros"},
            {"a.category_unknown", "Desconocido"},
            {"a.category_updater", "Actualizador"},
            {"a.clear_selection", "Limpiar"},
            {"a.command", "Comando"},
            {"a.confirm", "Confirmar"},
            {"a.confirm_ack", "Entiendo el riesgo"},
            {"a.confirm_body", "Esto cambia la configuración del sistema. ¿Continuar?"},
            {"a.confirm_critical", "Riesgo crítico: desactivarlo o quitarlo puede romper el inicio o la seguridad."},
            {"a.confirm_title", "Confirmar cambio"},
            {"a.detail", "DETALLE"},
            {"a.edit", "Editar comando"},
            {"a.edit_label", "Nuevo comando"},
            {"a.edit_title", "Editar comando"},
            {"a.editable", "Editable"},
            {"a.empty", "No se encontraron entradas de inicio"},
            {"a.empty_hint", "Prueba a relajar los filtros."},
            {"a.error", "Error"},
            {"a.export_csv", "Exportar CSV"},
            {"a.export_json", "Exportar JSON"},
            {"a.exported", "Exportado"},
            {"a.eyebrow", "INICIO Y TAREAS"},
            {"a.history_empty", "Aún no hay registros de auditoría"},
            {"a.kind", "TIPO"},
            {"a.kind_all", "Todo"},
            {"a.kind_startup", "Inicio"},
            {"a.kind_task", "Programadas"},
            {"a.last_run", "Última ejecución"},
            {"a.location", "Ubicación"},
            {"a.no", "No"},
            {"a.no_selection", "Selecciona un elemento para ver sus detalles."},
            {"a.notes", "Notas"},
            {"a.reason", "Motivo"},
            {"a.removable", "Se puede quitar"},
            {"a.remove", "Quitar"},
            {"a.reversible", "Reversible"},
            {"a.risk", "RIESGO"},
            {"a.risk_critical", "Crítico"},
            {"a.risk_high", "Alto"},
            {"a.risk_low", "Bajo"},
            {"a.risk_medium", "Medio"},
            {"a.rollback", "Revertir"},
            {"a.rollback_last", "Revertir el último cambio"},
            {"a.save", "Guardar"},
            {"a.scope", "ÁMBITO"},
            {"a.scope_system", "Sistema"},
            {"a.scope_user", "Usuario"},
            {"a.search", "Buscar por nombre, comando o ubicación…"},
            {"a.select_all", "Seleccionar todo"},
            {"a.selected", "seleccionados"},
            {"a.sort", "Ordenar"},
            {"a.sort_impact", "Impacto"},
            {"a.sort_name", "Nombre"},
            {"a.sort_source", "Origen"},
            {"a.state", "ESTADO"},
            {"a.state_off", "Desactivado"},
            {"a.state_on", "Activo"},
            {"a.status_items", "elementos"},
            {"a.tab_history", "Historial de auditoría"},
            {"a.tab_list", "Lista"},
            {"a.trigger", "Desencadenador"},
            {"a.trigger_boot", "Arranque"},
            {"a.trigger_event", "Evento"},
            {"a.trigger_idle", "Inactivo"},
            {"a.trigger_logon", "Inicio de sesión"},
            {"a.trigger_schedule", "Programación"},
            {"a.trigger_unknown", "Desconocido"},
            {"a.yes", "Sí"},
            {"s.autostart", "Todo lo que se inicia solo: entradas de inicio y tareas programadas"},
            {"t.autostart_nav", "Inicio y tareas"},
            {"s.autostart_nav", "Aplicaciones que se inician solas"},
            {"t.autostart", "Inicio y tareas programadas"},
        };
        const char *ru[][2] = {
            {"side.subtitle", "ОЧИСТКА СИСТЕМЫ"},
            {"side.menu", "МЕНЮ"},
            {"side.analysis", "АНАЛИЗ И НАСТРОЙКА"},
            {"side.storage", "ХРАНИЛИЩЕ"},
            {"side.diskUsage", "Использование диска"},
            {"side.full", "занято"},
            {"side.theme", "Тема"},
            {"side.changeTheme", "Сменить тему"},
            {"t.dashboard", "Панель"},
            {"t.cleaner", "Очистка"},
            {"t.history", "История"},
            {"t.system", "Система"},
            {"t.files", "Файлы"},
            {"t.disk", "Диск"},
            {"t.optimize", "Память"},
            {"t.settings", "Настройки"},
            {"s.dashboard", "Обзор состояния системы"},
            {"s.cleaner", "Безопасно освобождайте место"},
            {"s.history", "Прошлые операции очистки"},
            {"s.system", "Живой пульс оборудования"},
            {"s.files", "Поиск больших файлов и дубликатов"},
            {"s.disk", "Размеры папок"},
            {"disk.up", "Вверх"},
            {"disk.volumes", "Тома"},
            {"disk.empty", "Пустая папка"},
            {"disk.cancel", "Стоп"},
            {"s.optimize", "Снижение нагрузки на память"},
            {"s.settings", "Тема, прозрачность и движение"},
            {"d.reclaim", "МЕСТО ДЛЯ ОЧИСТКИ"},
            {"d.ready", "ГОТОВО"},
            {"d.scanning", "СКАНИРОВАНИЕ"},
            {"d.analyzing", "Анализ…"},
            {"d.scanAll", "Сканировать всё"},
            {"d.scanning2", "Сканирование…"},
            {"d.openCleaner", "Открыть очистку"},
            {"d.quickScan", "Быстрое сканирование"},
            {"d.quickScanSub", "Предпросмотр всего"},
            {"d.cleaner", "Очистка"},
            {"d.cleanerSub", "Выберите и очистите"},
            {"d.appearance", "Внешний вид"},
            {"d.appearanceSub", "Тема и прозрачность"},
            {"d.system", "СИСТЕМА"},
            {"d.live", "ОНЛАЙН"},
            {"d.charts", "ГРАФИКИ"},
            {"d.distribution", "Распределение диска"},
            {"d.wave", "Волна ЦП / ОЗУ"},
            {"d.noData", "Пока нет данных"},
            {"d.noDataSub", "Запустите сканирование, чтобы увидеть место."},
            {"d.lastReport", "Последний отчёт:"},
            {"d.diskFull", "диск полон"},
            {"t.schedule", "Расписание"},
            {"s.schedule", "Ежедневная автоочистка"},
            {"sched.eyebrow", "ОТЛОЖЕННАЯ ОЧИСТКА"},
            {"sched.title", "Sweep на автопилоте"},
            {"sched.sub", "Тихая ежедневная очистка"},
            {"sched.backend", "Движок"},
            {"sched.status", "Статус"},
            {"sched.hour", "Час"},
            {"sched.selection", "Очистители"},
            {"sched.hint", "напр. firefox.cache (пусто = по умолчанию)"},
            {"sched.enable", "Включить ежедневно"},
            {"sched.disable", "Выключить"},
            {"sched.working", "Работаю…"},
            {"sched.refresh", "Обновить"},
            {"sched.memopt", "Задача памяти"},
            {"t.memory", "Память"},
            {"s.memory", "Память в реальном времени и оптимизация"},
            {"t.startup_mgr", "Автозапуск"},
            {"s.startup_mgr", "Что запускается при входе"},
            {"t.sysinfo", "Здоровье системы"},
            {"s.sysinfo", "CPU, диски, SMART и температура"},
            {"t.network", "Сеть"},
            {"s.network", "DNS и кэши пакетов"},
            {"t.privacy", "Приватность"},
            {"s.privacy", "Телеметрия, недавние файлы и буфер обмена"},
            {"g.optimize", "Оптимизировать"},
            {"g.optimize_aggressive", "Агрессивный режим (swap/standby)"},
            {"g.impact_high", "Высокое влияние"},
            {"g.impact_medium", "Среднее влияние"},
            {"g.impact_low", "Низкое влияние"},
            {"g.flush_dns", "Сбросить DNS"},
            {"g.clean_caches", "Очистить кэши пакетов"},
            {"g.shield", "Отключить телеметрию"},
            {"g.wipe", "Очистить историю и буфер"},
            {"g.search_startup", "Поиск в автозапуске…"},
            {"g.health_green", "Здорово"},
            {"g.health_yellow", "Предупреждение"},
            {"g.health_red", "Критично"},
            {"g.uptime", "Время работы"},
            {"g.temperature", "Температура"},
            {"g.dry_run", "Пробный запуск (просмотр)"},
            {"g.recent_files", "Недавние файлы"},
            {"g.clipboard", "Буфер обмена"},
            {"g.working", "Работаю…"},
            {"g.refresh", "Обновить"},
            {"g.close", "Отключить"},
            {"g.open", "Включить"},
            {"g.cores", "ядер"},
            {"g.cpu", "ЦП"},
            {"g.memory", "Память"},
            {"g.swap", "Подкачка"},
            {"reclaimed", "освобождено"},
            {"g.needs_root", "Требуется пароль root/администратора"},
            {"g.elev_retry", "Повторить с правами администратора"},
            {"w.title", "Добро пожаловать в Sweep"},
            {"w.sub", "Твой ПК — чистый и под контролем."},
            {"w.tour", "Пройти обучение"},
            {"w.skip", "Пропустить"},
            {"w.next", "Далее"},
            {"w.back", "Назад"},
            {"w.done", "Готово"},
            {"g.no_engine", "Движок очистки (sweep) не найден — установите его рядом с приложением."},
            {"g.apply", "Применить"},
            {"g.automem", "Автообслуживание"},
            {"g.automem_desc", "Проверка каждые 5 мин, очистка при RAM выше 80%."},
            {"g.smart", "SMART"},
            {"a.all", "Все"},
            {"a.author", "Издатель"},
            {"u.undo_available", "Доступна отмена"},
            {"u.undo", "Отменить"},
            {"u.undo_last", "Отменить последнюю очистку"},
            {"u.restore", "Восстановить"},
            {"u.runs_title", "Прошлые очистки"},
            {"u.no_runs", "Прошлых очисток пока нет."},
            {"u.backup_title", "Сначала создать резервную копию?"},
            {"u.backup_body", "Перед продолжением рекомендуем создать резервную копию системы."},
            {"u.create_backup", "Создать копию"},
            {"u.continue", "Продолжить"},
            {"u.review_first", "Сначала просмотрите предпросмотр, затем нажмите «Очистить» ещё раз."},
            {"u.backup_open_failed", "Не удалось открыть настройки резервного копирования. Создайте копию вручную."},
            {"a.backup", "Резервная копия"},
            {"a.cancel", "Отмена"},
            {"a.capabilities", "ВОЗМОЖНОСТИ"},
            {"a.category", "КАТЕГОРИЯ"},
            {"a.category_driver", "Драйвер"},
            {"a.category_os", "ОС"},
            {"a.category_security", "Безопасность"},
            {"a.category_third_party", "Сторонние"},
            {"a.category_unknown", "Неизвестно"},
            {"a.category_updater", "Обновление"},
            {"a.clear_selection", "Сбросить"},
            {"a.command", "Команда"},
            {"a.confirm", "Подтвердить"},
            {"a.confirm_ack", "Я понимаю риск"},
            {"a.confirm_body", "Это изменит конфигурацию системы. Продолжить?"},
            {"a.confirm_critical", "Критический риск: отключение или удаление может нарушить запуск или безопасность."},
            {"a.confirm_title", "Подтвердите изменение"},
            {"a.detail", "ПОДРОБНОСТИ"},
            {"a.edit", "Изменить команду"},
            {"a.edit_label", "Новая команда"},
            {"a.edit_title", "Изменить команду"},
            {"a.editable", "Редактируемо"},
            {"a.empty", "Записи автозагрузки не найдены"},
            {"a.empty_hint", "Попробуйте ослабить фильтры."},
            {"a.error", "Ошибка"},
            {"a.export_csv", "Экспорт CSV"},
            {"a.export_json", "Экспорт JSON"},
            {"a.exported", "Экспортировано"},
            {"a.eyebrow", "АВТОЗАПУСК И ЗАДАЧИ"},
            {"a.history_empty", "Записей аудита пока нет"},
            {"a.kind", "ТИП"},
            {"a.kind_all", "Все"},
            {"a.kind_startup", "Автозапуск"},
            {"a.kind_task", "Запланированные"},
            {"a.last_run", "Последний запуск"},
            {"a.location", "Расположение"},
            {"a.no", "Нет"},
            {"a.no_selection", "Выберите элемент, чтобы увидеть подробности."},
            {"a.notes", "Заметки"},
            {"a.reason", "Причина"},
            {"a.removable", "Удаляемо"},
            {"a.remove", "Удалить"},
            {"a.reversible", "Обратимо"},
            {"a.risk", "РИСК"},
            {"a.risk_critical", "Критический"},
            {"a.risk_high", "Высокий"},
            {"a.risk_low", "Низкий"},
            {"a.risk_medium", "Средний"},
            {"a.rollback", "Откатить"},
            {"a.rollback_last", "Откатить последнее изменение"},
            {"a.save", "Сохранить"},
            {"a.scope", "ОБЛАСТЬ"},
            {"a.scope_system", "Система"},
            {"a.scope_user", "Пользователь"},
            {"a.search", "Поиск по имени, команде или расположению…"},
            {"a.select_all", "Выбрать все"},
            {"a.selected", "выбрано"},
            {"a.sort", "Сортировка"},
            {"a.sort_impact", "Влияние"},
            {"a.sort_name", "Имя"},
            {"a.sort_source", "Источник"},
            {"a.state", "СОСТОЯНИЕ"},
            {"a.state_off", "Отключено"},
            {"a.state_on", "Включено"},
            {"a.status_items", "элементов"},
            {"a.tab_history", "Журнал аудита"},
            {"a.tab_list", "Список"},
            {"a.trigger", "Триггер"},
            {"a.trigger_boot", "Загрузка"},
            {"a.trigger_event", "Событие"},
            {"a.trigger_idle", "Простой"},
            {"a.trigger_logon", "Вход в систему"},
            {"a.trigger_schedule", "Расписание"},
            {"a.trigger_unknown", "Неизвестно"},
            {"a.yes", "Да"},
            {"s.autostart", "Всё, что запускается само: автозагрузка и запланированные задачи"},
            {"t.autostart_nav", "Автозапуск и задачи"},
            {"s.autostart_nav", "Программы, запускаемые автоматически"},
            {"t.autostart", "Автозапуск и запланированные задачи"},
        };
        const char *fr[][2] = {
            {"side.subtitle", "NETTOYAGE SYSTÈME"},
            {"side.menu", "MENU"},
            {"side.analysis", "ANALYSE ET RÉGLAGES"},
            {"side.storage", "STOCKAGE"},
            {"side.diskUsage", "Utilisation du disque"},
            {"side.full", "plein"},
            {"side.theme", "Thème"},
            {"side.changeTheme", "Changer de thème"},
            {"t.dashboard", "Tableau de bord"},
            {"t.cleaner", "Nettoyage"},
            {"t.history", "Historique"},
            {"t.system", "Système"},
            {"t.files", "Fichiers"},
            {"t.disk", "Disque"},
            {"t.optimize", "Mémoire"},
            {"t.settings", "Réglages"},
            {"s.dashboard", "Aperçu de l'état du système"},
            {"s.cleaner", "Récupérez de l'espace disque en sécurité"},
            {"s.history", "Opérations de nettoyage passées"},
            {"s.system", "Pouls matériel en direct"},
            {"s.files", "Traque fichiers volumineux et doublons"},
            {"s.disk", "Tailles des dossiers"},
            {"disk.up", "Haut"},
            {"disk.volumes", "Volumes"},
            {"disk.empty", "Dossier vide"},
            {"disk.cancel", "Arrêter"},
            {"s.optimize", "Réduire la pression mémoire"},
            {"s.settings", "Thème, transparence et mouvement"},
            {"d.reclaim", "ESPACE RÉCUPÉRABLE"},
            {"d.ready", "PRÊT"},
            {"d.scanning", "ANALYSE"},
            {"d.analyzing", "Analyse…"},
            {"d.scanAll", "Tout analyser"},
            {"d.scanning2", "Analyse…"},
            {"d.openCleaner", "Ouvrir le nettoyage"},
            {"d.quickScan", "Analyse rapide"},
            {"d.quickScanSub", "Aperçu de tout"},
            {"d.cleaner", "Nettoyage"},
            {"d.cleanerSub", "Sélectionner et nettoyer"},
            {"d.appearance", "Apparence"},
            {"d.appearanceSub", "Thème et transparence"},
            {"d.system", "SYSTÈME"},
            {"d.live", "EN DIRECT"},
            {"d.charts", "GRAPHIQUES"},
            {"d.distribution", "Distribution du disque"},
            {"d.wave", "Vague CPU / RAM"},
            {"d.noData", "Pas encore de données"},
            {"d.noDataSub", "Lancez une analyse pour voir l'espace récupérable."},
            {"d.lastReport", "Dernier rapport :"},
            {"d.diskFull", "disque plein"},
            {"t.schedule", "Planification"},
            {"s.schedule", "Nettoyage quotidien automatique"},
            {"sched.eyebrow", "NETTOYAGE PLANIFIÉ"},
            {"sched.title", "Sweep en pilote automatique"},
            {"sched.sub", "Nettoyage silencieux quotidien"},
            {"sched.backend", "Moteur"},
            {"sched.status", "État"},
            {"sched.hour", "Heure"},
            {"sched.selection", "Nettoyeurs"},
            {"sched.hint", "p. ex. firefox.cache (vide = défaut)"},
            {"sched.enable", "Activer quotidien"},
            {"sched.disable", "Désactiver"},
            {"sched.working", "En cours…"},
            {"sched.refresh", "Actualiser"},
            {"sched.memopt", "Tâche mémoire"},
            {"t.memory", "Mémoire"},
            {"s.memory", "Utilisation mémoire en direct et optimisation"},
            {"t.startup_mgr", "Démarrage"},
            {"s.startup_mgr", "Ce qui se lance à l'ouverture de session"},
            {"t.sysinfo", "Santé du système"},
            {"s.sysinfo", "CPU, disques, SMART et température"},
            {"t.network", "Réseau"},
            {"s.network", "DNS et caches de paquets"},
            {"t.privacy", "Confidentialité"},
            {"s.privacy", "Télémétrie, fichiers récents et presse-papiers"},
            {"g.optimize", "Optimiser"},
            {"g.optimize_aggressive", "Mode agressif (swap/standby)"},
            {"g.impact_high", "Impact élevé"},
            {"g.impact_medium", "Impact moyen"},
            {"g.impact_low", "Impact faible"},
            {"g.flush_dns", "Vider le DNS"},
            {"g.clean_caches", "Nettoyer les caches de paquets"},
            {"g.shield", "Désactiver la télémétrie"},
            {"g.wipe", "Effacer l'historique et le presse-papiers"},
            {"g.search_startup", "Rechercher des entrées de démarrage…"},
            {"g.health_green", "Sain"},
            {"g.health_yellow", "Avertissement"},
            {"g.health_red", "Critique"},
            {"g.uptime", "Durée de fonctionnement"},
            {"g.temperature", "Température"},
            {"g.dry_run", "Simulation (aperçu)"},
            {"g.recent_files", "Fichiers récents"},
            {"g.clipboard", "Presse-papiers"},
            {"g.working", "En cours…"},
            {"g.refresh", "Actualiser"},
            {"g.close", "Désactiver"},
            {"g.open", "Activer"},
            {"g.cores", "cœurs"},
            {"g.cpu", "CPU"},
            {"g.memory", "Mémoire"},
            {"g.swap", "Swap"},
            {"reclaimed", "récupéré"},
            {"g.needs_root", "Mot de passe root/administrateur requis"},
            {"g.elev_retry", "Réessayer en admin"},
            {"w.title", "Bienvenue dans Sweep"},
            {"w.sub", "Ton PC, propre et sous contrôle."},
            {"w.tour", "Faire la visite"},
            {"w.skip", "Passer"},
            {"w.next", "Suivant"},
            {"w.back", "Retour"},
            {"w.done", "Terminé"},
            {"g.no_engine", "Moteur de nettoyage (sweep) introuvable — installez-le à côté de l'app."},
            {"g.apply", "Appliquer"},
            {"g.automem", "Maintenance auto"},
            {"g.automem_desc", "Vérifie toutes les 5 min, optimise si RAM > 80 %."},
            {"g.smart", "SMART"},
            {"a.all", "Tout"},
            {"a.author", "Auteur"},
            {"u.undo_available", "Annulation disponible"},
            {"u.undo", "Annuler"},
            {"u.undo_last", "Annuler le dernier nettoyage"},
            {"u.restore", "Restaurer"},
            {"u.runs_title", "Nettoyages précédents"},
            {"u.no_runs", "Aucun nettoyage précédent."},
            {"u.backup_title", "Créer d'abord une sauvegarde ?"},
            {"u.backup_body", "Nous recommandons de créer une sauvegarde système avant de continuer."},
            {"u.create_backup", "Créer une sauvegarde"},
            {"u.continue", "Continuer"},
            {"u.review_first", "Vérifiez l'aperçu, puis appuyez de nouveau sur Nettoyer."},
            {"u.backup_open_failed", "Impossible d'ouvrir les paramètres de sauvegarde. Faites une sauvegarde manuelle."},
            {"a.backup", "Sauvegarde"},
            {"a.cancel", "Annuler"},
            {"a.capabilities", "CAPACITÉS"},
            {"a.category", "CATÉGORIE"},
            {"a.category_driver", "Pilote"},
            {"a.category_os", "SE"},
            {"a.category_security", "Sécurité"},
            {"a.category_third_party", "Tiers"},
            {"a.category_unknown", "Inconnu"},
            {"a.category_updater", "Mise à jour"},
            {"a.clear_selection", "Effacer"},
            {"a.command", "Commande"},
            {"a.confirm", "Confirmer"},
            {"a.confirm_ack", "Je comprends le risque"},
            {"a.confirm_body", "Cela modifie la configuration du système. Continuer ?"},
            {"a.confirm_critical", "Risque critique : le désactiver ou le supprimer peut casser le démarrage ou la sécurité."},
            {"a.confirm_title", "Confirmer la modification"},
            {"a.detail", "DÉTAIL"},
            {"a.edit", "Modifier la commande"},
            {"a.edit_label", "Nouvelle commande"},
            {"a.edit_title", "Modifier la commande"},
            {"a.editable", "Modifiable"},
            {"a.empty", "Aucune entrée de démarrage trouvée"},
            {"a.empty_hint", "Essayez d'assouplir les filtres."},
            {"a.error", "Erreur"},
            {"a.export_csv", "Exporter CSV"},
            {"a.export_json", "Exporter JSON"},
            {"a.exported", "Exporté"},
            {"a.eyebrow", "DÉMARRAGE ET TÂCHES"},
            {"a.history_empty", "Aucune entrée d'audit pour l'instant"},
            {"a.kind", "TYPE"},
            {"a.kind_all", "Tout"},
            {"a.kind_startup", "Démarrage"},
            {"a.kind_task", "Planifiées"},
            {"a.last_run", "Dernière exécution"},
            {"a.location", "Emplacement"},
            {"a.no", "Non"},
            {"a.no_selection", "Sélectionnez un élément pour voir ses détails."},
            {"a.notes", "Notes"},
            {"a.reason", "Raison"},
            {"a.removable", "Supprimable"},
            {"a.remove", "Supprimer"},
            {"a.reversible", "Réversible"},
            {"a.risk", "RISQUE"},
            {"a.risk_critical", "Critique"},
            {"a.risk_high", "Élevé"},
            {"a.risk_low", "Faible"},
            {"a.risk_medium", "Moyen"},
            {"a.rollback", "Annuler"},
            {"a.rollback_last", "Annuler la dernière modification"},
            {"a.save", "Enregistrer"},
            {"a.scope", "PORTÉE"},
            {"a.scope_system", "Système"},
            {"a.scope_user", "Utilisateur"},
            {"a.search", "Rechercher par nom, commande ou emplacement…"},
            {"a.select_all", "Tout sélectionner"},
            {"a.selected", "sélectionnés"},
            {"a.sort", "Trier"},
            {"a.sort_impact", "Impact"},
            {"a.sort_name", "Nom"},
            {"a.sort_source", "Source"},
            {"a.state", "ÉTAT"},
            {"a.state_off", "Désactivé"},
            {"a.state_on", "Activé"},
            {"a.status_items", "éléments"},
            {"a.tab_history", "Historique d'audit"},
            {"a.tab_list", "Liste"},
            {"a.trigger", "Déclencheur"},
            {"a.trigger_boot", "Démarrage"},
            {"a.trigger_event", "Événement"},
            {"a.trigger_idle", "Inactivité"},
            {"a.trigger_logon", "Ouverture de session"},
            {"a.trigger_schedule", "Planification"},
            {"a.trigger_unknown", "Inconnu"},
            {"a.yes", "Oui"},
            {"s.autostart", "Tout ce qui démarre seul : entrées de démarrage et tâches planifiées"},
            {"t.autostart_nav", "Démarrage et tâches"},
            {"s.autostart_nav", "Applications lancées automatiquement"},
            {"t.autostart", "Démarrage et tâches planifiées"},
        };
        const char *de[][2] = {
            {"side.subtitle", "SYSTEMREINIGUNG"},
            {"side.menu", "MENÜ"},
            {"side.analysis", "ANALYSE & SETUP"},
            {"side.storage", "SPEICHER"},
            {"side.diskUsage", "Festplattennutzung"},
            {"side.full", "voll"},
            {"side.theme", "Design"},
            {"side.changeTheme", "Design wechseln"},
            {"t.dashboard", "Übersicht"},
            {"t.cleaner", "Reinigung"},
            {"t.history", "Verlauf"},
            {"t.system", "System"},
            {"t.files", "Dateien"},
            {"t.disk", "Disk"},
            {"t.optimize", "Arbeitsspeicher"},
            {"t.settings", "Einstellungen"},
            {"s.dashboard", "Systemstatus im Überblick"},
            {"s.cleaner", "Speicherplatz sicher zurückgewinnen"},
            {"s.history", "Frühere Reinigungen"},
            {"s.system", "Live-Hardwarepuls"},
            {"s.files", "Große und doppelte Dateien finden"},
            {"s.disk", "Ordnergrößen im Blick"},
            {"disk.up", "Hoch"},
            {"disk.volumes", "Laufwerke"},
            {"disk.empty", "Leerer Ordner"},
            {"disk.cancel", "Stopp"},
            {"s.optimize", "Speicherdruck senken"},
            {"s.settings", "Design, Transparenz und Bewegung"},
            {"d.reclaim", "FREIGEBBARER SPEICHER"},
            {"d.ready", "BEREIT"},
            {"d.scanning", "SCANNE"},
            {"d.analyzing", "Analysiere…"},
            {"d.scanAll", "Alles scannen"},
            {"d.scanning2", "Scanne…"},
            {"d.openCleaner", "Reinigung öffnen"},
            {"d.quickScan", "Schnellscan"},
            {"d.quickScanSub", "Vorschau für alles"},
            {"d.cleaner", "Reinigung"},
            {"d.cleanerSub", "Wählen und reinigen"},
            {"d.appearance", "Darstellung"},
            {"d.appearanceSub", "Design und Transparenz"},
            {"d.system", "SYSTEM"},
            {"d.live", "LIVE"},
            {"d.charts", "DIAGRAMME"},
            {"d.distribution", "Festplattenverteilung"},
            {"d.wave", "CPU-/RAM-Kurve"},
            {"d.noData", "Noch keine Daten"},
            {"d.noDataSub", "Starte einen Scan für freigebbaren Speicher."},
            {"d.lastReport", "Letzter Bericht:"},
            {"d.diskFull", "Platte voll"},
            {"t.schedule", "Zeitplan"},
            {"s.schedule", "Automatische tägliche Reinigung"},
            {"sched.eyebrow", "GEPLANTE REINIGUNG"},
            {"sched.title", "Sweep im Autopilot"},
            {"sched.sub", "Tägliche stille Reinigung"},
            {"sched.backend", "Backend"},
            {"sched.status", "Status"},
            {"sched.hour", "Stunde"},
            {"sched.selection", "Reiniger"},
            {"sched.hint", "z. B. firefox.cache (leer = Standard)"},
            {"sched.enable", "Täglich aktivieren"},
            {"sched.disable", "Deaktivieren"},
            {"sched.working", "Läuft…"},
            {"sched.refresh", "Aktualisieren"},
            {"sched.memopt", "Speicher-Task"},
            {"t.memory", "Arbeitsspeicher"},
            {"s.memory", "Live-Speicherverbrauch und Optimierung"},
            {"t.startup_mgr", "Autostart"},
            {"s.startup_mgr", "Was beim Anmelden startet"},
            {"t.sysinfo", "Systemzustand"},
            {"s.sysinfo", "CPU, Laufwerke, SMART und Temperatur"},
            {"t.network", "Netzwerk"},
            {"s.network", "DNS- und Paket-Caches"},
            {"t.privacy", "Privatsphäre"},
            {"s.privacy", "Telemetrie, zuletzt verwendete Dateien und Zwischenablage"},
            {"g.optimize", "Optimieren"},
            {"g.optimize_aggressive", "Aggressiver Modus (Swap/Standby)"},
            {"g.impact_high", "Hohe Auswirkung"},
            {"g.impact_medium", "Mittlere Auswirkung"},
            {"g.impact_low", "Geringe Auswirkung"},
            {"g.flush_dns", "DNS leeren"},
            {"g.clean_caches", "Paket-Caches bereinigen"},
            {"g.shield", "Telemetrie deaktivieren"},
            {"g.wipe", "Verlauf und Zwischenablage löschen"},
            {"g.search_startup", "Autostart-Einträge suchen…"},
            {"g.health_green", "Gesund"},
            {"g.health_yellow", "Warnung"},
            {"g.health_red", "Kritisch"},
            {"g.uptime", "Laufzeit"},
            {"g.temperature", "Temperatur"},
            {"g.dry_run", "Probelauf (Vorschau)"},
            {"g.recent_files", "Zuletzt verwendete Dateien"},
            {"g.clipboard", "Zwischenablage"},
            {"g.working", "Läuft…"},
            {"g.refresh", "Aktualisieren"},
            {"g.close", "Deaktivieren"},
            {"g.open", "Aktivieren"},
            {"g.cores", "Kerne"},
            {"g.cpu", "CPU"},
            {"g.memory", "Arbeitsspeicher"},
            {"g.swap", "Swap"},
            {"reclaimed", "freigegeben"},
            {"g.needs_root", "Root-/Administratorkennwort erforderlich"},
            {"g.elev_retry", "Als Admin wiederholen"},
            {"w.title", "Willkommen bei Sweep"},
            {"w.sub", "Dein PC – sauber und unter Kontrolle."},
            {"w.tour", "Rundgang starten"},
            {"w.skip", "Überspringen"},
            {"w.next", "Weiter"},
            {"w.back", "Zurück"},
            {"w.done", "Fertig"},
            {"g.no_engine", "Bereinigungs-Engine (sweep) nicht gefunden — bitte neben der App installieren."},
            {"g.apply", "Anwenden"},
            {"g.automem", "Automatische Wartung"},
            {"g.automem_desc", "Prüft alle 5 Min, trimmt bei RAM über 80 %."},
            {"g.smart", "SMART"},
            {"a.all", "Alle"},
            {"a.author", "Herausgeber"},
            {"u.undo_available", "Rückgängig verfügbar"},
            {"u.undo", "Rückgängig"},
            {"u.undo_last", "Letzte Bereinigung rückgängig"},
            {"u.restore", "Wiederherstellen"},
            {"u.runs_title", "Frühere Bereinigungen"},
            {"u.no_runs", "Noch keine früheren Bereinigungen."},
            {"u.backup_title", "Zuerst ein Backup erstellen?"},
            {"u.backup_body", "Wir empfehlen, vor dem Fortfahren ein System-Backup zu erstellen."},
            {"u.create_backup", "Backup erstellen"},
            {"u.continue", "Fortfahren"},
            {"u.review_first", "Prüfe zuerst die Vorschau, dann erneut auf Bereinigen drücken."},
            {"u.backup_open_failed", "Backup-Einstellungen konnten nicht geöffnet werden. Bitte manuell sichern."},
            {"a.backup", "Sicherung"},
            {"a.cancel", "Abbrechen"},
            {"a.capabilities", "FÄHIGKEITEN"},
            {"a.category", "KATEGORIE"},
            {"a.category_driver", "Treiber"},
            {"a.category_os", "BS"},
            {"a.category_security", "Sicherheit"},
            {"a.category_third_party", "Drittanbieter"},
            {"a.category_unknown", "Unbekannt"},
            {"a.category_updater", "Updater"},
            {"a.clear_selection", "Leeren"},
            {"a.command", "Befehl"},
            {"a.confirm", "Bestätigen"},
            {"a.confirm_ack", "Ich verstehe das Risiko"},
            {"a.confirm_body", "Dies ändert die Systemkonfiguration. Fortfahren?"},
            {"a.confirm_critical", "Kritisches Risiko: Deaktivieren oder Entfernen kann Start oder Sicherheit beschädigen."},
            {"a.confirm_title", "Änderung bestätigen"},
            {"a.detail", "DETAIL"},
            {"a.edit", "Befehl bearbeiten"},
            {"a.edit_label", "Neuer Befehl"},
            {"a.edit_title", "Befehl bearbeiten"},
            {"a.editable", "Bearbeitbar"},
            {"a.empty", "Keine Autostart-Einträge gefunden"},
            {"a.empty_hint", "Lockern Sie die Filter."},
            {"a.error", "Fehler"},
            {"a.export_csv", "CSV exportieren"},
            {"a.export_json", "JSON exportieren"},
            {"a.exported", "Exportiert"},
            {"a.eyebrow", "AUTOSTART UND AUFGABEN"},
            {"a.history_empty", "Noch keine Audit-Einträge"},
            {"a.kind", "ART"},
            {"a.kind_all", "Alle"},
            {"a.kind_startup", "Autostart"},
            {"a.kind_task", "Geplant"},
            {"a.last_run", "Letzter Lauf"},
            {"a.location", "Speicherort"},
            {"a.no", "Nein"},
            {"a.no_selection", "Wählen Sie ein Element für Details."},
            {"a.notes", "Hinweise"},
            {"a.reason", "Grund"},
            {"a.removable", "Entfernbar"},
            {"a.remove", "Entfernen"},
            {"a.reversible", "Umkehrbar"},
            {"a.risk", "RISIKO"},
            {"a.risk_critical", "Kritisch"},
            {"a.risk_high", "Hoch"},
            {"a.risk_low", "Niedrig"},
            {"a.risk_medium", "Mittel"},
            {"a.rollback", "Zurücksetzen"},
            {"a.rollback_last", "Letzte Änderung zurücksetzen"},
            {"a.save", "Speichern"},
            {"a.scope", "BEREICH"},
            {"a.scope_system", "System"},
            {"a.scope_user", "Benutzer"},
            {"a.search", "Name, Befehl oder Speicherort suchen…"},
            {"a.select_all", "Alle auswählen"},
            {"a.selected", "ausgewählt"},
            {"a.sort", "Sortieren"},
            {"a.sort_impact", "Auswirkung"},
            {"a.sort_name", "Name"},
            {"a.sort_source", "Quelle"},
            {"a.state", "STATUS"},
            {"a.state_off", "Deaktiviert"},
            {"a.state_on", "Aktiv"},
            {"a.status_items", "Einträge"},
            {"a.tab_history", "Audit-Verlauf"},
            {"a.tab_list", "Liste"},
            {"a.trigger", "Auslöser"},
            {"a.trigger_boot", "Start"},
            {"a.trigger_event", "Ereignis"},
            {"a.trigger_idle", "Leerlauf"},
            {"a.trigger_logon", "Anmeldung"},
            {"a.trigger_schedule", "Zeitplan"},
            {"a.trigger_unknown", "Unbekannt"},
            {"a.yes", "Ja"},
            {"s.autostart", "Alles, was von selbst startet: Autostart-Einträge und geplante Aufgaben"},
            {"t.autostart_nav", "Autostart & Aufgaben"},
            {"s.autostart_nav", "Programme, die automatisch starten"},
            {"t.autostart", "Autostart und geplante Aufgaben"},
        };
        const char *pt[][2] = {
            {"side.subtitle", "LIMPEZA DO SISTEMA"},
            {"side.menu", "MENU"},
            {"side.analysis", "ANÁLISE E AJUSTES"},
            {"side.storage", "ARMAZENAMENTO"},
            {"side.diskUsage", "Uso do disco"},
            {"side.full", "cheio"},
            {"side.theme", "Tema"},
            {"side.changeTheme", "Mudar tema"},
            {"t.dashboard", "Painel"},
            {"t.cleaner", "Limpeza"},
            {"t.history", "Histórico"},
            {"t.system", "Sistema"},
            {"t.files", "Arquivos"},
            {"t.disk", "Disco"},
            {"t.optimize", "Memória"},
            {"t.settings", "Ajustes"},
            {"s.dashboard", "Visão geral do sistema"},
            {"s.cleaner", "Recupere espaço em disco com segurança"},
            {"s.history", "Limpezas anteriores"},
            {"s.system", "Pulso do hardware ao vivo"},
            {"s.files", "Caça arquivos grandes e duplicados"},
            {"s.disk", "Tamanhos de pastas"},
            {"disk.up", "Subir"},
            {"disk.volumes", "Volumes"},
            {"disk.empty", "Pasta vazia"},
            {"disk.cancel", "Parar"},
            {"s.optimize", "Reduzir pressão de memória"},
            {"s.settings", "Tema, transparência e movimento"},
            {"d.reclaim", "ESPAÇO RECUPERÁVEL"},
            {"d.ready", "PRONTO"},
            {"d.scanning", "ANALISANDO"},
            {"d.analyzing", "Analisando…"},
            {"d.scanAll", "Analisar tudo"},
            {"d.scanning2", "Analisando…"},
            {"d.openCleaner", "Abrir limpeza"},
            {"d.quickScan", "Análise rápida"},
            {"d.quickScanSub", "Prévia de tudo"},
            {"d.cleaner", "Limpeza"},
            {"d.cleanerSub", "Selecione e limpe"},
            {"d.appearance", "Aparência"},
            {"d.appearanceSub", "Tema e transparência"},
            {"d.system", "SISTEMA"},
            {"d.live", "AO VIVO"},
            {"d.charts", "GRÁFICOS"},
            {"d.distribution", "Distribuição do disco"},
            {"d.wave", "Onda CPU / RAM"},
            {"d.noData", "Sem dados ainda"},
            {"d.noDataSub", "Execute uma análise para ver o espaço recuperável."},
            {"d.lastReport", "Último relatório:"},
            {"d.diskFull", "disco cheio"},
            {"t.schedule", "Agendamento"},
            {"s.schedule", "Limpeza diária automática"},
            {"sched.eyebrow", "LIMPEZA AGENDADA"},
            {"sched.title", "Sweep no piloto automático"},
            {"sched.sub", "Limpeza silenciosa diária"},
            {"sched.backend", "Motor"},
            {"sched.status", "Estado"},
            {"sched.hour", "Hora"},
            {"sched.selection", "Limpadores"},
            {"sched.hint", "ex.: firefox.cache (vazio = padrão)"},
            {"sched.enable", "Ativar diário"},
            {"sched.disable", "Desativar"},
            {"sched.working", "A trabalhar…"},
            {"sched.refresh", "Atualizar"},
            {"sched.memopt", "Tarefa de memória"},
            {"t.memory", "Memória"},
            {"s.memory", "Uso de memória ao vivo e otimização"},
            {"t.startup_mgr", "Arranque"},
            {"s.startup_mgr", "O que inicia ao entrar"},
            {"t.sysinfo", "Saúde do sistema"},
            {"s.sysinfo", "CPU, discos, SMART e temperatura"},
            {"t.network", "Rede"},
            {"s.network", "DNS e caches de pacotes"},
            {"t.privacy", "Privacidade"},
            {"s.privacy", "Telemetria, ficheiros recentes e área de transferência"},
            {"g.optimize", "Otimizar"},
            {"g.optimize_aggressive", "Modo agressivo (swap/standby)"},
            {"g.impact_high", "Impacto alto"},
            {"g.impact_medium", "Impacto médio"},
            {"g.impact_low", "Impacto baixo"},
            {"g.flush_dns", "Limpar DNS"},
            {"g.clean_caches", "Limpar caches de pacotes"},
            {"g.shield", "Desativar telemetria"},
            {"g.wipe", "Limpar histórico e área de transferência"},
            {"g.search_startup", "Procurar entradas de arranque…"},
            {"g.health_green", "Saudável"},
            {"g.health_yellow", "Aviso"},
            {"g.health_red", "Crítico"},
            {"g.uptime", "Tempo ativo"},
            {"g.temperature", "Temperatura"},
            {"g.dry_run", "Simulação (pré-visualização)"},
            {"g.recent_files", "Ficheiros recentes"},
            {"g.clipboard", "Área de transferência"},
            {"g.working", "A trabalhar…"},
            {"g.refresh", "Atualizar"},
            {"g.close", "Desativar"},
            {"g.open", "Ativar"},
            {"g.cores", "núcleos"},
            {"g.cpu", "CPU"},
            {"g.memory", "Memória"},
            {"g.swap", "Swap"},
            {"reclaimed", "recuperado"},
            {"g.needs_root", "É necessária a palavra-passe de root/administrador"},
            {"g.elev_retry", "Tentar como administrador"},
            {"w.title", "Bem-vindo ao Sweep"},
            {"w.sub", "O teu PC, limpo e sob controlo."},
            {"w.tour", "Fazer uma visita"},
            {"w.skip", "Ignorar"},
            {"w.next", "Seguinte"},
            {"w.back", "Anterior"},
            {"w.done", "Concluído"},
            {"g.no_engine", "Motor de limpeza (sweep) não encontrado — instale-o junto ao app."},
            {"g.apply", "Aplicar"},
            {"g.automem", "Manutenção automática"},
            {"g.automem_desc", "Verifica a cada 5 min, otimiza se RAM passar de 80%."},
            {"g.smart", "SMART"},
            {"a.all", "Tudo"},
            {"u.undo_available", "Desfazer disponível"},
            {"u.undo", "Desfazer"},
            {"u.undo_last", "Desfazer a última limpeza"},
            {"u.restore", "Restaurar"},
            {"u.runs_title", "Limpezas anteriores"},
            {"u.no_runs", "Sem limpezas anteriores."},
            {"u.backup_title", "Criar um backup antes?"},
            {"u.backup_body", "Recomendamos criar um backup do sistema antes de continuar."},
            {"u.create_backup", "Criar backup"},
            {"u.continue", "Continuar"},
            {"u.review_first", "Revise a pré-visualização e prima Limpar novamente."},
            {"u.backup_open_failed", "Não foi possível abrir as configurações de backup. Faça um backup manual."},
            {"a.author", "Autor"},
            {"a.backup", "Cópia"},
            {"a.cancel", "Cancelar"},
            {"a.capabilities", "CAPACIDADES"},
            {"a.category", "CATEGORIA"},
            {"a.category_driver", "Driver"},
            {"a.category_os", "SO"},
            {"a.category_security", "Segurança"},
            {"a.category_third_party", "Terceiros"},
            {"a.category_unknown", "Desconhecido"},
            {"a.category_updater", "Atualizador"},
            {"a.clear_selection", "Limpar"},
            {"a.command", "Comando"},
            {"a.confirm", "Confirmar"},
            {"a.confirm_ack", "Compreendo o risco"},
            {"a.confirm_body", "Isto altera a configuração do sistema. Continuar?"},
            {"a.confirm_critical", "Risco crítico: desativar ou remover pode quebrar a inicialização ou a segurança."},
            {"a.confirm_title", "Confirmar alteração"},
            {"a.detail", "DETALHE"},
            {"a.edit", "Editar comando"},
            {"a.edit_label", "Novo comando"},
            {"a.edit_title", "Editar comando"},
            {"a.editable", "Editável"},
            {"a.empty", "Nenhuma entrada de inicialização encontrada"},
            {"a.empty_hint", "Tente afrouxar os filtros."},
            {"a.error", "Erro"},
            {"a.export_csv", "Exportar CSV"},
            {"a.export_json", "Exportar JSON"},
            {"a.exported", "Exportado"},
            {"a.eyebrow", "INICIALIZAÇÃO E TAREFAS"},
            {"a.history_empty", "Ainda não há registos de auditoria"},
            {"a.kind", "TIPO"},
            {"a.kind_all", "Tudo"},
            {"a.kind_startup", "Inicialização"},
            {"a.kind_task", "Agendadas"},
            {"a.last_run", "Última execução"},
            {"a.location", "Local"},
            {"a.no", "Não"},
            {"a.no_selection", "Selecione um item para ver os detalhes."},
            {"a.notes", "Notas"},
            {"a.reason", "Motivo"},
            {"a.removable", "Removível"},
            {"a.remove", "Remover"},
            {"a.reversible", "Reversível"},
            {"a.risk", "RISCO"},
            {"a.risk_critical", "Crítico"},
            {"a.risk_high", "Alto"},
            {"a.risk_low", "Baixo"},
            {"a.risk_medium", "Médio"},
            {"a.rollback", "Reverter"},
            {"a.rollback_last", "Reverter a última alteração"},
            {"a.save", "Guardar"},
            {"a.scope", "ESCOPO"},
            {"a.scope_system", "Sistema"},
            {"a.scope_user", "Usuário"},
            {"a.search", "Pesquisar por nome, comando ou local…"},
            {"a.select_all", "Selecionar tudo"},
            {"a.selected", "selecionados"},
            {"a.sort", "Ordenar"},
            {"a.sort_impact", "Impacto"},
            {"a.sort_name", "Nome"},
            {"a.sort_source", "Origem"},
            {"a.state", "ESTADO"},
            {"a.state_off", "Desativado"},
            {"a.state_on", "Ativo"},
            {"a.status_items", "itens"},
            {"a.tab_history", "Histórico de auditoria"},
            {"a.tab_list", "Lista"},
            {"a.trigger", "Gatilho"},
            {"a.trigger_boot", "Arranque"},
            {"a.trigger_event", "Evento"},
            {"a.trigger_idle", "Inatividade"},
            {"a.trigger_logon", "Início de sessão"},
            {"a.trigger_schedule", "Agendamento"},
            {"a.trigger_unknown", "Desconhecido"},
            {"a.yes", "Sim"},
            {"s.autostart", "Tudo o que inicia sozinho: entradas de inicialização e tarefas agendadas"},
            {"t.autostart_nav", "Inicialização e tarefas"},
            {"s.autostart_nav", "Aplicações que iniciam sozinhas"},
            {"t.autostart", "Inicialização e tarefas agendadas"},
        };
        const char *it[][2] = {
            {"side.subtitle", "PULIZIA SISTEMA"},
            {"side.menu", "MENU"},
            {"side.analysis", "ANALISI E IMPOSTAZIONI"},
            {"side.storage", "ARCHIVIO"},
            {"side.diskUsage", "Uso del disco"},
            {"side.full", "pieno"},
            {"side.theme", "Tema"},
            {"side.changeTheme", "Cambia tema"},
            {"t.dashboard", "Pannello"},
            {"t.cleaner", "Pulizia"},
            {"t.history", "Cronologia"},
            {"t.system", "Sistema"},
            {"t.files", "File"},
            {"t.disk", "Disco"},
            {"t.optimize", "Memoria"},
            {"t.settings", "Impostazioni"},
            {"s.dashboard", "Panoramica dello stato di sistema"},
            {"s.cleaner", "Recupera spazio su disco in sicurezza"},
            {"s.history", "Pulizie precedenti"},
            {"s.system", "Segnale hardware in diretta"},
            {"s.files", "Caccia a file grandi e duplicati"},
            {"s.disk", "Dimensioni cartelle"},
            {"disk.up", "Su"},
            {"disk.volumes", "Volumi"},
            {"disk.empty", "Cartella vuota"},
            {"disk.cancel", "Ferma"},
            {"s.optimize", "Riduci la pressione sulla memoria"},
            {"s.settings", "Tema, trasparenza e movimento"},
            {"d.reclaim", "SPAZIO RECUPERABILE"},
            {"d.ready", "PRONTO"},
            {"d.scanning", "SCANSIONE"},
            {"d.analyzing", "Analisi…"},
            {"d.scanAll", "Scansiona tutto"},
            {"d.scanning2", "Scansione…"},
            {"d.openCleaner", "Apri pulizia"},
            {"d.quickScan", "Scansione rapida"},
            {"d.quickScanSub", "Anteprima di tutto"},
            {"d.cleaner", "Pulizia"},
            {"d.cleanerSub", "Seleziona e pulisci"},
            {"d.appearance", "Aspetto"},
            {"d.appearanceSub", "Tema e trasparenza"},
            {"d.system", "SISTEMA"},
            {"d.live", "IN DIRETTA"},
            {"d.charts", "GRAFICI"},
            {"d.distribution", "Distribuzione del disco"},
            {"d.wave", "Onda CPU / RAM"},
            {"d.noData", "Ancora nessun dato"},
            {"d.noDataSub", "Esegui una scansione per vedere lo spazio."},
            {"d.lastReport", "Ultimo rapporto:"},
            {"d.diskFull", "disco pieno"},
            {"t.schedule", "Pianificazione"},
            {"s.schedule", "Pulizia giornaliera automatica"},
            {"sched.eyebrow", "PULIZIA PIANIFICATA"},
            {"sched.title", "Sweep con il pilota automatico"},
            {"sched.sub", "Pulizia silenziosa giornaliera"},
            {"sched.backend", "Motore"},
            {"sched.status", "Stato"},
            {"sched.hour", "Ora"},
            {"sched.selection", "Pulitori"},
            {"sched.hint", "es. firefox.cache (vuoto = predefinito)"},
            {"sched.enable", "Attiva giornaliero"},
            {"sched.disable", "Disattiva"},
            {"sched.working", "Operativo…"},
            {"sched.refresh", "Aggiorna"},
            {"sched.memopt", "Attività memoria"},
            {"t.memory", "Memoria"},
            {"s.memory", "Uso memoria in tempo reale e ottimizzazione"},
            {"t.startup_mgr", "Avvio"},
            {"s.startup_mgr", "Cosa si avvia all'accesso"},
            {"t.sysinfo", "Salute del sistema"},
            {"s.sysinfo", "CPU, dischi, SMART e temperatura"},
            {"t.network", "Rete"},
            {"s.network", "DNS e cache dei pacchetti"},
            {"t.privacy", "Privacy"},
            {"s.privacy", "Telemetria, file recenti e appunti"},
            {"g.optimize", "Ottimizza"},
            {"g.optimize_aggressive", "Modalità aggressiva (swap/standby)"},
            {"g.impact_high", "Impatto alto"},
            {"g.impact_medium", "Impatto medio"},
            {"g.impact_low", "Impatto basso"},
            {"g.flush_dns", "Svuota DNS"},
            {"g.clean_caches", "Pulisci cache dei pacchetti"},
            {"g.shield", "Disattiva telemetria"},
            {"g.wipe", "Cancella cronologia e appunti"},
            {"g.search_startup", "Cerca voci di avvio…"},
            {"g.health_green", "Sano"},
            {"g.health_yellow", "Avviso"},
            {"g.health_red", "Critico"},
            {"g.uptime", "Tempo di attività"},
            {"g.temperature", "Temperatura"},
            {"g.dry_run", "Prova (anteprima)"},
            {"g.recent_files", "File recenti"},
            {"g.clipboard", "Appunti"},
            {"g.working", "Operativo…"},
            {"g.refresh", "Aggiorna"},
            {"g.close", "Disattiva"},
            {"g.open", "Attiva"},
            {"g.cores", "core"},
            {"g.cpu", "CPU"},
            {"g.memory", "Memoria"},
            {"g.swap", "Swap"},
            {"reclaimed", "recuperato"},
            {"g.needs_root", "Serve la password di root/amministratore"},
            {"g.elev_retry", "Riprova come amministratore"},
            {"w.title", "Benvenuto in Sweep"},
            {"w.sub", "Il tuo PC, pulito e sotto controllo."},
            {"w.tour", "Fai il tour"},
            {"w.skip", "Salta"},
            {"w.next", "Avanti"},
            {"w.back", "Indietro"},
            {"w.done", "Fatto"},
            {"g.no_engine", "Motore di pulizia (sweep) non trovato — installalo accanto all'app."},
            {"g.apply", "Applica"},
            {"g.automem", "Manutenzione automatica"},
            {"g.automem_desc", "Controlla ogni 5 min, ottimizza se la RAM supera l'80%."},
            {"g.smart", "SMART"},
            {"a.all", "Tutti"},
            {"a.author", "Autore"},
            {"u.undo_available", "Annulla disponibile"},
            {"u.undo", "Annulla"},
            {"u.undo_last", "Annulla l'ultima pulizia"},
            {"u.restore", "Ripristina"},
            {"u.runs_title", "Pulizie precedenti"},
            {"u.no_runs", "Nessuna pulizia precedente."},
            {"u.backup_title", "Creare prima un backup?"},
            {"u.backup_body", "Consigliamo di creare un backup di sistema prima di continuare."},
            {"u.create_backup", "Crea backup"},
            {"u.continue", "Continua"},
            {"u.review_first", "Controlla l'anteprima, poi premi di nuovo Pulisci."},
            {"u.backup_open_failed", "Impossibile aprire le impostazioni di backup. Esegui un backup manuale."},
            {"a.backup", "Backup"},
            {"a.cancel", "Annulla"},
            {"a.capabilities", "CAPACITÀ"},
            {"a.category", "CATEGORIA"},
            {"a.category_driver", "Driver"},
            {"a.category_os", "SO"},
            {"a.category_security", "Sicurezza"},
            {"a.category_third_party", "Terze parti"},
            {"a.category_unknown", "Sconosciuto"},
            {"a.category_updater", "Aggiornamento"},
            {"a.clear_selection", "Pulisci"},
            {"a.command", "Comando"},
            {"a.confirm", "Conferma"},
            {"a.confirm_ack", "Comprendo il rischio"},
            {"a.confirm_body", "Questo modifica la configurazione di sistema. Continuare?"},
            {"a.confirm_critical", "Rischio critico: disattivarlo o rimuoverlo può compromettere l'avvio o la sicurezza."},
            {"a.confirm_title", "Conferma modifica"},
            {"a.detail", "DETTAGLIO"},
            {"a.edit", "Modifica comando"},
            {"a.edit_label", "Nuovo comando"},
            {"a.edit_title", "Modifica comando"},
            {"a.editable", "Modificabile"},
            {"a.empty", "Nessuna voce di avvio trovata"},
            {"a.empty_hint", "Prova ad allentare i filtri."},
            {"a.error", "Errore"},
            {"a.export_csv", "Esporta CSV"},
            {"a.export_json", "Esporta JSON"},
            {"a.exported", "Esportato"},
            {"a.eyebrow", "AVVIO E ATTIVITÀ"},
            {"a.history_empty", "Nessuna voce di controllo"},
            {"a.kind", "TIPO"},
            {"a.kind_all", "Tutti"},
            {"a.kind_startup", "Avvio"},
            {"a.kind_task", "Pianificate"},
            {"a.last_run", "Ultima esecuzione"},
            {"a.location", "Percorso"},
            {"a.no", "No"},
            {"a.no_selection", "Seleziona un elemento per vedere i dettagli."},
            {"a.notes", "Note"},
            {"a.reason", "Motivo"},
            {"a.removable", "Rimuovibile"},
            {"a.remove", "Rimuovi"},
            {"a.reversible", "Reversibile"},
            {"a.risk", "RISCHIO"},
            {"a.risk_critical", "Critico"},
            {"a.risk_high", "Alto"},
            {"a.risk_low", "Basso"},
            {"a.risk_medium", "Medio"},
            {"a.rollback", "Ripristina"},
            {"a.rollback_last", "Ripristina l'ultima modifica"},
            {"a.save", "Salva"},
            {"a.scope", "AMBITO"},
            {"a.scope_system", "Sistema"},
            {"a.scope_user", "Utente"},
            {"a.search", "Cerca per nome, comando o percorso…"},
            {"a.select_all", "Seleziona tutto"},
            {"a.selected", "selezionati"},
            {"a.sort", "Ordina"},
            {"a.sort_impact", "Impatto"},
            {"a.sort_name", "Nome"},
            {"a.sort_source", "Origine"},
            {"a.state", "STATO"},
            {"a.state_off", "Disattivato"},
            {"a.state_on", "Attivo"},
            {"a.status_items", "elementi"},
            {"a.tab_history", "Cronologia di controllo"},
            {"a.tab_list", "Elenco"},
            {"a.trigger", "Trigger"},
            {"a.trigger_boot", "Avvio"},
            {"a.trigger_event", "Evento"},
            {"a.trigger_idle", "Inattività"},
            {"a.trigger_logon", "Accesso"},
            {"a.trigger_schedule", "Pianificazione"},
            {"a.trigger_unknown", "Sconosciuto"},
            {"a.yes", "Sì"},
            {"s.autostart", "Tutto ciò che parte da solo: voci di avvio e attività pianificate"},
            {"t.autostart_nav", "Avvio e attività"},
            {"s.autostart_nav", "App che partono da sole"},
            {"t.autostart", "Avvio e attività pianificate"},
        };
        for (const auto &pair : en) m_en.insert(QString::fromUtf8(pair[0]), QString::fromUtf8(pair[1]));
        for (const auto &pair : tr) m_tr.insert(QString::fromUtf8(pair[0]), QString::fromUtf8(pair[1]));
        for (const auto &pair : es) m_es.insert(QString::fromUtf8(pair[0]), QString::fromUtf8(pair[1]));
        for (const auto &pair : ru) m_ru.insert(QString::fromUtf8(pair[0]), QString::fromUtf8(pair[1]));
        for (const auto &pair : fr) m_fr.insert(QString::fromUtf8(pair[0]), QString::fromUtf8(pair[1]));
        for (const auto &pair : de) m_de.insert(QString::fromUtf8(pair[0]), QString::fromUtf8(pair[1]));
        for (const auto &pair : pt) m_pt.insert(QString::fromUtf8(pair[0]), QString::fromUtf8(pair[1]));
        for (const auto &pair : it) m_it.insert(QString::fromUtf8(pair[0]), QString::fromUtf8(pair[1]));
    }
    QString lang() const { return m_lang; }
    void setLang(const QString &v) {
        if (v == m_lang) return;
        m_lang = v;
        emit langChanged();
    }
    Q_INVOKABLE QString tr(const QString &key) const {
        if (m_lang == QLatin1String("tr") && m_tr.contains(key)) return m_tr.value(key);
        if (m_lang == QLatin1String("es") && m_es.contains(key)) return m_es.value(key);
        if (m_lang == QLatin1String("ru") && m_ru.contains(key)) return m_ru.value(key);
        if (m_lang == QLatin1String("fr") && m_fr.contains(key)) return m_fr.value(key);
        if (m_lang == QLatin1String("de") && m_de.contains(key)) return m_de.value(key);
        if (m_lang == QLatin1String("pt") && m_pt.contains(key)) return m_pt.value(key);
        if (m_lang == QLatin1String("it") && m_it.contains(key)) return m_it.value(key);
        if (m_en.contains(key)) return m_en.value(key);
        return key;
    }
signals:
    void langChanged();
private:
    QString m_lang = QStringLiteral("tr");
    QHash<QString, QString> m_en, m_tr, m_es, m_ru, m_fr, m_de, m_pt, m_it;
};

// ---------------------------------------------------------------------------
// Tray: sistem tepsi simgesi + menü + bildirimler.
// ---------------------------------------------------------------------------
class Tray : public QObject {
    Q_OBJECT
public:
    explicit Tray(const QIcon &icon, QObject *parent = nullptr) : QObject(parent) {
        m_tray.setIcon(icon);
        m_tray.setToolTip(QStringLiteral("Sweep"));
        QMenu *menu = new QMenu();
        QAction *open = menu->addAction(QStringLiteral("Sweep"));
        QAction *scan = menu->addAction(QStringLiteral("Hızlı tara"));
        QAction *settings = menu->addAction(QStringLiteral("Ayarlar"));
        menu->addSeparator();
        QAction *quit = menu->addAction(QStringLiteral("Çıkış"));
        m_tray.setContextMenu(menu);
        connect(open, &QAction::triggered, this, &Tray::showRequested);
        connect(scan, &QAction::triggered, this, &Tray::scanRequested);
        connect(settings, &QAction::triggered, this, &Tray::settingsRequested);
        connect(quit, &QAction::triggered, this, &Tray::quitRequested);
        connect(&m_tray, &QSystemTrayIcon::activated, this, [this](QSystemTrayIcon::ActivationReason r) {
            if (r == QSystemTrayIcon::Trigger || r == QSystemTrayIcon::DoubleClick)
                emit showRequested();
        });
        m_tray.show();
    }
    void notify(const QString &title, const QString &message) {
        if (QSystemTrayIcon::supportsMessages())
            m_tray.showMessage(title, message, QSystemTrayIcon::Information, 6000);
    }
signals:
    void showRequested();
    void scanRequested();
    void settingsRequested();
    void quitRequested();
private:
    QSystemTrayIcon m_tray;
};

// Async bridge: runs the installed `sweep` CLI, streams signals to QML.
// No cxx-qt, no Rust linkage — the --json CLI is the stable interface.
class SweepBridge : public QObject {
    Q_OBJECT
    Q_PROPERTY(double progress READ progress NOTIFY progressChanged)
    Q_PROPERTY(bool elevating READ elevating NOTIFY elevatingChanged)
public:
    explicit SweepBridge(QObject *parent = nullptr) : QObject(parent) {
        m_sweepBin = resolveSweepBin();
        connect(&m_proc, &QProcess::finished, this, &SweepBridge::onFinished);
        connect(&m_proc, &QProcess::readyReadStandardOutput,
                this, &SweepBridge::onOutput);
        connect(&m_proc, &QProcess::readyReadStandardError,
                this, &SweepBridge::onStderr);
        connect(&m_elevTimer, &QTimer::timeout, this, &SweepBridge::pollElevated);
        m_elevTimer.setInterval(300);
    }
    double progress() const { return m_progress; }
    bool elevating() const { return m_elevating; }
    Q_INVOKABLE void listCleaners() { startOp(Op::List, {"--quiet", "--json", "list"}); }
    Q_INVOKABLE void preview(const QStringList &sel) {
        // `--summary` keeps the report at a few kB: the preview of everything is
        // one entry per matched file, i.e. tens of megabytes of JSON that the
        // GUI would have to parse on its UI thread.
        QStringList args{"--quiet", "--summary", "--json", "preview"};
        if (sel.isEmpty()) args << "--all";
        else args += sel;
        startOp(Op::Preview, args);
    }
    // GUI temizlikleri her zaman yedekli yapılır: tek tıkla geri alma
    // (`undo --last`) ancak bu dizindeki yedeklerle çalışır.
    Q_INVOKABLE QString backupDir() const {
        return QStandardPaths::writableLocation(QStandardPaths::AppDataLocation)
            + QStringLiteral("/sweep-backups");
    }
    Q_INVOKABLE void clean(const QStringList &sel) {
        QStringList args{"--quiet", "--summary", "--json", "--yes", "clean",
                         "--backup-dir", backupDir()};
        if (sel.isEmpty()) args << "--all";
        else args += sel;
        startOp(Op::Clean, args);
    }
    Q_INVOKABLE void undoLast() {
        startOp(Op::Undo, {"--quiet", "--json", "--yes", "undo",
                           "--backup-dir", backupDir(), "--last"});
    }
    Q_INVOKABLE void undoRun(const QString &id) {
        if (id.isEmpty()) { undoLast(); return; }
        startOp(Op::Undo, {"--quiet", "--json", "--yes", "undo",
                           "--backup-dir", backupDir(), "--run", id});
    }
    Q_INVOKABLE void undoList() {
        startOp(Op::UndoList, {"--quiet", "--json", "undo",
                               "--backup-dir", backupDir(), "--list"});
    }
    // OS-native backup hedefi: icat yok, her platformda gerçek karşılık.
    // Başarısızlıkta false döner; QML manuel-yedek yönergesini gösterir.
    Q_INVOKABLE bool openSystemBackup() {
#ifdef Q_OS_WIN
        return QDesktopServices::openUrl(QUrl(QStringLiteral("ms-settings:backup")));
#elif defined(Q_OS_MACOS)
        return QDesktopServices::openUrl(QUrl(QStringLiteral(
            "x-apple.systempreferences:com.apple.TimeMachine-Settings")));
#else
        const QString prefs = QStandardPaths::findExecutable(QStringLiteral("deja-dup-preferences"));
        if (!prefs.isEmpty())
            return QProcess::startDetached(prefs, {});
        const QString app = QStandardPaths::findExecutable(QStringLiteral("deja-dup"));
        if (!app.isEmpty())
            return QProcess::startDetached(app, {});
        return false;
#endif
    }
    Q_INVOKABLE void history() { startOp(Op::History, {"--quiet", "--json", "history"}); }
    Q_INVOKABLE void bigfiles(int top) { startOp(Op::Bigfiles, {"--quiet", "--json", "bigfiles", "--top", QString::number(top > 0 ? top : 20)}); }
    // Disk ekrani: klasor olcumu (salt okunur `sweep du`, async + iptalli).
    Q_INVOKABLE void du(const QString &path) { startOp(Op::Du, {"--quiet", "--json", "du", path}); }
    // Kopru mesgul mu? Oto-bakim sayaci ust uste is bindirmesin.
    Q_INVOKABLE bool busy() const { return m_proc.state() != QProcess::NotRunning || m_elevating; }
    // Bagli birimler (Windows'ta C:, D: ...): QStorageInfo capraz platformdur.
    Q_INVOKABLE QString volumes() {
        QJsonArray arr;
        for (const QStorageInfo &v : QStorageInfo::mountedVolumes()) {
            if (!v.isReady() || v.isReadOnly() || v.bytesTotal() <= 0) continue;
            QJsonObject o;
            o[QStringLiteral("root")] = v.rootPath();
            o[QStringLiteral("name")] = v.displayName();
            o[QStringLiteral("total")] = double(v.bytesTotal());
            o[QStringLiteral("free")] = double(v.bytesAvailable());
            arr.append(o);
        }
        return QString::fromUtf8(QJsonDocument(arr).toJson(QJsonDocument::Compact));
    }
    Q_INVOKABLE void dupes() { startOp(Op::Dupes, {"--quiet", "--json", "dupes"}); }
    Q_INVOKABLE void diagnostics() { startOp(Op::Diagnostics, {"--quiet", "--json", "diagnostics"}); }
    Q_INVOKABLE void memopt() { startOp(Op::Memopt, {"--quiet", "--json", "memopt"}); }
    Q_INVOKABLE void memoptWithSudo(const QString &password) {
#ifdef Q_OS_WIN
        // Windows'ta sudo yok: memopt artık gerçek iş yapar (EmptyWorkingSet),
        // yükseltme gerekmez.
        Q_UNUSED(password);
        memopt();
#else
        if (m_proc.state() != QProcess::NotRunning) {
            emit failed(QStringLiteral("busy"));
            return;
        }
        m_op = Op::MemoptSudo;
        m_cancelled = false;
        m_buf.clear();
        m_tail.clear();
        setProgress(0.5);
        m_proc.start(QStringLiteral("sudo"),
                     {"-S", "-k", "-p", QStringLiteral(""), "--", m_sweepBin, "--quiet", "--json", "memopt"});
        if (m_proc.state() == QProcess::NotRunning)
            emit failed(QStringLiteral("cannot start sudo"));
        if (!m_proc.waitForStarted(3000))
            emit failed(QStringLiteral("sudo did not start"));
        m_proc.write(password.toUtf8() + "\n");
#endif
    }
    // --- Disk-dışı sistem zekası: bellek / başlangıç / sysinfo / ağ / gizlilik
    // Hepsi `sweep` CLI'ının --json çıktısını döndürür (QML JSON.parse eder).
    // Agresif bellek optimizasyonu yükseltme isteyebilir; sonuç `memoptReady`
    // sinyaliyle gelir, bu yüzden o tek çağrı eşzamanlı değil `startOp`tur.
    Q_INVOKABLE void memoptAggressive() {
        startOp(Op::Memopt, {"--quiet", "--json", "memopt", "--aggressive"});
    }
    Q_INVOKABLE QString memoptDryRun() {
        return runSweepSync({"memopt", "--dry-run", "--json"}, 20000);
    }
    // Temizlik motoru bulundu mu? Bulunamazsa bellek ekrani sessizce bos
    // kalmak yerine aciklama bandi gosterir (Windows'ta exe yalniz
    // calistirildiginda olur).
    Q_INVOKABLE bool sweepBinOk() const {
        const QFileInfo f(m_sweepBin);
        if (f.isFile()) return true;
        return !QStandardPaths::findExecutable(m_sweepBin).isEmpty();
    }
    Q_INVOKABLE QString sweepBinPath() const { return m_sweepBin; }
    Q_INVOKABLE void memoptAggressiveWithSudo(const QString &password) {
#ifdef Q_OS_WIN
        // Windows'ta sudo yok: yükseltme UAC ile ayrı süreçte yapılır.
        Q_UNUSED(password);
        memoptAggressiveElevated();
#else
        if (m_proc.state() != QProcess::NotRunning) {
            emit failed(QStringLiteral("busy"));
            return;
        }
        m_op = Op::MemoptSudo;
        m_cancelled = false;
        m_buf.clear();
        m_tail.clear();
        setProgress(0.5);
        m_proc.start(QStringLiteral("sudo"),
                     {"-S", "-k", "-p", QStringLiteral(""), "--", m_sweepBin,
                      "--quiet", "--json", "memopt", "--aggressive"});
        if (m_proc.state() == QProcess::NotRunning)
            emit failed(QStringLiteral("cannot start sudo"));
        if (!m_proc.waitForStarted(3000))
            emit failed(QStringLiteral("sudo did not start"));
        m_proc.write(password.toUtf8() + "\n");
#endif
    }
    Q_INVOKABLE void memoptAggressiveElevated() {
        runElevated({"memopt", "--aggressive", "--quiet", "--json"});
    }
    Q_INVOKABLE QString startupList(bool impact) {
        QStringList args{"startup", "list", "--json"};
        if (impact) args << "--impact";
        return runSweepSync(args, 30000);
    }
    Q_INVOKABLE QString startupSet(const QString &id, bool enabled) {
        return runSweepSync({"startup", enabled ? QStringLiteral("enable")
                                                : QStringLiteral("disable"), id},
                            20000);
    }
    // --- Başlangıç ve Zamanlanmış Görevler ekranı -------------------------
    // Filtreler burada CLI bayraklarına çevrilir; QML ham komut satırı kurmaz.
    // Liste uçları senkrondur: mevcut `startupList` de senkrondur ve dönen
    // QString'i QML `JSON.parse` eder. Kayıt defteri + Görev Zamanlayıcı
    // numaralandırması meşgul makinelerde yavaş olabildiği için cömert bir
    // zaman aşımı (60 sn) verilir; QML çağrıyı bir sonraki kareye erteleyip
    // yükleme durumunu gösterir.
    Q_INVOKABLE QString autostartList(const QString &argsJson) {
        QStringList args{"--quiet", "--json", "startup", "list"};
        args += autostartFilterArgs(argsJson, false);
        args << "--impact";
        return runSweepSync(args, 60000);
    }
    Q_INVOKABLE QString tasksList(const QString &argsJson) {
        QStringList args{"--quiet", "--json", "tasks", "list"};
        args += autostartFilterArgs(argsJson, true);
        return runSweepSync(args, 60000);
    }
    // Tek giriş noktası: {"action":"enable|disable|remove|edit","id":…,"command":…}
    // `startup` op yüzeyi Discovery::All üzerinde çalıştığı için hem başlangıç
    // girdilerini hem zamanlanmış görevleri hedefler.
    Q_INVOKABLE QString autostartAct(const QString &argsJson) {
        const QJsonObject o = parseJsonObject(argsJson);
        const QString action = o.value(QStringLiteral("action")).toString().trimmed();
        const QString id = o.value(QStringLiteral("id")).toString().trimmed();
        const QString command = o.value(QStringLiteral("command")).toString();
        if (action.isEmpty() || id.isEmpty())
            return QStringLiteral("error: action and id are required");
        QStringList args{"--quiet", "--json", "startup", action, id};
        if (action == QLatin1String("edit") && !command.isEmpty())
            args << QStringLiteral("--command") << command;
        return runSweepSync(args, 30000);
    }
    Q_INVOKABLE QString autostartRollback(const QString &id, bool last) {
        QStringList args{"--quiet", "--json", "startup", "rollback"};
        if (!id.trimmed().isEmpty()) args << id;
        if (last) args << QStringLiteral("--last");
        return runSweepSync(args, 30000);
    }
    Q_INVOKABLE QString autostartHistory(int limit) {
        return runSweepSync({"--quiet", "--json", "startup", "history",
                             "--limit", QString::number(limit > 0 ? limit : 100)},
                            30000);
    }
    // Denetim defterini dışa aktarır (JSON/CSV). Göreli yol Belgeler klasörüne
    // çözülür; QML yalnız dosya adı gönderir ve sonucu JSON'dan okur.
    Q_INVOKABLE QString autostartExport(const QString &path, const QString &format) {
        QString target = path.trimmed();
        if (target.isEmpty()) return QStringLiteral("error: path required");
        if (QFileInfo(target).isRelative()) {
            QString dir = QStandardPaths::writableLocation(QStandardPaths::DocumentsLocation);
            if (dir.isEmpty()) dir = QDir::homePath();
            target = QDir(dir).filePath(target);
        }
        QStringList args{"--quiet", "--json", "startup", "history",
                         "--limit", QStringLiteral("0"), "--to", target};
        const QString fmt = format.trimmed();
        if (!fmt.isEmpty()) args << QStringLiteral("--format") << fmt;
        return runSweepSync(args, 30000);
    }
    Q_INVOKABLE QString sysinfoDump() { return runSweepSync({"sysinfo", "--json"}, 30000); }
    Q_INVOKABLE QString sysinfoHealth() {
        return runSweepSync({"sysinfo", "--health", "--json"}, 30000);
    }
    Q_INVOKABLE QString networkFlush() {
        return runSweepSync({"network", "flush", "--json"}, 20000);
    }
    Q_INVOKABLE QString networkClean(bool dryRun) {
        QStringList args{"network", "clean", "--json"};
        if (dryRun) args << "--dry-run";
        return runSweepSync(args, 60000);
    }
    Q_INVOKABLE QString privacyShield(bool dryRun) {
        QStringList args{"privacy", "shield", "--json"};
        if (dryRun) args << "--dry-run";
        return runSweepSync(args, 30000);
    }
    Q_INVOKABLE QString privacyWipe(bool dryRun) {
        QStringList args{"privacy", "wipe", "--json"};
        if (dryRun) args << "--dry-run";
        return runSweepSync(args, 30000);
    }
    Q_INVOKABLE bool needsElevated() {
#ifdef Q_OS_WIN
        // `net session` yalnız yükseltilmiş yöneticide 0 döner;
        // ek kitaplık ya da manifest gerekmez.
        QProcess p;
        p.start(QStringLiteral("net"), {QStringLiteral("session")});
        if (!p.waitForFinished(3000)) return false;
        return p.exitCode() != 0;
#else
        QProcess p;
        p.start(QStringLiteral("id"), {"-u"});
        p.waitForFinished(3000);
        return p.readAllStandardOutput().trimmed() != "0";
#endif
    }
    // Yükseltilmiş (UAC) ayrı bir `sweep.exe` süreci çalıştırır ve JSON
    // çıktısını geçici dosyadan okuyup normal sinyallerle QML'ye iletir.
    // `args` ilk öğesi operasyondur: clean / preview / memopt / ...
    Q_INVOKABLE void runElevated(const QStringList &args) {
#ifdef Q_OS_WIN
        if (m_proc.state() != QProcess::NotRunning || m_elevating) {
            emit failed(QStringLiteral("busy"));
            return;
        }
        const QString outFile = QDir::temp().filePath(
            QStringLiteral("sweep-elevated-%1.json").arg(QCoreApplication::applicationPid()));
        QFile::remove(outFile);
        // cmd /c ""C:\...\sweep.exe" --quiet --json clean ... > "out.json" 2>&1"
        // Every elevated op must speak JSON: the result comes back through the
        // same signals as a normal run. `clean` used to be elevated *without*
        // `--json`, so the GUI parsed a human report and reported "clean failed"
        // while the deletion had in fact gone through.
        QStringList full{QStringLiteral("--quiet"), QStringLiteral("--json")};
        full += args;
        // Bosluklu argumanlari (or. bosluklu gorev kimligi) tirnakla; yoksa
        // cmd satiri bolunur ve yukseltlmis cagri yanlis hedefe gider.
        for (int i = 0; i < full.size(); ++i)
            if (full.at(i).contains(QLatin1Char(' ')))
                full[i] = QStringLiteral("\"") + full.at(i) + QStringLiteral("\"");
        const QString cmdLine = QStringLiteral("\"\"%1\" %2 > \"%3\" 2>&1\"")
            .arg(m_sweepBin, full.join(QLatin1Char(' ')), QDir::toNativeSeparators(outFile));
        SHELLEXECUTEINFOW sei = {};
        sei.cbSize = sizeof(sei);
        sei.fMask = SEE_MASK_NOCLOSEPROCESS;
        sei.lpVerb = L"runas";
        sei.lpFile = L"cmd.exe";
        const std::wstring params = cmdLine.toStdWString();
        sei.lpParameters = params.c_str();
        sei.nShow = SW_HIDE;
        if (!ShellExecuteExW(&sei)) {
            // Kullanıcı UAC istemini reddettiğinde 1223 (ERROR_CANCELLED) döner.
            emit failed(QStringLiteral("elevation cancelled"));
            return;
        }
        m_elevHandle = sei.hProcess;
        m_elevOut = outFile;
        m_elevOp = args.value(0);
        m_elevating = true;
        setProgress(0.5);
        emit elevatingChanged();
        m_elevTimer.start();
#else
        Q_UNUSED(args);
        emit failed(QStringLiteral("elevation is Windows-only"));
#endif
    }
    Q_INVOKABLE void cancel() {
        if (m_proc.state() != QProcess::NotRunning) {
            m_cancelled = true;
            m_proc.terminate();
            QTimer::singleShot(2000, this, [this]() {
                if (m_proc.state() != QProcess::NotRunning) m_proc.kill();
            });
        }
    }
    // Çıkışta arka plan işini öldür: yoksa uzun tarama (`sweep du` gibi)
    // yetim kalıp Görev Yöneticisi'nde ikinci bir "sweep" gibi görünür.
    // Tepsiye inerken DEĞİL yalnız gerçekten çıkarken çağrılır (aboutToQuit).
    void stopBackgroundWork() {
        if (m_proc.state() != QProcess::NotRunning) {
            m_proc.kill();
            m_proc.waitForFinished(2000);
        }
    }
    // argsJson → CLI bayrakları (başlangıç/görev listeleri için). Bozuk ya da
    // eksik JSON filtresiz listeye düşer; hatalı *eylem* yükü çağıranda açıkça
    // reddedilir. `tasks` yalnız `tasks` komutunun desteklediği bayrakları alır.
    static QJsonObject parseJsonObject(const QString &argsJson) {
        const QJsonDocument doc = QJsonDocument::fromJson(argsJson.toUtf8());
        return doc.isObject() ? doc.object() : QJsonObject();
    }
    static QStringList autostartFilterArgs(const QString &argsJson, bool tasks) {
        const QJsonObject o = parseJsonObject(argsJson);
        auto text = [&o](const char *k) {
            return o.value(QLatin1String(k)).toString().trimmed();
        };
        QStringList args;
        const QString state = text("state");
        if (state == QLatin1String("enabled")) args << QStringLiteral("--enabled");
        else if (state == QLatin1String("disabled")) args << QStringLiteral("--disabled");
        const QString search = text("search");
        if (!search.isEmpty()) args << QStringLiteral("--search") << search;
        const QString sort = text("sort");
        if (!sort.isEmpty()) args << QStringLiteral("--sort") << sort;
        if (tasks) {
            const QString trigger = text("trigger");
            if (!trigger.isEmpty()) args << QStringLiteral("--trigger") << trigger;
        } else {
            const QString kind = text("kind");
            if (!kind.isEmpty()) args << QStringLiteral("--kind") << kind;
            const QString scope = text("scope");
            if (!scope.isEmpty()) args << QStringLiteral("--scope") << scope;
            const QString category = text("category");
            if (!category.isEmpty()) args << QStringLiteral("--category") << category;
            const QString risk = text("risk");
            if (!risk.isEmpty()) args << QStringLiteral("--risk") << risk;
        }
        return args;
    }
    // Zamanlanmış temizlik: eşzamanlı kısa komutlar (çıktı birkaç satır).
    QString runSweepSync(const QStringList &args, int timeoutMs = 15000) {
        QProcess p;
        p.start(m_sweepBin, args);
        // Baslayamadiysa (motor yok) "timeout" degil acikalis; yoksa QML
        // JSON.parse edemeyip "unreadable result" gosterir, kullanici ne
        // oldugunu anlayamaz.
        if (!p.waitForStarted(3000))
            return QStringLiteral("cannot start ") + m_sweepBin;
        if (!p.waitForFinished(timeoutMs)) return QStringLiteral("timeout");
        const QString err = QString::fromUtf8(p.readAllStandardError()).trimmed();
        const QString out = QString::fromUtf8(p.readAllStandardOutput()).trimmed();
        if (out.isEmpty() && !err.isEmpty()) return err.split(QLatin1Char('\n')).last().trimmed();
        return out;
    }
    Q_INVOKABLE QString schedStatus() { return runSweepSync({QStringLiteral("schedule")}); }
    Q_INVOKABLE QString schedBackend() {
#ifdef Q_OS_WIN
        return QStringLiteral("Task Scheduler");
#elif defined(Q_OS_MACOS)
        return QStringLiteral("launchd");
#else
        return QStringLiteral("systemd");
#endif
    }
    Q_INVOKABLE QString schedEnable(int hour, const QString &selection) {
        QStringList args{QStringLiteral("schedule"), QStringLiteral("--enable"),
                         QStringLiteral("--hour"), QString::number(hour)};
        const QStringList parts = selection.split(QLatin1Char(' '), Qt::SkipEmptyParts);
        if (!parts.isEmpty()) args += parts;
        return runSweepSync(args);
    }
    Q_INVOKABLE QString schedDisable() {
        return runSweepSync({QStringLiteral("schedule"), QStringLiteral("--disable")});
    }
    // Bellek gorevi (guvenli memopt): ayni saat duzeni, ayri gorev.
    Q_INVOKABLE QString schedMemoptEnable(int hour) {
        return runSweepSync({QStringLiteral("schedule"), QStringLiteral("--enable"),
                             QStringLiteral("--memopt"), QStringLiteral("--hour"),
                             QString::number(hour)});
    }
    Q_INVOKABLE QString schedMemoptDisable() {
        return runSweepSync({QStringLiteral("schedule"), QStringLiteral("--disable"),
                             QStringLiteral("--memopt")});
    }
    Q_INVOKABLE QString schedMemoptStatus() {
        return runSweepSync({QStringLiteral("schedule"), QStringLiteral("--memopt")});
    }
    // Tepsi / pencere kontrolü (main()'de QQuickWindow'a bağlanır).
    void setWindow(QQuickWindow *w) { m_window = w; }
    Q_INVOKABLE void showWindow() {
        if (!m_window) return;
        m_window->show();
        m_window->raise();
        m_window->requestActivate();
#ifdef Q_OS_WIN
        const HWND hwnd = reinterpret_cast<HWND>(m_window->winId());
        if (hwnd) {
            ShowWindow(hwnd, IsIconic(hwnd) ? SW_RESTORE : SW_SHOW);
            SetForegroundWindow(hwnd);
        }
#endif
    }
    Q_INVOKABLE void hideWindow() { if (m_window) m_window->hide(); }
    Q_INVOKABLE void quitApp() { QCoreApplication::quit(); }
signals:
    void listReady(const QString &json);
    void previewReady(const QString &json);
    void cleanReady(const QString &json);
    void undoReady(const QString &json);
    void undoListReady(const QString &json);
    void historyReady(const QString &json);
    void bigfilesReady(const QString &json);
    void duReady(const QString &json);
    void dupesReady(const QString &json);
    void diagnosticsReady(const QString &json);
    void memoptReady(const QString &json);
    void schedElevatedReady(const QString &json);
    void startupElevatedReady(const QString &json);
    void progressLine(const QString &line);
    void failed(const QString &msg);
    void progressChanged();
    void elevatingChanged();
private:
    enum class Op { None, List, Preview, Clean, Undo, UndoList, History, Bigfiles, Du, Dupes, Diagnostics, Memopt, MemoptSudo };
    void startOp(Op op, const QStringList &args) {
        if (m_proc.state() != QProcess::NotRunning || m_elevating) {
            emit failed(QStringLiteral("busy"));
            return;
        }
        m_op = op;
        m_cancelled = false;
        m_buf.clear();
        m_tail.clear();
        setProgress(0.0);
        m_proc.start(m_sweepBin, args);
        if (m_proc.state() == QProcess::NotRunning)
            emit failed(QStringLiteral("cannot start sweep"));
    }
    void setProgress(double v) {
        m_progress = v;
        emit progressChanged();
    }
    void stamp(const QString &line) {
        const QString ts = QDateTime::currentDateTime().toString(QStringLiteral("hh:mm:ss"));
        emit progressLine((ts + QStringLiteral(" ") + line).trimmed());
    }
    void emitJsonForOp(const QString &op, const QString &json) {
        if (op == QLatin1String("preview")) emit previewReady(json);
        else if (op == QLatin1String("clean")) emit cleanReady(json);
        else if (op == QLatin1String("history")) emit historyReady(json);
        else if (op == QLatin1String("bigfiles")) emit bigfilesReady(json);
        else if (op == QLatin1String("dupes")) emit dupesReady(json);
        else if (op == QLatin1String("diagnostics")) emit diagnosticsReady(json);
        else if (op == QLatin1String("memopt")) emit memoptReady(json);
        else if (op == QLatin1String("schedule")) emit schedElevatedReady(json);
        else if (op == QLatin1String("startup")) emit startupElevatedReady(json);
        else if (op == QLatin1String("list")) emit listReady(json);
        else emit failed(QStringLiteral("unknown elevated op"));
    }
private slots:
    void onOutput() {
        const QByteArray chunk = m_proc.readAllStandardOutput();
        if (chunk.isEmpty()) return;
        m_buf += chunk;
        // `--json preview --all` is tens of megabytes and Rust's line-buffered
        // stdout delivers it in thousands of chunks. Decoding the whole
        // accumulated buffer on *every* chunk is O(n^2): the GUI thread burns
        // all its time in QString::fromUtf8, stops draining the pipe, and the
        // child then blocks on write — the "never finishes" stall. Decode only
        // the newly arrived chunk and keep just the unfinished last line.
        m_tail += QString::fromUtf8(chunk);
        const int end = m_tail.lastIndexOf('\n');
        if (end >= 0) {
            const int prev = m_tail.lastIndexOf('\n', end - 1);
            const QString line = m_tail.mid(prev + 1, end - prev - 1).trimmed();
            if (!line.isEmpty()) stamp(line);
            m_tail = m_tail.mid(end + 1);
        }
        // A progress line is short; bound the retained tail so a JSON line with
        // no terminator (or a very long path) cannot grow it without bound.
        if (m_tail.size() > 8192) m_tail = m_tail.right(8192);
    }
    void onStderr() {
        const QString err = QString::fromUtf8(m_proc.readAllStandardError()).trimmed();
        if (!err.isEmpty()) {
            const int nl = err.lastIndexOf('\n');
            stamp(nl >= 0 ? err.mid(nl + 1).trimmed() : err);
        }
    }
    void onFinished(int code, QProcess::ExitStatus status) {
        Q_UNUSED(code);
        setProgress(1.0);
        if (m_cancelled) {
            m_cancelled = false;
            emit failed(QStringLiteral("cancelled"));
            return;
        }
        // sweep exits non-zero when the report contains failures, but stdout
        // still carries a valid JSON report — prefer the report over the code.
        const QString out = QString::fromUtf8(m_buf).trimmed();
        if (status == QProcess::CrashExit || out.isEmpty()) {
            emit failed(QStringLiteral("sweep failed"));
            return;
        }
        switch (m_op) {
        case Op::List: emit listReady(out); break;
        case Op::Preview: emit previewReady(out); break;
        case Op::Clean: emit cleanReady(out); break;
        case Op::Undo: emit undoReady(out); break;
        case Op::UndoList: emit undoListReady(out); break;
        case Op::History: emit historyReady(out); break;
        case Op::Bigfiles: emit bigfilesReady(out); break;
        case Op::Du: emit duReady(out); break;
        case Op::Dupes: emit dupesReady(out); break;
        case Op::Diagnostics: emit diagnosticsReady(out); break;
        case Op::Memopt: emit memoptReady(out); break;
        case Op::MemoptSudo: emit memoptReady(out); break;
        default: break;
        }

        m_op = Op::None;
    }
    void pollElevated() {
#ifdef Q_OS_WIN
        if (!m_elevating) return;
        const DWORD rc = WaitForSingleObject(m_elevHandle, 0);
        if (rc != WAIT_OBJECT_0) return;
        CloseHandle(m_elevHandle);
        m_elevHandle = nullptr;
        m_elevTimer.stop();
        m_elevating = false;
        setProgress(1.0);
        emit elevatingChanged();
        QFile f(m_elevOut);
        const QString out = f.open(QIODevice::ReadOnly)
            ? QString::fromUtf8(f.readAll()).trimmed()
            : QString();
        QFile::remove(m_elevOut);
        if (out.isEmpty()) {
            emit failed(QStringLiteral("elevated sweep produced no output"));
            return;
        }
        emitJsonForOp(m_elevOp, out);
#endif
    }
private:
    QProcess m_proc;
    QByteArray m_buf;
    // Unfinished last line of stdout, decoded incrementally by onOutput.
    QString m_tail;
    QString m_sweepBin;
    Op m_op = Op::None;
    double m_progress = 0.0;
    bool m_cancelled = false;
    QQuickWindow *m_window = nullptr;
    QTimer m_elevTimer;
    bool m_elevating = false;
#ifdef Q_OS_WIN
    void *m_elevHandle = nullptr;
    QString m_elevOut, m_elevOp;
#endif
};

// Live system stats from /proc + statvfs, 1 Hz. Zeroes where unavailable.
class SysInfo : public QObject {
    Q_OBJECT
    Q_PROPERTY(double cpuPct READ cpuPct NOTIFY dataChanged)
    Q_PROPERTY(double memUsed READ memUsed NOTIFY dataChanged)
    Q_PROPERTY(double memTotal READ memTotal NOTIFY dataChanged)
    Q_PROPERTY(double swapUsed READ swapUsed NOTIFY dataChanged)
    Q_PROPERTY(double swapTotal READ swapTotal NOTIFY dataChanged)
    Q_PROPERTY(double diskUsed READ diskUsed NOTIFY dataChanged)
    Q_PROPERTY(double diskTotal READ diskTotal NOTIFY dataChanged)
    Q_PROPERTY(double ioRate READ ioRate NOTIFY dataChanged)
    Q_PROPERTY(int cpuCount READ cpuCount NOTIFY dataChanged)
    Q_PROPERTY(QVariantList cpuCores READ cpuCores NOTIFY dataChanged)
    Q_PROPERTY(bool winMica READ winMica NOTIFY micaChanged)
    Q_PROPERTY(QString osName READ osName CONSTANT)
    Q_PROPERTY(QString kernel READ kernel CONSTANT)
    Q_PROPERTY(QString cpuModel READ cpuModel CONSTANT)
    Q_PROPERTY(QString hostName READ hostName CONSTANT)
public:
    explicit SysInfo(QObject *parent = nullptr) : QObject(parent) {
#ifdef Q_OS_WIN
        m_osName = QSysInfo::prettyProductName();
        m_kernel = QSysInfo::kernelVersion();
        m_cpuModel = QString::fromLocal8Bit(std::getenv("PROCESSOR_IDENTIFIER") ? std::getenv("PROCESSOR_IDENTIFIER") : "");
        if (m_cpuModel.isEmpty()) m_cpuModel = QSysInfo::currentCpuArchitecture();
        m_hostName = QSysInfo::machineHostName();
        bool ok = false;
        m_cpuCount = QString::fromLocal8Bit(std::getenv("NUMBER_OF_PROCESSORS") ? std::getenv("NUMBER_OF_PROCESSORS") : "1").toInt(&ok);
        if (!ok || m_cpuCount < 1) m_cpuCount = 1;
#else
        m_osName = readOs(); m_kernel = readFirst("/proc/sys/kernel/osrelease");
        m_cpuModel = readCpu(); m_hostName = readFirst("/proc/sys/kernel/hostname");
#endif
        connect(&m_timer, &QTimer::timeout, this, &SysInfo::refresh);
        m_timer.setInterval(1000);
        refresh();
        m_timer.start();
    }
    double cpuPct() const { return m_cpu; }
    double memUsed() const { return m_memUsed; }
    double memTotal() const { return m_memTotal; }
    double swapUsed() const { return m_swapUsed; }
    double swapTotal() const { return m_swapTotal; }
    double ioRate() const { return m_io; }
    int cpuCount() const { return m_cpuCount; }
    QVariantList cpuCores() const { return m_cores; }
    bool winMica() const { return m_mica; }
    void setMica(bool on) { m_mica = on; emit micaChanged(); }
    QString osName() const { return m_osName; }
    QString kernel() const { return m_kernel; }
    QString cpuModel() const { return m_cpuModel; }
    QString hostName() const { return m_hostName; }
    static QString readFirst(const QString &path) {
        QFile f(path);
        if (f.open(QIODevice::ReadOnly)) return QString::fromUtf8(f.readAll()).trimmed();
        return QString();
    }
    static QString readOs() {
        QFile f(QStringLiteral("/etc/os-release"));
        if (f.open(QIODevice::ReadOnly)) {
            QTextStream in(&f);
            QString line;
            while (in.readLineInto(&line))
                if (line.startsWith(QStringLiteral("PRETTY_NAME=")))
                    return line.mid(12).trimmed().remove('"');
        }
        return QStringLiteral("Linux");
    }
    static QString readCpu() {
        QFile f(QStringLiteral("/proc/cpuinfo"));
        if (f.open(QIODevice::ReadOnly)) {
            QTextStream in(&f);
            QString line;
            while (in.readLineInto(&line))
                if (line.startsWith(QStringLiteral("model name")))
                    return line.section(':', 1).trimmed();
        }
        return QString();
    }
    double diskUsed() const { return m_diskUsed; }
    double diskTotal() const { return m_diskTotal; }
signals:
    void dataChanged();
    void micaChanged();
private slots:
    void refresh() {
#ifdef Q_OS_WIN
        refreshWindows();
#else
        QFile stat(QStringLiteral("/proc/stat"));
        if (stat.open(QIODevice::ReadOnly)) {
            QTextStream in(&stat);
            QString line;
            // genel toplam (`cpu ...`)
            if (in.readLineInto(&line)) {
                const QStringList f = line.split(' ', Qt::SkipEmptyParts);
                if (f.size() > 4) {
                    double idle = f[4].toDouble() + f[5].toDouble();
                    double total = 0;
                    for (int i = 1; i < f.size(); i++) total += f[i].toDouble();
                    if (total > m_prevTotal)
                        m_cpu = qBound(0.0, (1.0 - (idle - m_prevIdle) / (total - m_prevTotal)) * 100.0, 100.0);
                    m_prevIdle = idle;
                    m_prevTotal = total;
                }
            }
            // per-core: her `cpuN` satırı ayrı bir çekirdek
            QVariantList cores;
            int idx = 0;
            while (in.readLineInto(&line)) {
                if (!line.startsWith(QStringLiteral("cpu"))) break;
                if (line.startsWith(QStringLiteral("cpu "))) continue;
                const QStringList parts = line.split(' ', Qt::SkipEmptyParts);
                if (parts.size() < 5) continue;
                const quint64 idle = parts[4].toULongLong() + parts[5].toULongLong();
                quint64 total = 0;
                for (int i = 1; i < parts.size(); i++) total += parts[i].toULongLong();
                double pct = 0;
                if (idx < m_corePrevTotal.size() && m_corePrevTotal[idx] > 0 && total > m_corePrevTotal[idx])
                    pct = qBound(0.0, (1.0 - double(idle - m_corePrevIdle[idx]) / double(total - m_corePrevTotal[idx])) * 100.0, 100.0);
                while (m_corePrevIdle.size() <= idx) { m_corePrevIdle.append(0); m_corePrevTotal.append(0); }
                m_corePrevIdle[idx] = idle;
                m_corePrevTotal[idx] = total;
                cores.append(pct);
                idx++;
            }
            if (idx > 0) { m_cpuCount = idx; m_cores = cores; }
        }
        QFile mem(QStringLiteral("/proc/meminfo"));
        if (mem.open(QIODevice::ReadOnly)) {
            QTextStream in(&mem);
            double total = 0, avail = 0, swapTotal = 0, swapFree = 0;
            QString line;
            while (in.readLineInto(&line)) {
                if (line.startsWith(QStringLiteral("MemTotal:"))) total = line.split(' ', Qt::SkipEmptyParts)[1].toDouble() * 1024;
                else if (line.startsWith(QStringLiteral("MemAvailable:"))) avail = line.split(' ', Qt::SkipEmptyParts)[1].toDouble() * 1024;
                else if (line.startsWith(QStringLiteral("SwapTotal:"))) swapTotal = line.split(' ', Qt::SkipEmptyParts)[1].toDouble() * 1024;
                else if (line.startsWith(QStringLiteral("SwapFree:"))) swapFree = line.split(' ', Qt::SkipEmptyParts)[1].toDouble() * 1024;
            }
            if (total > 0) { m_memTotal = total; m_memUsed = total - avail; }
            m_swapTotal = swapTotal;
            m_swapUsed = qMax(0.0, swapTotal - swapFree);
        }
#ifndef Q_OS_WIN
        struct statvfs vfs;
        if (statvfs("/", &vfs) == 0 && vfs.f_blocks > 0) {
            m_diskTotal = double(vfs.f_blocks) * double(vfs.f_frsize);
            m_diskUsed = m_diskTotal - double(vfs.f_bavail) * double(vfs.f_frsize);
        }
#endif
        QFile disk(QStringLiteral("/proc/diskstats"));
        if (disk.open(QIODevice::ReadOnly)) {
            QTextStream in(&disk);
            double sectors = 0;
            QString line;
            while (in.readLineInto(&line)) {
                const QStringList f = line.split(' ', Qt::SkipEmptyParts);
                if (f.size() > 13) sectors += f[5].toDouble() + f[9].toDouble();
            }
            if (m_prevIo > 0) m_io = (sectors - m_prevIo) * 512.0;
            m_prevIo = sectors;
        }
#endif
        emit dataChanged();
    }
#ifdef Q_OS_WIN
    static quint64 filetimeToU64(const FILETIME &ft) {
        return (quint64(ft.dwHighDateTime) << 32) | quint64(ft.dwLowDateTime);
    }
    // Windows canlı verisi: hepsi kernel32, ek bağlantı gerekmez.
    void refreshWindows() {
        FILETIME idle, kernel, user;
        if (GetSystemTimes(&idle, &kernel, &user)) {
            const quint64 idleT = filetimeToU64(idle);
            const quint64 totalT = filetimeToU64(kernel) + filetimeToU64(user);
            if (totalT > m_prevTotalT)
                m_cpu = qBound(0.0, (1.0 - double(idleT - m_prevIdleT) / double(totalT - m_prevTotalT)) * 100.0, 100.0);
            m_prevIdleT = idleT;
            m_prevTotalT = totalT;
        }
        MEMORYSTATUSEX mem = {};
        mem.dwLength = sizeof(mem);
        if (GlobalMemoryStatusEx(&mem) && mem.ullTotalPhys > 0) {
            m_memTotal = double(mem.ullTotalPhys);
            m_memUsed = double(mem.ullTotalPhys - mem.ullAvailPhys);
            // Sayfa dosyası (pagefile) = Windows takas karsiligi; yoksa 0
            // kalir ve QML "--" gosterir.
            m_swapTotal = double(mem.ullTotalPageFile);
            m_swapUsed = double(mem.ullTotalPageFile - mem.ullAvailPageFile);
        }
        ULARGE_INTEGER avail, total, free;
        if (GetDiskFreeSpaceExW(L"C:\\", &avail, &total, &free) && total.QuadPart > 0) {
            m_diskTotal = double(total.QuadPart);
            m_diskUsed = double(total.QuadPart - free.QuadPart);
        }
        // Windows: çekirdek düzeyi raporlama global GetSystemTimes ile sınırlı;
        // dürüst olalım — liste boş kalır, QML tek global bar gösterir.
        m_cores.clear();
        m_io = 0;
        emit dataChanged();
    }
#endif
private:
    QTimer m_timer;
    QString m_osName, m_kernel, m_cpuModel, m_hostName;
    double m_cpu = 0, m_memUsed = 0, m_memTotal = 1;
    double m_swapUsed = 0, m_swapTotal = 0;
    double m_diskUsed = 0, m_diskTotal = 1, m_io = 0;
    double m_prevIdle = 0, m_prevTotal = 0, m_prevIo = 0;
    int m_cpuCount = 1;
    QVariantList m_cores;
    QVector<quint64> m_corePrevIdle, m_corePrevTotal;
    bool m_mica = false;
#ifdef Q_OS_WIN
    quint64 m_prevIdleT = 0, m_prevTotalT = 0;
#endif
};

#include "main.moc"

class SweepNativeFilter : public QAbstractNativeEventFilter {
public:
    explicit SweepNativeFilter(SweepBridge *bridge) : m_bridge(bridge) {}
    bool nativeEventFilter(const QByteArray &, void *message, qintptr *) override {
#ifdef Q_OS_WIN
        const auto *msg = static_cast<MSG *>(message);
        if (msg && msg->message == sweepShowMessage()) {
            m_bridge->showWindow();
            return true;
        }
#else
        Q_UNUSED(message);
#endif
        return false;
    }
private:
    SweepBridge *m_bridge;
};

int main(int argc, char **argv) {
    // --- Grafik API: donanim varsa GPU, yoksa yazilim -----------------------
    // Qt 6.8'in win64_mingw derlemesinde ANGLE yok (libEGL/libGLESv2 yok),
    // yani Qt Quick dogrudan makinenin OpenGL surucusune bagimli. Surucu yok
    // ya da bozuksa Qt Quick bir GL baglami kuramayip qFatal firlatiyor ve
    // surec ntdll icinde ACCESS_VIOLATION ile oluyor. Once yokluyoruz:
    // baglam kurulamiyorsa yazilim sahnesine dusuyoruz (kirilganlik yerine
    // biraz daha yavas ama calisan bir arayuz).
    //   SWEEP_SOFTWARE=1 -> yazilima zorla
    //   SWEEP_GPU=1      -> GPU'ya zorla (yoklamayi atla)
    const bool forceSoftware =
        !QString::fromLocal8Bit(qgetenv("SWEEP_SOFTWARE")).isEmpty();
    const bool forceGpu = !QString::fromLocal8Bit(qgetenv("SWEEP_GPU")).isEmpty();

    // Varsayilan YAZILIM. Bir yoklama (QOpenGLContext) baglamin kuruldugunu
    // soyledigi halde sahne grafigi sonra cokertiyor olabilir; asimetri acik:
    // yanlis negatif = uygulama hic acilmiyor, yanlis pozitif = biraz yavas
    // ama calisan arayuz. Bu yuzden donanim hizlandirma ACIKCA istenmeli.
    bool useSoftware = forceSoftware || !forceGpu;
    if (!useSoftware) {
        // SWEEP_GPU=1: yine de yokla; baglam kurulamazsa yazilima don.
        QOpenGLContext probe;
        useSoftware = !(probe.create() && probe.isValid());
    }
    if (useSoftware) {
        qputenv("QSG_RENDER_BACKEND", "software");
        QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);
    }

    // QApplication, not QGuiApplication: the tray menu is a QMenu (a
    // QWidget) and constructing one under a QGuiApplication aborts the
    // process with "QWidget: Cannot create a QWidget without QApplication"
    // exactly when a real system tray exists. Qt6::Widgets is linked and
    // deployed already, so this is the configuration the app was built for.
    QApplication app(argc, argv);
    app.setApplicationName(QStringLiteral("Sweep"));
    // Kapatınca tepsiye inen GUI ikinci açılışta yeni süreç başlatmasın.
    // SWEEP_GRAB / SWEEP_SCREEN test kancaları kilidi atlar.
    QLockFile singleton(QDir::temp().absoluteFilePath(QStringLiteral("org.sweep.Sweep.lock")));
    if (qgetenv("SWEEP_GRAB").isEmpty() && qgetenv("SWEEP_SCREEN").isEmpty()
        && !singleton.tryLock(100)) {
#ifdef Q_OS_WIN
        wakeExistingSweep();
#endif
        return 0;
    }
    {
        // Global default font with emoji fallback so glyphs like "🧠⚡💾"
        // render on hosts without Qt's bundled emoji font.
        QFont f = app.font();
        f.setFamily(QStringLiteral("Noto Sans,Noto Color Emoji,Helvetica,Arial,sans-serif"));
        app.setFont(f);
    }
    QQmlApplicationEngine engine;
    SweepBridge bridge;
    QObject::connect(&app, &QCoreApplication::aboutToQuit, [&bridge]() {
        bridge.stopBackgroundWork();
    });
    SweepNativeFilter nativeFilter(&bridge);
#ifdef Q_OS_WIN
    app.installNativeEventFilter(&nativeFilter);
#endif
    SysInfo sys;
    AppSettings settings;
    Translator i18n;
    i18n.setLang(settings.language());

    engine.rootContext()->setContextProperty(QStringLiteral("sweep"), &bridge);
    engine.rootContext()->setContextProperty(QStringLiteral("sys"), &sys);
    engine.rootContext()->setContextProperty(QStringLiteral("settings"), &settings);
    engine.rootContext()->setContextProperty(QStringLiteral("i18n"), &i18n);
    // ponytail: dist/AppDir (usr/bin->usr/qml) + dev + konteyner sırası
    const QString here = QCoreApplication::applicationDirPath();
    QString envDir = QString::fromLocal8Bit(std::getenv("SWEEP_QML_DIR") ? std::getenv("SWEEP_QML_DIR") : "");
    QStringList cand;
    if (!envDir.isEmpty()) cand << envDir + QStringLiteral("/Main.qml");
    cand << here + QStringLiteral("/../qml/Main.qml")
         << here + QStringLiteral("/qml/Main.qml")
         << QStringLiteral("/usr/qml/Main.qml")
         << QStringLiteral("/usr/share/sweep-qml/qml/Main.qml")
         << QStringLiteral("/app/qml/Main.qml")
         << QDir::current().filePath(QStringLiteral("packaging/qml/qml/Main.qml"))
         << QDir::current().filePath(QStringLiteral("qml/Main.qml"));
    QString qml;
    for (const QString &c : cand)
        if (QFileInfo(c).isReadable()) { qml = c; break; }
    if (qml.isEmpty()) return 1;
    const QDir qmlDir = QFileInfo(qml).dir();
    engine.addImportPath(qmlDir.absolutePath());
    // Window/taskbar icon: resolved next to the QML sources, so the shell
    // picks it up without needing a platform-specific resource step.
    const QString iconPath = qmlDir.filePath(QStringLiteral("assets/sweep-logo.png"));
    if (QFileInfo::exists(iconPath))
        app.setWindowIcon(QIcon(iconPath));
    engine.load(QUrl::fromLocalFile(QFileInfo(qml).absoluteFilePath()));
    if (engine.rootObjects().isEmpty())
        return 1;

    // Smoke-test kancasi: SWEEP_SCREEN=StartupMgr gibi bir deger verilirse
    // acilis ekrani o olur. Boylece displaysiz (offscreen) bir ortamda her rota
    // tek tek yuklenip QML hatalari yakalanabilir - bkz. packaging/qml/
    // smoke-wsl.sh. Degisken bos veya yoksa davranis hic degismez; `show()`
    // bulunamazsa da yalnizca bir uyari yazilir, uygulama normal acilir.
    {
        const QString forced = QString::fromUtf8(qgetenv("SWEEP_SCREEN"));
        if (!forced.isEmpty()) {
            QObject *rootObj = engine.rootObjects().first();
            QVariant discarded;
            const bool ok = QMetaObject::invokeMethod(
                rootObj, "show",
                Q_RETURN_ARG(QVariant, discarded),
                Q_ARG(QVariant, forced));
            if (!ok) {
                QTextStream err(stderr);
                err << "sweep-qml: SWEEP_SCREEN='" << forced
                    << "' could not be applied (root object has no show())"
                    << Qt::endl;
            }
        }
    }

    // Pencere hazır olduktan sonra: köprüye bağla, Windows'a özel DWM
    // efektlerini (koyu başlık + Win11 Mica) uygula, tepsi simgesini kur.
    QTimer::singleShot(0, [&engine, &bridge, &sys, &settings]() {
        if (engine.rootObjects().isEmpty()) return;
        auto *win = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
        if (!win) return;
        bridge.setWindow(win);
#ifdef Q_OS_WIN
        const HWND hwnd = reinterpret_cast<HWND>(win->winId());
        if (hwnd)
            SetPropW(hwnd, kSweepProp, HANDLE(1));
        const QOperatingSystemVersion v = QOperatingSystemVersion::current();
        const QOperatingSystemVersion win11(QOperatingSystemVersion::Windows, 10, 0, 22000);
        const QOperatingSystemVersion win11_22h2(QOperatingSystemVersion::Windows, 10, 0, 22621);
        bool mica = false;
        // hwnd boşsa (pencere henüz oluşmadı / başarısız) DWM'ye hiç
        // dokunmuyoruz: 0 tutamacıyla çağrı en iyi durumda sessizce başarısız
        // olur, en kötü durumda çökme kaynağı olur.
        // SWEEP_NO_MICA=1: koyu başlık + Mica efektlerini tamamen atla
        // (Win10'da desteklenmeyen DWM öznitelikleri çökme kaynağı olabilir).
        if (hwnd && QString::fromLocal8Bit(qgetenv("SWEEP_NO_MICA")).isEmpty()) {
            // Koyu başlık çubuğu (DWMWA_USE_IMMERSIVE_DARK_MODE = 20)
            BOOL dark = TRUE;
            DwmSetWindowAttribute(hwnd, 20, &dark, sizeof(dark));
            if (v >= win11_22h2) {
                // DWMWA_SYSTEMBACKDROP_TYPE = 38, DWMSBT_MAINWINDOW = 2 (Mica)
                const int backdrop = 2;
                mica = DwmSetWindowAttribute(hwnd, 38, &backdrop, sizeof(backdrop)) == S_OK;
            } else if (v >= win11) {
                // Eski Win11 yapıları: DWMWA_MICA_EFFECT = 1029
                BOOL effect = TRUE;
                mica = DwmSetWindowAttribute(hwnd, 1029, &effect, sizeof(effect)) == S_OK;
            }
        }
        if (mica) {
            // Mica'nın görünmesi için pencere arka planı saydam olmalı;
            // QML zaten kendi arka planını çizer.
            win->setColor(QColor(Qt::transparent));
            sys.setMica(true);
        }
#endif
        // SWEEP_NO_TRAY=1: tepsisiz baslat. Tepsi, displaysiz bir smoke
        // testinde hic kurulmadigi icin (isSystemTrayAvailable false) gercek
        // ekrandaki cokmeleri ayirt etmek icin kapatilabilir olmali.
        if (QSystemTrayIcon::isSystemTrayAvailable()
            && QString::fromLocal8Bit(qgetenv("SWEEP_NO_TRAY")).isEmpty()) {
            auto *tray = new Tray(win->icon(), &bridge);
            QObject::connect(tray, &Tray::showRequested, &bridge, &SweepBridge::showWindow);
            QObject::connect(tray, &Tray::scanRequested, &bridge, [&bridge]() {
                bridge.showWindow();
                bridge.preview(QStringList());
            });
            QObject::connect(tray, &Tray::settingsRequested, &bridge, [&bridge]() {
                bridge.showWindow();
            });
            QObject::connect(tray, &Tray::quitRequested, &bridge, [&settings]() {
                settings.setQuitting(true);
                QCoreApplication::quit();
            });
            QObject::connect(&bridge, &SweepBridge::cleanReady, tray, [tray](const QString &) {
                tray->notify(QStringLiteral("Sweep"),
                             QStringLiteral("Temizlik tamamlandı — alan geri kazanıldı."));
            });
            QObject::connect(&bridge, &SweepBridge::memoptReady, tray, [tray](const QString &) {
                tray->notify(QStringLiteral("Sweep"),
                             QStringLiteral("Bellek optimizasyonu tamamlandı."));
            });
        }
    });

    // Diagnostic hook. With SWEEP_GRAB=<file.png> the window renders its own
    // scene to that PNG after it is up, then the app exits (0 = written).
    // Screen capture tools cannot do this reliably (occlusion, locked session,
    // RDP), so this is the deterministic proof that the GUI really paints.
    // See packaging/windows/README.md ("GUI acilmiyor").
    {
        const QString grabPath = QString::fromUtf8(qgetenv("SWEEP_GRAB"));
        if (!grabPath.isEmpty()) {
            QTimer::singleShot(3500, [&engine, grabPath]() {
                QTextStream out(stdout);
                if (engine.rootObjects().isEmpty()) {
                    out << "sweep-qml: SWEEP_GRAB failed - no root object" << Qt::endl;
                    QCoreApplication::exit(3);
                    return;
                }
                auto *w = qobject_cast<QQuickWindow *>(engine.rootObjects().first());
                if (!w) {
                    out << "sweep-qml: SWEEP_GRAB failed - root is not a window" << Qt::endl;
                    QCoreApplication::exit(3);
                    return;
                }
                const QImage img = w->grabWindow();
                const bool ok = !img.isNull() && img.save(grabPath);
                out << "sweep-qml: SWEEP_GRAB " << (ok ? "wrote " : "FAILED to write ")
                    << grabPath << " (" << img.width() << "x" << img.height() << ")"
                    << Qt::endl;
                QCoreApplication::exit(ok ? 0 : 4);
            });
        }
    }

    return app.exec();
}