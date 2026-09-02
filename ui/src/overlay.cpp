#include "overlay.hpp"

#include <QGuiApplication>
#include <QHBoxLayout>
#include <QPainter>
#include <QPainterPath>
#include <QScreen>

namespace lexaloud {

static constexpr int kBarWidth = 500;
static constexpr int kBarHeight = 80;
static constexpr int kCornerRadius = 16;
static constexpr int kBottomMargin = 24;
static constexpr int kPollMs = 200;

static const char* kLabelPause = "\u23f8";  // ⏸
static const char* kLabelPlay = "\u23f5";   // ⏵
static const char* kLabelSkip = "\u23ed";   // ⏭
static const char* kLabelStop = "\u23f9";   // ⏹

OverlayWindow::OverlayWindow(QWidget* parent)
    : QWidget(parent, Qt::FramelessWindowHint | Qt::WindowStaysOnTopHint | Qt::Tool) {
    setAttribute(Qt::WA_TranslucentBackground);
    setAttribute(Qt::WA_ShowWithoutActivating);
    setWindowFlag(Qt::X11BypassWindowManagerHint, false);
    m_visible = false;
    setupWindow();
    buildUi();
    positionWindow();

    m_timer = new QTimer(this);
    m_timer->setInterval(kPollMs);
    connect(m_timer, &QTimer::timeout, this, &OverlayWindow::pollState);
    m_timer->start();
}

OverlayWindow::~OverlayWindow() = default;

void OverlayWindow::setupWindow() {
    setFixedSize(kBarWidth, kBarHeight);
    setWindowTitle(QStringLiteral("Lexaloud Overlay"));
}

void OverlayWindow::buildUi() {
    auto* layout = new QHBoxLayout(this);
    layout->setContentsMargins(16, 8, 12, 8);
    layout->setSpacing(8);

    m_label = new QLabel(QString(), this);
    m_label->setStyleSheet(QStringLiteral("color: #ffffff; font-size: 14px; font-weight: 500;"));
    m_label->setTextInteractionFlags(Qt::NoTextInteraction);
    layout->addWidget(m_label, 1);

    auto makeButton = [this](const char* label, auto slot) {
        auto* btn = new QToolButton(this);
        btn->setText(QString::fromUtf8(label));
        btn->setToolButtonStyle(Qt::ToolButtonTextOnly);
        btn->setAutoRaise(true);
        btn->setFocusPolicy(Qt::NoFocus);
        btn->setStyleSheet(
            QStringLiteral("QToolButton { color: rgba(255,255,255,0.9); font-size: 20px;"
                           " padding: 4px 10px; border: none; background: transparent; }"
                           "QToolButton:hover { background: rgba(255,255,255,0.15);"
                           " border-radius: 6px; }"));
        connect(btn, &QToolButton::clicked, this, slot);
        return btn;
    };

    m_btnPause = makeButton(kLabelPause, &OverlayWindow::onPauseResume);
    m_btnSkip = makeButton(kLabelSkip, &OverlayWindow::onSkip);
    m_btnStop = makeButton(kLabelStop, &OverlayWindow::onStop);
    layout->addWidget(m_btnPause);
    layout->addWidget(m_btnSkip);
    layout->addWidget(m_btnStop);
}

void OverlayWindow::positionWindow() {
    QScreen* screen = QGuiApplication::primaryScreen();
    if (screen == nullptr) {
        return;
    }
    QRect geom = screen->availableGeometry();
    int x = geom.x() + (geom.width() - kBarWidth) / 2;
    int y = geom.y() + geom.height() - kBarHeight - kBottomMargin;
    setGeometry(x, y, kBarWidth, kBarHeight);
}

void OverlayWindow::paintEvent(QPaintEvent* event) {
    Q_UNUSED(event)
    QPainter painter(this);
    painter.setRenderHint(QPainter::Antialiasing);
    painter.setPen(Qt::NoPen);
    QColor bg(25, 25, 25, static_cast<int>(0.85 * 255));
    painter.setBrush(bg);
    QRectF rect(0, 0, width(), height());
    painter.drawRoundedRect(rect, kCornerRadius, kCornerRadius);
}

void OverlayWindow::pollState() {
    ApiResult r = m_client.getState();
    if (r.statusCode != 200 || r.json.isNull() || !r.json.isObject()) {
        applyState(QStringLiteral("idle"), QString());
        return;
    }
    QJsonObject obj = r.json.object();
    QString state = obj.value(QStringLiteral("state")).toString(QStringLiteral("idle"));
    QString sentence = obj.value(QStringLiteral("current_sentence")).toString();
    applyState(state, sentence);
}

void OverlayWindow::applyState(const QString& state, const QString& sentence) {
    updateLabel(state, sentence);
    updateButtons(state);
    updateVisibility(state);
    m_lastState = state;
}

void OverlayWindow::updateLabel(const QString& state, const QString& sentence) {
    if (state == QLatin1String("speaking") || state == QLatin1String("paused")) {
        QString text = sentence.isEmpty() ? QStringLiteral("Preparing\u2026") : sentence;
        int avail = m_label->width() > 0 ? m_label->width() : kBarWidth - 160;
        QFontMetrics fm(m_label->font());
        QString elided = fm.elidedText(text, Qt::ElideRight, avail);
        m_label->setText(elided);
    } else {
        m_label->setText(QString());
    }
}

void OverlayWindow::updateButtons(const QString& state) {
    bool active = (state == QLatin1String("speaking") || state == QLatin1String("paused"));
    m_btnPause->setEnabled(active);
    m_btnSkip->setEnabled(active);
    m_btnStop->setEnabled(active);
    if (state == QLatin1String("paused")) {
        m_btnPause->setText(QString::fromUtf8(kLabelPlay));
    } else {
        m_btnPause->setText(QString::fromUtf8(kLabelPause));
    }
}

void OverlayWindow::updateVisibility(const QString& state) {
    bool shouldShow = (state == QLatin1String("speaking") || state == QLatin1String("paused"));
    if (shouldShow && !m_visible) {
        show();
        m_visible = true;
        positionWindow();
    } else if (!shouldShow && m_visible) {
        hide();
        m_visible = false;
    }
}

void OverlayWindow::setSentence(const QString& sentence) {
    m_label->setText(sentence);
}

void OverlayWindow::setPlaybackState(const QString& state) {
    applyState(state, m_label->text());
}

QString OverlayWindow::currentSentence() const {
    return m_label ? m_label->text() : QString();
}

void OverlayWindow::postAction(const QString& path) {
    // Fire-and-forget
    if (path == QLatin1String("/toggle")) {
        m_client.postToggle();
    } else if (path == QLatin1String("/skip")) {
        m_client.postSkip();
    } else if (path == QLatin1String("/stop")) {
        m_client.postStop();
    }
}

void OverlayWindow::onPauseResume() {
    postAction(QStringLiteral("/toggle"));
}

void OverlayWindow::onSkip() {
    postAction(QStringLiteral("/skip"));
}

void OverlayWindow::onStop() {
    postAction(QStringLiteral("/stop"));
}

}  // namespace lexaloud
