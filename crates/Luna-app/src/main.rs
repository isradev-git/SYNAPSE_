// In debug builds, keep the console for dev output.
// In release builds, hide it for a clean user experience.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod input;
mod keyboard;
mod mouse;
mod pane_ops;
mod render;
mod search;
mod state;

fn main() {
    init_logging();

    #[cfg(target_os = "windows")]
    std::panic::set_hook(Box::new(|info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let msg = format!("Luna crashed:\n{}\n\nLocation: {}", payload, location);

        tracing::error!("PANIC: {} at {}", payload, location);
        show_windows_msgbox("Luna Crash", &msg);
    }));

    #[cfg(not(target_os = "windows"))]
    std::panic::set_hook(Box::new(|info| {
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        tracing::error!("PANIC: {} at {}", payload, location);
        eprintln!("Luna crashed:\n{}\n\nLocation: {}", payload, location);
    }));

    if let Err(e) = try_main() {
        let msg = format!("Luna failed to start:\n\n{}", e);
        tracing::error!("STARTUP ERROR: {}", e);
        #[cfg(target_os = "windows")]
        {
            show_windows_msgbox("Luna Error", &msg);
        }
        #[cfg(not(target_os = "windows"))]
        {
            eprintln!("{}", msg);
        }
        std::process::exit(1);
    }
}

fn init_logging() {
    #[cfg(target_os = "windows")]
    {
        let dir = std::env::temp_dir().join("Luna");
        let _ = std::fs::create_dir_all(&dir);
        if let Ok(file) = std::fs::File::create(dir.join("luna.log")) {
            let writer = std::sync::Mutex::new(file);
            tracing_subscriber::fmt()
                .with_writer(writer)
                .with_ansi(false)
                .try_init()
                .ok();
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        tracing_subscriber::fmt()
            .with_writer(std::io::stderr)
            .try_init()
            .ok();
    }
}

fn try_main() -> Result<(), Box<dyn std::error::Error>> {
    let (app, event_loop) = app::App::new()?;
    app.run(event_loop)
}

#[cfg(target_os = "windows")]
fn show_windows_msgbox(title: &str, text: &str) {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let title_wide: Vec<u16> = OsStr::new(title)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let text_wide: Vec<u16> = OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    extern "system" {
        fn MessageBoxW(
            hwnd: *mut std::ffi::c_void,
            text: *const u16,
            caption: *const u16,
            utype: u32,
        ) -> i32;
    }

    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text_wide.as_ptr(),
            title_wide.as_ptr(),
            0x10, // MB_ICONERROR
        );
    }
}
