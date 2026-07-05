pragma Singleton
import QtQuick
import Chaft

QtObject {
    // App.qml binds this to the persisted controller theme selection.
    // Unknown or blank IDs resolve to the catalog default.
    property string activeThemeId: Themes.defaultThemeId
    readonly property var activeTheme: Themes.themeById(activeThemeId)

    readonly property color rail: activeTheme.rail
    readonly property color railElevated: activeTheme.railElevated
    readonly property color railText: activeTheme.railText
    readonly property color sidebar: activeTheme.sidebar
    readonly property color sidebarInput: activeTheme.sidebarInput
    readonly property color sidebarActive: activeTheme.sidebarActive
    readonly property color sidebarText: activeTheme.sidebarText
    readonly property color sidebarTextStrong: activeTheme.sidebarTextStrong
    readonly property color sidebarTextSoft: activeTheme.sidebarTextSoft
    readonly property color sidebarTextMuted: activeTheme.sidebarTextMuted
    readonly property color surfaceBase: activeTheme.surfaceBase
    readonly property color surfaceRaised: activeTheme.surfaceRaised
    readonly property color borderSubtle: activeTheme.borderSubtle
    readonly property color textStrong: activeTheme.textStrong
    readonly property color textMuted: activeTheme.textMuted
    readonly property color accent: activeTheme.accent
    readonly property color onAccent: activeTheme.onAccent
    readonly property color success: activeTheme.success
    readonly property color secure: activeTheme.secure
    readonly property color secureSurface: activeTheme.secureSurface
    readonly property color warning: activeTheme.warning
    readonly property color warningSurface: activeTheme.warningSurface
    readonly property color warningText: activeTheme.warningText
    readonly property string fontUi: "Space Grotesk"
    readonly property string fontMono: "JetBrains Mono"
    readonly property int fontSizeXs: 11
    readonly property int fontSizeSm: 12
    readonly property int fontSizeMd: 14
    readonly property int fontSizeLg: 15
    readonly property int fontSizeXl: 18
    readonly property int radiusXs: 3
    readonly property int radiusSm: 6
    readonly property int radiusMd: 8
    readonly property int space1: 4
    readonly property int space2: 8
    readonly property int space3: 12
    readonly property int space4: 16
    property bool motionEnabled: true
    readonly property int motionQuickMs: 140
}
