import QtQuick
import QtTest
import Chaft

TestCase {
    id: testCase
    name: "ComposerReactivityUx"
    when: windowShown

    property var composer: null

    Component {
        id: composerComponent

        ComposerBar {
            width: 720
            channelName: "general"
        }
    }

    SignalSpy {
        id: sendSpy
        signalName: "sendRequested"
    }

    SignalSpy {
        id: blockedActionSpy
        signalName: "blockedActionRequested"
    }

    function init() {
        composer = composerComponent.createObject(testCase)
        verify(composer !== null)
        sendSpy.target = composer
        blockedActionSpy.target = composer
    }

    function cleanup() {
        sendSpy.clear()
        sendSpy.target = null
        blockedActionSpy.clear()
        blockedActionSpy.target = null
        composer.destroy()
        composer = null
    }

    function test_pending_operation_preserves_draft_until_success() {
        composer.setDraft("Keep this visible")
        composer.operationPending = true

        compare(composer.submitDraft(), false)
        compare(composer.draftText(), "Keep this visible")
        compare(sendSpy.count, 0)

        composer.operationPending = false
        compare(composer.submitDraft(), true)
        compare(sendSpy.count, 1)
        compare(sendSpy.signalArguments[0][0], "Keep this visible")
        // The owner clears only after the durable completion signal.
        compare(composer.draftText(), "Keep this visible")
    }

    function test_missing_key_blocks_send_but_offers_recovery_action() {
        composer.setDraft("Send after access is ready")
        composer.blockedReason = "This private room is still preparing secure message access."
        composer.blockedActionLabel = "Check for key"
        wait(0)

        compare(composer.blocked, true)
        compare(composer.submitDraft(), false)
        compare(composer.draftText(), "Send after access is ready")
        compare(sendSpy.count, 0)

        var field = findChild(composer, "composerMessageField")
        verify(field !== null)
        compare(field.enabled, true)

        var reason = findChild(composer, "composerBlockedReason")
        verify(reason !== null)
        compare(reason.text, composer.blockedReason)

        var action = findChild(composer, "composerBlockedAction")
        verify(action !== null)
        compare(action.text, "Check for key")
        action.clicked()
        compare(blockedActionSpy.count, 1)
    }
}
