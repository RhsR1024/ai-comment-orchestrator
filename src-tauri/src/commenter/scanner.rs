use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WriteStrategy {
    AnnotateInPlace,
    SidecarOnly,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileKind {
    pub normalized_extension: String,
    pub language_hint: Option<String>,
    pub write_strategy: WriteStrategy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScannedFile {
    pub relative_path: String,
    pub absolute_path: PathBuf,
    pub kind: FileKind,
}

impl FileKind {
    fn annotate_in_place(normalized_extension: String, language_hint: String) -> Self {
        Self {
            normalized_extension,
            language_hint: Some(language_hint),
            write_strategy: WriteStrategy::AnnotateInPlace,
        }
    }

    fn sidecar_only(normalized_extension: String, language_hint: String) -> Self {
        Self {
            normalized_extension,
            language_hint: Some(language_hint),
            write_strategy: WriteStrategy::SidecarOnly,
        }
    }

    fn skip(normalized_extension: String) -> Self {
        Self {
            normalized_extension,
            language_hint: None,
            write_strategy: WriteStrategy::Skip,
        }
    }
}

pub fn classify_extension(extension: &str) -> FileKind {
    let normalized = extension
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase();

    match normalized.as_str() {
        "json" => FileKind::sidecar_only(normalized.clone(), normalized),
        "go" | "java" | "py" | "ts" | "js" | "vue" | "sh" | "yaml" | "yml" | "xml"
        | "properties" | "tpl" => FileKind::annotate_in_place(normalized.clone(), normalized),
        _ => FileKind::skip(normalized),
    }
}

pub const BUILT_IN_EXCLUDED_DIRECTORIES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "dist",
    "build",
    "out",
    "target",
    "bin",
    "obj",
    ".cache",
    ".next",
    ".nuxt",
    ".idea",
    ".vscode",
    ".gradle",
    ".pnpm-store",
    ".yarn",
    ".turbo",
    ".vite",
    "vendor",
    "__pycache__",
    ".venv",
    "venv",
    "coverage",
    ".trellis",
    ".superpowers",
    ".worktrees",
    ".commenter-data",
    "gen",
];

pub fn scan_project_tree(
    project_root: &Path,
    include_extensions: &[String],
    exclude_directories: &[String],
) -> std::io::Result<Vec<ScannedFile>> {
    if !project_root.exists() {
        return Ok(Vec::new());
    }

    let normalized_includes = include_extensions
        .iter()
        .map(|value| normalize_extension(value))
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let mut normalized_excludes = exclude_directories
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    for builtin in BUILT_IN_EXCLUDED_DIRECTORIES {
        let candidate = builtin.to_ascii_lowercase();
        if !normalized_excludes.contains(&candidate) {
            normalized_excludes.push(candidate);
        }
    }

    let mut files = Vec::new();

    for entry in WalkDir::new(project_root)
        .into_iter()
        .filter_entry(|entry| !is_excluded_directory(entry.path(), &normalized_excludes))
    {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }

        let extension = entry
            .path()
            .extension()
            .and_then(|value| value.to_str())
            .map(normalize_extension)
            .unwrap_or_default();
        if !normalized_includes.is_empty() && !normalized_includes.contains(&extension) {
            continue;
        }

        let kind = classify_extension(&extension);
        if matches!(kind.write_strategy, WriteStrategy::Skip) {
            continue;
        }

        let relative_path = entry
            .path()
            .strip_prefix(project_root)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");

        files.push(ScannedFile {
            relative_path,
            absolute_path: entry.path().to_path_buf(),
            kind,
        });
    }

    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(files)
}

fn normalize_extension(value: &str) -> String {
    value.trim().trim_start_matches('.').to_ascii_lowercase()
}

fn is_excluded_directory(path: &Path, excluded_directories: &[String]) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy().to_ascii_lowercase();
        excluded_directories.contains(&name)
    })
}
