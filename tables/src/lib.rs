mod columntable;
mod rowtable;

pub use columntable::*;
pub use rowtable::*;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TableError {
	#[error("incorrect number of columns: expected {0}, got {1}")]
	IncorrectNumberOfColumns(usize, usize),

	#[error("value too wide: max width is {0}, value is {1}")]
	ValueTooWide(usize, usize),

	#[error("index out of bounds: index {0} is out of bounds for length {1}")]
	IndexOutOfBounds(usize, usize),
}
