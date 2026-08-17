use tokio::net::TcpStream;

#[derive(Debug)]
pub struct RedisClient {
    pub stream: TcpStream,
}
