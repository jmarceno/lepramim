#include "control_window.hpp"

#include <QFormLayout>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMessageBox>
#include <QVBoxLayout>

namespace lexaloud {

const VoiceEntry kKokoroVoices[] = {
    {QStringLiteral("af_heart"), QStringLiteral("Heart \u2014 American female, warm (default)")},
    {QStringLiteral("af_alloy"), QStringLiteral("Alloy \u2014 American female")},
    {QStringLiteral("af_aoede"), QStringLiteral("Aoede \u2014 American female")},
    {QStringLiteral("af_bella"), QStringLiteral("Bella \u2014 American female")},
    {QStringLiteral("af_jessica"), QStringLiteral("Jessica \u2014 American female")},
    {QStringLiteral("af_kore"), QStringLiteral("Kore \u2014 American female")},
    {QStringLiteral("af_nicole"), QStringLiteral("Nicole \u2014 American female")},
    {QStringLiteral("af_nova"), QStringLiteral("Nova \u2014 American female")},
    {QStringLiteral("af_river"), QStringLiteral("River \u2014 American female")},
    {QStringLiteral("af_sarah"), QStringLiteral("Sarah \u2014 American female")},
    {QStringLiteral("af_sky"), QStringLiteral("Sky \u2014 American female")},
    {QStringLiteral("am_adam"), QStringLiteral("Adam \u2014 American male")},
    {QStringLiteral("am_echo"), QStringLiteral("Echo \u2014 American male")},
    {QStringLiteral("am_eric"), QStringLiteral("Eric \u2014 American male")},
    {QStringLiteral("am_fenrir"), QStringLiteral("Fenrir \u2014 American male")},
    {QStringLiteral("am_liam"), QStringLiteral("Liam \u2014 American male")},
    {QStringLiteral("am_michael"), QStringLiteral("Michael \u2014 American male")},
    {QStringLiteral("am_onyx"), QStringLiteral("Onyx \u2014 American male")},
    {QStringLiteral("am_puck"), QStringLiteral("Puck \u2014 American male")},
    {QStringLiteral("am_santa"), QStringLiteral("Santa \u2014 American male")},
    {QStringLiteral("bf_alice"), QStringLiteral("Alice \u2014 British female")},
    {QStringLiteral("bf_emma"), QStringLiteral("Emma \u2014 British female")},
    {QStringLiteral("bf_isabella"), QStringLiteral("Isabella \u2014 British female")},
    {QStringLiteral("bf_lily"), QStringLiteral("Lily \u2014 British female")},
    {QStringLiteral("bm_daniel"), QStringLiteral("Daniel \u2014 British male")},
    {QStringLiteral("bm_fable"), QStringLiteral("Fable \u2014 British male")},
    {QStringLiteral("bm_george"), QStringLiteral("George \u2014 British male")},
    {QStringLiteral("bm_lewis"), QStringLiteral("Lewis \u2014 British male")},
    {QStringLiteral("ef_dora"), QStringLiteral("Dora \u2014 Spanish female")},
    {QStringLiteral("em_alex"), QStringLiteral("Alex \u2014 Spanish male")},
    {QStringLiteral("em_santa"), QStringLiteral("Santa \u2014 Spanish male")},
    {QStringLiteral("ff_siwis"), QStringLiteral("Siwis \u2014 French female")},
    {QStringLiteral("hf_alpha"), QStringLiteral("Alpha \u2014 Hindi female")},
    {QStringLiteral("hf_beta"), QStringLiteral("Beta \u2014 Hindi female")},
    {QStringLiteral("hm_omega"), QStringLiteral("Omega \u2014 Hindi male")},
    {QStringLiteral("hm_psi"), QStringLiteral("Psi \u2014 Hindi male")},
    {QStringLiteral("if_sara"), QStringLiteral("Sara \u2014 Italian female")},
    {QStringLiteral("im_nicola"), QStringLiteral("Nicola \u2014 Italian male")},
    {QStringLiteral("jf_alpha"), QStringLiteral("Alpha \u2014 Japanese female")},
    {QStringLiteral("jf_gongitsune"), QStringLiteral("Gongitsune \u2014 Japanese female")},
    {QStringLiteral("jf_nezumi"), QStringLiteral("Nezumi \u2014 Japanese female")},
    {QStringLiteral("jf_tebukuro"), QStringLiteral("Tebukuro \u2014 Japanese female")},
    {QStringLiteral("jm_kumo"), QStringLiteral("Kumo \u2014 Japanese male")},
    {QStringLiteral("pf_dora"), QStringLiteral("Dora \u2014 Brazilian Portuguese female")},
    {QStringLiteral("pm_alex"), QStringLiteral("Alex \u2014 Brazilian Portuguese male")},
    {QStringLiteral("pm_santa"), QStringLiteral("Santa \u2014 Brazilian Portuguese male")},
    {QStringLiteral("zf_xiaobei"), QStringLiteral("Xiaobei \u2014 Mandarin Chinese female")},
    {QStringLiteral("zf_xiaoni"), QStringLiteral("Xiaoni \u2014 Mandarin Chinese female")},
    {QStringLiteral("zf_xiaoxiao"), QStringLiteral("Xiaoxiao \u2014 Mandarin Chinese female")},
    {QStringLiteral("zf_xiaoyi"), QStringLiteral("Xiaoyi \u2014 Mandarin Chinese female")},
    {QStringLiteral("zm_yunjian"), QStringLiteral("Yunjian \u2014 Mandarin Chinese male")},
    {QStringLiteral("zm_yunxi"), QStringLiteral("Yunxi \u2014 Mandarin Chinese male")},
    {QStringLiteral("zm_yunxia"), QStringLiteral("Yunxia \u2014 Mandarin Chinese male")},
    {QStringLiteral("zm_yunyang"), QStringLiteral("Yunyang \u2014 Mandarin Chinese male")},
};
const int kKokoroVoicesCount = static_cast<int>(sizeof(kKokoroVoices) / sizeof(kKokoroVoices[0]));

const LanguageEntry kLanguages[] = {
    {QStringLiteral("en-us"), QStringLiteral("English (US)")},
    {QStringLiteral("en-gb"), QStringLiteral("English (UK)")},
    {QStringLiteral("es"), QStringLiteral("Spanish")},
    {QStringLiteral("fr-fr"), QStringLiteral("French")},
    {QStringLiteral("hi"), QStringLiteral("Hindi")},
    {QStringLiteral("it"), QStringLiteral("Italian")},
    {QStringLiteral("ja"), QStringLiteral("Japanese")},
    {QStringLiteral("pt-br"), QStringLiteral("Portuguese (Brazil)")},
    {QStringLiteral("zh"), QStringLiteral("Chinese (Mandarin)")},
};
const int kLanguagesCount = static_cast<int>(sizeof(kLanguages) / sizeof(kLanguages[0]));

QString speedHintForValue(double speed) {
    if (speed >= 0.85 && speed <= 1.3) {
        return QStringLiteral("%1\u00d7 \u2014 safe range for dense reading.")
            .arg(speed, 0, 'f', 2);
    }
    if (speed < 0.85) {
        return QStringLiteral("%1\u00d7 \u2014 slower than natural; may feel dragged.")
            .arg(speed, 0, 'f', 2);
    }
    if (speed <= 1.5) {
        return QStringLiteral(
                   "%1\u00d7 \u2014 fine for familiar material, may strain comprehension on new "
                   "dense text.")
            .arg(speed, 0, 'f', 2);
    }
    return QStringLiteral(
               "%1\u00d7 \u2014 risky for unfamiliar academic material; comprehension drops.")
        .arg(speed, 0, 'f', 2);
}

ControlWindow::ControlWindow(QWidget* parent) : QWidget(parent) {
    setWindowTitle(QStringLiteral("Lexaloud \u2014 Control"));
    setMinimumSize(520, 480);
    setupUi();
    loadCurrentConfig();
}

ControlWindow::~ControlWindow() = default;

void ControlWindow::setupUi() {
    auto* outer = new QVBoxLayout(this);
    outer->setContentsMargins(16, 16, 16, 16);
    outer->setSpacing(12);

    m_tabs = new QTabWidget(this);

    // Voices tab
    auto* voiceTab = new QWidget();
    auto* voiceLayout = new QVBoxLayout(voiceTab);
    voiceLayout->setContentsMargins(12, 8, 12, 12);
    voiceLayout->setSpacing(8);

    auto* voiceFrame = new QGroupBox(QStringLiteral("Voice"), voiceTab);
    auto* voiceBox = new QVBoxLayout(voiceFrame);
    voiceBox->setContentsMargins(12, 8, 12, 12);
    voiceBox->setSpacing(8);

    m_voiceCombo = new QComboBox(voiceFrame);
    for (int i = 0; i < kKokoroVoicesCount; ++i) {
        m_voiceCombo->addItem(kKokoroVoices[i].label, kKokoroVoices[i].id);
    }
    voiceBox->addWidget(m_voiceCombo);

    voiceBox->addWidget(new QLabel(QStringLiteral("Language"), voiceFrame));
    m_langCombo = new QComboBox(voiceFrame);
    for (int i = 0; i < kLanguagesCount; ++i) {
        m_langCombo->addItem(kLanguages[i].label, kLanguages[i].id);
    }
    voiceBox->addWidget(m_langCombo);

    auto* speedRow = new QHBoxLayout();
    speedRow->addWidget(new QLabel(QStringLiteral("Speed"), voiceFrame));
    m_speedSlider = new QSlider(Qt::Horizontal, voiceFrame);
    m_speedSlider->setRange(50, 200);
    m_speedSlider->setSingleStep(5);
    m_speedSlider->setPageStep(10);
    m_speedSlider->setTickPosition(QSlider::TicksBelow);
    m_speedSlider->setTickInterval(25);
    speedRow->addWidget(m_speedSlider, 1);
    m_speedValue = new QLabel(QStringLiteral("1.00\u00d7"), voiceFrame);
    speedRow->addWidget(m_speedValue);
    voiceBox->addLayout(speedRow);

    m_speedHint = new QLabel(QString(), voiceFrame);
    m_speedHint->setStyleSheet(QStringLiteral("color: gray;"));
    m_speedHint->setWordWrap(true);
    voiceBox->addWidget(m_speedHint);
    connect(m_speedSlider, &QSlider::valueChanged, this, &ControlWindow::onSpeedChanged);

    voiceLayout->addWidget(voiceFrame);
    voiceLayout->addStretch(1);

    // Preprocessor tab
    auto* preTab = new QWidget();
    auto* preLayout = new QVBoxLayout(preTab);
    preLayout->setContentsMargins(12, 12, 12, 12);
    preLayout->setSpacing(8);
    auto* preGroup = new QGroupBox(QStringLiteral("Preprocessor"), preTab);
    auto* preBox = new QVBoxLayout(preGroup);
    m_dedupeToggle = new QCheckBox(QStringLiteral("Deduplicate MathJax selection"), preGroup);
    m_dedupeToggle->setChecked(true);
    preBox->addWidget(m_dedupeToggle);
    m_stripMarkdownToggle = new QCheckBox(QStringLiteral("Strip Markdown"), preGroup);
    m_stripMarkdownToggle->setChecked(true);
    preBox->addWidget(m_stripMarkdownToggle);
    m_stripNumericCitationsToggle =
        new QCheckBox(QStringLiteral("Strip numeric bracket citations"), preGroup);
    m_stripNumericCitationsToggle->setChecked(true);
    preBox->addWidget(m_stripNumericCitationsToggle);
    m_expandLatinToggle = new QCheckBox(QStringLiteral("Expand Latin abbreviations"), preGroup);
    m_expandLatinToggle->setChecked(true);
    preBox->addWidget(m_expandLatinToggle);
    m_normalizeNumbersToggle = new QCheckBox(QStringLiteral("Normalize numbers"), preGroup);
    m_normalizeNumbersToggle->setChecked(true);
    preBox->addWidget(m_normalizeNumbersToggle);
    preLayout->addWidget(preGroup);
    preLayout->addStretch(1);

    // Config/Advanced tab
    auto* advTab = new QWidget();
    auto* advLayout = new QVBoxLayout(advTab);
    advLayout->setContentsMargins(12, 12, 12, 12);
    advLayout->setSpacing(8);
    auto* advGroup = new QGroupBox(QStringLiteral("Advanced"), advTab);
    auto* advBox = new QVBoxLayout(advGroup);
    m_overlayToggle =
        new QCheckBox(QStringLiteral("Show floating overlay when speaking"), advGroup);
    m_overlayToggle->setToolTip(
        QStringLiteral("Displays a small translucent bar at the bottom of the screen "
                       "showing the current sentence with pause/skip/stop buttons."));
    advBox->addWidget(m_overlayToggle);
    advLayout->addWidget(advGroup);
    advLayout->addStretch(1);

    // Model status tab
    auto* modelTab = new QWidget();
    auto* modelLayout = new QVBoxLayout(modelTab);
    modelLayout->setContentsMargins(12, 12, 12, 12);
    auto* modelGroup = new QGroupBox(QStringLiteral("Model status"), modelTab);
    auto* modelBox = new QVBoxLayout(modelGroup);
    auto* modelLabel = new QLabel(
        QStringLiteral("Model artifacts are managed by the Rust daemon. Use diagnostics."),
        modelGroup);
    modelLabel->setWordWrap(true);
    modelBox->addWidget(modelLabel);
    modelLayout->addWidget(modelGroup);
    modelLayout->addStretch(1);

    m_tabs->addTab(voiceTab, QStringLiteral("Voice"));
    m_tabs->addTab(preTab, QStringLiteral("Preprocessor"));
    m_tabs->addTab(advTab, QStringLiteral("Advanced"));
    m_tabs->addTab(modelTab, QStringLiteral("Models"));

    outer->addWidget(m_tabs, 1);

    // Test speak row
    auto* testRow = new QHBoxLayout();
    auto* testLabel = new QLabel(QStringLiteral("Test speak:"), this);
    testRow->addWidget(testLabel);
    m_testSpeakButton = new QPushButton(QStringLiteral("Speak test sentence"), this);
    connect(m_testSpeakButton, &QPushButton::clicked, this, &ControlWindow::onTestSpeak);
    testRow->addWidget(m_testSpeakButton);
    testRow->addStretch(1);
    outer->addLayout(testRow);

    m_statusLabel = new QLabel(QString(), this);
    m_statusLabel->setStyleSheet(QStringLiteral("color: gray;"));
    m_statusLabel->setWordWrap(true);
    outer->addWidget(m_statusLabel);

    auto* buttonBox = new QHBoxLayout();
    buttonBox->addStretch(1);
    m_applyButton = new QPushButton(QStringLiteral("Apply settings"), this);
    connect(m_applyButton, &QPushButton::clicked, this, &ControlWindow::applySettings);
    buttonBox->addWidget(m_applyButton);

    m_closeButton = new QPushButton(QStringLiteral("Close"), this);
    connect(m_closeButton, &QPushButton::clicked, this, &QWidget::close);
    buttonBox->addWidget(m_closeButton);
    outer->addLayout(buttonBox);
}

void ControlWindow::loadCurrentConfig() {
    ApiResult r = m_client.getConfig();
    if (r.isSuccess() && r.json.isObject()) {
        loadFromJson(r.json);
        return;
    }
    // Fallback defaults
    m_voiceCombo->setCurrentIndex(m_voiceCombo->findData(QStringLiteral("af_heart")));
    m_langCombo->setCurrentIndex(m_langCombo->findData(QStringLiteral("en-us")));
    m_speedSlider->setValue(100);
    onSpeedChanged(100);
    m_overlayToggle->setChecked(false);
}

void ControlWindow::loadFromJson(const QJsonDocument& doc) {
    QJsonObject root = doc.object();
    QJsonObject provider = root.value(QStringLiteral("provider")).toObject();
    QString voice = provider.value(QStringLiteral("voice")).toString(QStringLiteral("af_heart"));
    QString lang = provider.value(QStringLiteral("lang")).toString(QStringLiteral("en-us"));
    double speed = provider.value(QStringLiteral("speed")).toDouble(1.0);

    int vi = m_voiceCombo->findData(voice);
    if (vi >= 0) {
        m_voiceCombo->setCurrentIndex(vi);
        m_statusLabel->setText(QString());
    } else {
        m_statusLabel->setText(
            QStringLiteral("Note: current voice '%1' is outside the curated list; "
                           "edit ~/.config/lexaloud/config.toml directly to keep it.")
                .arg(voice));
    }
    int li = m_langCombo->findData(lang);
    m_langCombo->setCurrentIndex(li >= 0 ? li : 0);

    double clamped = qBound(0.5, speed, 2.0);
    m_speedSlider->setValue(qRound(clamped * 100.0));
    onSpeedChanged(m_speedSlider->value());

    QJsonObject advanced = root.value(QStringLiteral("advanced")).toObject();
    bool overlay = advanced.value(QStringLiteral("overlay")).toBool(false);
    m_overlayToggle->setChecked(overlay);

    QJsonObject pre = root.value(QStringLiteral("preprocessor")).toObject();
    if (pre.contains(QStringLiteral("dedupe_mathjax_selection"))) {
        m_dedupeToggle->setChecked(pre.value(QStringLiteral("dedupe_mathjax_selection")).toBool());
    }
    if (pre.contains(QStringLiteral("strip_markdown"))) {
        m_stripMarkdownToggle->setChecked(pre.value(QStringLiteral("strip_markdown")).toBool());
    }
    if (pre.contains(QStringLiteral("strip_numeric_bracket_citations"))) {
        m_stripNumericCitationsToggle->setChecked(
            pre.value(QStringLiteral("strip_numeric_bracket_citations")).toBool());
    }
    if (pre.contains(QStringLiteral("expand_latin_abbreviations"))) {
        m_expandLatinToggle->setChecked(
            pre.value(QStringLiteral("expand_latin_abbreviations")).toBool());
    }
    if (pre.contains(QStringLiteral("normalize_numbers"))) {
        m_normalizeNumbersToggle->setChecked(
            pre.value(QStringLiteral("normalize_numbers")).toBool());
    }
}

QJsonDocument ControlWindow::currentConfigAsJson() const {
    QJsonObject provider;
    provider[QStringLiteral("voice")] = selectedVoice();
    provider[QStringLiteral("lang")] = selectedLang();
    provider[QStringLiteral("speed")] = speedFromSlider();

    QJsonObject advanced;
    advanced[QStringLiteral("overlay")] = m_overlayToggle->isChecked();

    QJsonObject pre;
    pre[QStringLiteral("dedupe_mathjax_selection")] = m_dedupeToggle->isChecked();
    pre[QStringLiteral("strip_markdown")] = m_stripMarkdownToggle->isChecked();
    pre[QStringLiteral("strip_numeric_bracket_citations")] =
        m_stripNumericCitationsToggle->isChecked();
    pre[QStringLiteral("expand_latin_abbreviations")] = m_expandLatinToggle->isChecked();
    pre[QStringLiteral("normalize_numbers")] = m_normalizeNumbersToggle->isChecked();

    QJsonObject root;
    root[QStringLiteral("provider")] = provider;
    root[QStringLiteral("advanced")] = advanced;
    root[QStringLiteral("preprocessor")] = pre;
    return QJsonDocument(root);
}

double ControlWindow::speedFromSlider() const {
    return qRound(m_speedSlider->value() / 5.0) * 5.0 / 100.0;
}

void ControlWindow::setSpeedSliderFromValue(double speed) {
    double clamped = qBound(0.5, speed, 2.0);
    m_speedSlider->setValue(qRound(clamped * 100.0));
}

QString ControlWindow::selectedVoice() const {
    return m_voiceCombo->currentData().toString();
}

QString ControlWindow::selectedLang() const {
    return m_langCombo->currentData().toString();
}

void ControlWindow::onSpeedChanged(int value) {
    double v = value / 100.0;
    m_speedValue->setText(QStringLiteral("%1\u00d7").arg(v, 0, 'f', 2));
    m_speedHint->setText(speedHintForValue(v));
}

void ControlWindow::applySettings() {
    QString voice = selectedVoice();
    QString lang = selectedLang();
    if (voice.isEmpty() || lang.isEmpty()) {
        m_statusLabel->setText(QStringLiteral("Pick a voice and a language first."));
        return;
    }
    QJsonDocument doc = currentConfigAsJson();
    ApiResult r = m_client.postConfig(doc);
    if (!r.isSuccess()) {
        m_statusLabel->setText(QStringLiteral("Saving config failed: %1").arg(r.errorString));
        return;
    }
    double speed = speedFromSlider();
    QString summary =
        QStringLiteral("voice=%1, lang=%2, speed=%3\u00d7").arg(voice, lang).arg(speed, 0, 'f', 2);
    m_statusLabel->setText(
        QStringLiteral("Saved %1; it applies on the next playback start.").arg(summary));
    emit configSaved();
}

void ControlWindow::refreshFromDaemon() {
    loadCurrentConfig();
}

void ControlWindow::onTestSpeak() {
    ApiResult r = m_client.postSpeak(QStringLiteral("Hello from Lexaloud. This is a test."),
                                     QStringLiteral("replace"));
    if (r.isSuccess()) {
        m_statusLabel->setText(QStringLiteral("Test speak sent."));
    } else {
        m_statusLabel->setText(QStringLiteral("Test speak failed: %1").arg(r.errorString));
    }
}

}  // namespace lexaloud
