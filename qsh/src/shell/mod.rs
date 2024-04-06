use common::io::IOTriple;
use std::{collections::HashMap, io::Write};
use thiserror::Error;

use crate::{
	buffer::Buffer,
	parser::{
		self,
		consumers::{Command, Pipeline, QuotedOrUnquotedString},
		types::{ParserError, Token},
	},
	process::{ExitCode, Process, ProcessPipeline, WaitError},
};

pub struct Shell {
	environment: HashMap<String, String>,
	pub triple: IOTriple,
}

impl Shell {
	pub fn new() -> Self {
		Shell {
			environment: default_environment_vars(),
			triple: IOTriple::default(),
		}
	}

	pub fn run(&mut self) {
		let input = self.triple.stdin();
		let output = self.triple.stdout();
		let mut err = self.triple.stderr();
		let mut buffer = Buffer::new(input, output);

		loop {
			let prompt = self.environment.get("PS1").unwrap();
			let line = match buffer.read(prompt) {
				Ok(line) => line,
				Err(e) => {
					writeln!(err, "Error reading input: {}", e).unwrap();
					return;
				}
			};

			let pipeline = match self.evaluate(&line) {
				Ok(pipeline) => pipeline,
				Err(PipelineError::ParserError(e)) => {
					writeln!(err, "Error evaluating input: {}", e).unwrap();
					continue;
				}
				Err(PipelineError::WaitError(e)) => {
					writeln!(err, "Error waiting for process: {}", e).unwrap();
					continue;
				}
				Err(PipelineError::NoPipeline) => continue,
			};

			match pipeline.get_exit_code() {
				Some(ExitCode::Success(code)) => self.environment.insert("?".to_owned(), code.to_string()),
				Some(ExitCode::Err(code)) => self.environment.insert("?".to_owned(), code.to_string()),
				None => panic!("BUG: pipeline has terminated, but no exit code found"),
			};
		}
	}

	/// Evaluate the input as a shell expression.
	pub fn evaluate(&mut self, input: &str) -> Result<ProcessPipeline, PipelineError> {
		let mut err = self.triple.stderr();

		let raw_pipe = match parser::try_parse::<Pipeline>(input) {
			Ok(Some(expr)) => expr,
			Ok(None) => return Err(PipelineError::NoPipeline),
			Err(e) => {
				writeln!(err, "Error parsing input: {}", e).unwrap();
				return Err(PipelineError::ParserError(e));
			}
		};

		if raw_pipe.token.commands.is_empty() {
			return Err(PipelineError::NoPipeline);
		}

		Ok(self.execute(raw_pipe, self.triple)?)
	}

	fn execute(&mut self, raw_pipe: Token<Pipeline>, triple: IOTriple) -> Result<ProcessPipeline, WaitError> {
		let commands = raw_pipe
			.token
			.commands
			.iter()
			.map(|c| {
				let args = self.concrete_arguments(c);
				Process::new(args)
			})
			.collect();

		let mut pipeline = ProcessPipeline::new(commands);
		pipeline.execute(triple)?;

		pipeline.wait()?;

		Ok(pipeline)
	}

	/// Construct the concrete expression from the token.
	/// At the moment, this just takes each string literally, but eventually this will do variable interpolation etc.
	fn concrete_arguments(&mut self, expression: &Token<Command>) -> Vec<String> {
		let mut args = Vec::new();
		for arg in expression.token.parts.iter() {
			let mut build = String::new();
			for token in arg.token.parts.iter() {
				match &token.token {
					QuotedOrUnquotedString::Unquoted(decoded)
					| QuotedOrUnquotedString::SingleQuoted(decoded)
					| QuotedOrUnquotedString::DoubleQuoted(decoded) => build.push_str(decoded),
				}
			}

			args.push(build);
		}

		args
	}
}

#[derive(Debug, Error)]
pub enum PipelineError {
	#[error("Error waiting for process: {0}")]
	WaitError(#[from] WaitError),

	#[error("Error parsing input: {0}")]
	ParserError(#[from] ParserError),

	#[error("No pipeline found")]
	NoPipeline,
}

fn default_environment_vars() -> HashMap<String, String> {
	let mut env = HashMap::new();
	env.insert("PATH".to_string(), "/bin:/usr/bin".to_string());
	env.insert("PS1".to_string(), "$ ".to_string());
	env
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_shell_concrete_expression() {
		let mut shell = Shell::new();
		assert_eq!(
			shell.concrete_arguments(&parser::try_parse("echo hello world").unwrap().unwrap()),
			vec!["echo", "hello", "world"]
		);
		assert_eq!(
			shell.concrete_arguments(&parser::try_parse("echo 'hello' \"world\"").unwrap().unwrap()),
			vec!["echo", "hello", "world"]
		);
		assert_eq!(
			shell.concrete_arguments(&parser::try_parse("echo'hello'\"world\"").unwrap().unwrap()),
			vec!["echohelloworld"]
		);
	}
}
