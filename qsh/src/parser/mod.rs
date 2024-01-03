use self::types::{Consumer, ParserResult};

pub mod consumers;
pub mod types;

pub fn try_parse<T: Consumer + Sized>(input: &str) -> ParserResult<T> {
    let chars: Vec<char> = input.chars().collect();
    T::try_consume(&chars, 0)
}
