use std::path::{Path, PathBuf};

/// tauri-build 2.6 copies resources three levels above OUT_DIR, which works
/// for Cargo's old build/<pkg>-<hash>/out layout but not build/<pkg>/<hash>/out.
/// Copy to the parent of `build` instead (as tauri-build 2.7.5 does; drop this
/// workaround once Vela takes it). The mapping is tauri.conf.json's
/// "resources/mpv-scripts/": "mpv-scripts/".
///
/// This is beside the binary only when Cargo keeps intermediates and final
/// artifacts together. A build script cannot see a build-dir or target-dir
/// set only in a Cargo config file, so OUT_DIR cannot locate a separately
/// configured target directory. The E2E runner copies these resources beside
/// the executable from Cargo's report (tests/e2e/app-binary.mjs). Other
/// unbundled builds with separate directories must arrange that copy too.
/// Bundles are unaffected: the bundler copies resources from the source tree.
pub fn copy_mpv_scripts(out_dir: &Path, source: &Path) -> std::io::Result<PathBuf> {
    let profile_dir = out_dir
        .ancestors()
        .find(|path| path.file_name() == Some(std::ffi::OsStr::new("build")))
        .and_then(Path::parent)
        .ok_or_else(|| std::io::Error::other("OUT_DIR lies outside a profile's build directory"))?;
    let target = profile_dir.join("mpv-scripts");
    std::fs::create_dir_all(&target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            std::fs::copy(entry.path(), target.join(entry.file_name()))?;
        }
    }
    Ok(target)
}

