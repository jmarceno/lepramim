import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import app.lepramim 1.0

Button {
    id: root
    readonly property LepramimTheme theme: LepramimTheme {}
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
            color: root.primary ? "#0d1f1c" : theme.textPrimary
            font: root.font
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            Layout.fillWidth: true
        }
    }

    background: Rectangle {
        implicitHeight: root.compact ? 32 : 40
        radius: theme.radiusSm
        color: {
            if (!root.enabled)
                return root.primary ? "#2a6f66" : theme.cardBgRaised
            if (root.down)
                return root.primary ? Qt.darker(theme.accent, 1.15) : theme.borderSubtle
            if (root.hovered)
                return root.primary ? Qt.lighter(theme.accent, 1.08) : theme.cardBgRaised
            return root.primary ? theme.accent : theme.cardBgRaised
        }
        border.width: root.primary ? 0 : 1
        border.color: theme.borderSubtle
    }
}
