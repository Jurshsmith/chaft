pragma ComponentBehavior: Bound

import QtQuick
import QtTest
import Chaft

TestCase {
    id: testCase
    name: "DataPortabilityUx"
    when: windowShown

    property var panel: null

    QtObject {
        id: mockController
        property bool workspaceExportAvailable: true
        property bool runtimeLocked: false
        property string selectedWorkspaceId: "wrk_design"
        property var workspaceExportJob: ({ state: "idle" })
        property string lastExportPath: ""
        property string lastOpenedPath: ""

        function exportWorkspaceArchive(path) {
            lastExportPath = String(path || "")
            return true
        }

        function openContainingFolder(path) {
            lastOpenedPath = String(path || "")
            return true
        }
    }

    Component {
        id: panelComponent

        DataPortabilityPanel {
            width: 760
            height: 620
            controller: mockController
            workspaceName: "Design Guild"
        }
    }

    function init() {
        mockController.workspaceExportAvailable = true
        mockController.runtimeLocked = false
        mockController.workspaceExportJob = ({ state: "idle" })
        mockController.lastExportPath = ""
        mockController.lastOpenedPath = ""
        panel = panelComponent.createObject(testCase)
        verify(panel !== null)
        wait(0)
    }

    function cleanup() {
        panel.destroy()
        panel = null
    }

    function test_discloses_scope_and_adds_zip_extension() {
        var disclosure = findChild(panel, "workspaceCopyDisclosure")
        verify(disclosure !== null)
        verify(disclosure.text.indexOf("currently available on this device") >= 0)
        verify(disclosure.text.indexOf("locally available attachments") >= 0)
        verify(disclosure.text.indexOf("unsynced or missing history") >= 0)
        verify(disclosure.text.indexOf("encryption keys") >= 0)

        verify(/^chaft-design-guild-\d{8}-\d{6}-\d{3}\.zip$/
            .test(panel.suggestedFileName()))

        compare(panel.exportToPath("/tmp/design-guild-copy"), true)
        compare(mockController.lastExportPath, "/tmp/design-guild-copy.zip")
    }

    function test_running_state_blocks_duplicate_and_keeps_chat_usable_copy() {
        mockController.workspaceExportJob = ({
            state: "running",
            outputPath: "/tmp/design-guild-copy.zip"
        })
        wait(0)

        var button = findChild(panel, "downloadWorkspaceCopyButton")
        verify(button !== null)
        compare(button.enabled, false)
        compare(panel.exportToPath("/tmp/duplicate.zip"), false)
        compare(mockController.lastExportPath, "")

        var detail = findChild(panel, "workspaceCopyStatusDetail")
        verify(detail !== null)
        compare(detail.text, "You can continue using Chaft.")
        var busy = findChild(panel, "workspaceCopyBusyIndicator")
        verify(busy !== null)
        compare(busy.running, true)
    }

    function test_success_state_summarizes_and_opens_folder() {
        mockController.workspaceExportJob = ({
            state: "succeeded",
            outputPath: "/tmp/design-guild-copy.zip",
            channelCount: 4,
            messageCount: 127,
            includedAttachmentCount: 3,
            warningCount: 1
        })
        wait(0)

        var title = findChild(panel, "workspaceCopyStatusTitle")
        verify(title !== null)
        compare(title.text, "Workspace export saved")

        var detail = findChild(panel, "workspaceCopyStatusDetail")
        verify(detail !== null)
        verify(detail.text.indexOf("4 rooms, 127 messages, and 3 attachments") >= 0)
        verify(detail.text.indexOf("1 item may be missing") >= 0)
        verify(detail.text.indexOf("completeness.json") >= 0)

        var outputPath = findChild(panel, "workspaceExportOutputPath")
        verify(outputPath !== null)
        compare(outputPath.Accessible.name,
                "Export file path: /tmp/design-guild-copy.zip")

        var openButton = findChild(panel, "openWorkspaceCopyFolderButton")
        verify(openButton !== null)
        compare(panel.exportSucceeded, true)
        openButton.clicked()
        compare(mockController.lastOpenedPath, "/tmp/design-guild-copy.zip")
    }

    function test_failure_state_shows_reason_and_allows_retry() {
        mockController.workspaceExportJob = ({
            state: "failed",
            outputPath: "/tmp/design-guild-copy.zip",
            error: "The destination is not writable."
        })
        wait(0)

        var title = findChild(panel, "workspaceCopyStatusTitle")
        var detail = findChild(panel, "workspaceCopyStatusDetail")
        var button = findChild(panel, "downloadWorkspaceCopyButton")
        compare(title.text, "Workspace export failed")
        compare(detail.text, "The destination is not writable.")
        compare(button.text, "Try again")
        compare(button.enabled, true)
    }

    function test_completed_copy_from_another_workspace_does_not_replace_current_status() {
        mockController.workspaceExportJob = ({
            state: "succeeded",
            workspaceId: "wrk_previous",
            workspaceName: "Previous workspace",
            outputPath: "/tmp/previous-copy.zip",
            channelCount: 2,
            messageCount: 12
        })
        wait(0)

        var statusCard = findChild(panel, "workspaceCopyStatusCard")
        var button = findChild(panel, "downloadWorkspaceCopyButton")
        compare(panel.exportState, "idle")
        compare(statusCard.visible, false)
        compare(button.text, "Export ZIP…")
        compare(button.enabled, true)
    }

    function test_unavailable_reason_distinguishes_locked_and_unsupported() {
        mockController.workspaceExportAvailable = false
        mockController.runtimeLocked = true
        wait(0)

        var unavailable = findChild(panel, "workspaceCopyUnavailableText")
        verify(unavailable !== null)
        compare(unavailable.text, "Unlock Chaft to export this workspace.")

        mockController.runtimeLocked = false
        wait(0)
        compare(unavailable.text, "Update Chaft to export workspace data.")
    }
}
