import QtQuick
import QtTest
import Chaft

TestCase {
    id: testCase
    name: "TimelineMessageUx"
    when: windowShown

    property var timelineView: null
    property var inspectorPanel: null

    Component {
        id: timelineComponent

        TimelineView {
            width: 720
            height: 520
            actionsEnabled: true
            autoFollowLatest: false
        }
    }

    Component {
        id: inspectorComponent

        InspectorThreadPanel {
            width: 320
            replyCount: 1
            replyPreviews: [{
                messageId: "thread-reply",
                authorDeviceId: "device-3",
                authorDisplayName: "Lin",
                body: "A thread reply",
                deleted: false
            }]
        }
    }

    SignalSpy {
        id: parentReplySpy
        signalName: "replyParentRequested"
    }

    SignalSpy {
        id: threadReplySpy
        signalName: "replySelected"
    }

    function init() {
        timelineView = timelineComponent.createObject(testCase)
        verify(timelineView !== null)
        parentReplySpy.target = timelineView
        inspectorPanel = inspectorComponent.createObject(testCase)
        verify(inspectorPanel !== null)
        threadReplySpy.target = inspectorPanel
    }

    function cleanup() {
        parentReplySpy.clear()
        parentReplySpy.target = null
        threadReplySpy.clear()
        threadReplySpy.target = null
        timelineView.destroy()
        timelineView = null
        inspectorPanel.destroy()
        inspectorPanel = null
    }

    function message(body, replyPreview) {
        return {
            kind: "message",
            eventId: "event-1",
            messageId: "message-1",
            authorDeviceId: "device-1",
            authorDisplayName: "Ada",
            physicalMs: Date.now(),
            body: body,
            encrypted: true,
            bodyDecrypted: true,
            deleted: false,
            attachments: [],
            reactions: {},
            myReactions: [],
            threadReplyCount: 0,
            replyPreview: replyPreview || null
        }
    }

    function test_untrusted_resource_markup_falls_back_to_plain_text() {
        var source = "**Bold** ![tracker](https://tracker.invalid/pixel.png)"
            + " <img src=\"file:///tmp/private.png\"> [link](https://example.com)"

        compare(timelineView.messageCanUseMarkdown(source), false)
        compare(timelineView.messageCanUseMarkdown(
            "**Bold** `code` [link](https://example.com)"
        ), true)
        compare(timelineView.messageCanUseMarkdown("Two is < three **with emphasis**"), true)
        compare(timelineView.messageCanUseMarkdown("<b>Raw HTML</b>"), false)

        timelineView.timelineModel = [message(source, null)]
        timelineView.forceLayout()
        wait(0)

        var row = timelineView.itemAtIndex(0)
        verify(row !== null)
        var body = findChild(row, "timelineMessageBody")
        verify(body !== null)
        compare(body.textFormat, TextEdit.PlainText)
        compare(body.text, source)
        compare(body.readOnly, true)
        compare(body.selectByMouse, true)
        compare(body.selectByKeyboard, true)
        body.selectAll()
        verify(body.selectedText.length > 0)
    }

    function test_long_message_can_expand_and_collapse() {
        var lines = []
        for (var i = 0; i < 40; i += 1) {
            lines.push("Line " + String(i) + " with enough text to remain readable.")
        }
        timelineView.timelineModel = [message(lines.join("\n\n"), null)]
        timelineView.forceLayout()
        wait(0)

        var row = timelineView.itemAtIndex(0)
        verify(row !== null)
        tryCompare(row, "bodyOverflowing", true)
        compare(row.bodyExpanded, false)

        var expansionButton = findChild(row, "messageExpansionButton")
        verify(expansionButton !== null)
        compare(expansionButton.label, "Show more")

        expansionButton.activated()
        compare(row.bodyExpanded, true)
        compare(expansionButton.label, "Show less")

        timelineView.timelineModel = [message(lines.join("\n\n"), null)]
        timelineView.forceLayout()
        wait(0)
        row = timelineView.itemAtIndex(0)
        verify(row !== null)
        compare(row.bodyExpanded, true)
        expansionButton = findChild(row, "messageExpansionButton")
        verify(expansionButton !== null)

        expansionButton.activated()
        compare(row.bodyExpanded, false)
        compare(expansionButton.label, "Show more")
    }

    function test_pending_message_is_visible_without_destructive_actions() {
        var pending = message("A message that is still being saved", null)
        pending.eventId = "pending-event-1"
        pending.messageId = ""
        pending.pendingLocal = true
        pending.deliveryState = "Saving on this device..."
        timelineView.timelineModel = [pending]
        timelineView.forceLayout()
        wait(0)

        var row = timelineView.itemAtIndex(0)
        verify(row !== null)
        compare(row.pendingLocal, true)
        var deliveryStatus = findChild(row, "pendingDeliveryStatus")
        verify(deliveryStatus !== null)
        compare(deliveryStatus.text, "Saving on this device...")
        var body = findChild(row, "timelineMessageBody")
        verify(body !== null)
        compare(body.text.trim(), pending.body)
    }

    function test_parent_reply_preview_emits_navigation_request() {
        timelineView.timelineModel = [message("Reply body", {
            messageId: "parent-message",
            authorDeviceId: "device-2",
            authorDisplayName: "Grace",
            body: "Parent body",
            deleted: false
        })]
        timelineView.forceLayout()
        wait(0)

        var row = timelineView.itemAtIndex(0)
        verify(row !== null)
        var preview = findChild(row, "replyParentPreview")
        verify(preview !== null)
        compare(preview.parentMessageId, "parent-message")

        preview.activate()
        compare(parentReplySpy.count, 1)
        compare(parentReplySpy.signalArguments[0][0], "parent-message")
    }

    function test_thread_reply_preview_emits_navigation_request() {
        wait(0)
        var replyRow = findChild(inspectorPanel, "threadReplyRow")
        verify(replyRow !== null)
        compare(replyRow.replyMessageId, "thread-reply")

        replyRow.activate()
        compare(threadReplySpy.count, 1)
        compare(threadReplySpy.signalArguments[0][0], "thread-reply")
    }
}
