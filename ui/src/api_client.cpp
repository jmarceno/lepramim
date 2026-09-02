#include "api_client.hpp"

#include <unistd.h>

#include <QDir>
#include <QFileInfo>
#include <QJsonParseError>
#include <QLocalSocket>
#include <QStandardPaths>
#include <QUrl>

namespace lexaloud {

QString ApiClient::defaultSocketPath() {
    QByteArray xdg = qgetenv("XDG_RUNTIME_DIR");
    QString base;
    if (!xdg.isEmpty()) {
        base = QString::fromLocal8Bit(xdg);
    } else {
        base = QStringLiteral("/run/user/%1").arg(QString::number(getuid()));
    }
    QDir dir(base);
    return dir.filePath(QStringLiteral("lexaloud/lexaloud.sock"));
}

ApiClient::ApiClient(const QString& socketPath)
    : m_socketPath(socketPath.isEmpty() ? defaultSocketPath() : socketPath),
      m_timeoutMs(kDefaultTimeoutMs) {}

void ApiClient::setSocketPath(const QString& path) {
    m_socketPath = path;
}

QString ApiClient::socketPath() const {
    return m_socketPath;
}

void ApiClient::setTimeoutMs(int ms) {
    m_timeoutMs = ms;
}

int ApiClient::timeoutMs() const {
    return m_timeoutMs;
}

QByteArray ApiClient::buildRequest(const QString& method, const QString& path,
                                   const QByteArray& body, const QByteArray& contentType) {
    QByteArray req;
    req.reserve(256 + body.size());
    req.append(method.toUtf8());
    req.append(" ");
    req.append(path.toUtf8());
    req.append(" HTTP/1.1\r\n");
    req.append("Host: lexaloud\r\n");
    req.append("Connection: close\r\n");
    if (!body.isEmpty()) {
        QByteArray ct = contentType.isEmpty() ? QByteArrayLiteral("application/json") : contentType;
        req.append("Content-Type: ");
        req.append(ct);
        req.append("\r\n");
        req.append("Content-Length: ");
        req.append(QByteArray::number(body.size()));
        req.append("\r\n");
    } else if (method == QStringLiteral("POST")) {
        req.append("Content-Length: 0\r\n");
    }
    req.append("\r\n");
    if (!body.isEmpty()) {
        req.append(body);
    }
    return req;
}

ApiResult ApiClient::parseResponse(const QByteArray& raw) {
    ApiResult result;
    if (raw.isEmpty()) {
        result.errorString = QStringLiteral("empty response");
        return result;
    }
    if (raw.size() > static_cast<qsizetype>(kMaxBodyBytes + 8192)) {
        result.errorString = QStringLiteral("response body exceeds 256KB limit");
        return result;
    }
    qsizetype headerEnd = raw.indexOf("\r\n\r\n");
    if (headerEnd < 0) {
        result.errorString = QStringLiteral("malformed HTTP response: missing header terminator");
        return result;
    }
    QByteArray header = raw.left(headerEnd);
    QByteArray body = raw.mid(headerEnd + 4);

    QList<QByteArray> lines = header.split('\n');
    if (lines.isEmpty()) {
        result.errorString = QStringLiteral("malformed HTTP response: empty header");
        return result;
    }
    QByteArray statusLine = lines.first().trimmed();
    QList<QByteArray> parts = statusLine.split(' ');
    if (parts.size() < 2) {
        result.errorString = QStringLiteral("malformed status line");
        return result;
    }
    bool ok = false;
    int code = parts.at(1).toInt(&ok);
    if (!ok) {
        result.errorString = QStringLiteral("invalid status code");
        return result;
    }
    result.statusCode = code;

    // Honor Content-Length if present: if body is shorter than declared, mark truncated.
    qsizetype contentLength = -1;
    for (const QByteArray& line : lines) {
        QByteArray lower = line.toLower();
        if (lower.startsWith("content-length:")) {
            qsizetype colon = line.indexOf(':');
            QByteArray val = line.mid(colon + 1).trimmed();
            bool cOk = false;
            int cl = val.toInt(&cOk);
            if (cOk) {
                contentLength = static_cast<qsizetype>(cl);
            }
        }
    }
    if (contentLength >= 0 && body.size() < contentLength) {
        result.errorString = QStringLiteral("truncated response body");
        result.rawBody = body;
        return result;
    }
    if (body.size() > static_cast<qsizetype>(kMaxBodyBytes)) {
        result.errorString = QStringLiteral("response body exceeds 256KB limit");
        result.rawBody = body.left(kMaxBodyBytes);
        return result;
    }
    result.rawBody = body;

    if (!body.isEmpty()) {
        // Check truncated JSON due to bound
        QJsonParseError err{};
        QJsonDocument doc = QJsonDocument::fromJson(body, &err);
        if (err.error != QJsonParseError::NoError) {
            // Allow empty or non-JSON bodies for non-2xx? But for success we need JSON.
            // Report parse error but keep status.
            if (code >= 200 && code < 300) {
                result.errorString = QStringLiteral("malformed JSON: %1").arg(err.errorString());
                return result;
            }
            // For error responses, still try to keep body as text json fallback
            result.json = QJsonDocument();
            result.errorString.clear();
            // Keep rawBody for debugging; not treating as fatal for error codes
            if (err.error != QJsonParseError::NoError && body.trimmed().isEmpty()) {
                result.json = QJsonDocument();
            }
            return result;
        }
        result.json = doc;
    } else {
        result.json = QJsonDocument();
    }
    return result;
}

ApiResult ApiClient::request(const QString& method, const QString& path, const QByteArray& body,
                             const QByteArray& contentType) const {
    ApiResult out;
    QLocalSocket socket;
    socket.connectToServer(m_socketPath);
    if (!socket.waitForConnected(m_timeoutMs)) {
        out.errorString = QStringLiteral("daemon not running: %1").arg(socket.errorString());
        out.statusCode = 0;
        return out;
    }
    QByteArray req = buildRequest(method, path, body, contentType);
    qint64 written = socket.write(req);
    Q_UNUSED(written)
    if (!socket.waitForBytesWritten(m_timeoutMs)) {
        out.errorString = QStringLiteral("write timeout: %1").arg(socket.errorString());
        return out;
    }
    socket.flush();

    QByteArray raw;
    raw.reserve(4096);
    // Read until disconnect or timeout
    while (true) {
        if (socket.waitForReadyRead(m_timeoutMs)) {
            QByteArray chunk = socket.readAll();
            raw.append(chunk);
            if (raw.size() > static_cast<qsizetype>(kMaxBodyBytes + 8192)) {
                out.errorString = QStringLiteral("response too large");
                out.statusCode = 0;
                socket.disconnectFromServer();
                return out;
            }
            // If socket has bytesAvailable, continue without waiting
            while (socket.bytesAvailable() > 0) {
                raw.append(socket.readAll());
            }
            // Check if we have complete header and body per Content-Length
            qsizetype he = raw.indexOf("\r\n\r\n");
            if (he >= 0) {
                // Try to parse Content-Length
                QByteArray header = raw.left(he);
                qsizetype cl = -1;
                for (const QByteArray& line : header.split('\n')) {
                    if (line.toLower().startsWith("content-length:")) {
                        qsizetype colon = line.indexOf(':');
                        bool ok = false;
                        int v = line.mid(colon + 1).trimmed().toInt(&ok);
                        if (ok) {
                            cl = static_cast<qsizetype>(v);
                        }
                    }
                }
                if (cl >= 0) {
                    qsizetype bodyLen = raw.size() - (he + 4);
                    if (bodyLen >= cl) {
                        break;
                    }
                } else {
                    // No content-length: wait for disconnect
                    // Continue to next wait
                }
            }
        } else {
            // timeout or disconnect
            if (socket.state() == QLocalSocket::UnconnectedState) {
                // Grab any remaining
                raw.append(socket.readAll());
                break;
            }
            // If we already have header, treat timeout as end
            if (raw.contains("\r\n\r\n")) {
                raw.append(socket.readAll());
                break;
            }
            if (raw.isEmpty()) {
                out.errorString = QStringLiteral("timeout waiting for daemon response");
                return out;
            }
            break;
        }
        if (socket.state() == QLocalSocket::UnconnectedState) {
            raw.append(socket.readAll());
            break;
        }
        // Prevent infinite loop: if no more data and socket not ready, break after one extra check
        if (!socket.waitForReadyRead(100)) {
            raw.append(socket.readAll());
            break;
        }
    }
    socket.disconnectFromServer();
    if (socket.state() != QLocalSocket::UnconnectedState) {
        socket.waitForDisconnected(500);
    }
    if (raw.isEmpty()) {
        out.errorString = QStringLiteral("empty response from daemon");
        return out;
    }
    return parseResponse(raw);
}

ApiResult ApiClient::getState() const {
    return request(QStringLiteral("GET"), QStringLiteral("/state"));
}

ApiResult ApiClient::getHealthz() const {
    return request(QStringLiteral("GET"), QStringLiteral("/healthz"));
}

ApiResult ApiClient::postPause() const {
    return request(QStringLiteral("POST"), QStringLiteral("/pause"));
}

ApiResult ApiClient::postResume() const {
    return request(QStringLiteral("POST"), QStringLiteral("/resume"));
}

ApiResult ApiClient::postToggle() const {
    return request(QStringLiteral("POST"), QStringLiteral("/toggle"));
}

ApiResult ApiClient::postStop() const {
    return request(QStringLiteral("POST"), QStringLiteral("/stop"));
}

ApiResult ApiClient::postSkip() const {
    return request(QStringLiteral("POST"), QStringLiteral("/skip"));
}

ApiResult ApiClient::postBack() const {
    return request(QStringLiteral("POST"), QStringLiteral("/back"));
}

ApiResult ApiClient::postSpeak(const QString& text, const QString& mode) const {
    QJsonObject obj;
    obj[QStringLiteral("text")] = text;
    obj[QStringLiteral("mode")] = mode.isEmpty() ? QStringLiteral("replace") : mode;
    QJsonDocument doc(obj);
    QByteArray body = doc.toJson(QJsonDocument::Compact);
    return request(QStringLiteral("POST"), QStringLiteral("/speak"), body);
}

ApiResult ApiClient::getConfig() const {
    return request(QStringLiteral("GET"), QStringLiteral("/config"));
}

ApiResult ApiClient::postConfig(const QJsonDocument& doc) const {
    QByteArray body = doc.toJson(QJsonDocument::Compact);
    return request(QStringLiteral("POST"), QStringLiteral("/config"), body,
                   QByteArrayLiteral("application/json"));
}

ApiResult ApiClient::getModelsStatus() const {
    return request(QStringLiteral("GET"), QStringLiteral("/models/status"));
}

ApiResult ApiClient::getDiagnostics() const {
    return request(QStringLiteral("GET"), QStringLiteral("/diagnostics"));
}

}  // namespace lexaloud
