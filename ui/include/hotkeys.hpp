#pragma once

#include <QObject>

namespace lexaloud {

class ApiClient;

// In-process Meta+R / Meta+P: KGlobalAccel on KDE, session D-Bus for GNOME shortcuts.
// Never spawns the AppImage per keypress.
class HotkeyManager : public QObject {
    Q_OBJECT
public:
    explicit HotkeyManager(ApiClient* client, QObject* parent = nullptr);
    ~HotkeyManager() override;

    bool isRegistered() const {
        return m_registered;
    }

public slots:
    Q_SCRIPTABLE void SpeakSelection();
    Q_SCRIPTABLE void Toggle();

private slots:
    void onGlobalShortcutReleased(const QString& component, const QString& action,
                                  qlonglong timestamp);
    void doSpeak();

private:
    void registerSessionService();
    void registerKGlobalAccel();
    void unregisterKGlobalAccel();

    ApiClient* m_client = nullptr;
    bool m_registered = false;
};

}  // namespace lexaloud
