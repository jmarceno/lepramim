import QtQuick
import app.lepramim 1.0

Rectangle {
    id: root
    readonly property LepramimTheme theme: LepramimTheme {}
    property color dotColor: theme.statusGreen
    width: 8
    height: 8
    radius: 4
    color: root.dotColor
}
