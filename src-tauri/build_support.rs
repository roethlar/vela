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

#[cfg(test)]
mod tests {
    use super::*;

    struct Tree(PathBuf);

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn copies_every_mpv_resource_in_old_new_and_target_triple_layouts() {
        let tree = Tree(std::env::temp_dir().join(format!("vela-build-{}", uuid::Uuid::new_v4())));
        let source = tree.0.join("resources/mpv-scripts");
        std::fs::create_dir_all(&source).unwrap();
        let files = [
            ("markers.lua", b"-- markers\n\0".as_slice()),
            ("autocrop.lua", b"-- crop\n".as_slice()),
            ("LICENSE.GPL", b"license\n".as_slice()),
            ("empty", b"".as_slice()),
        ];
        for (name, contents) in files {
            std::fs::write(source.join(name), contents).unwrap();
        }
        for profile in ["target/debug", "target/x86_64-pc-windows-msvc/debug"] {
            for layout in ["build/vela-hash/out", "build/vela/hash/out"] {
                let profile = tree.0.join(profile);
                let out_dir = profile.join(layout);
                std::fs::create_dir_all(&out_dir).unwrap();
                let target = copy_mpv_scripts(&out_dir, &source).unwrap();
                assert_eq!(target, profile.join("mpv-scripts"));
                assert_eq!(std::fs::read_dir(&target).unwrap().count(), files.len());
                for (name, contents) in files {
                    assert_eq!(std::fs::read(target.join(name)).unwrap(), contents);
                }
                // Each layout must work from an empty destination, not inherit
                // the previous layout's copies (as the E2E build once did).
                std::fs::remove_dir_all(target).unwrap();
            }
        }
    }
}
