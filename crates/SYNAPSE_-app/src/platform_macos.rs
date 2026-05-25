#[cfg(target_os = "macos")]
pub fn apply_window_blur(window: &winit::window::Window) {
    unsafe { apply_vibrancy(window) }
}

#[cfg(target_os = "macos")]
unsafe fn apply_vibrancy(window: &winit::window::Window) {
    use objc2::rc::Retained;
    use objc2_foundation::MainThreadMarker;
    use objc2_app_kit::{
        NSAutoresizingMaskOptions, NSView, NSVisualEffectBlendingMode, NSVisualEffectMaterial,
        NSVisualEffectState, NSVisualEffectView, NSWindowOrderingMode,
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = match window.window_handle() {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!("window_blur: failed to get window handle: {e}");
            return;
        }
    };

    let ns_view_ptr = match handle.as_raw() {
        RawWindowHandle::AppKit(h) => h.ns_view.as_ptr(),
        _ => {
            tracing::warn!("window_blur: non-AppKit handle on macOS");
            return;
        }
    };

    let view = &*(ns_view_ptr as *const NSView);

    let ns_window = match view.window() {
        Some(w) => w,
        None => {
            tracing::warn!("window_blur: NSView has no parent NSWindow yet");
            return;
        }
    };

    let content_view = match ns_window.contentView() {
        Some(v) => v,
        None => {
            tracing::warn!("window_blur: NSWindow has no contentView");
            return;
        }
    };

    let frame = content_view.frame();

    // winit always runs the event loop on the main thread on macOS.
    let mtm = MainThreadMarker::new_unchecked();
    let effect_view: Retained<NSVisualEffectView> =
        NSVisualEffectView::initWithFrame(mtm.alloc::<NSVisualEffectView>(), frame);

    effect_view.setMaterial(NSVisualEffectMaterial::UnderWindowBackground);
    effect_view.setBlendingMode(NSVisualEffectBlendingMode::BehindWindow);
    effect_view.setState(NSVisualEffectState::Active);

    let mask = NSAutoresizingMaskOptions::NSViewWidthSizable
        | NSAutoresizingMaskOptions::NSViewHeightSizable;
    effect_view.setAutoresizingMask(mask);

    content_view.addSubview_positioned_relativeTo(
        &effect_view,
        NSWindowOrderingMode::NSWindowBelow,
        None,
    );

    tracing::info!("window_blur: NSVisualEffectView applied (UnderWindowBackground)");
}
