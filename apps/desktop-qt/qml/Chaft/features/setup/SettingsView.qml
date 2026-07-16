import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Chaft

Item {
    id: root

    property var app
    property string destination: "settings"
    property string category: "profile"
    property alias approvalInviteModeEnabled: setupPanel.approvalInviteModeEnabled
    readonly property bool settingsDestination: destination === "settings"
    readonly property bool wideNavigation: width >= 860
    readonly property var personalCategories: [
        { id: "profile", label: "Profile" },
        { id: "preferences", label: "Preferences" }
    ]
    readonly property var workspaceCategories: [
        { id: "workspace", label: "General" },
        { id: "devices", label: "Devices & keys" },
        { id: "backup", label: "Backup" },
        { id: "advanced", label: "Advanced" }
    ]
    readonly property var availableCategories: chaftController.hasRuntimeWorkspace
        ? personalCategories.concat(workspaceCategories)
        : personalCategories

    signal closeRequested
    signal categoryRequested(string categoryId)
    signal peopleAccessRequested(bool focusInvite)

    function categoryIndex(categoryId) {
        for (var i = 0; i < root.availableCategories.length; i += 1) {
            if (root.availableCategories[i].id === categoryId) {
                return i
            }
        }
        return 0
    }

    function startPendingWorkspaceInviteReplacement(inviteId) {
        return setupPanel.startPendingWorkspaceInviteReplacement(inviteId)
    }

    function openPeopleAccessSection() {
        setupPanel.openPeopleAccessSection()
    }

    function openAccessRequestsSection() {
        setupPanel.openAccessRequestsSection()
    }

    function openFirstWaitingJoinRequestReview() {
        return setupPanel.openFirstWaitingJoinRequestReview()
    }

    function openInvitationsSection() {
        setupPanel.openInvitationsSection()
    }

    function focusInviteForm() {
        setupPanel.focusInviteForm()
    }

    function openProfileAvatarPicker() {
        setupPanel.openProfileAvatarPicker()
    }

    Rectangle {
        anchors.fill: parent
        color: Tokens.surfaceBase

        Rectangle {
            id: pageHeader
            anchors.top: parent.top
            anchors.left: parent.left
            anchors.right: parent.right
            height: 64
            color: Tokens.surfaceBase

            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: Tokens.space4
                anchors.rightMargin: Tokens.space3
                spacing: Tokens.space3

                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 1

                    Text {
                        Layout.fillWidth: true
                        text: root.settingsDestination ? "Settings" : "People & Access"
                        color: Tokens.textStrong
                        font.pixelSize: Tokens.fontSizeXl
                        font.weight: Font.Bold
                        elide: Text.ElideRight
                    }

                    Text {
                        visible: !root.settingsDestination
                            && root.app
                            && String(root.app.workspaceSnapshot.name || "").length > 0
                        Layout.fillWidth: true
                        text: root.app ? String(root.app.workspaceSnapshot.name || "") : ""
                        color: Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeXs
                        elide: Text.ElideRight
                    }
                }

                Button {
                    text: "Close"
                    implicitWidth: 68
                    implicitHeight: 34
                    Accessible.name: root.settingsDestination
                        ? "Close settings"
                        : "Close People & Access"
                    onClicked: root.closeRequested()
                    ToolTip.visible: hovered
                    ToolTip.text: Accessible.name
                }
            }

            Rectangle {
                anchors.left: parent.left
                anchors.right: parent.right
                anchors.bottom: parent.bottom
                height: 1
                color: Tokens.borderSubtle
            }
        }

        RowLayout {
            anchors.top: pageHeader.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            spacing: 0

            Rectangle {
                visible: root.settingsDestination && root.wideNavigation
                Layout.fillHeight: true
                Layout.preferredWidth: visible ? 184 : 0
                color: Tokens.surfaceRaised

                ColumnLayout {
                    anchors.top: parent.top
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.margins: Tokens.space3
                    spacing: Tokens.space1

                    Repeater {
                        model: root.availableCategories

                        delegate: ColumnLayout {
                            id: categoryGroup
                            required property var modelData

                            Layout.fillWidth: true
                            Layout.topMargin: String(categoryGroup.modelData.id || "") === "workspace"
                                ? Tokens.space3
                                : 0
                            spacing: Tokens.space1

                            Text {
                                Layout.fillWidth: true
                                Layout.leftMargin: Tokens.space2
                                visible: String(categoryGroup.modelData.id || "") === "profile"
                                    || String(categoryGroup.modelData.id || "") === "workspace"
                                text: String(categoryGroup.modelData.id || "") === "workspace"
                                    ? "Workspace"
                                    : "Personal"
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                font.weight: Font.DemiBold
                            }

                            Button {
                                id: categoryButton
                                Layout.fillWidth: true
                                implicitHeight: 36
                                checkable: true
                                checked: root.category === String(categoryGroup.modelData.id || "")
                                text: String(categoryGroup.modelData.label || "")
                                Accessible.name: text + " settings"
                                Accessible.description: checked
                                    ? "Current settings category"
                                    : "Open settings category"
                                onClicked: root.categoryRequested(
                                    String(categoryGroup.modelData.id || ""))

                                contentItem: Text {
                                    text: categoryButton.text
                                    color: categoryButton.enabled
                                        ? Tokens.textStrong
                                        : Tokens.textMuted
                                    font: categoryButton.font
                                    horizontalAlignment: Text.AlignLeft
                                    verticalAlignment: Text.AlignVCenter
                                    elide: Text.ElideRight
                                }
                            }
                        }
                    }
                }

                Rectangle {
                    anchors.top: parent.top
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    width: 1
                    color: Tokens.borderSubtle
                }
            }

            Item {
                id: pageContent
                Layout.fillWidth: true
                Layout.fillHeight: true

                ComboBox {
                    id: compactCategorySelector
                    visible: root.settingsDestination && !root.wideNavigation
                    anchors.top: parent.top
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.topMargin: Tokens.space3
                    anchors.leftMargin: Tokens.space4
                    anchors.rightMargin: Tokens.space4
                    model: root.availableCategories
                    textRole: "label"
                    Accessible.name: "Settings category"
                    onActivated: function(index) {
                        var row = root.availableCategories[index] || ({})
                        root.categoryRequested(String(row.id || "profile"))
                    }
                }

                Binding {
                    target: compactCategorySelector
                    property: "currentIndex"
                    value: root.categoryIndex(root.category)
                }

                SetupPanel {
                    id: setupPanel
                    app: root.app
                    category: root.settingsDestination ? root.category : "people"
                    anchors.top: compactCategorySelector.visible
                        ? compactCategorySelector.bottom
                        : parent.top
                    anchors.bottom: parent.bottom
                    anchors.horizontalCenter: parent.horizontalCenter
                    anchors.topMargin: compactCategorySelector.visible
                        ? Tokens.space3
                        : Tokens.space4
                    anchors.bottomMargin: Tokens.space4
                    width: Math.max(0, Math.min(
                        parent.width - Tokens.space4 * 2,
                        root.settingsDestination ? 840 : 920))
                    onCategoryRequested: function(categoryId) {
                        if (categoryId === "people") {
                            root.peopleAccessRequested(false)
                        } else {
                            root.categoryRequested(categoryId)
                        }
                    }
                }
            }
        }
    }
}
