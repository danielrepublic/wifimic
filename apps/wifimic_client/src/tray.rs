use std::time::Instant;

use crate::control::{AudioRenderer, ControlError, ControlPlane, DatagramTransport};

pub(crate) const TOOLTIP: &str = "wifimic-client";
pub(crate) const RESTART_LABEL: &str = "Restart";
pub(crate) const EXIT_LABEL: &str = "Exit";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MenuEventId {
    Restart,
    Exit,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MenuEvent {
    id: MenuEventId,
}

impl MenuEvent {
    #[cfg(test)]
    pub(crate) const fn restart() -> Self {
        Self {
            id: MenuEventId::Restart,
        }
    }

    #[cfg(test)]
    pub(crate) const fn exit() -> Self {
        Self {
            id: MenuEventId::Exit,
        }
    }

    #[cfg(test)]
    pub(crate) const fn unknown() -> Self {
        Self {
            id: MenuEventId::Unknown,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClientRunState {
    Running,
    ShutdownRequested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayDispatch {
    Ignored,
    Restarted { session_id: u64 },
    ExitRequested,
}

pub(crate) trait TrayControl {
    type Error;

    fn restart(&mut self, now: Instant, epoch_ms: u64) -> Result<u64, Self::Error>;
    fn stop(&mut self, now: Instant) -> Result<(), Self::Error>;
    fn render_ready(&mut self, now: Instant) -> Result<(), Self::Error>;
}

impl<T, R> TrayControl for ControlPlane<T, R>
where
    T: DatagramTransport,
    R: AudioRenderer,
{
    type Error = ControlError;

    fn restart(&mut self, now: Instant, epoch_ms: u64) -> Result<u64, Self::Error> {
        ControlPlane::restart(self, now, epoch_ms)
    }

    fn stop(&mut self, now: Instant) -> Result<(), Self::Error> {
        ControlPlane::stop(self, now)
    }

    fn render_ready(&mut self, now: Instant) -> Result<(), Self::Error> {
        ControlPlane::render_ready(self, now).map(|_| ())
    }
}

pub(crate) fn dispatch_menu_event<C: TrayControl>(
    control: &mut C,
    event: MenuEvent,
    now: Instant,
    epoch_ms: u64,
    state: &mut ClientRunState,
) -> Result<TrayDispatch, C::Error> {
    if matches!(state, ClientRunState::ShutdownRequested) {
        return Ok(TrayDispatch::Ignored);
    }

    match event.id {
        MenuEventId::Restart => control
            .restart(now, epoch_ms)
            .map(|session_id| TrayDispatch::Restarted { session_id }),
        MenuEventId::Exit => {
            let result = control.stop(now);
            *state = ClientRunState::ShutdownRequested;
            result.map(|()| TrayDispatch::ExitRequested)
        }
        MenuEventId::Unknown => Ok(TrayDispatch::Ignored),
    }
}

pub(crate) fn render_if_running<C: TrayControl>(
    control: &mut C,
    now: Instant,
    state: ClientRunState,
) -> Result<Option<()>, C::Error> {
    match state {
        ClientRunState::Running => control.render_ready(now).map(Some),
        ClientRunState::ShutdownRequested => Ok(None),
    }
}

#[cfg(windows)]
#[derive(Debug, thiserror::Error)]
pub(crate) enum TrayError {
    #[error("tray {operation} failed: {detail}")]
    Operation {
        operation: &'static str,
        detail: String,
    },
}

#[cfg(windows)]
pub(crate) struct TrayRuntime {
    _icon: tray_icon::TrayIcon,
    restart_item: tray_icon::menu::MenuItem,
    exit_item: tray_icon::menu::MenuItem,
}

#[cfg(windows)]
impl TrayRuntime {
    pub(crate) fn new() -> Result<Self, TrayError> {
        use tray_icon::{
            menu::{Menu, MenuItem},
            Icon, TrayIconBuilder,
        };

        let icon = Icon::from_resource(1, None).map_err(|source| TrayError::Operation {
            operation: "load embedded tray icon resource",
            detail: source.to_string(),
        })?;
        let menu = Menu::new();
        let restart_item = MenuItem::new(RESTART_LABEL, true, None);
        let exit_item = MenuItem::new(EXIT_LABEL, true, None);
        menu.append(&restart_item)
            .map_err(|source| TrayError::Operation {
                operation: "append Restart menu item",
                detail: source.to_string(),
            })?;
        menu.append(&exit_item)
            .map_err(|source| TrayError::Operation {
                operation: "append Exit menu item",
                detail: source.to_string(),
            })?;
        let icon = TrayIconBuilder::new()
            .with_icon(icon)
            .with_menu(Box::new(menu))
            .with_tooltip(TOOLTIP)
            .build()
            .map_err(|source| TrayError::Operation {
                operation: "create tray icon",
                detail: source.to_string(),
            })?;

        Ok(Self {
            _icon: icon,
            restart_item,
            exit_item,
        })
    }

    pub(crate) fn poll_menu_event(&self) -> Option<MenuEvent> {
        use tray_icon::menu::MenuEvent as NativeMenuEvent;

        NativeMenuEvent::receiver().try_recv().ok().map(|event| {
            let id = if event.id == self.restart_item.id() {
                MenuEventId::Restart
            } else if event.id == self.exit_item.id() {
                MenuEventId::Exit
            } else {
                MenuEventId::Unknown
            };
            MenuEvent { id }
        })
    }
}

#[cfg(windows)]
pub(crate) fn pump_windows_messages() {
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE,
    };

    // SAFETY: MSG is a plain Win32 struct and these are the documented calls;
    // None targets the hidden tray window owned by this thread.
    unsafe {
        let mut message = MSG::default();
        while PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

#[cfg(test)]
#[path = "tray_tests.rs"]
mod tests;
