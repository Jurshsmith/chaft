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
    property bool appearanceOptionsOpen: false

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
        var normalized = status.toLowerCase();
        if (status.length === 0
                || root.idleStatuses.indexOf(normalized) !== -1
                || normalized.indexOf("sharing address ") === 0) {
            return "";
        }
        return status;
    }
    readonly property var pendingAccessRequests: root.app ? root.app.pendingAccessRequests : []
    readonly property var curatedThemeIds: [
        "chaft-signal",
        "chaft-canvas",
        "midnight-relay",
        "terminal-phosphor",
        "sakura-morning"
    ]
    readonly property var onboardingSteps: [
        {
            step: "1",
            title: "Create",
            caption: "Name a private workspace"
        },
        {
            step: "2",
            title: "Invite",
            caption: "Share a one-use or multi-use invite"
        },
        {
            step: "3",
            title: "Stay reachable",
            caption: "Keep Chaft open while teammates sync"
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

            BrandMark {
                Layout.preferredWidth: 72
                Layout.preferredHeight: 72
                Layout.bottomMargin: Tokens.space1
            }

            Text {
                Layout.fillWidth: true
                text: root.rawStoreMode
                    ? "Viewing read-only history"
                    : "Start with a workspace."
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
                    ? "Choose a workspace folder to unlock the full app."
                    : "Create a private space or join one with an invite. Conversations sync directly when teammates are online."
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
                    text: "Pending access"
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
                        readonly property bool secureClaim: String(
                            modelData.sourceType || "").trim() === "invite_claim"

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
                                visible: false
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

                            Text {
                                Layout.fillWidth: true
                                text: pendingRequestCard.modelData.workspaceLabel
                                    + " · " + pendingRequestCard.modelData.displayLabel
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                elide: Text.ElideRight
                            }

                            RowLayout {
                                Layout.fillWidth: true
                                spacing: Tokens.space2

                                Button {
                                    Layout.fillWidth: true
                                    visible: pendingRequestCard.modelData.canOpenInvite
                                        || pendingRequestCard.modelData.canCheckResponse
                                        || pendingRequestCard.modelData.canSendDirect
                                        || pendingRequestCard.modelData.canShareRequest
                                    text: pendingRequestCard.modelData.status === "approved"
                                        && pendingRequestCard.modelData.canOpenInvite
                                        ? (pendingRequestCard.secureClaim
                                            ? "Open access"
                                            : "Open invite")
                                        : ((pendingRequestCard.modelData.status === "sent"
                                                || pendingRequestCard.modelData.status === "sent_unpersisted"
                                                || pendingRequestCard.modelData.status === "unverified_response")
                                            && pendingRequestCard.modelData.canCheckResponse
                                            ? (chaftController.accessEnvelopePullInFlight
                                                ? "Checking..."
                                                : (pendingRequestCard.modelData.status === "unverified_response"
                                                        || pendingRequestCard.modelData.status === "sent_unpersisted"
                                                    ? "Check again"
                                                    : (pendingRequestCard.secureClaim
                                                        ? "Check for access"
                                                        : "Check for approval")))
                                            : (pendingRequestCard.modelData.status === "sending"
                                                ? "Sending..."
                                                : (pendingRequestCard.modelData.status === "send_failed"
                                                    ? "Try again"
                                                    : (pendingRequestCard.modelData.canSendDirect
                                                        ? "Send now"
                                                        : (pendingRequestCard.modelData.canShareRequest
                                                            ? (pendingRequestCard.secureClaim
                                                                ? "Copy join request"
                                                                : "Copy request link")
                                                            : "Open invite")))))
                                    enabled: !chaftController.joinRequestSubmitInFlight
                                        && !chaftController.accessEnvelopePullInFlight
                                        && pendingRequestCard.modelData.status !== "sending"
                                    onClicked: {
                                        if (pendingRequestCard.modelData.status === "approved"
                                                && pendingRequestCard.modelData.canOpenInvite) {
                                            root.app.openWorkspaceEntry("join")
                                        } else if ((pendingRequestCard.modelData.status === "sent"
                                                    || pendingRequestCard.modelData.status === "sent_unpersisted"
                                                    || pendingRequestCard.modelData.status === "unverified_response")
                                                && pendingRequestCard.modelData.canCheckResponse) {
                                            root.app.checkPendingAccessRequestResponse(
                                                pendingRequestCard.modelData)
                                        } else if (pendingRequestCard.modelData.canSendDirect) {
                                            root.app.sendPendingAccessRequest(
                                                pendingRequestCard.modelData)
                                        } else if (pendingRequestCard.modelData.canShareRequest) {
                                            root.app.copyPendingAccessRequest(
                                                pendingRequestCard.modelData)
                                        } else {
                                            root.app.openWorkspaceEntry("join")
                                        }
                                    }
                                }

                                Item {
                                    Layout.fillWidth: true
                                }

                                Button {
                                    text: "⋯"
                                    Accessible.name: pendingRequestCard.secureClaim
                                        ? "More join request actions"
                                        : "More request actions"
                                    onClicked: pendingRequestActionsMenu.open()

                                    Menu {
                                        id: pendingRequestActionsMenu
                                        y: parent.height

                                        MenuItem {
                                            visible: pendingRequestCard.modelData.status === "sent"
                                                && pendingRequestCard.modelData.canSendDirect
                                            text: pendingRequestCard.secureClaim
                                                ? "Resend join request"
                                                : "Resend request"
                                            enabled: !chaftController.joinRequestSubmitInFlight
                                            onTriggered: root.app.sendPendingAccessRequest(
                                                pendingRequestCard.modelData)
                                        }

                                        MenuItem {
                                            visible: pendingRequestCard.modelData.canShareRequest
                                            text: pendingRequestCard.secureClaim
                                                ? "Copy join request"
                                                : "Copy request link"
                                            enabled: !chaftController.joinRequestSubmitInFlight
                                            onTriggered: root.app.copyPendingAccessRequest(
                                                pendingRequestCard.modelData)
                                        }

                                        MenuItem {
                                            visible: pendingRequestCard.modelData.canShareRequest
                                            text: pendingRequestCard.secureClaim
                                                ? "Save join request"
                                                : "Save request file"
                                            enabled: !chaftController.joinRequestSubmitInFlight
                                            onTriggered: root.app.openSavePendingAccessRequestDialog(
                                                pendingRequestCard.modelData)
                                        }

                                        MenuItem {
                                            visible: pendingRequestCard.modelData.canCheckResponse
                                                && pendingRequestCard.modelData.status !== "sent"
                                            text: pendingRequestCard.secureClaim
                                                ? "Check for access"
                                                : "Check for approval"
                                            enabled: !chaftController.joinRequestSubmitInFlight
                                                && !chaftController.accessEnvelopePullInFlight
                                            onTriggered: root.app.checkPendingAccessRequestResponse(
                                                pendingRequestCard.modelData)
                                        }

                                        MenuItem {
                                            text: pendingRequestCard.secureClaim
                                                ? "Hide join request"
                                                : "Hide request"
                                            enabled: !chaftController.joinRequestSubmitInFlight
                                            onTriggered: root.app.confirmDismissPendingAccessRequest(
                                                pendingRequestCard.modelData)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            ColumnLayout {
                Layout.fillWidth: true
                Layout.topMargin: Tokens.space3
                visible: !root.rawStoreMode
                spacing: Tokens.space3

                OnboardingActionCard {
                    Layout.fillWidth: true
                    primary: true
                    glyph: "#"
                    title: "Create workspace"
                    body: "Create a private space and invite your team."
                    actionable: root.app.runtimeAccessReady
                    onActivated: root.app.openWorkspaceEntry("create")
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: Tokens.space2

                    Text {
                        Layout.fillWidth: true
                        text: "Already have a workspace?"
                        color: Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeSm
                        elide: Text.ElideRight
                    }

                    Button {
                        text: "Join workspace"
                        enabled: root.app.runtimeAccessReady
                        Accessible.description: "Use an invite, request link, or access file"
                        onClicked: root.app.openWorkspaceEntry("join")
                    }
                }

                Flow {
                    Layout.fillWidth: true
                    spacing: Tokens.space1

                    Button {
                        flat: true
                        text: "Import key kit"
                        enabled: root.app.runtimeAccessReady
                        Accessible.name: "Import decryption key kit"
                        Accessible.description: "Import saved decryption keys. A fresh device still needs an invite."
                        onClicked: root.app.openWorkspaceEntry("join", "restore")
                    }

                    Button {
                        flat: true
                        text: "Explore demo"
                        Accessible.name: "Explore demo workspace"
                        Accessible.description: "Open a read-only sample workspace"
                        onClicked: root.app.startDemoTour()
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

                Button {
                    flat: true
                    text: root.appearanceOptionsOpen
                        ? "Hide theme previews"
                        : "Preview themes"
                    Accessible.description: root.appearanceOptionsOpen
                        ? "Collapse theme choices"
                        : "Show a few theme choices"
                    onClicked: root.appearanceOptionsOpen = !root.appearanceOptionsOpen
                }

                Flow {
                    Layout.fillWidth: true
                    visible: root.appearanceOptionsOpen
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

                Text {
                    Layout.fillWidth: true
                    visible: root.appearanceOptionsOpen
                    text: String(Themes.catalog.length) + " themes are available in Appearance."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeXs
                    wrapMode: Text.WordWrap
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
