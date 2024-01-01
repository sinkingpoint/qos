use std::{io::{self, Read}, fmt::{Display, Formatter, self}};
use thiserror::Error;
use escapes_derive::EscapeSequence;

pub const ESC: char = '\x1b';

/// The CSI (Control Sequence Introducer) character.
pub const CSI: char = '[';

/// The error that can occur when parsing ANSI escape sequences.
#[derive(Error, Debug)]
pub enum AnsiParserError {
    #[error("Unsupported ANSI escape sequence")]
    Malformed,

    #[error("Expected {0} parameters but found {1}")]
    NumParams(usize, usize),

    #[error("Unsupported ANSI escape sequence: {0}")]
    Unsupported(char),

    #[error("IO error: {0}")]
    IO(#[from] io::Error),
}

/// A trait for parsing ANSI escape sequences.
trait EscapeSequence: Display {
    fn parse(params: &[u8]) -> Result<Self, AnsiParserError> where Self: Sized;
}

/// Move the cursor up by the given amount of lines.
#[derive(Debug, PartialEq, EscapeSequence)]
#[escape('A')]
pub struct CursorUp(#[default(1)] pub u8);

/// Move the cursor down by the given amount of lines.
#[derive(Debug, PartialEq, EscapeSequence)]
#[escape('B')]
pub struct CursorDown(#[default(1)] pub u8);

/// Move the cursor down by the given amount of lines.
#[derive(Debug, PartialEq, EscapeSequence)]
#[escape('C')]
pub struct CursorForward(#[default(1)] pub u8);

/// Move the cursor down by the given amount of lines.
#[derive(Debug, PartialEq, EscapeSequence)]
#[escape('D')]
pub struct CursorBack(#[default(1)] pub u8);

#[derive(Debug, PartialEq, EscapeSequence)]
#[escape('K')]
pub struct EraseInLine(#[default(0)] pub u8);

#[derive(Debug, PartialEq)]
pub enum ANSIEscapeSequence {
    CursorUp(CursorUp),
    CursorDown(CursorDown),
    CursorForward(CursorForward),
    CursorBack(CursorBack),
    EraseInLine(EraseInLine),
}

impl ANSIEscapeSequence {
    fn new(c: char, params: &[u8]) -> Result<ANSIEscapeSequence, AnsiParserError> {
        match c {
            'A' => Ok(ANSIEscapeSequence::CursorUp(CursorUp::parse(params)?)),
            'B' => Ok(ANSIEscapeSequence::CursorDown(CursorDown::parse(params)?)),
            'C' => Ok(ANSIEscapeSequence::CursorForward(CursorForward::parse(params)?)),
            'D' => Ok(ANSIEscapeSequence::CursorBack(CursorBack::parse(params)?)),
            'K' => Ok(ANSIEscapeSequence::EraseInLine(EraseInLine::parse(params)?)),
            _ => Err(AnsiParserError::Unsupported(c)),
        }
    }

    /// Read an ANSI escape sequence from the given reader. Assumes that the first byte (ESC) has already been read.
    pub fn read<T: Read>(reader: &mut T) -> Result<ANSIEscapeSequence, AnsiParserError> {
        let mut char_buffer = [0; 1];
        reader.read_exact(&mut char_buffer)?;

        // All the escape sequences we care about start with CSI ('[').
        if char_buffer[0] != CSI as u8 {
            return Err(AnsiParserError::Malformed);
        }

        // Parse the parameters.
        // Parameters are numeric values separated by semicolons and are terminated by a letter, e.g. 1;2;3A.
        let mut params = Vec::new();
        let mut param_buffer = String::new();
        loop {
            reader.read_exact(&mut char_buffer)?;
            let c = char_buffer[0] as char;

            if c.is_ascii_digit() {
                param_buffer.push(char_buffer[0] as char);
                continue;
            } else if !param_buffer.is_empty() {
                params.push(param_buffer.parse().map_err(|_| AnsiParserError::IO(std::io::Error::new(std::io::ErrorKind::InvalidData, "Failed to parse parameter")))?);
                param_buffer.clear();
            }

            if c != ';' {
                break;
            }
        }

        if params.is_empty() {
            params.push(1);
        }

        ANSIEscapeSequence::new(char_buffer[0] as char, &params)
    }
}

impl Display for ANSIEscapeSequence {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ANSIEscapeSequence::CursorUp(c) => write!(f, "{}", c),
            ANSIEscapeSequence::CursorDown(c) => write!(f, "{}", c),
            ANSIEscapeSequence::CursorForward(c) => write!(f, "{}", c),
            ANSIEscapeSequence::CursorBack(c) => write!(f, "{}", c),
            ANSIEscapeSequence::EraseInLine(c) => write!(f, "{}", c),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_cursor_up() {
        assert_eq!(ANSIEscapeSequence::CursorUp(CursorUp(1)).to_string(), "\x1b[1A");
        assert_eq!(ANSIEscapeSequence::CursorUp(CursorUp(10)).to_string(), "\x1b[10A");

        assert_eq!(ANSIEscapeSequence::read(&mut "[A".as_bytes()).unwrap(), ANSIEscapeSequence::CursorUp(CursorUp(1)));
        assert_eq!(ANSIEscapeSequence::read(&mut "[1A".as_bytes()).unwrap(), ANSIEscapeSequence::CursorUp(CursorUp(1)));
        assert_eq!(ANSIEscapeSequence::read(&mut "[10A".as_bytes()).unwrap(), ANSIEscapeSequence::CursorUp(CursorUp(10)));
    }

    #[test]
    fn test_cursor_down() {
        assert_eq!(ANSIEscapeSequence::CursorDown(CursorDown(1)).to_string(), "\x1b[1B");
        assert_eq!(ANSIEscapeSequence::CursorDown(CursorDown(10)).to_string(), "\x1b[10B");

        assert_eq!(ANSIEscapeSequence::read(&mut "[B".as_bytes()).unwrap(), ANSIEscapeSequence::CursorDown(CursorDown(1)));
        assert_eq!(ANSIEscapeSequence::read(&mut "[1B".as_bytes()).unwrap(), ANSIEscapeSequence::CursorDown(CursorDown(1)));
        assert_eq!(ANSIEscapeSequence::read(&mut "[10B".as_bytes()).unwrap(), ANSIEscapeSequence::CursorDown(CursorDown(10)));
    }

    #[test]
    fn test_cursor_forward() {
        assert_eq!(ANSIEscapeSequence::CursorForward(CursorForward(1)).to_string(), "\x1b[1C");
        assert_eq!(ANSIEscapeSequence::CursorForward(CursorForward(10)).to_string(), "\x1b[10C");

        assert_eq!(ANSIEscapeSequence::read(&mut "[C".as_bytes()).unwrap(), ANSIEscapeSequence::CursorForward(CursorForward(1)));
        assert_eq!(ANSIEscapeSequence::read(&mut "[1C".as_bytes()).unwrap(), ANSIEscapeSequence::CursorForward(CursorForward(1)));
        assert_eq!(ANSIEscapeSequence::read(&mut "[10C".as_bytes()).unwrap(), ANSIEscapeSequence::CursorForward(CursorForward(10)));
    }

    #[test]
    fn test_cursor_back() {
        assert_eq!(ANSIEscapeSequence::CursorBack(CursorBack(1)).to_string(), "\x1b[1D");
        assert_eq!(ANSIEscapeSequence::CursorBack(CursorBack(10)).to_string(), "\x1b[10D");

        assert_eq!(ANSIEscapeSequence::read(&mut "[D".as_bytes()).unwrap(), ANSIEscapeSequence::CursorBack(CursorBack(1)));
        assert_eq!(ANSIEscapeSequence::read(&mut "[1D".as_bytes()).unwrap(), ANSIEscapeSequence::CursorBack(CursorBack(1)));
        assert_eq!(ANSIEscapeSequence::read(&mut "[10D".as_bytes()).unwrap(), ANSIEscapeSequence::CursorBack(CursorBack(10)));
    }

    #[test]
    fn test_erase_in_line() {
        assert_eq!(ANSIEscapeSequence::EraseInLine(EraseInLine(0)).to_string(), "\x1b[0K");
        assert_eq!(ANSIEscapeSequence::EraseInLine(EraseInLine(1)).to_string(), "\x1b[1K");
        assert_eq!(ANSIEscapeSequence::EraseInLine(EraseInLine(2)).to_string(), "\x1b[2K");

        assert_eq!(ANSIEscapeSequence::read(&mut "[0K".as_bytes()).unwrap(), ANSIEscapeSequence::EraseInLine(EraseInLine(0)));
        assert_eq!(ANSIEscapeSequence::read(&mut "[1K".as_bytes()).unwrap(), ANSIEscapeSequence::EraseInLine(EraseInLine(1)));
        assert_eq!(ANSIEscapeSequence::read(&mut "[2K".as_bytes()).unwrap(), ANSIEscapeSequence::EraseInLine(EraseInLine(2)));
    }
}
