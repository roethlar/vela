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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResolutionOverride {
    P720,
    P1080,
    P1440,
    P2160,
    P4320,
}

impl ResolutionOverride {
    pub(crate) fn normalize(value: Option<&str>) -> Option<Self> {
        match value {
            Some("720p") => Some(Self::P720),
            Some("1080p") => Some(Self::P1080),
            Some("1440p") => Some(Self::P1440),
            Some("2160p") => Some(Self::P2160),
            Some("4320p") => Some(Self::P4320),
            _ => None,
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
    pub(crate) fn normalize(value: Option<&str>) -> Option<Self> {
        match value {
            Some("enabled") => Some(Self::Enabled),
            Some("disabled") => Some(Self::Disabled),
            _ => None,
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

/// The current Vela-window output name is passed to mpv for a manual launch.
/// Wayland compositors may still choose a different output; the observed mpv
/// output then becomes authoritative for successors.
pub(crate) async fn current_screen_name(app: &tauri::AppHandle) -> Option<String> {
    let monitor = current_monitor(app)?;
    platform_screen_name(app, monitor).await
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
        .and_then(|value| value.names.first().cloned())
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
            let mut source = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
            source.header = DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
                adapterId: path.sourceInfo.adapterId,
                id: path.sourceInfo.id,
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

            let mut color = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO::default();
            color.header = DISPLAYCONFIG_DEVICE_INFO_HEADER {
                r#type: DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
                size: size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>() as u32,
                adapterId: path.targetInfo.adapterId,
                id: path.targetInfo.id,
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
        if std::env::var("XDG_SESSION_TYPE")
            .ok()
            .is_some_and(|value| value.eq_ignore_ascii_case("x11"))
            || std::env::var_os("WAYLAND_DISPLAY").is_none()
        {
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
                wl_output::Event::Mode {
                    flags,
                    width,
                    height,
                    ..
                } if matches!(flags, WEnum::Value(value) if value.contains(wl_output::Mode::Current)) =>
                {
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
    fn unknown_display_override_values_normalize_to_auto() {
        assert_eq!(ResolutionOverride::normalize(None), None);
        assert_eq!(ResolutionOverride::normalize(Some("5k")), None);
        assert_eq!(HdrOverride::normalize(Some("maybe")), None);
        assert_eq!(
            ResolutionOverride::normalize(Some("2160p")),
            Some(ResolutionOverride::P2160)
        );
        assert_eq!(
            HdrOverride::normalize(Some("disabled")),
            Some(HdrOverride::Disabled)
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


    #[cfg(target_os = "macos")]
    #[test]
    fn mac_screen_name_matches_mpv_product_and_serial_contract() {
        assert_eq!(mac_mpv_name("Studio Display", 1234), "Studio Display (1234)");
    }
}
