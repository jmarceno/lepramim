#pragma once

#include <QCheckBox>
#include <QComboBox>
#include <QDoubleSpinBox>
#include <QGroupBox>
#include <QLabel>
#include <QPushButton>
#include <QSlider>
#include <QTabWidget>
#include <QWidget>

#include "api_client.hpp"

namespace lexaloud {

struct VoiceEntry {
    QString id;
    QString label;
};

struct LanguageEntry {
    QString id;
    QString label;
};

extern const VoiceEntry kKokoroVoices[];
extern const int kKokoroVoicesCount;
extern const LanguageEntry kLanguages[];
extern const int kLanguagesCount;

QString speedHintForValue(double speed);

class ControlWindow : public QWidget {
    Q_OBJECT
public:
    explicit ControlWindow(QWidget* parent = nullptr);
    ~ControlWindow() override;

    // Accessors for tests
    QComboBox* voiceCombo() const {
        return m_voiceCombo;
    }
    QComboBox* langCombo() const {
        return m_langCombo;
    }
    QSlider* speedSlider() const {
        return m_speedSlider;
    }
    QLabel* speedValueLabel() const {
        return m_speedValue;
    }
    QLabel* speedHintLabel() const {
        return m_speedHint;
    }
    QCheckBox* overlayToggle() const {
        return m_overlayToggle;
    }
    QCheckBox* dedupeMathJaxToggle() const {
        return m_dedupeToggle;
    }
    QCheckBox* stripMarkdownToggle() const {
        return m_stripMarkdownToggle;
    }
    QLabel* statusLabel() const {
        return m_statusLabel;
    }
    QPushButton* applyButton() const {
        return m_applyButton;
    }
    QPushButton* testSpeakButton() const {
        return m_testSpeakButton;
    }
    QTabWidget* tabWidget() const {
        return m_tabs;
    }

    void loadFromJson(const QJsonDocument& doc);
    QJsonDocument currentConfigAsJson() const;

    // Helpers exposed for testing value mapping
    double speedFromSlider() const;
    void setSpeedSliderFromValue(double speed);
    QString selectedVoice() const;
    QString selectedLang() const;

public slots:
    void refreshFromDaemon();
    void applySettings();

signals:
    void configSaved();

private slots:
    void onSpeedChanged(int value);
    void onTestSpeak();

private:
    void setupUi();
    void loadCurrentConfig();

    ApiClient m_client;
    QTabWidget* m_tabs = nullptr;

    // Voice tab
    QComboBox* m_voiceCombo = nullptr;
    QComboBox* m_langCombo = nullptr;
    QSlider* m_speedSlider = nullptr;
    QLabel* m_speedValue = nullptr;
    QLabel* m_speedHint = nullptr;

    // Preprocessor tab
    QCheckBox* m_dedupeToggle = nullptr;
    QCheckBox* m_stripMarkdownToggle = nullptr;
    QCheckBox* m_stripNumericCitationsToggle = nullptr;
    QCheckBox* m_expandLatinToggle = nullptr;
    QCheckBox* m_normalizeNumbersToggle = nullptr;

    // Advanced tab
    QCheckBox* m_overlayToggle = nullptr;

    // Common
    QLabel* m_statusLabel = nullptr;
    QPushButton* m_applyButton = nullptr;
    QPushButton* m_testSpeakButton = nullptr;
    QPushButton* m_closeButton = nullptr;
};

}  // namespace lexaloud
