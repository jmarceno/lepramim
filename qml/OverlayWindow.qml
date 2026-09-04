import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import app.lepramim

Window {
    id: root
    property var controller

    width: 520
    height: 88
    visible: controller.overlayVisible
    color: "transparent"
    title: "Lepramim overlay"
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint | Qt.Tool

    onClosing: (close) => {
        close.accepted = false
    }

    Rectangle {
        anchors.fill: parent
        anchors.margins: 4
        radius: 16
        color: "#1a1d22d9"
        border.width: 1
        border.color: Theme.borderSubtle

        RowLayout {
            anchors.fill: parent
            anchors.margins: 14
            spacing: 12

            StatusDot {
                Layout.alignment: Qt.AlignVCenter
                dotColor: Theme.statusGreen
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Label {
                    text: root.controller.playbackStatusLabel
                    color: Theme.textPrimary
                    font.pixelSize: 12
                    font.bold: true
                }
                Label {
                    Layout.fillWidth: true
                    text: root.controller.currentSentence.length > 0
                          ? root.controller.currentSentence
                          : "Preparing…"
                    color: Theme.textSecondary
                    font.pixelSize: 13
                    elide: Text.ElideRight
                }
            }

            Row {
                spacing: 6
                Repeater {
                    model: [
                        { glyph: "⏮", action: "back" },
                        { glyph: root.controller.playbackPaused ? "▶" : "⏸", action: "toggle" },
                        { glyph: "⏭", action: "skip" },
                        { glyph: "⏹", action: "stop" }
                    ]
                    delegate: Rectangle {
                        required property var modelData
                        width: 34
                        height: 34
                        radius: 8
                        color: btn.containsMouse ? Theme.accentSoft : Theme.cardBgRaised
                        Label {
                            anchors.centerIn: parent
                            text: modelData.glyph
                            color: Theme.textPrimary
                            font.pixelSize: 14
                        }
                        MouseArea {
                            id: btn
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                if (modelData.action === "back")
                                    root.controller.overlayBack()
                                else if (modelData.action === "toggle")
                                    root.controller.overlayToggle()
                                else if (modelData.action === "skip")
                                    root.controller.overlaySkip()
                                else
                                    root.controller.overlayStop()
                            }
                        }
                    }
                }
            }
        }

        MouseArea {
            anchors.fill: parent
            z: -1
            acceptedButtons: Qt.LeftButton
            onPressed: root.startSystemMove()
        }
    }
}
