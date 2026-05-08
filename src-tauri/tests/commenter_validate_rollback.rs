use ai_comment_orchestrator::commenter::{
    rollback::{can_overwrite_for_rollback, RollbackGuard},
    validate::{validate_candidate, FileValidationInput, ValidationDecision},
};

#[test]
fn strong_validation_rejects_markdown_fence() {
    let result = validate_candidate(
        FileValidationInput::source("main.go", "package main\nfunc main() {}\n"),
        "```go\npackage main\nfunc main() {}\n```",
    );

    assert!(matches!(result.decision, ValidationDecision::Reject(_)));
}

#[test]
fn rollback_refuses_when_current_hash_drifted() {
    let outcome = can_overwrite_for_rollback("before-hash", "after-hash", "user-edited-hash");
    assert_eq!(outcome, RollbackGuard::Conflict);
}
