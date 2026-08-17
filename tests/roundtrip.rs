//! Integration test: exercises the crate's public API end to end,
//! the same way an external consumer would.

use redis_from_scratch::{
    RedisValue,
    codec::{decoder::decode_redis_value, encoder::encode_cmd, parse_cmd_to_strings},
};
use tokio_util::bytes::BytesMut;

#[test]
fn full_command_round_trip() {
    // 1. Parse what the user typed
    let tokens = parse_cmd_to_strings("SET greeting \"hello world\"").unwrap();
    assert_eq!(tokens, ["SET", "greeting", "hello world"]);

    // 2. Encode it onto the wire as RESP
    let encoded = encode_cmd(tokens).unwrap();
    assert_eq!(
        &encoded[..],
        b"*3\r\n$3\r\nSET\r\n$8\r\ngreeting\r\n$11\r\nhello world\r\n"
    );

    // 3. Decode it back, as the server side would
    let mut buf = BytesMut::from(&encoded[..]);
    let decoded = decode_redis_value(&mut buf).unwrap();
    assert_eq!(
        decoded,
        RedisValue::Array(vec![
            RedisValue::BulkString("SET".to_string()),
            RedisValue::BulkString("greeting".to_string()),
            RedisValue::BulkString("hello world".to_string()),
        ])
    );
    assert!(buf.is_empty());
}

#[test]
fn parser_errors_propagate() {
    let result = parse_cmd_to_strings("SET key \"unterminated");
    assert!(result.is_err());
}
