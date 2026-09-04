import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import app.lepramim

Item {
    id: root
    property string label: ""
    property string iconGlyph: ""
    property bool selected: false
    signal clicked()

    height: 40
    implicitHeight: 40

    Rectangle {
        anchors.fill: parent
        radius: Theme.radiusSm
        color: root.selected ? Theme.accentSoft : "transparent"

        Rectangle {
            visible: root.selected
            anchors.left: parent.left
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            width: 3
            radius: 1.5
            color: Theme.accent
        }

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 12
            spacing: 12

            Label {
                text: root.iconGlyph
                color: root.selected ? Theme.accent : Theme.textMuted
                font.pixelSize: 15
                Layout.preferredWidth: 18
                horizontalAlignment: Text.AlignHCenter
            }

            Label {
                text: root.label
                color: root.selected ? Theme.accent : Theme.textPrimary
                font.pixelSize: 14
                font.bold: root.selected
                Layout.fillWidth: true
            }
        }
    }

    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
