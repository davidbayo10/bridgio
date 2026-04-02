use std::fmt;

#[derive(Debug)]
#[allow(dead_code)]
pub enum AppError {
    Aws(String),
    Io(std::io::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Aws(msg) => write!(f, "AWS error: {msg}"),
            AppError::Io(e) => write!(f, "IO error: {e}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e)
    }
}
