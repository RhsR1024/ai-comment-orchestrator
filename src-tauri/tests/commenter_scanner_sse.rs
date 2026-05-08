use ai_comment_orchestrator::commenter::{
    scanner::{classify_extension, WriteStrategy},
    sse::collect_sse_content,
};

#[test]
fn scanner_classifies_json_as_sidecar_only() {
    let kind = classify_extension("json");
    assert_eq!(kind.write_strategy, WriteStrategy::SidecarOnly);
}

#[test]
fn sse_parser_ignores_reasoning_and_collects_content() {
    let raw = concat!(
        "data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"忽略\"}}]}\n\n",
        "data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n\n",
        "data: [DONE]\n\n",
    );
    let output = collect_sse_content(raw.as_bytes()).expect("parse");
    assert_eq!(output, "你好");
}
