use futures::{SinkExt, StreamExt};
use redis_from_scratch::{RedisValue, codec::RedisCodec, errors::CliErrors};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader, Stdin},
    net::TcpStream,
};
use tokio_util::codec::Framed;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Redis Client Built with Rust");

    let addr = "127.0.0.1:6379";

    let stream = TcpStream::connect(addr).await?;
    let mut framed = Framed::new(stream, RedisCodec);

    let stdin = io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut stdout = io::stdout();

    loop {
        stdout.write_all(b"> ").await?;
        stdout.flush().await?;

        let (cmd, end) = read_line(&mut reader).await?;

        if end {
            break;
        }

        let request = parse_request(&cmd)?;
        let Some(request) = request else {
            continue;
        };

        framed.send(request).await?;

        match framed.next().await {
            Some(Ok(value)) => {
                stdout.write_all(format!("{}\n", value).as_bytes()).await?;
            }
            Some(Err(e)) => {
                stdout
                    .write_all(format!("(codec error) {}\n", e).as_bytes())
                    .await?;
            }
            None => {
                stdout.write_all(b"server closed connection\n").await?;
                break;
            }
        }
    }

    Ok(())
}

/// Parse a line of user input into a RESP array of bulk strings,
/// e.g. `SET key "a b"` -> `*3\r\n$3\r\nSET\r\n$3\r\nkey\r\n$3\r\na b\r\n`.
fn parse_request(cmd: &str) -> Result<Option<RedisValue>, CliErrors> {
    let tokens = redis_from_scratch::codec::parse_cmd_to_strings(cmd)?;

    if tokens.is_empty() {
        return Ok(None);
    }

    Ok(Some(RedisValue::Array(
        tokens.into_iter().map(RedisValue::BulkString).collect(),
    )))
}

async fn read_line(stdin: &mut BufReader<Stdin>) -> Result<(String, bool), CliErrors> {
    let mut line = String::new();
    let n = stdin.read_line(&mut line).await?;

    if n == 0 {
        return Ok(("".to_string(), true));
    }

    let cmd = line.trim_end_matches(['\r', '\n']);

    Ok((cmd.to_string(), false))
}
