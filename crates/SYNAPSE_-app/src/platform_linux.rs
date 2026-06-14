#[cfg(target_os = "linux")]
pub fn apply_window_blur(window: &winit::window::Window) {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = match window.window_handle() {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!("window_blur: failed to get window handle: {e}");
            return;
        }
    };

    match handle.as_raw() {
        RawWindowHandle::Xlib(h) => set_kde_blur_x11(h.window.get()),
        RawWindowHandle::Xcb(h) => set_kde_blur_x11(h.window.get() as u64),
        RawWindowHandle::Wayland(_) => {
            // winit delegates to org_kde_kwin_blur_manager when available (KDE Plasma/KWin).
            // On compositors without the protocol this is silently ignored.
            window.set_blur(true);
            tracing::info!("window_blur: Wayland org_kde_kwin_blur requested via winit");
        }
        _ => tracing::info!("window_blur: unrecognised Linux display backend, skipping"),
    }
}

/// Set `_KDE_NET_WM_BLUR_BEHIND_REGION` on the X11 window.
/// An empty region list signals "blur the entire window", which KDE Plasma
/// and picom/compton both honour.
#[cfg(target_os = "linux")]
fn set_kde_blur_x11(window_xid: u64) {
    use x11_dl::xlib::{Xlib, XA_CARDINAL, PropModeReplace};

    let xlib = match Xlib::open() {
        Ok(x) => x,
        Err(e) => {
            tracing::warn!("window_blur: failed to load libX11: {e}");
            return;
        }
    };

    unsafe {
        // Open a short-lived connection for this property write.
        let display = (xlib.XOpenDisplay)(std::ptr::null());
        if display.is_null() {
            tracing::warn!("window_blur: XOpenDisplay failed (is $DISPLAY set?)");
            return;
        }

        let atom = (xlib.XInternAtom)(
            display,
            b"_KDE_NET_WM_BLUR_BEHIND_REGION\0".as_ptr() as *const _,
            0, // create atom if it doesn't exist
        );

        // Empty data (0 elements) = blur entire window.
        (xlib.XChangeProperty)(
            display,
            window_xid as x11_dl::xlib::Window,
            atom,
            XA_CARDINAL,
            32,
            PropModeReplace,
            std::ptr::null(),
            0,
        );

        (xlib.XFlush)(display);
        (xlib.XCloseDisplay)(display);
    }

    tracing::info!("window_blur: _KDE_NET_WM_BLUR_BEHIND_REGION set on X11 window {window_xid:#x}");
}
