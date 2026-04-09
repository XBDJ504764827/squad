use regex::Regex;

use crate::models::{
    AgentError, LogEnvelope, ParseRule, ParseRuleKind, ParsedLogEvent,
};

#[derive(Debug)]
struct CompiledRule {
    rule: ParseRule,
    regex: Regex,
}

#[derive(Debug, Default)]
pub struct LogParser {
    rules: Vec<CompiledRule>,
}

impl LogParser {
    pub fn new(rules: Vec<ParseRule>) -> Result<Self, AgentError> {
        Ok(Self {
            rules: Self::compile_rules(rules)?,
        })
    }

    pub fn replace_rules(&mut self, rules: Vec<ParseRule>) -> Result<(), AgentError> {
        self.rules = Self::compile_rules(rules)?;
        Ok(())
    }

    pub fn parse(&self, envelope: &LogEnvelope) -> Option<ParsedLogEvent> {
        for compiled_rule in &self.rules {
            let captures = compiled_rule.regex.captures(&envelope.raw_line)?;
            let mut payload = std::collections::BTreeMap::new();

            for name in compiled_rule.regex.capture_names().flatten() {
                if let Some(value) = captures.name(name) {
                    payload.insert(name.to_string(), value.as_str().to_string());
                }
            }

            return Some(ParsedLogEvent {
                agent_id: envelope.agent_id.clone(),
                rule_id: compiled_rule.rule.id.clone(),
                event_type: compiled_rule.rule.event_type.clone(),
                severity: compiled_rule.rule.severity.clone(),
                source: envelope.source.clone(),
                cursor: envelope.cursor.clone(),
                line_number: envelope.line_number,
                raw_line: envelope.raw_line.clone(),
                observed_at: envelope.observed_at.clone(),
                payload,
            });
        }

        None
    }

    fn compile_rules(rules: Vec<ParseRule>) -> Result<Vec<CompiledRule>, AgentError> {
        rules.into_iter()
            .map(|rule| {
                let regex = match rule.kind {
                    ParseRuleKind::Regex => Regex::new(&rule.pattern).map_err(|err| {
                        AgentError::InvalidParseRule {
                            rule_id: rule.id.clone(),
                            message: err.to_string(),
                        }
                    })?,
                };

                Ok(CompiledRule { rule, regex })
            })
            .collect()
    }
}
