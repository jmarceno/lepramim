#pragma once

#include <QAction>
#include <QMenu>
#include <QProcess>
#include <QSystemTrayIcon>
#include <QTimer>

#include "api_client.hpp"

namespace lexaloud {
class ControlWindow;

struct TrayActionState {
    bool speakEnabled = false;
    bool pauseEnabled = false;
    bool stopEnabled = false;
    bool isWarming = false;
    bool isActive = false;
    QString toggleLabel;
    QString tooltip;
    QString iconState;  // "running" or "stopped"
};

// Pure function for testing: maps daemon state to action enablement.
TrayActionState trayStateForDaemon(const QString& stateStr, bool active);

class Tray : public QSystemTrayIcon {
    Q_OBJECT
public:
    explicit Tray(const QString& iconPath = QString(), QObject* parent = nullptr);
    ~Tray() override;

    void setControlWindow(ControlWindow* window);
    void refreshState();
    TrayActionState currentTrayState() const;

    // Exposed for tests
    QAction* actionShortcut() const {
        return m_actionShortcut;
    }
    QAction* actionToggleDaemon() const {
        return m_actionToggleDaemon;
    }
    QAction* actionSpeak() const {
        return m_actionSpeak;
    }
    QAction* actionPause() const {
        return m_actionPause;
    }
    QAction* actionStop() const {
        return m_actionStop;
    }
    QAction* actionControl() const {
        return m_actionControl;
    }
    QAction* actionAutostart() const {
        return m_actionAutostart;
    }
    QAction* actionQuit() const {
        return m_actionQuit;
    }

    // Apply a given daemon state string without polling (test helper).
    void applyDaemonState(const QString& stateStr, bool active);

    static QString autostartDesktopPath();
    static bool isAutostartEnabled();
    static bool setAutostartEnabled(bool enabled);

signals:
    void quitRequested();

private slots:
    void onToggleDaemon();
    void onSpeakSelection();
    void onPauseResume();
    void onStopPlayback();
    void onOpenControl();
    void onAutostartToggled(bool checked);
    void onQuit();
    void pollState();

private:
    void buildMenu();
    QString shortcutLabel() const;
    bool isDaemonActive() const;
    void updateIcon(const QString& desired);
    void setTrayTooltip(const QString& tip);

    ApiClient m_client;
    QMenu* m_menu = nullptr;
    QAction* m_actionShortcut = nullptr;
    QAction* m_actionToggleDaemon = nullptr;
    QAction* m_actionSpeak = nullptr;
    QAction* m_actionPause = nullptr;
    QAction* m_actionStop = nullptr;
    QAction* m_actionControl = nullptr;
    QAction* m_actionAutostart = nullptr;
    QAction* m_actionQuit = nullptr;
    QTimer* m_timer = nullptr;
    ControlWindow* m_controlWindow = nullptr;
    QString m_iconPath;
    QString m_currentIconState;
    bool m_systemdMode = false;
};

}  // namespace lexaloud
