use std::fmt;

pub mod client; // Redis client

// encode commands init by client, and decode result from server
pub mod codec; // encode and decode commands

/// client errors
pub mod errors;

/// Redis values
///
/// | First byte | Type          |
/// | ---------- | ------------- |
/// | `+`        | Simple String |
/// | `-`        | Simple Error  |
/// | `:`        | Integer       |
/// | `$`        | Bulk String   |
/// | `*`        | Array         |
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedisValue {
    SimpleString(String),
    BulkString(String),
    Integer(i64),
    Array(Vec<RedisValue>),
    Err(String),
}

impl fmt::Display for RedisValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RedisValue::SimpleString(s) => write!(f, "{}", s),
            RedisValue::BulkString(s) => write!(f, "\"{}\"", s),
            RedisValue::Integer(n) => write!(f, "(integer) {}", n),
            RedisValue::Array(items) => {
                for (i, item) in items.iter().enumerate() {
                    writeln!(f, "{}) {}", i + 1, item)?;
                }
                Ok(())
            }
            RedisValue::Err(s) => write!(f, "(error) {}", s),
        }
    }
}
