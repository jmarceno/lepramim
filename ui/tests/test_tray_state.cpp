#include <QtTest/QtTest>

#include "tray.hpp"

using namespace lexaloud;

class TestTrayState : public QObject {
    Q_OBJECT
private slots:
    void testIdle();
    void testWarming();
    void testSpeaking();
    void testPaused();
    void testPureFunction();
    void testTrayApply();
    void testActionOrder();
};

void TestTrayState::testIdle() {
    TrayActionState s = trayStateForDaemon(QStringLiteral("idle"), false);
    QVERIFY(!s.isActive);
    QVERIFY(!s.isWarming);
    QVERIFY(!s.speakEnabled);
    QVERIFY(!s.pauseEnabled);
    QVERIFY(!s.stopEnabled);
    QCOMPARE(s.iconState, QStringLiteral("stopped"));
    QCOMPARE(s.tooltip, QStringLiteral("Lexaloud: stopped"));
    QCOMPARE(s.toggleLabel, QStringLiteral("Start daemon"));
}

void TestTrayState::testWarming() {
    TrayActionState s = trayStateForDaemon(QStringLiteral("warming"), true);
    QVERIFY(s.isActive);
    QVERIFY(s.isWarming);
    QVERIFY(!s.speakEnabled);
    QVERIFY(!s.pauseEnabled);
    QVERIFY(!s.stopEnabled);
    QCOMPARE(s.iconState, QStringLiteral("running"));
    QCOMPARE(s.tooltip, QStringLiteral("Lexaloud: warming up"));
    QVERIFY(s.toggleLabel.contains(QStringLiteral("warming")));
}

void TestTrayState::testSpeaking() {
    TrayActionState s = trayStateForDaemon(QStringLiteral("speaking"), true);
    QVERIFY(s.isActive);
    QVERIFY(!s.isWarming);
    QVERIFY(s.speakEnabled);
    QVERIFY(s.pauseEnabled);
    QVERIFY(s.stopEnabled);
    QCOMPARE(s.iconState, QStringLiteral("running"));
    QCOMPARE(s.tooltip, QStringLiteral("Lexaloud: running"));
    QCOMPARE(s.toggleLabel, QStringLiteral("Stop daemon"));
}

void TestTrayState::testPaused() {
    TrayActionState s = trayStateForDaemon(QStringLiteral("paused"), true);
    QVERIFY(s.isActive);
    QVERIFY(!s.isWarming);
    QVERIFY(s.speakEnabled);
    QVERIFY(s.pauseEnabled);
    QVERIFY(s.stopEnabled);
    QCOMPARE(s.iconState, QStringLiteral("running"));
}

void TestTrayState::testPureFunction() {
    // Empty state string when inactive => stopped
    TrayActionState s1 = trayStateForDaemon(QString(), false);
    QCOMPARE(s1.iconState, QStringLiteral("stopped"));
    // Unknown state but active => running but ready
    TrayActionState s2 = trayStateForDaemon(QStringLiteral("unknown"), true);
    QVERIFY(s2.isActive);
    QCOMPARE(s2.iconState, QStringLiteral("running"));
    QVERIFY(s2.speakEnabled);
}

void TestTrayState::testTrayApply() {
    Tray tray(QStringLiteral(""));  // NOLINT
    // Idle
    tray.applyDaemonState(QStringLiteral("idle"), false);
    QVERIFY(!tray.actionSpeak()->isEnabled());
    QVERIFY(!tray.actionPause()->isEnabled());
    QVERIFY(!tray.actionStop()->isEnabled());
    QCOMPARE(tray.actionToggleDaemon()->text(), QStringLiteral("Start daemon"));

    // Speaking
    tray.applyDaemonState(QStringLiteral("speaking"), true);
    QVERIFY(tray.actionSpeak()->isEnabled());
    QVERIFY(tray.actionPause()->isEnabled());
    QVERIFY(tray.actionStop()->isEnabled());
    QCOMPARE(tray.actionToggleDaemon()->text(), QStringLiteral("Stop daemon"));

    // Warming disables playback
    tray.applyDaemonState(QStringLiteral("warming"), true);
    QVERIFY(!tray.actionSpeak()->isEnabled());
    QVERIFY(!tray.actionPause()->isEnabled());
    QVERIFY(!tray.actionStop()->isEnabled());
    QVERIFY(tray.actionToggleDaemon()->text().contains(QStringLiteral("warming")));

    // Paused enables
    tray.applyDaemonState(QStringLiteral("paused"), true);
    QVERIFY(tray.actionSpeak()->isEnabled());
    QVERIFY(tray.actionPause()->isEnabled());
    QVERIFY(tray.actionStop()->isEnabled());
}

void TestTrayState::testActionOrder() {
    Tray tray(QStringLiteral(""));  // NOLINT
    QList<QAction*> actions = tray.contextMenu()->actions();
    // Expected order: shortcut, separator, toggle, speak, pause, stop, separator, control,
    // autostart, quit Separators are QAction with isSeparator()
    int idx = 0;
    QCOMPARE(actions.at(idx++)->text(), QStringLiteral("Shortcut: Meta+R"));
    QVERIFY(actions.at(idx++)->isSeparator());
    QCOMPARE(actions.at(idx++)->text(), QStringLiteral("Start daemon"));  // initial idle label
    QCOMPARE(actions.at(idx++)->text(), QStringLiteral("Speak highlighted selection"));
    QCOMPARE(actions.at(idx++)->text(), QStringLiteral("Pause / resume"));
    QCOMPARE(actions.at(idx++)->text(), QStringLiteral("Stop current playback"));
    QVERIFY(actions.at(idx++)->isSeparator());
    QCOMPARE(actions.at(idx++)->text(), QStringLiteral("Control window\u2026"));
    QCOMPARE(actions.at(idx++)->text(), QStringLiteral("Start with desktop"));
    QCOMPARE(actions.at(idx++)->text(), QStringLiteral("Quit Lexaloud"));
    QCOMPARE(actions.size(), idx);
}

QTEST_MAIN(TestTrayState)
#include "test_tray_state.moc"
