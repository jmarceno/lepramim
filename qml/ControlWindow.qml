import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import app.lepramim

Window {
    id: root
    required property var controller

    width: 1120
    height: 720
    minimumWidth: 960
    minimumHeight: 640
    visible: controller.control_visible
    color: Theme.windowBg
    title: "Lepramim"
    flags: Qt.Window | Qt.FramelessWindowHint

    onClosing: (close) => {
        close.accepted = false
        controller.hideControl()
    }

    Rectangle {
        anchors.fill: parent
        color: Theme.windowBg
        radius: 0

        ColumnLayout {
            anchors.fill: parent
            spacing: 0

            // Custom title bar
            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 44
                color: Theme.windowBg

                MouseArea {
                    anchors.fill: parent
                    acceptedButtons: Qt.LeftButton
                    property point start
                    onPressed: (mouse) => {
                        start = Qt.point(mouse.x, mouse.y)
                    }
                    onPositionChanged: (mouse) => {
                        if (pressed) {
                            root.startSystemMove()
                        }
                    }
                    onDoubleClicked: {
                        if (root.visibility === Window.Maximized)
                            root.showNormal()
                        else
                            root.showMaximized()
                    }
                }

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 14
                    anchors.rightMargin: 10
                    spacing: 10

                    Image {
                        source: "qrc:/qt/qml/app/lepramim/src/lepramim/icons/lepramim.svg"
                        sourceSize.width: 22
                        sourceSize.height: 22
                        Layout.preferredWidth: 22
                        Layout.preferredHeight: 22
                        fillMode: Image.PreserveAspectFit
                    }

                    Label {
                        text: "Lepramim"
                        color: Theme.textPrimary
                        font.pixelSize: 15
                        font.bold: true
                        Layout.fillWidth: true
                    }

                    Row {
                        spacing: 4
                        Repeater {
                            model: [
                                { glyph: "—", action: "min" },
                                { glyph: "□", action: "max" },
                                { glyph: "✕", action: "close" }
                            ]
                            delegate: Rectangle {
                                required property var modelData
                                width: 32
                                height: 28
                                radius: 6
                                color: winBtn.containsMouse
                                       ? (modelData.action === "close" ? "#e35d6a" : Theme.cardBgRaised)
                                       : "transparent"
                                Label {
                                    anchors.centerIn: parent
                                    text: modelData.glyph
                                    color: Theme.textSecondary
                                    font.pixelSize: 12
                                }
                                MouseArea {
                                    id: winBtn
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: {
                                        if (modelData.action === "min")
                                            root.showMinimized()
                                        else if (modelData.action === "max") {
                                            if (root.visibility === Window.Maximized)
                                                root.showNormal()
                                            else
                                                root.showMaximized()
                                        } else {
                                            controller.hideControl()
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            RowLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 0

                Sidebar {
                    controller: root.controller
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    color: Theme.windowBg

                    ColumnLayout {
                        anchors.fill: parent
                        anchors.margins: 24
                        spacing: 18

                        RowLayout {
                            Layout.fillWidth: true
                            ColumnLayout {
                                spacing: 4
                                Layout.fillWidth: true
                                Label {
                                    text: root.controller.page_title
                                    color: Theme.textPrimary
                                    font.pixelSize: 28
                                    font.bold: true
                                }
                                Label {
                                    text: root.controller.page_subtitle
                                    color: Theme.textMuted
                                    font.pixelSize: 13
                                    wrapMode: Text.WordWrap
                                    Layout.fillWidth: true
                                }
                            }
                            TealButton {
                                visible: root.controller.control_tab === 0
                                text: "▶  Read selection"
                                onClicked: root.controller.readSelection()
                            }
                        }

                        StackLayout {
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            currentIndex: root.controller.control_tab

                            VoicePage { controller: root.controller }
                            PreprocessorPage { controller: root.controller }
                            AdvancedPage { controller: root.controller }
                            ModelsPage { controller: root.controller }
                        }

                        RowLayout {
                            Layout.fillWidth: true
                            Label {
                                text: "Changes affect the next playback session."
                                color: Theme.textMuted
                                font.pixelSize: 12
                                Layout.fillWidth: true
                            }
                            Label {
                                visible: root.controller.status_message.length > 0
                                text: root.controller.status_message
                                color: Theme.textSecondary
                                font.pixelSize: 12
                                elide: Text.ElideRight
                                Layout.maximumWidth: 360
                            }
                            TealButton {
                                text: "Apply settings"
                                onClicked: root.controller.applySettings()
                            }
                        }
                    }
                }
            }
        }
    }
}
