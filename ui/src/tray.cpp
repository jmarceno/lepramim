#include "tray.hpp"

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QIcon>
#include <QPainter>
#include <QPixmap>
#include <QProcess>
#include <QStandardPaths>
#include <QSvgRenderer>

#include "control_window.hpp"

namespace lexaloud {

static QIcon tintedIcon(const QString& path, double opacity) {
    if (path.isEmpty() || !QFileInfo::exists(path)) {
        QIcon fallback;
        return fallback;
    }
    QSvgRenderer renderer(path);
    QImage image(64, 64, QImage::Format_ARGB32);
    image.fill(Qt::transparent);
    QPainter painter(&image);
    painter.setOpacity(opacity);
    renderer.render(&painter);
    painter.end();
    QPixmap pixmap = QPixmap::fromImage(image);
    pixmap.setDevicePixelRatio(2.0);
    return QIcon(pixmap);
}

TrayActionState trayStateForDaemon(const QString& stateStr, bool active) {
    TrayActionState s;
    s.isActive = active;
    s.isWarming = (stateStr == QLatin1String("warming"));
    if (s.isWarming) {
        s.iconState = QStringLiteral("running");
        s.tooltip = QStringLiteral("Lexaloud: warming up");
        s.toggleLabel = QStringLiteral("Stop daemon (warming\u2026)");
    } else if (active) {
        s.iconState = QStringLiteral("running");
        s.tooltip = QStringLiteral("Lexaloud: running");
        s.toggleLabel = QStringLiteral("Stop daemon");
    } else {
        s.iconState = QStringLiteral("stopped");
        s.tooltip = QStringLiteral("Lexaloud: stopped");
        s.toggleLabel = QStringLiteral("Start daemon");
    }
    bool ready = active && !s.isWarming;
    s.speakEnabled = ready;
    s.pauseEnabled = ready;
    s.stopEnabled = ready;
    return s;
}

QString Tray::autostartDesktopPath() {
    QString configHome = qEnvironmentVariable("XDG_CONFIG_HOME");
    if (configHome.isEmpty()) {
        configHome = QDir::homePath() + QStringLiteral("/.config");
    }
    return QDir(configHome).filePath(QStringLiteral("autostart/lexaloud.desktop"));
}

bool Tray::isAutostartEnabled() {
    return QFileInfo::exists(autostartDesktopPath());
}

bool Tray::setAutostartEnabled(bool enabled) {
    QString path = autostartDesktopPath();
    if (!enabled) {
        if (QFileInfo::exists(path)) {
            return QFile::remove(path);
        }
        return true;
    }
    QDir dir = QFileInfo(path).dir();
    if (!dir.exists() && !dir.mkpath(QStringLiteral("."))) {
        return false;
    }
    QString binary = QCoreApplication::applicationFilePath();
    QFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Truncate | QIODevice::Text)) {
        return false;
    }
    QString content = QStringLiteral(
        "[Desktop Entry]\n"
        "Type=Application\n"
        "Name=Lexaloud\n"
        "GenericName=Text to Speech\n"
        "Comment=Local Kokoro text-to-speech tool\n"
        "Exec=%1\n"
        "Terminal=false\n"
        "Categories=AudioVideo;Audio;Accessibility;\n"
        "X-GNOME-Autostart-enabled=true\n");
    content = content.arg(binary);
    qint64 w = file.write(content.toUtf8());
    file.close();
    return w > 0;
}

Tray::Tray(const QString& iconPath, QObject* parent)
    : QSystemTrayIcon(parent), m_iconPath(iconPath) {
    // Build icons
    QIcon running = tintedIcon(iconPath, 1.0);
    QIcon stopped = tintedIcon(iconPath, 0.35);
    if (running.isNull() || stopped.isNull()) {
        // Fallback to generic icons
        running = QIcon::fromTheme(QStringLiteral("audio-x-generic"));
        stopped = QIcon::fromTheme(QStringLiteral("audio-x-generic"));
    }
    setIcon(stopped);
    m_currentIconState = QStringLiteral("stopped");
    setToolTip(QStringLiteral("Lexaloud: stopped"));

    m_menu = new QMenu();
    buildMenu();
    setContextMenu(m_menu);

    m_timer = new QTimer(this);
    m_timer->setInterval(1000);
    connect(m_timer, &QTimer::timeout, this, &Tray::pollState);
    m_timer->start();
    // Initial poll
    QTimer::singleShot(100, this, &Tray::pollState);
}

Tray::~Tray() = default;

void Tray::setControlWindow(ControlWindow* window) {
    m_controlWindow = window;
}

void Tray::buildMenu() {
    m_actionShortcut = new QAction(shortcutLabel(), m_menu);
    m_actionShortcut->setEnabled(false);
    m_actionShortcut->setToolTip(
        QStringLiteral("Global shortcut \u2014 press it with text selected"));
    m_menu->addAction(m_actionShortcut);
    m_menu->addSeparator();

    m_actionToggleDaemon = new QAction(QStringLiteral("Start daemon"), m_menu);
    connect(m_actionToggleDaemon, &QAction::triggered, this, &Tray::onToggleDaemon);
    m_menu->addAction(m_actionToggleDaemon);

    m_actionSpeak = new QAction(QStringLiteral("Speak highlighted selection"), m_menu);
    connect(m_actionSpeak, &QAction::triggered, this, &Tray::onSpeakSelection);
    m_menu->addAction(m_actionSpeak);

    m_actionPause = new QAction(QStringLiteral("Pause / resume"), m_menu);
    connect(m_actionPause, &QAction::triggered, this, &Tray::onPauseResume);
    m_menu->addAction(m_actionPause);

    m_actionStop = new QAction(QStringLiteral("Stop current playback"), m_menu);
    connect(m_actionStop, &QAction::triggered, this, &Tray::onStopPlayback);
    m_menu->addAction(m_actionStop);

    m_menu->addSeparator();

    m_actionControl = new QAction(QStringLiteral("Control window\u2026"), m_menu);
    connect(m_actionControl, &QAction::triggered, this, &Tray::onOpenControl);
    m_menu->addAction(m_actionControl);

    m_actionAutostart = new QAction(QStringLiteral("Start with desktop"), m_menu);
    m_actionAutostart->setCheckable(true);
    m_actionAutostart->setChecked(isAutostartEnabled());
    m_actionAutostart->setToolTip(
        QStringLiteral("Launch Lexaloud (tray + speech service) automatically at login."));
    connect(m_actionAutostart, &QAction::toggled, this, &Tray::onAutostartToggled);
    m_menu->addAction(m_actionAutostart);

    m_actionQuit = new QAction(QStringLiteral("Quit Lexaloud"), m_menu);
    connect(m_actionQuit, &QAction::triggered, this, &Tray::onQuit);
    m_menu->addAction(m_actionQuit);
}

QString Tray::shortcutLabel() const {
    // In Rust port, shortcuts are managed elsewhere; show default.
    // Try to read via environment? Keep simple.
    return QStringLiteral("Shortcut: Meta+R");
}

bool Tray::isDaemonActive() const {
    // Check via UDS first, fallback to systemd
    ApiResult r = m_client.getState();
    if (r.statusCode == 200) {
        return true;
    }
    // Try systemctl via QProcess (non-blocking check with short timeout via sync)
    QProcess proc;
    proc.start(QStringLiteral("systemctl"),
               QStringList{QStringLiteral("--user"), QStringLiteral("is-active"),
                           QStringLiteral("lexaloud.service")});
    if (!proc.waitForFinished(800)) {
        proc.kill();
        return false;
    }
    QByteArray out = proc.readAllStandardOutput().trimmed();
    return out == QByteArrayLiteral("active");
}

void Tray::updateIcon(const QString& desired) {
    if (desired == m_currentIconState) {
        return;
    }
    double opacity = (desired == QLatin1String("running")) ? 1.0 : 0.35;
    QIcon icon = tintedIcon(m_iconPath, opacity);
    if (!icon.isNull()) {
        setIcon(icon);
    }
    m_currentIconState = desired;
}

void Tray::setTrayTooltip(const QString& tip) {
    setToolTip(tip);
}

void Tray::pollState() {
    refreshState();
}

void Tray::refreshState() {
    bool active = isDaemonActive();
    QString stateStr;
    ApiResult r = m_client.getState();
    if (r.statusCode == 200 && r.json.isObject()) {
        QJsonObject obj = r.json.object();
        stateStr = obj.value(QStringLiteral("state")).toString();
    } else if (!active) {
        stateStr = QString();
    }
    applyDaemonState(stateStr, active);
}

void Tray::applyDaemonState(const QString& stateStr, bool active) {
    TrayActionState st = trayStateForDaemon(stateStr, active);
    updateIcon(st.iconState);
    setTrayTooltip(st.tooltip);
    if (m_actionToggleDaemon->text() != st.toggleLabel) {
        m_actionToggleDaemon->setText(st.toggleLabel);
    }
    m_actionSpeak->setEnabled(st.speakEnabled);
    m_actionPause->setEnabled(st.pauseEnabled);
    m_actionStop->setEnabled(st.stopEnabled);
}

TrayActionState Tray::currentTrayState() const {
    TrayActionState s;
    s.speakEnabled = m_actionSpeak->isEnabled();
    s.pauseEnabled = m_actionPause->isEnabled();
    s.stopEnabled = m_actionStop->isEnabled();
    s.tooltip = toolTip();
    s.toggleLabel = m_actionToggleDaemon->text();
    s.iconState = m_currentIconState;
    return s;
}

void Tray::onToggleDaemon() {
    QProcess* proc = new QProcess(this);
    bool active = isDaemonActive();
    QString action = active ? QStringLiteral("stop") : QStringLiteral("start");
    proc->start(QStringLiteral("systemctl"),
                QStringList{QStringLiteral("--user"), action, QStringLiteral("lexaloud.service")});
    connect(proc, QOverload<int, QProcess::ExitStatus>::of(&QProcess::finished), proc,
            &QObject::deleteLater);
    QTimer::singleShot(500, this, &Tray::refreshState);
}

void Tray::onSpeakSelection() {
    // Best-effort: use wl-paste/xclip path? For now POST empty speak via daemon as placeholder.
    // Real selection capture is platform-specific; we delegate to lexaloud binary if present.
    QProcess::startDetached(QStringLiteral("lexaloud"),
                            QStringList{QStringLiteral("speak-selection")});
}

void Tray::onPauseResume() {
    ApiResult r = m_client.postToggle();
    Q_UNUSED(r)
    QTimer::singleShot(300, this, &Tray::refreshState);
}

void Tray::onStopPlayback() {
    ApiResult r = m_client.postStop();
    Q_UNUSED(r)
    QTimer::singleShot(300, this, &Tray::refreshState);
}

void Tray::onOpenControl() {
    if (m_controlWindow == nullptr) {
        m_controlWindow = new ControlWindow();
        m_controlWindow->setAttribute(Qt::WA_DeleteOnClose, false);
    }
    m_controlWindow->show();
    m_controlWindow->raise();
    m_controlWindow->activateWindow();
}

void Tray::onAutostartToggled(bool checked) {
    if (!setAutostartEnabled(checked)) {
        // Revert
        m_actionAutostart->setChecked(!checked);
    }
}

void Tray::onQuit() {
    emit quitRequested();
    QCoreApplication::quit();
}

}  // namespace lexaloud
