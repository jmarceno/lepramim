import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import app.lepramim

Item {
    id: root
    property var controller

    RowLayout {
        anchors.fill: parent
        spacing: 16

        // Left: Voice & language
        Card {
            Layout.fillWidth: true
            Layout.fillHeight: true
            title: "Voice & language"

            ColumnLayout {
                width: parent.width
                spacing: 14

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12

                    LabeledCombo {
                        Layout.fillWidth: true
                        label: "VOICE"
                        model: root.controller.voiceLabels
                        currentIndex: Math.max(0, root.controller.voiceIndex)
                        onActivated: (index) => root.controller.selectVoiceAt(index)
                    }

                    LabeledCombo {
                        Layout.fillWidth: true
                        label: "LANGUAGE"
                        model: root.controller.languageLabels
                        currentIndex: Math.max(0, root.controller.languageIndex)
                        onActivated: (index) => root.controller.selectLanguageAt(index)
                    }
                }

                CheckBox {
                    id: filterBox
                    text: "Filter voices by language"
                    checked: root.controller.filterVoicesByLang
                    onToggled: root.controller.applyFilterVoicesByLang(checked)

                    indicator: Rectangle {
                        implicitWidth: 18
                        implicitHeight: 18
                        x: filterBox.leftPadding
                        y: parent.height / 2 - height / 2
                        radius: 4
                        color: filterBox.checked ? Theme.accent : Theme.inputBg
                        border.color: filterBox.checked ? Theme.accent : Theme.borderSubtle
                        border.width: 1
                        Label {
                            anchors.centerIn: parent
                            text: "✓"
                            color: "#0d1f1c"
                            font.pixelSize: 12
                            font.bold: true
                            visible: filterBox.checked
                        }
                    }
                    contentItem: Label {
                        text: filterBox.text
                        color: Theme.textSecondary
                        font.pixelSize: 13
                        leftPadding: filterBox.indicator.width + 10
                        verticalAlignment: Text.AlignVCenter
                    }
                }

                TealSlider {
                    Layout.fillWidth: true
                    value: root.controller.speed
                    from: 0.5
                    to: 2.0
                    hint: "Recommended for dense reading: 0.85× – 1.30×"
                    onValueChanged: {
                        if (Math.abs(value - root.controller.speed) > 0.001)
                            root.controller.applySpeed(value)
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 110
                    radius: Theme.radiusSm
                    color: Theme.inputBg
                    border.width: 1
                    border.color: Theme.borderSubtle

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 12
                        spacing: 10

                        Label {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            text: "The quick brown fox jumps over the lazy dog."
                            color: Theme.textSecondary
                            wrapMode: Text.WordWrap
                            font.pixelSize: 13
                        }

                        TealButton {
                            text: "▶  Test voice"
                            compact: true
                            Layout.alignment: Qt.AlignLeft
                            onClicked: root.controller.testVoice()
                        }
                    }
                }
            }
        }

        // Right column
        ColumnLayout {
            Layout.preferredWidth: 280
            Layout.fillHeight: true
            spacing: 16

            Card {
                Layout.fillWidth: true
                title: "Playback"
                contentSpacing: 12

                ColumnLayout {
                    width: parent.width
                    spacing: 12

                    RowLayout {
                        spacing: 8
                        StatusDot {
                            Layout.alignment: Qt.AlignVCenter
                            dotColor: root.controller.playbackActive ? Theme.statusGreen : Theme.statusGreen
                        }
                        ColumnLayout {
                            spacing: 2
                            Label {
                                text: root.controller.playbackStatusLabel
                                color: Theme.textPrimary
                                font.pixelSize: 14
                                font.bold: true
                            }
                            Label {
                                text: root.controller.playbackStatusDetail
                                color: Theme.textMuted
                                font.pixelSize: 12
                                wrapMode: Text.WordWrap
                                Layout.fillWidth: true
                            }
                        }
                    }

                    ColumnLayout {
                        spacing: 6
                        Label {
                            text: "ACCELERATION"
                            color: Theme.textDim
                            font.pixelSize: 11
                            font.bold: true
                            font.letterSpacing: 0.8
                        }
                        RowLayout {
                            spacing: 8
                            Rectangle {
                                radius: 6
                                color: Theme.inputBg
                                border.width: 1
                                border.color: Theme.borderSubtle
                                implicitWidth: accelRow.implicitWidth + 14
                                implicitHeight: 26
                                RowLayout {
                                    id: accelRow
                                    anchors.centerIn: parent
                                    spacing: 6
                                    StatusDot {
                                        width: 7
                                        height: 7
                                        radius: 3.5
                                        dotColor: Theme.statusGreen
                                    }
                                    Label {
                                        text: root.controller.accelerationLabel
                                        color: Theme.textPrimary
                                        font.pixelSize: 12
                                        font.bold: true
                                    }
                                }
                            }
                            Label {
                                text: "Local inference"
                                color: Theme.textMuted
                                font.pixelSize: 12
                            }
                        }
                    }
                }
            }

            Card {
                Layout.fillWidth: true
                Layout.fillHeight: true
                title: "Floating overlay"
                contentSpacing: 12

                ColumnLayout {
                    width: parent.width
                    spacing: 12

                    RowLayout {
                        Layout.fillWidth: true
                        Label {
                            text: "Optional always-on-top playback controls."
                            color: Theme.textMuted
                            font.pixelSize: 12
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                        TealSwitch {
                            checked: root.controller.overlayEnabled
                            onToggled: root.controller.applyOverlayEnabled(checked)
                        }
                    }

                    Rectangle {
                        Layout.fillWidth: true
                        Layout.preferredHeight: 96
                        radius: Theme.radiusSm
                        color: Theme.inputBg
                        border.width: 1
                        border.color: Theme.borderSubtle

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: 12
                            spacing: 8

                            RowLayout {
                                spacing: 6
                                StatusDot { Layout.alignment: Qt.AlignVCenter }
                                Label {
                                    text: "Speaking"
                                    color: Theme.textPrimary
                                    font.pixelSize: 12
                                    font.bold: true
                                }
                            }
                            Label {
                                Layout.fillWidth: true
                                text: "Current sentence appears here while reading…"
                                color: Theme.textMuted
                                font.pixelSize: 12
                                elide: Text.ElideRight
                            }
                            RowLayout {
                                spacing: 14
                                Label { text: "⏮"; color: Theme.textSecondary; font.pixelSize: 14 }
                                Label { text: "⏸"; color: Theme.textSecondary; font.pixelSize: 14 }
                                Label { text: "⏭"; color: Theme.textSecondary; font.pixelSize: 14 }
                                Label { text: "⏹"; color: Theme.textSecondary; font.pixelSize: 14 }
                                Item { Layout.fillWidth: true }
                            }
                        }
                    }
                }
            }
        }
    }
}
