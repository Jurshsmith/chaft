pragma ComponentBehavior: Bound

import QtQuick
import QtTest
import Chaft

TestCase {
    id: testCase
    name: "ToastHostUx"
    when: windowShown

    ToastHost {
        id: toastHost
        width: 340

        onActionTriggered: function(actionId) {
            if (actionId === "review-workspace-export:wrk_previous") {
                toastHost.show(
                    "warning",
                    "Couldn’t switch workspaces. Try again.",
                    "Try again",
                    actionId,
                    8000)
            }
        }
    }

    function init() {
        toastHost.entries = []
    }

    function test_action_can_replace_its_toast_before_original_is_dismissed() {
        toastHost.show(
            "warning",
            "Workspace export failed.",
            "Review",
            "review-workspace-export:wrk_previous",
            8000)
        wait(0)

        var actionButton = findChild(toastHost, "toastActionButton")
        verify(actionButton !== null)
        actionButton.clicked()
        wait(0)

        compare(toastHost.entries.length, 1)
        compare(toastHost.entries[0].message,
                "Couldn’t switch workspaces. Try again.")
        compare(toastHost.entries[0].actionLabel, "Try again")
        compare(toastHost.entries[0].actionId,
                "review-workspace-export:wrk_previous")
    }
}
