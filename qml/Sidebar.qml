import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import app.lepramim 1.0

Item {
    id: root
    readonly property LepramimTheme theme: LepramimTheme {}
    required property var controller

    width: 236
    Layout.fillHeight: true
    Layout.preferredWidth: 236

    Rectangle {
        anchors.fill: parent
        color: theme.windowBg

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 18
            spacing: 18

            ColumnLayout {
                spacing: 4
                Label {
                    text: "ENGINE"
                    color: theme.textDim
                    font.pixelSize: 11
                    font.bold: true
                    font.letterSpacing: 1.0
                }
                RowLayout {
                    spacing: 8
                    StatusDot {
                        Layout.alignment: Qt.AlignVCenter
                        dotColor: root.controller.engine_running ? theme.statusGreen : theme.textDim
                    }
                    ColumnLayout {
                        spacing: 2
                        Label {
                            text: root.controller.engine_running ? "Running" : "Stopped"
                            color: theme.textPrimary
                            font.pixelSize: 14
                            font.bold: true
                        }
                        Label {
                            text: "Local — private"
                            color: theme.textMuted
                            font.pixelSize: 12
                        }
                    }
                }
            }

            ColumnLayout {
                spacing: 4
                Layout.fillWidth: true

                NavItem {
                    Layout.fillWidth: true
                    label: "Voice"
                    iconGlyph: "🕪"
                    selected: root.controller.control_tab === 0
                    onClicked: root.controller.selectTab(0)
                }
                NavItem {
                    Layout.fillWidth: true
                    label: "Preprocessor"
                    iconGlyph: "▽"
                    selected: root.controller.control_tab === 1
                    onClicked: root.controller.selectTab(1)
                }
                NavItem {
                    Layout.fillWidth: true
                    label: "Advanced"
                    iconGlyph: "☰"
                    selected: root.controller.control_tab === 2
                    onClicked: root.controller.selectTab(2)
                }
                NavItem {
                    Layout.fillWidth: true
                    label: "Models"
                    iconGlyph: "▤"
                    selected: root.controller.control_tab === 3
                    onClicked: root.controller.selectTab(3)
                }
            }

            Item { Layout.fillHeight: true }

            Rectangle {
                Layout.fillWidth: true
                radius: theme.radiusSm
                color: theme.cardBg
                implicitHeight: shortcutsCol.implicitHeight + 24

                ColumnLayout {
                    id: shortcutsCol
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.margins: 12
                    spacing: 10

                    Label {
                        text: "GLOBAL SHORTCUTS"
                        color: theme.textDim
                        font.pixelSize: 11
                        font.bold: true
                        font.letterSpacing: 0.8
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        Label {
                            text: "Meta + R"
                            color: theme.textPrimary
                            font.pixelSize: 12
                            font.bold: true
                        }
                        Item { Layout.fillWidth: true }
                        Label {
                            text: "Read selection"
                            color: theme.textMuted
                            font.pixelSize: 12
                        }
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        Label {
                            text: "Meta + P"
                            color: theme.textPrimary
                            font.pixelSize: 12
                            font.bold: true
                        }
                        Item { Layout.fillWidth: true }
                        Label {
                            text: "Pause / resume"
                            color: theme.textMuted
                            font.pixelSize: 12
                        }
                    }
                }
            }
        }
    }
}
