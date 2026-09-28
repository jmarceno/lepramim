import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import app.lepramim 1.0

Window {
    id: root
    readonly property LepramimTheme theme: LepramimTheme {}
    required property var controller

    width: 420
    height: 160
    visible: controller.warning_visible
    color: theme.windowBg
    title: "Lepramim"
    flags: Qt.Dialog

    onClosing: (close) => {
        close.accepted = false
        controller.dismissWarning()
    }

    Rectangle {
        anchors.fill: parent
        color: theme.windowBg

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 14

            Label {
                text: "Lepramim"
                color: theme.textPrimary
                font.pixelSize: 18
                font.bold: true
            }
            Label {
                Layout.fillWidth: true
                text: root.controller.warning_text
                color: theme.textSecondary
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
