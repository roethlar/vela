"""Probe M2 only: give a debug Vela a way to run probes/m2-vela/drive.js.

Edits the probe's own checkout on the runner (src-tauri/src/lib.rs), never
the repository: with VELA_PROBE_SCRIPT naming a file, the app evaluates it in
the main window's page three seconds after start-up and prints every
"vela-probe" event the page emits as "probe: <payload>" on stderr. The page
may emit events under Vela's default capability (core:default).
"""
import sys

path = sys.argv[1]
source = open(path).read()
anchor = """        .setup(move |app| {
            use tauri::Manager;
"""
assert source.count(anchor) == 1, "the setup anchor moved; update probes/m2-vela/hook.py"
hook = anchor + """
            // PROBE M2 ONLY (probes/m2-vela/hook.py); never committed.
            if let Some(script) = std::env::var_os("VELA_PROBE_SCRIPT")
                .and_then(|path| std::fs::read_to_string(path).ok())
            {
                use tauri::Listener;
                app.handle().listen_any("vela-probe", |event| {
                    eprintln!("probe: {}", event.payload());
                });
                let handle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(3));
                    match handle.get_webview_window("main") {
                        Some(window) => {
                            if let Err(error) = window.eval(script.as_str()) {
                                eprintln!("probe: eval failed: {error}");
                            }
                        }
                        None => eprintln!("probe: no main window"),
                    }
                });
            }
"""
open(path, "w").write(source.replace(anchor, hook))
print("probe hook added to", path)
