fn main() {
    println!("cargo:rerun-if-env-changed=APPLE_SIGNING_IDENTITY");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        // Ensure Swift runtime dylibs required by ScreenCaptureKit bridge can be resolved
        // when running test binaries outside an app bundle.
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }

    build_swift_ai_helper();

    let manifest = tauri_build::AppManifest::new().commands(&[
        "greet",
        "check_permissions",
        "get_apple_intelligence_eligibility",
        "request_screen_recording",
        "request_accessibility",
        "start_recording",
        "pause_recording",
        "resume_recording",
        "stop_recording",
        "get_steps",
        "update_step_note",
        "update_step_description",
        "update_step_crop",
        "export_guide",
        "delete_step",
        "reorder_steps",
        "open_editor_window",
        "discard_recording",
        "generate_step_descriptions",
        "get_startup_state",
        "mark_startup_seen",
        "dismiss_whats_new",
    ]);

    let attrs = tauri_build::Attributes::new().app_manifest(manifest);
    tauri_build::try_build(attrs).expect("failed to run build script");
}

fn build_swift_ai_helper() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }

    let manifest_dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));

    let swift_dir = manifest_dir.join("swift");
    let mut sources: Vec<std::path::PathBuf> = Vec::new();
    let read_dir = std::fs::read_dir(&swift_dir)
        .unwrap_or_else(|e| panic!("read swift dir {}: {e}", swift_dir.display()));
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("swift") {
            sources.push(path);
        }
    }
    sources.sort();
    if sources.is_empty() {
        return;
    }
    for src in &sources {
        println!("cargo:rerun-if-changed={}", src.display());
    }

    let dst = out_dir.join("stepcast_ai_helper");
    let resource_dst = manifest_dir.join("bin").join("stepcast_ai_helper");

    // Incremental: skip compiling if the output is newer than the source.
    let latest_src_mtime = sources
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok()?.modified().ok())
        .max();
    let mut needs_compile = true;
    if let (Some(src_mtime), Ok(dst_meta)) = (latest_src_mtime, std::fs::metadata(&dst)) {
        if let Ok(dst_mtime) = dst_meta.modified() {
            if dst_mtime >= src_mtime {
                needs_compile = false;
            }
        }
    }

    if needs_compile {
        let mut cmd = std::process::Command::new("swiftc");
        cmd.arg("-parse-as-library").arg("-O");
        for src in &sources {
            cmd.arg(src);
        }
        let status = cmd
            .arg("-o")
            .arg(&dst)
            .status()
            .expect("failed to run swiftc");

        if !status.success() {
            panic!("swiftc failed: {status}");
        }
    }

    // Also emit the helper as a bundle resource for notarization/codesigning correctness.
    // We execute this helper directly from the app bundle in release builds.
    let resource_dir = resource_dst.parent().expect("resource dst parent");
    std::fs::create_dir_all(resource_dir)
        .unwrap_or_else(|e| panic!("create resource dir {}: {e}", resource_dir.display()));

    let should_copy = match (std::fs::read(&dst), std::fs::read(&resource_dst)) {
        (Ok(a), Ok(b)) => a != b,
        (Ok(_), Err(_)) => true,
        (Err(e), _) => panic!("read swift helper output {}: {e}", dst.display()),
    };

    if should_copy {
        std::fs::copy(&dst, &resource_dst).unwrap_or_else(|e| {
            panic!(
                "copy swift helper {} -> {}: {e}",
                dst.display(),
                resource_dst.display()
            )
        });
    }

    #[cfg(target_os = "macos")]
    {
        use std::os::unix::fs::PermissionsExt;
        let perm = std::fs::Permissions::from_mode(0o755);
        let _ = std::fs::set_permissions(&resource_dst, perm);
    }

    // Notarization requires nested executables in the app bundle to be properly signed.
    // Tauri signs app binaries, but custom resource executables need an explicit pass.
    if let Ok(identity_raw) = std::env::var("APPLE_SIGNING_IDENTITY") {
        let identity = identity_raw.trim();
        if !identity.is_empty() {
            let status = std::process::Command::new("codesign")
                .arg("--force")
                .arg("--sign")
                .arg(identity)
                .arg("--timestamp")
                .arg("--options")
                .arg("runtime")
                .arg(&resource_dst)
                .status()
                .unwrap_or_else(|e| panic!("failed to run codesign for {}: {e}", resource_dst.display()));

            if !status.success() {
                panic!(
                    "codesign failed for swift helper {} with status {status}",
                    resource_dst.display()
                );
            }
        }
    }
}
