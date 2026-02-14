use serde_json::Value;

#[test]
fn tauri_conf_has_restrictive_csp_and_capabilities() {
    let raw = include_str!("../tauri.conf.json");
    let json: Value = serde_json::from_str(raw).expect("parse tauri.conf.json");

    let security = &json["app"]["security"];
    let csp = security["csp"]
        .as_object()
        .expect("csp should be an object");
    let dev_csp = security["devCsp"]
        .as_object()
        .expect("devCsp should be an object");

    let img_src = csp["img-src"].as_str().unwrap_or("");
    assert!(img_src.contains("asset:"), "csp img-src must allow asset:");
    assert!(
        img_src.contains("http://asset.localhost"),
        "csp img-src must allow asset.localhost"
    );

    let connect_src = csp["connect-src"].as_str().unwrap_or("");
    assert!(
        connect_src.contains("ipc:"),
        "csp connect-src must allow ipc:"
    );
    assert!(
        connect_src.contains("http://ipc.localhost"),
        "csp connect-src must allow ipc.localhost"
    );

    let dev_default = dev_csp["default-src"].as_str().unwrap_or("");
    assert!(
        dev_default.contains("http://localhost:1420"),
        "devCsp default-src must allow dev server"
    );
    let dev_connect = dev_csp["connect-src"].as_str().unwrap_or("");
    assert!(
        dev_connect.contains("ws://localhost:1420"),
        "devCsp connect-src must allow HMR websocket"
    );

    let capabilities = security["capabilities"]
        .as_array()
        .expect("capabilities must be an array");
    assert!(
        capabilities.iter().any(|v| v == "default"),
        "capabilities must include 'default'"
    );
}

#[test]
fn opener_open_url_is_scoped_to_system_preferences() {
    let raw = include_str!("../capabilities/default.json");
    let json: Value = serde_json::from_str(raw).expect("parse default capability");
    let perms = json["permissions"]
        .as_array()
        .expect("permissions should be array");

    let opener = perms
        .iter()
        .find(|perm| perm["identifier"] == "opener:allow-open-url")
        .expect("opener:allow-open-url permission missing");
    let allow = opener["allow"]
        .as_array()
        .expect("opener allow should be array");
    let has_system_prefs = allow
        .iter()
        .any(|entry| entry["url"] == "x-apple.systempreferences:*");
    assert!(
        has_system_prefs,
        "open-url must be scoped to system preferences"
    );
}

#[test]
fn build_script_generates_command_permissions() {
    let build_rs = include_str!("../build.rs");
    assert!(
        build_rs.contains("AppManifest::new()"),
        "build.rs must configure AppManifest"
    );
    assert!(
        build_rs.contains(".commands("),
        "build.rs must set AppManifest::commands"
    );
}

#[test]
fn default_capability_allows_app_commands() {
    let raw = include_str!("../capabilities/default.json");
    let json: Value = serde_json::from_str(raw).expect("parse default capability");
    let perms = json["permissions"]
        .as_array()
        .expect("permissions should be array");

    let perm_ids: Vec<&str> = perms.iter().filter_map(|perm| perm.as_str()).collect();
    let required = [
        "allow-check-permissions",
        "allow-request-screen-recording",
        "allow-request-accessibility",
        "allow-start-recording",
        "allow-pause-recording",
        "allow-resume-recording",
        "allow-stop-recording",
        "allow-get-steps",
        "allow-export-guide",
        "allow-discard-recording",
    ];

    for id in required {
        assert!(
            perm_ids.iter().any(|perm| perm == &id),
            "default capability missing permission: {id}"
        );
    }
}
