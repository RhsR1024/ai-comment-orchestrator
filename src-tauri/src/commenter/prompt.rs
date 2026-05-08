pub const FALLBACK_SYSTEM_PROMPT: &str = "你是一个为代码添加详细中文注释的助手。\n\
严格要求：\n\
1. 不要修改任何业务逻辑、函数语义或重要结构。\n\
2. 必须返回完整的源码内容，不能截断、删行或重排函数。\n\
3. 所有注释必须使用中文。\n\
4. 不要在响应中输出 Markdown 代码围栏（```）或任何解释性文字。\n\
5. 注释要解释为什么而不仅仅是是什么，覆盖参数、返回值、边界情况。\n\
6. 遵循目标语言的注释规范。";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnnotationPromptParts {
    pub system: String,
    pub user: String,
}

pub fn build_annotation_prompt(
    prompt_template: &str,
    language_hint: &str,
    relative_path: &str,
    source_text: &str,
) -> AnnotationPromptParts {
    let system = if prompt_template.trim().is_empty() {
        FALLBACK_SYSTEM_PROMPT.to_string()
    } else {
        prompt_template.to_string()
    };

    let language_label = if language_hint.trim().is_empty() {
        "源代码".to_string()
    } else {
        language_hint.to_string()
    };

    let user = format!(
        "请按上面的中文注释规范，为下面这个 {language} 文件添加详细中文注释。\n\
要求：\n\
- 直接返回完整文件内容，不要包裹 ``` 围栏。\n\
- 保留原有缩进、换行、空行、字符串内容、import 顺序。\n\
- 只新增注释，不要删改任何业务代码。\n\
- 当文件已有少量注释时，可以补全到详细程度，但不要替换或删除原注释。\n\n\
文件路径: {path}\n\n\
=== BEGIN SOURCE ===\n{source}\n=== END SOURCE ===\n",
        language = language_label,
        path = relative_path,
        source = source_text,
    );

    AnnotationPromptParts { system, user }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_default_system_when_template_blank() {
        let parts = build_annotation_prompt("   ", "go", "src/main.go", "package main");
        assert_eq!(parts.system, FALLBACK_SYSTEM_PROMPT);
        assert!(parts.user.contains("src/main.go"));
        assert!(parts.user.contains("package main"));
    }

    #[test]
    fn uses_provided_template_as_system_message() {
        let parts = build_annotation_prompt(
            "# 自定义注释规范\n使用中文",
            "ts",
            "src/index.ts",
            "console.log(1)",
        );
        assert!(parts.system.starts_with("# 自定义注释规范"));
        assert!(parts.user.contains("src/index.ts"));
        assert!(parts.user.contains("console.log(1)"));
    }
}
