use std::sync::mpsc::{self, Receiver, Sender};

use ksni::blocking::{Handle, TrayMethods};
use ksni::menu::{MenuItem, StandardItem};

use crate::icon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Show,
    Quit,
}

pub struct TimerTray {
    tx: Sender<Command>,
    ctx: egui::Context,
}

impl TimerTray {
    fn send(&self, command: Command) {
        let _ = self.tx.send(command);
        self.ctx.request_repaint();
    }
}

impl ksni::Tray for TimerTray {
    const MENU_ON_ACTIVATE: bool = false;

    fn id(&self) -> String {
        "floating-timer-rs".into()
    }

    fn title(&self) -> String {
        "Floating Timer".into()
    }

    fn icon_name(&self) -> String {
        "floating-timer-rs".into()
    }

    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        icon::tray_icons()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        ksni::ToolTip {
            title: "Floating Timer".into(),
            description: "Always-on-top countdown timer".into(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(Command::Show);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Show".into(),
                activate: Box::new(|this: &mut Self| this.send(Command::Show)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|this: &mut Self| this.send(Command::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Spawn the tray icon service on its own thread.
///
/// Returns a channel receiver for menu/click commands and the tray handle, which
/// must be kept alive for as long as the icon should be shown.
pub fn spawn(ctx: egui::Context) -> (Receiver<Command>, Option<Handle<TimerTray>>) {
    let (tx, rx) = mpsc::channel();
    let tray = TimerTray { tx, ctx };
    match tray.spawn() {
        Ok(handle) => (rx, Some(handle)),
        Err(err) => {
            eprintln!("Could not create system tray icon: {err}");
            (rx, None)
        }
    }
}
