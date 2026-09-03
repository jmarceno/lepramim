# Lepramim architecture

Lepramim is a **single Rust binary** with an in-process **Iced** desktop shell and a **child daemon** for speech synthesis.

## Processes

```mermaid
flowchart TD
    subgraph AppProc["lepramim app process"]
        Iced["iced::daemon"]
        Tray["ksni StatusNotifier tray"]
        HK["D-Bus org.lepramim.App + KGlobalAccel"]
        Cap["selection capture"]
        Win["Control / Overlay / Onboarding windows"]
    end

    subgraph DaemonProc["lepramim daemon child"]
        API["Axum UDS HTTP"]
        Player["Player + Kokoro + CPAL"]
    end

    Iced --> Tray
    Iced --> HK
    Iced --> Win
    HK --> Cap
    Tray --> Cap
    Cap -->|"POST /speak"| API
    Iced -->|"GET /state poll"| API
    Iced -->|"GET/POST /config"| API
    AppProc -->|"spawn lepramim daemon"| DaemonProc
```

## Components

| Layer | Technology | Role |
|-------|------------|------|
| GUI | Iced 0.13 (tiny-skia) | Tray-first daemon; windows on demand |
| Tray | ksni (StatusNotifier) | Tray menu without GTK |
| Hotkeys | zbus + KGlobalAccel | Meta+R speak, Meta+P toggle |
| Capture | wl-paste / xclip / synthetic Ctrl+C | Selection capture in `src/ui/capture.rs` |
| Daemon API | Axum over Unix socket | TTS, audio, MPRIS, config |
| Models | ureq + SHA-256 verify | Kokoro ONNX + voice bank download |

## Lifecycle

1. Opening the app ensures the config file exists (without waiting for models).
2. The app starts the tray icon and hotkeys; the welcome window opens if the
   speech models are missing.
3. When models are ready, the app spawns its speech-engine child process.
4. Quitting from the tray menu shuts the engine down and waits for it to exit.

## Configuration

The control window **GETs full config, merges edits, POSTs full config** so partial updates cannot wipe unrelated TOML sections.

## Packaging

- Native stage: `scripts/build-native.sh` (cargo only, one binary).
- AppImage: `scripts/build-appimage.sh` bundles ldd-discovered GUI/audio deps; **no Qt**.
