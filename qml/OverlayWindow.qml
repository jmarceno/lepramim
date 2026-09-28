import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import app.lepramim 1.0

Window {
    id: root
    readonly property LepramimTheme theme: LepramimTheme {}
    required property var controller

    width: 520
    height: 88
    visible: controller.overlay_visible
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
        // Qt 8-digit hex is #AARRGGBB (alpha first).
        color: "#d91a1d22"
        border.width: 1
        border.color: theme.borderSubtle

        RowLayout {
            anchors.fill: parent
            anchors.margins: 14
            spacing: 12

            StatusDot {
                Layout.alignment: Qt.AlignVCenter
                dotColor: theme.statusGreen
            }

            ColumnLayout {
                Layout.fillWidth: true
                spacing: 2
                Label {
                    text: root.controller.playback_status_label
                    color: theme.textPrimary
                    font.pixelSize: 12
                    font.bold: true
                }
                Label {
                    Layout.fillWidth: true
                    text: root.controller.current_sentence.length > 0
                          ? root.controller.current_sentence
                          : "Preparing…"
                    color: theme.textSecondary
                    font.pixelSize: 13
                    elide: Text.ElideRight
                }
            }

            Row {
                spacing: 6
                Repeater {
                    model: [
                        { glyph: "⏮", action: "back" },
                        { glyph: root.controller.playback_paused ? "▶" : "⏸", action: "toggle" },
                        { glyph: "⏭", action: "skip" },
                        { glyph: "⏹", action: "stop" }
                    ]
                    delegate: Rectangle {
                        required property var modelData
                        width: 34
                        height: 34
                        radius: 8
                        color: btn.containsMouse ? theme.accentSoft : theme.cardBgRaised
                        Label {
                            anchors.centerIn: parent
                            text: modelData.glyph
                            color: theme.textPrimary
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
