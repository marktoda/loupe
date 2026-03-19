#[test]
fn parse_minimal_session_fixture() {
    let content = std::fs::read_to_string("tests/fixtures/minimal_session.jsonl").unwrap();
    let mut items = Vec::new();
    let mut result = None;

    for line in content.lines() {
        if let loupe::parser::ParseResult::Parsed(new_items, meta) = loupe::parser::parse_line(line)
        {
            items.extend(new_items);
            if let Some(r) = meta.session_result {
                result = Some(r);
            }
        }
    }

    assert!(!items.is_empty(), "Should have parsed some items");
    assert!(
        items
            .iter()
            .any(|i| matches!(i, loupe::run::TranscriptItem::SessionStart { .. })),
        "Should have SessionStart"
    );
    assert!(
        items
            .iter()
            .any(|i| matches!(i, loupe::run::TranscriptItem::AssistantText { .. })),
        "Should have AssistantText"
    );
    assert!(
        items
            .iter()
            .any(|i| matches!(i, loupe::run::TranscriptItem::ToolUse { .. })),
        "Should have ToolUse"
    );
    assert!(result.is_some(), "Should have a session result");
    let result = result.unwrap();
    assert_eq!(result.subtype, "success");
    assert!(!result.is_error);
    assert!(result.total_cost_usd > 0.0);
}

#[test]
fn parse_malformed_lines_dont_panic() {
    let lines = vec![
        "",
        "not json",
        "{}",
        r#"{"type":"unknown_future_type"}"#,
        r#"{"type":"system","subtype":"unknown_subtype"}"#,
        r#"{"type":"assistant","message":{}}"#,
        r#"{"type":"result"}"#,
    ];

    for line in lines {
        // Should never panic — returns Skipped or Error
        let _ = loupe::parser::parse_line(line);
    }
}
