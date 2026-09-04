import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import app.lepramim

Item {
    id: root
    property var controller

    Card {
        anchors.fill: parent
        title: "Models"

        ColumnLayout {
            width: parent.width
            spacing: 14

            Label {
                Layout.fillWidth: true
                text: root.controller.modelsStatus
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
                    enabled: !root.controller.downloadActive
                    onClicked: root.controller.startDownload()
                }
            }

            Label {
                visible: root.controller.downloadActive || root.controller.downloadStatus.length > 0
                Layout.fillWidth: true
                text: root.controller.downloadStatus
                color: Theme.textMuted
                font.pixelSize: 12
                wrapMode: Text.WordWrap
            }

            ProgressBar {
                visible: root.controller.downloadActive
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
        }
    }
}
