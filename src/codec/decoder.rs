use crate::{RedisValue, errors::CliErrors};
use tokio_util::bytes::{Buf, BytesMut};

pub fn decode_redis_value(bs: &mut BytesMut) -> Result<RedisValue, CliErrors> {
    if bs.is_empty() {
        return Err(CliErrors::Incomplete);
    }

    let prefix = bs.get_u8();

    match prefix {
        b'+' => decode_simple_string(bs),
        b':' => decode_integer(bs),
        b'$' => {
            let size = read_size(bs)?;
            decode_bulk_strings(bs, size as usize)
        }
        b'-' => decode_error(bs),
        b'*' => {
            let size = read_size(bs)? as usize;
            decode_array(bs, size)
        }

        _ => Err(CliErrors::InvalidRedisValue(format!(
            "Unknown redis data type: {}",
            prefix
        ))),
    }
}

pub fn decode_simple_string(bs: &mut BytesMut) -> Result<RedisValue, CliErrors> {
    let mut bss = Vec::new();

    loop {
        if bs.is_empty() {
            return Err(CliErrors::Incomplete);
        }

        let e = bs.get_u8();

        if e == b'\r' {
            if bs.is_empty() {
                return Err(CliErrors::Incomplete);
            }
            let e1 = bs.get_u8();

            if e1 != b'\n' {
                return Err(CliErrors::InvalidRedisValue("expect \\n".to_string()));
            }
            break;
        }

        bss.push(e);
    }

    let s = String::from_utf8_lossy(&bss);
    Ok(RedisValue::SimpleString(s.to_string()))
}

pub fn decode_bulk_strings(bs: &mut BytesMut, size: usize) -> Result<RedisValue, CliErrors> {
    if bs.len() < size + 2 {
        return Err(CliErrors::Incomplete);
    }

    let bulk_bs = bs[..size].to_vec();
    let tm = bs[size];
    let tm1 = bs[size + 1];
    if tm != b'\r' || tm1 != b'\n' {
        return Err(CliErrors::InvalidRedisValue("bad bulk string".to_string()));
    }

    // advance past the data and \r\n
    bs.advance(size + 2);

    let s = String::from_utf8_lossy(&bulk_bs);
    Ok(RedisValue::BulkString(s.to_string()))
}

pub fn decode_integer(bs: &mut BytesMut) -> Result<RedisValue, CliErrors> {
    let mut bss = Vec::new();

    loop {
        if bs.is_empty() {
            return Err(CliErrors::Incomplete);
        }

        let e = bs.get_u8();

        if e == b'\r' {
            if bs.is_empty() {
                return Err(CliErrors::Incomplete);
            }
            let e1 = bs.get_u8();

            if e1 != b'\n' {
                return Err(CliErrors::InvalidRedisValue("expect \\n".to_string()));
            }
            break;
        }

        bss.push(e);
    }

    let n_s = String::from_utf8_lossy(&bss);
    let integer = n_s.parse::<i64>()?;
    Ok(RedisValue::Integer(integer))
}

pub fn decode_array(bs: &mut BytesMut, size: usize) -> Result<RedisValue, CliErrors> {
    let mut arr = Vec::new();

    for _ in 0..size {
        arr.push(decode_redis_value(bs)?);
    }

    Ok(RedisValue::Array(arr))
}

pub fn decode_error(bs: &mut BytesMut) -> Result<RedisValue, CliErrors> {
    let mut bss = Vec::new();

    loop {
        if bs.is_empty() {
            return Err(CliErrors::Incomplete);
        }

        let e = bs.get_u8();
        if e == b'\r' {
            if bs.is_empty() {
                return Err(CliErrors::Incomplete);
            }
            let e1 = bs.get_u8();
            if e1 != b'\n' {
                return Err(CliErrors::InvalidRedisValue("expect \\n".to_string()));
            }
            break;
        }

        bss.push(e);
    }

    let n_s = String::from_utf8_lossy(&bss);
    Ok(RedisValue::Err(n_s.to_string()))
}

fn read_size(bs: &mut BytesMut) -> Result<u64, CliErrors> {
    let mut bss = Vec::new();
    loop {
        if bs.is_empty() {
            return Err(CliErrors::Incomplete);
        }

        let e = bs.get_u8();
        if e == b'\r' {
            if bs.is_empty() {
                return Err(CliErrors::Incomplete);
            }
            let e1 = bs.get_u8();
            if e1 != b'\n' {
                return Err(CliErrors::InvalidRedisValue("expect \\n".to_string()));
            }
            break;
        }
        bss.push(e);
    }

    let n_s = String::from_utf8_lossy(&bss);
    Ok(n_s.parse::<u64>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf(s: &str) -> BytesMut {
        BytesMut::from(s.as_bytes())
    }

    #[test]
    fn decodes_simple_string() {
        let value = decode_redis_value(&mut buf("+OK\r\n")).unwrap();
        assert_eq!(value, RedisValue::SimpleString("OK".to_string()));
    }

    #[test]
    fn decodes_integers() {
        let value = decode_redis_value(&mut buf(":123\r\n")).unwrap();
        assert_eq!(value, RedisValue::Integer(123));

        let value = decode_redis_value(&mut buf(":-42\r\n")).unwrap();
        assert_eq!(value, RedisValue::Integer(-42));
    }

    #[test]
    fn rejects_non_numeric_integer() {
        assert!(decode_redis_value(&mut buf(":abc\r\n")).is_err());
    }

    #[test]
    fn decodes_bulk_string() {
        let value = decode_redis_value(&mut buf("$5\r\nhello\r\n")).unwrap();
        assert_eq!(value, RedisValue::BulkString("hello".to_string()));
    }

    #[test]
    fn decodes_empty_bulk_string() {
        let value = decode_redis_value(&mut buf("$0\r\n\r\n")).unwrap();
        assert_eq!(value, RedisValue::BulkString(String::new()));
    }

    #[test]
    fn decodes_error() {
        let value = decode_redis_value(&mut buf("-ERR unknown command\r\n")).unwrap();
        assert_eq!(value, RedisValue::Err("ERR unknown command".to_string()));
    }

    #[test]
    fn decodes_array() {
        let value = decode_redis_value(&mut buf("*2\r\n$3\r\nGET\r\n$3\r\nkey\r\n")).unwrap();
        assert_eq!(
            value,
            RedisValue::Array(vec![
                RedisValue::BulkString("GET".to_string()),
                RedisValue::BulkString("key".to_string()),
            ])
        );
    }

    #[test]
    fn decodes_nested_array() {
        let value = decode_redis_value(&mut buf("*2\r\n*2\r\n:1\r\n:2\r\n+done\r\n")).unwrap();
        assert_eq!(
            value,
            RedisValue::Array(vec![
                RedisValue::Array(vec![RedisValue::Integer(1), RedisValue::Integer(2)]),
                RedisValue::SimpleString("done".to_string()),
            ])
        );
    }

    #[test]
    fn rejects_unknown_prefix() {
        assert!(decode_redis_value(&mut buf("?nope\r\n")).is_err());
    }

    #[test]
    fn rejects_empty_buffer() {
        assert!(decode_redis_value(&mut BytesMut::new()).is_err());
    }

    #[test]
    fn rejects_truncated_bulk_string() {
        assert!(decode_redis_value(&mut buf("$5\r\nhe")).is_err());
    }

    #[test]
    fn leaves_remaining_bytes_in_buffer() {
        let mut bs = buf("$5\r\nhello\r\n+OK\r\n");
        decode_redis_value(&mut bs).unwrap();
        assert_eq!(&bs[..], b"+OK\r\n");
    }
}
