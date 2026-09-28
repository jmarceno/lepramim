import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import app.lepramim 1.0

Window {
    id: root
    readonly property LepramimTheme theme: LepramimTheme {}
    required property var controller

    width: 440
    height: 240
    visible: controller.onboarding_visible
    color: theme.windowBg
    title: "Welcome to Lepramim"
    flags: Qt.Dialog

    onClosing: (close) => {
        close.accepted = false
        controller.skipOnboarding()
    }

    Rectangle {
        anchors.fill: parent
        color: theme.windowBg

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 14

            Label {
                text: "Welcome to Lepramim"
                color: theme.textPrimary
                font.pixelSize: 22
                font.bold: true
            }
            Label {
                Layout.fillWidth: true
                text: root.controller.download_status.length > 0
                      ? root.controller.download_status
                      : "Download the Kokoro speech model to start reading highlighted text aloud."
                color: theme.textSecondary
                font.pixelSize: 13
                wrapMode: Text.WordWrap
            }
            ProgressBar {
                Layout.fillWidth: true
                from: 0
                to: 100
                value: root.controller.download_percent
                background: Rectangle {
                    implicitHeight: 8
                    radius: 4
                    color: theme.sliderTrack
                }
                contentItem: Item {
                    implicitHeight: 8
                    Rectangle {
                        width: parent.width * parent.parent.visualPosition
                        height: parent.height
                        radius: 4
                        color: theme.accent
                    }
                }
            }
            Label {
                Layout.fillWidth: true
                text: root.controller.download_filename
                color: theme.textMuted
                font.pixelSize: 12
                elide: Text.ElideMiddle
            }
            Item { Layout.fillHeight: true }
            RowLayout {
                Layout.fillWidth: true
                Item { Layout.fillWidth: true }
                TealButton {
                    text: "Skip"
                    primary: false
                    onClicked: root.controller.skipOnboarding()
                }
                TealButton {
                    text: "Continue"
                    onClicked: root.controller.continueOnboarding()
                }
            }
        }
    }
}
