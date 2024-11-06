use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::str::FromStr;
use std::time::{SystemTime, Duration};

#[derive(Debug)]
enum RedisCommand {
    Ping,
    Echo(String),
    Set { key: String, value: String, expiry: Option<Duration> },
    Get(String),
    Unknown,
}

impl FromStr for RedisCommand {
    type Err = ();

    fn from_str(command: &str) -> Result<Self, Self::Err> {
        println!("Received command: {:?}", command); // Debugging output
        let parts: Vec<&str> = command.lines().collect();
        println!("Command parts: {:?}", parts); // Debugging output

        match parts.as_slice() {
            ["*1", "$4", "PING"] => Ok(RedisCommand::Ping),
            ["*2", "$4", "ECHO", len, message]
                if len.starts_with('$') => {
                    Ok(RedisCommand::Echo(message.to_string()))
            }
            ["*5", "$3", "SET", key_len, key, value_len, value, px,
                expiry_len, expiry] if key_len.starts_with('$')
                && value_len.starts_with('$')
                && expiry_len.starts_with('$')
                && px.to_lowercase() == "px" =>
            {
                let expiry_ms = expiry.parse::<u64>().ok();
                let expiry = expiry_ms.map(Duration::from_millis);
                println!("Parsed SET with PX: key={}, value={}, expiry={:?}", key, value, expiry); // Debugging output
                Ok(RedisCommand::Set { key: key.to_string(),
                    value: value.to_string(), expiry })
            }
            ["*3", "$3", "SET", key_len, key, value_len, value]
                if key_len.starts_with('$') && value_len.starts_with('$') =>
            {
                println!("Parsed SET without PX: key={}, value={}", key, value); // Debugging output
                Ok(RedisCommand::Set { key: key.to_string(),
                    value: value.to_string(), expiry: None })
            }
            ["*2", "$3", "GET", key_len, key]
                if key_len.starts_with('$') => {
                    Ok(RedisCommand::Get(key.to_string()))
            }
            _ => {
                println!("Unknown command structure: {:?}", parts); // Debugging output
                Ok(RedisCommand::Unknown)
            }
        }
    }
}


#[tokio::main]
async fn main() -> tokio::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379").await?;
    println!("Server listening on port 6379");

    let store: Arc<Mutex<HashMap<String, (String, Option<SystemTime>)>>> =
        Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (mut stream, _) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                eprintln!("Failed to accept connection: {:?}", e);
                continue;
            }
        };

        let store = Arc::clone(&store);

        tokio::spawn(async move {
            let mut buf = vec![0; 512];
            loop {
                match stream.read(&mut buf).await {
                    Ok(0) => break, // Client closed connection
                    Ok(n) => {
                        if let Some(response) =
                            handle_command(&buf[..n], &store)
                        {
                            if stream.write_all(response.as_bytes())
                                .await.is_err()
                            {
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

fn handle_command(
    input: &[u8],
    store: &Arc<Mutex<HashMap<String, (String, Option<SystemTime>)>>>
) -> Option<String> {
    let input_str = String::from_utf8_lossy(input);
    println!("Received command in handle_command: {}", input_str); // Debug output

    match input_str.parse::<RedisCommand>() {
        Ok(RedisCommand::Ping) => {
            println!("Handling PING command"); // Debug output
            Some("+PONG\r\n".to_string())
        }
        Ok(RedisCommand::Echo(message)) => {
            println!("Handling ECHO command with message: {}", message); // Debug output
            Some(format!("${}\r\n{}\r\n", message.len(), message))
        }
        Ok(RedisCommand::Set { key, value, expiry }) => {
            println!("Handling SET command with key: {}, value: {}, expiry: {:?}", key, value, expiry); // Debug output
            let mut store = store.lock().unwrap();
            let expiry_time = expiry.map(|dur| SystemTime::now() + dur);
            store.insert(key, (value, expiry_time));
            Some("+OK\r\n".to_string())
        }
        Ok(RedisCommand::Get(key)) => {
            println!("Handling GET command for key: {}", key); // Debug output
            let mut store = store.lock().unwrap();
            if let Some((value, expiry)) = store.get(&key) {
                if let Some(expiry_time) = expiry {
                    if SystemTime::now() > *expiry_time {
                        println!("Key {} has expired", key); // Debug output
                        store.remove(&key);
                        return Some("$-1\r\n".to_string());
                    }
                }
                Some(format!("${}\r\n{}\r\n", value.len(), value))
            } else {
                println!("Key {} does not exist", key); // Debug output
                Some("$-1\r\n".to_string())
            }
        }
        Ok(RedisCommand::Unknown) => {
            println!("Received unknown command"); // Debug output
            Some("-ERR unknown command\r\n".to_string())
        }
        Err(_) => {
            println!("Failed to parse command"); // Debug output
            None
        }
    }
}

