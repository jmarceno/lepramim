#include <QJsonDocument>
#include <QJsonObject>
#include <QtTest/QtTest>

#include "control_window.hpp"

using namespace lexaloud;

class TestControlWindow : public QObject {
    Q_OBJECT
private slots:
    void testVoiceMapping();
    void testLanguageMapping();
    void testSpeedMapping();
    void testSpeedHints();
    void testConfigJsonRoundtrip();
    void testOverlayToggle();
    void testUnknownVoiceShowsNote();
};

void TestControlWindow::testVoiceMapping() {
    ControlWindow w;
    // Default voice should be af_heart
    QComboBox* cb = w.voiceCombo();
    QVERIFY(cb != nullptr);
    QCOMPARE(cb->count(), kKokoroVoicesCount);
    // Verify first entry
    QCOMPARE(cb->itemData(0).toString(), QStringLiteral("af_heart"));
    // Load specific voice
    QJsonObject provider;
    provider[QStringLiteral("voice")] = QStringLiteral("bf_emma");
    provider[QStringLiteral("lang")] = QStringLiteral("en-gb");
    provider[QStringLiteral("speed")] = 1.25;
    QJsonObject root;
    root[QStringLiteral("provider")] = provider;
    QJsonDocument doc(root);
    w.loadFromJson(doc);
    QCOMPARE(w.selectedVoice(), QStringLiteral("bf_emma"));
    QCOMPARE(w.selectedLang(), QStringLiteral("en-gb"));
    QCOMPARE(w.speedFromSlider(), 1.25);
}

void TestControlWindow::testLanguageMapping() {
    ControlWindow w;
    QComboBox* lc = w.langCombo();
    QVERIFY(lc != nullptr);
    QCOMPARE(lc->count(), kLanguagesCount);
    QCOMPARE(lc->itemData(0).toString(), QStringLiteral("en-us"));
    QCOMPARE(lc->itemData(1).toString(), QStringLiteral("en-gb"));
}

void TestControlWindow::testSpeedMapping() {
    ControlWindow w;
    // Slider 50 -> 0.5
    w.setSpeedSliderFromValue(0.5);
    QCOMPARE(w.speedSlider()->value(), 50);
    QCOMPARE(w.speedFromSlider(), 0.5);

    w.setSpeedSliderFromValue(1.0);
    QCOMPARE(w.speedSlider()->value(), 100);
    QCOMPARE(w.speedFromSlider(), 1.0);

    w.setSpeedSliderFromValue(2.0);
    QCOMPARE(w.speedSlider()->value(), 200);
    QCOMPARE(w.speedFromSlider(), 2.0);

    // Clamp
    w.setSpeedSliderFromValue(0.1);
    QCOMPARE(w.speedSlider()->value(), 50);
    w.setSpeedSliderFromValue(5.0);
    QCOMPARE(w.speedSlider()->value(), 200);

    // Label updates
    w.setSpeedSliderFromValue(1.0);
    QVERIFY(w.speedValueLabel()->text().contains(QStringLiteral("1.00")));
    w.setSpeedSliderFromValue(1.5);
    QVERIFY(w.speedValueLabel()->text().contains(QStringLiteral("1.50")));
}

void TestControlWindow::testSpeedHints() {
    QString h1 = speedHintForValue(1.0);
    QVERIFY(h1.contains(QStringLiteral("safe"), Qt::CaseInsensitive));
    QString h2 = speedHintForValue(0.6);
    QVERIFY(h2.contains(QStringLiteral("slower"), Qt::CaseInsensitive));
    QString h3 = speedHintForValue(1.4);
    QVERIFY(h3.contains(QStringLiteral("familiar"), Qt::CaseInsensitive));
    QString h4 = speedHintForValue(1.9);
    QVERIFY(h4.contains(QStringLiteral("risky"), Qt::CaseInsensitive));
}

void TestControlWindow::testConfigJsonRoundtrip() {
    ControlWindow w;
    QJsonObject provider;
    provider[QStringLiteral("voice")] = QStringLiteral("am_adam");
    provider[QStringLiteral("lang")] = QStringLiteral("ja");
    provider[QStringLiteral("speed")] = 1.75;
    QJsonObject advanced;
    advanced[QStringLiteral("overlay")] = true;
    QJsonObject pre;
    pre[QStringLiteral("dedupe_mathjax_selection")] = false;
    pre[QStringLiteral("strip_markdown")] = false;
    QJsonObject root;
    root[QStringLiteral("provider")] = provider;
    root[QStringLiteral("advanced")] = advanced;
    root[QStringLiteral("preprocessor")] = pre;
    QJsonDocument doc(root);
    w.loadFromJson(doc);

    QJsonDocument out = w.currentConfigAsJson();
    QJsonObject outRoot = out.object();
    QCOMPARE(outRoot.value(QStringLiteral("provider"))
                 .toObject()
                 .value(QStringLiteral("voice"))
                 .toString(),
             QStringLiteral("am_adam"));
    QCOMPARE(outRoot.value(QStringLiteral("provider"))
                 .toObject()
                 .value(QStringLiteral("lang"))
                 .toString(),
             QStringLiteral("ja"));
    QCOMPARE(outRoot.value(QStringLiteral("advanced"))
                 .toObject()
                 .value(QStringLiteral("overlay"))
                 .toBool(),
             true);
    QCOMPARE(outRoot.value(QStringLiteral("preprocessor"))
                 .toObject()
                 .value(QStringLiteral("dedupe_mathjax_selection"))
                 .toBool(),
             false);
    // Speed double precision
    double sp = outRoot.value(QStringLiteral("provider"))
                    .toObject()
                    .value(QStringLiteral("speed"))
                    .toDouble();
    QVERIFY(qAbs(sp - 1.75) < 0.01);
}

void TestControlWindow::testOverlayToggle() {
    ControlWindow w;
    QJsonObject provider;
    provider[QStringLiteral("voice")] = QStringLiteral("af_heart");
    provider[QStringLiteral("lang")] = QStringLiteral("en-us");
    provider[QStringLiteral("speed")] = 1.0;
    QJsonObject adv;
    adv[QStringLiteral("overlay")] = true;
    QJsonObject root;
    root[QStringLiteral("provider")] = provider;
    root[QStringLiteral("advanced")] = adv;
    QJsonDocument doc(root);
    w.loadFromJson(doc);
    QVERIFY(w.overlayToggle()->isChecked());

    adv[QStringLiteral("overlay")] = false;
    root[QStringLiteral("advanced")] = adv;
    QJsonDocument doc2(root);
    w.loadFromJson(doc2);
    QVERIFY(!w.overlayToggle()->isChecked());

    // Current JSON respects toggle
    w.overlayToggle()->setChecked(true);
    QJsonDocument cur = w.currentConfigAsJson();
    QVERIFY(cur.object()
                .value(QStringLiteral("advanced"))
                .toObject()
                .value(QStringLiteral("overlay"))
                .toBool());
}

void TestControlWindow::testUnknownVoiceShowsNote() {
    ControlWindow w;
    QJsonObject provider;
    provider[QStringLiteral("voice")] = QStringLiteral("unknown_voice_xyz");
    provider[QStringLiteral("lang")] = QStringLiteral("en-us");
    provider[QStringLiteral("speed")] = 1.0;
    QJsonObject root;
    root[QStringLiteral("provider")] = provider;
    QJsonDocument doc(root);
    w.loadFromJson(doc);
    // Status label should contain note about unknown voice
    QVERIFY(w.statusLabel()->text().contains(QStringLiteral("unknown_voice_xyz")));
}

QTEST_MAIN(TestControlWindow)
#include "test_control_window.moc"
