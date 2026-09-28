import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import app.lepramim 1.0

ColumnLayout {
    id: root
    readonly property LepramimTheme theme: LepramimTheme {}
    property string label: ""
    property alias model: combo.model
    property alias currentIndex: combo.currentIndex
    property alias currentText: combo.currentText
    property alias displayText: combo.displayText
    signal activated(int index)

    spacing: 6

    Label {
        text: root.label
        color: theme.textDim
        font.pixelSize: 11
        font.bold: true
        font.letterSpacing: 0.8
    }

    ComboBox {
        id: combo
        Layout.fillWidth: true
        Layout.preferredHeight: 40
        font.pixelSize: 14

        background: Rectangle {
            radius: theme.radiusSm
            color: theme.inputBg
            border.width: 1
            border.color: theme.borderSubtle
        }

        contentItem: Label {
            leftPadding: 12
            rightPadding: combo.indicator.width + 16
            text: combo.displayText
            color: theme.textPrimary
            font: combo.font
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
        }

        indicator: Label {
            x: combo.width - width - 12
            y: (combo.height - height) / 2
            text: "▾"
            color: theme.textMuted
            font.pixelSize: 12
        }

        popup: Popup {
            y: combo.height + 4
            width: combo.width
            implicitHeight: Math.min(360, contentItem.implicitHeight + 16)
            padding: 8

            background: Rectangle {
                color: theme.cardBgRaised
                radius: theme.radiusSm
                border.width: 1
                border.color: theme.borderSubtle
            }

            contentItem: ListView {
                clip: true
                implicitHeight: contentHeight
                model: combo.popup.visible ? combo.delegateModel : null
                currentIndex: combo.highlightedIndex
                ScrollIndicator.vertical: ScrollIndicator {}
            }
        }

        delegate: ItemDelegate {
            width: combo.width - 16
            highlighted: combo.highlightedIndex === index
            contentItem: Label {
                text: modelData
                color: theme.textPrimary
                font.pixelSize: 13
                elide: Text.ElideRight
                verticalAlignment: Text.AlignVCenter
            }
            background: Rectangle {
                color: highlighted ? theme.accentSoft : "transparent"
                radius: 6
            }
        }

        onActivated: (index) => root.activated(index)
    }
}
