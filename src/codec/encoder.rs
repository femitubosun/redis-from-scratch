use tokio_util::bytes::{BufMut, Bytes, BytesMut};

use crate::{RedisValue, errors::CliErrors};

pub fn encode_cmd(values: Vec<String>) -> Result<Bytes, CliErrors> {
    let cmd = RedisValue::Array(values.into_iter().map(RedisValue::BulkString).collect());
    encode_redis_value(cmd)
}

pub fn encode_simple_string(val: RedisValue) -> Result<Bytes, CliErrors> {
    match val {
        RedisValue::SimpleString(s) => Ok(Bytes::from(format!("+{}\r\n", s))),
        _ => Err(CliErrors::InvalidRedisValue(
            "expected SimpleString".to_string(),
        )),
    }
}

pub fn encode_bulk_strings(val: RedisValue) -> Result<Bytes, CliErrors> {
    match val {
        RedisValue::BulkString(s) => Ok(Bytes::from(format!("${}\r\n{}\r\n", s.len(), s))),
        _ => Err(CliErrors::InvalidRedisValue(
            "expected BulkString".to_string(),
        )),
    }
}

pub fn encode_integer(val: i64) -> Result<Bytes, CliErrors> {
    Ok(Bytes::from(format!(":{}\r\n", val)))
}

pub fn encode_array(val: RedisValue) -> Result<Bytes, CliErrors> {
    match val {
        RedisValue::Array(items) => {
            let mut buf = BytesMut::new();
            buf.put_slice(format!("*{}\r\n", items.len()).as_bytes());
            for item in items {
                let encoded = encode_redis_value(item)?;
                buf.put_slice(&encoded);
            }
            Ok(buf.freeze())
        }
        _ => Err(CliErrors::InvalidRedisValue("expected Array".to_string())),
    }
}

pub fn encode_redis_value(val: RedisValue) -> Result<Bytes, CliErrors> {
    match val {
        RedisValue::SimpleString(_) => encode_simple_string(val),
        RedisValue::BulkString(_) => encode_bulk_strings(val),
        RedisValue::Integer(n) => encode_integer(n),
        RedisValue::Array(_) => encode_array(val),
        RedisValue::Err(s) => Ok(Bytes::from(format!("-{}\r\n", s))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::decoder::decode_redis_value;

    #[test]
    fn encodes_command_as_resp_array() {
        let bytes = encode_cmd(vec!["GET".to_string(), "key".to_string()]).unwrap();
        assert_eq!(&bytes[..], b"*2\r\n$3\r\nGET\r\n$3\r\nkey\r\n");
    }

    #[test]
    fn encodes_simple_string() {
        let bytes = encode_simple_string(RedisValue::SimpleString("OK".to_string())).unwrap();
        assert_eq!(&bytes[..], b"+OK\r\n");
    }

    #[test]
    fn rejects_wrong_variant() {
        assert!(encode_simple_string(RedisValue::Integer(1)).is_err());
        assert!(encode_bulk_strings(RedisValue::SimpleString("x".to_string())).is_err());
        assert!(encode_array(RedisValue::Integer(1)).is_err());
    }

    #[test]
    fn encodes_bulk_string() {
        let bytes = encode_bulk_strings(RedisValue::BulkString("hello".to_string())).unwrap();
        assert_eq!(&bytes[..], b"$5\r\nhello\r\n");
    }

    #[test]
    fn encodes_integer() {
        let bytes = encode_integer(42).unwrap();
        assert_eq!(&bytes[..], b":42\r\n");

        let bytes = encode_integer(-7).unwrap();
        assert_eq!(&bytes[..], b":-7\r\n");
    }

    #[test]
    fn encodes_nested_array() {
        let value = RedisValue::Array(vec![
            RedisValue::Integer(1),
            RedisValue::Array(vec![RedisValue::Err("boom".to_string())]),
        ]);
        let bytes = encode_array(value).unwrap();
        assert_eq!(&bytes[..], b"*2\r\n:1\r\n*1\r\n-boom\r\n");
    }

    #[test]
    fn encode_then_decode_round_trip() {
        let bytes = encode_cmd(vec![
            "SET".to_string(),
            "key".to_string(),
            "value".to_string(),
        ])
        .unwrap();

        let mut buf = BytesMut::from(&bytes[..]);
        let decoded = decode_redis_value(&mut buf).unwrap();
        assert_eq!(
            decoded,
            RedisValue::Array(vec![
                RedisValue::BulkString("SET".to_string()),
                RedisValue::BulkString("key".to_string()),
                RedisValue::BulkString("value".to_string()),
            ])
        );
        assert!(buf.is_empty());
    }
}
