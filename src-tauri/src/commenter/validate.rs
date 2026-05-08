#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileValidationInput {
    pub file_name: String,
    pub extension: String,
    pub original_source: String,
}

impl FileValidationInput {
    pub fn source(file_name: &str, original_source: &str) -> Self {
        let extension = file_name
            .rsplit_once('.')
            .map(|(_, extension)| extension.to_ascii_lowercase())
            .unwrap_or_default();

        Self {
            file_name: file_name.to_string(),
            extension,
            original_source: original_source.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationDecision {
    Accept,
    Reject(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    pub decision: ValidationDecision,
}

impl ValidationResult {
    fn accept() -> Self {
        Self {
            decision: ValidationDecision::Accept,
        }
    }

    fn reject(reason: &str) -> Self {
        Self {
            decision: ValidationDecision::Reject(reason.to_string()),
        }
    }
}

const MIN_LENGTH_RATIO: f32 = 0.6;
const MAX_LENGTH_RATIO: f32 = 5.0;
const MIN_LINE_RATIO: f32 = 0.6;
const MAX_LINE_RATIO: f32 = 5.0;

pub fn validate_candidate(input: FileValidationInput, candidate: &str) -> ValidationResult {
    let trimmed = candidate.trim();
    if trimmed.is_empty() {
        return ValidationResult::reject("候选结果为空");
    }

    if contains_markdown_fence(candidate) {
        return ValidationResult::reject("候选结果包含 Markdown 围栏 (```)");
    }

    if contains_obvious_explanation_prefix(candidate) {
        return ValidationResult::reject("候选结果疑似包含模型解释性前言");
    }

    let original_len = input.original_source.chars().count().max(1) as f32;
    let candidate_len = candidate.chars().count() as f32;
    let length_ratio = candidate_len / original_len;
    if length_ratio < MIN_LENGTH_RATIO {
        return ValidationResult::reject(&format!(
            "候选结果字符数明显缩短 (ratio={:.2})，疑似截断或丢失代码",
            length_ratio
        ));
    }
    if length_ratio > MAX_LENGTH_RATIO {
        return ValidationResult::reject(&format!(
            "候选结果字符数膨胀过多 (ratio={:.2})，疑似插入了大量额外内容",
            length_ratio
        ));
    }

    let original_lines = input.original_source.lines().count().max(1) as f32;
    let candidate_lines = candidate.lines().count() as f32;
    let line_ratio = candidate_lines / original_lines;
    if line_ratio < MIN_LINE_RATIO {
        return ValidationResult::reject(&format!(
            "候选结果行数明显缩短 (ratio={:.2})，疑似删除代码",
            line_ratio
        ));
    }
    if line_ratio > MAX_LINE_RATIO {
        return ValidationResult::reject(&format!(
            "候选结果行数膨胀过多 (ratio={:.2})",
            line_ratio
        ));
    }

    if let Some(reason) = check_language_specific(&input, candidate) {
        return ValidationResult::reject(&reason);
    }

    ValidationResult::accept()
}

fn contains_markdown_fence(candidate: &str) -> bool {
    candidate.contains("```")
}

fn contains_obvious_explanation_prefix(candidate: &str) -> bool {
    let head: String = candidate.chars().take(80).collect();
    let head_lower = head.to_ascii_lowercase();
    head_lower.starts_with("好的")
        || head_lower.starts_with("以下是")
        || head_lower.starts_with("here is")
        || head_lower.starts_with("here's")
        || head_lower.starts_with("sure,")
}

fn check_language_specific(input: &FileValidationInput, candidate: &str) -> Option<String> {
    match input.extension.as_str() {
        "go" => {
            if input.original_source.contains("package ") && !candidate.contains("package ") {
                return Some("Go 候选结果缺少 package 声明".to_string());
            }
        }
        "java" => {
            if input.original_source.contains("class ")
                && !candidate.contains("class ")
                && !candidate.contains("interface ")
                && !candidate.contains("enum ")
            {
                return Some("Java 候选结果缺少 class/interface/enum 声明".to_string());
            }
        }
        "py" => {
            if input.original_source.contains("def ")
                && !candidate.contains("def ")
                && !candidate.contains("class ")
            {
                return Some("Python 候选结果缺少 def/class 声明".to_string());
            }
        }
        "ts" | "js" | "vue" => {
            let original_braces = input.original_source.matches('{').count();
            let candidate_braces = candidate.matches('{').count();
            if original_braces > 4 && candidate_braces * 2 < original_braces {
                return Some("脚本候选结果花括号数量异常减少".to_string());
            }
        }
        _ => {}
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_markdown_fence() {
        let input = FileValidationInput::source("main.go", "package main\nfunc main() {}\n");
        let result = validate_candidate(input, "```go\npackage main\nfunc main() {}\n```");
        assert!(matches!(result.decision, ValidationDecision::Reject(_)));
    }

    #[test]
    fn rejects_excessive_shrinkage() {
        let original = "package main\n\nfunc one() {}\nfunc two() {}\nfunc three() {}\nfunc four() {}\nfunc five() {}\n";
        let input = FileValidationInput::source("main.go", original);
        let result = validate_candidate(input, "package main\n");
        assert!(matches!(result.decision, ValidationDecision::Reject(_)));
    }

    #[test]
    fn accepts_reasonable_annotation() {
        let original = "package main\n\nfunc main() { println(\"hi\") }\n";
        let annotated = "// Package main 程序入口\npackage main\n\n// main 程序入口函数\nfunc main() { println(\"hi\") }\n";
        let input = FileValidationInput::source("main.go", original);
        let result = validate_candidate(input, annotated);
        assert_eq!(result.decision, ValidationDecision::Accept);
    }

    #[test]
    fn rejects_explanatory_prefix() {
        let original = "package main\nfunc main(){}\n";
        let candidate = "好的，下面是为你添加注释后的版本：\npackage main\nfunc main(){}\n";
        let input = FileValidationInput::source("main.go", original);
        let result = validate_candidate(input, candidate);
        assert!(matches!(result.decision, ValidationDecision::Reject(_)));
    }
}
