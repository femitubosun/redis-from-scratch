use std::num::ParseIntError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CliErrors {
    #[error("unterminated string")]
    UnterminatedString,

    #[error("invalid redis value: {0}")]
    InvalidRedisValue(String),

    #[error("invalid redis integer: {0}")]
    InvalidRedisInteger(#[from] ParseIntError),

    /// Not an error condition for streaming: the buffer does not yet hold a
    /// complete RESP frame. The codec waits for more bytes from the socket.
    #[error("incomplete frame: more bytes needed")]
    Incomplete,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
