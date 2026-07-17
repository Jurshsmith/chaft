import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import QtCore
import Chaft

Item {
    id: root

    property var controller: null
    property string workspaceName: ""
    readonly property var exportJob: root.controller
        ? (root.controller.workspaceExportJob || ({ state: "idle" }))
        : ({ state: "idle" })
    readonly property string rawExportState: String(root.exportJob.state || "idle")
    readonly property string selectedWorkspaceId: root.controller
        ? String(root.controller.selectedWorkspaceId || "")
        : ""
    readonly property string jobWorkspaceId: String(root.exportJob.workspaceId || "")
    readonly property bool exportJobMatchesWorkspace:
        root.selectedWorkspaceId.length === 0
        || root.jobWorkspaceId.length === 0
        || root.selectedWorkspaceId === root.jobWorkspaceId
    readonly property string exportState: root.rawExportState === "running"
        || root.exportJobMatchesWorkspace
        ? root.rawExportState
        : "idle"
    readonly property bool exportRunning: root.rawExportState === "running"
    readonly property bool exportSucceeded: root.exportState === "succeeded"
    readonly property bool exportFailed: root.exportState === "failed"
    readonly property bool exportAvailable: root.controller
        ? root.controller.workspaceExportAvailable === true
        : false

    function safeWorkspaceSlug() {
        var value = String(root.workspaceName || "workspace")
            .trim()
            .toLowerCase()
            .replace(/[^a-z0-9._-]+/g, "-")
            .replace(/^[._-]+|[._-]+$/g, "")
        return value.length > 0 ? value.slice(0, 64) : "workspace"
    }

    function suggestedFileName() {
        return "chaft-" + root.safeWorkspaceSlug() + "-"
            + new Date().toISOString().slice(0, 10) + ".zip"
    }

    function fileUrlFromLocalPath(path) {
        var normalized = String(path || "").replace(/\\/g, "/")
        if (normalized.length === 0) {
            return ""
        }
        if (Qt.platform.os === "windows") {
            return "file:///" + encodeURI(normalized)
                .replace(/#/g, "%23").replace(/\?/g, "%3F")
        }
        return "file://" + encodeURI(normalized)
            .replace(/#/g, "%23").replace(/\?/g, "%3F")
    }

    function suggestedFileUrl() {
        var folder = StandardPaths.writableLocation(StandardPaths.DownloadLocation)
        if (String(folder || "").length === 0) {
            folder = StandardPaths.writableLocation(StandardPaths.DocumentsLocation)
        }
        if (String(folder || "").length === 0) {
            folder = StandardPaths.writableLocation(StandardPaths.HomeLocation)
        }
        if (String(folder || "").length === 0) {
            return root.suggestedFileName()
        }
        return root.fileUrlFromLocalPath(
            String(folder).replace(/\/$/, "") + "/" + root.suggestedFileName())
    }

    function localPathFromUrl(urlValue) {
        var value = String(urlValue || "")
        if (value.indexOf("file://") !== 0) {
            return value
        }
        var path = decodeURIComponent(value.replace(/^file:\/\//, ""))
        if (Qt.platform.os === "windows" && path.charAt(0) === "/") {
            path = path.slice(1)
        }
        return path
    }

    function zipOutputPath(path) {
        var value = String(path || "")
        return /\.zip$/i.test(value) ? value : value + ".zip"
    }

    function exportToPath(path) {
        if (!root.controller || root.exportRunning || !root.exportAvailable) {
            return false
        }
        return root.controller.exportWorkspaceArchive(root.zipOutputPath(path))
    }

    function chooseExportLocation() {
        if (root.exportRunning || !root.exportAvailable) {
            return
        }
        saveWorkspaceCopyDialog.selectedFile = root.suggestedFileUrl()
        saveWorkspaceCopyDialog.open()
    }

    ScrollView {
        anchors.fill: parent
        clip: true
        contentWidth: availableWidth
        contentHeight: panelColumn.implicitHeight + Tokens.space4 * 2
        ScrollBar.horizontal.policy: ScrollBar.AlwaysOff
        ScrollBar.vertical.policy: ScrollBar.AsNeeded

        ColumnLayout {
            id: panelColumn
            x: Tokens.space2
            y: Tokens.space2
            width: Math.max(0, parent.width - Tokens.space4)
            spacing: Tokens.space4

            ColumnLayout {
                Layout.fillWidth: true
                spacing: Tokens.space1

                Text {
                    Layout.fillWidth: true
                    text: "Data & portability"
                    color: Tokens.textStrong
                    font.pixelSize: Tokens.fontSizeXl
                    font.weight: Font.Bold
                }

                Text {
                    Layout.fillWidth: true
                    text: "Take a usable copy of this workspace with you."
                    color: Tokens.textMuted
                    font.pixelSize: Tokens.fontSizeSm
                    wrapMode: Text.WordWrap
                }
            }

            Rectangle {
                Layout.fillWidth: true
                implicitHeight: exportCardColumn.implicitHeight + Tokens.space4 * 2
                radius: Tokens.radiusMd
                color: Tokens.surfaceRaised
                border.width: 1
                border.color: Tokens.borderSubtle

                ColumnLayout {
                    id: exportCardColumn
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: Tokens.space4
                    spacing: Tokens.space3

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Tokens.space3

                        ColumnLayout {
                            Layout.fillWidth: true
                            spacing: Tokens.space1

                            Text {
                                Layout.fillWidth: true
                                text: "Workspace copy"
                                color: Tokens.textStrong
                                font.pixelSize: Tokens.fontSizeLg
                                font.weight: Font.DemiBold
                            }

                            Text {
                                Layout.fillWidth: true
                                text: "ZIP · browsable HTML · structured JSONL"
                                color: Tokens.textMuted
                                font.pixelSize: Tokens.fontSizeXs
                                wrapMode: Text.WordWrap
                            }
                        }

                        Button {
                            objectName: "downloadWorkspaceCopyButton"
                            text: root.exportSucceeded
                                ? "Download another copy"
                                : (root.exportFailed ? "Try again" : "Download workspace copy")
                            enabled: root.exportAvailable && !root.exportRunning
                            Accessible.name: text
                            Accessible.description: "Choose where to save a portable workspace ZIP"
                            onClicked: root.chooseExportLocation()
                        }
                    }

                    Text {
                        objectName: "workspaceCopyDisclosure"
                        Layout.fillWidth: true
                        text: "Includes the current workspace data this device can read—including private rooms and direct messages—plus available attachments. Deleted message content, encryption keys, credentials, invites, and peer addresses are not included. This is a portability copy, not a Chaft backup or restore file."
                        color: Tokens.textStrong
                        font.pixelSize: Tokens.fontSizeSm
                        wrapMode: Text.WordWrap
                    }

                    Text {
                        Layout.fillWidth: true
                        text: "The saved ZIP is not encrypted. Keep it somewhere private and share it only with people who should be able to read the workspace."
                        color: Tokens.warningText
                        font.pixelSize: Tokens.fontSizeSm
                        wrapMode: Text.WordWrap
                    }

                    Text {
                        objectName: "workspaceCopyUnavailableText"
                        visible: !root.exportAvailable && !root.exportRunning
                        Layout.fillWidth: true
                        text: "Unlock this workspace or update Chaft before creating a copy."
                        color: Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeXs
                        wrapMode: Text.WordWrap
                    }
                }
            }

            Rectangle {
                objectName: "workspaceCopyStatusCard"
                visible: root.exportState !== "idle"
                Layout.fillWidth: true
                implicitHeight: exportStatusColumn.implicitHeight + Tokens.space4 * 2
                radius: Tokens.radiusMd
                color: root.exportFailed ? Tokens.warningSurface : Tokens.surfaceRaised
                border.width: 1
                border.color: root.exportFailed ? Tokens.warning : Tokens.borderSubtle

                ColumnLayout {
                    id: exportStatusColumn
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: Tokens.space4
                    spacing: Tokens.space2

                    RowLayout {
                        Layout.fillWidth: true
                        spacing: Tokens.space2

                        BusyIndicator {
                            objectName: "workspaceCopyBusyIndicator"
                            visible: root.exportRunning
                            running: root.exportRunning
                            implicitWidth: 24
                            implicitHeight: 24
                            Accessible.name: "Creating workspace copy"
                        }

                        Text {
                            objectName: "workspaceCopyStatusTitle"
                            Layout.fillWidth: true
                            text: root.exportRunning
                                ? root.runningStatusTitle()
                                : (root.exportSucceeded
                                    ? "Workspace copy saved"
                                    : "Workspace copy was not created")
                            color: root.exportFailed
                                ? Tokens.warningText
                                : Tokens.textStrong
                            font.pixelSize: Tokens.fontSizeMd
                            font.weight: Font.DemiBold
                            wrapMode: Text.WordWrap
                        }
                    }

                    Text {
                        objectName: "workspaceCopyStatusDetail"
                        Layout.fillWidth: true
                        text: root.exportRunning
                            ? "You can keep using Chaft while this finishes."
                            : (root.exportFailed
                                ? String(root.exportJob.error || "Try a different save location.")
                                : root.successSummary())
                        color: root.exportFailed
                            ? Tokens.warningText
                            : Tokens.textMuted
                        font.pixelSize: Tokens.fontSizeSm
                        wrapMode: Text.WordWrap
                    }

                    Text {
                        visible: !root.exportRunning
                            && String(root.exportJob.outputPath || "").length > 0
                        Layout.fillWidth: true
                        text: String(root.exportJob.outputPath || "")
                        color: Tokens.textMuted
                        font.family: Tokens.fontMono
                        font.pixelSize: Tokens.fontSizeXs
                        elide: Text.ElideMiddle
                    }

                    Button {
                        objectName: "openWorkspaceCopyFolderButton"
                        visible: root.exportSucceeded
                        text: "Open containing folder"
                        Accessible.name: text
                        onClicked: {
                            if (root.controller) {
                                root.controller.openContainingFolder(
                                    String(root.exportJob.outputPath || ""))
                            }
                        }
                    }
                }
            }
        }
    }

    function successSummary() {
        var channels = Number(root.exportJob.channelCount || 0)
        var messages = Number(root.exportJob.messageCount || 0)
        var attachments = Number(root.exportJob.includedAttachmentCount || 0)
        var warnings = Number(root.exportJob.warningCount || 0)
        var summary = channels + " room" + (channels === 1 ? "" : "s")
            + ", " + messages + " message" + (messages === 1 ? "" : "s")
            + ", and " + attachments + " attachment"
            + (attachments === 1 ? "" : "s") + " included."
        if (warnings > 0) {
            summary += " Review the archive’s completeness report for "
                + warnings + " notice" + (warnings === 1 ? "." : "s.")
        }
        return summary
    }

    function runningStatusTitle() {
        var name = String(root.exportJob.workspaceName || "").trim()
        return name.length > 0
            ? "Creating a copy of “" + name + "”…"
            : "Creating your workspace copy…"
    }

    FileDialog {
        id: saveWorkspaceCopyDialog
        title: "Save workspace copy"
        fileMode: FileDialog.SaveFile
        nameFilters: [ "Chaft workspace copies (*.zip)" ]
        onAccepted: root.exportToPath(root.localPathFromUrl(selectedFile))
    }
}
