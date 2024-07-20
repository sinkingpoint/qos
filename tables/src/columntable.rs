use crate::TableError;
use std::fmt::{self, Display, Formatter};

/// A setting that can be applied to a table.
pub enum TableSetting {
	/// Add a seperator between the headers and the rows.
	HeaderSeperator,

	/// Add a seperator between the columns.
	ColumnSeperators,

	/// Add a border around the table.
	Border,
}

/// A table that can be printed to the console.
pub struct Table<const COLS: usize> {
	headers: Option<[String; COLS]>,
	rows: Vec<[String; COLS]>,

	// A memoized copy of the amaximum width of each column.
	widths: [usize; COLS],

	// Settings
	/// Add a seperator between the headers and the rows.
	header_seperator: bool,

	/// Add a seperator between the columns.
	column_seperators: bool,

	/// Add a border around the table.
	border: bool,
}

impl<const COLS: usize> Default for Table<COLS> {
	fn default() -> Self {
		Table::new()
	}
}

impl<const COLS: usize> Table<COLS> {
	/// Create a new table.
	pub fn new() -> Table<COLS> {
		Table {
			headers: None,
			rows: Vec::new(),
			widths: [0; COLS],

			header_seperator: false,
			column_seperators: false,
			border: false,
		}
	}

	/// Create a new table with headers.
	pub fn new_with_headers(headers: [&str; COLS]) -> Table<COLS> {
		let headers = headers.map(|s| s.to_owned());
		let widths = headers.clone().map(|s| s.len());
		Table {
			headers: Some(headers),
			rows: Vec::new(),
			widths,

			header_seperator: false,
			column_seperators: false,
			border: false,
		}
	}

	/// Add a setting to the table.
	pub fn with_setting(&mut self, setting: TableSetting) -> &mut Self {
		match setting {
			TableSetting::HeaderSeperator => self.header_seperator = true,
			TableSetting::ColumnSeperators => self.column_seperators = true,
			TableSetting::Border => self.border = true,
		}
		self
	}

	fn width(&self) -> usize {
		// The width of all the columns.
		let mut base_width = self.widths.iter().sum::<usize>() + COLS - 1;

		if self.border {
			// If we have a border, we have two on each side of the table.
			base_width += 4;
		}

		if self.column_seperators {
			// If we have column seperators, we have three (two spaces and a |) for each column.
			base_width += COLS - 1;
		}

		base_width
	}

	pub fn add_row(&mut self, row: [&str; COLS]) -> Result<(), TableError> {
		let row = row.map(|s| s.to_owned());
		for (i, cell) in row.iter().enumerate() {
			self.widths[i] = self.widths[i].max(cell.len());
		}

		self.rows.push(row);
		Ok(())
	}

	fn write_row(&self, f: &mut fmt::Formatter, row: &[String]) -> fmt::Result {
		if self.border {
			write!(f, "| ")?;
		}

		for (i, cell) in row.iter().enumerate() {
			write!(f, "{:width$}", cell, width = self.widths[i])?;
			if i != row.len() - 1 {
				write!(f, " ")?;
				if self.column_seperators {
					write!(f, "| ")?;
				}
			}
		}

		if self.border {
			write!(f, " |")?;
		}

		writeln!(f)?;
		Ok(())
	}
}

impl<const COLS: usize> Display for Table<COLS> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		let width = self.width();
		let border_width = if self.column_seperators { width } else { width - 2 };
		if self.border {
			writeln!(f, "+{}+", "-".repeat(border_width))?;
		}

		if let Some(headers) = &self.headers {
			self.write_row(f, headers)?;

			if self.header_seperator {
				if self.border {
					writeln!(f, "|{}|", "-".repeat(width))?;
				} else {
					writeln!(f, "{}", "-".repeat(width))?;
				}
			}
		}

		for row in &self.rows {
			self.write_row(f, row)?;
		}

		if self.border {
			writeln!(f, "+{}+", "-".repeat(border_width))?;
		}
		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_table() {
		let mut table = Table::new_with_headers(["Name", "Age", "Occupation"]);
		table.add_row(["Colin", "25", "Software Engineer"]).unwrap();
		table.add_row(["John", "30", "Doctor"]).unwrap();
		table.add_row(["Jane", "28", "Nurse"]).unwrap();

		let output = format!("{}", table);
		assert_eq!(
			output,
			"Name  Age Occupation       \n\
							Colin 25  Software Engineer\n\
							John  30  Doctor           \n\
							Jane  28  Nurse            \n"
		);
	}

	#[test]
	fn test_table_with_border() {
		let mut table = Table::new_with_headers(["Name", "Age", "Occupation"]);
		table.add_row(["Colin", "25", "Software Engineer"]).unwrap();
		table.add_row(["John", "30", "Doctor"]).unwrap();
		table.add_row(["Jane", "28", "Nurse"]).unwrap();
		table.with_setting(TableSetting::Border);

		let output = format!("{}", table);
		assert_eq!(
			output,
			"+-----------------------------+\n\
							| Name  Age Occupation        |\n\
							| Colin 25  Software Engineer |\n\
							| John  30  Doctor            |\n\
							| Jane  28  Nurse             |\n\
							+-----------------------------+\n",
			"\n{}",
			output
		);
	}

	#[test]
	fn test_table_with_header_seperator() {
		let mut table = Table::new_with_headers(["Name", "Age", "Occupation"]);
		table.add_row(["Colin", "25", "Software Engineer"]).unwrap();
		table.add_row(["John", "30", "Doctor"]).unwrap();
		table.add_row(["Jane", "28", "Nurse"]).unwrap();
		table.with_setting(TableSetting::HeaderSeperator);

		let output = format!("{}", table);
		assert_eq!(
			output,
			"Name  Age Occupation       \n\
							---------------------------\n\
							Colin 25  Software Engineer\n\
							John  30  Doctor           \n\
							Jane  28  Nurse            \n"
		);
	}

	#[test]
	fn test_table_with_column_seperators() {
		let mut table = Table::new_with_headers(["Name", "Age", "Occupation"]);
		table.add_row(["Colin", "25", "Software Engineer"]).unwrap();
		table.add_row(["John", "30", "Doctor"]).unwrap();
		table.add_row(["Jane", "28", "Nurse"]).unwrap();
		table.with_setting(TableSetting::ColumnSeperators);

		let output = format!("{}", table);
		assert_eq!(
			output,
			"Name  | Age | Occupation       \n\
							Colin | 25  | Software Engineer\n\
							John  | 30  | Doctor           \n\
							Jane  | 28  | Nurse            \n"
		);
	}

	#[test]
	fn test_table_with_all_settings() {
		let mut table = Table::new_with_headers(["Name", "Age", "Occupation"]);
		table.add_row(["Colin", "25", "Software Engineer"]).unwrap();
		table.add_row(["John", "30", "Doctor"]).unwrap();
		table.add_row(["Jane", "28", "Nurse"]).unwrap();
		table
			.with_setting(TableSetting::Border)
			.with_setting(TableSetting::HeaderSeperator)
			.with_setting(TableSetting::ColumnSeperators);

		let output = format!("{}", table);
		assert_eq!(
			output,
			"+---------------------------------+\n\
							| Name  | Age | Occupation        |\n\
							|---------------------------------|\n\
							| Colin | 25  | Software Engineer |\n\
							| John  | 30  | Doctor            |\n\
							| Jane  | 28  | Nurse             |\n\
							+---------------------------------+\n",
			"\n{}",
			output
		);
	}

	#[test]
	fn test_table_without_headers() {
		let mut table = Table::new();
		table.add_row(["Colin", "25", "Software Engineer"]).unwrap();
		table.add_row(["John", "30", "Doctor"]).unwrap();
		table.add_row(["Jane", "28", "Nurse"]).unwrap();
		table.headers = None;

		let output = format!("{}", table);
		assert_eq!(
			output,
			"Colin 25 Software Engineer\n\
							John  30 Doctor           \n\
							Jane  28 Nurse            \n"
		);
	}
}
