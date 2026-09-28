import QtQuick
import QtQuick.Controls
import app.lepramim 1.0

Switch {
    id: root
    readonly property LepramimTheme theme: LepramimTheme {}

    indicator: Rectangle {
        implicitWidth: 44
        implicitHeight: 24
        x: root.leftPadding
        y: parent.height / 2 - height / 2
        radius: 12
        color: root.checked ? theme.accent : theme.sliderTrack
        border.width: root.checked ? 0 : 1
        border.color: theme.borderSubtle

        Rectangle {
            x: root.checked ? parent.width - width - 3 : 3
            y: 3
            width: 18
            height: 18
            radius: 9
            color: "#ffffff"
            Behavior on x { NumberAnimation { duration: 120; easing.type: Easing.OutCubic } }
        }
    }
}
