#[path = "build_support.rs"]
mod build_support;

fn main() {
    tauri_build::build();
    println!("cargo:rerun-if-changed=resources/mpv-scripts");
    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo sets OUT_DIR"));
    build_support::copy_mpv_scripts(&out_dir, std::path::Path::new("resources/mpv-scripts"))
        .expect("copy the mpv scripts under the build profile");
}
