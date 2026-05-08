use std::path::Path;

use ai_comment_orchestrator::commenter::artifacts::CommenterRunPaths;

#[test]
fn builds_run_directory_structure_under_data_dir() {
    let run_paths =
        CommenterRunPaths::new(Path::new("C:/tmp/data-root"), "run-123").expect("paths");

    assert!(run_paths.run_root.ends_with("run-123"));
    assert!(run_paths.manifest_root.ends_with("manifest"));
    assert!(run_paths.before_root.ends_with("before"));
    assert!(run_paths.candidate_root.ends_with("candidates"));
    assert!(run_paths.sidecar_root.ends_with("sidecars"));
    assert!(run_paths.request_root.ends_with("request"));
    assert!(run_paths.response_root.ends_with("response"));
    assert!(run_paths.logs_root.ends_with("logs"));
}
