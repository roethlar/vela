//! OpenGL through EGL on the Wayland connection Vela's window already uses.
//!
//! The machine's own `libEGL` and `libwayland-egl` are opened at run time:
//! they belong to its GPU driver and compositor and are never shipped
//! (`.agents/plans/embedded-player.md`, "System pieces that are NOT
//! shipped"). Signatures and constants are transcribed from the Khronos EGL
//! 1.5 headers and the extensions named where they are used.
//!
//! One rule is not visible in the code below: the EGL display belongs to the
//! whole process. GTK and the web view use the same one, so it is never
//! terminated here.

use std::ffi::{c_char, c_int, c_void, CStr};

type Display = *mut c_void;
type Config = *mut c_void;
type Context = *mut c_void;
type Surface = *mut c_void;
type Attrib = isize;

const PLATFORM_WAYLAND: u32 = 0x31D8;
const OPENGL_API: u32 = 0x30A2;
const NONE: c_int = 0x3038;
const SURFACE_TYPE: c_int = 0x3033;
const WINDOW_BIT: c_int = 0x0004;
const RENDERABLE_TYPE: c_int = 0x3040;
const OPENGL_BIT: c_int = 0x0008;
const RED_SIZE: c_int = 0x3024;
const GREEN_SIZE: c_int = 0x3023;
const BLUE_SIZE: c_int = 0x3022;
const ALPHA_SIZE: c_int = 0x3021;
const CONTEXT_MAJOR_VERSION: c_int = 0x3098;
const CONTEXT_MINOR_VERSION: c_int = 0x30FB;
const CONTEXT_OPENGL_PROFILE_MASK: c_int = 0x30FD;
const CONTEXT_OPENGL_CORE_PROFILE_BIT: c_int = 0x0001;
/// `EGL_EXT_pixel_format_float`.
const COLOR_COMPONENT_TYPE: c_int = 0x3339;
const COLOR_COMPONENT_TYPE_FIXED: c_int = 0x333A;
/// `EGL_EXT_device_query` and `EGL_EXT_device_drm_render_node`.
const DEVICE: c_int = 0x322C;
const DRM_RENDER_NODE_FILE: c_int = 0x3377;
const GL_COLOR_BUFFER_BIT: u32 = 0x4000;

/// The functions used, resolved once.
struct Api {
    get_platform_display: unsafe extern "C" fn(u32, *mut c_void, *const Attrib) -> Display,
    initialize: unsafe extern "C" fn(Display, *mut c_int, *mut c_int) -> u32,
    bind_api: unsafe extern "C" fn(u32) -> u32,
    choose_config:
        unsafe extern "C" fn(Display, *const c_int, *mut Config, c_int, *mut c_int) -> u32,
    get_config_attrib: unsafe extern "C" fn(Display, Config, c_int, *mut c_int) -> u32,
    create_context: unsafe extern "C" fn(Display, Config, Context, *const c_int) -> Context,
    create_platform_window_surface:
        unsafe extern "C" fn(Display, Config, *mut c_void, *const Attrib) -> Surface,
    make_current: unsafe extern "C" fn(Display, Surface, Surface, Context) -> u32,
    swap_interval: unsafe extern "C" fn(Display, c_int) -> u32,
    swap_buffers: unsafe extern "C" fn(Display, Surface) -> u32,
    get_proc_address: unsafe extern "C" fn(*const c_char) -> *mut c_void,
    get_error: unsafe extern "C" fn() -> c_int,
    destroy_surface: unsafe extern "C" fn(Display, Surface) -> u32,
    destroy_context: unsafe extern "C" fn(Display, Context) -> u32,
    release_thread: unsafe extern "C" fn() -> u32,
    window_create: unsafe extern "C" fn(*mut c_void, c_int, c_int) -> *mut c_void,
    window_resize: unsafe extern "C" fn(*mut c_void, c_int, c_int, c_int, c_int),
    window_destroy: unsafe extern "C" fn(*mut c_void),
}

// Plain function pointers into libraries that stay loaded.
unsafe impl Send for Api {}
unsafe impl Sync for Api {}

fn open(name: &CStr) -> Result<*mut c_void, String> {
    // SAFETY: a valid C string. These are the machine's own graphics
    // libraries, found by the system's loader the way GTK finds them.
    let handle = unsafe { libc::dlopen(name.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if handle.is_null() {
        Err(format!("{} is not available", name.to_string_lossy()))
    } else {
        Ok(handle)
    }
}

fn api() -> Result<&'static Api, String> {
    static API: std::sync::OnceLock<Result<Api, String>> = std::sync::OnceLock::new();
    API.get_or_init(|| {
        let egl = open(c"libEGL.so.1")?;
        let wayland_egl = open(c"libwayland-egl.so.1")?;
        macro_rules! symbol {
            ($lib:expr, $name:literal) => {{
                // SAFETY: a live library handle and a C string literal.
                let address = unsafe { libc::dlsym($lib, concat!($name, "\0").as_ptr().cast()) };
                if address.is_null() {
                    return Err(format!("{} is missing from the system's EGL", $name));
                }
                // SAFETY: the address is the named function, whose signature
                // is the field's type (see the module comment).
                unsafe { super::function_at(address) }
            }};
        }
        Ok(Api {
            get_platform_display: symbol!(egl, "eglGetPlatformDisplay"),
            initialize: symbol!(egl, "eglInitialize"),
            bind_api: symbol!(egl, "eglBindAPI"),
            choose_config: symbol!(egl, "eglChooseConfig"),
            get_config_attrib: symbol!(egl, "eglGetConfigAttrib"),
            create_context: symbol!(egl, "eglCreateContext"),
            create_platform_window_surface: symbol!(egl, "eglCreatePlatformWindowSurface"),
            make_current: symbol!(egl, "eglMakeCurrent"),
            swap_interval: symbol!(egl, "eglSwapInterval"),
            swap_buffers: symbol!(egl, "eglSwapBuffers"),
            get_proc_address: symbol!(egl, "eglGetProcAddress"),
            get_error: symbol!(egl, "eglGetError"),
            destroy_surface: symbol!(egl, "eglDestroySurface"),
            destroy_context: symbol!(egl, "eglDestroyContext"),
            release_thread: symbol!(egl, "eglReleaseThread"),
            window_create: symbol!(wayland_egl, "wl_egl_window_create"),
            window_resize: symbol!(wayland_egl, "wl_egl_window_resize"),
            window_destroy: symbol!(wayland_egl, "wl_egl_window_destroy"),
        })
    })
    .as_ref()
    .map_err(Clone::clone)
}

/// Resolve an OpenGL function for mpv. The shape is the callback mpv asks for.
pub(crate) unsafe extern "C" fn proc_address(_ctx: *mut c_void, name: *const c_char) -> *mut c_void {
    match api() {
        // SAFETY: mpv passes a valid function name.
        Ok(api) => unsafe { (api.get_proc_address)(name) },
        Err(_) => std::ptr::null_mut(),
    }
}

/// What one window configuration offers, as far as choosing between them goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Format {
    pub(crate) red: i32,
    pub(crate) green: i32,
    pub(crate) blue: i32,
    pub(crate) alpha: i32,
    /// Fixed-point channels; the alternative is floating point, which the
    /// spike found unnecessary and which is not used.
    pub(crate) fixed: bool,
}

/// Pick the configuration to draw video into: ten bits per channel when the
/// GPU offers it (HDR needs them), otherwise eight. Among equals, the one
/// with the least alpha: the video surface is opaque, and an alpha channel
/// takes bits away from colour in a 32-bit format.
pub(crate) fn pick_format(formats: &[Format]) -> Option<(usize, i32)> {
    for depth in [10, 8] {
        let best = formats
            .iter()
            .enumerate()
            .filter(|(_, f)| f.fixed && f.red == depth && f.green == depth && f.blue == depth)
            .min_by_key(|(_, f)| f.alpha);
        if let Some((index, _)) = best {
            return Some((index, depth));
        }
    }
    None
}

/// Whether this machine's GPU can give the video a window to draw into.
/// Used to decide availability before a play is started.
pub(crate) fn probe(wl_display: *mut c_void) -> Result<i32, String> {
    let api = api()?;
    let (display, configs) = configs(api, wl_display)?;
    let _ = display;
    let formats: Vec<Format> = configs.iter().map(|(_, format)| *format).collect();
    pick_format(&formats)
        .map(|(_, depth)| depth)
        .ok_or_else(|| "the GPU offers no suitable OpenGL window format".to_string())
}

fn configs(api: &Api, wl_display: *mut c_void) -> Result<(Display, Vec<(Config, Format)>), String> {
    // SAFETY: `wl_display` is the live Wayland connection of Vela's window.
    let display =
        unsafe { (api.get_platform_display)(PLATFORM_WAYLAND, wl_display, std::ptr::null()) };
    if display.is_null() {
        return Err("EGL has no display for this Wayland connection".to_string());
    }
    let (mut major, mut minor) = (0, 0);
    // SAFETY: a valid display and out-parameters. Initialising a display that
    // GTK already initialised is allowed and only counts a reference.
    if unsafe { (api.initialize)(display, &mut major, &mut minor) } == 0 {
        return Err("EGL could not be initialised".to_string());
    }
    let wanted = [SURFACE_TYPE, WINDOW_BIT, RENDERABLE_TYPE, OPENGL_BIT, NONE];
    let mut raw: Vec<Config> = vec![std::ptr::null_mut(); 512];
    let mut count = 0;
    // SAFETY: the attribute list is NONE-terminated and `raw` has room for
    // the number of entries passed.
    let chosen = unsafe {
        (api.choose_config)(display, wanted.as_ptr(), raw.as_mut_ptr(), raw.len() as c_int, &mut count)
    };
    if chosen == 0 {
        return Err("EGL could not list its window formats".to_string());
    }
    raw.truncate(count.max(0) as usize);
    let attribute = |config: Config, name: c_int, default: c_int| {
        let mut value = default;
        // SAFETY: a valid display, config and out-parameter. An attribute the
        // driver does not know leaves `value` at the default.
        unsafe { (api.get_config_attrib)(display, config, name, &mut value) };
        value
    };
    let configs = raw
        .into_iter()
        .map(|config| {
            let format = Format {
                red: attribute(config, RED_SIZE, 0),
                green: attribute(config, GREEN_SIZE, 0),
                blue: attribute(config, BLUE_SIZE, 0),
                alpha: attribute(config, ALPHA_SIZE, 0),
                fixed: attribute(config, COLOR_COMPONENT_TYPE, COLOR_COMPONENT_TYPE_FIXED)
                    == COLOR_COMPONENT_TYPE_FIXED,
            };
            (config, format)
        })
        .collect();
    Ok((display, configs))
}

/// An OpenGL context drawing into one Wayland surface. Created, used and
/// dropped on one thread, on which its context stays current throughout.
pub(crate) struct Window {
    api: &'static Api,
    display: Display,
    context: Context,
    surface: Surface,
    native: *mut c_void,
    /// Bits per colour channel of the window: 10 or 8.
    pub(crate) depth: i32,
    clear_color: unsafe extern "C" fn(f32, f32, f32, f32),
    clear: unsafe extern "C" fn(u32),
}

impl Window {
    /// # Safety
    /// `wl_display` must be the live connection `wl_surface` belongs to, and
    /// `wl_surface` must outlive the window.
    pub(crate) unsafe fn create(
        wl_display: *mut c_void,
        wl_surface: *mut c_void,
        width: i32,
        height: i32,
    ) -> Result<Self, String> {
        let api = api()?;
        let (display, configs) = configs(api, wl_display)?;
        let formats: Vec<Format> = configs.iter().map(|(_, format)| *format).collect();
        let (index, depth) = pick_format(&formats)
            .ok_or_else(|| "the GPU offers no suitable OpenGL window format".to_string())?;
        let config = configs[index].0;
        // The API binding is per thread; this thread draws with desktop OpenGL.
        if unsafe { (api.bind_api)(OPENGL_API) } == 0 {
            return Err("this EGL has no desktop OpenGL".to_string());
        }
        let context_attributes = [
            CONTEXT_MAJOR_VERSION,
            3,
            CONTEXT_MINOR_VERSION,
            3,
            CONTEXT_OPENGL_PROFILE_MASK,
            CONTEXT_OPENGL_CORE_PROFILE_BIT,
            NONE,
        ];
        let context = unsafe {
            (api.create_context)(display, config, std::ptr::null_mut(), context_attributes.as_ptr())
        };
        if context.is_null() {
            return Err(format!("no OpenGL 3.3 context (EGL error {:#x})", unsafe {
                (api.get_error)()
            }));
        }
        let native = unsafe { (api.window_create)(wl_surface, width.max(1), height.max(1)) };
        if native.is_null() {
            unsafe { (api.destroy_context)(display, context) };
            return Err("the Wayland EGL window could not be created".to_string());
        }
        let surface = unsafe {
            (api.create_platform_window_surface)(display, config, native, std::ptr::null())
        };
        if surface.is_null() {
            let error = unsafe { (api.get_error)() };
            unsafe {
                (api.window_destroy)(native);
                (api.destroy_context)(display, context);
            }
            return Err(format!("no EGL surface for the video (EGL error {error:#x})"));
        }
        let mut window = Window {
            api,
            display,
            context,
            surface,
            native,
            depth,
            clear_color: noop_clear_color,
            clear: noop_clear,
        };
        if unsafe { (api.make_current)(display, surface, surface, context) } == 0 {
            return Err("the OpenGL context could not be made current".to_string());
        }
        // Never wait for the compositor in the swap: mpv times its own frames.
        unsafe { (api.swap_interval)(display, 0) };
        let clear_color = unsafe { (api.get_proc_address)(c"glClearColor".as_ptr()) };
        let clear = unsafe { (api.get_proc_address)(c"glClear".as_ptr()) };
        if !clear_color.is_null() && !clear.is_null() {
            // SAFETY: the two OpenGL functions of those names and signatures.
            window.clear_color = unsafe { super::function_at(clear_color) };
            window.clear = unsafe { super::function_at(clear) };
        }
        Ok(window)
    }

    /// Fill the window with opaque black and show it.
    pub(crate) fn show_black(&self) {
        // SAFETY: this window's context is current on this thread.
        unsafe {
            (self.clear_color)(0.0, 0.0, 0.0, 1.0);
            (self.clear)(GL_COLOR_BUFFER_BIT);
        }
        self.swap();
    }

    /// Show what has been drawn.
    pub(crate) fn swap(&self) {
        // SAFETY: a live display and surface, current on this thread.
        unsafe { (self.api.swap_buffers)(self.display, self.surface) };
    }

    /// Change the size of the buffers drawn from now on, in pixels.
    pub(crate) fn resize(&self, width: i32, height: i32) {
        // SAFETY: a live native window.
        unsafe { (self.api.window_resize)(self.native, width.max(1), height.max(1), 0, 0) };
    }

    /// Open the DRM render node of the GPU this window draws with, for
    /// hardware decoding. mpv would normally find the GPU through its own
    /// Wayland window; the library Vela ships has none, and the render node
    /// gives the same result (Slice 0, A10). `None` when the driver does not
    /// say, in which case mpv decodes in software.
    pub(crate) fn render_node(&self) -> Option<std::os::fd::OwnedFd> {
        type QueryDisplayAttrib = unsafe extern "C" fn(Display, c_int, *mut Attrib) -> u32;
        type QueryDeviceString = unsafe extern "C" fn(*mut c_void, c_int) -> *const c_char;
        let query_display = unsafe { (self.api.get_proc_address)(c"eglQueryDisplayAttribEXT".as_ptr()) };
        let query_device = unsafe { (self.api.get_proc_address)(c"eglQueryDeviceStringEXT".as_ptr()) };
        if query_display.is_null() || query_device.is_null() {
            return None;
        }
        // SAFETY: the two extension functions of those names and signatures.
        let query_display: QueryDisplayAttrib = unsafe { super::function_at(query_display) };
        let query_device: QueryDeviceString = unsafe { super::function_at(query_device) };
        let mut device: Attrib = 0;
        if unsafe { query_display(self.display, DEVICE, &mut device) } == 0 || device == 0 {
            return None;
        }
        let node = unsafe { query_device(device as *mut c_void, DRM_RENDER_NODE_FILE) };
        if node.is_null() {
            return None;
        }
        use std::os::unix::ffi::OsStrExt;
        let path = std::ffi::OsStr::from_bytes(unsafe { CStr::from_ptr(node) }.to_bytes());
        std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)
            .ok()
            .map(std::os::fd::OwnedFd::from)
    }
}

unsafe extern "C" fn noop_clear_color(_: f32, _: f32, _: f32, _: f32) {}
unsafe extern "C" fn noop_clear(_: u32) {}

impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: everything below is live and owned by this window, on the
        // thread that created it. The display itself is left alone: GTK and
        // the web view share it.
        unsafe {
            (self.api.make_current)(
                self.display,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            (self.api.destroy_surface)(self.display, self.surface);
            (self.api.window_destroy)(self.native);
            (self.api.destroy_context)(self.display, self.context);
            (self.api.release_thread)();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(red: i32, green: i32, blue: i32, alpha: i32, fixed: bool) -> Format {
        Format {
            red,
            green,
            blue,
            alpha,
            fixed,
        }
    }

    #[test]
    fn ten_bits_without_alpha_is_preferred() {
        // What Mesa offers on the development machine, in an unhelpful order.
        let formats = [
            format(8, 8, 8, 8, true),
            format(8, 8, 8, 0, true),
            format(10, 10, 10, 2, true),
            format(10, 10, 10, 0, true),
            format(16, 16, 16, 16, false),
        ];
        assert_eq!(pick_format(&formats), Some((3, 10)));
    }

    #[test]
    fn eight_bits_is_the_fallback_and_floating_point_is_never_picked() {
        let formats = [
            format(16, 16, 16, 16, false),
            format(8, 8, 8, 8, true),
            format(8, 8, 8, 0, true),
            // Ten-bit floating point does not exist, but nothing here should
            // depend on that.
            format(10, 10, 10, 0, false),
        ];
        assert_eq!(pick_format(&formats), Some((2, 8)));
        assert_eq!(pick_format(&[format(16, 16, 16, 16, false)]), None);
        assert_eq!(pick_format(&[]), None);
    }

    #[test]
    fn a_format_with_unequal_channels_is_not_a_video_format() {
        // 5-6-5 and the like.
        assert_eq!(pick_format(&[format(5, 6, 5, 0, true)]), None);
        assert_eq!(
            pick_format(&[format(10, 10, 8, 0, true), format(8, 8, 8, 0, true)]),
            Some((1, 8))
        );
    }
}
