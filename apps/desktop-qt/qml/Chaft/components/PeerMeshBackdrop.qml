import QtQuick
import Chaft

// Token-colored peer constellation for empty surfaces: one accent node (this
// device) among drifting muted peers. Layout is seeded deterministically so
// screenshots stay stable; drift and pulse run only while visible and
// Tokens.motionEnabled. Alpha stays low so foreground text contrast is
// unaffected in every theme.
Canvas {
    id: root
    property int nodeCount: 22
    property real linkDistance: 0.24
    property real pulsePhase: 0.5
    property var meshNodes: []

    onWidthChanged: root.requestPaint()
    onHeightChanged: root.requestPaint()
    onVisibleChanged: {
        if (visible) {
            root.requestPaint()
        }
    }
    Component.onCompleted: {
        root.rebuildNodes()
        root.requestPaint()
    }

    function seededSequence(count) {
        var values = []
        var state = 0x9e3779b9
        for (var i = 0; i < count; ++i) {
            state = (state ^ (state << 13)) >>> 0
            state = (state ^ (state >>> 17)) >>> 0
            state = (state ^ (state << 5)) >>> 0
            values.push((state >>> 8) / 16777216.0)
        }
        return values
    }

    function rebuildNodes() {
        var randoms = root.seededSequence(root.nodeCount * 4)
        var built = []
        for (var i = 0; i < root.nodeCount; ++i) {
            built.push({
                x: randoms[i * 4],
                y: randoms[i * 4 + 1],
                vx: (randoms[i * 4 + 2] - 0.5) * 0.0009,
                vy: (randoms[i * 4 + 3] - 0.5) * 0.0009
            })
        }
        root.meshNodes = built
    }

    function stepNodes() {
        var moved = root.meshNodes
        for (var i = 0; i < moved.length; ++i) {
            var node = moved[i]
            node.x += node.vx
            node.y += node.vy
            if (node.x < -0.05) { node.x = 1.05 }
            if (node.x > 1.05) { node.x = -0.05 }
            if (node.y < -0.05) { node.y = 1.05 }
            if (node.y > 1.05) { node.y = -0.05 }
        }
        root.requestPaint()
    }

    Timer {
        interval: 40
        repeat: true
        running: root.visible && Tokens.motionEnabled
        onTriggered: root.stepNodes()
    }

    SequentialAnimation on pulsePhase {
        running: root.visible && Tokens.motionEnabled
        loops: Animation.Infinite
        NumberAnimation { from: 0; to: 1; duration: 1700; easing.type: Easing.InOutSine }
        NumberAnimation { from: 1; to: 0; duration: 1700; easing.type: Easing.InOutSine }
    }
    onPulsePhaseChanged: root.requestPaint()

    onPaint: {
        var ctx = root.getContext("2d")
        ctx.clearRect(0, 0, root.width, root.height)
        if (root.meshNodes.length === 0 || root.width <= 0 || root.height <= 0) {
            return
        }

        var muted = Tokens.textMuted
        var accent = Tokens.accent
        var maxDistance = root.linkDistance
        var i
        var j

        for (i = 0; i < root.meshNodes.length; ++i) {
            for (j = i + 1; j < root.meshNodes.length; ++j) {
                var dx = root.meshNodes[i].x - root.meshNodes[j].x
                var dy = root.meshNodes[i].y - root.meshNodes[j].y
                var distance = Math.sqrt(dx * dx + dy * dy)
                if (distance < maxDistance) {
                    var strength = 1.0 - distance / maxDistance
                    ctx.strokeStyle = Qt.rgba(muted.r, muted.g, muted.b, 0.05 + strength * 0.07)
                    ctx.lineWidth = 1
                    ctx.beginPath()
                    ctx.moveTo(root.meshNodes[i].x * root.width, root.meshNodes[i].y * root.height)
                    ctx.lineTo(root.meshNodes[j].x * root.width, root.meshNodes[j].y * root.height)
                    ctx.stroke()
                }
            }
        }

        for (i = 1; i < root.meshNodes.length; ++i) {
            ctx.fillStyle = Qt.rgba(muted.r, muted.g, muted.b, 0.3)
            ctx.beginPath()
            ctx.arc(root.meshNodes[i].x * root.width, root.meshNodes[i].y * root.height,
                    1.6, 0, Math.PI * 2)
            ctx.fill()
        }

        var hero = root.meshNodes[0]
        var heroX = hero.x * root.width
        var heroY = hero.y * root.height
        ctx.fillStyle = Qt.rgba(accent.r, accent.g, accent.b, 0.10 + root.pulsePhase * 0.10)
        ctx.beginPath()
        ctx.arc(heroX, heroY, 9 + root.pulsePhase * 5, 0, Math.PI * 2)
        ctx.fill()
        ctx.fillStyle = Qt.rgba(accent.r, accent.g, accent.b, 0.85)
        ctx.beginPath()
        ctx.arc(heroX, heroY, 3, 0, Math.PI * 2)
        ctx.fill()
    }
}
