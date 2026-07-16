import QtQuick
import Chaft

Rectangle {
    id: root

    property string avatarId: ""
    property string workspaceId: ""
    property string identityId: ""
    property string displayName: ""
    property bool warning: false
    readonly property string resolvedAvatarId: warning
        ? ""
        : AvatarCatalog.resolvedAvatarId(avatarId, workspaceId, identityId)
    readonly property var avatarParts: AvatarCatalog.parse(resolvedAvatarId)
    readonly property var avatarPalette: avatarParts === null
        ? null
        : AvatarCatalog.palettes[avatarParts.palette]
    readonly property bool relaymarkVisible: avatarParts !== null && !warning
    readonly property color routeColor: avatarPalette === null
        ? "transparent"
        : avatarPalette.route
    readonly property color glowColor: avatarPalette === null
        ? "transparent"
        : avatarPalette.glow

    implicitWidth: 36
    implicitHeight: 36
    radius: Math.max(Tokens.radiusSm, Math.min(width, height) * 0.27)
    color: warning
        ? Tokens.warning
        : (avatarPalette === null ? Tokens.surfaceRaised : avatarPalette.background)
    border.width: 1
    border.color: warning
        ? Tokens.warning
        : (avatarPalette === null
            ? Tokens.borderSubtle
            : Qt.rgba(routeColor.r, routeColor.g, routeColor.b, 0.34))
    clip: true

    Accessible.role: Accessible.Graphic
    Accessible.name: warning
        ? "Timeline warning"
        : displayName.length > 0
            ? displayName + " avatar, " + AvatarCatalog.description(resolvedAvatarId)
            : AvatarCatalog.description(resolvedAvatarId)

    Canvas {
        id: canvas
        anchors.fill: parent
        visible: root.relaymarkVisible
        antialiasing: true

        function point(x, y, angle, mirror) {
            var dx = (mirror ? 1 - x : x) - 0.5
            var dy = y - 0.5
            var cosine = Math.cos(angle)
            var sine = Math.sin(angle)
            return [
                0.5 + dx * cosine - dy * sine,
                0.5 + dx * sine + dy * cosine
            ]
        }

        function geometryPoints(index) {
            switch (index) {
            case 0: return [[0.2, 0.72], [0.38, 0.54], [0.58, 0.54], [0.8, 0.28]]
            case 1: return [[0.18, 0.68], [0.38, 0.68], [0.32, 0.42], [0.64, 0.42], [0.78, 0.22]]
            case 2: return [[0.2, 0.65], [0.5, 0.32], [0.8, 0.65]]
            case 3: return [[0.22, 0.7], [0.5, 0.2], [0.78, 0.7], [0.22, 0.7]]
            case 4: return [[0.24, 0.7], [0.24, 0.3], [0.7, 0.3], [0.7, 0.7], [0.46, 0.7]]
            case 5: return [[0.18, 0.55], [0.32, 0.28], [0.66, 0.24], [0.82, 0.52], [0.62, 0.75], [0.3, 0.72]]
            case 6: return [[0.18, 0.7], [0.38, 0.7], [0.38, 0.28], [0.62, 0.28], [0.62, 0.7], [0.82, 0.7]]
            case 7: return [[0.2, 0.7], [0.48, 0.48], [0.48, 0.22], [0.48, 0.48], [0.8, 0.62]]
            case 8: return [[0.18, 0.5], [0.82, 0.5], [0.5, 0.5], [0.5, 0.18], [0.5, 0.82]]
            case 9: return [[0.16, 0.62], [0.32, 0.36], [0.5, 0.62], [0.68, 0.36], [0.84, 0.62]]
            case 10: return [[0.18, 0.7], [0.7, 0.7], [0.7, 0.28], [0.34, 0.28], [0.34, 0.52], [0.54, 0.52]]
            case 11: return [[0.18, 0.68], [0.36, 0.32], [0.5, 0.62], [0.64, 0.24], [0.82, 0.54]]
            case 12: return [[0.16, 0.62], [0.38, 0.62], [0.5, 0.36], [0.62, 0.62], [0.84, 0.62]]
            case 13: return [[0.2, 0.7], [0.5, 0.2], [0.8, 0.7], [0.5, 0.54], [0.2, 0.7]]
            case 14: return [[0.18, 0.7], [0.36, 0.48], [0.52, 0.66], [0.68, 0.36], [0.84, 0.54]]
            default: return [[0.18, 0.58], [0.34, 0.58], [0.42, 0.32], [0.56, 0.72], [0.66, 0.42], [0.84, 0.42]]
            }
        }

        function paintRelaymark() {
            var context = getContext("2d")
            context.clearRect(0, 0, width, height)
            if (root.avatarParts === null || root.avatarPalette === null) {
                return
            }

            var size = Math.min(width, height)
            var xOffset = (width - size) / 2
            var yOffset = (height - size) / 2
            var angle = root.avatarParts.position * Math.PI / 4
            var mirror = root.avatarParts.position % 2 === 1
            var points = geometryPoints(root.avatarParts.geometry)

            context.save()
            context.translate(xOffset, yOffset)
            context.strokeStyle = Qt.rgba(
                root.glowColor.r, root.glowColor.g,
                root.glowColor.b, 0.24)
            context.lineWidth = Math.max(1, size * 0.055)
            context.beginPath()
            context.arc(size * 0.5, size * 0.5, size * 0.32, 0, Math.PI * 2)
            context.stroke()

            context.strokeStyle = root.avatarPalette.route
            context.lineWidth = Math.max(1.5, size * 0.082)
            context.lineCap = "round"
            context.lineJoin = "round"
            context.beginPath()
            for (var i = 0; i < points.length; i += 1) {
                var transformed = point(points[i][0], points[i][1], angle, mirror)
                var px = transformed[0] * size
                var py = transformed[1] * size
                if (i === 0) {
                    context.moveTo(px, py)
                } else {
                    context.lineTo(px, py)
                }
            }
            context.stroke()

            var nodeIndexes = [0, Math.floor(points.length / 2), points.length - 1]
            for (var node = 0; node < nodeIndexes.length; node += 1) {
                var nodePoint = points[nodeIndexes[node]]
                var placed = point(nodePoint[0], nodePoint[1], angle, mirror)
                context.beginPath()
                context.fillStyle = node === 1
                    ? root.avatarPalette.node
                    : root.avatarPalette.route
                context.arc(placed[0] * size, placed[1] * size,
                            Math.max(1.7, size * (node === 1 ? 0.09 : 0.065)),
                            0, Math.PI * 2)
                context.fill()
            }
            context.restore()
        }

        onPaint: paintRelaymark()
        onWidthChanged: requestPaint()
        onHeightChanged: requestPaint()
    }

    Text {
        anchors.centerIn: parent
        visible: !root.relaymarkVisible
        text: root.warning ? "!" : AvatarCatalog.initials(root.displayName)
        color: root.warning ? Tokens.onAccent : Tokens.textStrong
        font.pixelSize: Math.max(Tokens.fontSizeXs, Math.min(root.width, root.height) * 0.34)
        font.weight: Font.DemiBold
    }

    onResolvedAvatarIdChanged: canvas.requestPaint()
    onAvatarPaletteChanged: canvas.requestPaint()
}
