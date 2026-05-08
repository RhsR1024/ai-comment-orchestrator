use std::fs;

use ai_comment_orchestrator::commenter::{
    commands::{CommenterCommandSurface, CommenterEnqueueRunRequest, CommenterProjectProfileDraft},
    models::{CommentProjectSettings, CommentRunMode, JsonHandlingStrategy},
};
use tempfile::tempdir;

fn draft(root_path: &str) -> CommenterProjectProfileDraft {
    CommenterProjectProfileDraft {
        project_key: "demo".to_string(),
        profile_name: "Demo".to_string(),
        root_path: root_path.to_string(),
        include_extensions: vec!["go".to_string()],
        exclude_directories: vec!["node_modules".to_string()],
        prompt_template: "annotate".to_string(),
        settings: CommentProjectSettings {
            credential_profile_key: "OPENAI_API_KEY".to_string(),
            default_run_mode: CommentRunMode::Review,
            default_max_workers: 2,
            default_max_retries: 1,
            default_max_files: 10,
            allow_light_rewrite: true,
            json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
            api_base_url: "https://example.com".to_string(),
            api_model: "glm-5.0".to_string(),
            api_bearer_token: String::new(),
            request_timeout_secs: 600,
        },
    }
}

#[test]
fn reloads_profiles_and_runs_from_disk_after_restart() {
    let temp = tempdir().expect("tempdir");
    let project_root = temp.path().join("project");
    fs::create_dir_all(project_root.join("src")).expect("src dir");
    fs::write(
        project_root.join("src/main.go"),
        "package main\nfunc main() {}\n",
    )
    .expect("source");

    let data_root = temp.path().join(".commenter-data");
    let service = CommenterCommandSurface::new(&data_root);
    service
        .upsert_project_profile(draft(project_root.to_string_lossy().as_ref()))
        .expect("profile");
    let handle = service
        .enqueue_run(CommenterEnqueueRunRequest {
            profile_key: "demo".to_string(),
            requested_by: Some("test".to_string()),
            run_mode: CommentRunMode::Review,
            max_workers: 2,
            max_retries: 1,
            max_files: 10,
            allow_light_rewrite: true,
            json_handling_strategy: JsonHandlingStrategy::SidecarOnly,
        })
        .expect("run");
    drop(service);

    let reopened = CommenterCommandSurface::new(&data_root);
    let profiles = reopened.list_project_profiles().expect("profiles");
    let runs = reopened.list_runs().expect("runs");

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0].project_key, "demo");
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].run_key, handle.run_key);
}
