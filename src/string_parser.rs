/*
 * String Parser Utilities using Winnow
 *
 * High-performance string parsing utilities to handle various encoding issues,
 * escape sequences, and malformed string literals that can occur in configuration
 * files and CLI output formatting.
 *
 * Uses the winnow parser combinator library for optimal performance and
 * flexibility in handling complex string parsing scenarios.
 */

use std::collections::HashMap;
use winnow::{
    ascii::{digit1, space0},
    combinator::{alt, delimited, opt, preceded, repeat, terminated},
    token::{literal, none_of, take_while},
    PResult, Parser,
};

/// Safe string builder that handles encoding issues
#[derive(Debug, Clone, Default)]
pub struct SafeStringBuilder {
    content: String,
}

impl SafeStringBuilder {
    pub fn new() -> Self {
        Self {
            content: String::new(),
        }
    }

    /// Add a line with automatic newline handling
    pub fn line<S: AsRef<str>>(&mut self, text: S) -> &mut Self {
        self.content.push_str(text.as_ref());
        self.content.push('\n');
        self
    }

    /// Add text without newline
    pub fn text<S: AsRef<str>>(&mut self, text: S) -> &mut Self {
        self.content.push_str(text.as_ref());
        self
    }

    /// Add a formatted line
    pub fn formatted_line(&mut self, format: &str, _args: std::fmt::Arguments) -> &mut Self {
        use std::fmt::Write;
        let _ = write!(self.content, "{}\n", format);
        self
    }

    /// Build the final string
    pub fn build(self) -> String {
        self.content
    }

    /// Get current length
    pub fn len(&self) -> usize {
        self.content.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

/// Parse escape sequences in strings
pub fn parse_escape_sequence<'a>(input: &mut &'a str) -> PResult<char> {
    preceded(
        '\\',
        alt((
            'n'.map(|_| '\n'),
            't'.map(|_| '\t'),
            'r'.map(|_| '\r'),
            '\\'.map(|_| '\\'),
            '"'.map(|_| '"'),
            '\''.map(|_| '\''),
            '0'.map(|_| '\0'),
        )),
    )
    .parse_next(input)
}

/// Parse a quoted string with escape sequence support
pub fn parse_quoted_string<'a>(input: &mut &'a str) -> PResult<String> {
    delimited(
        '"',
        repeat(0.., alt((parse_escape_sequence, none_of('"')))),
        '"',
    )
    .parse_next(input)
    .map(|chars: Vec<char>| chars.into_iter().collect())
}

/// Parse configuration key-value pairs
pub fn parse_config_line<'a>(input: &mut &'a str) -> PResult<(String, String)> {
    let key = terminated(
        take_while(1.., |c: char| c.is_alphanumeric() || c == '_' || c == '-'),
        (space0, '=', space0),
    )
    .parse_next(input)?;

    let value = alt((
        parse_quoted_string,
        take_while(1.., |c: char| !c.is_whitespace() && c != '#').map(|s: &str| s.to_string()),
    ))
    .parse_next(input)?;

    Ok((key.to_string(), value))
}

/// Parse ISDN timer configuration format
pub fn parse_timer_config<'a>(input: &mut &'a str) -> PResult<HashMap<String, u32>> {
    let mut timers = HashMap::new();

    let entries: Vec<_> = repeat(
        0..,
        (
            take_while(1.., |c: char| c.is_alphanumeric() || c == '_'),
            (space0, '=', space0),
            digit1.map(|s: &str| s.parse::<u32>().unwrap_or(0)),
            opt((space0, '\n')),
        ),
    )
    .parse_next(input)?;

    for (key, _, value, _) in entries {
        timers.insert(String::from(key), value);
    }

    Ok(timers)
}

/// Parse ISDN configuration section
#[derive(Debug, Clone)]
pub struct IsdnConfigSection {
    pub name: String,
    pub properties: HashMap<String, String>,
}

pub fn parse_isdn_config_section<'a>(input: &mut &'a str) -> PResult<IsdnConfigSection> {
    let name = delimited('[', take_while(1.., |c: char| c != ']'), ']').parse_next(input)?;

    let entries: Vec<(String, String)> =
        repeat(0.., preceded(space0, parse_config_line)).parse_next(input)?;

    let properties: HashMap<String, String> = entries.into_iter().collect();

    Ok(IsdnConfigSection {
        name: name.to_string(),
        properties,
    })
}

/// Safe string formatting that avoids encoding issues
pub fn safe_format(template: &str, replacements: &HashMap<&str, &str>) -> String {
    let mut result = template.to_string();

    for (key, value) in replacements {
        let placeholder = format!("{{{}}}", key);
        result = result.replace(&placeholder, value);
    }

    result
}

/// Validate and clean string content
pub fn sanitize_string(input: &str) -> String {
    input
        .chars()
        .filter(|&c| c.is_ascii_graphic() || c.is_ascii_whitespace())
        .collect()
}

/// Parse codec type from string safely  
pub fn parse_codec_type<'a>(input: &mut &'a str) -> PResult<String> {
    alt((
        literal("u-Law").value("uLaw".to_string()),
        literal("μ-Law").value("uLaw".to_string()),
        literal("mu-Law").value("uLaw".to_string()),
        literal("A-Law").value("ALaw".to_string()),
        literal("a-Law").value("ALaw".to_string()),
        literal("G.711").value("G711".to_string()),
        literal("G.729").value("G729".to_string()),
        // Fallback for unknown codecs
        take_while(1.., |c: char| c.is_alphanumeric() || c == '.' || c == '-')
            .map(|s: &str| s.to_string()),
    ))
    .parse_next(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_safe_string_builder() {
        let mut builder = SafeStringBuilder::new();
        builder
            .line("First line") // "First line\n"
            .text("Second ") // "First line\nSecond "
            .text("part"); // "First line\nSecond part"

        // Just test the basic functionality for now
        let result = builder.build();
        assert_eq!(result, "First line\nSecond part");
    }

    #[test]
    fn test_parse_quoted_string() {
        let mut input = r#""Hello, \"world\"!""#;
        let result = parse_quoted_string(&mut input).unwrap();
        assert_eq!(result, r#"Hello, "world"!"#);
    }

    #[test]
    fn test_parse_config_line() {
        let mut input = "timeout = 5000";
        let (key, value) = parse_config_line(&mut input).unwrap();
        assert_eq!(key, "timeout");
        assert_eq!(value, "5000");
    }

    #[test]
    fn test_parse_codec_type() {
        let test_cases = [
            ("μ-Law", "uLaw"),
            ("u-Law", "uLaw"),
            ("A-Law", "ALaw"),
            ("G.711", "G711"),
        ];

        for (input, expected) in test_cases {
            let mut test_input = input;
            let result = parse_codec_type(&mut test_input).unwrap();
            assert_eq!(result, expected);
        }
    }

    #[test]
    fn test_safe_format() {
        let template = "Hello {name}, you have {count} messages.";
        let mut replacements = HashMap::new();
        replacements.insert("name", "Alice");
        replacements.insert("count", "3");

        let result = safe_format(template, &replacements);
        assert_eq!(result, "Hello Alice, you have 3 messages.");
    }

    #[test]
    fn test_sanitize_string() {
        let input = "Hello\x00World\x7F!";
        let result = sanitize_string(input);
        assert_eq!(result, "HelloWorld!");
    }
}
