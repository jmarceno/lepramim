#include "capture.hpp"

#include <QByteArray>
#include <QClipboard>
#include <QCoreApplication>
#include <QDebug>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QProcess>
#include <QStandardPaths>
#include <QThread>
#include <QtDBus/QDBusConnection>
#include <QtDBus/QDBusInterface>
#include <QtDBus/QDBusReply>

#include "api_client.hpp"

namespace lexaloud {
namespace {

constexpr int kMaxBytes = 200 * 1024;
constexpr int kToolTimeoutMs = 400;
constexpr int kInjectorTimeoutMs = 250;

QString findTool(const QString& name) {
    QString path = QStandardPaths::findExecutable(name);
    if (!path.isEmpty()) {
        return path;
    }
    const QString sibling = QDir(QCoreApplication::applicationDirPath()).filePath(name);
    if (QFileInfo::exists(sibling)) {
        return sibling;
    }
    return {};
}

bool runCapture(const QString& program, const QStringList& args, QByteArray* stdoutBytes) {
    if (program.isEmpty()) {
        return false;
    }
    QProcess proc;
    proc.setProcessChannelMode(QProcess::SeparateChannels);
    proc.start(program, args);
    if (!proc.waitForFinished(kToolTimeoutMs)) {
        proc.kill();
        proc.waitForFinished(200);
        return false;
    }
    *stdoutBytes = proc.readAllStandardOutput();
    return proc.exitStatus() == QProcess::NormalExit && proc.exitCode() == 0;
}

QString decodeText(const QByteArray& raw, bool* truncated) {
    QByteArray data = raw;
    *truncated = false;
    if (data.size() > kMaxBytes) {
        int cut = kMaxBytes;
        while (cut > 0 && (static_cast<unsigned char>(data[cut]) & 0xC0) == 0x80) {
            --cut;
        }
        data = data.left(cut);
        *truncated = true;
    }
    return QString::fromUtf8(data);
}

bool isWayland() {
    const QByteArray session = qgetenv("XDG_SESSION_TYPE").toLower();
    return session == "wayland" || !qEnvironmentVariableIsEmpty("WAYLAND_DISPLAY");
}

QString qtClipboardText(QClipboard::Mode mode) {
    QClipboard* cb = QGuiApplication::clipboard();
    if (cb == nullptr) {
        return {};
    }
    return cb->text(mode).trimmed();
}

bool readPrimaryX11(SelectionCapture* out) {
    QByteArray raw;
    const QString xclip = findTool(QStringLiteral("xclip"));
    if (!runCapture(xclip,
                    QStringList{QStringLiteral("-o"), QStringLiteral("-selection"),
                                QStringLiteral("primary")},
                    &raw)) {
        return false;
    }
    bool truncated = false;
    out->text = decodeText(raw, &truncated).trimmed();
    out->source = QStringLiteral("primary/xclip");
    out->truncated = truncated;
    return !out->text.isEmpty();
}

bool readClipboardTool(SelectionCapture* out) {
    QByteArray raw;
    bool ok = false;
    if (isWayland()) {
        const QString wl = findTool(QStringLiteral("wl-paste"));
        ok = runCapture(wl, QStringList{QStringLiteral("--no-newline")}, &raw);
        out->source = QStringLiteral("clipboard/wl-paste");
    } else {
        const QString xclip = findTool(QStringLiteral("xclip"));
        ok = runCapture(xclip,
                        QStringList{QStringLiteral("-o"), QStringLiteral("-selection"),
                                    QStringLiteral("clipboard")},
                        &raw);
        out->source = QStringLiteral("clipboard/xclip");
    }
    if (!ok) {
        return false;
    }
    bool truncated = false;
    out->text = decodeText(raw, &truncated).trimmed();
    out->truncated = truncated;
    return !out->text.isEmpty();
}

bool readKlipper(SelectionCapture* out) {
    QDBusInterface iface(QStringLiteral("org.kde.klipper"), QStringLiteral("/klipper"),
                         QStringLiteral("org.kde.klipper.klipper"), QDBusConnection::sessionBus());
    if (!iface.isValid()) {
        return false;
    }
    QDBusReply<QString> reply = iface.call(QStringLiteral("getClipboardContents"));
    if (!reply.isValid()) {
        return false;
    }
    QString text = reply.value().trimmed();
    if (text.startsWith(QStringLiteral("▨"))) {
        return false;
    }
    if (text.isEmpty()) {
        return false;
    }
    out->text = text;
    out->source = QStringLiteral("clipboard/klipper");
    out->truncated = false;
    return true;
}

QString currentClipboardText() {
    QString qt = qtClipboardText(QClipboard::Clipboard);
    if (!qt.isEmpty()) {
        return qt;
    }
    SelectionCapture clip;
    if (readClipboardTool(&clip)) {
        return clip.text;
    }
    if (readKlipper(&clip)) {
        return clip.text;
    }
    return {};
}

bool tryForceCopy() {
    auto run = [](const QString& program, const QStringList& args, const QByteArray& input = {}) {
        if (program.isEmpty()) {
            return false;
        }
        QProcess proc;
        proc.start(program, args);
        if (!input.isEmpty()) {
            proc.write(input);
            proc.closeWriteChannel();
        }
        if (!proc.waitForFinished(kInjectorTimeoutMs)) {
            proc.kill();
            proc.waitForFinished(150);
            return false;
        }
        return proc.exitStatus() == QProcess::NormalExit && proc.exitCode() == 0;
    };

    bool injected = false;
    if (run(findTool(QStringLiteral("ydotool")),
            QStringList{QStringLiteral("key"), QStringLiteral("29:1"), QStringLiteral("46:1"),
                        QStringLiteral("46:0"), QStringLiteral("29:0")})) {
        injected = true;
    } else if (run(findTool(QStringLiteral("wtype")),
                   QStringList{QStringLiteral("-M"), QStringLiteral("ctrl"), QStringLiteral("-P"),
                               QStringLiteral("c"), QStringLiteral("-m"), QStringLiteral("ctrl")})) {
        injected = true;
    } else if (run(findTool(QStringLiteral("xdotool")),
                   QStringList{QStringLiteral("key"), QStringLiteral("ctrl+c")})) {
        injected = true;
    } else if (run(findTool(QStringLiteral("dotool")), QStringList(),
                   QByteArray("key Ctrl_L+c\n"))) {
        injected = true;
    }
    if (!injected) {
        return false;
    }
    QThread::msleep(80);
    QCoreApplication::processEvents();
    return true;
}

}  // namespace

SelectionCapture resolveCapture(const QString& selection, const QString& clipboardBefore,
                                const QString& clipboardAfter) {
    SelectionCapture out;
    const QString sel = selection.trimmed();
    const QString before = clipboardBefore.trimmed();
    const QString after = clipboardAfter.trimmed();
    if (!sel.isEmpty()) {
        out.text = sel;
        out.source = QStringLiteral("primary/qt");
        return out;
    }
    if (!after.isEmpty() && after != before) {
        out.text = after;
        out.source = QStringLiteral("clipboard/updated");
        return out;
    }
    if (!after.isEmpty()) {
        out.text = after;
        out.source = QStringLiteral("clipboard");
        return out;
    }
    if (!before.isEmpty()) {
        out.text = before;
        out.source = QStringLiteral("clipboard");
        return out;
    }
    return out;
}

void notifyUser(const QString& title, const QString& body) {
    const QString notify = findTool(QStringLiteral("notify-send"));
    if (notify.isEmpty()) {
        return;
    }
    QProcess::startDetached(notify, QStringList{title, body});
}

SelectionCapture captureHighlightedText() {
    if (!isWayland()) {
        const QString qtSel = qtClipboardText(QClipboard::Selection);
        if (!qtSel.isEmpty()) {
            SelectionCapture out;
            out.text = qtSel;
            out.source = QStringLiteral("primary/qt");
            return out;
        }
        SelectionCapture primary;
        if (readPrimaryX11(&primary)) {
            return primary;
        }
    }

    const QString before = currentClipboardText();
    tryForceCopy();
    const QString after = currentClipboardText();
    return resolveCapture(QString(), before, after);
}

void speakCapturedSelection(ApiClient& client) {
    SelectionCapture cap = captureHighlightedText();
    if (cap.text.isEmpty()) {
        notifyUser(
            QStringLiteral("Select text first"),
            QStringLiteral(
                "Lexaloud could not capture a selection. Copy the text (Ctrl+C) and try again."));
        return;
    }
    qWarning() << "Lexaloud capture from" << cap.source << "chars" << cap.text.size()
               << "excerpt" << cap.text.left(80);
    if (cap.truncated) {
        notifyUser(QStringLiteral("Selection truncated"),
                   QStringLiteral("Lexaloud captured the first part of a larger selection."));
    }
    ApiResult r = client.postSpeak(cap.text, QStringLiteral("replace"));
    if (r.isDaemonDown()) {
        notifyUser(QStringLiteral("Lexaloud"), QStringLiteral("Speech daemon is not running."));
    } else if (!r.isSuccess()) {
        notifyUser(QStringLiteral("Lexaloud"),
                   QStringLiteral("Could not start speech. Is the control window working?"));
    }
}

void togglePlayback(ApiClient& client) {
    client.postToggle();
}

}  // namespace lexaloud
