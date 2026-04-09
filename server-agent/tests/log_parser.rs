use std::collections::BTreeMap;

use server_agent::{LogEnvelope, LogParser, ParseRule, ParseRuleKind};

fn make_rule(id: &str, pattern: &str, event_type: &str, severity: &str) -> ParseRule {
    ParseRule {
        id: id.to_string(),
        kind: ParseRuleKind::Regex,
        pattern: pattern.to_string(),
        event_type: event_type.to_string(),
        severity: severity.to_string(),
    }
}

fn make_envelope(raw_line: &str) -> LogEnvelope {
    LogEnvelope {
        agent_id: "agent-1".to_string(),
        source: "server".to_string(),
        cursor: "cursor-1".to_string(),
        line_number: 7,
        raw_line: raw_line.to_string(),
        observed_at: "1710000000".to_string(),
    }
}

#[test]
fn regex_rule_match_builds_structured_payload() {
    let parser = LogParser::new(vec![make_rule(
        "chat-line",
        r"^\[(?P<time>\d{2}:\d{2})\] \[Chat\] (?P<player>[^:]+): (?P<message>.+)$",
        "chat",
        "info",
    )])
    .expect("create parser");

    let event = parser
        .parse(&make_envelope("[19:42] [Chat] RiverFox: hold north"))
        .expect("rule should match");

    let mut expected_payload = BTreeMap::new();
    expected_payload.insert("time".to_string(), "19:42".to_string());
    expected_payload.insert("player".to_string(), "RiverFox".to_string());
    expected_payload.insert("message".to_string(), "hold north".to_string());

    assert_eq!(event.rule_id, "chat-line");
    assert_eq!(event.event_type, "chat");
    assert_eq!(event.severity, "info");
    assert_eq!(event.payload, expected_payload);
}

#[test]
fn returns_none_when_no_rule_matches() {
    let parser = LogParser::new(vec![make_rule(
        "chat-line",
        r"^\[Chat\] (?P<player>[^:]+): (?P<message>.+)$",
        "chat",
        "info",
    )])
    .expect("create parser");

    assert!(parser.parse(&make_envelope("server boot complete")).is_none());
}

#[test]
fn replacing_rules_takes_effect_immediately() {
    let mut parser = LogParser::new(vec![make_rule(
        "chat-line",
        r"^\[Chat\] (?P<player>[^:]+): (?P<message>.+)$",
        "chat",
        "info",
    )])
    .expect("create parser");

    assert!(parser.parse(&make_envelope("[Kill] Fox -> Mint")).is_none());

    parser
        .replace_rules(vec![make_rule(
            "kill-line",
            r"^\[Kill\] (?P<attacker>[^ ]+) -> (?P<victim>.+)$",
            "kill",
            "warn",
        )])
        .expect("replace rules");

    let event = parser
        .parse(&make_envelope("[Kill] Fox -> Mint"))
        .expect("new rule should match");

    assert_eq!(event.rule_id, "kill-line");
    assert_eq!(event.event_type, "kill");
    assert_eq!(event.severity, "warn");
    assert_eq!(event.payload.get("attacker").map(String::as_str), Some("Fox"));
    assert_eq!(event.payload.get("victim").map(String::as_str), Some("Mint"));
}
