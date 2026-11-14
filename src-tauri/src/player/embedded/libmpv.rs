//! The mpv library Vela ships, loaded at run time.
//!
//! Nothing here is linked at build time and nothing falls back to a library
//! the machine happens to have (owner ruling D7 and licence rule D4 of
//! `.agents/plans/embedded-player.md`): the one library opened is the file at
//! the absolute path the caller names, which is the copy inside Vela's own
//! resources. If it cannot be opened the embedded player is unavailable and
//! the play goes to the external player.
//!
//! The functions are called through a hand-written table. Their signatures
//! and the constants below are transcribed from mpv's public headers
//! (`client.h`, `render.h`, `render_gl.h`, client API 2.5 and 2.6).

use std::ffi::{c_char, c_double, c_int, c_ulong, c_void, CStr, CString};
use std::path::{Path, PathBuf};

/// The file name of the library inside the shipped set.
pub(crate) const LIBRARY_FILE: &str = "libmpv.so.2";

/// The client API this code was written against. A library with a different
/// major number has changed incompatibly; a lower minor number lacks
/// something used here.
const API_MAJOR: c_ulong = 2;
const API_MINOR_AT_LEAST: c_ulong = 5;

#[repr(C)]
pub(crate) struct RawHandle {
    _opaque: [u8; 0],
}

#[repr(C)]
pub(crate) struct RawRenderContext {
    _opaque: [u8; 0],
}

/// `mpv_render_param`.
#[repr(C)]
pub(crate) struct RenderParam {
    pub(crate) kind: c_int,
    pub(crate) data: *mut c_void,
}

pub(crate) const RENDER_PARAM_END: c_int = 0;
pub(crate) const RENDER_PARAM_API_TYPE: c_int = 1;
pub(crate) const RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
pub(crate) const RENDER_PARAM_OPENGL_FBO: c_int = 3;
pub(crate) const RENDER_PARAM_FLIP_Y: c_int = 4;
pub(crate) const RENDER_PARAM_DEPTH: c_int = 5;
pub(crate) const RENDER_PARAM_DRM_DISPLAY_V2: c_int = 16;
pub(crate) const RENDER_API_TYPE_OPENGL: &CStr = c"opengl";
/// `MPV_RENDER_UPDATE_FRAME`.
pub(crate) const RENDER_UPDATE_FRAME: u64 = 1;

/// `mpv_opengl_init_params`.
#[repr(C)]
pub(crate) struct OpenGlInitParams {
    pub(crate) get_proc_address:
        Option<unsafe extern "C" fn(ctx: *mut c_void, name: *const c_char) -> *mut c_void>,
    pub(crate) get_proc_address_ctx: *mut c_void,
}

/// `mpv_opengl_fbo`.
#[repr(C)]
pub(crate) struct OpenGlFbo {
    pub(crate) fbo: c_int,
    pub(crate) w: c_int,
    pub(crate) h: c_int,
    pub(crate) internal_format: c_int,
}

/// `mpv_opengl_drm_params_v2`. Only `render_fd` is used: it names the GPU for
/// hardware decoding. The other fields describe a display Vela does not drive.
#[repr(C)]
pub(crate) struct OpenGlDrmParamsV2 {
    pub(crate) fd: c_int,
    pub(crate) crtc_id: c_int,
    pub(crate) connector_id: c_int,
    pub(crate) atomic_request_ptr: *mut *mut c_void,
    pub(crate) render_fd: c_int,
}

/// `mpv_event`.
#[repr(C)]
pub(crate) struct RawEvent {
    pub(crate) event_id: c_int,
    pub(crate) error: c_int,
    pub(crate) reply_userdata: u64,
    pub(crate) data: *mut c_void,
}

const EVENT_NONE: c_int = 0;
const EVENT_SHUTDOWN: c_int = 1;
/// `MPV_EVENT_SET_PROPERTY_REPLY`: the answer to `mpv_set_property_async`.
const EVENT_SET_PROPERTY_REPLY: c_int = 4;
/// `MPV_EVENT_PROPERTY_CHANGE`: an observed property has changed.
const EVENT_PROPERTY_CHANGE: c_int = 22;
/// `MPV_FORMAT_STRING`.
const FORMAT_STRING: c_int = 1;

/// `mpv_event_property`, the data of a property change.
#[repr(C)]
struct RawProperty {
    name: *const c_char,
    format: c_int,
    data: *mut c_void,
}

/// What mpv has to say, as far as the play acts on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Event {
    /// Nothing is waiting.
    None,
    /// The player has shut down: the file ended, or mpv was told to quit.
    Shutdown,
    /// The answer to an ask ([`super::hdr::Targetable::ask`]) made with
    /// `request`: whether mpv took the value.
    Answer { request: u64, taken: bool },
    /// One of mpv's targets has changed, by Vela's ask or by anything else,
    /// or is reported afresh. The only properties Vela observes are those
    /// `hdr` names ([`super::hdr::Targetable::observe`]); `registration` is
    /// the one this report was made under. `value` is the value as text, or
    /// `None` when mpv cannot say.
    TargetChanged {
        registration: u64,
        option: String,
        value: Option<String>,
    },
    /// Anything else, which the play does not act on.
    Other,
}

/// Read one of mpv's events. A refusal is a negative `error`
/// (`/usr/include/mpv/client.h`, `MPV_EVENT_SET_PROPERTY_REPLY`).
fn classify(event_id: c_int, error: c_int, reply_userdata: u64) -> Event {
    match event_id {
        EVENT_NONE => Event::None,
        EVENT_SHUTDOWN => Event::Shutdown,
        EVENT_SET_PROPERTY_REPLY => Event::Answer {
            request: reply_userdata,
            taken: error >= 0,
        },
        _ => Event::Other,
    }
}

/// Read a property change made under `registration` (the event's
/// `reply_userdata`): the property's name and its value as text, both
/// copied, since mpv reuses them at the next call on its handle.
///
/// # Safety
/// `property` is NULL or the data of an `MPV_EVENT_PROPERTY_CHANGE` that has
/// not yet been replaced by another call on its handle.
unsafe fn property_change(registration: u64, property: *const RawProperty) -> Event {
    if property.is_null() {
        return Event::Other;
    }
    // SAFETY: the caller's contract.
    let property = unsafe { &*property };
    if property.name.is_null() {
        return Event::Other;
    }
    // SAFETY: a NUL-terminated name owned by mpv (client.h).
    let option = unsafe { CStr::from_ptr(property.name) }
        .to_string_lossy()
        .into_owned();
    // A string's data is the address of a `char *` (client.h,
    // `MPV_FORMAT_STRING`); a property mpv cannot read has format 0 and no
    // data.
    let value = if property.format == FORMAT_STRING && !property.data.is_null() {
        // SAFETY: as just said.
        let text = unsafe { *(property.data as *const *const c_char) };
        (!text.is_null())
            .then(|| unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned())
    } else {
        None
    };
    Event::TargetChanged {
        registration,
        option,
        value,
    }
}

type Callback = Option<unsafe extern "C" fn(ctx: *mut c_void)>;

/// The loaded library and the functions Vela calls in it.
pub(crate) struct LibMpv {
    client_api_version: unsafe extern "C" fn() -> c_ulong,
    create: unsafe extern "C" fn() -> *mut RawHandle,
    set_option_string:
        unsafe extern "C" fn(*mut RawHandle, *const c_char, *const c_char) -> c_int,
    initialize: unsafe extern "C" fn(*mut RawHandle) -> c_int,
    set_property_async:
        unsafe extern "C" fn(*mut RawHandle, u64, *const c_char, c_int, *mut c_void) -> c_int,
    observe_property: unsafe extern "C" fn(*mut RawHandle, u64, *const c_char, c_int) -> c_int,
    unobserve_property: unsafe extern "C" fn(*mut RawHandle, u64) -> c_int,
    command: unsafe extern "C" fn(*mut RawHandle, *mut *const c_char) -> c_int,
    get_property_string: unsafe extern "C" fn(*mut RawHandle, *const c_char) -> *mut c_char,
    wait_event: unsafe extern "C" fn(*mut RawHandle, c_double) -> *mut RawEvent,
    set_wakeup_callback: unsafe extern "C" fn(*mut RawHandle, Callback, *mut c_void),
    terminate_destroy: unsafe extern "C" fn(*mut RawHandle),
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    free: unsafe extern "C" fn(*mut c_void),
    render_context_create: unsafe extern "C" fn(
        *mut *mut RawRenderContext,
        *mut RawHandle,
        *mut RenderParam,
    ) -> c_int,
    render_context_set_update_callback:
        unsafe extern "C" fn(*mut RawRenderContext, Callback, *mut c_void),
    render_context_update: unsafe extern "C" fn(*mut RawRenderContext) -> u64,
    render_context_render: unsafe extern "C" fn(*mut RawRenderContext, *mut RenderParam) -> c_int,
    render_context_report_swap: unsafe extern "C" fn(*mut RawRenderContext),
    render_context_free: unsafe extern "C" fn(*mut RawRenderContext),
}

// The table is plain function pointers into a library that stays loaded for
// the life of the process; mpv's client API may be called from any thread.
unsafe impl Send for LibMpv {}
unsafe impl Sync for LibMpv {}

/// Where the shipped set is.
///
/// A release build looks in exactly one place, the `libmpv` directory of
/// Vela's own resources. A development build may be pointed at a set built
/// somewhere else (`VELA_LIBMPV_DIR`) so a developer does not have to install
/// one into the build tree; a release build ignores that variable, so no
/// environment can make a shipped Vela load a library that is not its own.
pub(crate) fn set_directory(
    resource_dir: &Path,
    development_build: bool,
    development_override: Option<&std::ffi::OsStr>,
) -> PathBuf {
    match development_override.filter(|dir| !dir.is_empty()) {
        Some(dir) if development_build => PathBuf::from(dir),
        _ => resource_dir.join("libmpv"),
    }
}

/// Why a path cannot be handed to the system's loader as it is.
///
/// The loader treats a name without a directory as a request to SEARCH for a
/// library, and a relative path depends on the current directory. Either
/// could end in a library that is not Vela's.
fn refuse_unless_exact(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("its path is not absolute".to_string());
    }
    if path.file_name() != Some(std::ffi::OsStr::new(LIBRARY_FILE)) {
        return Err(format!("it is not named {LIBRARY_FILE}"));
    }
    Ok(())
}

fn last_loader_error() -> String {
    // SAFETY: dlerror returns NULL or a NUL-terminated string owned by the
    // loader, valid until the next loader call on this thread.
    let message = unsafe { libc::dlerror() };
    if message.is_null() {
        "the system's loader gave no reason".to_string()
    } else {
        unsafe { CStr::from_ptr(message) }
            .to_string_lossy()
            .into_owned()
    }
}

impl LibMpv {
    /// Open the library at exactly `path` and resolve every function used.
    pub(crate) fn load(path: &Path) -> Result<Self, String> {
        refuse_unless_exact(path)?;
        use std::os::unix::ffi::OsStrExt;
        let c_path = CString::new(path.as_os_str().as_bytes())
            .map_err(|_| "its path contains a NUL byte".to_string())?;
        // NOW: a missing symbol fails here, not in the middle of a play.
        // LOCAL: the library's symbols are not offered to the rest of the
        // process. DEEPBIND: the library resolves its own references inside
        // its own set first. Vela's process already holds the system's
        // FreeType, HarfBuzz, FriBidi and dav1d (the web view needs them);
        // without this the shipped copies would be bypassed.
        let flags = libc::RTLD_NOW | libc::RTLD_LOCAL | libc::RTLD_DEEPBIND;
        // SAFETY: `c_path` is a valid C string. Loading runs the library's
        // initialisers; the file is Vela's own shipped library.
        let handle = unsafe { libc::dlopen(c_path.as_ptr(), flags) };
        if handle.is_null() {
            return Err(last_loader_error());
        }

        macro_rules! symbol {
            ($name:literal) => {{
                // SAFETY: `handle` is a live library handle and the name is a
                // C string literal.
                let address = unsafe { libc::dlsym(handle, concat!($name, "\0").as_ptr().cast()) };
                if address.is_null() {
                    return Err(format!("it has no function {}", $name));
                }
                // SAFETY: the address is the named mpv function, whose
                // signature is the field's type (see the module comment).
                unsafe { super::function_at(address) }
            }};
        }

        let lib = LibMpv {
            client_api_version: symbol!("mpv_client_api_version"),
            create: symbol!("mpv_create"),
            set_option_string: symbol!("mpv_set_option_string"),
            initialize: symbol!("mpv_initialize"),
            set_property_async: symbol!("mpv_set_property_async"),
            observe_property: symbol!("mpv_observe_property"),
            unobserve_property: symbol!("mpv_unobserve_property"),
            command: symbol!("mpv_command"),
            get_property_string: symbol!("mpv_get_property_string"),
            wait_event: symbol!("mpv_wait_event"),
            set_wakeup_callback: symbol!("mpv_set_wakeup_callback"),
            terminate_destroy: symbol!("mpv_terminate_destroy"),
            error_string: symbol!("mpv_error_string"),
            free: symbol!("mpv_free"),
            render_context_create: symbol!("mpv_render_context_create"),
            render_context_set_update_callback: symbol!("mpv_render_context_set_update_callback"),
            render_context_update: symbol!("mpv_render_context_update"),
            render_context_render: symbol!("mpv_render_context_render"),
            render_context_report_swap: symbol!("mpv_render_context_report_swap"),
            render_context_free: symbol!("mpv_render_context_free"),
        };
        lib.refuse_another_file(path)?;
        let (major, minor) = lib.api_version();
        check_api_version(major, minor)?;
        Ok(lib)
    }

    /// The loader hands back a library it already holds when asked for one
    /// with the same name. Vela loads no other mpv, but if anything in the
    /// process ever did, this is where it would be caught: the functions must
    /// come from the file that was asked for.
    fn refuse_another_file(&self, asked: &Path) -> Result<(), String> {
        let mut info: libc::Dl_info = unsafe { std::mem::zeroed() };
        // SAFETY: `info` is a valid out-parameter; the address is a function
        // in a loaded library.
        let found = unsafe { libc::dladdr(self.create as *const c_void, &mut info) };
        if found == 0 || info.dli_fname.is_null() {
            return Err("the system's loader cannot say which file it came from".to_string());
        }
        use std::os::unix::ffi::OsStrExt;
        // SAFETY: a non-NULL `dli_fname` is a NUL-terminated path owned by
        // the loader for as long as the library stays loaded.
        let from = PathBuf::from(std::ffi::OsStr::from_bytes(
            unsafe { CStr::from_ptr(info.dli_fname) }.to_bytes(),
        ));
        let same = match (std::fs::canonicalize(&from), std::fs::canonicalize(asked)) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        };
        if same {
            Ok(())
        } else {
            Err("another mpv library is already loaded in this process".to_string())
        }
    }

    /// The library's client API version, as (major, minor).
    pub(crate) fn api_version(&self) -> (c_ulong, c_ulong) {
        // SAFETY: takes no arguments and only reads a constant.
        let version = unsafe { (self.client_api_version)() };
        (version >> 16, version & 0xffff)
    }

    /// mpv's text for one of its error codes.
    pub(crate) fn error_text(&self, code: c_int) -> String {
        // SAFETY: returns a static string for every input.
        let text = unsafe { (self.error_string)(code) };
        if text.is_null() {
            format!("mpv error {code}")
        } else {
            unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
        }
    }

    fn check(&self, code: c_int) -> Result<(), String> {
        if code < 0 {
            Err(self.error_text(code))
        } else {
            Ok(())
        }
    }
}

fn check_api_version(major: c_ulong, minor: c_ulong) -> Result<(), String> {
    if major == API_MAJOR && minor >= API_MINOR_AT_LEAST {
        Ok(())
    } else {
        Err(format!(
            "its interface is version {major}.{minor}, and Vela needs {API_MAJOR}.{API_MINOR_AT_LEAST} or a later {API_MAJOR}.x"
        ))
    }
}

/// One mpv player inside Vela's process. Dropping it shuts the player down
/// and waits for it.
pub(crate) struct Player {
    lib: std::sync::Arc<LibMpv>,
    raw: *mut RawHandle,
}

// A handle may be used from any thread, one call at a time or several: mpv's
// client API is thread-safe. Vela nevertheless uses each player from the one
// thread that owns it.
unsafe impl Send for Player {}

fn c_string(what: &str, text: &str) -> Result<CString, String> {
    CString::new(text).map_err(|_| format!("{what} contains a NUL byte"))
}

impl Player {
    /// Create a player that has not been configured or started yet.
    pub(crate) fn create(lib: std::sync::Arc<LibMpv>) -> Result<Self, String> {
        // mpv refuses to start unless numbers are formatted the C way, and
        // GTK switches the process to the user's locale at startup.
        // SAFETY: a valid category and a static C string.
        unsafe { libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr()) };
        // SAFETY: no arguments; returns NULL on failure.
        let raw = unsafe { (lib.create)() };
        if raw.is_null() {
            return Err("mpv could not create a player".to_string());
        }
        Ok(Player { lib, raw })
    }

    /// Set an option before [`Player::initialize`].
    pub(crate) fn set_option(&self, name: &str, value: &str) -> Result<(), String> {
        let c_name = c_string("an option name", name)?;
        let c_value = c_string("an option value", value)?;
        // SAFETY: a live handle and two valid C strings.
        let code = unsafe { (self.lib.set_option_string)(self.raw, c_name.as_ptr(), c_value.as_ptr()) };
        self.lib.check(code)
    }

    pub(crate) fn initialize(&self) -> Result<(), String> {
        // SAFETY: a live, not yet initialised handle.
        let code = unsafe { (self.lib.initialize)(self.raw) };
        self.lib.check(code)
    }

    /// Run an mpv command, such as `["loadfile", url]`.
    pub(crate) fn command(&self, words: &[&str]) -> Result<(), String> {
        let owned = words
            .iter()
            .map(|word| c_string("a command", word))
            .collect::<Result<Vec<_>, _>>()?;
        let mut pointers: Vec<*const c_char> = owned.iter().map(|word| word.as_ptr()).collect();
        pointers.push(std::ptr::null());
        // SAFETY: a live handle and a NULL-terminated array of C strings that
        // outlive the call.
        let code = unsafe { (self.lib.command)(self.raw, pointers.as_mut_ptr()) };
        self.lib.check(code)
    }

    pub(crate) fn property(&self, name: &str) -> Option<String> {
        let c_name = c_string("a property name", name).ok()?;
        // SAFETY: a live handle and a valid C string. The result is NULL or a
        // string mpv allocated, released below with mpv's own free.
        let text = unsafe { (self.lib.get_property_string)(self.raw, c_name.as_ptr()) };
        if text.is_null() {
            return None;
        }
        let value = unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned();
        unsafe { (self.lib.free)(text.cast()) };
        Some(value)
    }

    /// The next waiting event, or [`Event::None`]. Never waits, so it may be
    /// called on the thread that draws (render.h: `mpv_wait_event` with a
    /// timeout of zero is one of the calls allowed there).
    pub(crate) fn next_event(&self) -> Event {
        self.event(0.0)
    }

    /// The next event, waiting up to `limit` for one. A waiting call: only
    /// while no renderer exists for this player, because once one does, mpv
    /// may itself be waiting for the thread that would be waiting here.
    pub(crate) fn wait_for_event(&self, limit: std::time::Duration) -> Event {
        self.event(limit.as_secs_f64())
    }

    fn event(&self, timeout: c_double) -> Event {
        // SAFETY: a live handle. The returned event is valid until the next
        // call on this handle; its fields are copied out here.
        let event = unsafe { (self.lib.wait_event)(self.raw, timeout) };
        if event.is_null() {
            return Event::None;
        }
        let (event_id, error, reply_userdata, data) = unsafe {
            (
                (*event).event_id,
                (*event).error,
                (*event).reply_userdata,
                (*event).data,
            )
        };
        if event_id == EVENT_PROPERTY_CHANGE {
            // SAFETY: this event's data is an `mpv_event_property`, read
            // before the next call on this handle.
            return unsafe { property_change(reply_userdata, data.cast()) };
        }
        classify(event_id, error, reply_userdata)
    }

    /// Have mpv call `callback(context)` whenever an event is waiting. The
    /// callback runs on mpv's threads and must only wake the owner.
    ///
    /// # Safety
    /// `context` must stay valid until the player is dropped or the callback
    /// is replaced.
    pub(crate) unsafe fn set_wakeup(
        &self,
        callback: unsafe extern "C" fn(*mut c_void),
        context: *mut c_void,
    ) {
        unsafe { (self.lib.set_wakeup_callback)(self.raw, Some(callback), context) };
    }

    pub(crate) fn lib(&self) -> &std::sync::Arc<LibMpv> {
        &self.lib
    }

    pub(crate) fn raw(&self) -> *mut RawHandle {
        self.raw
    }
}

impl super::hdr::Targetable for Player {
    /// Ask the running player, without waiting, to change one of the options
    /// that say what it draws for. mpv answers later with an event carrying
    /// `request` ([`Event::Answer`]). Safe on the thread that draws
    /// (`mpv_set_property_async` is on render.h's list); a waiting setter
    /// there can wait out mpv's own timeout for that very thread (review
    /// finding hd-3).
    fn ask(&self, request: u64, option: &str, value: &str) -> bool {
        let (Ok(c_option), Ok(c_value)) = (
            c_string("an option name", option),
            c_string("an option value", value),
        ) else {
            return false;
        };
        // For a string, mpv takes the address of the `char *`, not the
        // pointer itself (client.h, `MPV_FORMAT_STRING`).
        let mut data: *const c_char = c_value.as_ptr();
        // SAFETY: a live handle, a valid C string, and the address of a
        // pointer to another; mpv copies both strings before it returns.
        let code = unsafe {
            (self.lib.set_property_async)(
                self.raw,
                request,
                c_option.as_ptr(),
                FORMAT_STRING,
                (&mut data as *mut *const c_char).cast(),
            )
        };
        // A negative code: the ask was not sent and no answer will come.
        code >= 0
    }

    /// Observe one of those options as text under `registration`. mpv reports
    /// its value as it next reads it, and then every change of it, whoever
    /// makes it, as [`Event::TargetChanged`]. Neither this nor `unobserve`
    /// waits for mpv's core (client.c: they mark the property and wake the
    /// core), so both are safe on the thread that draws.
    fn observe(&self, registration: u64, option: &str) -> bool {
        let Ok(c_option) = c_string("an option name", option) else {
            return false;
        };
        // SAFETY: a live handle and a valid C string, which mpv copies.
        let code = unsafe {
            (self.lib.observe_property)(self.raw, registration, c_option.as_ptr(), FORMAT_STRING)
        };
        code >= 0
    }

    /// Stop the reports made under `registration`.
    fn unobserve(&self, registration: u64) {
        // SAFETY: a live handle. The count of registrations removed is of no
        // use here: none is not an error.
        unsafe { (self.lib.unobserve_property)(self.raw, registration) };
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // SAFETY: the handle is live and never used again. This asks mpv to
        // quit and blocks until every one of its threads has ended.
        unsafe { (self.lib.terminate_destroy)(self.raw) };
    }
}

/// mpv drawing through OpenGL into a framebuffer its owner provides. Must be
/// created, used and dropped on the thread whose OpenGL context it uses, and
/// dropped before its [`Player`].
pub(crate) struct Renderer {
    lib: std::sync::Arc<LibMpv>,
    raw: *mut RawRenderContext,
}

impl Renderer {
    /// # Safety
    /// An OpenGL context must be current on this thread, `get_proc_address`
    /// must resolve its functions, and `render_fd` must be `-1` or an open
    /// DRM render node that outlives the renderer.
    pub(crate) unsafe fn create(
        player: &Player,
        get_proc_address: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
        render_fd: c_int,
    ) -> Result<Self, String> {
        let lib = player.lib().clone();
        let mut gl = OpenGlInitParams {
            get_proc_address: Some(get_proc_address),
            get_proc_address_ctx: std::ptr::null_mut(),
        };
        let mut drm = OpenGlDrmParamsV2 {
            fd: -1,
            crtc_id: -1,
            connector_id: -1,
            atomic_request_ptr: std::ptr::null_mut(),
            render_fd,
        };
        let mut params = vec![
            RenderParam {
                kind: RENDER_PARAM_API_TYPE,
                data: RENDER_API_TYPE_OPENGL.as_ptr() as *mut c_void,
            },
            RenderParam {
                kind: RENDER_PARAM_OPENGL_INIT_PARAMS,
                data: (&mut gl as *mut OpenGlInitParams).cast(),
            },
        ];
        if render_fd >= 0 {
            // Hardware decoding needs to know which GPU to use. mpv would
            // normally learn that from its own window; here it is told.
            params.push(RenderParam {
                kind: RENDER_PARAM_DRM_DISPLAY_V2,
                data: (&mut drm as *mut OpenGlDrmParamsV2).cast(),
            });
        }
        params.push(RenderParam {
            kind: RENDER_PARAM_END,
            data: std::ptr::null_mut(),
        });
        let mut raw: *mut RawRenderContext = std::ptr::null_mut();
        let code =
            unsafe { (lib.render_context_create)(&mut raw, player.raw(), params.as_mut_ptr()) };
        lib.check(code)?;
        Ok(Renderer { lib, raw })
    }

    /// Have mpv call `callback(context)` when there is a new frame to draw.
    ///
    /// # Safety
    /// As for [`Player::set_wakeup`].
    pub(crate) unsafe fn set_update(
        &self,
        callback: unsafe extern "C" fn(*mut c_void),
        context: *mut c_void,
    ) {
        unsafe { (self.lib.render_context_set_update_callback)(self.raw, Some(callback), context) };
    }

    /// Whether mpv has a new frame since the last call.
    pub(crate) fn has_new_frame(&self) -> bool {
        // SAFETY: a live render context on its own thread.
        unsafe { (self.lib.render_context_update)(self.raw) & RENDER_UPDATE_FRAME != 0 }
    }

    /// Draw the current frame into the default framebuffer, `width` by
    /// `height` pixels with `depth` bits per channel, bottom row first.
    pub(crate) fn draw(&self, width: i32, height: i32, depth: i32) {
        let mut fbo = OpenGlFbo {
            fbo: 0,
            w: width,
            h: height,
            internal_format: 0,
        };
        let mut flip: c_int = 1;
        let mut depth: c_int = depth;
        let mut params = [
            RenderParam {
                kind: RENDER_PARAM_OPENGL_FBO,
                data: (&mut fbo as *mut OpenGlFbo).cast(),
            },
            RenderParam {
                kind: RENDER_PARAM_FLIP_Y,
                data: (&mut flip as *mut c_int).cast(),
            },
            RenderParam {
                kind: RENDER_PARAM_DEPTH,
                data: (&mut depth as *mut c_int).cast(),
            },
            RenderParam {
                kind: RENDER_PARAM_END,
                data: std::ptr::null_mut(),
            },
        ];
        // SAFETY: a live render context whose OpenGL context is current, and
        // parameters that outlive the call.
        unsafe { (self.lib.render_context_render)(self.raw, params.as_mut_ptr()) };
    }

    /// Tell mpv the frame it drew has been dealt with; it times the next one
    /// from this. Called for every frame drawn, presented or held: after a
    /// frame, mpv's video thread waits up to 200 ms for this call.
    pub(crate) fn frame_shown(&self) {
        // SAFETY: a live render context on its own thread.
        unsafe { (self.lib.render_context_report_swap)(self.raw) };
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        // SAFETY: live, never used again, and the OpenGL context it was
        // created with is current on this thread (the type's contract).
        unsafe { (self.lib.render_context_free)(self.raw) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsStr;

    #[test]
    fn a_release_build_looks_only_inside_velas_own_resources() {
        let resources = Path::new("/usr/lib/Vela");
        let elsewhere = OsStr::new("/home/dev/vela/.libmpv/out/libmpv");
        assert_eq!(
            set_directory(resources, false, Some(elsewhere)),
            Path::new("/usr/lib/Vela/libmpv"),
            "a release build ignores the development variable"
        );
        assert_eq!(
            set_directory(resources, false, None),
            Path::new("/usr/lib/Vela/libmpv")
        );
        // A development build may be pointed elsewhere, and falls back to the
        // same place when it is not.
        assert_eq!(set_directory(resources, true, Some(elsewhere)), Path::new(elsewhere));
        assert_eq!(
            set_directory(resources, true, None),
            Path::new("/usr/lib/Vela/libmpv")
        );
        assert_eq!(
            set_directory(resources, true, Some(OsStr::new(""))),
            Path::new("/usr/lib/Vela/libmpv"),
            "an empty variable is not a directory"
        );
    }

    #[test]
    fn only_an_exact_file_is_ever_handed_to_the_loader() {
        // A bare name makes the system's loader search its own directories,
        // which is how a machine's own mpv library would get in.
        for searched in ["libmpv.so.2", "libmpv.so", "./libmpv.so.2", "libmpv/libmpv.so.2"] {
            let refused = LibMpv::load(Path::new(searched)).err();
            assert_eq!(
                refused.as_deref(),
                Some("its path is not absolute"),
                "{searched} must be refused before the loader sees it"
            );
        }
        for other in ["/usr/lib/libmpv.so", "/usr/lib/libavcodec.so.63", "/usr/lib/"] {
            let refused = LibMpv::load(Path::new(other)).err();
            assert_eq!(
                refused.as_deref(),
                Some("it is not named libmpv.so.2"),
                "{other} must be refused before the loader sees it"
            );
        }
    }

    #[test]
    fn a_missing_library_reports_the_loaders_own_reason() {
        let missing = Path::new("/nonexistent-vela-test/libmpv/libmpv.so.2");
        let reason = LibMpv::load(missing).err().expect("nothing is there to load");
        assert!(
            reason.contains("No such file") || reason.contains("cannot open"),
            "the reason must be the loader's: {reason}"
        );
    }

    #[test]
    fn only_a_compatible_interface_version_is_accepted() {
        assert!(check_api_version(2, 5).is_ok());
        assert!(check_api_version(2, 6).is_ok());
        assert!(check_api_version(2, 40).is_ok());
        assert!(check_api_version(2, 4).is_err(), "older than what is used");
        assert!(check_api_version(1, 109).is_err(), "the previous interface");
        assert!(check_api_version(3, 0).is_err(), "a later, incompatible one");
        assert_eq!(
            check_api_version(3, 0).unwrap_err(),
            "its interface is version 3.0, and Vela needs 2.5 or a later 2.x"
        );
    }

    #[test]
    fn mpvs_events_are_read_as_what_they_are() {
        assert_eq!(
            classify(4, 0, 7),
            Event::Answer {
                request: 7,
                taken: true
            }
        );
        // A value mpv refuses comes back as a negative error in the answer.
        assert_eq!(
            classify(4, -9, 7),
            Event::Answer {
                request: 7,
                taken: false
            }
        );
        assert_eq!(classify(1, 0, 0), Event::Shutdown);
        assert_eq!(classify(0, 0, 0), Event::None);
        // Anything else, such as idle (11). A property change (22) is read
        // by `property_change`, never here.
        for other in [2, 3, 6, 11, 24] {
            assert_eq!(classify(other, 0, 9), Event::Other, "{other}");
        }
    }

    #[test]
    fn a_property_change_is_read_as_its_name_and_its_text() {
        let name = CString::new("an-option").unwrap();
        let text = CString::new("bt.1886").unwrap();
        let mut data: *const c_char = text.as_ptr();
        let mut property = RawProperty {
            name: name.as_ptr(),
            format: FORMAT_STRING,
            data: (&mut data as *mut *const c_char).cast(),
        };
        // SAFETY: a property laid out as mpv lays it out, alive for the call.
        let read = |property: &RawProperty| unsafe { property_change(57, property) };
        assert_eq!(
            read(&property),
            Event::TargetChanged {
                registration: 57,
                option: "an-option".to_string(),
                value: Some("bt.1886".to_string())
            }
        );
        // A property mpv cannot read: format 0, no data.
        property.format = 0;
        property.data = std::ptr::null_mut();
        assert_eq!(
            read(&property),
            Event::TargetChanged {
                registration: 57,
                option: "an-option".to_string(),
                value: None
            }
        );
        // Any other format is not text, and is not read as text.
        property.format = 4;
        property.data = (&mut data as *mut *const c_char).cast();
        assert_eq!(
            read(&property),
            Event::TargetChanged {
                registration: 57,
                option: "an-option".to_string(),
                value: None
            }
        );
        // SAFETY: NULL is allowed.
        assert_eq!(unsafe { property_change(57, std::ptr::null()) }, Event::Other);
    }
}
