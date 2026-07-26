import QtQuick
import QtTest

TestCase {
    name: "QtSdk"

    Item {
        id: item
        width: 16
        height: 16
    }

    function test_qtQuickIsUsable() {
        compare(item.width, 16)
        verify(item.height > 0)
    }
}
