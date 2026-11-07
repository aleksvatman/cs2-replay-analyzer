use std::fmt;

#[derive(Debug)]
pub enum ParseError {
    Io(std::io::Error),
    Demo(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Io(e) => write!(f, "io error: {e}"),
            ParseError::Demo(e) => write!(f, "demo parse error: {e}"),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<std::io::Error> for ParseError {
    fn from(e: std::io::Error) -> Self {
        ParseError::Io(e)
    }
}
