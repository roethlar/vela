//! Let a Wayland compositor that prefers server decorations own the titlebar.
//! Other desktops keep tao's window unchanged. Called once, during setup.

fn should_remove_titlebar(
    linux: bool,
    wayland: bool,
    prefers_ssd: Option<bool>,
    main_window_exists: bool,
) -> bool {
    linux && wayland && prefers_ssd.unwrap_or(false) && main_window_exists
}

#[cfg(target_os = "linux")]
pub(crate) use linux::configure;

#[cfg(target_os = "linux")]
mod linux {
    use gtk::glib::translate::ToGlibPtr;
    use gtk::prelude::*;
    use tauri::Manager;

    /// Setup runs on GTK's thread, before the event loop can start playback.
    pub(crate) fn configure(app: &tauri::AppHandle) {
        let window = app
            .get_webview_window("main")
            .and_then(|window| window.gtk_window().ok());
        let display = window.as_ref().map(|window| window.display());
        let wayland = display
            .as_ref()
            .is_some_and(|display| display.type_().name() == "GdkWaylandDisplay");
        let preference = display
            .as_ref()
            .filter(|_| wayland)
            .and_then(display_prefers_ssd);
        if !super::should_remove_titlebar(
            cfg!(target_os = "linux"),
            wayland,
            preference,
            window.is_some(),
        ) {
            return;
        }
        if let Some(window) = window {
            remove_titlebar(&window);
        }
    }

    /// Only called for a Wayland display. Like the embedded host's GDK
    /// lookups, this also works with GTK built without the Wayland backend.
    fn display_prefers_ssd(display: &gtk::gdk::Display) -> Option<bool> {
        type PrefersSsd =
            unsafe extern "C" fn(*mut gtk::gdk::ffi::GdkDisplay) -> gtk::glib::ffi::gboolean;
        // SAFETY: a valid C string; search the libraries already loaded by GTK.
        let address = unsafe {
            libc::dlsym(
                libc::RTLD_DEFAULT,
                c"gdk_wayland_display_prefers_ssd".as_ptr(),
            )
        };
        if address.is_null() {
            return None;
        }
        // SAFETY: this symbol takes a GdkDisplay* and returns gboolean (not
        // a pointer, unlike the embedded host's Wayland lookup functions).
        let prefers_ssd: PrefersSsd = unsafe { std::mem::transmute(address) };
        // SAFETY: a live Wayland GdkDisplay, on GTK's own thread.
        Some(unsafe { prefers_ssd(display.to_glib_none().0) != 0 })
    }

    fn remove_titlebar(window: &gtk::ApplicationWindow) {
        // Hold a reference so even a failed replacement can put tao's bar back.
        let Some(titlebar) = window.titlebar() else {
            return;
        };
        let visible = window.is_visible();
        let realized = window.is_realized();
        // GtkWindow::size excludes CSD. Unrealize clears the resize request;
        // tao's default size is only 1x1, so reissue the actual content size.
        let size = window.size();
        if size.0 <= 0 || size.1 <= 0 {
            return;
        }

        let removed = replace_titlebar(window, None, size);
        let restored = restore_mapping(window, visible, realized);
        if !removed || !restored {
            // GTK's setters return void. If their observable postconditions
            // failed, best effort is to restore the original bar and mapping.
            let _ = replace_titlebar(window, Some(&titlebar), size);
            let _ = restore_mapping(window, visible, realized);
        }
    }

    fn replace_titlebar(
        window: &gtk::ApplicationWindow,
        titlebar: Option<&gtk::Widget>,
        size: (i32, i32),
    ) -> bool {
        // tao 0.35.3 shows visible windows in its constructor. Hide/unrealize
        // first: set_titlebar on a realized window warns and resets its size.
        if window.is_realized() {
            window.hide();
            window.unrealize();
        }
        if window.is_realized() {
            return false;
        }
        window.set_titlebar(titlebar);
        window.resize(size.0, size.1);
        window.titlebar().as_ref() == titlebar
    }

    fn restore_mapping(window: &gtk::ApplicationWindow, visible: bool, realized: bool) -> bool {
        // Keep this GtkWindow and its children. GTK retains the visual,
        // app-paintable flag, title, geometry hints and signal connections.
        // Its unmap/map keeps window states; tao's queued initial maximize
        // still targets this same object. Do not override either with guesses.
        if visible {
            window.show();
        } else if realized {
            window.realize();
        }
        window.is_visible() == visible
            && window.is_realized() == realized
            && (!realized || window.window().is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::should_remove_titlebar;

    #[test]
    fn linux_wayland_with_ssd_and_main_window_removes_titlebar() {
        assert!(should_remove_titlebar(true, true, Some(true), true));
    }

    #[test]
    fn non_linux_keeps_titlebar() {
        assert!(!should_remove_titlebar(false, true, Some(true), true));
    }

    #[test]
    fn x11_keeps_titlebar() {
        assert!(!should_remove_titlebar(true, false, Some(true), true));
    }

    #[test]
    fn compositor_without_ssd_preference_keeps_titlebar() {
        assert!(!should_remove_titlebar(true, true, Some(false), true));
    }

    #[test]
    fn missing_ssd_symbol_keeps_titlebar() {
        assert!(!should_remove_titlebar(true, true, None, true));
    }

    #[test]
    fn missing_main_window_keeps_titlebar() {
        assert!(!should_remove_titlebar(true, true, Some(true), false));
    }
}
