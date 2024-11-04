use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::str::FromStr;

#[derive(Debug)]
enum RedisCommand {
    Ping,
    Echo(String),
    Unknown,
}

impl FromStr for RedisCommand {
    type Err = ();

    fn from_str(command: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = command.lines().collect();

        match parts.as_slice() {
            // Matches a PING command: *1\r\n$4\r\nPING\r\n
            ["*1", "$4", "PING"] => Ok(RedisCommand::Ping),

            // Matches an ECHO command: *2\r\n$4\r\nECHO\r\n$<len>\r\n<message>\r\n
            ["*2", "$4", "ECHO", len, message] if len.starts_with('$') => {
                Ok(RedisCommand::Echo(message.to_string()))
            }

            // If the command doesn't match known patterns
            _ => Ok(RedisCommand::Unknown),
        }
    }
}

#[tokio::main]
async fn main() -> tokio::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379").await?;
    println!("Server listening on port 6379");

    loop {
        let (mut stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("Failed to accept connection: {:?}", e);
                continue;
            }
        };

        tokio::spawn(async move {
            let mut buf = vec![0; 512];
            loop {
                match stream.read(&mut buf).await {
                    Ok(0) => break, // Client closed connection
                    Ok(n) => {
                        if let Some(response) = handle_command(&buf[..n]) {
                            if stream.write_all(response.as_bytes()).await.is_err() {
                                eprintln!("Failed to send response");
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read from stream: {:?}", e);
                        break;
                    }
                }
            }
        });
    }
}

fn handle_command(input: &[u8]) -> Option<String> {
    let input_str = String::from_utf8_lossy(input);
    match input_str.parse::<RedisCommand>() {
        Ok(RedisCommand::Ping) => Some("+PONG\r\n".to_string()),
        Ok(RedisCommand::Echo(message)) => Some(format!("${}\r\n{}\r\n", message.len(), message)),
        Ok(RedisCommand::Unknown) => Some("-ERR unknown command\r\n".to_string()),
        Err(_) => None,
    }
}
