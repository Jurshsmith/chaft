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
        "create a workspace first",
        "local history ready",
        "local history preview",
        "demo workspace",
        "loading workspace data...",
        "loading local history..."
    ]
    readonly property string meaningfulStatus: {
        var status = String(chaftController.syncStatus || "").trim();
        if (status.length === 0 || root.idleStatuses.indexOf(status.toLowerCase()) !== -1) {
            return "";
        }
        return status;
    }
    readonly property var pendingAccessRequests: root.app ? root.app.pendingAccessRequests : []
    readonly property var curatedThemeIds: ["midnight-relay", "synthwave-84", "terminal-phosphor", "paper-atelier", "sakura-morning"]
    readonly property var onboardingSteps: [
        {
            step: "1",
            title: "Create",
            caption: "Start a private workspace for your team"
        },
        {
            step: "2",
            title: "Invite people",
            caption: "Share one invite with each teammate"
        },
        {
            step: "3",
            title: "Stay reachable",
            caption: "Keep Chaft open while teammates catch up"
        }
    ]

    PeerMeshBackdrop {
        anchors.fill: parent
    }

    Flickable {
        id: contentFlick
        anchors.fill: parent
        boundsBehavior: Flickable.StopAtBounds
        clip: true
        contentWidth: width
        contentHeight: Math.max(height, onboardingContent.y + onboardingContent.implicitHeight + Tokens.space3 * 2)

        ScrollBar.vertical: ScrollBar {
            policy: contentFlick.contentHeight > contentFlick.height ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff
        }

        ColumnLayout {
            id: onboardingContent
            x: Math.round((contentFlick.width - width) / 2)
            y: Math.max(Tokens.space3 * 2, Math.round((contentFlick.height - implicitHeight) / 2 - contentFlick.height * 0.04))
            width: Math.min(720, Math.max(300, contentFlick.width - 96))
            spacing: Tokens.space4

            Text {
                Layout.fillWidth: true
                text: root.rawStoreMode
                    ? "Viewing read-only history"
                    : "Create, join, or restore\na workspace."
                color: Tokens.textStrong
                font.pixelSize: Tokens.fontSizeDisplay
                font.weight: Font.Bold
                lineHeight: 1.05
                wrapMode: Text.WordWrap
            }

            Text {
                Layout.fillWidth: true
                Layout.maximumWidth: 560
                text: root.rawStoreMode ? "Read-only history view. " + "Choose a workspace folder to unlock the full app." : "Chaft keeps conversations private and updates when teammates are reachable. No server account required."
                color: Tokens.textMuted
                font.pixelSize: Tokens.fontSizeMd
                lineHeight: 1.25
                wrapMode: Text.WordWrap
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.topMargin: Tokens.space3
                visible: !root.rawStoreMode && root.pendingAccessRequests.length > 0
                spacing: Tokens.space2

                Text {
                    Layout.fillWidth: true
                    text: "Pending requests"
                    color: Tokens.textStrong
                    font.pixelSize: Tokens.fontSizeXs
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                }

                Repeater {
                    model: root.pendingAccessRequests

                    delegate: Rectangle {
                        id: pendingRequestCard
                        required property var modelData

                        Layout.fillWidth: true
                        implicitHeight: pendingRequestContent.implicitHeight + Tokens.space3 * 2
                        radius: Tokens.radiusSm
                        color: Tokens.surfaceRaised
                        border.width: 1
                        border.color: Tokens.borderSubtle

                        ColumnLayout {
                            id: pendingRequestContent
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.margins: Tokens.space3
                            spacing: Tokens.space2

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Tokens.space2

                                Rectangle {
                                    Layout.preferredWidth: 66
                                    Layout.preferredHeight: 26
                                    radius: Tokens.radiusSm
                                    color: Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.18)
                                    border.width: 1
                                    border.color: Qt.rgba(Tokens.accent.r, Tokens.accent.g, Tokens.accent.b, 0.5)

                                    Text {
                                        anchors.centerIn: parent
                                        text: pendingRequestCard.modelData.statusBadgeLabel
                                        color: Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeXs
                                        font.weight: Font.DemiBold
                                    }
                                }

                                ColumnLayout {
                                    Layout.fillWidth: true
                                    spacing: 1

                                    Text {
                                        Layout.fillWidth: true
                                        text: pendingRequestCard.modelData.statusTitle
                                        color: Tokens.textStrong
                                        font.pixelSize: Tokens.fontSizeSm
                                        font.weight: Font.DemiBold
                                        elide: Text.ElideRight
                                    }

                                    Text {
                                        Layout.fillWidth: true
                                        text: pendingRequestCard.modelData.statusMessage
                                        color: Tokens.textMuted
                                        font.pixelSize: Tokens.fontSizeXs
                                        wrapMode: Text.WordWrap
                                    }
                                }
                            }

                            GridLayout {
                                Layout.fillWidth: true
                                columns: root.width > 520 ? 2 : 1
                                columnSpacing: Tokens.space3
                                rowSpacing: Tokens.space1

                                Text {
                                    Layout.fillWidth: true
                                    text: "Workspace: " + pendingRequestCard.modelData.workspaceLabel
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    text: "Name: " + pendingRequestCard.modelData.displayLabel
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    visible: pendingRequestCard.modelData.deliveryLabel !== "an owner or admin"
                                    text: "Admin: " + pendingRequestCard.modelData.deliveryLabel
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    visible: String(pendingRequestCard.modelData.sourceLabel || "").length > 0
                                    text: "Started with: " + pendingRequestCard.modelData.sourceLabel
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    visible: String(pendingRequestCard.modelData.receiptLabel || "").length > 0
                                    text: pendingRequestCard.modelData.receiptLabel
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    elide: Text.ElideRight
                                }

                                Text {
                                    Layout.fillWidth: true
                                    visible: String(pendingRequestCard.modelData.error || "").length > 0
                                    text: "Reason: " + String(pendingRequestCard.modelData.error || "")
                                    color: Tokens.textMuted
                                    font.pixelSize: Tokens.fontSizeXs
                                    elide: Text.ElideRight
                                }
                            }

                            GridLayout {
                                Layout.fillWidth: true
                                columns: root.width > 720 ? 6 : (root.width > 460 ? 3 : 2)
                                columnSpacing: Tokens.space2
                                rowSpacing: Tokens.space2

                                Button {
                                    Layout.fillWidth: true
                                    visible: pendingRequestCard.modelData.canSendDirect
                                    text: pendingRequestCard.modelData.status === "sending" ? "Sending..." : (pendingRequestCard.modelData.status === "sent" ? "Resend" : (pendingRequestCard.modelData.status === "send_failed" ? "Try again" : "Send now"))
                                    enabled: !chaftController.joinRequestSubmitInFlight && pendingRequestCard.modelData.status !== "sending"
                                    onClicked: root.app.sendPendingAccessRequest(pendingRequestCard.modelData)
                                }

                                Button {
                                    Layout.fillWidth: true
                                    visible: pendingRequestCard.modelData.canShareRequest
                                    text: "Copy link"
                                    enabled: !chaftController.joinRequestSubmitInFlight
                                    Accessible.name: "Copy request link"
                                    onClicked: root.app.copyPendingAccessRequest(pendingRequestCard.modelData)
                                }

                                Button {
                                    Layout.fillWidth: true
                                    visible: pendingRequestCard.modelData.canShareRequest
                                    text: "Save file"
                                    enabled: !chaftController.joinRequestSubmitInFlight
                                    Accessible.name: "Save request file"
                                    onClicked: root.app.openSavePendingAccessRequestDialog(pendingRequestCard.modelData)
                                }

                                Button {
                                    Layout.fillWidth: true
                                    visible: pendingRequestCard.modelData.canCheckResponse
                                    text: chaftController.accessEnvelopePullInFlight ? "Checking..." : "Check"
                                    enabled: !chaftController.joinRequestSubmitInFlight && !chaftController.accessEnvelopePullInFlight
                                    Accessible.name: "Check for access approval"
                                    onClicked: root.app.checkPendingAccessRequestResponse(pendingRequestCard.modelData)
                                }

                                Button {
                                    Layout.fillWidth: true
                                    visible: pendingRequestCard.modelData.canOpenInvite
                                    text: "Open invite"
                                    enabled: !chaftController.joinRequestSubmitInFlight
                                    onClicked: root.app.openWorkspaceEntry("join")
                                }

                                Button {
                                    Layout.fillWidth: true
                                    text: "Hide"
                                    enabled: !chaftController.joinRequestSubmitInFlight
                                    Accessible.name: "Hide access request reminder"
                                    onClicked: root.app.confirmDismissPendingAccessRequest(pendingRequestCard.modelData)
                                }
                            }
                        }
                    }
                }
            }

            GridLayout {
                Layout.fillWidth: true
                Layout.topMargin: Tokens.space3
                visible: !root.rawStoreMode
                columns: root.width > 700 ? 3 : (root.width > 520 ? 2 : 1)
                columnSpacing: Tokens.space3
                rowSpacing: Tokens.space3

                OnboardingActionCard {
                    Layout.fillWidth: true
                    primary: true
                    glyph: "#"
                    title: "Create workspace"
                    body: "Start a private space, then invite teammates any time."
                    actionable: root.app.runtimeAccessReady
                    onActivated: root.app.openWorkspaceEntry("create")
                }

                OnboardingActionCard {
                    Layout.fillWidth: true
                    glyph: "⇄"
                    title: "Join workspace"
                    body: "Have an invite, request link, or access file? Open it here to join or request access."
                    actionable: root.app.runtimeAccessReady
                    onActivated: root.app.openWorkspaceEntry("join")
                }

                OnboardingActionCard {
                    Layout.fillWidth: true
                    glyph: "↺"
                    title: "Restore workspace"
                    body: "Have a recovery kit? Restore access, then fetch history when a teammate is reachable."
                    actionable: root.app.runtimeAccessReady
                    onActivated: root.app.openWorkspaceEntry("join", "restore")
                }
            }

            Text {
                visible: !root.rawStoreMode
                text: "Explore demo workspace"
                color: Tokens.accent
                font.pixelSize: Tokens.fontSizeSm
                font.underline: demoLinkMouse.containsMouse
                activeFocusOnTab: true

                Accessible.role: Accessible.Button
                Accessible.name: "Explore demo workspace"
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
                    text: "Make it yours with 24 themes in Appearance."
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
                                root.app.applyThemeChoice(themeId);
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
}
