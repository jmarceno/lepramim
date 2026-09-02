#pragma once

#include <QString>

namespace lexaloud {

class ApiClient;

struct SelectionCapture {
    QString text;
    QString source;
    bool truncated = false;
};

// Pick between Qt/X11 selection and clipboard snapshots. Empty selection is
// the Wayland path: never trust a sticky PRIMARY buffer.
SelectionCapture resolveCapture(const QString& selection, const QString& clipboardBefore,
                                const QString& clipboardAfter);

// Capture highlighted text. On Wayland this skips PRIMARY (often stale or a
// 2s wl-paste hang) and uses Qt clipboard plus a synthetic Ctrl+C. On X11 it
// still prefers PRIMARY. Returns empty text when nothing usable was found.
SelectionCapture captureHighlightedText();

void speakCapturedSelection(ApiClient& client);
void togglePlayback(ApiClient& client);
void notifyUser(const QString& title, const QString& body);

}  // namespace lexaloud
