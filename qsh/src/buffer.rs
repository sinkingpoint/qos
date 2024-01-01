use std::io::{Read, self, Write};

use escapes::{ESC, ANSIEscapeSequence, CursorForward, CursorBack, EraseInLine};

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
    pub fn read(&mut self) -> io::Result<String> {
        loop {
            let c = self.read_char()?;
            if c == '\n' {
                writeln!(self.writer).expect("Failed to write to stdout");
                return Ok(self.flush());
            } else if c == ESC {
                self.handle_escape_sequence()?;
            } else {
                self.push_char(c);
            }
        }
    }

    /// Handle an ANSI escape sequence.
    fn handle_escape_sequence(&mut self) -> io::Result<()> {
        let escape = ANSIEscapeSequence::read(&mut self.reader).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse ANSI escape sequence: {}", e),
            )
        })?;

        match escape {
            ANSIEscapeSequence::CursorForward(amt) => self.move_cursor(amt.0 as isize),
            ANSIEscapeSequence::CursorBack(amt) => self.move_cursor(-(amt.0 as isize)),
            _ => (),
        }

        return Ok(());
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

        // Write out the ANSI escape sequence to move the cursor.
        if new_position > self.position as isize {
            write!(self.writer, "{}", CursorForward(new_position as u8 - self.position as u8)).expect("Failed to write to stdout");
        } else if new_position < self.position as isize {
            write!(self.writer, "{}", CursorBack(self.position as u8 - new_position as u8)).expect("Failed to write to stdout");
        }

        self.position = new_position as usize;
    }

    /// Add a character to the buffer at the current position.
    fn push_char(&mut self, c: char) {
        if self.position == self.buffer.len() {
            self.buffer.push(c);
        } else {
            self.buffer.insert(self.position, c);
        }

        self.rerender();

        self.position += 1;
    }

    // Rewrite the current line, starting from the current position.
    fn rerender(&mut self) {
        write!(self.writer, "{}{}", EraseInLine(0), &self.buffer[self.position..]).expect("Failed to write to stdout");
        // After rewriting a line, we are at the end of it. If we were in the middle of the string, we need to move the cursor back.
        if self.buffer.len() > (self.position + 1) {
            write!(self.writer, "{}", CursorBack((self.buffer.len() - (self.position + 1)) as u8)).expect("Failed to write to stdout");
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
