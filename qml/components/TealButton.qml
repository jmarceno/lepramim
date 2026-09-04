import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import app.lepramim

Button {
    id: root
    property bool primary: true
    property bool compact: false

    leftPadding: compact ? 12 : 16
    rightPadding: compact ? 12 : 16
    topPadding: compact ? 8 : 10
    bottomPadding: compact ? 8 : 10
    font.pixelSize: compact ? 13 : 14
    font.bold: true

    contentItem: RowLayout {
        spacing: 8
        // Placeholder for optional icon via text prefix handled by caller
        Label {
            text: root.text
            color: root.primary ? "#0d1f1c" : Theme.textPrimary
            font: root.font
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            Layout.fillWidth: true
        }
    }

    background: Rectangle {
        implicitHeight: root.compact ? 32 : 40
        radius: Theme.radiusSm
        color: {
            if (!root.enabled)
                return root.primary ? "#2a6f66" : Theme.cardBgRaised
            if (root.down)
                return root.primary ? Qt.darker(Theme.accent, 1.15) : Theme.borderSubtle
            if (root.hovered)
                return root.primary ? Qt.lighter(Theme.accent, 1.08) : Theme.cardBgRaised
            return root.primary ? Theme.accent : Theme.cardBgRaised
        }
        border.width: root.primary ? 0 : 1
        border.color: Theme.borderSubtle
    }
}
