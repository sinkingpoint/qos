mod builtins;

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

	builtins: HashMap<String, Box<dyn builtins::Builtin>>,
}

enum Executable {
	Builtin(i32),
	Pipeline(ProcessPipeline),
}

impl Shell {
	pub fn new() -> Self {
		Shell {
			environment: default_environment_vars(),
			triple: IOTriple::default(),
			builtins: default_builtins(),
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

			let exit_code = match self.evaluate(&line) {
				Ok(Executable::Pipeline(pipeline)) => match pipeline.get_exit_code() {
					Some(ExitCode::Success(code)) => code,
					Some(ExitCode::Err(code)) => code as i32,
					None => panic!("BUG: pipeline has terminated, but no exit code found"),
				},
				Ok(Executable::Builtin(code)) => code,
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

			self.environment.insert("?".to_owned(), exit_code.to_string());
		}
	}

	/// Evaluate the input as a shell expression.
	fn evaluate(&mut self, input: &str) -> Result<Executable, PipelineError> {
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

	fn execute(&mut self, raw_pipe: Token<Pipeline>, triple: IOTriple) -> Result<Executable, WaitError> {
		let commands: Vec<Process> = raw_pipe
			.token
			.commands
			.iter()
			.map(|c| {
				let args = self.concrete_arguments(c);
				Process::new(args)
			})
			.collect();

		// If there's only one command, try to execute it as a builtin.
		if commands.len() == 1 {
			match self.try_execute_as_builtin(triple, &commands[0]) {
				Ok(Some(exec)) => return Ok(exec),
				Ok(None) => (),
				Err(e) => return Err(e),
			}
		}

		let mut pipeline = ProcessPipeline::new(commands);
		pipeline.execute(triple)?;

		pipeline.wait()?;

		Ok(Executable::Pipeline(pipeline))
	}

	/// Try to execute the command as a builtin, returning the exit code if it was able to be run.
	fn try_execute_as_builtin(&mut self, triple: IOTriple, process: &Process) -> Result<Option<Executable>, WaitError> {
		let argv = &process.argv;

		if let Some(builtin) = self.builtins.get(&argv[0]) {
			let code = builtin.run(&argv[1..], triple, self)?;
			return Ok(Some(Executable::Builtin(code)));
		}

		Ok(None)
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

fn default_builtins() -> HashMap<String, Box<dyn builtins::Builtin>> {
	let mut builtins = HashMap::new();
	builtins.insert(
		"clear".to_string(),
		Box::new(builtins::Clear) as Box<dyn builtins::Builtin>,
	);
	builtins
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
