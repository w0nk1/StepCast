#[test]
fn capture_region_uses_screencapture_cli() {
    const SOURCE: &str = include_str!("../src/recorder/cg_capture.rs");

    assert!(
        SOURCE.contains("screencapture"),
        "capture should use screencapture CLI"
    );
    assert!(
        SOURCE.contains("-R"),
        "capture should use -R flag for region capture"
    );
}
