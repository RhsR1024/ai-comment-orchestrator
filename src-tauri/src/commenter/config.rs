use std::{env, fs, path::PathBuf};

use serde::Deserialize;

use super::models::{CommentAppSettings, CommentCredentialProfile, CredentialSourceKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CredentialSource {
    EnvVar(String),
    InlineSecretHandle(String),
    JsonFile { path: PathBuf, key: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigError {
    EmptySourceReference,
    InvalidJsonReference,
    MissingJsonPath,
    MissingJsonKey,
    MissingCredentialReference,
    MissingEnvVar(String),
    EmptyResolvedCredential,
    UnsupportedInlineSecretHandle(String),
    ReadJsonFile(String),
    JsonKeyNotFound(String),
    JsonValueNotString(String),
}

pub fn resolve_credential_source(
    profile: &CommentCredentialProfile,
) -> Result<CredentialSource, ConfigError> {
    let source_reference = profile.source_reference.trim();
    if source_reference.is_empty() {
        return Err(ConfigError::EmptySourceReference);
    }

    match profile.source_kind {
        CredentialSourceKind::EnvVar => Ok(CredentialSource::EnvVar(source_reference.to_string())),
        CredentialSourceKind::InlineSecret => Ok(CredentialSource::InlineSecretHandle(
            source_reference.to_string(),
        )),
        CredentialSourceKind::JsonFile => {
            let reference: JsonFileReference = serde_json::from_str(source_reference)
                .map_err(|_| ConfigError::InvalidJsonReference)?;
            let path = reference.path.trim();
            if path.is_empty() {
                return Err(ConfigError::MissingJsonPath);
            }

            let key = reference.key.trim();
            if key.is_empty() {
                return Err(ConfigError::MissingJsonKey);
            }

            Ok(CredentialSource::JsonFile {
                path: PathBuf::from(path),
                key: key.to_string(),
            })
        }
    }
}

pub fn resolve_bearer_token(app_settings: &CommentAppSettings) -> Result<String, ConfigError> {
    normalize_bearer_token_value(&app_settings.api_bearer_token)
}

pub fn describe_bearer_token_source(
    app_settings: &CommentAppSettings,
) -> Result<String, ConfigError> {
    if app_settings.api_bearer_token.trim().is_empty() {
        return Err(ConfigError::MissingCredentialReference);
    }

    Ok("global api_bearer_token".to_string())
}

pub fn resolve_credential_value(source: &CredentialSource) -> Result<String, ConfigError> {
    let token = match source {
        CredentialSource::EnvVar(name) => {
            env::var(name).map_err(|_| ConfigError::MissingEnvVar(name.to_string()))?
        }
        CredentialSource::InlineSecretHandle(handle) => {
            return Err(ConfigError::UnsupportedInlineSecretHandle(
                handle.to_string(),
            ));
        }
        CredentialSource::JsonFile { path, key } => {
            let raw = fs::read_to_string(path)
                .map_err(|error| ConfigError::ReadJsonFile(error.to_string()))?;
            let value: serde_json::Value =
                serde_json::from_str(&raw).map_err(|_| ConfigError::InvalidJsonReference)?;
            value
                .get(key)
                .ok_or_else(|| ConfigError::JsonKeyNotFound(key.to_string()))?
                .as_str()
                .ok_or_else(|| ConfigError::JsonValueNotString(key.to_string()))?
                .to_string()
        }
    };

    normalize_bearer_token_value(&token)
}

fn normalize_bearer_token_value(token: &str) -> Result<String, ConfigError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(ConfigError::EmptyResolvedCredential);
    }

    let token = match token.split_once(' ') {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("bearer") => rest.trim(),
        _ => token,
    };
    if token.is_empty() {
        return Err(ConfigError::EmptyResolvedCredential);
    }

    Ok(token.to_string())
}

#[derive(Debug, Deserialize)]
struct JsonFileReference {
    path: String,
    key: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(kind: CredentialSourceKind, source_reference: &str) -> CommentCredentialProfile {
        CommentCredentialProfile {
            id: 1,
            profile_key: "default".to_string(),
            display_name: "Default".to_string(),
            source_kind: kind,
            source_reference: source_reference.to_string(),
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn resolves_env_var_and_inline_secret_handles_without_persisting_secret_values() {
        let env_var =
            resolve_credential_source(&profile(CredentialSourceKind::EnvVar, "OPENAI_API_KEY"))
                .expect("env var");
        let inline_secret = resolve_credential_source(&profile(
            CredentialSourceKind::InlineSecret,
            "secret://credential/ui-inline",
        ))
        .expect("inline secret handle");

        assert_eq!(
            env_var,
            CredentialSource::EnvVar("OPENAI_API_KEY".to_string())
        );
        assert_eq!(
            inline_secret,
            CredentialSource::InlineSecretHandle("secret://credential/ui-inline".to_string())
        );
    }

    #[test]
    fn rejects_blank_json_file_members() {
        let missing_path = resolve_credential_source(&profile(
            CredentialSourceKind::JsonFile,
            r#"{"path":" ","key":"token"}"#,
        ));
        let missing_key = resolve_credential_source(&profile(
            CredentialSourceKind::JsonFile,
            r#"{"path":"C:/tmp/token.json","key":" "}"#,
        ));

        assert_eq!(missing_path, Err(ConfigError::MissingJsonPath));
        assert_eq!(missing_key, Err(ConfigError::MissingJsonKey));
    }

    fn app_settings(api_bearer_token: &str) -> CommentAppSettings {
        CommentAppSettings {
            global_max_workers: 1,
            api_concurrency_limit: 1,
            api_bearer_token: api_bearer_token.to_string(),
        }
    }

    #[test]
    fn resolves_global_api_bearer_token() {
        let token = resolve_bearer_token(&app_settings(" direct-token ")).expect("global token");

        assert_eq!(token, "direct-token");
    }

    #[test]
    fn strips_bearer_prefix_from_global_api_token() {
        let token =
            resolve_bearer_token(&app_settings(" Bearer direct-token ")).expect("global token");

        assert_eq!(token, "direct-token");
    }

    #[test]
    fn rejects_missing_global_api_token_without_env_fallback() {
        env::set_var("COMMENTER_TEST_TOKEN", "env-token");
        let result = resolve_bearer_token(&app_settings(""));
        env::remove_var("COMMENTER_TEST_TOKEN");

        assert_eq!(result, Err(ConfigError::EmptyResolvedCredential));
    }
}
