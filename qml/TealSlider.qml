import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import app.lepramim 1.0

ColumnLayout {
    id: root
    readonly property LepramimTheme theme: LepramimTheme {}
    property alias value: slider.value
    property real from: 0.5
    property real to: 2.0
    property string valueText: value.toFixed(2) + "×"
    property string leftLabel: "0.50×"
    property string rightLabel: "2.00×"
    property string hint: ""

    spacing: 8

    RowLayout {
        Layout.fillWidth: true
        Label {
            text: "READING SPEED"
            color: theme.textDim
            font.pixelSize: 11
            font.bold: true
            font.letterSpacing: 0.8
            Layout.fillWidth: true
        }
        Label {
            text: root.valueText
            color: theme.accent
            font.pixelSize: 14
            font.bold: true
        }
    }

    Slider {
        id: slider
        Layout.fillWidth: true
        from: root.from
        to: root.to
        stepSize: 0.01

        background: Rectangle {
            x: slider.leftPadding
            y: slider.topPadding + slider.availableHeight / 2 - height / 2
            width: slider.availableWidth
            height: 6
            radius: 3
            color: theme.sliderTrack

            Rectangle {
                width: slider.visualPosition * parent.width
                height: parent.height
                radius: 3
                color: theme.accent
            }
        }

        handle: Rectangle {
            x: slider.leftPadding + slider.visualPosition * (slider.availableWidth - width)
            y: slider.topPadding + slider.availableHeight / 2 - height / 2
            width: 18
            height: 18
            radius: 9
            color: "#ffffff"
            border.width: 2
            border.color: theme.accent
        }
    }

    RowLayout {
        Layout.fillWidth: true
        Label {
            text: root.leftLabel
            color: theme.textDim
            font.pixelSize: 11
            Layout.fillWidth: true
        }
        Label {
            text: root.rightLabel
            color: theme.textDim
            font.pixelSize: 11
        }
    }

    Label {
        visible: root.hint.length > 0
        text: root.hint
        color: theme.textMuted
        font.pixelSize: 12
        wrapMode: Text.WordWrap
        Layout.fillWidth: true
    }
}
