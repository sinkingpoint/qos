use std::fmt::{self, Display, Formatter};

use crate::TableError;

/// RowTable is a table that prints values in aligned rows,
/// with each row being at most `max_width` characters wide.
/// e.g.
/// README.md src/ rowtable.rs
/// foo       bar  baz
pub struct RowTable {
	values: Vec<String>,

	/// The maximum width (in characters) of the table.
	max_width: usize,

	/// The number of values in each row.
	chunk_size: usize,
}

impl RowTable {
	pub fn new(max_width: usize) -> Self {
		Self {
			values: Vec::new(),
			max_width,
			chunk_size: 0,
		}
	}

	/// Calculate the maximum width of each column when the values are split into chunks of `chunk_size`.
	fn max_column_widths_with_chunk_size(&self, chunk_size: usize) -> Vec<usize> {
		let mut max_column_widths = vec![0; chunk_size];
		for chunk in self.values.chunks(chunk_size) {
			for (i, value) in chunk.iter().enumerate() {
				max_column_widths[i] = max_column_widths[i].max(value.len());
			}
		}
		max_column_widths
	}

	/// Find the largest chunk size that fits within `max_width`.
	fn find_new_chunk_size(&self) -> usize {
		let mut chunk_size = self.values.len();
		while chunk_size > 0 {
			let max_column_widths = self.max_column_widths_with_chunk_size(chunk_size);

			if max_column_widths.iter().sum::<usize>() + chunk_size - 1 <= self.max_width {
				break;
			}

			chunk_size -= 1;
		}
		chunk_size
	}

	/// Add a value to the table.
	pub fn add_value(&mut self, value: String) -> Result<(), TableError> {
		if value.len() > self.max_width {
			return Err(TableError::ValueTooWide(self.max_width, value.len()));
		}

		self.values.push(value);
		self.chunk_size = self.find_new_chunk_size();
		Ok(())
	}
}

impl Display for RowTable {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		let max_column_widths = self.max_column_widths_with_chunk_size(self.chunk_size);
		for chunks in self.values.chunks(self.chunk_size) {
			for (i, chunk) in chunks.iter().enumerate() {
				write!(f, "{:width$}", chunk, width = max_column_widths[i])?;
				if i != chunks.len() - 1 {
					write!(f, " ")?;
				}
			}
			writeln!(f)?;
		}
		Ok(())
	}
}

#[cfg(test)]
mod test {
	use super::*;
	#[test]
	fn test_one_row() {
		let mut table = RowTable::new(80);
		table.add_value("hello".to_string()).unwrap();
		table.add_value("world".to_string()).unwrap();
		assert_eq!(table.to_string(), "hello world\n");
	}

	#[test]
	fn test_two_rows() {
		let mut table = RowTable::new(11);
		table.add_value("hello".to_string()).unwrap();
		table.add_value("world".to_string()).unwrap();
		table.add_value("foo".to_string()).unwrap();
		table.add_value("bar".to_string()).unwrap();
		assert_eq!(table.to_string(), "hello world\nfoo   bar  \n", "\n{}", table);
	}

	#[test]
	fn test_value_includes_padding() {
		let mut table = RowTable::new(10);
		table.add_value("hello".to_string()).unwrap();
		table.add_value("world".to_string()).unwrap();
		assert_eq!(table.to_string(), "hello\nworld\n");
	}
}
