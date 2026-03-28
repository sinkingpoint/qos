use std::fmt::{self, Display, Formatter};

use escapes::{Color, RESET};

use crate::TableError;

/// RowTable is a table that prints values in aligned rows,
/// with each row being at most `max_width` characters wide.
/// e.g.
/// README.md src/ rowtable.rs
/// foo       bar  baz
pub struct RowTable {
	values: Vec<(String, Option<Vec<Color>>)>,

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
			for (i, (value, _)) in chunk.iter().enumerate() {
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

		self.values.push((value, None));
		self.chunk_size = self.find_new_chunk_size();
		Ok(())
	}

	// Style a value in the table with the given ANSI escape sequences.
	pub fn style_value(&mut self, index: usize, escapes: Vec<Color>) {
		if index >= self.values.len() {
			return;
		}

		self.values[index].1 = Some(escapes);
	}

	// Reset the style of a value in the table.
	pub fn reset_value_style(&mut self, index: usize) {
		if index >= self.values.len() {
			return;
		}
		self.values[index].1 = None;
	}

	pub fn num_rows(&self) -> usize {
		(self.values.len() + self.chunk_size - 1) / self.chunk_size
	}

	pub fn num_cols(&self) -> usize {
		self.chunk_size
	}

	pub fn num_values(&self) -> usize {
		self.values.len()
	}

	pub fn value(&self, index: usize) -> Option<&String> {
		self.values.get(index).map(|(value, _)| value)
	}
}

impl Display for RowTable {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		if self.values.is_empty() {
			return Ok(());
		}

		let max_column_widths = self.max_column_widths_with_chunk_size(self.chunk_size);
		for chunks in self.values.chunks(self.chunk_size) {
			for (i, (value, escapes)) in chunks.iter().enumerate() {
				let escapes_str = if let Some(escapes) = escapes {
					escapes.iter().map(|e| e.to_string()).collect::<String>()
				} else {
					String::new()
				};
				write!(
					f,
					"{}{:width$}{}",
					escapes_str,
					value,
					RESET,
					width = max_column_widths[i]
				)?;
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
