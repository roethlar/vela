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
    /// Every output connected now, by every name mpv may give it; `None`
    /// when that is not known: when the list is not known to be complete in
    /// the names mpv may use (`screens_from`).
    pub connected: Option<Vec<Output>>,
    /// Every output that was seen (the system's own list, then Tauri's
    /// monitors), complete or not. It gives a decided output its size
    /// (`profile_basis`) even when what is connected is not known: an
    /// incomplete list still gives the outputs it does list (fourth review of
    /// finding pw-1). It never tells whether a name is connected.
    pub seen: Vec<Output>,
}

impl Screens {
    /// Whether an output of this name is connected now. When that is not
    /// known (`connected` is `None`), or the list names none of them, nothing
    /// can be told, and the name is taken as it is, as before this check
    /// existed.
    pub(crate) fn is_connected(&self, name: &str) -> bool {
        match &self.connected {
            Some(outputs) if outputs.iter().any(|output| !output.names.is_empty()) => {
                outputs.iter().any(|output| output.is_called(name))
            }
            _ => true,
        }
    }

    fn output_called(&self, name: &str) -> Option<&Output> {
        self.seen.iter().find(|output| output.is_called(name))
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
/// passed on with it. Otherwise the output's own size, from the outputs seen
/// (`Screens::seen`, whether or not that list is complete), counts. An
/// output that was not seen is given the values of the screen holding Vela's
/// window, as
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
    // The screen holding Vela's window is that very monitor, even where
    // another output carries the same name (two screens of one model).
    let output = name
        .and_then(|name| {
            if screens.velas.as_deref() == Some(name) {
                screens.velas_output.as_ref()
            } else {
                screens.output_called(name)
            }
        })
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

/// The HDR state of a decided output, asked of the system for that output
/// (its name, size and place), by the same rules as `detect_profile`.
pub(crate) async fn hdr_of(app: &tauri::AppHandle, basis: &ProfileBasis) -> HdrState {
    native_hdr_state(
        app,
        basis.name.clone(),
        basis.width_px,
        basis.height_px,
        basis.position,
    )
    .await
}

/// The profile of a decided output: its basis, with its HDR state.
pub(crate) fn profile_with(basis: ProfileBasis, hdr: HdrState) -> DisplayProfile {
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
pub(crate) struct NamedMonitor {
    monitor: MonitorDescriptor,
    mpv_name: Option<String>,
}

#[cfg(test)]
impl NamedMonitor {
    /// A monitor as Tauri gives it, at `x` on the desktop, and the name mpv
    /// takes for it (`None`: none could be found).
    pub(crate) fn for_test(
        name: &str,
        mpv_name: Option<&str>,
        width_px: u32,
        height_px: u32,
        x: i32,
    ) -> Self {
        Self {
            monitor: MonitorDescriptor {
                name: Some(name.to_string()),
                width_px,
                height_px,
                position_x: x,
                position_y: 0,
            },
            mpv_name: mpv_name.map(str::to_string),
        }
    }
}

async fn named_monitor(app: &tauri::AppHandle, monitor: MonitorDescriptor) -> NamedMonitor {
    let mpv_name = platform_screen_name(app, monitor.clone()).await;
    NamedMonitor { monitor, mpv_name }
}

/// What the system lists of its outputs besides Tauri's monitors
/// (`platform_outputs`), and whether that list is complete.
///
/// Only a Wayland session lists any, so off Linux nothing builds the last two
/// variants (the code that reads them is shared).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) enum PlatformOutputs {
    /// Not a Wayland session: the system names no output otherwise than
    /// Tauri does, and there is nothing to add.
    NoneToAdd,
    /// A Wayland session whose compositor listed every one of its outputs,
    /// each with its connector name.
    Complete(Vec<Output>),
    /// A Wayland session whose compositor's list could not be read (empty),
    /// or names some output by its model only. What it does list still
    /// names the screen holding Vela's window (`connector_of`), but it does
    /// not tell which names are connected.
    Incomplete(Vec<Output>),
}

impl PlatformOutputs {
    fn outputs(&self) -> &[Output] {
        match self {
            Self::NoneToAdd => &[],
            Self::Complete(outputs) | Self::Incomplete(outputs) => outputs,
        }
    }
}

/// What `look_at_screens` saw, put together: the monitor holding Vela's
/// window, every monitor Tauri lists (`None` when they could not be listed),
/// and the outputs the system lists by names Tauri does not give
/// (`platform_outputs`). Kept apart from the asking so that a test can run it.
///
/// What is connected is known only when the list is complete in the names
/// mpv may use (third review of finding pw-1): Tauri's monitors could be
/// listed, each of them has the name mpv takes for it (on macOS a failed
/// translation leaves one without), and in a Wayland session the
/// compositor's own list could be read and names every output by its
/// connector. Otherwise `connected` is `None`, and every name is taken as it
/// is, as before the check existed.
pub(crate) fn screens_from(
    velas: Option<NamedMonitor>,
    monitors: Option<Vec<NamedMonitor>>,
    platform: PlatformOutputs,
) -> Screens {
    let velas_output = velas.as_ref().map(output_of);
    let velas_name = velas.and_then(|velas| velas.mpv_name).map(|name| {
        connector_of(&name, platform.outputs())
            .map(str::to_string)
            .unwrap_or(name)
    });
    let platform_complete = match &platform {
        PlatformOutputs::NoneToAdd | PlatformOutputs::Complete(_) => true,
        PlatformOutputs::Incomplete(_) => false,
    };
    let complete = platform_complete
        && monitors
            .as_ref()
            .is_some_and(|monitors| monitors.iter().all(|monitor| monitor.mpv_name.is_some()));
    let seen = platform
        .outputs()
        .iter()
        .cloned()
        .chain(monitors.iter().flatten().map(output_of))
        .collect::<Vec<_>>();
    Screens {
        velas: velas_name,
        velas_output,
        connected: complete.then(|| seen.clone()),
        seen,
    }
}

/// The concrete name of the screen holding Vela's window, from the system's
/// own list of outputs (second review of finding pw-1). On Wayland Tauri
/// names a screen by its model, which two screens of one model share; the
/// compositor lists each output by its connector name ("DP-3") first, which
/// is what mpv takes and names no other screen. It is taken only when
/// exactly one listed output carries Vela's screen's name. When several do,
/// none is taken and the name stays as Tauri gives it: nothing Tauri gives
/// tells them apart reliably (third review of finding pw-1: Tauri's sizes
/// are scaled where the compositor's modes are not, so a size can point at
/// the other screen).
fn connector_of<'a>(name: &str, platform: &'a [Output]) -> Option<&'a str> {
    let mut carrying = platform.iter().filter(|output| output.is_called(name));
    match (carrying.next(), carrying.next()) {
        (Some(one), None) => one.names.first().map(String::as_str),
        _ => None,
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
/// ("DP-3"); the compositor gives both. A list that could not be read is
/// incomplete, so nothing is then known to be connected. A session that
/// says it is Wayland while its WAYLAND_DISPLAY is empty names no Wayland
/// display: Vela's window and mpv are then on X11, which name a screen
/// alike, and there is nothing to add (the end-to-end venue runs so, under
/// Xvfb inside a Wayland desktop). `wayland_session` itself, which the HDR
/// query also uses, is left as it was.
#[cfg(target_os = "linux")]
async fn platform_outputs() -> PlatformOutputs {
    tauri::async_runtime::spawn_blocking(|| {
        if !wayland_session() || !names_a_display(std::env::var_os("WAYLAND_DISPLAY").as_deref()) {
            return PlatformOutputs::NoneToAdd;
        }
        wayland_hdr::outputs().unwrap_or(PlatformOutputs::Incomplete(Vec::new()))
    })
    .await
    .unwrap_or(PlatformOutputs::Incomplete(Vec::new()))
}

#[cfg(not(target_os = "linux"))]
async fn platform_outputs() -> PlatformOutputs {
    PlatformOutputs::NoneToAdd
}

/// Whether a WAYLAND_DISPLAY value names a display: set, and not empty.
#[cfg(target_os = "linux")]
fn names_a_display(value: Option<&std::ffi::OsStr>) -> bool {
    value.is_some_and(|value| !value.is_empty())
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
        /// The current mode, as the output's hardware takes it (before its
        /// transform). `query` matches by this, as it always has.
        width: u32,
        height: u32,
        /// How the output is turned on the desktop (the geometry event).
        transform: Option<WEnum<wl_output::Transform>>,
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
                        transform: None,
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
                wl_output::Event::Geometry {
                    model, transform, ..
                } => {
                    output.model = Some(model);
                    output.transform = Some(transform);
                }
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
    /// at its current mode (`listed`).
    pub(super) fn outputs() -> Result<super::PlatformOutputs, String> {
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
        Ok(listed(
            state
                .outputs
                .iter()
                .map(|output| Described {
                    name: output.name.clone(),
                    model: output.model.clone(),
                    width: output.width,
                    height: output.height,
                    transform: output.transform,
                })
                .collect(),
        ))
    }

    /// One output as the compositor described it.
    #[derive(Debug, Clone, Default)]
    pub(super) struct Described {
        /// The connector name (`wl_output` version 4 and later).
        pub name: Option<String>,
        pub model: Option<String>,
        /// The current mode, before the output's transform.
        pub width: u32,
        pub height: u32,
        pub transform: Option<WEnum<wl_output::Transform>>,
    }

    /// Whether a transform turns the output a quarter turn, so that it is
    /// shown with its mode's width and height swapped (third review of
    /// finding pw-1, comment 4: a portrait output).
    pub(super) fn quarter_turned(transform: Option<WEnum<wl_output::Transform>>) -> bool {
        use wl_output::Transform;
        matches!(
            transform,
            Some(WEnum::Value(
                Transform::_90 | Transform::_270 | Transform::Flipped90 | Transform::Flipped270
            ))
        )
    }

    /// The compositor's outputs as Vela lists them: each by its connector
    /// name first and its model, at its size as it is shown (its mode, turned
    /// by its transform), and the list complete only when every output has
    /// a connector name, the name mpv takes (third review of finding pw-1).
    /// A compositor that gives models only, or one output without its
    /// connector, leaves the list incomplete.
    pub(super) fn listed(described: Vec<Described>) -> super::PlatformOutputs {
        let named = |name: &Option<String>| name.as_deref().is_some_and(|name| !name.trim().is_empty());
        let complete = described.iter().all(|output| named(&output.name));
        let outputs = described
            .into_iter()
            .map(|output| {
                let (width_px, height_px) = if quarter_turned(output.transform) {
                    (output.height, output.width)
                } else {
                    (output.width, output.height)
                };
                super::Output {
                    names: output
                        .name
                        .into_iter()
                        .chain(output.model)
                        .filter(|name| !name.trim().is_empty())
                        .collect(),
                    width_px,
                    height_px,
                    position: None,
                }
            })
            .filter(|output| !output.names.is_empty())
            .collect();
        if complete {
            super::PlatformOutputs::Complete(outputs)
        } else {
            super::PlatformOutputs::Incomplete(outputs)
        }
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
            connected: Some(connected.clone()),
            seen: connected,
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
        NamedMonitor::for_test(name, Some(mpv_name), width_px, height_px, x)
    }

    use PlatformOutputs::{Complete, Incomplete, NoneToAdd};

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
            Complete(vec![compositor.clone()]),
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
        let on_studio = screens_from(Some(studio), Some(vec![hdmi.clone()]), NoneToAdd);
        assert_eq!(on_studio.velas.as_deref(), Some("Studio Display (7)"));
        assert_eq!(on_studio.velas_output.map(|output| output.width_px), Some(5120));
        // The monitors could not be listed: nothing is known to be connected,
        // and Vela's screen is still named.
        let unlisted = screens_from(Some(hdmi), None, Complete(vec![compositor]));
        assert_eq!(unlisted.connected, None);
        assert_eq!(unlisted.velas.as_deref(), Some("HDMI-1"));
        assert_eq!(screens_from(None, None, NoneToAdd), Screens::default());
    }

    // Third review of finding pw-1, comments 1 and 2: what is connected is
    // known only from a list complete in the names mpv may use. In each way
    // of being incomplete nothing is known, and every name is taken; a
    // complete list still rejects a name it does not have.
    #[test]
    fn connected_is_known_only_from_a_complete_list() {
        let model = monitor("XG27UQDMS", "XG27UQDMS", 3840, 2160, 0);
        let hdmi = monitor("HDMI-1", "HDMI-1", 1920, 1080, 3840);
        let compositor = listed(&["DP-3", "XG27UQDMS"], 3840, 2160, None);
        let complete = screens_from(
            Some(model.clone()),
            Some(vec![model.clone(), hdmi.clone()]),
            Complete(vec![compositor.clone()]),
        );
        assert!(complete.connected.is_some(), "a complete list is known");
        assert!(complete.is_connected("DP-3") && complete.is_connected("HDMI-1"));
        assert!(!complete.is_connected("DP-1"), "a complete list rejects a name it lacks");
        assert!(!complete.is_connected("no-such-screen"));
        // Not a Wayland session: Tauri's names are mpv's, and complete.
        let x11 = screens_from(Some(hdmi.clone()), Some(vec![hdmi.clone()]), NoneToAdd);
        assert!(x11.connected.is_some());
        assert!(!x11.is_connected("DP-1"));

        let without_mpv_name = NamedMonitor::for_test("Studio Display", None, 5120, 2880, -5120);
        for (why, screens) in [
            (
                "the compositor's list could not be read",
                screens_from(
                    Some(model.clone()),
                    Some(vec![model.clone(), hdmi.clone()]),
                    Incomplete(Vec::new()),
                ),
            ),
            (
                "an output the compositor lists by its model only",
                screens_from(
                    Some(model.clone()),
                    Some(vec![model.clone(), hdmi.clone()]),
                    Incomplete(vec![compositor.clone(), listed(&["U2720Q"], 3840, 2160, None)]),
                ),
            ),
            (
                "a monitor without the name mpv takes for it",
                screens_from(
                    Some(hdmi.clone()),
                    Some(vec![hdmi.clone(), without_mpv_name.clone()]),
                    NoneToAdd,
                ),
            ),
            (
                "Tauri's monitors could not be listed",
                screens_from(Some(hdmi.clone()), None, NoneToAdd),
            ),
        ] {
            assert_eq!(screens.connected, None, "{why}");
            for name in ["DP-1", "HDMI-1", "no-such-screen"] {
                assert!(screens.is_connected(name), "{why}: {name} is taken");
            }
        }
    }

    // An empty WAYLAND_DISPLAY names no display, so the compositor's list is
    // not asked for (the end-to-end venue: Xvfb inside a Wayland desktop).
    #[cfg(target_os = "linux")]
    #[test]
    fn an_empty_wayland_display_names_none() {
        assert!(names_a_display(Some(std::ffi::OsStr::new("wayland-1"))));
        assert!(!names_a_display(Some(std::ffi::OsStr::new(""))));
        assert!(!names_a_display(None));
    }

    // The compositor's list is complete only when every output has its
    // connector name.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_compositor_list_is_complete_only_when_every_output_has_its_connector() {
        use wayland_hdr::{listed as from_compositor, Described};
        let described = |name: Option<&str>, model: &str| Described {
            name: name.map(str::to_string),
            model: Some(model.to_string()),
            width: 3840,
            height: 2160,
            ..Default::default()
        };
        let both = vec![described(Some("DP-1"), "XG27UQDMS"), described(Some("DP-2"), "U2720Q")];
        assert_eq!(
            from_compositor(both),
            Complete(vec![
                listed(&["DP-1", "XG27UQDMS"], 3840, 2160, None),
                listed(&["DP-2", "U2720Q"], 3840, 2160, None),
            ])
        );
        assert_eq!(
            from_compositor(vec![described(Some("DP-1"), "XG27UQDMS"), described(None, "U2720Q")]),
            Incomplete(vec![
                listed(&["DP-1", "XG27UQDMS"], 3840, 2160, None),
                listed(&["U2720Q"], 3840, 2160, None),
            ]),
            "one output without its connector"
        );
        assert_eq!(
            from_compositor(vec![described(Some("  "), "XG27UQDMS")]),
            Incomplete(vec![listed(&["XG27UQDMS"], 3840, 2160, None)]),
            "a blank connector is none"
        );
        assert_eq!(
            from_compositor(vec![described(None, "XG27UQDMS"), described(None, "U2720Q")]),
            Incomplete(vec![
                listed(&["XG27UQDMS"], 3840, 2160, None),
                listed(&["U2720Q"], 3840, 2160, None),
            ]),
            "a compositor that gives models only"
        );
    }

    // Third review of finding pw-1, comment 4: the compositor's outputs are
    // given as they are shown. Each of the eight transforms, and the
    // reviewer's probe: a saved portrait output other than Vela's monitor is
    // ranked for 1080x1920, not for its unturned mode.
    #[cfg(target_os = "linux")]
    #[test]
    fn the_compositors_outputs_are_given_as_they_are_shown() {
        use wayland_client::{protocol::wl_output::Transform, WEnum};
        use wayland_hdr::{listed as from_compositor, Described};
        let shown = |transform: Option<WEnum<Transform>>| {
            let outputs = from_compositor(vec![Described {
                name: Some("DP-2".to_string()),
                model: Some("U2720Q".to_string()),
                width: 1920,
                height: 1080,
                transform,
            }]);
            let output = &outputs.outputs()[0];
            (output.width_px, output.height_px)
        };
        for (transform, expected) in [
            (Transform::Normal, (1920, 1080)),
            (Transform::_90, (1080, 1920)),
            (Transform::_180, (1920, 1080)),
            (Transform::_270, (1080, 1920)),
            (Transform::Flipped, (1920, 1080)),
            (Transform::Flipped90, (1080, 1920)),
            (Transform::Flipped180, (1920, 1080)),
            (Transform::Flipped270, (1080, 1920)),
        ] {
            assert_eq!(shown(Some(WEnum::Value(transform))), expected, "{transform:?}");
        }
        assert_eq!(shown(None), (1920, 1080), "no transform told: as the mode");
        assert_eq!(shown(Some(WEnum::Unknown(99))), (1920, 1080), "a transform not known: as the mode");

        // The reviewer's probe: Vela on HDMI-1, the saved DP-2 turned to
        // portrait.
        let hdmi = monitor("HDMI-1", "HDMI-1", 1920, 1080, 0);
        let portrait = monitor("U2720Q", "U2720Q", 1080, 1920, 1920);
        let screens = screens_from(
            Some(hdmi.clone()),
            Some(vec![hdmi, portrait]),
            from_compositor(vec![
                Described {
                    name: Some("HDMI-1".to_string()),
                    model: Some("HDMI-1".to_string()),
                    width: 1920,
                    height: 1080,
                    transform: Some(WEnum::Value(Transform::Normal)),
                },
                Described {
                    name: Some("DP-2".to_string()),
                    model: Some("U2720Q".to_string()),
                    width: 1920,
                    height: 1080,
                    transform: Some(WEnum::Value(Transform::_270)),
                },
            ]),
        );
        assert_eq!(size(&profile_basis(Some("DP-2"), None, &screens)), (Some("DP-2"), 1080, 1920));
    }

    // On Wayland Tauri names a screen by its model, so two screens of one
    // model share a name. Second review of finding pw-1: the screen holding
    // Vela's window is named by its own connector where one output carries
    // the model. Third review, comment 3: where two do, no size or position
    // tells them apart reliably (Tauri's sizes are scaled, the compositor's
    // modes are not), so no connector is taken, the name stays the model's,
    // and the profile is still that of the monitor holding Vela's window.
    #[test]
    fn two_screens_of_one_model_give_no_connector() {
        let left = monitor("XG27UQDMS", "XG27UQDMS", 3840, 2160, 0);
        let right = monitor("XG27UQDMS", "XG27UQDMS", 2560, 1440, 3840);
        let both = vec![left.clone(), right.clone()];
        let compositor = vec![
            listed(&["DP-1", "XG27UQDMS"], 3840, 2160, None),
            listed(&["DP-2", "XG27UQDMS"], 2560, 1440, None),
        ];
        let on_right = screens_from(Some(right.clone()), Some(both.clone()), Complete(compositor.clone()));
        assert_eq!(on_right.velas.as_deref(), Some("XG27UQDMS"), "no connector: the model stays");
        let basis = profile_basis(on_right.velas.as_deref(), None, &on_right);
        assert_eq!(size(&basis), (Some("XG27UQDMS"), 2560, 1440), "Vela's own monitor's values");
        assert_eq!(basis.position, Some((3840, 0)));
        assert_eq!(
            size(&profile_basis(Some("DP-1"), None, &on_right)),
            (Some("DP-1"), 3840, 2160),
            "the other screen, by its connector, is its own"
        );
        assert!(on_right.is_connected("DP-1") && on_right.is_connected("DP-2"));
        assert!(on_right.is_connected("XG27UQDMS"));
        let on_left = screens_from(Some(left.clone()), Some(both.clone()), Complete(compositor));
        assert_eq!(on_left.velas.as_deref(), Some("XG27UQDMS"));
        assert_eq!(size(&profile_basis(Some("DP-1"), None, &on_left)), (Some("DP-1"), 3840, 2160));
        let basis = profile_basis(on_left.velas.as_deref(), None, &on_left);
        assert_eq!((size(&basis), basis.position), ((Some("XG27UQDMS"), 3840, 2160), Some((0, 0))));

        // Every combination of sizes, for either output holding Vela's
        // window: whatever the sizes, no connector. Among them the
        // reviewer's probe: Vela's screen is DP-1, 3840x2160 shown at 150%,
        // which Tauri gives as 5120x2880, beside a native 5120x2880 DP-2.
        let sizes = [(3840, 2160), (2560, 1440), (5120, 2880), (1920, 1080)];
        for first in sizes {
            for second in sizes {
                for tauri in sizes {
                    let velas = monitor("XG27UQDMS", "XG27UQDMS", tauri.0, tauri.1, 0);
                    let compositor = vec![
                        listed(&["DP-1", "XG27UQDMS"], first.0, first.1, None),
                        listed(&["DP-2", "XG27UQDMS"], second.0, second.1, None),
                    ];
                    let seen = screens_from(Some(velas.clone()), Some(vec![velas]), Complete(compositor));
                    assert_eq!(
                        seen.velas.as_deref(),
                        Some("XG27UQDMS"),
                        "outputs {first:?} and {second:?}, Tauri gives {tauri:?}"
                    );
                    let basis = profile_basis(seen.velas.as_deref(), None, &seen);
                    assert_eq!(size(&basis), (Some("XG27UQDMS"), tauri.0, tauri.1));
                }
            }
        }

        // One output of that model: its connector, whatever size it lists.
        let one = screens_from(
            Some(right.clone()),
            Some(vec![right.clone()]),
            Complete(vec![listed(&["DP-3", "XG27UQDMS"], 5120, 2880, None)]),
        );
        assert_eq!(one.velas.as_deref(), Some("DP-3"));
        // A compositor that gives no connector names: the model stays.
        let nameless = screens_from(
            Some(right.clone()),
            Some(vec![right]),
            Incomplete(vec![listed(&["XG27UQDMS"], 2560, 1440, None)]),
        );
        assert_eq!(nameless.velas.as_deref(), Some("XG27UQDMS"));
    }

    // Fourth review of finding pw-1 (the attack's finding 1): when what is
    // connected is not known, the outputs that were seen still give a
    // decided output its own size; it is not ranked for Vela's screen. Both
    // of the attack's probes, and every other way of being incomplete.
    #[test]
    fn an_incomplete_list_still_gives_a_decided_output_its_size() {
        // Wayland: Vela's 1080p screen is listed by its model only; the
        // compositor lists DP-1 (4K) by its connector.
        let model = monitor("U2720Q", "U2720Q", 1920, 1080, 0);
        let wayland = screens_from(
            Some(model.clone()),
            Some(vec![model.clone()]),
            Incomplete(vec![
                listed(&["U2720Q"], 1920, 1080, None),
                listed(&["DP-1", "XG27UQDMS"], 3840, 2160, None),
            ]),
        );
        assert_eq!(wayland.connected, None);
        assert_eq!(
            size(&profile_basis(Some("DP-1"), None, &wayland)),
            (Some("DP-1"), 3840, 2160),
            "Wayland: DP-1 is ranked for its own size"
        );
        // macOS: "Monitor #1" has no mpv name; "Monitor #2" is the saved
        // "LG HDR 4K (7)".
        let first = NamedMonitor::for_test("Monitor #1", None, 5120, 2880, 0);
        let second = monitor("Monitor #2", "LG HDR 4K (7)", 3840, 2160, 5120);
        let mac = screens_from(Some(first.clone()), Some(vec![first, second.clone()]), NoneToAdd);
        assert_eq!(mac.connected, None);
        assert_eq!(
            size(&profile_basis(Some("LG HDR 4K (7)"), None, &mac)),
            (Some("LG HDR 4K (7)"), 3840, 2160),
            "macOS: the saved screen is ranked for its own size"
        );
        // The compositor's list could not be read: Tauri's monitors still
        // give theirs.
        let hdmi = monitor("HDMI-1", "HDMI-1", 1920, 1080, 0);
        let unread = screens_from(
            Some(hdmi.clone()),
            Some(vec![hdmi.clone(), second.clone()]),
            Incomplete(Vec::new()),
        );
        assert_eq!(unread.connected, None);
        assert_eq!(
            size(&profile_basis(Some("LG HDR 4K (7)"), None, &unread)),
            (Some("LG HDR 4K (7)"), 3840, 2160),
            "an unread compositor list"
        );
        // Tauri's monitors could not be listed: the compositor's list still
        // gives its outputs.
        let unlisted = screens_from(
            Some(hdmi),
            None,
            Complete(vec![listed(&["DP-1", "XG27UQDMS"], 3840, 2160, None)]),
        );
        assert_eq!(unlisted.connected, None);
        assert_eq!(
            size(&profile_basis(Some("DP-1"), None, &unlisted)),
            (Some("DP-1"), 3840, 2160),
            "Tauri's monitors unlisted"
        );
        // What was seen never tells what is connected: every name stands.
        assert!(mac.is_connected("DP-9") && wayland.is_connected("DP-9"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_screen_name_matches_mpv_product_and_serial_contract() {
        assert_eq!(mac_mpv_name("Studio Display", 1234), "Studio Display (1234)");
    }
}
