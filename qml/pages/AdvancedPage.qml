import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import app.lepramim

Item {
    id: root
    required property var controller

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
        title: "Advanced"

        ColumnLayout {
            width: parent.width
            spacing: 8

            PrefCheck {
                text: "Strip parenthetical citations"
                checked: root.controller.strip_parenthetical_citations
                onToggled: root.controller.applyStripParentheticalCitations(checked)
            }
            PrefCheck {
                text: "Expand academic abbreviations"
                checked: root.controller.expand_academic
                onToggled: root.controller.applyExpandAcademic(checked)
            }
            PrefCheck {
                text: "Normalize URLs"
                checked: root.controller.normalize_urls
                onToggled: root.controller.applyNormalizeUrls(checked)
            }
            PrefCheck {
                text: "Normalize math symbols"
                checked: root.controller.normalize_math_symbols
                onToggled: root.controller.applyNormalizeMathSymbols(checked)
            }
            PrefCheck {
                text: "PDF cleanup"
                checked: root.controller.pdf_cleanup
                onToggled: root.controller.applyPdfCleanup(checked)
            }
            PrefCheck {
                text: "Enable Speech Rule Engine for LaTeX"
                checked: root.controller.sre_latex_enabled
                onToggled: root.controller.applySreLatexEnabled(checked)
            }
            PrefCheck {
                text: "Low-memory mode (load voices on first playback)"
                checked: root.controller.low_memory_mode
                onToggled: root.controller.applyLowMemoryMode(checked)
            }

            Label {
                Layout.fillWidth: true
                text: "Low-memory mode skips pre-loading the Kokoro model and voices (~300-400 MB) at startup. The first playback takes longer while they load. Takes effect after you restart the app."
                color: Theme.textMuted
                font.pixelSize: 12
                wrapMode: Text.WordWrap
            }

            Label {
                Layout.topMargin: 8
                Layout.fillWidth: true
                text: "Floating overlay is toggled on the Voice page. Changes apply after you press Apply settings."
                color: Theme.textMuted
                font.pixelSize: 12
                wrapMode: Text.WordWrap
            }
        }
    }
}
