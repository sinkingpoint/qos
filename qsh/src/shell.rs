use crate::{parser::{self, consumers::{Expression, QuotedOrUnquotedString}, types::Token}, process::{Process, WaitError}};

pub struct Shell {
}

impl Shell {
    /// Evaluate the input as a shell expression.
    pub fn evaluate(&mut self, input: &str) -> Result<(), WaitError>{
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

        process.wait()
    }

    /// Construct the concrete expression from the token.
    /// At the moment, this just takes each string literally, but eventually this will do variable interpolation etc.
    fn concrete_arguments(&mut self, expression: Token<Expression>) -> Vec<String> {
        let mut args = Vec::new();
        for arg in expression.token.parts {
            let mut build = String::new();
            for token in arg.token.parts {
                match token.token {
                    QuotedOrUnquotedString::Unquoted(decoded) | QuotedOrUnquotedString::SingleQuoted(decoded) | QuotedOrUnquotedString::DoubleQuoted(decoded) => build.push_str(&decoded),
                }
            }

            args.push(build);
        }

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_concrete_expression() {
        let mut shell = Shell {};
        assert_eq!(shell.concrete_arguments(parser::try_parse("echo hello world").unwrap().unwrap()), vec!["echo", "hello", "world"]);
        assert_eq!(shell.concrete_arguments(parser::try_parse("echo 'hello' \"world\"").unwrap().unwrap()), vec!["echo", "hello", "world"]);
        assert_eq!(shell.concrete_arguments(parser::try_parse("echo'hello'\"world\"").unwrap().unwrap()), vec!["echohelloworld"]);
    }
}