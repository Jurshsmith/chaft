import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

// First-run hero for the no-workspace state, plus the read-only raw-store
// variant. All colors flow from Tokens so the surface follows every theme;
// actions reuse the existing workspace entry flow on the App root.
Item {
    id: root
    property var app

    readonly property bool rawStoreMode: chaftController.rawEventStoreMode
    // Idle prompts from the controller restate what the cards already say;
    // only surface statuses that carry real signal (errors, lock state).
    readonly property var idleStatuses: [
        "create a workspace first", "local event log", "demo workspace",
        "loading local runtime...", "loading event store..."
    ]
    readonly property string meaningfulStatus: {
        var status = String(chaftController.syncStatus || "").trim()
        if (status.length === 0 || root.idleStatuses.indexOf(status.toLowerCase()) !== -1) {
            return ""
        }
        return status
    }
    readonly property var curatedThemeIds: [
        "midnight-relay", "synthwave-84", "terminal-phosphor",
        "paper-atelier", "sakura-morning"
    ]
    readonly property var onboardingSteps: [
        {
            step: "1",
            title: "Create",
            caption: "A signed, encrypted workspace lives on this device"
        },
        {
            step: "2",
            title: "Host or back up",
            caption: "Serve it to peers or keep encrypted replicas"
        },
        {
            step: "3",
            title: "Invite devices",
            caption: "Grant access with signed invites and shared keys"
        }
    ]

    PeerMeshBackdrop {
        anchors.fill: parent
    }

    ColumnLayout {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.verticalCenter: parent.verticalCenter
        anchors.verticalCenterOffset: -Math.round(parent.height * 0.04)
        width: Math.min(720, Math.max(300, parent.width - 96))
        spacing: Tokens.space4

        Text {
            Layout.fillWidth: true
            text: root.rawStoreMode
                ? "Viewing a raw event store"
                : "Your workspace,\non your machine."
            color: Tokens.textStrong
            font.pixelSize: Tokens.fontSizeDisplay
            font.weight: Font.Bold
            lineHeight: 1.05
            wrapMode: Text.WordWrap
        }

        Text {
            Layout.fillWidth: true
            Layout.maximumWidth: 560
            text: root.rawStoreMode
                ? "Read-only inspection of a local events.db without runtime keys. "
                    + "Point CHAFT_RUNTIME_DIR at a runtime directory to unlock the full shell."
                : "Chaft syncs signed, end-to-end-encrypted history directly between "
                    + "your devices and peers. No server, no account."
            color: Tokens.textMuted
            font.pixelSize: Tokens.fontSizeMd
            lineHeight: 1.25
            wrapMode: Text.WordWrap
        }

        GridLayout {
            Layout.fillWidth: true
            Layout.topMargin: Tokens.space3
            visible: !root.rawStoreMode
            columns: root.width > 640 ? 2 : 1
            columnSpacing: Tokens.space3
            rowSpacing: Tokens.space3

            OnboardingActionCard {
                Layout.fillWidth: true
                primary: true
                glyph: "#"
                title: "Create a workspace"
                body: "Start fresh on this device. Invite your other devices and teammates any time."
                actionable: root.app.runtimeAccessReady
                onActivated: root.app.openWorkspaceEntry("create")
            }

            OnboardingActionCard {
                Layout.fillWidth: true
                glyph: "⇄"
                title: "Join a workspace"
                body: "Have an invite? Pull history from a peer and unlock it with your workspace key."
                actionable: root.app.runtimeAccessReady
                onActivated: root.app.openWorkspaceEntry("join")
            }
        }

        Text {
            visible: !root.rawStoreMode
            text: "or peek at a demo workspace"
            color: Tokens.accent
            font.pixelSize: Tokens.fontSizeSm
            font.underline: demoLinkMouse.containsMouse
            activeFocusOnTab: true

            Accessible.role: Accessible.Button
            Accessible.name: "Peek at a demo workspace"
            Accessible.description: "Open a read-only sample workspace to explore the interface"
            Accessible.onPressAction: root.app.startDemoTour()

            Rectangle {
                anchors.fill: parent
                anchors.margins: -2
                radius: Tokens.radiusXs
                color: "transparent"
                border.color: Tokens.accent
                border.width: parent.activeFocus ? 2 : 0
            }

            MouseArea {
                id: demoLinkMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.app.startDemoTour()
            }

            Keys.onPressed: function (event) {
                if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
                    root.app.startDemoTour();
                    event.accepted = true;
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            Layout.topMargin: Tokens.space3
            visible: !root.rawStoreMode
            height: 1
            color: Tokens.borderSubtle
            opacity: 0.6
        }

        GridLayout {
            Layout.fillWidth: true
            visible: !root.rawStoreMode
            columns: root.width > 640 ? 3 : 1
            columnSpacing: Tokens.space4
            rowSpacing: Tokens.space2

            Repeater {
                model: root.onboardingSteps

                delegate: RowLayout {
                    id: stepDelegate
                    required property var modelData

                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    Text {
                        Layout.alignment: Qt.AlignTop
                        text: stepDelegate.modelData.step
                        color: Tokens.accent
                        font.family: Tokens.fontMono
                        font.pixelSize: Tokens.fontSizeMd
                        font.weight: Font.Bold
                    }

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 1

                        Text {
                            Layout.fillWidth: true
                            text: stepDelegate.modelData.title
                            color: Tokens.textStrong
                            font.pixelSize: Tokens.fontSizeSm
                            font.weight: Font.Medium
                        }

                        Text {
                            Layout.fillWidth: true
                            text: stepDelegate.modelData.caption
                            color: Tokens.textMuted
                            font.pixelSize: Tokens.fontSizeXs
                            wrapMode: Text.WordWrap
                        }
                    }
                }
            }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.topMargin: Tokens.space3
            visible: !root.rawStoreMode
            spacing: Tokens.space2

            Text {
                text: "Make it yours — 24 themes live in Setup → Appearance"
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeXs
                font.weight: Font.DemiBold
            }

            Flow {
                Layout.fillWidth: true
                spacing: Tokens.space2

                Repeater {
                    model: root.curatedThemeIds

                    delegate: ThemeSwatch {
                        id: curatedSwatchDelegate
                        required property var modelData

                        themeData: Themes.themeById(curatedSwatchDelegate.modelData)
                        active: curatedSwatchDelegate.modelData === Tokens.activeThemeId
                        onChosen: function (themeId) {
                            root.app.applyThemeChoice(themeId)
                        }
                    }
                }
            }
        }

        Text {
            Layout.fillWidth: true
            visible: root.meaningfulStatus.length > 0
            text: root.meaningfulStatus
            color: Tokens.textMuted
            font.family: Tokens.fontMono
            font.pixelSize: Tokens.fontSizeXs
            elide: Text.ElideRight
        }
    }
}
