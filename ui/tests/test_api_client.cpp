#include <QtTest/QtTest>

#include "api_client.hpp"

using namespace lexaloud;

class TestApiClient : public QObject {
    Q_OBJECT
private slots:
    void testValidJson();
    void testValidHealthz();
    void testTruncated();
    void testMalformed();
    void testDaemonDown();
    void testBodyTooLarge();
    void testBuildRequest();
    void testParseEmpty();
    void testStatusCodes();
};

void TestApiClient::testValidJson() {
    QByteArray raw =
        "HTTP/1.1 200 OK\r\n"
        "Content-Type: application/json\r\n"
        "Content-Length: 62\r\n"
        "\r\n"
        "{\"state\":\"speaking\",\"current_sentence\":\"hello\",\"pending_count\":1}";
    ApiResult r = ApiClient::parseResponse(raw);
    QCOMPARE(r.statusCode, 200);
    QVERIFY(r.errorString.isEmpty());
    QVERIFY(r.json.isObject());
    QCOMPARE(r.json.object().value(QStringLiteral("state")).toString(), QStringLiteral("speaking"));
}

void TestApiClient::testValidHealthz() {
    QByteArray raw =
        "HTTP/1.1 200 OK\r\n"
        "Content-Length: 15\r\n"
        "\r\n"
        "{\"status\":\"ok\"}";
    ApiResult r = ApiClient::parseResponse(raw);
    QCOMPARE(r.statusCode, 200);
    QVERIFY(r.errorString.isEmpty());
    QVERIFY(r.json.isObject());
}

void TestApiClient::testTruncated() {
    // Header says 62 but body only 10
    QByteArray raw =
        "HTTP/1.1 200 OK\r\n"
        "Content-Length: 62\r\n"
        "\r\n"
        "{\"state\":";
    ApiResult r = ApiClient::parseResponse(raw);
    QVERIFY(!r.errorString.isEmpty());
    QVERIFY(r.errorString.contains(QStringLiteral("truncated"), Qt::CaseInsensitive));
}

void TestApiClient::testMalformed() {
    // Missing header terminator
    QByteArray raw = "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n{\"bad\"}";
    ApiResult r = ApiClient::parseResponse(raw);
    QVERIFY(!r.errorString.isEmpty());

    // Invalid JSON body on 200
    QByteArray raw2 =
        "HTTP/1.1 200 OK\r\n"
        "Content-Length: 6\r\n"
        "\r\n"
        "{bad:}";
    ApiResult r2 = ApiClient::parseResponse(raw2);
    QVERIFY(!r2.errorString.isEmpty());
    QVERIFY(r2.errorString.contains(QStringLiteral("malformed"), Qt::CaseInsensitive));

    // Malformed status line
    QByteArray raw3 = "BADLINE\r\n\r\n{}";
    ApiResult r3 = ApiClient::parseResponse(raw3);
    QVERIFY(!r3.errorString.isEmpty());
}

void TestApiClient::testDaemonDown() {
    // Use an impossible socket path
    ApiClient client(QStringLiteral("/tmp/lexaloud-nonexistent-12345/lexaloud.sock"));
    client.setTimeoutMs(300);
    ApiResult r = client.getState();
    QVERIFY(!r.errorString.isEmpty());
    QCOMPARE(r.statusCode, 0);
    QVERIFY(r.isDaemonDown());
}

void TestApiClient::testBodyTooLarge() {
    // Build a body larger than 256KB
    QByteArray big(300 * 1024, 'x');
    QByteArray raw =
        "HTTP/1.1 200 OK\r\nContent-Length: " + QByteArray::number(big.size()) + "\r\n\r\n" + big;
    ApiResult r = ApiClient::parseResponse(raw);
    QVERIFY(!r.errorString.isEmpty());
    QVERIFY(r.errorString.contains(QStringLiteral("256KB"), Qt::CaseInsensitive));
}

void TestApiClient::testBuildRequest() {
    QByteArray req = ApiClient::buildRequest(QStringLiteral("GET"), QStringLiteral("/state"));
    QVERIFY(req.contains("GET /state"));
    QVERIFY(req.contains("Host: lexaloud"));

    QByteArray body = "{\"text\":\"hi\"}";
    QByteArray post =
        ApiClient::buildRequest(QStringLiteral("POST"), QStringLiteral("/speak"), body);
    QVERIFY(post.contains("POST /speak"));
    QVERIFY(post.contains("Content-Length:"));
    QVERIFY(post.contains(body));
}

void TestApiClient::testParseEmpty() {
    QByteArray raw;
    ApiResult r = ApiClient::parseResponse(raw);
    QVERIFY(!r.errorString.isEmpty());

    QByteArray raw2 = "HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n";
    ApiResult r2 = ApiClient::parseResponse(raw2);
    QCOMPARE(r2.statusCode, 204);
    QVERIFY(r2.errorString.isEmpty());
}

void TestApiClient::testStatusCodes() {
    QByteArray raw404 =
        "HTTP/1.1 404 Not Found\r\n"
        "Content-Length: 22\r\n"
        "\r\n"
        "{\"detail\":\"not found\"}";
    ApiResult r = ApiClient::parseResponse(raw404);
    QCOMPARE(r.statusCode, 404);
    // For error codes, JSON may still be parsed
    QVERIFY(r.json.isObject() || !r.errorString.isEmpty());

    QByteArray raw413 =
        "HTTP/1.1 413 Payload Too Large\r\n"
        "Content-Length: 0\r\n"
        "\r\n";
    ApiResult r2 = ApiClient::parseResponse(raw413);
    QCOMPARE(r2.statusCode, 413);
}

QTEST_MAIN(TestApiClient)
#include "test_api_client.moc"
