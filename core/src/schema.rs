use crate::ConversationEntry;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, PartialEq)]
pub enum SchemaError {
    InvalidJson { line: usize, message: String },
    MissingField { line: usize, field: String },
    InvalidFormat { line: usize, expected: String },
}

impl core::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SchemaError::InvalidJson { line, message } => {
                write!(f, "Invalid JSON at line {}: {}", line, message)
            }
            SchemaError::MissingField { line, field } => {
                write!(f, "Missing field '{}' at line {}", field, line)
            }
            SchemaError::InvalidFormat { line, expected } => {
                write!(f, "Invalid format at line {}: expected {}", line, expected)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SchemaError {}

pub struct JsonlParser;

impl JsonlParser {
    pub fn parse(content: &str) -> Result<Vec<ConversationEntry>, SchemaError> {
        let mut entries = Vec::new();

        for (idx, line) in content.lines().enumerate() {
            let line_num = idx + 1;

            if line.trim().is_empty() {
                continue;
            }

            let entry = Self::parse_line(line, line_num)?;
            entries.push(entry);
        }

        Ok(entries)
    }

    fn parse_line(line: &str, line_num: usize) -> Result<ConversationEntry, SchemaError> {
        let value: serde_json::Value =
            serde_json::from_str(line).map_err(|e| SchemaError::InvalidJson {
                line: line_num,
                message: format!("{}", e),
            })?;

        let obj = value
            .as_object()
            .ok_or_else(|| SchemaError::InvalidFormat {
                line: line_num,
                expected: "JSON object".to_string(),
            })?;

        if !obj.contains_key("messages") {
            return Err(SchemaError::MissingField {
                line: line_num,
                field: "messages".to_string(),
            });
        }

        let entry: ConversationEntry =
            serde_json::from_str(line).map_err(|e| SchemaError::InvalidJson {
                line: line_num,
                message: format!("{}", e),
            })?;

        validate_conversation(&entry, line_num)?;

        Ok(entry)
    }
}

pub fn validate_conversation(
    entry: &ConversationEntry,
    line_num: usize,
) -> Result<(), SchemaError> {
    if entry.messages.is_empty() {
        return Err(SchemaError::InvalidFormat {
            line: line_num,
            expected: "at least 1 message".to_string(),
        });
    }

    if entry.messages.len() < 2 {
        return Err(SchemaError::InvalidFormat {
            line: line_num,
            expected: "at least 2 messages (minimum 1 turn)".to_string(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonl_valid() {
        let content = r#"{"messages":[{"role":"user","content":"Hello"},{"role":"assistant","content":"Hi there!"}]}
{"messages":[{"role":"user","content":"What is Rust?"},{"role":"assistant","content":"A systems programming language."}]}
{"messages":[{"role":"user","content":"How are you?"},{"role":"assistant","content":"I'm doing well!"}]}
{"messages":[{"role":"user","content":"Tell me a joke"},{"role":"assistant","content":"Why did the chicken cross the road?"}]}
{"messages":[{"role":"user","content":"Goodbye"},{"role":"assistant","content":"See you later!"}]}"#;

        let result = JsonlParser::parse(content);
        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn test_jsonl_invalid_json() {
        let content = r#"{"messages":[{"role":"user","content":"Hello"},{"role":"assistant","content":"Hi!"}]}
{invalid json here}"#;

        let result = JsonlParser::parse(content);
        assert!(result.is_err());
        match result {
            Err(SchemaError::InvalidJson { line, .. }) => {
                assert_eq!(line, 2);
            }
            _ => panic!("Expected InvalidJson error"),
        }
    }

    #[test]
    fn test_jsonl_missing_field() {
        let content = r#"{"messages":[{"role":"user","content":"Hello"},{"role":"assistant","content":"Hi!"}]}
{"other_field": "value"}"#;

        let result = JsonlParser::parse(content);
        assert!(result.is_err());
        match result {
            Err(SchemaError::MissingField { line, field }) => {
                assert_eq!(line, 2);
                assert_eq!(field, "messages");
            }
            _ => panic!("Expected MissingField error"),
        }
    }

    #[test]
    fn test_jsonl_invalid_format_single_message() {
        let content = r#"{"messages":[{"role":"user","content":"Hello"}]}"#;

        let result = JsonlParser::parse(content);
        assert!(result.is_err());
        match result {
            Err(SchemaError::InvalidFormat { line, expected }) => {
                assert_eq!(line, 1);
                assert!(expected.contains("2 messages"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_jsonl_empty_messages() {
        let content = r#"{"messages":[]}"#;

        let result = JsonlParser::parse(content);
        assert!(result.is_err());
        match result {
            Err(SchemaError::InvalidFormat { line, expected }) => {
                assert_eq!(line, 1);
                assert!(expected.contains("1 message"));
            }
            _ => panic!("Expected InvalidFormat error"),
        }
    }

    #[test]
    fn test_conversation_with_function_call() {
        let content = r#"{"messages":[{"role":"user","content":"What's the weather?"},{"role":"assistant","content":"Let me check.","function_call":{"name":"get_weather","arguments":"{}"}},{"role":"function","content":"Sunny"}]}"#;

        let result = JsonlParser::parse(content);
        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].messages.len(), 3);
        assert!(entries[0].messages[1].function_call.is_some());
    }

    #[test]
    fn test_jsonl_skip_empty_lines() {
        let content = r#"{"messages":[{"role":"user","content":"Hello"},{"role":"assistant","content":"Hi!"}]}

{"messages":[{"role":"user","content":"Goodbye"},{"role":"assistant","content":"Bye!"}]}"#;

        let result = JsonlParser::parse(content);
        assert!(result.is_ok());
        let entries = result.unwrap();
        assert_eq!(entries.len(), 2);
    }
}
