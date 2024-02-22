use std::{collections::HashMap, fs::File, os::fd::FromRawFd};

use crate::{
    buffer::Buffer,
    parser::{
        self,
        consumers::{Expression, QuotedOrUnquotedString},
        types::Token,
    },
    process::{Process, ProcessState, WaitError, ExitCode},
};

pub struct Shell {
    environment: HashMap<String, String>,
}

impl Shell {
    pub fn new() -> Self {
        Shell {
            environment: default_environment_vars(),
        }
    }

    pub fn run(&mut self, input: File, output: File) {
        let mut buffer = Buffer::new(input, output);

        loop {
            let prompt = self.environment.get("PS1").unwrap();
            let line = match buffer.read(prompt) {
                Ok(line) => line,
                Err(_e) => {
                    return;
                }
            };

            self.evaluate(&line).unwrap();
        }
    }

    /// Evaluate the input as a shell expression.
    pub fn evaluate(&mut self, input: &str) -> Result<(), WaitError> {
        let expression = match parser::try_parse::<Expression>(input) {
            Ok(Some(expr)) => expr,
            Ok(None) => return Ok(()),
            Err(e) => {
                println!("Error: {}", e);
                return Ok(());
            }
        };

        let args = self.concrete_arguments(expression);
        let mut process = Process::new(args);
        process.start()?;

        process.wait()?;

        match process.state {
            ProcessState::Terminated(exitcode) => {
                if let ExitCode::Success(code) = exitcode {
                    println!("Process exited with code {}", code);
                }
            }
            _ => {}
        };
        Ok(())
    }

    /// Construct the concrete expression from the token.
    /// At the moment, this just takes each string literally, but eventually this will do variable interpolation etc.
    fn concrete_arguments(&mut self, expression: Token<Expression>) -> Vec<String> {
        let mut args = Vec::new();
        for arg in expression.token.parts {
            let mut build = String::new();
            for token in arg.token.parts {
                match token.token {
                    QuotedOrUnquotedString::Unquoted(decoded)
                    | QuotedOrUnquotedString::SingleQuoted(decoded)
                    | QuotedOrUnquotedString::DoubleQuoted(decoded) => build.push_str(&decoded),
                }
            }

            args.push(build);
        }

        args
    }
}

fn default_environment_vars() -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("PATH".to_string(), "/bin:/usr/bin".to_string());
    env.insert("PS1".to_string(), "$ ".to_string());
    env
}

pub fn stdout() -> File {
    unsafe { File::from_raw_fd(1) }
}

pub fn stdin() -> File {
    unsafe { File::from_raw_fd(0) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_concrete_expression() {
        let mut shell = Shell::new();
        assert_eq!(
            shell.concrete_arguments(parser::try_parse("echo hello world").unwrap().unwrap()),
            vec!["echo", "hello", "world"]
        );
        assert_eq!(
            shell.concrete_arguments(parser::try_parse("echo 'hello' \"world\"").unwrap().unwrap()),
            vec!["echo", "hello", "world"]
        );
        assert_eq!(
            shell.concrete_arguments(parser::try_parse("echo'hello'\"world\"").unwrap().unwrap()),
            vec!["echohelloworld"]
        );
    }
}
