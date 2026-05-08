use std::path::PathBuf;

use ai_comment_orchestrator::commenter::{
    config::{resolve_credential_source, CredentialSource},
    models::{CommentCredentialProfile, CredentialSourceKind},
};

#[test]
fn resolves_json_file_reference_payload() {
    let profile = CommentCredentialProfile {
        id: 1,
        profile_key: "json-source".to_string(),
        display_name: "JSON Source".to_string(),
        source_kind: CredentialSourceKind::JsonFile,
        source_reference: r#"{"path":"C:/tmp/token.json","key":"access_token"}"#.to_string(),
        created_at: 1,
        updated_at: 1,
    };

    let resolved = resolve_credential_source(&profile).expect("resolved source");

    assert_eq!(
        resolved,
        CredentialSource::JsonFile {
            path: PathBuf::from("C:/tmp/token.json"),
            key: "access_token".to_string(),
        }
    );
}
