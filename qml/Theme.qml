pragma Singleton
import QtQuick

QtObject {
    // Shared palette with GravaAI (same color values; layout metrics below
    // stay per-app). Qt 8-digit hex is #AARRGGBB (alpha first), not CSS
    // #RRGGBBAA — a trailing alpha (e.g. "#2fb3a433") parses as olive
    // #b3a433, not teal.
    readonly property color windowBg: "#121418"
    readonly property color cardBg: "#1b1e25"
    readonly property color cardBgRaised: "#232833"
    readonly property color accent: "#2fb3a3"
    readonly property color accentMuted: "#332fb3a3"
    readonly property color accentSoft: "#222fb3a3"
    readonly property color accentStrong: "#3fd4c1"
    readonly property color textPrimary: "#ffffff"
    readonly property color textSecondary: "#c5c9ce"
    readonly property color textMuted: "#8b929a"
    readonly property color textDim: "#6b7280"
    readonly property color statusGreen: "#3ecf8e"
    readonly property color statusGreenBg: "#223ecf8e"
    readonly property color danger: "#e5656e"
    readonly property color dangerBg: "#26e5656e"
    readonly property color dangerStrong: "#f07880"
    readonly property color warning: "#e8b339"
    readonly property color warningBg: "#26e8b339"
    readonly property color borderSubtle: "#2a303b"
    readonly property color inputBg: "#12151b"
    readonly property color sliderTrack: "#2a2f38"
    readonly property color accentText: textPrimary
    readonly property color hover: cardBgRaised
    readonly property color scrim: cardBg
    readonly property int radius: 10
    readonly property int radiusSm: 8
    readonly property int sidebarWidth: 236
}
