import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Window
import app.lepramim 1.0

// Fallback overlay for X11 (and Wayland compositors without layer-shell).
//
// On Wayland the app draws the overlay itself on a zwlr_layer_shell_v1
// surface (src/ui/overlay_wayland.rs): ordinary toplevel windows cannot pick
// their own position there, so this window would end up wherever the
// compositor felt like placing it. This window is the small, bottom-center
// equivalent used where explicit positioning still works.
Window {
    id: root
    readonly property LepramimTheme theme: LepramimTheme {}
    required property var controller

    readonly property bool processing: root.controller.speech_processing
    readonly property bool paused: root.controller.playback_paused
    readonly property string statusText: root.processing
                                         ? "Processing"
                                         : (root.paused ? "Paused" : "Speaking")
    readonly property string phraseText: root.processing
                                         ? "Preparing audio…"
                                         : (root.controller.current_sentence.length > 0
                                            ? root.controller.current_sentence
                                            : "…")

    width: 360
    height: 34
    visible: root.controller.overlay_visible && !root.controller.overlay_native
    color: "transparent"
    title: "Lepramim overlay"
    flags: Qt.Window | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
           | Qt.Tool | Qt.WindowDoesNotAcceptFocus | Qt.WindowTransparentForInput

    // Window.x/y use virtual-desktop coordinates, so add the screen offset on
    // multi-monitor setups (the monitors may not start at 0,0).
    x: Math.round(Screen.virtualX + (Screen.width - width) / 2)
    y: Math.round(Screen.virtualY + Screen.height - height - 84)

    Rectangle {
        anchors.fill: parent
        radius: 17
        color: Qt.rgba(0.102, 0.114, 0.133, 0.94)
        border.width: 1
        border.color: root.theme.borderSubtle

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            spacing: 8

            Rectangle {
                id: dot
                Layout.alignment: Qt.AlignVCenter
                width: 8
                height: 8
                radius: 4
                color: root.processing ? root.theme.warning
                                      : (root.paused ? root.theme.textMuted
                                                     : root.theme.statusGreen)
                SequentialAnimation on opacity {
                    running: root.visible
                    loops: Animation.Infinite
                    NumberAnimation {
                        from: 0.35; to: 1.0; duration: 700
                        easing.type: Easing.InOutSine
                    }
                    NumberAnimation {
                        from: 1.0; to: 0.35; duration: 700
                        easing.type: Easing.InOutSine
                    }
                }
            }

            Label {
                Layout.alignment: Qt.AlignVCenter
                Layout.preferredWidth: 66
                text: root.statusText
                color: root.theme.textSecondary
                font.pixelSize: 11
                font.weight: Font.DemiBold
                elide: Text.ElideRight
            }

            // Clipped marquee: pauses at both ends, then scrolls the phrase
            // once so long sentences stay readable in the tiny pill.
            Item {
                id: clip
                Layout.fillWidth: true
                Layout.preferredHeight: 18
                clip: true
                readonly property real overflow: Math.max(0, phrase.implicitWidth - width)

                Text {
                    id: phrase
                    anchors.verticalCenter: parent.verticalCenter
                    x: 0
                    text: root.phraseText
                    color: "#b9bfc8"
                    font.pixelSize: 12
                    wrapMode: Text.NoWrap
                    onTextChanged: {
                        x = 0
                        if (marquee.running)
                            marquee.restart()
                    }
                }

                SequentialAnimation {
                    id: marquee
                    running: root.visible && !root.processing && !root.paused
                             && clip.overflow > 1
                    loops: Animation.Infinite
                    PauseAnimation { duration: 1100 }
                    NumberAnimation {
                        target: phrase
                        property: "x"
                        from: 0
                        to: -clip.overflow
                        duration: Math.max(900, clip.overflow * 33)
                        easing.type: Easing.Linear
                    }
                    PauseAnimation { duration: 1100 }
                    PropertyAction { target: phrase; property: "x"; value: 0 }
                }

                onOverflowChanged: {
                    phrase.x = 0
                    if (marquee.running)
                        marquee.restart()
                }
            }
        }
    }
}
