#pragma once

#include <QByteArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QString>

namespace lexaloud {

struct ApiResult {
    int statusCode = 0;
    QJsonDocument json;
    QString errorString;
    QByteArray rawBody;

    bool isSuccess() const {
        return errorString.isEmpty() && statusCode >= 200 && statusCode < 300;
    }
    bool isDaemonDown() const {
        return !errorString.isEmpty() && statusCode == 0;
    }
};

class ApiClient {
public:
    explicit ApiClient(const QString& socketPath = QString());
    ApiClient(const ApiClient&) = delete;
    ApiClient& operator=(const ApiClient&) = delete;

    static QString defaultSocketPath();
    static QByteArray buildRequest(const QString& method, const QString& path,
                                   const QByteArray& body = QByteArray(),
                                   const QByteArray& contentType = QByteArray());
    static ApiResult parseResponse(const QByteArray& raw);

    ApiResult request(const QString& method, const QString& path,
                      const QByteArray& body = QByteArray(),
                      const QByteArray& contentType = QByteArray()) const;

    ApiResult getState() const;
    ApiResult getHealthz() const;
    ApiResult postPause() const;
    ApiResult postResume() const;
    ApiResult postToggle() const;
    ApiResult postStop() const;
    ApiResult postSkip() const;
    ApiResult postBack() const;
    ApiResult postSpeak(const QString& text, const QString& mode) const;
    ApiResult getConfig() const;
    ApiResult postConfig(const QJsonDocument& doc) const;
    ApiResult getModelsStatus() const;
    ApiResult getDiagnostics() const;

    void setSocketPath(const QString& path);
    QString socketPath() const;
    void setTimeoutMs(int ms);
    int timeoutMs() const;

    static constexpr int kDefaultTimeoutMs = 5000;
    static constexpr int kMaxBodyBytes = 256 * 1024;

private:
    QString m_socketPath;
    int m_timeoutMs = kDefaultTimeoutMs;
};

}  // namespace lexaloud
