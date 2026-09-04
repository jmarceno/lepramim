import QtQuick
import QtQuick.Controls
import app.lepramim

Rectangle {
    id: root
    color: Theme.cardBg
    radius: Theme.radius
    border.width: 0

    // Rectangle never derives implicit size from children, so a card used
    // without fillWidth/fillHeight inside a Row/Column layout would collapse
    // to zero. Forward the body's content-driven size instead.
    implicitWidth: body.implicitWidth + root.contentPadding * 2
    implicitHeight: body.implicitHeight + root.contentPadding * 2

    default property alias content: body.data
    property alias title: titleLabel.text
    property int contentPadding: 18
    property int contentSpacing: 14

    Column {
        id: body
        anchors.fill: parent
        anchors.margins: root.contentPadding
        spacing: root.contentSpacing

        Label {
            id: titleLabel
            visible: text.length > 0
            color: Theme.textPrimary
            font.pixelSize: 16
            font.bold: true
            width: parent.width
        }
    }
}
