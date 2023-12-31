use super::types::*;
use std::collections::HashMap;
use once_cell::sync::Lazy;

// Maps escaped characters to their decoded value.
static ESCAPED_CHARS_MAP: Lazy<HashMap<char, char>> = Lazy::new(|| {
    let mut map = HashMap::new();
    map.insert('b', '\u{0008}');
    map.insert('f', '\u{000c}');
    map.insert('n', '\n');
    map.insert('r', '\r');
    map.insert('t', '\t');
    map.insert('\\', '\\');
    map
});

// Consumes a sequence of whitespace characters.
#[derive(Debug)]
struct WhitespaceConsumer;

impl Consumer for WhitespaceConsumer {
    fn try_consume(input: &[char], start: usize) -> ParserResult<Self> {
        let literal: String = input[start..].iter().take_while(|c| c.is_whitespace()).collect();

        if literal.is_empty() {
            return Ok(None);
        }

        Ok(Some(Token {
            length: literal.len(),
            literal,
            start,
            token: WhitespaceConsumer
        }))
    }
}

// Consumes a single escaped character, e.g. "\x". Doesn't concern itself
// with whether its a valid escape sequence or not, just that it's a \ followed by another character.
#[derive(Debug)]
struct EscapedCharacterConsumer {
    decoded: char,
}

impl Consumer for EscapedCharacterConsumer {
    fn try_consume(input: &[char], start: usize) -> ParserResult<Self> {
        if input[start] != '\\' {
            return Ok(None);
        }

        let literal;
        let decoded: char;
        if !has_available_chars(input, start, 2) {
            return Err(ParserError::new("Invalid escape sequence", start));
        }

        let next = &input[start + 1];
        if ESCAPED_CHARS_MAP.contains_key(next) {
            literal = input[start..start + 2].iter().collect::<String>();
            decoded = *ESCAPED_CHARS_MAP.get(next).unwrap();
        } else if let Some(token) = HexCharacterConsumer::try_consume(input, start)? {
            literal = token.literal;
            decoded = token.token.decoded;
        } else {
            return Ok(None);
        }

        Ok(Some(Token {
            length: literal.len(),
            literal,
            start,
            token: EscapedCharacterConsumer {
                decoded
            }
        }))
    }
}

// Consumes a single hex character, e.g. "\u1234", i.e. a \ followed by a u followed by 2-4 hex characters.
#[derive(Debug)]
struct HexCharacterConsumer {
    decoded: char
}

impl Consumer for HexCharacterConsumer {
    fn try_consume(input: &[char], start: usize) -> ParserResult<Self> {
        let mut literal = String::from("\\u");
        let mut length = 2;
        let mut encoded_char = String::new();

        if !has_available_chars(input, start, 4) || input[start] != '\\' || input[start + 1] != 'u' {
            return Ok(None);
        }

        let end = if start + 6 < input.len() { start + 6 } else { input.len() };
        for c in input[start + 2..end].iter().take_while(|c| c.is_ascii_hexdigit()) {
            literal.push(*c);
            encoded_char.push(*c);
            length += 1;
        }

        let c = u32::from_str_radix(&encoded_char, 16).unwrap_or_else( |_| panic!("BUG: Invalid hex character: {}", encoded_char));
        let char = std::char::from_u32(c).unwrap_or_else(|| panic!("BUG: Invalid hex character from u32: {}", c));

        Ok(Some(Token {
            literal,
            start,
            length,
            token: HexCharacterConsumer {
                decoded: char
            }
        }))
    }
}

// Consumes a single unescaped character, e.g. "a", "1", etc, but _not_ the start of an escape or a quote - '\' or '"'.
#[derive(Debug)]
struct UnescapedCharacterConsumer<const QUOTE: char> {
    decoded: char,
}

impl<const QUOTE: char> Consumer for UnescapedCharacterConsumer<QUOTE> {
    fn try_consume(input: &[char], start: usize) -> ParserResult<Self> {
        if input[start] == '\\' || input[start] == QUOTE {
            return Ok(None);
        }

        Ok(Some(Token {
            literal: input[start..start + 1].iter().collect::<String>(),
            start,
            length: 1,
            token: UnescapedCharacterConsumer {
                decoded: input[start]
            }
        }))
    }
}

// Consumes a single escaped character, e.g. "\x", "\u1234", etc, returning an error if the escape sequence is invalid.
#[derive(Debug)]
struct EscapedStringCharConsumer<const QUOTE: char> {
    decoded: char,
}

impl<const QUOTE: char> Consumer for EscapedStringCharConsumer<QUOTE> {
    fn try_consume(input: &[char], start: usize) -> ParserResult<Self> {
        if let Some(token) = EscapedCharacterConsumer::try_consume(input, start)? {
            Ok(Some(Token {
                literal: token.literal,
                start,
                length: token.length,
                token: EscapedStringCharConsumer {
                    decoded: token.token.decoded
                }
            }))
        } else if has_available_chars(input, start, 2) && input[start] == '\\' {
            if input[start + 1] == QUOTE {
                return Ok(Some(Token {
                    literal: input[start..start + 2].iter().collect::<String>(),
                    start,
                    length: 2,
                    token: EscapedStringCharConsumer {
                        decoded: QUOTE
                    }
                }));
            } else {
                return Err(ParserError::new(&format!("Invalid escape sequence: \\{}", input[start+1]), start));
            }
        } else {
            Ok(None)
        }
    }
}

// Consumes a string surrounded by the given quotes, with escapes. e.g. "hello world", 'hello world', etc.
#[derive(Debug)]
pub struct QuotedStringConsumer<const QUOTE:char> {
    pub decoded: String
}

impl<const QUOTE: char> Consumer for QuotedStringConsumer<QUOTE> {
    fn try_consume(input: &[char], start: usize) -> ParserResult<Self> {
        if !has_available_chars(input, start, 2) || input[start] != QUOTE {
            return Ok(None);
        }

        let mut literal = String::from(QUOTE);
        let mut decoded = String::new();
        let mut length = 1;

        while start + length < input.len() {
            if let Some(token) = UnescapedCharacterConsumer::<QUOTE>::try_consume(input, start + length)? {
                literal.push_str(&token.literal);
                decoded.push(token.token.decoded);
                length += token.length;
            } else if let Some(token) = EscapedStringCharConsumer::<QUOTE>::try_consume(input, start + length)? {
                literal.push_str(&token.literal);
                decoded.push(token.token.decoded);
                length += token.length;
            } else {
                break;
            }
        }

        if has_available_chars(input, start + length, 1) && input[start + length] == QUOTE {
            literal.push(QUOTE);
            length += 1;
        } else {
            return Err(ParserError::new(&format!("Expected closing quote: {}", QUOTE), start));
        }

        Ok(Some(Token {
            literal,
            start,
            length,
            token: QuotedStringConsumer {
                decoded
            }
        }))
    }
}

// Consumes a single quoted string, with escapes. e.g. 'hello world', 'foo\\', etc.
pub type SingleQuotedStringConsumer = QuotedStringConsumer<'\''>;

// Consumes a double quoted string, with escapes. e.g. "hello world", "foo\\", etc.
pub type DoubleQuotedStringConsumer = QuotedStringConsumer<'"'>;

// Consumes a single character that is not whitespace, a quote, or a backslash.
#[derive(Debug)]
struct UnquotedCharacterConsumer {
    decoded: char,
}

impl Consumer for UnquotedCharacterConsumer {
    fn try_consume(input: &[char], start: usize) -> ParserResult<Self> {
        let c = &input[start];
        if c.is_whitespace() || c == &'\'' || c == &'"' || c == &'\\' {
            return Ok(None);
        }

        Ok(Some(Token {
            literal: String::from(*c),
            start,
            length: 1,
            token: UnquotedCharacterConsumer {
                decoded: *c
            }
        }))
    }
}

// Consumes a string that is not surrounded by quotes, e.g. hello world, foo\\, etc.
#[derive(Debug)]
pub struct UnquotedStringConsumer {
    pub decoded: String
}

impl Consumer for UnquotedStringConsumer {
    fn try_consume(input: &[char], start: usize) -> ParserResult<Self> {
        let mut literal = String::new();
        let mut decoded = String::new();
        let mut length = 0;

        while start + length < input.len() {
            if let Some(token) = UnquotedCharacterConsumer::try_consume(input, start + length)? {
                literal.push_str(&token.literal);
                decoded.push(token.token.decoded);
                length += token.length;
            } else if let Some(token) = EscapedCharacterConsumer::try_consume(input, start + length)? {
                literal.push_str(&token.literal);
                decoded.push(token.token.decoded);
                length += token.length;
            } else {
                break;
            }
        }

        if length > 0 {
            Ok(Some(Token {
                literal,
                start,
                length,
                token: UnquotedStringConsumer {
                    decoded
                }
            }))
        } else {
            Ok(None)
        }
    }
}

// Consumes a string that is either quoted or unquoted.
#[derive(Debug, PartialEq)]
pub enum QuotedOrUnquotedStringConsumer {
    SingleQuoted,
    DoubleQuoted,
    Unquoted
}

impl Consumer for QuotedOrUnquotedStringConsumer {
    fn try_consume(input: &[char], start: usize) -> ParserResult<Self> {
        if let Some(token) = SingleQuotedStringConsumer::try_consume(input, start)?  {
            return Ok(Some(Token {
                literal: token.literal,
                start,
                length: token.length,
                token: QuotedOrUnquotedStringConsumer::SingleQuoted
            }));
        } else if let Some(token) = DoubleQuotedStringConsumer::try_consume(input, start)? {
            return Ok(Some(Token {
                literal: token.literal,
                start,
                length: token.length,
                token: QuotedOrUnquotedStringConsumer::DoubleQuoted
            }));
        } else if let Some(token) = UnquotedStringConsumer::try_consume(input, start)? {
            return Ok(Some(Token {
                literal: token.literal,
                start,
                length: token.length,
                token: QuotedOrUnquotedStringConsumer::Unquoted
            }));
        }

        Ok(None)
    }
}

// Consumes a string that is made up of component strings, each of which is either quoted or unquoted.
// e.g. "hello world"foo'bar' would be parsed into 3 parts: "hello world", foo, and 'bar'.
#[derive(Debug, PartialEq)]
pub struct CombinedStringConsumer {
    parts: Vec<Token<QuotedOrUnquotedStringConsumer>>
}

impl Consumer for CombinedStringConsumer {
    fn try_consume(input: &[char], start: usize) -> ParserResult<Self> {
        let mut parts = Vec::new();
        let mut length = 0;

        while start + length < input.len() {
            if let Some(token) = QuotedOrUnquotedStringConsumer::try_consume(input, start + length)? {
                length += token.length;
                parts.push(token);
            } else {
                break;
            }
        }

        if parts.is_empty() {
            return Ok(None);
        }

        Ok(Some(Token {
            literal: parts.iter().map(|p| p.literal.clone()).collect::<String>(),
            start,
            length,
            token: CombinedStringConsumer {
                parts
            }
        }))
    }
}

// Consumes a string that is made up of component strings. e.g. "/bin/sh -c 'echo hello world'" would be parsed into 3 parts: "/bin/sh", "-c", and "'echo hello world'".
#[derive(Debug, PartialEq)]
pub struct ExpressionConsumer {
    pub parts: Vec<Token<CombinedStringConsumer>>
}

impl Consumer for ExpressionConsumer {
    fn try_consume(input: &[char], start: usize) -> ParserResult<Self> {
        let mut literal = String::new();
        let mut parts = Vec::new();
        let mut length = 0;

        while start + length < input.len() {
            if let Some(c) = WhitespaceConsumer::try_consume(input, start + length)? {
                literal.push_str(&c.literal);
                length += c.length;
            } else if let Some(token) = CombinedStringConsumer::try_consume(input, start + length)? {
                literal += &token.literal;
                length += token.length;
                parts.push(token);
            } else {
                break;
            }
        }

        if parts.is_empty() {
            return Ok(None);
        }

        Ok(Some(Token {
            literal,
            start,
            length,
            token: ExpressionConsumer {
                parts
            }
        }))
    }
}

fn has_available_chars(input: &[char], start: usize, len: usize) -> bool {
    start + len <= input.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_whitespace_consumer() {
        let input = "   \t\t\n";
        let chars = input.chars().collect::<Vec<char>>();
        let token = WhitespaceConsumer::try_consume(&chars, 0).unwrap().unwrap();
        assert_eq!(token.literal, "   \t\t\n");
        assert_eq!(token.start, 0);
        assert_eq!(token.length, 6);
    }

    #[test]
    fn test_escaped_character_consumer() {
        let input = "\\u1234";
        let chars = input.chars().collect::<Vec<char>>();
        let token = EscapedCharacterConsumer::try_consume(&chars, 0).unwrap().unwrap();
        assert_eq!(token.literal, "\\u1234");
        assert_eq!(token.start, 0);
        assert_eq!(token.length, 6);
    }

    #[test]
    fn test_hex_character_consumer() {
        let hex_chars = &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'A', 'B','C', 'D', 'E', 'F'];
        for c in hex_chars {
            let input = format!("\\u{}{}", c, c);
            let chars = input.chars().collect::<Vec<char>>();
            let token = HexCharacterConsumer::try_consume(&chars, 0).unwrap().unwrap();
            assert_eq!(token.literal, input, "Failed for {}", c);
            assert_eq!(token.start, 0, "Failed for {}", c);
            assert_eq!(token.length, 4, "Failed for {}", c);
        }

        let token = HexCharacterConsumer::try_consume(&['\\', 'u', '1', '2', 'b', 'd'], 0).unwrap().unwrap();
        assert_eq!(token.literal, "\\u12bd");
        assert_eq!(token.token.decoded, '\u{12bd}');
    }

    #[test]
    fn test_unescaped_character_consumer() {
        let input = "a";
        let chars = input.chars().collect::<Vec<char>>();
        let token = UnescapedCharacterConsumer::<'"'>::try_consume(&chars, 0).unwrap().unwrap();
        assert_eq!(token.literal, "a");
        assert_eq!(token.start, 0);
        assert_eq!(token.length, 1);

        let input = "\\b";
        let chars = input.chars().collect::<Vec<char>>();
        let token = UnescapedCharacterConsumer::<'"'>::try_consume(&chars, 0).unwrap();
        assert!(token.is_none());
    }

    #[test]
    fn test_escaped_string_char_consumer() {
        let token = EscapedStringCharConsumer::<'"'>::try_consume(&['\\', '"'], 0).unwrap().unwrap();
        assert_eq!(token.literal, "\\\"");
        assert_eq!(token.start, 0);
        assert_eq!(token.length, 2);

        let token = EscapedStringCharConsumer::<'"'>::try_consume(&['\\', 'u', '1', 'f', 'b', '8'], 0).unwrap().unwrap();
        assert_eq!(token.literal, "\\u1fb8");
        assert_eq!(token.start, 0);
        assert_eq!(token.length, 6);

        let token = EscapedStringCharConsumer::<'"'>::try_consume(&['\\', 'z'], 0);
        assert!(token.is_err());
    }

    #[test]
    fn test_quoted_string_consumer() {
        let input = "\"\\\\\"";
        let chars = input.chars().collect::<Vec<char>>();
        let token = DoubleQuotedStringConsumer::try_consume(&chars, 0).unwrap().unwrap();
        assert_eq!(token.literal, input);
        assert_eq!(token.token.decoded, "\\");
        assert_eq!(token.start, 0);
        assert_eq!(token.length, 4);

        let input = "\'\\u1fb8\\'\'";
        let chars = input.chars().collect::<Vec<char>>();
        let token = SingleQuotedStringConsumer::try_consume(&chars, 0).unwrap().unwrap();
        assert_eq!(token.literal, input);
        assert_eq!(token.token.decoded, "\u{1fb8}'");
        assert_eq!(token.start, 0);
        assert_eq!(token.length, 10);
    }

    #[test]
    fn test_unquoted_character_consumer() {
        let token = UnquotedCharacterConsumer::try_consume(&['a'], 0).unwrap().unwrap();
        assert_eq!(token.literal, "a");
        assert_eq!(token.start, 0);
        assert_eq!(token.length, 1);

        let token = UnquotedCharacterConsumer::try_consume(&['a', 'b'], 0).unwrap().unwrap();
        assert_eq!(token.literal, "a");
        assert_eq!(token.start, 0);
        assert_eq!(token.length, 1);

        assert!(UnquotedCharacterConsumer::try_consume(&['"'], 0).unwrap().is_none());
    }

    #[test]
    fn test_unquoted_string_consumer() {
        let input = "abc";
        let chars = input.chars().collect::<Vec<char>>();
        let token = UnquotedStringConsumer::try_consume(&chars, 0).unwrap().unwrap();
        assert_eq!(token.literal, input);
        assert_eq!(token.token.decoded, input);
        assert_eq!(token.start, 0);
        assert_eq!(token.length, 3);

        let input = "abc\\u1fb8";
        let chars = input.chars().collect::<Vec<char>>();
        let token = UnquotedStringConsumer::try_consume(&chars, 0).unwrap().unwrap();
        assert_eq!(token.literal, input);
        assert_eq!(token.token.decoded, "abc\u{1fb8}");
        assert_eq!(token.start, 0);
        assert_eq!(token.length, 9);
    }

    #[test]
    fn test_combined_string_consumer() {
        let input = "abc'test'\"${FOO}\"";
        let chars = input.chars().collect::<Vec<char>>();
        let token = CombinedStringConsumer::try_consume(&chars, 0).unwrap().unwrap();
        assert_eq!(token.literal, input);
        assert_eq!(token.start, 0);
        assert_eq!(token.length, 17, "Failed for {}. Got: {}", input, token.literal);
        assert_eq!(token.token.parts.len(), 3);
        assert_eq!(token.token.parts, vec![
            Token {
                literal: "abc".to_string(),
                start: 0,
                length: 3,
                token: QuotedOrUnquotedStringConsumer::Unquoted
            },
            Token {
                literal: "'test'".to_string(),
                start: 3,
                length: 6,
                token: QuotedOrUnquotedStringConsumer::SingleQuoted
            },
            Token {
                literal: "\"${FOO}\"".to_string(),
                start: 9,
                length: 8,
                token: QuotedOrUnquotedStringConsumer::DoubleQuoted
            }
        ]);

        let input = "abc'test'\"${FOO}\"   test";
        let chars = input.chars().collect::<Vec<char>>();
        let token = CombinedStringConsumer::try_consume(&chars, 0).unwrap().unwrap();
        assert_eq!(token.literal, "abc'test'\"${FOO}\"");
        assert_eq!(token.start, 0);
        assert_eq!(token.length, 17, "Failed for {}. Got: {}", input, token.literal);
        assert_eq!(token.token.parts.len(), 3);
        assert_eq!(token.token.parts, vec![
            Token {
                literal: "abc".to_string(),
                start: 0,
                length: 3,
                token: QuotedOrUnquotedStringConsumer::Unquoted
            },
            Token {
                literal: "'test'".to_string(),
                start: 3,
                length: 6,
                token: QuotedOrUnquotedStringConsumer::SingleQuoted
            },
            Token {
                literal: "\"${FOO}\"".to_string(),
                start: 9,
                length: 8,
                token: QuotedOrUnquotedStringConsumer::DoubleQuoted
            }
        ]);
    }

    #[test]
    fn test_expression_consumer() {
        let input = "./bin/sh -c 'echo \"hello world\"'";
        let chars = input.chars().collect::<Vec<char>>();
        let token = ExpressionConsumer::try_consume(&chars, 0).unwrap().unwrap();

        assert_eq!(token.literal, input);
        assert_eq!(token.start, 0);
        assert_eq!(token.length, 32);
        assert_eq!(token.token.parts.len(), 3);
        assert_eq!(token.token.parts, vec![
            Token {
                literal: "./bin/sh".to_string(),
                start: 0,
                length: 8,
                token: CombinedStringConsumer {
                    parts: vec![
                        Token {
                            literal: "./bin/sh".to_string(),
                            start: 0,
                            length: 8,
                            token: QuotedOrUnquotedStringConsumer::Unquoted
                        }
                    ]
                }
            },
            Token {
                literal: "-c".to_string(),
                start: 9,
                length: 2,
                token: CombinedStringConsumer {
                    parts: vec![
                        Token {
                            literal: "-c".to_string(),
                            start: 9,
                            length: 2,
                            token: QuotedOrUnquotedStringConsumer::Unquoted
                        }
                    ]
                }
            },
            Token {
                literal: "'echo \"hello world\"'".to_string(),
                start: 12,
                length: 20,
                token: CombinedStringConsumer {
                    parts: vec![
                        Token {
                            literal: "'echo \"hello world\"'".to_string(),
                            start: 12,
                            length: 20,
                            token: QuotedOrUnquotedStringConsumer::SingleQuoted
                        }
                    ]
                }
            }
        ]);

        let input = "./bin/sh -c 'echo \"hello world\"";
        let chars = input.chars().collect::<Vec<char>>();
        let token = ExpressionConsumer::try_consume(&chars, 0);
        assert!(token.is_err(), "Expected failure, but got {:?}", token.unwrap());
    }
}
