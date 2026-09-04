import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import app.lepramim

Item {
    id: root
    required property var controller

    Card {
        anchors.fill: parent
        title: "Models"

        ColumnLayout {
            width: parent.width
            spacing: 14

            Label {
                Layout.fillWidth: true
                text: root.controller.models_status
                color: Theme.textSecondary
                font.pixelSize: 13
                wrapMode: Text.WordWrap
            }

            RowLayout {
                spacing: 10
                TealButton {
                    text: "Refresh"
                    primary: false
                    compact: true
                    onClicked: root.controller.refreshModels()
                }
                TealButton {
                    text: "Download missing models"
                    compact: true
                    enabled: !root.controller.download_active
                    onClicked: root.controller.startDownload()
                }
            }

            Label {
                visible: root.controller.download_active || root.controller.download_status.length > 0
                Layout.fillWidth: true
                text: root.controller.download_status
                color: Theme.textMuted
                font.pixelSize: 12
                wrapMode: Text.WordWrap
            }

            ProgressBar {
                visible: root.controller.download_active
                Layout.fillWidth: true
                from: 0
                to: 100
                value: root.controller.download_percent

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
        }
    }
}
