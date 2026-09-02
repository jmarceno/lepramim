#include <QApplication>
#include <QCommandLineParser>
#include <QFileInfo>
#include <QMessageBox>
#include <QTimer>

#include "control_window.hpp"
#include "onboarding.hpp"
#include "overlay.hpp"
#include "tray.hpp"

using namespace lexaloud;

int main(int argc, char* argv[]) {
    QApplication app(argc, argv);
    app.setApplicationName(QStringLiteral("Lexaloud"));
    app.setQuitOnLastWindowClosed(false);

    QCommandLineParser parser;
    parser.setApplicationDescription(QStringLiteral("Lexaloud Qt frontend"));
    parser.addHelpOption();
    parser.addVersionOption();
    QCommandLineOption showControl(QStringLiteral("control"),
                                   QStringLiteral("Show control window on start"));
    parser.addOption(showControl);
    QCommandLineOption withOverlay(QStringLiteral("overlay"),
                                   QStringLiteral("Show floating overlay"));
    parser.addOption(withOverlay);
    parser.process(app);

    QString iconPath = QStringLiteral(":/icons/lexaloud.svg");
    // Try filesystem fallback locations
    if (!QFileInfo::exists(iconPath)) {
        QStringList candidates = {
            QStringLiteral("/usr/share/icons/hicolor/scalable/apps/lexaloud.svg"),
            QStringLiteral("/usr/share/pixmaps/lexaloud.svg"),
        };
        for (const QString& c : candidates) {
            if (QFileInfo::exists(c)) {
                iconPath = c;
                break;
            }
        }
        if (!QFileInfo::exists(iconPath)) {
            // Use empty to fallback to theme icon
            iconPath = QString();
        }
    }

    if (!QSystemTrayIcon::isSystemTrayAvailable()) {
        QMessageBox::warning(nullptr, QStringLiteral("Lexaloud"),
                             QStringLiteral("System tray not available on this desktop."));
    }

    Tray tray(iconPath);
    ControlWindow control;
    tray.setControlWindow(&control);

    // Close-to-tray: control window close hides instead of quitting when tray is visible
    QObject::connect(&control, &QWidget::destroyed, &app, []() {});

    // Handle tray quit
    QObject::connect(&tray, &Tray::quitRequested, &app, &QApplication::quit);

    // Overlay optional
    OverlayWindow* overlay = nullptr;
    if (parser.isSet(withOverlay)) {
        overlay = new OverlayWindow();
        overlay->show();
    }

    tray.show();
    if (parser.isSet(showControl)) {
        control.show();
        control.raise();
        control.activateWindow();
    }

    (void)tray;
    (void)control;

    int ret = app.exec();

    if (overlay != nullptr) {
        overlay->hide();
        delete overlay;
    }
    tray.hide();
    return ret;
}
