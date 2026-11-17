fn main() {
    tauri_build::build();
    copy_mpv_scripts_next_to_the_binary();
}

/// tauri-build 2.6 copies `bundle.resources` to the directory three levels
/// above `OUT_DIR`. That is the profile directory (`target/debug`) only in
/// Cargo's old build-dir layout, `build/<pkg>-<hash>/out`; Cargo 1.99 and
/// later lay it out as `build/<pkg>/<hash>/out`, so the copies land in
/// `target/debug/build/` and a debug or `--no-bundle` build, whose resource
/// directory is the binary's own, finds no mpv scripts: markers and autocrop
/// were missing from the Windows end-to-end build (its runner's Cargo 1.100),
/// and from any fresh build here. So copy them where Tauri resolves them for
/// such builds: the profile directory, the parent of the `build` directory
/// above `OUT_DIR` (as tauri-build 2.7.5 does; drop this once Vela takes it).
/// Bundles are unaffected: the bundler copies resources from the source tree.
/// The mapping is tauri.conf.json's, `"resources/mpv-scripts/": "mpv-scripts/"`
/// (tests/dependency-policy.test.mjs holds the two together).
fn copy_mpv_scripts_next_to_the_binary() {
    use std::path::{Path, PathBuf};
    println!("cargo:rerun-if-changed=resources/mpv-scripts");
    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let profile_dir = out_dir
        .ancestors()
        .find(|path| path.file_name() == Some(std::ffi::OsStr::new("build")))
        .and_then(Path::parent)
        .expect("OUT_DIR lies under the profile's build directory");
    let target = profile_dir.join("mpv-scripts");
    std::fs::create_dir_all(&target).expect("create the mpv-scripts resource directory");
    for entry in std::fs::read_dir("resources/mpv-scripts").expect("read resources/mpv-scripts") {
        let entry = entry.expect("list resources/mpv-scripts");
        if entry.file_type().is_ok_and(|kind| kind.is_file()) {
            std::fs::copy(entry.path(), target.join(entry.file_name()))
                .expect("copy an mpv script next to the binary");
        }
    }
}
