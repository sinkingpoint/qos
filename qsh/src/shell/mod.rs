mod builtins;

use std::{collections::HashMap, io::Write};

use crate::{
    buffer::Buffer,
    parser::{
        self,
        consumers::{Expression, QuotedOrUnquotedString},
        types::Token,
    },
    process::{ExitCode, IOTriple, Process, ProcessState, WaitError},
};

use self::builtins::Builtin;

pub struct Shell {
    environment: HashMap<String, String>,
    builtins: HashMap<String, Box<dyn Builtin>>,
    pub triple: IOTriple,
}

impl Shell {
    pub fn new() -> Self {
        Shell {
            environment: default_environment_vars(),
            builtins: default_builtins(),
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

            match self.evaluate(&line) {
                Ok(()) => {}
                Err(e) => {
                    writeln!(err, "Error evaluating input: {}", e).unwrap();
                }
            }
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

        let mut err = self.triple.stderr();

        let args = self.concrete_arguments(expression);
        if let Some(builtin) = self.builtins.get(&args[0]) {
            match builtin.run(self.triple, &args) {
                ExitCode::Success(_) => {}
                ExitCode::Err(errno) => {
                    writeln!(err, "Process exited with error: {}", errno).unwrap();
                }
            }

            return Ok(());
        }

        let mut process = Process::new(args);
        process.start(self.triple)?;

        process.wait()?;

        if let ProcessState::Terminated(ExitCode::Success(code)) = process.state {
            write!(err, "Process exited with code {}", code).unwrap();
        }

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

fn default_builtins() -> HashMap<String, Box<dyn Builtin>> {
    let mut builtins: HashMap<String, Box<dyn Builtin>> = HashMap::new();
    builtins.insert("cat".to_string(), Box::new(builtins::Cat) as Box<dyn Builtin>);
    builtins
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
