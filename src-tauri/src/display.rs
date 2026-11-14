use serde::Serialize;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum HdrState {
    Enabled,
    Disabled,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DisplayEvidence {
    Native,
    MpvObserved,
    ManualOverride,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DisplayProfile {
    pub name: Option<String>,
    pub width_px: u32,
    pub height_px: u32,
    pub hdr: HdrState,
    pub evidence: DisplayEvidence,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ObservedDisplay {
    pub names: Vec<String>,
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
}

impl ObservedDisplay {
    /// The screen this observation names: the first of mpv's display names
    /// (a window across several screens reports them all). The one rule for
    /// every reader, so that a copy ranked for an observed screen and the
    /// window opened for that copy name the same screen (review finding pw-1).
    pub(crate) fn screen_name(&self) -> Option<&String> {
        self.names.first()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolutionOverride {
    P720,
    P1080,
    P1440,
    P2160,
    P4320,
}

impl ResolutionOverride {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "720p" => Ok(Self::P720),
            "1080p" => Ok(Self::P1080),
            "1440p" => Ok(Self::P1440),
            "2160p" => Ok(Self::P2160),
            "4320p" => Ok(Self::P4320),
            _ => Err("unknown display resolution".to_string()),
        }
    }

    pub(crate) fn from_config(value: Option<&str>) -> Result<Option<Self>, String> {
        match value {
            None => Ok(None),
            Some(value) => Self::parse(value).map(Some),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::P720 => "720p",
            Self::P1080 => "1080p",
            Self::P1440 => "1440p",
            Self::P2160 => "2160p",
            Self::P4320 => "4320p",
        }
    }

    fn dimensions(self) -> (u32, u32) {
        match self {
            Self::P720 => (1280, 720),
            Self::P1080 => (1920, 1080),
            Self::P1440 => (2560, 1440),
            Self::P2160 => (3840, 2160),
            Self::P4320 => (7680, 4320),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HdrOverride {
    Enabled,
    Disabled,
}

impl HdrOverride {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "enabled" => Ok(Self::Enabled),
            "disabled" => Ok(Self::Disabled),
            _ => Err("unknown HDR override".to_string()),
        }
    }

    pub(crate) fn from_config(value: Option<&str>) -> Result<Option<Self>, String> {
        match value {
            None => Ok(None),
            Some(value) => Self::parse(value).map(Some),
        }
    }

    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
        }
    }

    fn state(self) -> HdrState {
        match self {
            Self::Enabled => HdrState::Enabled,
            Self::Disabled => HdrState::Disabled,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct DisplayOverrides {
    pub resolution: Option<ResolutionOverride>,
    pub hdr: Option<HdrOverride>,
}

pub(crate) fn apply_overrides(
    detected: &DisplayProfile,
    overrides: DisplayOverrides,
) -> DisplayProfile {
    let mut effective = detected.clone();
    if let Some(resolution) = overrides.resolution {
        (effective.width_px, effective.height_px) = resolution.dimensions();
    }
    if let Some(hdr) = overrides.hdr {
        effective.hdr = hdr.state();
    }
    if overrides.resolution.is_some() || overrides.hdr.is_some() {
        effective.evidence = DisplayEvidence::ManualOverride;
    }
    effective
}

#[derive(Debug, Clone)]
struct MonitorDescriptor {
    name: Option<String>,
    width_px: u32,
    height_px: u32,
    position_x: i32,
    position_y: i32,
}

fn current_monitor(app: &tauri::AppHandle) -> Option<MonitorDescriptor> {
    use tauri::Manager;
    let monitor = app.get_webview_window("main")?.current_monitor().ok()??;
    Some(MonitorDescriptor {
        name: monitor.name().cloned(),
        width_px: monitor.size().width,
        height_px: monitor.size().height,
        position_x: monitor.position().x,
        position_y: monitor.position().y,
    })
}

/// One output (screen) the app can see, by every name Vela or mpv may give
/// it, with its own size, and where it is on the desktop when that is known.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Output {
    pub names: Vec<String>,
    pub width_px: u32,
    pub height_px: u32,
    pub position: Option<(i32, i32)>,
}

impl Output {
    fn is_called(&self, name: &str) -> bool {
        self.names.iter().any(|own| own == name)
    }
}

/// What a play's screen is decided from: the screen holding Vela's window and
/// the outputs connected now (review findings pw-1 and pw-2). Looked at once
/// per play, before its copy is chosen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Screens {
    /// The name of the screen holding Vela's window, as mpv takes it.
    pub velas: Option<String>,
    /// That screen itself.
    pub velas_output: Option<Output>,
    /// Every output connected now; `None` when they could not be listed.
    pub connected: Option<Vec<Output>>,
}

impl Screens {
    /// Whether an output of this name is connected now. When the outputs
    /// could not be listed, or the list names none of them, nothing can be
    /// told, and the name is taken as it is, as before this check existed.
    pub(crate) fn is_connected(&self, name: &str) -> bool {
        match &self.connected {
            Some(outputs) if outputs.iter().any(|output| !output.names.is_empty()) => {
                outputs.iter().any(|output| output.is_called(name))
            }
            _ => true,
        }
    }

    fn output_called(&self, name: &str) -> Option<&Output> {
        self.connected
            .iter()
            .flatten()
            .find(|output| output.is_called(name))
    }
}

/// What the profile of the output a play is shown on is worked out from;
/// only its HDR state is still to be asked of the system (`profile_of`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProfileBasis {
    pub name: Option<String>,
    pub width_px: u32,
    pub height_px: u32,
    pub position: Option<(i32, i32)>,
    pub evidence: DisplayEvidence,
}

/// The profile basis of the output named `name`, which has been decided
/// already: this never picks an output by itself (review finding pw-1).
///
/// `observed` is the play's observation the name was taken from, if it was.
/// It counts only when it names that very output first (its
/// `screen_name`), as `detect_profile` counts an observation: its size then
/// counts as the output's size now, and, as there, no desktop position is
/// passed on with it. Otherwise the output's own size, from the outputs
/// connected now, counts. An output the list does not have (it could not be
/// listed) is given the values of the screen holding Vela's window, as
/// `detect_profile` gives a play that has observed nothing.
pub(crate) fn profile_basis(
    name: Option<&str>,
    observed: Option<&ObservedDisplay>,
    screens: &Screens,
) -> ProfileBasis {
    let observed = observed.filter(|value| {
        name.is_some()
            && value.screen_name().map(String::as_str) == name
            && (value.width_px.is_some() || value.height_px.is_some())
    });
    let output = name
        .and_then(|name| screens.output_called(name))
        .or(screens.velas_output.as_ref());
    let width_px = observed
        .and_then(|value| value.width_px)
        .or_else(|| output.map(|value| value.width_px))
        .unwrap_or(0);
    let height_px = observed
        .and_then(|value| value.height_px)
        .or_else(|| output.map(|value| value.height_px))
        .unwrap_or(0);
    ProfileBasis {
        name: name.map(str::to_string),
        width_px,
        height_px,
        position: if observed.is_some() {
            None
        } else {
            output.and_then(|value| value.position)
        },
        evidence: if observed.is_some() {
            DisplayEvidence::MpvObserved
        } else {
            DisplayEvidence::Native
        },
    }
}

/// The profile of a decided output: its basis, and its HDR state asked of
/// the system for that output, by the same rules as `detect_profile`.
pub(crate) async fn profile_of(app: &tauri::AppHandle, basis: ProfileBasis) -> DisplayProfile {
    let hdr = native_hdr_state(
        app,
        basis.name.clone(),
        basis.width_px,
        basis.height_px,
        basis.position,
    )
    .await;
    DisplayProfile {
        name: basis.name,
        width_px: basis.width_px,
        height_px: basis.height_px,
        hdr,
        evidence: basis.evidence,
    }
}

/// The screen holding Vela's window and the outputs connected now. The
/// system is asked once, on a blocking thread, through Vela's window (Tauri
/// hands the question to the window's own thread and waits for the answer,
/// so it is never asked on an async worker); the name and the values of
/// Vela's screen both come from that one answer. A play that has no other
/// screen to go to is shown on Vela's screen. Wayland compositors may still
/// choose a different output; the observed mpv output then counts for every
/// later play.
pub(crate) async fn look_at_screens(app: &tauri::AppHandle) -> Screens {
    let asked = {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            use tauri::Manager;
            let window = app.get_webview_window("main")?;
            let velas = window.current_monitor().ok().flatten().map(|monitor| descriptor(&monitor));
            let all = window
                .available_monitors()
                .ok()
                .map(|monitors| monitors.iter().map(descriptor).collect::<Vec<_>>());
            Some((velas, all))
        })
        .await
        .ok()
        .flatten()
    };
    let Some((velas_monitor, monitors)) = asked else {
        return Screens::default();
    };
    let velas = match velas_monitor {
        Some(monitor) => Some(named_monitor(app, monitor).await),
        None => None,
    };
    let monitors = match monitors {
        Some(monitors) => {
            let mut named = Vec::with_capacity(monitors.len());
            for monitor in monitors {
                named.push(named_monitor(app, monitor).await);
            }
            Some(named)
        }
        None => None,
    };
    screens_from(velas, monitors, platform_outputs().await)
}

/// A monitor Tauri gives, with the name mpv takes for it
/// (`platform_screen_name`; the two differ on macOS).
#[derive(Debug, Clone)]
struct NamedMonitor {
    monitor: MonitorDescriptor,
    mpv_name: Option<String>,
}

async fn named_monitor(app: &tauri::AppHandle, monitor: MonitorDescriptor) -> NamedMonitor {
    let mpv_name = platform_screen_name(app, monitor.clone()).await;
    NamedMonitor { monitor, mpv_name }
}

/// What `look_at_screens` saw, put together: the monitor holding Vela's
/// window, every monitor Tauri lists (`None` when they could not be listed),
/// and the outputs the system lists by names Tauri does not give
/// (`platform_outputs`). Kept apart from the asking so that a test can run it.
fn screens_from(
    velas: Option<NamedMonitor>,
    monitors: Option<Vec<NamedMonitor>>,
    platform: Vec<Output>,
) -> Screens {
    let velas_output = velas.as_ref().map(output_of);
    let connected = monitors.map(|monitors| {
        platform
            .into_iter()
            .chain(monitors.iter().map(output_of))
            .collect::<Vec<_>>()
    });
    Screens {
        velas: velas.and_then(|velas| velas.mpv_name),
        velas_output,
        connected,
    }
}

fn descriptor(monitor: &tauri::Monitor) -> MonitorDescriptor {
    MonitorDescriptor {
        name: monitor.name().cloned(),
        width_px: monitor.size().width,
        height_px: monitor.size().height,
        position_x: monitor.position().x,
        position_y: monitor.position().y,
    }
}

/// A monitor as an output: Tauri's name for it and mpv's (they differ on
/// macOS, `platform_screen_name`).
fn output_of(named: &NamedMonitor) -> Output {
    let monitor = &named.monitor;
    let mut names: Vec<String> = monitor.name.iter().cloned().collect();
    if let Some(name) = &named.mpv_name {
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    Output {
        names,
        width_px: monitor.width_px,
        height_px: monitor.height_px,
        position: Some((monitor.position_x, monitor.position_y)),
    }
}

/// Outputs the system lists by names Tauri does not give. On Wayland, Tauri
/// names an output by its model ("XG27UQDMS") and mpv by its connector
/// ("DP-3"); the compositor gives both.
#[cfg(target_os = "linux")]
async fn platform_outputs() -> Vec<Output> {
    tauri::async_runtime::spawn_blocking(|| {
        if !wayland_session() {
            return Vec::new();
        }
        wayland_hdr::outputs().unwrap_or_default()
    })
    .await
    .unwrap_or_default()
}

#[cfg(not(target_os = "linux"))]
async fn platform_outputs() -> Vec<Output> {
    Vec::new()
}

#[cfg(target_os = "linux")]
fn wayland_session() -> bool {
    !(std::env::var("XDG_SESSION_TYPE")
        .ok()
        .is_some_and(|value| value.eq_ignore_ascii_case("x11"))
        || std::env::var_os("WAYLAND_DISPLAY").is_none())
}

#[cfg(target_os = "macos")]
fn mac_display_id(screen: &objc2_app_kit::NSScreen) -> Option<u32> {
    use objc2::msg_send;
    use objc2_foundation::ns_string;
    let description = screen.deviceDescription();
    let number = description.objectForKey(ns_string!("NSScreenNumber"))?;
    Some(unsafe { msg_send![&*number, unsignedIntValue] })
}

#[cfg(target_os = "macos")]
fn mac_mpv_name(product_name: &str, serial: u32) -> String {
    format!("{product_name} ({serial})")
}

#[cfg(target_os = "macos")]
fn mac_mpv_screen_name(screen: &objc2_app_kit::NSScreen) -> Option<String> {
    use objc2_core_graphics::CGDisplaySerialNumber;
    let display_id = mac_display_id(screen)?;
    Some(mac_mpv_name(
        &screen.localizedName().to_string(),
        CGDisplaySerialNumber(display_id),
    ))
}

#[cfg(target_os = "macos")]
fn mac_screen_matches(
    screen: &objc2_app_kit::NSScreen,
    target_name: Option<&str>,
    width_px: u32,
    height_px: u32,
    target_position: Option<(i32, i32)>,
) -> bool {
    use objc2_core_graphics::{CGDisplayBounds, CGDisplayPixelsHigh, CGDisplayPixelsWide};
    let Some(display_id) = mac_display_id(screen) else {
        return false;
    };
    if target_name.is_some_and(|name| {
        screen.localizedName().to_string() == name
            || mac_mpv_screen_name(screen).as_deref() == Some(name)
    }) {
        return true;
    }
    let bounds = CGDisplayBounds(display_id);
    target_position.is_some_and(|(x, y)| {
        bounds.origin.x.round() as i32 == x
            && bounds.origin.y.round() as i32 == y
            && CGDisplayPixelsWide(display_id) as u32 == width_px
            && CGDisplayPixelsHigh(display_id) as u32 == height_px
    })
}

#[cfg(target_os = "macos")]
async fn platform_screen_name(
    app: &tauri::AppHandle,
    monitor: MonitorDescriptor,
) -> Option<String> {
    use objc2::{MainThreadMarker, Message};
    use objc2_app_kit::NSScreen;

    let (send, receive) = tokio::sync::oneshot::channel();
    app.run_on_main_thread(move || {
        let name = MainThreadMarker::new().and_then(|mtm| {
            NSScreen::screens(mtm)
                .iter()
                .find(|screen| {
                    mac_screen_matches(
                        screen,
                        monitor.name.as_deref(),
                        monitor.width_px,
                        monitor.height_px,
                        Some((monitor.position_x, monitor.position_y)),
                    )
                })
                .map(|screen| screen.retain())
                .and_then(|screen| mac_mpv_screen_name(&screen))
        });
        let _ = send.send(name);
    })
    .ok()?;
    receive.await.ok().flatten()
}

#[cfg(not(target_os = "macos"))]
async fn platform_screen_name(
    _app: &tauri::AppHandle,
    monitor: MonitorDescriptor,
) -> Option<String> {
    monitor.name
}

pub(crate) async fn detect_profile(
    app: &tauri::AppHandle,
    observed: Option<ObservedDisplay>,
) -> DisplayProfile {
    let monitor = current_monitor(app);
    let observed = observed.filter(|value| {
        value.width_px.is_some() || value.height_px.is_some() || !value.names.is_empty()
    });

    let name = observed
        .as_ref()
        .and_then(|value| value.screen_name().cloned())
        .or_else(|| monitor.as_ref().and_then(|value| value.name.clone()));
    let width_px = observed
        .as_ref()
        .and_then(|value| value.width_px)
        .or_else(|| monitor.as_ref().map(|value| value.width_px))
        .unwrap_or(0);
    let height_px = observed
        .as_ref()
        .and_then(|value| value.height_px)
        .or_else(|| monitor.as_ref().map(|value| value.height_px))
        .unwrap_or(0);
    let position = if observed.is_none() {
        monitor
            .as_ref()
            .map(|value| (value.position_x, value.position_y))
    } else {
        None
    };
    let hdr = native_hdr_state(app, name.clone(), width_px, height_px, position).await;

    DisplayProfile {
        name,
        width_px,
        height_px,
        hdr,
        evidence: if observed.is_some() {
            DisplayEvidence::MpvObserved
        } else {
            DisplayEvidence::Native
        },
    }
}

#[cfg(target_os = "macos")]
async fn native_hdr_state(
    app: &tauri::AppHandle,
    target_name: Option<String>,
    _width_px: u32,
    _height_px: u32,
    target_position: Option<(i32, i32)>,
) -> HdrState {
    use objc2::{msg_send, sel, MainThreadMarker, Message};
    use objc2_app_kit::NSScreen;

    let (send, receive) = tokio::sync::oneshot::channel();
    if app
        .run_on_main_thread(move || {
            let result = MainThreadMarker::new()
                .and_then(|mtm| {
                    let screens = NSScreen::screens(mtm);
                    let exact = screens.iter().find(|screen| {
                        mac_screen_matches(
                            screen,
                            target_name.as_deref(),
                            _width_px,
                            _height_px,
                            target_position,
                        )
                    });
                    let sized = screens.iter().find(|screen| {
                        use objc2_core_graphics::{CGDisplayPixelsHigh, CGDisplayPixelsWide};
                        mac_display_id(screen).is_some_and(|display_id| {
                            CGDisplayPixelsWide(display_id) as u32 == _width_px
                                && CGDisplayPixelsHigh(display_id) as u32 == _height_px
                        })
                    });
                    let screen = exact
                        .or(sized)
                        .map(|screen| screen.retain())
                        .or_else(|| NSScreen::mainScreen(mtm));
                    screen.map(|screen| {
                        let has_edr_api: bool = unsafe {
                            msg_send![&*screen, respondsToSelector: sel!(maximumExtendedDynamicRangeColorComponentValue)]
                        };
                        if !has_edr_api {
                            HdrState::Unknown
                        } else if screen.maximumExtendedDynamicRangeColorComponentValue() > 1.0 {
                            HdrState::Enabled
                        } else {
                            HdrState::Disabled
                        }
                    })
                })
                .unwrap_or(HdrState::Unknown);
            let _ = send.send(result);
        })
        .is_err()
    {
        return HdrState::Unknown;
    }
    receive.await.unwrap_or(HdrState::Unknown)
}

#[cfg(target_os = "windows")]
async fn native_hdr_state(
    _app: &tauri::AppHandle,
    target_name: Option<String>,
    _width_px: u32,
    _height_px: u32,
    _target_position: Option<(i32, i32)>,
) -> HdrState {
    tauri::async_runtime::spawn_blocking(move || windows_hdr_state(target_name.as_deref()))
        .await
        .unwrap_or(HdrState::Unknown)
}

#[cfg(target_os = "windows")]
fn windows_hdr_state(target_name: Option<&str>) -> HdrState {
    use std::mem::size_of;
    use windows::Win32::Devices::Display::*;
    use windows::Win32::Foundation::ERROR_SUCCESS;

    let Some(target_name) = target_name else {
        return HdrState::Unknown;
    };
    unsafe {
        let mut path_count = 0;
        let mut mode_count = 0;
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
            != ERROR_SUCCESS
        {
            return HdrState::Unknown;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        if QueryDisplayConfig(
            QDC_ONLY_ACTIVE_PATHS,
            &mut path_count,
            paths.as_mut_ptr(),
            &mut mode_count,
            modes.as_mut_ptr(),
            None,
        ) != ERROR_SUCCESS
        {
            return HdrState::Unknown;
        }

        for path in paths.into_iter().take(path_count as usize) {
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                    size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                    adapterId: path.sourceInfo.adapterId,
                    id: path.sourceInfo.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut source.header) != 0 {
                continue;
            }
            let end = source
                .viewGdiDeviceName
                .iter()
                .position(|unit| *unit == 0)
                .unwrap_or(source.viewGdiDeviceName.len());
            let source_name = String::from_utf16_lossy(&source.viewGdiDeviceName[..end]);
            if !source_name.eq_ignore_ascii_case(target_name) {
                continue;
            }

            let mut color = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO {
                header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
                    r#type: DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
                    size: size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>() as u32,
                    adapterId: path.targetInfo.adapterId,
                    id: path.targetInfo.id,
                },
                ..Default::default()
            };
            if DisplayConfigGetDeviceInfo(&mut color.header) != 0 {
                return HdrState::Unknown;
            }
            return if color.Anonymous.value & 0x2 != 0 {
                HdrState::Enabled
            } else {
                HdrState::Disabled
            };
        }
    }
    HdrState::Unknown
}

#[cfg(target_os = "linux")]
async fn native_hdr_state(
    _app: &tauri::AppHandle,
    target_name: Option<String>,
    width_px: u32,
    height_px: u32,
    _target_position: Option<(i32, i32)>,
) -> HdrState {
    tauri::async_runtime::spawn_blocking(move || {
        if !wayland_session() {
            return HdrState::Disabled;
        }
        wayland_hdr::query(target_name.as_deref(), width_px, height_px).unwrap_or(HdrState::Unknown)
    })
    .await
    .unwrap_or(HdrState::Unknown)
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
async fn native_hdr_state(
    _app: &tauri::AppHandle,
    _target_name: Option<String>,
    _width_px: u32,
    _height_px: u32,
    _target_position: Option<(i32, i32)>,
) -> HdrState {
    HdrState::Unknown
}

#[cfg(target_os = "linux")]
mod wayland_hdr {
    use super::HdrState;
    use wayland_client::{
        delegate_noop,
        protocol::{wl_output, wl_registry},
        Connection, Dispatch, QueueHandle, WEnum,
    };
    use wayland_protocols::wp::color_management::v1::client::{
        wp_color_management_output_v1, wp_color_manager_v1, wp_image_description_info_v1,
        wp_image_description_v1,
    };

    #[derive(Debug)]
    struct Output {
        global_name: u32,
        proxy: wl_output::WlOutput,
        name: Option<String>,
        model: Option<String>,
        width: u32,
        height: u32,
    }

    #[derive(Default)]
    struct State {
        manager: Option<wp_color_manager_v1::WpColorManagerV1>,
        outputs: Vec<Output>,
        color_output: Option<wp_color_management_output_v1::WpColorManagementOutputV1>,
        image: Option<wp_image_description_v1::WpImageDescriptionV1>,
        info: Option<wp_image_description_info_v1::WpImageDescriptionInfoV1>,
        saw_transfer: bool,
        hdr_transfer: bool,
        result: Option<HdrState>,
    }

    impl Dispatch<wl_registry::WlRegistry, ()> for State {
        fn event(
            state: &mut Self,
            registry: &wl_registry::WlRegistry,
            event: wl_registry::Event,
            _: &(),
            _: &Connection,
            qh: &QueueHandle<Self>,
        ) {
            if let wl_registry::Event::Global {
                name,
                interface,
                version,
            } = event
            {
                match interface.as_str() {
                    "wl_output" => state.outputs.push(Output {
                        global_name: name,
                        proxy: registry.bind(name, version.min(4), qh, name),
                        name: None,
                        model: None,
                        width: 0,
                        height: 0,
                    }),
                    "wp_color_manager_v1" => {
                        state.manager = Some(registry.bind(name, version.min(3), qh, ()))
                    }
                    _ => {}
                }
            }
        }
    }

    impl Dispatch<wl_output::WlOutput, u32> for State {
        fn event(
            state: &mut Self,
            _: &wl_output::WlOutput,
            event: wl_output::Event,
            global_name: &u32,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            let Some(output) = state
                .outputs
                .iter_mut()
                .find(|output| output.global_name == *global_name)
            else {
                return;
            };
            match event {
                wl_output::Event::Name { name } => output.name = Some(name),
                wl_output::Event::Geometry { model, .. } => output.model = Some(model),
                wl_output::Event::Mode {
                    flags: WEnum::Value(value),
                    width,
                    height,
                    ..
                } if value.contains(wl_output::Mode::Current) => {
                    output.width = width.max(0) as u32;
                    output.height = height.max(0) as u32;
                }
                _ => {}
            }
        }
    }

    delegate_noop!(State: ignore wp_color_manager_v1::WpColorManagerV1);
    delegate_noop!(State: ignore wp_color_management_output_v1::WpColorManagementOutputV1);

    impl Dispatch<wp_image_description_v1::WpImageDescriptionV1, ()> for State {
        fn event(
            state: &mut Self,
            image: &wp_image_description_v1::WpImageDescriptionV1,
            event: wp_image_description_v1::Event,
            _: &(),
            _: &Connection,
            qh: &QueueHandle<Self>,
        ) {
            match event {
                wp_image_description_v1::Event::Ready { .. }
                | wp_image_description_v1::Event::Ready2 { .. } => {
                    state.info = Some(image.get_information(qh, ()));
                }
                wp_image_description_v1::Event::Failed { .. } => {
                    state.result = Some(HdrState::Unknown)
                }
                _ => {}
            }
        }
    }

    impl Dispatch<wp_image_description_info_v1::WpImageDescriptionInfoV1, ()> for State {
        fn event(
            state: &mut Self,
            _: &wp_image_description_info_v1::WpImageDescriptionInfoV1,
            event: wp_image_description_info_v1::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            match event {
                wp_image_description_info_v1::Event::TfNamed { tf } => {
                    state.saw_transfer = true;
                    let numeric: u32 = tf.into();
                    state.hdr_transfer |= numeric == 11 || numeric == 13;
                }
                wp_image_description_info_v1::Event::TfPower { .. } => {
                    state.saw_transfer = true;
                }
                wp_image_description_info_v1::Event::Done => {
                    state.result = Some(if state.hdr_transfer {
                        HdrState::Enabled
                    } else if state.saw_transfer {
                        HdrState::Disabled
                    } else {
                        HdrState::Unknown
                    });
                }
                _ => {}
            }
        }
    }

    /// The compositor's outputs, each by its connector name and its model,
    /// at its current mode.
    pub(super) fn outputs() -> Result<Vec<super::Output>, String> {
        let connection = Connection::connect_to_env().map_err(|error| error.to_string())?;
        let mut queue = connection.new_event_queue();
        let qh = queue.handle();
        connection.display().get_registry(&qh, ());
        let mut state = State::default();
        queue
            .roundtrip(&mut state)
            .map_err(|error| error.to_string())?;
        queue
            .roundtrip(&mut state)
            .map_err(|error| error.to_string())?;
        Ok(state
            .outputs
            .iter()
            .map(|output| super::Output {
                names: output
                    .name
                    .iter()
                    .chain(output.model.iter())
                    .filter(|name| !name.trim().is_empty())
                    .cloned()
                    .collect(),
                width_px: output.width,
                height_px: output.height,
                position: None,
            })
            .filter(|output| !output.names.is_empty())
            .collect())
    }

    pub(super) fn query(
        target_name: Option<&str>,
        width: u32,
        height: u32,
    ) -> Result<HdrState, String> {
        let connection = Connection::connect_to_env().map_err(|error| error.to_string())?;
        let mut queue = connection.new_event_queue();
        let qh = queue.handle();
        connection.display().get_registry(&qh, ());
        let mut state = State::default();
        queue
            .roundtrip(&mut state)
            .map_err(|error| error.to_string())?;
        queue
            .roundtrip(&mut state)
            .map_err(|error| error.to_string())?;

        let Some(manager) = state.manager.clone() else {
            return Ok(HdrState::Unknown);
        };
        let selected = state
            .outputs
            .iter()
            .find(|output| target_name.is_some_and(|name| output.name.as_deref() == Some(name)))
            .or_else(|| {
                state
                    .outputs
                    .iter()
                    .find(|output| output.width == width && output.height == height)
            })
            .map(|output| output.proxy.clone());
        let Some(output) = selected else {
            return Ok(HdrState::Unknown);
        };
        let color_output = manager.get_output(&output, &qh, ());
        let image = color_output.get_image_description(&qh, ());
        state.color_output = Some(color_output);
        state.image = Some(image);

        for _ in 0..4 {
            queue
                .roundtrip(&mut state)
                .map_err(|error| error.to_string())?;
            if let Some(result) = state.result {
                return Ok(result);
            }
        }
        Ok(HdrState::Unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detected() -> DisplayProfile {
        DisplayProfile {
            name: Some("Studio Display".to_string()),
            width_px: 5120,
            height_px: 2880,
            hdr: HdrState::Enabled,
            evidence: DisplayEvidence::Native,
        }
    }

    #[test]
    fn display_overrides_default_only_missing_and_reject_unknown_values() {
        assert_eq!(ResolutionOverride::from_config(None), Ok(None));
        assert!(ResolutionOverride::from_config(Some("5k")).is_err());
        assert!(HdrOverride::from_config(Some("maybe")).is_err());
        assert_eq!(
            ResolutionOverride::from_config(Some("2160p")),
            Ok(Some(ResolutionOverride::P2160))
        );
        assert_eq!(
            HdrOverride::from_config(Some("disabled")),
            Ok(Some(HdrOverride::Disabled))
        );
    }

    #[test]
    fn independent_manual_overrides_win_without_mutating_detected_profile() {
        let native = detected();
        let effective = apply_overrides(
            &native,
            DisplayOverrides {
                resolution: Some(ResolutionOverride::P1080),
                hdr: Some(HdrOverride::Disabled),
            },
        );
        assert_eq!((effective.width_px, effective.height_px), (1920, 1080));
        assert_eq!(effective.hdr, HdrState::Disabled);
        assert_eq!(effective.evidence, DisplayEvidence::ManualOverride);
        assert_eq!(native, detected());
    }

    #[test]
    fn auto_overrides_preserve_native_evidence() {
        let native = detected();
        assert_eq!(
            apply_overrides(&native, DisplayOverrides::default()),
            native
        );
    }

    fn output(name: &str, width_px: u32, height_px: u32, x: i32) -> Output {
        Output {
            names: vec![name.to_string()],
            width_px,
            height_px,
            position: Some((x, 0)),
        }
    }

    fn observed(names: &[&str], width_px: u32, height_px: u32) -> ObservedDisplay {
        ObservedDisplay {
            names: names.iter().map(|name| name.to_string()).collect(),
            width_px: Some(width_px),
            height_px: Some(height_px),
        }
    }

    fn screens(connected: Vec<Output>) -> Screens {
        Screens {
            velas: Some("HDMI-1".to_string()),
            velas_output: connected
                .iter()
                .find(|output| output.is_called("HDMI-1"))
                .cloned(),
            connected: Some(connected),
        }
    }

    fn size(basis: &ProfileBasis) -> (Option<&str>, u32, u32) {
        (basis.name.as_deref(), basis.width_px, basis.height_px)
    }

    // Review finding pw-1, comment 3: the profile is the named output's,
    // among two outputs and among three, and never one picked otherwise.
    #[test]
    fn the_profile_is_the_named_outputs_among_several() {
        let two = vec![output("DP-1", 3840, 2160, 0), output("HDMI-1", 1920, 1080, 3840)];
        let three = {
            let mut three = two.clone();
            three.insert(1, output("eDP-1", 2560, 1440, -2560));
            three
        };
        for connected in [two, three.clone()] {
            let screens = screens(connected.clone());
            for expected in &connected {
                let name = expected.names[0].as_str();
                let basis = profile_basis(Some(name), None, &screens);
                assert_eq!(size(&basis), (Some(name), expected.width_px, expected.height_px));
                assert_eq!(basis.position, expected.position, "{name}: where it is");
                assert_eq!(basis.evidence, DisplayEvidence::Native);
            }
        }
        // The size observed on the screen named counts as now, in either
        // order of an observation across several screens.
        let screens = screens(three);
        for (names, first) in [
            (&["DP-1", "HDMI-1"][..], "DP-1"),
            (&["HDMI-1", "DP-1"][..], "HDMI-1"),
            (&["eDP-1", "HDMI-1", "DP-1"][..], "eDP-1"),
            (&["DP-1", "eDP-1", "HDMI-1"][..], "DP-1"),
        ] {
            let seen = observed(names, 3000, 2000);
            let basis = profile_basis(Some(first), Some(&seen), &screens);
            assert_eq!(size(&basis), (Some(first), 3000, 2000), "{names:?}");
            assert_eq!(basis.position, None);
            assert_eq!(basis.evidence, DisplayEvidence::MpvObserved);
        }
        // An observation that names another screen first is not that
        // screen's: the named output's own size counts.
        let seen = observed(&["HDMI-1", "DP-1"], 3000, 2000);
        let basis = profile_basis(Some("DP-1"), Some(&seen), &screens);
        assert_eq!(size(&basis), (Some("DP-1"), 3840, 2160));
        assert_eq!(basis.evidence, DisplayEvidence::Native);
        // An observation of the screen with no size: the output's own.
        let unsized_seen = ObservedDisplay {
            names: vec!["eDP-1".to_string()],
            ..Default::default()
        };
        assert_eq!(
            size(&profile_basis(Some("eDP-1"), Some(&unsized_seen), &screens)),
            (Some("eDP-1"), 2560, 1440)
        );
    }

    #[test]
    fn an_output_the_list_lacks_gets_the_values_of_velas_screen() {
        let screens = screens(vec![output("DP-1", 3840, 2160, 0), output("HDMI-1", 1920, 1080, 3840)]);
        let basis = profile_basis(Some("DVI-0"), None, &screens);
        assert_eq!(size(&basis), (Some("DVI-0"), 1920, 1080));
        assert_eq!(size(&profile_basis(None, None, &screens)), (None, 1920, 1080));
        assert_eq!(size(&profile_basis(None, None, &Screens::default())), (None, 0, 0));
    }

    #[test]
    fn a_name_is_connected_when_an_output_has_it() {
        let screens = Screens {
            connected: Some(vec![Output {
                names: vec!["DP-3".to_string(), "XG27UQDMS".to_string()],
                ..Default::default()
            }]),
            ..Default::default()
        };
        assert!(screens.is_connected("DP-3"));
        assert!(screens.is_connected("XG27UQDMS"));
        assert!(!screens.is_connected("HDMI-1"));
        // Nothing to tell by: every name stands.
        assert!(Screens::default().is_connected("HDMI-1"));
        let nameless = Screens {
            connected: Some(vec![Output::default()]),
            ..Default::default()
        };
        assert!(nameless.is_connected("HDMI-1"));
    }

    fn monitor(name: &str, mpv_name: &str, width_px: u32, height_px: u32, x: i32) -> NamedMonitor {
        NamedMonitor {
            monitor: MonitorDescriptor {
                name: Some(name.to_string()),
                width_px,
                height_px,
                position_x: x,
                position_y: 0,
            },
            mpv_name: Some(mpv_name.to_string()),
        }
    }

    fn listed(names: &[&str], width_px: u32, height_px: u32, position: Option<(i32, i32)>) -> Output {
        Output {
            names: names.iter().map(|name| name.to_string()).collect(),
            width_px,
            height_px,
            position,
        }
    }

    // Second review of finding pw-1: what look_at_screens saw is put together
    // by a function a test can run. Vela's screen is named as mpv names it and
    // has its own values; the outputs connected are the system's own list
    // and every monitor Tauri lists, by both names.
    #[test]
    fn what_was_seen_of_the_screens_is_put_together() {
        let studio = monitor("Studio Display", "Studio Display (7)", 5120, 2880, 0);
        let hdmi = monitor("HDMI-1", "HDMI-1", 1920, 1080, 5120);
        let compositor = listed(&["DP-3", "XG27UQDMS"], 3840, 2160, None);
        let screens = screens_from(
            Some(hdmi.clone()),
            Some(vec![studio.clone(), hdmi.clone()]),
            vec![compositor.clone()],
        );
        assert_eq!(screens.velas.as_deref(), Some("HDMI-1"));
        assert_eq!(
            screens.velas_output,
            Some(listed(&["HDMI-1"], 1920, 1080, Some((5120, 0))))
        );
        assert_eq!(
            screens.connected,
            Some(vec![
                compositor.clone(),
                listed(&["Studio Display", "Studio Display (7)"], 5120, 2880, Some((0, 0))),
                listed(&["HDMI-1"], 1920, 1080, Some((5120, 0))),
            ])
        );
        for name in ["DP-3", "XG27UQDMS", "Studio Display", "Studio Display (7)", "HDMI-1"] {
            assert!(screens.is_connected(name), "{name}");
        }
        assert!(!screens.is_connected("DP-1"));
        // Vela's screen is named as mpv takes it, not as Tauri gives it.
        let on_studio = screens_from(Some(studio), Some(vec![hdmi.clone()]), Vec::new());
        assert_eq!(on_studio.velas.as_deref(), Some("Studio Display (7)"));
        assert_eq!(on_studio.velas_output.map(|output| output.width_px), Some(5120));
        // The monitors could not be listed: nothing is known to be connected,
        // and Vela's screen is still named.
        let unlisted = screens_from(Some(hdmi), None, vec![compositor]);
        assert_eq!(unlisted.connected, None);
        assert_eq!(unlisted.velas.as_deref(), Some("HDMI-1"));
        assert_eq!(screens_from(None, None, Vec::new()), Screens::default());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_screen_name_matches_mpv_product_and_serial_contract() {
        assert_eq!(mac_mpv_name("Studio Display", 1234), "Studio Display (1234)");
    }
}
