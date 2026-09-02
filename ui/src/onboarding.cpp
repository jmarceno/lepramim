#include "onboarding.hpp"

#include <QHBoxLayout>
#include <QVBoxLayout>

namespace lexaloud {

OnboardingDialog::OnboardingDialog(QWidget* parent) : QDialog(parent) {
    setWindowTitle(QStringLiteral("Lexaloud \u2014 preparing speech"));
    setMinimumWidth(420);
    setModal(true);
    setupUi();
}

OnboardingDialog::~OnboardingDialog() = default;

void OnboardingDialog::setupUi() {
    auto* layout = new QVBoxLayout(this);
    layout->setSpacing(8);

    m_titleLabel = new QLabel(QStringLiteral("Welcome to Lexaloud"), this);
    QFont f = m_titleLabel->font();
    f.setBold(true);
    f.setPointSize(f.pointSize() + 2);
    m_titleLabel->setFont(f);
    layout->addWidget(m_titleLabel);

    m_statusLabel = new QLabel(QStringLiteral("Downloading the Kokoro speech model\u2026"), this);
    m_statusLabel->setWordWrap(true);
    layout->addWidget(m_statusLabel);

    m_progress = new QProgressBar(this);
    m_progress->setRange(0, 100);
    m_progress->setValue(0);
    layout->addWidget(m_progress);

    m_fileLabel = new QLabel(QString(), this);
    m_fileLabel->setStyleSheet(QStringLiteral("color: gray;"));
    m_fileLabel->setWordWrap(true);
    layout->addWidget(m_fileLabel);

    auto* btnRow = new QHBoxLayout();
    btnRow->addStretch(1);
    m_skipButton = new QPushButton(QStringLiteral("Skip"), this);
    connect(m_skipButton, &QPushButton::clicked, this, &OnboardingDialog::onSkip);
    btnRow->addWidget(m_skipButton);

    m_continueButton = new QPushButton(QStringLiteral("Continue"), this);
    m_continueButton->setDefault(true);
    connect(m_continueButton, &QPushButton::clicked, this, &OnboardingDialog::onContinue);
    btnRow->addWidget(m_continueButton);
    layout->addLayout(btnRow);
}

void OnboardingDialog::setStatusText(const QString& text) {
    m_statusLabel->setText(text);
}

void OnboardingDialog::setProgress(int percent) {
    m_progress->setValue(qBound(0, percent, 100));
}

void OnboardingDialog::setFileLabel(const QString& text) {
    m_fileLabel->setText(text);
}

void OnboardingDialog::onContinue() {
    accept();
    emit onboardingAccepted();
}

void OnboardingDialog::onSkip() {
    reject();
    emit onboardingRejected();
}

}  // namespace lexaloud
