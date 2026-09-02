#include "hotkeys.hpp"

#include <QKeySequence>
#include <QProcess>
#include <QTimer>
#include <QtDBus/QDBusConnection>
#include <QtDBus/QDBusInterface>
#include <QtDBus/QDBusMessage>
#include <QtDBus/QDBusReply>

#include "api_client.hpp"
#include "capture.hpp"

namespace lexaloud {
namespace {

int keySequenceCombined(const QString& seqText) {
    const QKeySequence seq = QKeySequence::fromString(seqText);
    if (seq.isEmpty()) {
        return 0;
    }
    return seq[0].toCombined();
}

bool busctlSetShortcut(const QString& action, const QString& label, int keyInt) {
    QProcess proc;
    proc.start(QStringLiteral("busctl"),
               QStringList{QStringLiteral("--user"), QStringLiteral("call"),
                           QStringLiteral("org.kde.kglobalaccel"), QStringLiteral("/kglobalaccel"),
                           QStringLiteral("org.kde.KGlobalAccel"), QStringLiteral("setShortcut"),
                           QStringLiteral("asaiu"), QStringLiteral("4"), QStringLiteral("lexaloud"),
                           action, QStringLiteral("Lexaloud"), label, QStringLiteral("1"),
                           QString::number(keyInt), QStringLiteral("2")});
    if (!proc.waitForFinished(2000)) {
        proc.kill();
        return false;
    }
    return proc.exitStatus() == QProcess::NormalExit && proc.exitCode() == 0;
}

}  // namespace

HotkeyManager::HotkeyManager(ApiClient* client, QObject* parent)
    : QObject(parent), m_client(client) {
    setObjectName(QStringLiteral("lexaloudHotkeys"));
    registerSessionService();
    registerKGlobalAccel();
}

HotkeyManager::~HotkeyManager() {
    unregisterKGlobalAccel();
}

void HotkeyManager::registerSessionService() {
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        return;
    }
    bus.registerService(QStringLiteral("org.lexaloud.App"));
    bus.registerObject(QStringLiteral("/org/lexaloud/App"), this,
                       QDBusConnection::ExportScriptableSlots);
}

void HotkeyManager::registerKGlobalAccel() {
    QDBusConnection bus = QDBusConnection::sessionBus();
    if (!bus.isConnected()) {
        return;
    }
    QDBusInterface iface(QStringLiteral("org.kde.kglobalaccel"), QStringLiteral("/kglobalaccel"),
                         QStringLiteral("org.kde.KGlobalAccel"), bus);
    if (!iface.isValid()) {
        return;
    }

    struct Action {
        const char* id;
        const char* label;
        const char* key;
    };
    const Action actions[] = {
        {"speak-selection", "Speak highlighted selection", "Meta+R"},
        {"toggle", "Pause / resume", "Meta+P"},
    };
    for (const Action& action : actions) {
        const QStringList actionId{QStringLiteral("lexaloud"), QString::fromUtf8(action.id),
                                   QStringLiteral("Lexaloud"), QString::fromUtf8(action.label)};
        QDBusMessage reply = iface.call(QStringLiteral("doRegister"), actionId);
        if (reply.type() == QDBusMessage::ErrorMessage) {
            return;
        }
        iface.call(QStringLiteral("getComponent"), QStringLiteral("lexaloud"));
        const int keyInt = keySequenceCombined(QString::fromUtf8(action.key));
        if (keyInt != 0 && !busctlSetShortcut(QString::fromUtf8(action.id),
                                              QString::fromUtf8(action.label), keyInt)) {
            return;
        }
    }

    QDBusInterface component(QStringLiteral("org.kde.kglobalaccel"),
                             QStringLiteral("/component/lexaloud"),
                             QStringLiteral("org.kde.kglobalaccel.Component"), bus);
    QDBusReply<bool> active = component.call(QStringLiteral("isActive"));
    if (!active.isValid() || !active.value()) {
        return;
    }
    const bool connected = bus.connect(
        QStringLiteral("org.kde.kglobalaccel"), QStringLiteral("/component/lexaloud"),
        QStringLiteral("org.kde.kglobalaccel.Component"), QStringLiteral("globalShortcutReleased"),
        this, SLOT(onGlobalShortcutReleased(QString, QString, qlonglong)));
    if (!connected) {
        return;
    }
    m_registered = true;
}

void HotkeyManager::unregisterKGlobalAccel() {
    if (!m_registered) {
        return;
    }
    QDBusInterface iface(QStringLiteral("org.kde.kglobalaccel"), QStringLiteral("/kglobalaccel"),
                         QStringLiteral("org.kde.KGlobalAccel"), QDBusConnection::sessionBus());
    if (iface.isValid()) {
        iface.call(QStringLiteral("unregister"), QStringLiteral("lexaloud"),
                   QStringLiteral("speak-selection"));
        iface.call(QStringLiteral("unregister"), QStringLiteral("lexaloud"),
                   QStringLiteral("toggle"));
    }
    m_registered = false;
}

void HotkeyManager::SpeakSelection() {
    QTimer::singleShot(250, this, &HotkeyManager::doSpeak);
}

void HotkeyManager::Toggle() {
    if (m_client != nullptr) {
        togglePlayback(*m_client);
    }
}

void HotkeyManager::onGlobalShortcutReleased(const QString& component, const QString& action,
                                             qlonglong timestamp) {
    Q_UNUSED(timestamp)
    if (component != QLatin1String("lexaloud")) {
        return;
    }
    if (action == QLatin1String("speak-selection")) {
        SpeakSelection();
    } else if (action == QLatin1String("toggle")) {
        Toggle();
    }
}

void HotkeyManager::doSpeak() {
    if (m_client != nullptr) {
        speakCapturedSelection(*m_client);
    }
}

}  // namespace lexaloud
