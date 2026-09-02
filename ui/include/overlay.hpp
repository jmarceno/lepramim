#pragma once

#include <QLabel>
#include <QTimer>
#include <QToolButton>
#include <QWidget>

#include "api_client.hpp"

namespace lexaloud {

class OverlayWindow : public QWidget {
    Q_OBJECT
public:
    explicit OverlayWindow(QWidget* parent = nullptr);
    ~OverlayWindow() override;

    void setSentence(const QString& sentence);
    void setPlaybackState(const QString& state);

    bool isOverlayVisible() const {
        return m_visible;
    }
    QString currentSentence() const;
    QString currentState() const {
        return m_lastState;
    }

    // Test helpers
    QToolButton* pauseButton() const {
        return m_btnPause;
    }
    QToolButton* skipButton() const {
        return m_btnSkip;
    }
    QToolButton* stopButton() const {
        return m_btnStop;
    }
    QLabel* textLabel() const {
        return m_label;
    }

    void applyState(const QString& state, const QString& sentence);

protected:
    void paintEvent(QPaintEvent* event) override;

private slots:
    void pollState();

private:
    void setupWindow();
    void buildUi();
    void positionWindow();
    void updateVisibility(const QString& state);
    void updateButtons(const QString& state);
    void updateLabel(const QString& state, const QString& sentence);
    void postAction(const QString& path);
    void onPauseResume();
    void onSkip();
    void onStop();

    ApiClient m_client;
    QLabel* m_label = nullptr;
    QToolButton* m_btnPause = nullptr;
    QToolButton* m_btnSkip = nullptr;
    QToolButton* m_btnStop = nullptr;
    QTimer* m_timer = nullptr;
    bool m_visible = false;
    QString m_lastState;
};

}  // namespace lexaloud
