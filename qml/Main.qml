import QtQuick
import app.lepramim

QtObject {
    id: root

    property AppController controller: AppController {}

    property ControlWindow controlWindow: ControlWindow {
        controller: root.controller
    }
    property OverlayWindow overlayWindow: OverlayWindow {
        controller: root.controller
    }
    property OnboardingWindow onboardingWindow: OnboardingWindow {
        controller: root.controller
    }
    property WarningWindow warningWindow: WarningWindow {
        controller: root.controller
    }

    // Drain tray / hotkey / single-instance channels
    property Timer inputTimer: Timer {
        interval: 10
        running: true
        repeat: true
        onTriggered: root.controller.pollInput()
    }

    // Slow / fast daemon state polling
    property Timer tickTimer: Timer {
        interval: root.controller.fastPolling ? 200 : 1000
        running: true
        repeat: true
        onTriggered: root.controller.tick()
    }

    Connections {
        target: root.controller
        function onQuitRequestedChanged() {
            if (root.controller.quitRequested)
                Qt.quit()
        }
    }

    Component.onCompleted: {
        // Tray-first: control/overlay windows stay hidden until requested.
        Qt.application.quitOnLastWindowClosed = false
        root.controller.bootstrap()
    }
}
