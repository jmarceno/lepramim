"""XDG GlobalShortcuts portal integration for Lexaloud.

Registers global shortcuts via the `org.freedesktop.portal.GlobalShortcuts`
D-Bus portal so that KDE Plasma 6+, Hyprland, and Sway (with
xdg-desktop-portal-wlr) can trigger Lexaloud actions directly without
GNOME gsettings hacks.

GNOME 46/47 does NOT support this portal. GNOME users continue using
the gsettings-based hotkey path via the control window or manual
configuration.

Uses `dbus-fast` (already a dependency from the MPRIS2 commit). If the
portal is unavailable, `try_register()` returns False and the daemon
continues normally.
"""

# NOTE: do NOT use `from __future__ import annotations` in this module.
# dbus-fast reads return-type annotations at runtime to determine D-Bus
# signatures. With PEP 563 (stringified annotations), dbus-fast cannot
# distinguish our D-Bus type codes ("b", "s", "d") from forward refs.

import asyncio
import logging
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from .config import Config
    from .player import Player
    from .preprocessor import PreprocessorConfig

log = logging.getLogger(__name__)

# Portal well-known names
PORTAL_BUS_NAME = "org.freedesktop.portal.Desktop"
PORTAL_OBJECT_PATH = "/org/freedesktop/portal/desktop"
PORTAL_INTERFACE = "org.freedesktop.portal.GlobalShortcuts"

# Shortcut IDs and descriptions
SHORTCUTS: list[tuple[str, str, str, str]] = [
    (
        "lexaloud-speak-selection",
        "Speak highlighted selection",
        "Capture the highlighted text and speak it",
        "SUPER+R",
    ),
    ("lexaloud-speak-clipboard", "Speak clipboard", "Capture copied text and speak it", ""),
    ("lexaloud-toggle", "Pause / resume", "Pause or resume playback", "SUPER+P"),
    ("lexaloud-stop", "Stop", "Stop playback and clear the queue", ""),
    ("lexaloud-skip", "Skip sentence", "Skip to the next sentence", ""),
    ("lexaloud-back", "Back one sentence", "Rewind to the previous sentence", ""),
]


class ShortcutsAdapter:
    """Bridges Lexaloud's Player to the XDG GlobalShortcuts portal.

    Lifecycle:
        adapter = ShortcutsAdapter(player, cfg, preproc_config)
        registered = await adapter.try_register()
        ...
        adapter.disconnect()
    """

    def __init__(
        self,
        player: "Player",
        cfg: "Config",
        preproc_config: "PreprocessorConfig",
        normalizer: Any = None,
    ) -> None:
        self._player = player
        self._cfg = cfg
        self._preproc_config = preproc_config
        self._normalizer = normalizer
        self._bus: Any = None
        self._signal_handler: Any = None
        self._session_handle: str | None = None

    async def try_register(self) -> bool:
        """Attempt to bind shortcuts via the GlobalShortcuts portal.

        Returns True if successful, False if portal unavailable.
        """
        try:
            from dbus_fast.aio import MessageBus
        except ImportError:
            log.debug("dbus-fast not installed; GlobalShortcuts disabled")
            return False

        try:
            bus = await MessageBus().connect()
        except Exception as e:
            log.debug("could not connect to D-Bus session bus: %s", e)
            return False

        self._bus = bus

        try:
            introspection = await bus.introspect(PORTAL_BUS_NAME, PORTAL_OBJECT_PATH)
        except Exception as e:
            log.debug("GlobalShortcuts portal not available: %s", e)
            self._cleanup_bus()
            return False

        proxy = bus.get_proxy_object(PORTAL_BUS_NAME, PORTAL_OBJECT_PATH, introspection)

        try:
            iface = proxy.get_interface(PORTAL_INTERFACE)
        except Exception as e:
            log.debug("GlobalShortcuts interface not found: %s", e)
            self._cleanup_bus()
            return False

        # Subscribe to Activated signal before registering shortcuts
        try:
            iface.on_activated(self._on_activated)
            self._signal_handler = iface
        except Exception as e:
            log.debug("could not subscribe to Activated signal: %s", e)
            self._cleanup_bus()
            return False

        # Build the shortcuts list for CreateSession + BindShortcuts
        try:
            from dbus_fast import Variant

            session_handle = await self._create_session(iface, Variant)
            if session_handle is None:
                self._cleanup_bus()
                return False

            shortcut_descriptors = []
            for sid, _name, desc, preferred in SHORTCUTS:
                properties = {"description": Variant("s", desc)}
                if preferred:
                    properties["preferred_trigger"] = Variant("s", preferred)
                shortcut_descriptors.append((sid, properties))

            request_handle = await iface.call_bind_shortcuts(
                session_handle,
                shortcut_descriptors,
                "",  # parent_window
                {"handle_token": Variant("s", "lexaloud_bind")},
            )
            response, results = await self._await_request(request_handle)
            if response != 0:
                log.info("GlobalShortcuts binding was declined (response=%d)", response)
                self._cleanup_bus()
                return False
            bound = results.get("shortcuts", [])
            if not bound:
                log.info("GlobalShortcuts portal returned no bound shortcuts")
                self._cleanup_bus()
                return False
        except Exception as e:
            log.debug("could not bind shortcuts: %s", e)
            self._cleanup_bus()
            return False

        self._session_handle = session_handle
        log.info("GlobalShortcuts portal: %d shortcuts registered", len(bound))
        return True

    async def _create_session(self, iface, variant_cls):
        """Create a GlobalShortcuts session. Returns the session handle or None."""
        try:
            request_handle = await iface.call_create_session(
                {
                    "handle_token": variant_cls("s", "lexaloud"),
                    "session_handle_token": variant_cls("s", "lexaloud"),
                }
            )
            response, results = await self._await_request(request_handle)
            if response != 0:
                log.info("GlobalShortcuts session creation was declined (response=%d)", response)
                return None
            session_handle = results.get("session_handle")
            if hasattr(session_handle, "value"):
                session_handle = session_handle.value
            return str(session_handle) if session_handle else None
        except Exception as e:
            log.debug("CreateSession failed: %s", e)
            return None

    async def _await_request(self, request_handle: str) -> tuple[int, dict[str, Any]]:
        """Wait for the portal Request response instead of using its handle as data."""
        if self._bus is None:
            raise RuntimeError("D-Bus session is not connected")
        introspection = await self._bus.introspect(PORTAL_BUS_NAME, request_handle)
        request = self._bus.get_proxy_object(PORTAL_BUS_NAME, request_handle, introspection)
        iface = request.get_interface("org.freedesktop.portal.Request")
        response_future: asyncio.Future[tuple[int, dict[str, Any]]] = (
            asyncio.get_running_loop().create_future()
        )

        def on_response(response: int, results: dict[str, Any]) -> None:
            if not response_future.done():
                response_future.set_result((response, results))

        iface.on_response(on_response)
        return await asyncio.wait_for(response_future, timeout=30)

    def _on_activated(self, _session_handle, shortcut_id, _timestamp, _options) -> None:
        """Handle shortcut activation from the portal.

        This callback runs in the D-Bus event loop (same asyncio loop as
        the daemon). For async player operations, we schedule a task.
        """
        loop = None
        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            return

        if shortcut_id == "lexaloud-speak-selection":
            loop.create_task(self._speak_selection())
        elif shortcut_id == "lexaloud-speak-clipboard":
            loop.create_task(self._speak_clipboard())
        elif shortcut_id == "lexaloud-toggle":
            loop.create_task(self._toggle())
        elif shortcut_id == "lexaloud-stop":
            loop.create_task(self._player.stop())
        elif shortcut_id == "lexaloud-skip":
            loop.create_task(self._player.skip())
        elif shortcut_id == "lexaloud-back":
            loop.create_task(self._player.back())
        else:
            log.debug("unknown shortcut activated: %s", shortcut_id)

    async def _toggle(self) -> None:
        """Toggle pause/resume."""
        state = self._player.state.state
        if state == "speaking":
            await self._player.pause()
        elif state == "paused":
            await self._player.resume()

    async def _speak_selection(self) -> None:
        """Capture PRIMARY selection and speak it."""
        await self._capture_and_speak("primary")

    async def _speak_clipboard(self) -> None:
        """Capture CLIPBOARD and speak it."""
        await self._capture_and_speak("clipboard")

    async def _capture_and_speak(self, source: str) -> None:
        """Capture text from the given source and feed it to the player.

        Runs the capture subprocess in the daemon's executor (not spawning
        a separate process). The daemon runs as systemd --user and inherits
        WAYLAND_DISPLAY + DISPLAY, so wl-paste / xclip work.
        """
        from .preprocessor import preprocess_with_llm
        from .selection import SelectionError, read_clipboard, read_primary

        loop = asyncio.get_running_loop()
        max_bytes = self._cfg.capture.max_bytes
        timeout_s = self._cfg.capture.subprocess_timeout_s

        try:
            if source == "primary":
                result = await loop.run_in_executor(None, read_primary, max_bytes, timeout_s)
            else:
                result = await loop.run_in_executor(None, read_clipboard, max_bytes, timeout_s)
        except SelectionError as e:
            log.debug("shortcut capture failed (%s): %s", source, e)
            return
        except Exception as e:
            log.debug("shortcut capture error (%s): %s", source, e)
            return

        sentences = await preprocess_with_llm(result.text, self._preproc_config, self._normalizer)
        if not sentences:
            log.debug("shortcut capture produced no sentences from %s", source)
            return

        await self._player.speak(sentences, mode="replace")

    def _cleanup_bus(self) -> None:
        """Disconnect the bus and reset internal state."""
        self._signal_handler = None
        self._session_handle = None
        if self._bus is not None:
            try:
                self._bus.disconnect()
            except Exception:
                pass
            self._bus = None

    def disconnect(self) -> None:
        """Disconnect from the session bus. Safe to call if never connected."""
        self._cleanup_bus()
        log.debug("GlobalShortcuts disconnected")
