#include <QtTest/QtTest>

#include "capture.hpp"

using namespace lexaloud;

class TestCapture : public QObject {
    Q_OBJECT
private slots:
    void prefersSelectionOverClipboard();
    void usesUpdatedClipboardWhenSelectionEmpty();
    void fallsBackToUnchangedClipboard();
    void emptyWhenNothingPresent();
    void trimsWhitespace();
};

void TestCapture::prefersSelectionOverClipboard() {
    SelectionCapture cap = resolveCapture(QStringLiteral(" highlighted "), QStringLiteral("old"),
                                          QStringLiteral("new"));
    QCOMPARE(cap.text, QStringLiteral("highlighted"));
    QCOMPARE(cap.source, QStringLiteral("primary/qt"));
}

void TestCapture::usesUpdatedClipboardWhenSelectionEmpty() {
    SelectionCapture cap =
        resolveCapture(QString(), QStringLiteral("old clip"), QStringLiteral("fresh copy"));
    QCOMPARE(cap.text, QStringLiteral("fresh copy"));
    QCOMPARE(cap.source, QStringLiteral("clipboard/updated"));
}

void TestCapture::fallsBackToUnchangedClipboard() {
    SelectionCapture cap =
        resolveCapture(QString(), QStringLiteral("already copied"), QStringLiteral("already copied"));
    QCOMPARE(cap.text, QStringLiteral("already copied"));
    QCOMPARE(cap.source, QStringLiteral("clipboard"));
}

void TestCapture::emptyWhenNothingPresent() {
    SelectionCapture cap = resolveCapture(QStringLiteral("  "), QString(), QString());
    QVERIFY(cap.text.isEmpty());
}

void TestCapture::trimsWhitespace() {
    SelectionCapture cap =
        resolveCapture(QString(), QStringLiteral(" before\n"), QStringLiteral(" after \t"));
    QCOMPARE(cap.text, QStringLiteral("after"));
    QCOMPARE(cap.source, QStringLiteral("clipboard/updated"));
}

QTEST_GUILESS_MAIN(TestCapture)
#include "test_capture.moc"
