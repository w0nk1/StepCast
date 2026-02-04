fn main() {
    let manifest = tauri_build::AppManifest::new().commands(&[
        "greet",
        "check_permissions",
        "request_permissions",
        "start_recording",
        "pause_recording",
        "resume_recording",
        "stop_recording",
        "get_steps",
        "export_html",
        "export_markdown",
        "export_html_temp",
        "discard_recording",
    ]);

    let attrs = tauri_build::Attributes::new().app_manifest(manifest);
    tauri_build::try_build(attrs).expect("failed to run build script");
}
