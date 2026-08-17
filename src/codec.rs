use crate::RedisValue;
use crate::codec::decoder::decode_redis_value;
use crate::codec::encoder::encode_redis_value;
use crate::errors::CliErrors;
use tokio_util::bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

pub mod decoder;
pub mod encoder;

/// A tokio-util codec that turns a TCP byte stream into a stream of
/// [`RedisValue`] frames, and vice versa.
///
/// Use with [`tokio_util::codec::Framed`]:
/// ```no_run
/// # use redis_from_scratch::codec::RedisCodec;
/// # use tokio::net::TcpStream;
/// # use tokio_util::codec::Framed;
/// # async fn f(stream: TcpStream) {
/// let mut framed = Framed::new(stream, RedisCodec);
/// # }
/// ```
#[derive(Debug, Default, Clone, Copy)]
pub struct RedisCodec;

impl Decoder for RedisCodec {
    type Item = RedisValue;
    type Error = CliErrors;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<RedisValue>, CliErrors> {
        if src.is_empty() {
            return Ok(None);
        }

        // Parse on a copy: the decoder consumes bytes as it goes, so if the
        // frame turns out to be incomplete we must not lose them.
        let mut attempt = src.clone();
        match decode_redis_value(&mut attempt) {
            Ok(value) => {
                let consumed = src.len() - attempt.len();
                src.advance(consumed);
                Ok(Some(value))
            }
            // Not enough bytes yet — wait for the next read from the socket.
            Err(CliErrors::Incomplete) => Ok(None),
            Err(e) => Err(e),
        }
    }
}

impl Encoder<RedisValue> for RedisCodec {
    type Error = CliErrors;

    fn encode(&mut self, item: RedisValue, dst: &mut BytesMut) -> Result<(), CliErrors> {
        let bytes = encode_redis_value(item)?;
        dst.extend_from_slice(&bytes);
        Ok(())
    }
}

pub struct CmdParser {
    tokens: Vec<char>,
    cur: usize,
}

impl CmdParser {
    pub fn new(cmd: &str) -> Self {
        Self {
            tokens: cmd.chars().collect(),
            cur: 0,
        }
    }

    pub fn parse(&mut self) -> Result<Vec<String>, CliErrors> {
        if self.tokens.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        let mut chars = Vec::new();

        loop {
            if self.is_end() {
                break;
            }

            let tk = self.tokens[self.cur];
            match tk {
                ' ' | '\r' | '\n' | '\t' => {
                    self.cur += 1;

                    if !chars.is_empty() {
                        results.push(chars.drain(..).collect());
                    }

                    continue;
                }
                '"' => {
                    if !chars.is_empty() {
                        results.push(chars.drain(..).collect());
                    }
                    results.push(self.read_string()?);
                }
                _ => {
                    chars.push(tk);
                    self.cur += 1;
                }
            }
        }

        if !chars.is_empty() {
            results.push(String::from_iter(chars));
        }

        Ok(results)
    }

    fn is_end(&self) -> bool {
        self.cur >= self.tokens.len()
    }

    fn consume_char(&mut self) -> Option<char> {
        if !self.is_end() {
            let ch = Some(self.tokens[self.cur]);
            self.cur += 1;
            return ch;
        }

        None
    }

    fn read_string(&mut self) -> Result<String, CliErrors> {
        // skip the opening quote
        self.cur += 1;
        let mut chars = Vec::new();
        let mut meet_end = false;

        while let Some(ch) = self.consume_char() {
            if ch == '"' {
                meet_end = true;
                break;
            }

            chars.push(ch);
        }

        if !meet_end {
            return Err(CliErrors::UnterminatedString);
        }

        Ok(String::from_iter(chars))
    }
}

pub fn parse_cmd_to_strings(cmd: &str) -> Result<Vec<String>, CliErrors> {
    let mut parser = CmdParser::new(cmd);
    let values = parser.parse()?;

    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_command() {
        let result = parse_cmd_to_strings("GET key").unwrap();
        assert_eq!(result, ["GET", "key"]);
    }

    #[test]
    fn ignores_extra_whitespace() {
        let result = parse_cmd_to_strings("  GET   key  ").unwrap();
        assert_eq!(result, ["GET", "key"]);
    }

    #[test]
    fn parses_quoted_string_with_spaces() {
        let result = parse_cmd_to_strings("SET greeting \"hello world\"").unwrap();
        assert_eq!(result, ["SET", "greeting", "hello world"]);
    }

    #[test]
    fn parses_multiple_quoted_strings() {
        let result = parse_cmd_to_strings("SET \"my key\" \"my value\"").unwrap();
        assert_eq!(result, ["SET", "my key", "my value"]);
    }

    #[test]
    fn rejects_unterminated_string() {
        let result = parse_cmd_to_strings("SET key \"oops");
        assert!(matches!(result, Err(CliErrors::UnterminatedString)));
    }

    #[test]
    fn empty_input_yields_no_tokens() {
        assert_eq!(parse_cmd_to_strings("").unwrap(), Vec::<String>::new());
        assert_eq!(parse_cmd_to_strings("   ").unwrap(), Vec::<String>::new());
    }

    #[test]
    fn strips_crlf() {
        let result = parse_cmd_to_strings("PING\r\n").unwrap();
        assert_eq!(result, ["PING"]);
    }

    #[test]
    fn codec_waits_for_more_bytes_on_partial_frame() {
        let mut codec = RedisCodec;
        let mut buf = BytesMut::from(&b"$5\r\nhel"[..]);

        assert_eq!(codec.decode(&mut buf).unwrap(), None);
        // Buffer must be untouched, ready for the rest of the frame
        assert_eq!(&buf[..], b"$5\r\nhel");
    }

    #[test]
    fn codec_decodes_frame_arriving_in_two_chunks() {
        let mut codec = RedisCodec;
        let mut buf = BytesMut::from(&b"$5\r\nhel"[..]);
        assert_eq!(codec.decode(&mut buf).unwrap(), None);

        buf.extend_from_slice(b"lo\r\n");
        let value = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(value, RedisValue::BulkString("hello".to_string()));
        assert!(buf.is_empty());
    }

    #[test]
    fn codec_decodes_multiple_frames_from_one_buffer() {
        let mut codec = RedisCodec;
        let mut buf = BytesMut::from(&b"+OK\r\n:42\r\n"[..]);

        let first = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(first, RedisValue::SimpleString("OK".to_string()));

        let second = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(second, RedisValue::Integer(42));

        assert_eq!(codec.decode(&mut buf).unwrap(), None);
    }

    #[test]
    fn codec_encode_decode_round_trip() {
        let mut codec = RedisCodec;
        let value = RedisValue::Array(vec![
            RedisValue::BulkString("SET".to_string()),
            RedisValue::BulkString("key".to_string()),
            RedisValue::BulkString("value".to_string()),
        ]);

        let mut buf = BytesMut::new();
        codec.encode(value.clone(), &mut buf).unwrap();
        assert_eq!(&buf[..], b"*3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$5\r\nvalue\r\n");

        let decoded = codec.decode(&mut buf).unwrap().unwrap();
        assert_eq!(decoded, value);
    }
}
