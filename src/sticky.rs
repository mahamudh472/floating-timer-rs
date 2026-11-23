//! Makes the window sticky: always above other windows and visible on every
//! virtual desktop (and hidden from the task bar).
//!
//! Two mechanisms are used, mirroring the original PySide6 app:
//!
//! * KDE Plasma (Wayland or X11): a short KWin script is loaded over D-Bus.
//! * Any other X11 window manager: EWMH client messages are sent directly.

use std::thread;
use std::time::{Duration, Instant};

const WINDOW_TITLE: &str = "FloatingTimer";

/// Apply the sticky state shortly after startup. Compositors need the window
/// to be mapped before they accept these hints, hence the retries.
pub fn apply(window_id: Option<u64>) {
    thread::spawn(move || {
        let start = Instant::now();
        for delay_ms in [100u64, 500, 1500] {
            let elapsed = start.elapsed().as_millis() as u64;
            if delay_ms > elapsed {
                thread::sleep(Duration::from_millis(delay_ms - elapsed));
            }
            if let Err(err) = kwin_sticky()
                && std::env::var_os("DEBUG_STICKY").is_some()
            {
                eprintln!("KWin sticky hint failed: {err}");
            }
            if let Some(id) = window_id {
                x11_sticky(id);
            }
        }
    });
}

fn kwin_script() -> String {
    let pid = std::process::id();
    format!(
        r#"
var windows = workspace.windowList ? workspace.windowList() : workspace.clientList();
for (var i = 0; i < windows.length; i++) {{
    var win = windows[i];
    if (win.pid === {pid} || win.caption === "{WINDOW_TITLE}") {{
        win.keepAbove = true;
        win.onAllDesktops = true;
        win.skipTaskbar = true;
    }}
}}
"#
    )
}

fn kwin_sticky() -> zbus::Result<()> {
    let connection = zbus::blocking::Connection::session()?;
    let pid = std::process::id();
    let path = std::env::temp_dir().join(format!("floating-timer-rs-kwin-{pid}.js"));
    std::fs::write(&path, kwin_script()).map_err(zbus::Error::from)?;
    let plugin_name = format!("floating_timer_rs_{pid}");

    let reply = connection.call_method(
        Some("org.kde.KWin"),
        "/Scripting",
        Some("org.kde.kwin.Scripting"),
        "loadScript",
        &(path.to_string_lossy().as_ref(), plugin_name.as_str()),
    )?;
    let script_id: i32 = reply.body().deserialize()?;

    // Plasma 6 exposes /Scripting/Script<id>; Plasma 5 used /<id>.
    let plasma6 = format!("/Scripting/Script{script_id}");
    let plasma5 = format!("/{script_id}");
    let mut ran = connection
        .call_method(
            Some("org.kde.KWin"),
            plasma6.as_str(),
            Some("org.kde.kwin.Script"),
            "run",
            &(),
        )
        .is_ok();
    if !ran {
        ran = connection
            .call_method(
                Some("org.kde.KWin"),
                plasma5.as_str(),
                Some("org.kde.kwin.Script"),
                "run",
                &(),
            )
            .is_ok();
    }

    // Give the compositor a moment to evaluate the script before unloading it.
    thread::sleep(Duration::from_millis(150));
    let _ = connection.call_method(
        Some("org.kde.KWin"),
        "/Scripting",
        Some("org.kde.kwin.Scripting"),
        "unloadScript",
        &(plugin_name.as_str(),),
    );
    let _ = std::fs::remove_file(&path);

    if ran {
        Ok(())
    } else {
        Err(zbus::Error::Failure(
            "KWin script interface not available".into(),
        ))
    }
}

fn x11_sticky(window: u64) {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::EventMask;
    use x11rb::protocol::xproto::{ClientMessageData, ClientMessageEvent, ConnectionExt};

    let Ok((connection, screen_num)) = x11rb::connect(None) else {
        return;
    };
    let root = connection.setup().roots[screen_num].root;

    let atom = |name: &str| {
        connection
            .intern_atom(false, name.as_bytes())
            .ok()?
            .reply()
            .ok()
            .map(|reply| reply.atom)
    };
    let (Some(desktop), Some(state), Some(above)) = (
        atom("_NET_WM_DESKTOP"),
        atom("_NET_WM_STATE"),
        atom("_NET_WM_STATE_ABOVE"),
    ) else {
        return;
    };

    let event_mask = EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY;

    // Show on all virtual desktops (0xFFFFFFFF == sticky in EWMH).
    let desktop_event = ClientMessageEvent::new(
        32,
        window as u32,
        desktop,
        ClientMessageData::from([0xFFFF_FFFF, 2, 0, 0, 0]),
    );
    let _ = connection.send_event(false, root, event_mask, desktop_event);

    // Keep above other windows (_NET_WM_STATE_ADD).
    let above_event = ClientMessageEvent::new(
        32,
        window as u32,
        state,
        ClientMessageData::from([1, above, 0, 0, 0]),
    );
    let _ = connection.send_event(false, root, event_mask, above_event);

    let _ = connection.flush();
}
