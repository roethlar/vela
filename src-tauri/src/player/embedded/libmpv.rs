//! The mpv library Vela ships, loaded at run time.
//!
//! Nothing here is linked at build time and nothing falls back to a library
//! the machine happens to have (owner ruling D7 and licence rule D4 of
//! `.agents/plans/embedded-player.md`): the one library opened is the file at
//! the absolute path the caller names, which is the copy inside Vela's own
//! bundle (Frameworks on macOS). If it cannot be opened the embedded player is unavailable and
//! the play goes to the external player.
//!
//! The functions are called through a hand-written table. Their signatures
//! and the constants below are transcribed from mpv's public headers
//! (`client.h`, `render.h`, `render_gl.h`, client API 2.5 and 2.6).

use std::ffi::{c_char, c_double, c_int, c_ulong, c_void, CStr, CString};
use std::path::{Path, PathBuf};

/// The file name of the library inside the shipped set.
#[cfg(target_os = "linux")]
pub(crate) const LIBRARY_FILE: &str = "libmpv.so.2";
#[cfg(target_os = "windows")]
pub(crate) const LIBRARY_FILE: &str = WINDOWS_LIBRARY_FILE;
#[cfg(any(target_os = "windows", test))]
const WINDOWS_LIBRARY_FILE: &str = "libmpv-2.dll";

#[cfg(target_os = "macos")]
pub(crate) const LIBRARY_FILE: &str = MACOS_LIBRARY_FILE;
#[cfg(any(target_os = "macos", all(test, unix)))]
const MACOS_LIBRARY_FILE: &str = "libmpv.2.dylib";

/// The client API this code was written against. A library with a different
/// major number has changed incompatibly; a lower minor number lacks
/// something used here.
const API_MAJOR: c_ulong = 2;
const API_MINOR_AT_LEAST: c_ulong = 5;

#[repr(C)]
pub(crate) struct RawHandle {
    _opaque: [u8; 0],
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[repr(C)]
pub(crate) struct RawRenderContext {
    _opaque: [u8; 0],
}

/// `mpv_render_param`.
#[cfg(any(target_os = "linux", target_os = "macos", test))]
#[repr(C)]
pub(crate) struct RenderParam {
    pub(crate) kind: c_int,
    pub(crate) data: *mut c_void,
}

#[cfg(any(target_os = "linux", target_os = "macos", test))]
pub(crate) const RENDER_PARAM_END: c_int = 0;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) const RENDER_PARAM_API_TYPE: c_int = 1;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) const RENDER_PARAM_OPENGL_INIT_PARAMS: c_int = 2;
#[cfg(any(target_os = "linux", target_os = "macos", test))]
pub(crate) const RENDER_PARAM_OPENGL_FBO: c_int = 3;
#[cfg(any(target_os = "linux", target_os = "macos", test))]
pub(crate) const RENDER_PARAM_FLIP_Y: c_int = 4;
#[cfg(any(target_os = "linux", target_os = "macos", test))]
pub(crate) const RENDER_PARAM_DEPTH: c_int = 5;
#[cfg(any(target_os = "macos", test))]
const RENDER_PARAM_BLOCK_FOR_TARGET_TIME: c_int = 12;
#[cfg(any(target_os = "macos", test))]
const RENDER_PARAM_SKIP_RENDERING: c_int = 13;
#[cfg(target_os = "linux")]
pub(crate) const RENDER_PARAM_DRM_DISPLAY_V2: c_int = 16;
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) const RENDER_API_TYPE_OPENGL: &CStr = c"opengl";
/// `MPV_RENDER_UPDATE_FRAME`.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) const RENDER_UPDATE_FRAME: u64 = 1;

/// `mpv_opengl_init_params`.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[repr(C)]
pub(crate) struct OpenGlInitParams {
    pub(crate) get_proc_address:
        Option<unsafe extern "C" fn(ctx: *mut c_void, name: *const c_char) -> *mut c_void>,
    pub(crate) get_proc_address_ctx: *mut c_void,
}

/// `mpv_opengl_fbo`.
#[cfg(any(target_os = "linux", target_os = "macos", test))]
#[repr(C)]
pub(crate) struct OpenGlFbo {
    pub(crate) fbo: c_int,
    pub(crate) w: c_int,
    pub(crate) h: c_int,
    pub(crate) internal_format: c_int,
}

/// `mpv_opengl_drm_params_v2`. Only `render_fd` is used: it names the GPU for
/// hardware decoding. The other fields describe a display Vela does not drive.
#[cfg(target_os = "linux")]
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
    #[cfg(target_os = "macos")]
    StartFile,
    /// One atomic video-params snapshot; never mix a new transfer with an
    /// old file's primaries. The macOS host alone needs the source colour.
    #[cfg(target_os = "macos")]
    VideoParams {
        registration: u64,
        transfer: Option<String>,
        primaries: Option<String>,
    },
    /// The answer to an ask ([`super::hdr::Targetable::ask`]) made with
    /// `request`: whether mpv took the value.
    Answer { request: u64, taken: bool },
    /// One of mpv's targets has changed, by Vela's ask or by anything else,
    /// or is reported afresh. Observations protect colour targets and macOS
    /// video timing; `registration` is
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
        #[cfg(target_os = "macos")]
        6 => Event::StartFile, // MPV_EVENT_START_FILE
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
    #[cfg(target_os = "macos")]
    if option == "video-params" {
        let (transfer, primaries) = unsafe { video_params(property) };
        return Event::VideoParams {
            registration,
            transfer,
            primaries,
        };
    }
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

// mpv_node / mpv_node_list, client.h. Only STRING and NODE_MAP are read;
// the 64-bit union member preserves the size/alignment of all other cases.
#[cfg(any(target_os = "macos", test))]
const FORMAT_NODE: c_int = 6;
#[cfg(any(target_os = "macos", test))]
const FORMAT_NODE_MAP: c_int = 8;

#[cfg(any(target_os = "macos", test))]
#[repr(C)]
union NodeData {
    string: *const c_char,
    list: *const NodeList,
    _alignment: i64,
}
#[cfg(any(target_os = "macos", test))]
#[repr(C)]
struct Node {
    data: NodeData,
    format: c_int,
}
#[cfg(any(target_os = "macos", test))]
#[repr(C)]
struct NodeList {
    count: c_int,
    values: *const Node,
    keys: *const *const c_char,
}

#[cfg(any(target_os = "macos", test))]
unsafe fn video_params(property: &RawProperty) -> (Option<String>, Option<String>) {
    let mut transfer = None;
    let mut primaries = None;
    // SAFETY: the event owns a node for FORMAT_NODE until next_event.
    if property.format == FORMAT_NODE && !property.data.is_null() {
        let node = unsafe { &*property.data.cast::<Node>() };
        if node.format == FORMAT_NODE_MAP {
            let list = unsafe { node.data.list.as_ref() };
            if let Some(list) =
                list.filter(|l| l.count > 0 && !l.values.is_null() && !l.keys.is_null())
            {
                for i in 0..list.count as usize {
                    let key = unsafe { *list.keys.add(i) };
                    let value = unsafe { &*list.values.add(i) };
                    if key.is_null() || value.format != FORMAT_STRING {
                        continue;
                    }
                    let text = unsafe { value.data.string };
                    if text.is_null() {
                        continue;
                    }
                    let slot = match unsafe { CStr::from_ptr(key) }.to_bytes() {
                        b"gamma" => &mut transfer,
                        b"primaries" => &mut primaries,
                        _ => continue,
                    };
                    *slot = Some(
                        unsafe { CStr::from_ptr(text) }
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
    }
    (transfer, primaries)
}

type Callback = Option<unsafe extern "C" fn(ctx: *mut c_void)>;

/// The loaded library and the functions Vela calls in it.
pub(crate) struct LibMpv {
    client_api_version: unsafe extern "C" fn() -> c_ulong,
    create: unsafe extern "C" fn() -> *mut RawHandle,
    set_option_string:
        unsafe extern "C" fn(*mut RawHandle, *const c_char, *const c_char) -> c_int,
    initialize: unsafe extern "C" fn(*mut RawHandle) -> c_int,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    set_property_async:
        unsafe extern "C" fn(*mut RawHandle, u64, *const c_char, c_int, *mut c_void) -> c_int,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    observe_property: unsafe extern "C" fn(*mut RawHandle, u64, *const c_char, c_int) -> c_int,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    unobserve_property: unsafe extern "C" fn(*mut RawHandle, u64) -> c_int,
    command: unsafe extern "C" fn(*mut RawHandle, *mut *const c_char) -> c_int,
    command_async: unsafe extern "C" fn(*mut RawHandle, u64, *mut *const c_char) -> c_int,
    get_property_string: unsafe extern "C" fn(*mut RawHandle, *const c_char) -> *mut c_char,
    wait_event: unsafe extern "C" fn(*mut RawHandle, c_double) -> *mut RawEvent,
    set_wakeup_callback: unsafe extern "C" fn(*mut RawHandle, Callback, *mut c_void),
    terminate_destroy: unsafe extern "C" fn(*mut RawHandle),
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    free: unsafe extern "C" fn(*mut c_void),
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    render_context_create:
        unsafe extern "C" fn(*mut *mut RawRenderContext, *mut RawHandle, *mut RenderParam) -> c_int,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    render_context_set_update_callback:
        unsafe extern "C" fn(*mut RawRenderContext, Callback, *mut c_void),
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    render_context_update: unsafe extern "C" fn(*mut RawRenderContext) -> u64,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    render_context_render: unsafe extern "C" fn(*mut RawRenderContext, *mut RenderParam) -> c_int,
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    render_context_report_swap: unsafe extern "C" fn(*mut RawRenderContext),
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    render_context_free: unsafe extern "C" fn(*mut RawRenderContext),
}

// The table is plain function pointers into a library that stays loaded for
// the life of the process; mpv's client API may be called from any thread.
unsafe impl Send for LibMpv {}
unsafe impl Sync for LibMpv {}

/// Where the shipped set is.
///
/// A release build looks in exactly one place: beside the executable in
/// `../Frameworks` on macOS, or `libmpv` in Vela's resources elsewhere.
/// A development build may be pointed at a set built
/// somewhere else (`VELA_LIBMPV_DIR`) so a developer does not have to install
/// one into the build tree; a release build ignores that variable, so no
/// environment can make a shipped Vela load a library that is not its own.
pub(crate) fn set_directory(
    resource_dir: &Path,
    development_build: bool,
    development_override: Option<&std::ffi::OsStr>,
) -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        let _ = resource_dir;
        macos_set_directory(std::env::current_exe().ok().as_deref(), development_build, development_override)
    }
    #[cfg(not(target_os = "macos"))]
    resource_set_directory(resource_dir, development_build, development_override)
}

#[cfg(any(not(target_os = "macos"), test))]
fn resource_set_directory(
    resource_dir: &Path,
    development_build: bool,
    development_override: Option<&std::ffi::OsStr>,
) -> PathBuf {
    match development_override.filter(|dir| !dir.is_empty()) {
        Some(dir) if development_build => PathBuf::from(dir),
        _ => resource_dir.join("libmpv"),
    }
}

#[cfg(any(target_os = "macos", all(test, unix)))]
fn macos_set_directory(
    executable: Option<&Path>,
    development_build: bool,
    development_override: Option<&std::ffi::OsStr>,
) -> PathBuf {
    if let Some(dir) = development_override.filter(|dir| development_build && !dir.is_empty()) {
        return PathBuf::from(dir);
    }
    // No resource/system/CWD fallback if current_exe fails: the resulting
    // relative library name is rejected by refuse_unless_exact before dlopen.
    executable.filter(|path| path.is_absolute()).and_then(Path::parent)
        .map(|dir| dir.join("../Frameworks")).unwrap_or_default()
}

/// Why a path cannot be handed to the system's loader as it is.
///
/// The loader treats a name without a directory as a request to SEARCH for a
/// library, and a relative path depends on the current directory. Either
/// could end in a library that is not Vela's.
fn refuse_unless_exact(path: &Path) -> Result<(), String> {
    refuse_unless_named(path, LIBRARY_FILE)
}

fn refuse_unless_named(path: &Path, name: &str) -> Result<(), String> {
    if !path.is_absolute() {
        return Err("its path is not absolute".to_string());
    }
    if path.file_name() != Some(std::ffi::OsStr::new(name)) {
        return Err(format!("it is not named {name}"));
    }
    Ok(())
}

/// Apple's dyld(1) search-path, versioned-override, suffix, insertion and
/// root-path controls can replace or add images in an unhardened launch.
/// Refuse their presence (even an empty value) before running initialisers.
/// Inspect names only: values can contain private paths and are never logged.
#[cfg(any(target_os = "macos", test))]
fn refuse_macos_loader_environment(
    names: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Result<(), String> {
    const REDIRECTS: &[&str] = &[
        "DYLD_LIBRARY_PATH",
        "DYLD_FALLBACK_LIBRARY_PATH",
        "DYLD_FRAMEWORK_PATH",
        "DYLD_FALLBACK_FRAMEWORK_PATH",
        "DYLD_IMAGE_SUFFIX",
        "DYLD_INSERT_LIBRARIES",
        "DYLD_VERSIONED_LIBRARY_PATH",
        "DYLD_VERSIONED_FRAMEWORK_PATH",
        // dyld 4 (macOS 12 on) applies this only to simulator executables,
        // but macOS 11's dyld-832 prefixes it to a native process's absolute
        // load paths, and the set supports macOS 11 (pl-25).
        "DYLD_ROOT_PATH",
    ];
    for name in names {
        if let Some(redirect) = REDIRECTS
            .iter()
            .find(|redirect| name.as_ref() == std::ffi::OsStr::new(redirect))
        {
            return Err(format!("{redirect} can redirect the set's libraries"));
        }
    }
    Ok(())
}

/// Every loaded image named by the set must resolve inside that directory.
/// The caller supplies canonical paths; retain the loader's original name
/// too, since a dylib symlink can resolve to a differently named file. An
/// unresolved set image fails closed, while unrelated system images (which
/// can exist only in dyld's shared cache) need no filesystem counterpart.
#[cfg(any(target_os = "macos", test))]
fn refuse_images_outside_set(
    canonical_directory: &Path,
    dylib_names: &[std::ffi::OsString],
    images: &[(PathBuf, Option<PathBuf>)],
) -> Result<(), String> {
    for (loaded, canonical) in images {
        if loaded
            .file_name()
            .is_some_and(|name| dylib_names.iter().any(|own| own == name))
            && canonical.as_deref().and_then(Path::parent) != Some(canonical_directory)
        {
            return Err(format!(
                "{} was loaded from outside the set's directory",
                loaded.file_name().unwrap_or_default().to_string_lossy()
            ));
        }
    }
    Ok(())
}

/// A NULL entry or a changed count means an image may have been unloaded or
/// added during enumeration. Discard that whole sample and try again; never
/// pass an incomplete sample to the image-origin policy.
#[cfg(any(target_os = "macos", test))]
fn stable_macos_images<T>(
    mut count: impl FnMut() -> u32,
    mut image: impl FnMut(u32) -> Option<T>,
) -> Result<Vec<T>, String> {
    for _ in 0..3 {
        let before = count();
        let images: Option<Vec<T>> = (0..before).map(&mut image).collect();
        let after = count();
        if before == after {
            if let Some(images) = images {
                return Ok(images);
            }
        }
    }
    Err("the system's loader image list did not settle after 3 scans".into())
}

#[cfg(target_os = "macos")]
fn refuse_redirected_macos_images(library: &Path) -> Result<(), String> {
    use std::os::unix::ffi::OsStrExt;

    unsafe extern "C" {
        fn _dyld_image_count() -> u32;
        fn _dyld_get_image_name(index: u32) -> *const c_char;
    }

    let unknown = || "the system's loader cannot say which file it came from".to_string();
    let directory = library
        .parent()
        .ok_or_else(unknown)?
        .canonicalize()
        .map_err(|_| unknown())?;
    let mut dylib_names = Vec::new();
    for entry in std::fs::read_dir(&directory).map_err(|_| unknown())? {
        let entry = entry.map_err(|_| unknown())?;
        let path = entry.path();
        // is_file follows symlinks, including the versioned dylib aliases.
        if path.extension() == Some(std::ffi::OsStr::new("dylib")) && path.is_file() {
            dylib_names.push(entry.file_name());
        }
    }

    // SAFETY: dyld's image accessors take no pointers from us. Copy each
    // non-NULL, NUL-terminated name while the library is still loaded.
    let images = stable_macos_images(
        || unsafe { _dyld_image_count() },
        |index| {
            let name = unsafe { _dyld_get_image_name(index) };
            if name.is_null() {
                return None;
            }
            Some(PathBuf::from(std::ffi::OsStr::from_bytes(
                unsafe { CStr::from_ptr(name) }.to_bytes(),
            )))
        },
    )?;
    // Filesystem work need not lengthen the interval between count samples.
    let images: Vec<_> = images
        .into_iter()
        .map(|path| {
            let canonical = path.canonicalize().ok();
            (path, canonical)
        })
        .collect();
    refuse_images_outside_set(&directory, &dylib_names, &images)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
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

/// Do not run any library initialisers unless later bare-name DLL loads are
/// restricted too. Keep the failure decision testable without Windows.
#[cfg(any(target_os = "windows", test))]
fn load_after_dll_policy<T>(
    restrict: impl FnOnce() -> Result<(), String>,
    load: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    restrict().map_err(|error| format!("could not restrict Windows DLL search directories: {error}"))?;
    load()
}

impl LibMpv {
    /// Open the library at exactly `path` and resolve every function used.
    pub(crate) fn load(path: &Path) -> Result<Self, String> {
        refuse_unless_exact(path)?;
        #[cfg(target_os = "macos")]
        refuse_macos_loader_environment(std::env::vars_os().map(|(name, _)| name))?;
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        let handle = {
            use std::os::unix::ffi::OsStrExt;
            let c_path = CString::new(path.as_os_str().as_bytes())
                .map_err(|_| "its path contains a NUL byte".to_string())?;
            // NOW: a missing symbol fails here, not in the middle of a play.
            // LOCAL: the library's symbols are not offered to the rest of the
            // process. DEEPBIND: the library resolves its own references inside
            // its own set first. Vela's process already holds the system's
            // FreeType, HarfBuzz, FriBidi and dav1d (the web view needs them);
            // without this the shipped copies would be bypassed.
            #[cfg(target_os = "linux")]
            let flags = libc::RTLD_NOW | libc::RTLD_LOCAL | libc::RTLD_DEEPBIND;
            // Mach-O's two-level namespace binds references to their linked
            // dylib, so the shipped @rpath/@loader_path set needs no DEEPBIND.
            // Apple: Porting Unix / Compiling Your Code in OS X, "Namespaces".
            #[cfg(target_os = "macos")]
            let flags = libc::RTLD_NOW | libc::RTLD_LOCAL;
            // SAFETY: `c_path` is a valid C string. Loading runs the library's
            // initialisers; the file is Vela's own shipped library.
            let handle = unsafe { libc::dlopen(c_path.as_ptr(), flags) };
            if handle.is_null() {
                return Err(last_loader_error());
            }

            // A development launch can honour DYLD_* redirects for a
            // dependency even when mpv_create itself comes from our set.
            #[cfg(target_os = "macos")]
            if let Err(error) = refuse_redirected_macos_images(path) {
                // SAFETY: release this dlopen reference before any function
                // from the refused library is used or retained.
                unsafe { libc::dlclose(handle) };
                return Err(error);
            }

            handle
        };
        #[cfg(target_os = "windows")]
        let handle = {
            use std::os::windows::ffi::OsStrExt;
            use windows::{
                core::PCWSTR,
                Win32::System::LibraryLoader::{
                    LoadLibraryExW, SetDefaultDllDirectories, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
                    LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
                    LOAD_LIBRARY_SEARCH_USER_DIRS,
                },
            };
            let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
            if wide.contains(&0) {
                return Err("its path contains a NUL byte".into());
            }
            wide.push(0);
            load_after_dll_policy(
                || {
                    // The per-load flags below do not govern later DLL loads.
                    // libplacebo later loads D3D11/DXGI by bare name: exclude
                    // the application directory, CWD and PATH process-wide,
                    // before even the set's initialisers run. Vela adds no
                    // user DLL directories. Keep this policy for the process's
                    // lifetime; external mpv is a separate process.
                    // SAFETY: documented search flags, no pointer arguments.
                    unsafe {
                        SetDefaultDllDirectories(
                            LOAD_LIBRARY_SEARCH_SYSTEM32 | LOAD_LIBRARY_SEARCH_USER_DIRS,
                        )
                    }
                    .map_err(|error| error.to_string())
                },
                || {
                    // SAFETY: absolute, terminated path. Static imports still
                    // search this DLL's own directory and Windows' defaults.
                    // Explicit flags override the process policy: FFmpeg's
                    // own dynamic loader also opts into APPLICATION_DIR.
                    unsafe {
                        LoadLibraryExW(
                            PCWSTR(wide.as_ptr()),
                            None,
                            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
                        )
                    }
                    .map_err(|error| error.to_string())
                },
            )?
        };

        macro_rules! symbol {
            ($name:literal) => {{
                // SAFETY: `handle` is a live library handle and the name is a
                // C string literal.
                #[cfg(any(target_os = "linux", target_os = "macos"))]
                let address = unsafe { libc::dlsym(handle, concat!($name, "\0").as_ptr().cast()) };
                #[cfg(target_os = "windows")]
                let address = unsafe {
                    windows::Win32::System::LibraryLoader::GetProcAddress(
                        handle,
                        windows::core::PCSTR(concat!($name, "\0").as_ptr()),
                    )
                }
                .map_or(std::ptr::null_mut(), |function| function as *mut c_void);
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
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            set_property_async: symbol!("mpv_set_property_async"),
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            observe_property: symbol!("mpv_observe_property"),
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            unobserve_property: symbol!("mpv_unobserve_property"),
            command: symbol!("mpv_command"),
            command_async: symbol!("mpv_command_async"),
            get_property_string: symbol!("mpv_get_property_string"),
            wait_event: symbol!("mpv_wait_event"),
            set_wakeup_callback: symbol!("mpv_set_wakeup_callback"),
            terminate_destroy: symbol!("mpv_terminate_destroy"),
            error_string: symbol!("mpv_error_string"),
            free: symbol!("mpv_free"),
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            render_context_create: symbol!("mpv_render_context_create"),
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            render_context_set_update_callback: symbol!("mpv_render_context_set_update_callback"),
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            render_context_update: symbol!("mpv_render_context_update"),
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            render_context_render: symbol!("mpv_render_context_render"),
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            render_context_report_swap: symbol!("mpv_render_context_report_swap"),
            #[cfg(any(target_os = "linux", target_os = "macos"))]
            render_context_free: symbol!("mpv_render_context_free"),
        };
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        lib.refuse_another_file(path)?;
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::ffi::OsStringExt;
            let mut buffer = vec![0u16; 32768];
            // SAFETY: the handle returned by LoadLibraryExW and writable storage.
            let count = unsafe {
                windows::Win32::System::LibraryLoader::GetModuleFileNameW(Some(handle), &mut buffer)
            } as usize;
            if count == 0 || count >= buffer.len() {
                return Err("the system's loader cannot say which file it came from".into());
            }
            let from = PathBuf::from(std::ffi::OsString::from_wide(&buffer[..count]));
            if !matches!((std::fs::canonicalize(&from), std::fs::canonicalize(path)),
                (Ok(a), Ok(b)) if a == b)
            {
                return Err("another mpv library is already loaded in this process".into());
            }
        }
        let (major, minor) = lib.api_version();
        check_api_version(major, minor)?;
        Ok(lib)
    }

    /// The loader hands back a library it already holds when asked for one
    /// with the same name. Vela loads no other mpv, but if anything in the
    /// process ever did, this is where it would be caught: the functions must
    /// come from the file that was asked for.
    #[cfg(any(target_os = "linux", target_os = "macos"))]
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
        #[cfg(target_os = "linux")]
        unsafe {
            libc::setlocale(libc::LC_NUMERIC, c"C".as_ptr())
        };
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

    /// Ask mpv to end playback before its renderer is freed. Unlike
    /// `command`, this is safe on the thread that draws (client.h): it does
    /// not wait for mpv's core, which may be waiting for that thread.
    ///
    /// The reply is MPV_EVENT_COMMAND_REPLY (5), classified as `Other`,
    /// never an HDR `Answer` (MPV_EVENT_SET_PROPERTY_REPLY). Neither that
    /// reply nor acceptance here means playback has ended: the play keeps
    /// drawing until `Shutdown`, with a deadline even if queuing failed.
    pub(crate) fn request_quit(&self) {
        let mut words = [c"quit".as_ptr(), std::ptr::null()];
        // SAFETY: a live handle and a NULL-terminated array of C strings.
        // mpv copies the arguments before returning; no reply data is used.
        unsafe { (self.lib.command_async)(self.raw, 0, words.as_mut_ptr()) };
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

    #[cfg(target_os = "macos")]
    pub(crate) fn observe_video_params(&self, registration: u64) -> bool {
        // One map, rather than independently sampled gamma/primaries.
        // observe_property only registers work; it does not wait on the core.
        unsafe {
            (self.lib.observe_property)(
                self.raw,
                registration,
                c"video-params".as_ptr(),
                FORMAT_NODE,
            ) >= 0
        }
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
    #[cfg(target_os = "linux")]
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

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(crate) fn lib(&self) -> &std::sync::Arc<LibMpv> {
        &self.lib
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    pub(crate) fn raw(&self) -> *mut RawHandle {
        self.raw
    }
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "macos")]
impl super::session::macos_hdr::TargetAccess for Player {
    fn ask(&self, request: u64, name: &str, value: &str) -> bool {
        let (Ok(name), Ok(value)) = (
            c_string("an option name", name),
            c_string("an option value", value),
        ) else {
            return false;
        };
        let mut data = value.as_ptr();
        // mpv copies the strings before this nonblocking call returns.
        unsafe {
            (self.lib.set_property_async)(
                self.raw,
                request,
                name.as_ptr(),
                FORMAT_STRING,
                (&mut data as *mut *const c_char).cast(),
            ) >= 0
        }
    }
    fn observe(&self, registration: u64, name: &str) -> bool {
        let Ok(name) = c_string("a property name", name) else {
            return false;
        };
        unsafe {
            (self.lib.observe_property)(self.raw, registration, name.as_ptr(), FORMAT_STRING) >= 0
        }
    }
    fn unobserve(&self, registration: u64) {
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

/// mpv drawing through OpenGL into a framebuffer its owner provides. Its
/// context must be current for creation, use and drop, before its [`Player`]
/// is dropped. Linux confines it to one thread; macOS's private wrapper
/// migrates it only with that same CGL context locked and current.
#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(crate) struct Renderer {
    lib: std::sync::Arc<LibMpv>,
    raw: *mut RawRenderContext,
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Renderer {
    /// # Safety
    /// An OpenGL context must be current on this thread, `get_proc_address`
    /// must resolve its functions, and `render_fd` must be `-1` or an open
    /// DRM render node that outlives the renderer.
    pub(crate) unsafe fn create(
        player: &Player,
        get_proc_address: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
        #[cfg(target_os = "linux")] render_fd: c_int,
    ) -> Result<Self, String> {
        let lib = player.lib().clone();
        let mut gl = OpenGlInitParams {
            get_proc_address: Some(get_proc_address),
            get_proc_address_ctx: std::ptr::null_mut(),
        };
        #[cfg(target_os = "linux")]
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
        #[cfg(target_os = "linux")]
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
    /// As for [`Player::set_wakeup`]. The context must live until this
    /// renderer is dropped, which unregisters and waits out the callback.
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
    #[cfg(target_os = "linux")]
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

    #[cfg(target_os = "macos")]
    pub(crate) fn draw_into(
        &self,
        target: i32,
        width: i32,
        height: i32,
        depth: c_int,
    ) -> Result<(), String> {
        self.lib.check(with_macos_draw_params(
            target,
            width,
            height,
            depth,
            |params| {
                // SAFETY: a live render context whose OpenGL context is current;
                // the helper keeps all parameter storage alive through this call.
                unsafe { (self.lib.render_context_render)(self.raw, params.as_mut_ptr()) }
            },
        ))
    }

    /// Consume a frame that CA declined to draw. render.h permits omitting
    /// the target with SKIP_RENDERING, but still requires a current GL context.
    #[cfg(target_os = "macos")]
    pub(crate) fn skip(&self) -> Result<(), String> {
        self.lib.check(with_macos_skip_params(|params| {
            // SAFETY: same current, locked CGL context as draw_into; parameter
            // storage outlives the call. Skipping still needs frame_shown after.
            unsafe { (self.lib.render_context_render)(self.raw, params.as_mut_ptr()) }
        }))
    }

    /// Tell mpv the frame it drew has been dealt with; it times the next one
    /// from this. Called for every frame drawn, presented or held: after a
    /// frame, mpv's video thread waits up to 200 ms for this call.
    pub(crate) fn frame_shown(&self) {
        // SAFETY: a live render context on its own thread.
        unsafe { (self.lib.render_context_report_swap)(self.raw) };
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
impl Drop for Renderer {
    fn drop(&mut self) {
        // SAFETY: live, never used again, and the OpenGL context it was
        // created with is current on this thread (the type's contract).
        // Unregister first: mpv serializes this with any in-flight callback,
        // so its context may be released after the renderer is gone.
        unsafe {
            (self.lib.render_context_set_update_callback)(self.raw, None, std::ptr::null_mut());
            (self.lib.render_context_free)(self.raw);
        }
    }
}

// Main-thread drawing must not wait for mpv's target time (render.h, parameter
// 12). macos_options pairs this with video-timing-offset=0 so the video thread
// hands frames over when due. Keep the parameter storage alive in these helpers
// through the render call, and exercise the same lists in tests on every OS.
#[cfg(any(target_os = "macos", test))]
fn with_macos_draw_params<R>(
    target: i32,
    width: i32,
    height: i32,
    mut depth: c_int,
    render: impl FnOnce(&mut [RenderParam]) -> R,
) -> R {
    let mut fbo = OpenGlFbo {
        fbo: target,
        w: width,
        h: height,
        internal_format: 0,
    };
    let mut flip: c_int = 1;
    let mut block_for_target_time: c_int = 0;
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
            kind: RENDER_PARAM_BLOCK_FOR_TARGET_TIME,
            data: (&mut block_for_target_time as *mut c_int).cast(),
        },
        RenderParam {
            kind: RENDER_PARAM_END,
            data: std::ptr::null_mut(),
        },
    ];
    render(&mut params)
}

#[cfg(any(target_os = "macos", test))]
fn with_macos_skip_params<R>(render: impl FnOnce(&mut [RenderParam]) -> R) -> R {
    let mut skip: c_int = 1;
    let mut block_for_target_time: c_int = 0;
    let mut params = [
        RenderParam {
            kind: RENDER_PARAM_SKIP_RENDERING,
            data: (&mut skip as *mut c_int).cast(),
        },
        RenderParam {
            kind: RENDER_PARAM_BLOCK_FOR_TARGET_TIME,
            data: (&mut block_for_target_time as *mut c_int).cast(),
        },
        RenderParam {
            kind: RENDER_PARAM_END,
            data: std::ptr::null_mut(),
        },
    ];
    render(&mut params)
}

#[cfg(test)]
mod macos_environment_tests {
    use super::refuse_macos_loader_environment;

    #[test]
    fn every_dyld_library_redirect_is_refused_by_name() {
        for name in [
            "DYLD_LIBRARY_PATH",
            "DYLD_FALLBACK_LIBRARY_PATH",
            "DYLD_FRAMEWORK_PATH",
            "DYLD_FALLBACK_FRAMEWORK_PATH",
            "DYLD_IMAGE_SUFFIX",
            "DYLD_INSERT_LIBRARIES",
            "DYLD_VERSIONED_LIBRARY_PATH",
            "DYLD_VERSIONED_FRAMEWORK_PATH",
            // pl-25: native on macOS 11, whose dyld-832 honours it.
            "DYLD_ROOT_PATH",
        ] {
            let error = refuse_macos_loader_environment(["PATH", name, "HOME"]).expect_err(name);
            assert!(error.contains(name));
        }
    }

    #[test]
    fn ordinary_names_and_dyld_diagnostics_do_not_redirect_libraries() {
        assert!(refuse_macos_loader_environment(Vec::<std::ffi::OsString>::new()).is_ok());
        assert!(refuse_macos_loader_environment([
            "PATH",
            "HOME",
            "VELA_LIBMPV_DIR",
            "DYLD_PRINT_LIBRARIES",
            "DYLD_PRINT_SEARCHING",
            "DYLD_LIBRARY_PATH_EXTRA",
            "dyld_library_path",
        ])
        .is_ok());
    }
}

#[cfg(test)]
mod macos_render_tests {
    use super::*;

    #[test]
    fn macos_draw_params_disable_target_time_wait() {
        with_macos_draw_params(7, 1920, 1080, 16, |params| {
            // ABI values from mpv 0.42.0 render.h, including terminator.
            let kinds: Vec<_> = params.iter().map(|param| param.kind).collect();
            assert_eq!(kinds, [3, 4, 5, 12, 0]);
            assert!(params[..4].iter().all(|param| !param.data.is_null()));
            assert!(params[4].data.is_null());
            // SAFETY: the helper owns these correctly typed values throughout
            // the callback, just as it does for mpv_render_context_render.
            unsafe {
                let fbo = &*params[0].data.cast::<OpenGlFbo>();
                assert_eq!(
                    (fbo.fbo, fbo.w, fbo.h, fbo.internal_format),
                    (7, 1920, 1080, 0)
                );
                assert_eq!(*params[1].data.cast::<c_int>(), 1);
                assert_eq!(*params[2].data.cast::<c_int>(), 16);
                assert_eq!(*params[3].data.cast::<c_int>(), 0);
            }
        });
    }

    #[test]
    fn macos_skip_params_disable_target_time_wait_without_a_target() {
        with_macos_skip_params(|params| {
            let kinds: Vec<_> = params.iter().map(|param| param.kind).collect();
            assert_eq!(kinds, [13, 12, 0]);
            assert!(params[..2].iter().all(|param| !param.data.is_null()));
            assert!(params[2].data.is_null());
            // SAFETY: both inputs are c_ints owned by the helper for this call.
            unsafe {
                assert_eq!(*params[0].data.cast::<c_int>(), 1);
                assert_eq!(*params[1].data.cast::<c_int>(), 0);
            }
        });
    }
}

#[cfg(target_os = "linux")]
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
    fn command_replies_are_never_hdr_answers() {
        // MPV_EVENT_COMMAND_REPLY, whether accepted or refused, even with
        // the same userdata as an HDR ask. Only event type 4 is an Answer.
        for request in [0, 1, 7, u64::MAX] {
            for error in [0, -9] {
                assert_eq!(classify(5, error, request), Event::Other);
                assert_eq!(
                    classify(4, error, request),
                    Event::Answer {
                        request,
                        taken: error >= 0
                    }
                );
            }
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

#[cfg(test)]
mod windows_tests {
    use super::*;
    // Exercise the Windows resource rule on macOS too, where the public
    // entry point intentionally selects Frameworks from current_exe.
    #[cfg(target_os = "macos")]
    use super::resource_set_directory as set_directory;

    #[test]
    fn windows_dll_policy_precedes_loading() {
        let restricted = std::cell::Cell::new(false);
        let loaded = load_after_dll_policy(
            || {
                restricted.set(true);
                Ok(())
            },
            || {
                assert!(restricted.get(), "the policy must precede library initialisers");
                Ok("loaded")
            },
        );
        assert_eq!(loaded, Ok("loaded"));
    }

    #[test]
    fn windows_dll_policy_failure_refuses_loading() {
        let refused: Result<(), String> = load_after_dll_policy(
            || Err("access denied".into()),
            || panic!("a failed DLL policy must refuse the embedded library"),
        );
        assert_eq!(
            refused,
            Err("could not restrict Windows DLL search directories: access denied".into())
        );
    }

    #[test]
    fn windows_dll_policy_preserves_loader_failure() {
        let refused: Result<(), String> =
            load_after_dll_policy(|| Ok(()), || Err("missing library".into()));
        assert_eq!(refused, Err("missing library".into()));
    }

    #[test]
    fn windows_library_is_an_exact_dll_in_the_selected_set() {
        let resources = std::env::temp_dir().join("vela-resources");
        let elsewhere = std::env::temp_dir().join("vela-development-set");
        let release = set_directory(&resources, false, Some(elsewhere.as_os_str()))
            .join(WINDOWS_LIBRARY_FILE);
        assert_eq!(release, resources.join("libmpv/libmpv-2.dll"));
        assert!(refuse_unless_named(&release, WINDOWS_LIBRARY_FILE).is_ok());
        assert_eq!(
            set_directory(&resources, true, Some(elsewhere.as_os_str())).join(WINDOWS_LIBRARY_FILE),
            elsewhere.join("libmpv-2.dll")
        );
        for name in ["libmpv-2.dll", "libmpv/libmpv-2.dll", "./libmpv-2.dll"] {
            assert!(refuse_unless_named(Path::new(name), WINDOWS_LIBRARY_FILE).is_err());
        }
        assert!(refuse_unless_named(&resources.join("mpv.exe"), WINDOWS_LIBRARY_FILE).is_err());
    }
}


#[cfg(test)]
mod macos_scan_tests {
    use super::stable_macos_images;

    #[test]
    fn a_stable_image_list_is_used_without_retrying() {
        let mut counts = [2, 2].into_iter();
        assert_eq!(
            stable_macos_images(|| counts.next().unwrap(), Some),
            Ok(vec![0, 1])
        );
        assert_eq!(stable_macos_images(|| 0, Some::<u32>), Ok(vec![]));
    }

    #[test]
    fn changing_counts_discard_the_sample_until_the_list_settles() {
        for first_after in [1, 3] {
            let mut counts = [2, first_after, 2, 2].into_iter();
            let mut calls = 0;
            let images = stable_macos_images(
                || counts.next().unwrap(),
                |_| {
                    calls += 1;
                    Some(calls)
                },
            )
            .unwrap();
            assert_eq!(images, [3, 4], "discard both growth and shrinkage");
        }
    }

    #[test]
    fn a_null_entry_retries_even_when_the_count_has_not_changed() {
        let mut values = [Some("old"), None, Some("new"), Some("newer")].into_iter();
        let images = stable_macos_images(|| 2, |_| values.next().unwrap()).unwrap();
        assert_eq!(images, ["new", "newer"]);
    }

    #[test]
    fn continuous_churn_is_a_bounded_and_explained_refusal() {
        for missing in [false, true] {
            let mut count_calls = 0;
            let error = stable_macos_images(
                || {
                    count_calls += 1;
                    if missing {
                        1
                    } else {
                        count_calls
                    }
                },
                |index| (!missing).then_some(index),
            )
            .unwrap_err();
            assert_eq!(count_calls, 6, "three attempts, with two counts apiece");
            assert!(error.contains("did not settle after 3 scans"));
        }
    }
}

#[cfg(test)]
mod macos_image_tests {
    use super::*;

    #[test]
    fn set_images_must_resolve_inside_the_canonical_set_directory() {
        let root = std::env::temp_dir();
        let directory = root.join("vela-set");
        let own = directory.join("libavcodec.62.dylib");
        let other = root.join("other/libavcodec.62.dylib");
        let names = ["libmpv.2.dylib".into(), "libavcodec.62.dylib".into()];
        for (loaded, canonical, allowed) in [
            (own.clone(), Some(own.clone()), true),
            (other.clone(), Some(other.clone()), false),
            // A system library in dyld's cache need not exist on disk.
            (root.join("usr/lib/libSystem.B.dylib"), None, true),
            // A symlinked set directory is compared by its canonical path.
            (
                root.join("set-link/libavcodec.62.dylib"),
                Some(own.clone()),
                true,
            ),
            // A versioned alias can resolve to a different basename.
            (
                own.clone(),
                Some(directory.join("libavcodec.62.1.dylib")),
                true,
            ),
            // A symlink inside the set must not conceal an outside image,
            // even when the target has a different basename.
            (
                own.clone(),
                Some(root.join("other/libavcodec.62.1.dylib")),
                false,
            ),
            (
                own.clone(),
                Some(directory.join("nested/libavcodec.62.dylib")),
                false,
            ),
            (own, None, false),
        ] {
            let images = [
                (
                    directory.join("libmpv.2.dylib"),
                    Some(directory.join("libmpv.2.dylib")),
                ),
                (loaded.clone(), canonical),
            ];
            assert_eq!(
                refuse_images_outside_set(&directory, &names, &images).is_ok(),
                allowed,
                "{loaded:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn real_symlinks_accept_the_set_alias_and_reject_an_escaping_dylib() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!("vela-images-{}", uuid::Uuid::new_v4()));
        let directory = root.join("set");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("libavcodec.62.1.dylib"), []).unwrap();
        symlink(
            "libavcodec.62.1.dylib",
            directory.join("libavcodec.62.dylib"),
        )
        .unwrap();
        symlink(&directory, root.join("set-link")).unwrap();
        std::fs::write(root.join("outside.dylib"), []).unwrap();
        symlink("../outside.dylib", directory.join("libavformat.62.dylib")).unwrap();
        let canonical_directory = root.join("set-link").canonicalize().unwrap();
        let names = ["libavcodec.62.dylib".into(), "libavformat.62.dylib".into()];
        let image = |path: PathBuf| {
            let canonical = path.canonicalize().ok();
            (path, canonical)
        };
        assert!(refuse_images_outside_set(
            &canonical_directory,
            &names,
            &[image(root.join("set-link/libavcodec.62.dylib"))],
        )
        .is_ok());
        let refused = refuse_images_outside_set(
            &canonical_directory,
            &names,
            &[image(directory.join("libavformat.62.dylib"))],
        );
        std::fs::remove_dir_all(root).unwrap();
        assert!(refused.is_err());
    }
}

#[cfg(all(test, unix))]
mod macos_tests {
    use super::*;
    #[test]
    fn macos_library_is_an_exact_absolute_dylib_in_the_shipped_set() {
        let executable = Path::new("/Applications/Vela.app/Contents/MacOS/vela");
        let path = macos_set_directory(Some(executable), false, Some("/other".as_ref()))
            .join(MACOS_LIBRARY_FILE);
        assert_eq!(path, Path::new("/Applications/Vela.app/Contents/MacOS/../Frameworks/libmpv.2.dylib"));
        assert!(refuse_unless_named(&path, MACOS_LIBRARY_FILE).is_ok());
        for bad in ["libmpv.2.dylib", "./libmpv.2.dylib", "/tmp/libmpv.dylib", "/tmp/libmpv.so.2"] {
            assert!(refuse_unless_named(Path::new(bad), MACOS_LIBRARY_FILE).is_err(), "{bad}");
        }
        assert_eq!(macos_set_directory(Some(executable), true, Some("/dev/set".as_ref()))
            .join(MACOS_LIBRARY_FILE), Path::new("/dev/set/libmpv.2.dylib"));
        for development in [false, true] {
            for empty in [None, Some("".as_ref())] {
                assert_eq!(macos_set_directory(Some(executable), development, empty)
                    .join(MACOS_LIBRARY_FILE), path);
            }
        }
        for missing in [None, Some(Path::new("vela"))] {
            let path = macos_set_directory(missing, false, Some("/other".as_ref()))
                .join(MACOS_LIBRARY_FILE);
            assert!(refuse_unless_named(&path, MACOS_LIBRARY_FILE).is_err());
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_production_lookup_uses_the_executable_not_resources() {
        let expected = std::env::current_exe().unwrap().parent().unwrap().join("../Frameworks");
        assert_eq!(set_directory(Path::new("/unrelated/resources"), false, None), expected);
        assert_eq!(set_directory(Path::new("/unrelated/resources"), true, Some("/dev/set".as_ref())),
            Path::new("/dev/set"));
    }
}

#[cfg(test)]
mod macos_colour_tests {
    use super::*;

    #[test]
    fn video_colour_is_copied_from_one_map_not_printed_or_borrowed() {
        // Wire values from mpv 0.42 client.h, deliberately independent of
        // the decoder's constants: NODE=6, NODE_MAP=8, STRING=1.
        let keys = [
            c"primaries".as_ptr(),
            c"gamma".as_ptr(),
            c"irrelevant".as_ptr(),
        ];
        let values = [
            Node {
                data: NodeData {
                    string: c"display-p3".as_ptr(),
                },
                format: FORMAT_STRING,
            },
            Node {
                data: NodeData {
                    string: c"hlg".as_ptr(),
                },
                format: FORMAT_STRING,
            },
            Node {
                data: NodeData {
                    string: c"not-a-colour".as_ptr(),
                },
                format: FORMAT_STRING,
            },
        ];
        let list = NodeList {
            count: 3,
            values: values.as_ptr(),
            keys: keys.as_ptr(),
        };
        let mut node = Node {
            data: NodeData { list: &list },
            format: 8,
        };
        let property = RawProperty {
            name: c"video-params".as_ptr(),
            format: 6,
            data: (&mut node as *mut Node).cast(),
        };
        let result = unsafe { video_params(&property) };
        assert_eq!(result, (Some("hlg".into()), Some("display-p3".into())));
    }

    #[test]
    fn missing_or_non_string_video_colour_is_unknown() {
        let mut property = RawProperty {
            name: c"video-params".as_ptr(),
            format: 0,
            data: std::ptr::null_mut(),
        };
        assert_eq!(unsafe { video_params(&property) }, (None, None));
        let keys = [c"gamma".as_ptr()];
        let values = [Node {
            data: NodeData {
                string: c"pq".as_ptr(),
            },
            format: 0,
        }];
        let list = NodeList {
            count: 1,
            values: values.as_ptr(),
            keys: keys.as_ptr(),
        };
        let mut node = Node {
            data: NodeData { list: &list },
            format: 8,
        };
        property.format = 6;
        property.data = (&mut node as *mut Node).cast();
        assert_eq!(unsafe { video_params(&property) }, (None, None));
    }
}
