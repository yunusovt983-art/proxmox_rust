use thiserror::Error;

pub type SharedResult<T> = Result<T, SharedTypeError>;

#[derive(Debug, Error)]
pub enum SharedTypeError {
    #[error("invalid value for {field}: {value}")]
    InvalidValue { field: &'static str, value: String },
    #[error("parse error: {0}")]
    ParseError(String),
    #[error("unsupported value: {0}")]
    Unsupported(String),
}
