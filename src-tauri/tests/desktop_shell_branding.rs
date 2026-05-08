use std::{fs, path::PathBuf};

fn manifest_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn windows_desktop_entry_uses_window_subsystem() {
    let main_rs = fs::read_to_string(manifest_path().join("src").join("main.rs"))
        .expect("main.rs should be readable");

    assert!(
        main_rs.contains("windows_subsystem = \"windows\""),
        "main.rs should hide the Windows console window for desktop launches"
    );
}

#[test]
fn tauri_configuration_uses_comment_orchestrator_branding() {
    let tauri_conf = fs::read_to_string(manifest_path().join("tauri.conf.json"))
        .expect("tauri.conf.json should be readable");

    assert!(
        tauri_conf.contains("\"productName\": \"ai-comment-orchestrator\""),
        "productName should reflect the real application name"
    );
    assert!(
        tauri_conf.contains("\"title\": \"ai-comment-orchestrator\""),
        "window title should reflect the real application name"
    );
}
