#pragma once

#include <QDialog>
#include <QLabel>
#include <QProgressBar>
#include <QPushButton>
#include <QTimer>

namespace lexaloud {

class OnboardingDialog : public QDialog {
    Q_OBJECT
public:
    explicit OnboardingDialog(QWidget* parent = nullptr);
    ~OnboardingDialog() override;

    void setStatusText(const QString& text);
    void setProgress(int percent);
    void setFileLabel(const QString& text);

signals:
    void onboardingAccepted();
    void onboardingRejected();

private slots:
    void onContinue();
    void onSkip();

private:
    void setupUi();

    QLabel* m_titleLabel = nullptr;
    QLabel* m_statusLabel = nullptr;
    QProgressBar* m_progress = nullptr;
    QLabel* m_fileLabel = nullptr;
    QPushButton* m_continueButton = nullptr;
    QPushButton* m_skipButton = nullptr;
};

}  // namespace lexaloud
