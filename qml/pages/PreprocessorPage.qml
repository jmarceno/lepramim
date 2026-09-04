import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import app.lepramim

Item {
    id: root
    property var controller

    component PrefCheck: CheckBox {
        id: box
        indicator: Rectangle {
            implicitWidth: 18
            implicitHeight: 18
            x: box.leftPadding
            y: parent.height / 2 - height / 2
            radius: 4
            color: box.checked ? Theme.accent : Theme.inputBg
            border.color: box.checked ? Theme.accent : Theme.borderSubtle
            border.width: 1
            Label {
                anchors.centerIn: parent
                text: "✓"
                color: "#0d1f1c"
                font.pixelSize: 12
                font.bold: true
                visible: box.checked
            }
        }
        contentItem: Label {
            text: box.text
            color: Theme.textSecondary
            font.pixelSize: 14
            leftPadding: box.indicator.width + 10
            verticalAlignment: Text.AlignVCenter
        }
    }

    Card {
        anchors.fill: parent
        title: "Preprocessor"

        ColumnLayout {
            width: parent.width
            spacing: 8

            PrefCheck {
                text: "Deduplicate MathJax selection"
                checked: root.controller.dedupeMathjax
                onToggled: root.controller.applyDedupeMathjax(checked)
            }
            PrefCheck {
                text: "Strip Markdown"
                checked: root.controller.stripMarkdown
                onToggled: root.controller.applyStripMarkdown(checked)
            }
            PrefCheck {
                text: "Strip numeric bracket citations"
                checked: root.controller.stripNumericCitations
                onToggled: root.controller.applyStripNumericCitations(checked)
            }
            PrefCheck {
                text: "Expand Latin abbreviations"
                checked: root.controller.expandLatin
                onToggled: root.controller.applyExpandLatin(checked)
            }
            PrefCheck {
                text: "Normalize numbers"
                checked: root.controller.normalizeNumbers
                onToggled: root.controller.applyNormalizeNumbers(checked)
            }

            Label {
                Layout.topMargin: 8
                Layout.fillWidth: true
                text: "These filters run on highlighted text before speech synthesis."
                color: Theme.textMuted
                font.pixelSize: 12
                wrapMode: Text.WordWrap
            }
        }
    }
}
