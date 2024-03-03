use core::fmt;
use std::error::Error;

//
#[derive(Debug)]
pub struct ParserError {
	pub message: String,
	pub start: usize,
}

impl ParserError {
	pub fn new(message: &str, start: usize) -> Self {
		ParserError {
			message: message.to_string(),
			start,
		}
	}
}

impl Error for ParserError {}

impl fmt::Display for ParserError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "ParserError: {}", self.message)
	}
}

#[derive(Debug, PartialEq)]
pub struct Token<T> {
	pub literal: String,
	pub start: usize,
	pub length: usize,
	pub token: T,
}

pub type ParserResult<T> = Result<Option<Token<T>>, ParserError>;

pub trait Consumer {
	fn try_consume(input: &[char], start: usize) -> ParserResult<Self>
	where
		Self: Sized;
}
