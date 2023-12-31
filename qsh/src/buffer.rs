use std::io::{Read, self};

/// Buffer is a wrapper around a Read that handles terminal IO.
pub struct Buffer<T: Read> {
    /// The currently buffered input.
    buffer: String,

    /// The position of the cursor in the buffer.
    position: usize,
    reader: T,
}

impl<T: Read> Buffer<T> {
    pub fn new(reader: T) -> Self {
        Buffer {
            buffer: String::new(),
            position: 0,
            reader,
        }
    }

    /// Read a line from the buffer.
    pub fn read(&mut self) -> io::Result<String> {
        loop {
            let c = self.read_char()?;
            if c == '\n' {
                return Ok(self.flush());
            } else {
                self.push_char(c);
            }
        }
    }

    /// Add a character to the buffer at the current position.
    fn push_char(&mut self, c: char) {
        if self.position == self.buffer.len() {
            self.buffer.push(c);
        } else {
            self.buffer.insert(self.position, c);
        }

        self.position += 1;
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
        self.reader.read(&mut char_buffer)?;
        Ok(char_buffer[0] as char)
    }
}
