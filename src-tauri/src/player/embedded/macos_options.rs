//! Protected macOS options, separate from Linux colour negotiation.
use super::LaunchArg;

/// A standard-range OpenGL layer, tagged sRGB by the host. Explicit targets
/// override user HDR targets; mpv tone-maps PQ/HLG input into this SDR output.
/// 203 nits is mpv 0.42's documented SDR reference.
/// No wid, display hints, EGL/DRM or Wayland colour-management options.
pub(super) fn required_args() -> Vec<LaunchArg> {
    vec![
        LaunchArg::set("vo", "libmpv"),
        LaunchArg::set("force-window", "no"),
        // render.h pairs this with BLOCK_FOR_TARGET_TIME=0: hand frames to
        // the main-thread renderer when due, without making that thread wait.
        LaunchArg::set("video-timing-offset", "0"),
        LaunchArg::set("target-prim", "bt.709"),
        LaunchArg::set("target-trc", "srgb"),
        LaunchArg::set("target-peak", "203"),
        LaunchArg::set("icc-profile-auto", "no"),
        LaunchArg::set("icc-profile", ""),
        LaunchArg::set("osc", "no"),
        LaunchArg::set("input-default-bindings", "no"),
        LaunchArg::set("input-builtin-bindings", "no"),
        LaunchArg::set("input-vo-keyboard", "no"),
        LaunchArg::set("input-cursor", "no"),
        LaunchArg::set("input-media-keys", "no"),
        LaunchArg::set("input-terminal", "no"),
        LaunchArg::set("input-builtin-dragging", "no"),
        LaunchArg::set("input-builtin-drag-and-drop", "no"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn macos_options_use_the_render_api_in_standard_range_without_a_window_id() {
        let args = required_args();
        let value = |name| {
            args.iter().find_map(|arg| match arg {
                LaunchArg::Option { name: key, value } if *key == name => value.as_deref(),
                _ => None,
            })
        };
        assert_eq!(value("vo"), Some("libmpv"));
        assert_eq!(value("force-window"), Some("no"));
        assert_eq!(value("video-timing-offset"), Some("0"));
        assert_eq!(value("wid"), None);
        assert_eq!(value("target-prim"), Some("bt.709"));
        assert_eq!(value("target-trc"), Some("srgb"));
        assert_eq!(value("target-peak"), Some("203"));
        assert_eq!(value("input-media-keys"), Some("no"));
        assert_eq!(value("icc-profile"), Some(""));
    }
}
