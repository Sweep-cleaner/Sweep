cmake_minimum_required(VERSION 3.21)
project(sweep-qml VERSION 0.4.0 LANGUAGES CXX)
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
find_package(Qt6 REQUIRED COMPONENTS Core Gui Quick Qml Widgets)
qt_standard_project_setup()
qt_add_executable(sweep-qml main.cpp)

# GUI: no console window (/SUBSYSTEM:WINDOWS). Deliberately NOT
# WIN32_EXECUTABLE: that makes Qt link Qt6EntryPoint, whose win64_mingw build
# needs an older CRT than MinGW-w64 16 (UCRT) provides, so the link fails with
#   libQt6EntryPoint.a(qtentrypoint_win.cpp.obj): undefined reference to
#   `__imp___argc'
# Setting the subsystem directly avoids Qt6EntryPoint entirely. MinGW - unlike
# MSVC - needs no WinMain shim, so plain main() still works. Verified: the
# resulting PE reports subsystem 2 (GUI) and the GUI loads headless.
option(SWEEP_WIN_GUI_SUBSYSTEM "Link the GUI against the Windows subsystem (no console window)" ON)
if (SWEEP_WIN_GUI_SUBSYSTEM)
    # NOT WIN32_EXECUTABLE: that pulls in Qt6EntryPoint, which does not link
    # with MinGW-w64 16 (UCRT). Setting the subsystem directly keeps main() and
    # links cleanly (MinGW needs no WinMain shim, unlike MSVC).
    target_link_options(sweep-qml PRIVATE "-Wl,--subsystem,windows")
endif()
qt_add_qml_module(sweep-qml
  URI Sweep
  VERSION 1.0
  QML_FILES
    qml/Main.qml
    qml/Sidebar.qml
    qml/Dashboard.qml
    qml/CleanerScreen.qml
    qml/FilesScreen.qml
    qml/DiskScreen.qml
    qml/OptimizeScreen.qml
    qml/MemoryScreen.qml
    qml/StartupScreen.qml
    qml/AutostartScreen.qml
    qml/SysInfoScreen.qml
    qml/NetworkScreen.qml
    qml/PrivacyScreen.qml
    qml/ScheduleScreen.qml
    qml/SystemScreen.qml
    qml/HistoryScreen.qml
    qml/Settings.qml
    qml/ThemeManager.qml
    qml/ChartComponents.qml
    qml/NeonButton.qml
    qml/NeonCard.qml
    qml/NeonBadge.qml
    qml/NeonCheckBox.qml
    qml/NeonSlider.qml
    qml/GlassCard.qml
    qml/NavItem.qml
    qml/AuroraBackground.qml
    qml/ToastHost.qml
    qml/ProgressRing.qml
    qml/WelcomeScreen.qml
    qml/Celebrate.qml
    qml/I18nText.qml
    qml/themes/MidnightBlue.qml
    qml/themes/GruvboxDark.qml
    qml/themes/CatppuccinMocha.qml
    qml/themes/Nord.qml
    qml/themes/Dracula.qml
    qml/themes/TokyoNight.qml
    qml/themes/OneDark.qml
    qml/themes/SolarizedDark.qml
    qml/themes/MonokaiPro.qml
    qml/themes/Everforest.qml
    qml/themes/RosePine.qml
  RESOURCES
    qml/assets/sweep-logo.png
)
target_link_libraries(sweep-qml PRIVATE Qt6::Core Qt6::Gui Qt6::Quick Qt6::Qml Qt6::Widgets)
if(WIN32)
  # DwmSetWindowAttribute (koyu başlık + Mica) için dwmapi bağlanır;
  # MSVC'de #pragma comment(lib) de bunu halleder, MinGW için buraya gerekir.
  target_link_libraries(sweep-qml PRIVATE dwmapi)
endif()
install(TARGETS sweep-qml RUNTIME DESTINATION bin)
install(DIRECTORY qml DESTINATION share/sweep-qml)
