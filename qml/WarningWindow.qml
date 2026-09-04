import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import app.lepramim

Window {
    id: root
    property var controller

    width: 420
    height: 160
    visible: controller.warningVisible
    color: Theme.windowBg
    title: "Lepramim"
    flags: Qt.Dialog

    onClosing: (close) => {
        close.accepted = false
        controller.dismissWarning()
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.windowBg

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 14

            Label {
                text: "Lepramim"
                color: Theme.textPrimary
                font.pixelSize: 18
                font.bold: true
            }
            Label {
                Layout.fillWidth: true
                text: root.controller.warningText
                color: Theme.textSecondary
                font.pixelSize: 13
                wrapMode: Text.WordWrap
            }
            Item { Layout.fillHeight: true }
            TealButton {
                Layout.alignment: Qt.AlignRight
                text: "OK"
                onClicked: root.controller.dismissWarning()
            }
        }
    }
}
