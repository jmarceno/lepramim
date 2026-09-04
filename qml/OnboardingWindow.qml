import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import app.lepramim

Window {
    id: root
    property var controller

    width: 440
    height: 240
    visible: controller.onboardingVisible
    color: Theme.windowBg
    title: "Welcome to Lepramim"
    flags: Qt.Dialog

    onClosing: (close) => {
        close.accepted = false
        controller.skipOnboarding()
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.windowBg

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 24
            spacing: 14

            Label {
                text: "Welcome to Lepramim"
                color: Theme.textPrimary
                font.pixelSize: 22
                font.bold: true
            }
            Label {
                Layout.fillWidth: true
                text: root.controller.downloadStatus.length > 0
                      ? root.controller.downloadStatus
                      : "Download the Kokoro speech model to start reading highlighted text aloud."
                color: Theme.textSecondary
                font.pixelSize: 13
                wrapMode: Text.WordWrap
            }
            ProgressBar {
                Layout.fillWidth: true
                from: 0
                to: 100
                value: root.controller.downloadPercent
                background: Rectangle {
                    implicitHeight: 8
                    radius: 4
                    color: Theme.sliderTrack
                }
                contentItem: Item {
                    implicitHeight: 8
                    Rectangle {
                        width: parent.width * parent.parent.visualPosition
                        height: parent.height
                        radius: 4
                        color: Theme.accent
                    }
                }
            }
            Label {
                Layout.fillWidth: true
                text: root.controller.downloadFilename
                color: Theme.textMuted
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
