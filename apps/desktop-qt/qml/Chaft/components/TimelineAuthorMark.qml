import Chaft

AvatarMark {
    id: root
    property string label: ""
    property string authorDeviceId: ""
    property string authorAvatarId: ""
    property bool encrypted: false

    implicitWidth: 36
    implicitHeight: 36
    avatarId: root.authorAvatarId
    identityId: root.authorDeviceId
    displayName: root.label
}
