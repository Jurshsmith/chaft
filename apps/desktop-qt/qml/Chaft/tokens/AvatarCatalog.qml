pragma Singleton
import QtQuick

QtObject {
    readonly property int geometryCount: 16
    readonly property int positionCount: 8
    readonly property int paletteCount: 12
    readonly property int totalCount: geometryCount * positionCount * paletteCount

    readonly property var geometryNames: [
        "Relay", "Switchback", "Chevron", "Delta",
        "Circuit", "Orbit", "Ladder", "Fork",
        "Crossing", "Wave", "Spiral", "Constellation",
        "Bridge", "Beacon", "Mesh", "Pulse"
    ]
    readonly property var positionNames: [
        "North", "Northeast", "East", "Southeast",
        "South", "Southwest", "West", "Northwest"
    ]
    readonly property var palettes: [
        { name: "Signal Teal", background: "#073B3A", route: "#5EEAD4", node: "#ECFDF5", glow: "#14B8A6" },
        { name: "Relay Blue", background: "#142B52", route: "#93C5FD", node: "#EFF6FF", glow: "#3B82F6" },
        { name: "Orbit Violet", background: "#35205C", route: "#C4B5FD", node: "#F5F3FF", glow: "#8B5CF6" },
        { name: "Pulse Rose", background: "#5A1735", route: "#FDA4AF", node: "#FFF1F2", glow: "#F43F5E" },
        { name: "Ember", background: "#57220C", route: "#FDBA74", node: "#FFF7ED", glow: "#F97316" },
        { name: "Beacon Gold", background: "#4A3505", route: "#FDE68A", node: "#FFFBEB", glow: "#EAB308" },
        { name: "Canopy", background: "#123D25", route: "#86EFAC", node: "#F0FDF4", glow: "#22C55E" },
        { name: "Current", background: "#073B4C", route: "#67E8F9", node: "#ECFEFF", glow: "#06B6D4" },
        { name: "Indigo Link", background: "#252B66", route: "#A5B4FC", node: "#EEF2FF", glow: "#6366F1" },
        { name: "Magenta Mesh", background: "#541252", route: "#F0ABFC", node: "#FDF4FF", glow: "#D946EF" },
        { name: "Copper Wire", background: "#4A251D", route: "#FDBA9A", node: "#FFF7ED", glow: "#EA580C" },
        { name: "Slate Signal", background: "#263344", route: "#CBD5E1", node: "#F8FAFC", glow: "#64748B" }
    ]

    function pad2(value) {
        var number = Math.max(0, Number(value || 0))
        return number < 10 ? "0" + String(number) : String(number)
    }

    function avatarId(geometryIndex, positionIndex, paletteIndex) {
        var geometry = Math.max(0, Math.min(geometryCount - 1, Number(geometryIndex || 0)))
        var position = Math.max(0, Math.min(positionCount - 1, Number(positionIndex || 0)))
        var palette = Math.max(0, Math.min(paletteCount - 1, Number(paletteIndex || 0)))
        return "relay-v1:g" + pad2(geometry)
            + ":p" + pad2(position)
            + ":c" + pad2(palette)
    }

    function parse(avatarValue) {
        var value = String(avatarValue || "").trim()
        var match = /^relay-v1:g(\d{2}):p(\d{2}):c(\d{2})$/.exec(value)
        if (match === null) {
            return null
        }
        var geometry = Number(match[1])
        var position = Number(match[2])
        var palette = Number(match[3])
        if (geometry < 0 || geometry >= geometryCount
                || position < 0 || position >= positionCount
                || palette < 0 || palette >= paletteCount) {
            return null
        }
        return {
            id: avatarId(geometry, position, palette),
            geometry: geometry,
            position: position,
            palette: palette
        }
    }

    function isValid(avatarValue) {
        return parse(avatarValue) !== null
    }

    function unsignedHash(value) {
        var text = String(value || "")
        var hash = 2166136261
        for (var i = 0; i < text.length; i += 1) {
            hash ^= text.charCodeAt(i)
            hash = Math.imul(hash, 16777619)
        }
        return hash >>> 0
    }

    function deterministicAvatarId(workspaceId, identityId) {
        var workspace = String(workspaceId || "").trim()
        var identity = String(identityId || "").trim()
        var seed = workspace + "\u241f" + identity
        if (workspace.length === 0 && identity.length === 0) {
            return ""
        }
        return avatarId(
            unsignedHash(seed + ":geometry") % geometryCount,
            unsignedHash(seed + ":position") % positionCount,
            unsignedHash(seed + ":palette") % paletteCount)
    }

    function resolvedAvatarId(avatarValue, workspaceId, identityId) {
        var parsed = parse(avatarValue)
        if (parsed !== null) {
            return parsed.id
        }
        if (String(avatarValue || "").trim().length > 0) {
            return ""
        }
        return deterministicAvatarId(workspaceId, identityId)
    }

    function randomAvatarId() {
        return avatarId(
            Math.floor(Math.random() * geometryCount),
            Math.floor(Math.random() * positionCount),
            Math.floor(Math.random() * paletteCount))
    }

    function paletteFor(avatarValue) {
        var parsed = parse(avatarValue)
        return parsed === null ? null : palettes[parsed.palette]
    }

    function withPalette(avatarValue, paletteIndex) {
        var parsed = parse(avatarValue)
        if (parsed === null) {
            return avatarId(0, 0, paletteIndex)
        }
        return avatarId(parsed.geometry, parsed.position, paletteIndex)
    }

    function initials(displayName) {
        var value = String(displayName || "").trim()
        if (value.length === 0) {
            return "?"
        }
        var words = value.split(/[\s_-]+/).filter(function(word) {
            return word.length > 0
        })
        if (words.length > 1) {
            return (words[0].slice(0, 1) + words[1].slice(0, 1)).toUpperCase()
        }
        return value.slice(0, 2).toUpperCase()
    }

    function description(avatarValue) {
        var parsed = parse(avatarValue)
        if (parsed === null) {
            return "Initials avatar"
        }
        return geometryNames[parsed.geometry] + " "
            + positionNames[parsed.position] + ", "
            + palettes[parsed.palette].name
    }

    function normalizedUsedMap(usedAvatarIds, currentAvatarId) {
        var used = ({})
        var current = String(currentAvatarId || "")
        var rows = usedAvatarIds || []
        for (var i = 0; i < rows.length; i += 1) {
            var parsed = parse(rows[i])
            if (parsed !== null && parsed.id !== current) {
                used[parsed.id] = true
            }
        }
        return used
    }

    function choices(paletteIndex, usedAvatarIds, currentAvatarId, limit) {
        var palette = Math.max(0, Math.min(paletteCount - 1, Number(paletteIndex || 0)))
        var used = normalizedUsedMap(usedAvatarIds, currentAvatarId)
        var available = []
        var occupied = []
        var start = unsignedHash(String(currentAvatarId || "") + ":" + palette) % (geometryCount * positionCount)
        for (var offset = 0; offset < geometryCount * positionCount; offset += 1) {
            var flatIndex = (start + offset) % (geometryCount * positionCount)
            var geometry = Math.floor(flatIndex / positionCount)
            var position = flatIndex % positionCount
            var id = avatarId(geometry, position, palette)
            var choice = {
                avatarId: id,
                used: used[id] === true,
                label: description(id)
            }
            if (choice.used) {
                occupied.push(choice)
            } else {
                available.push(choice)
            }
        }
        var rows = available.concat(occupied)
        var maximum = Number(limit || 0)
        return maximum > 0 ? rows.slice(0, maximum) : rows
    }

    function shuffledAvatarId(currentAvatarId, usedAvatarIds) {
        var used = normalizedUsedMap(usedAvatarIds, currentAvatarId)
        for (var attempt = 0; attempt < 64; attempt += 1) {
            var candidate = randomAvatarId()
            if (candidate !== currentAvatarId && used[candidate] !== true) {
                return candidate
            }
        }
        return randomAvatarId()
    }
}
