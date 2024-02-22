use std::{
    cmp::Ordering,
    io::{self, Read, Write},
};

use escapes::{ANSIEscapeSequence, CursorBack, CursorForward, EraseInLine, ESC};

// The ASCII character for DEL.
const DELETE_CHAR: char = '\u{7f}';

/// Buffer is a wrapper around a Read that handles terminal IO.
pub struct Buffer<R: Read, W: Write> {
    /// The currently buffered input.
    buffer: String,

    /// The position of the cursor in the buffer.
    position: usize,
    reader: R,
    writer: W,
}

impl<R: Read, W: Write> Buffer<R, W> {
    pub fn new(reader: R, writer: W) -> Self {
        Buffer {
            buffer: String::new(),
            position: 0,
            reader,
            writer,
        }
    }

    /// Read a line from the buffer.
    pub fn read(&mut self, prompt: &str) -> io::Result<String> {
        write!(self.writer, "{}", prompt).expect("Failed to write to stdout");
        loop {
            let c = self.read_char()?;
            if c == '\n' {
                writeln!(self.writer).expect("Failed to write to stdout");
                return Ok(self.flush());
            } else if c == DELETE_CHAR {
                self.backspace();
            } else if c == ESC {
                self.handle_escape_sequence()?;
            } else {
                self.push_char(c);
            }
        }
    }

    /// Handle an ANSI escape sequence.
    fn handle_escape_sequence(&mut self) -> io::Result<()> {
        let escape = ANSIEscapeSequence::read(&mut self.reader)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, format!("Failed to parse ANSI escape sequence: {}", e)))?;

        match escape {
            ANSIEscapeSequence::CursorForward(amt) => self.move_cursor(amt.0 as isize),
            ANSIEscapeSequence::CursorBack(amt) => self.move_cursor(-(amt.0 as isize)),
            _ => (),
        }

        Ok(())
    }

    /// Move the cursor by the given amount across the buffer.
    fn move_cursor(&mut self, amt: isize) {
        // Find the new position and clamp it to the bounds of the buffer.
        let mut new_position = self.position as isize + amt;
        if new_position < 0 {
            new_position = 0;
        } else if new_position > self.buffer.len() as isize {
            new_position = self.buffer.len() as isize;
        }

        let new_position = new_position as usize;

        match new_position.cmp(&self.position) {
            Ordering::Less => write!(self.writer, "{}", CursorBack((self.position - new_position) as u8)).expect("Failed to write to stdout"),
            Ordering::Greater => write!(self.writer, "{}", CursorForward((new_position - self.position) as u8)).expect("Failed to write to stdout"),
            Ordering::Equal => (),
        }

        self.position = new_position;
    }

    /// Add a character to the buffer at the current position.
    fn push_char(&mut self, c: char) {
        if self.position == self.buffer.len() {
            self.buffer.push(c);
        } else {
            self.buffer.insert(self.position, c);
        }

        self.position += 1;
        self.rerender();
    }

    // Remove a character from the buffer at the current position.
    fn backspace(&mut self) {
        if self.position == 0 {
            return;
        }

        if self.position == self.buffer.len() {
            self.buffer.pop();
        } else {
            self.buffer.remove(self.position - 1);
        }

        write!(self.writer, "{}", CursorBack(2)).expect("Failed to write to stdout");
        self.position -= 1;
        self.rerender();
    }

    // Rewrite the current line, starting from the current position.
    fn rerender(&mut self) {
        let start = if self.position == 0 { 0 } else { self.position - 1 };
        write!(self.writer, "{}{}", EraseInLine(0), &self.buffer[start..]).expect("Failed to write to stdout");

        // After rewriting a line, we are at the end of it. If we were in the middle of the string, we need to move the cursor back.
        if self.buffer.len() > self.position {
            write!(self.writer, "{}", CursorBack((self.buffer.len() - self.position) as u8)).expect("Failed to write to stdout");
        }
    }

    /// Flush the buffer and return the contents.
    fn flush(&mut self) -> String {
        let buffer = self.buffer.clone();
        self.buffer.clear();
        self.position = 0;
        buffer
    }

    /// Read a single character from the input.
    fn read_char(&mut self) -> io::Result<char> {
        let mut char_buffer = [0; 1];
        self.reader.read_exact(&mut char_buffer)?;
        Ok(char_buffer[0] as char)
    }
}
