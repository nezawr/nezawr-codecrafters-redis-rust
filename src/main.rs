fn handle_command(
    input: &[u8],
    store: &Arc<Mutex<HashMap<String, (String, Option<SystemTime>)>>>,
) -> Option<String> {
    let input_str = String::from_utf8_lossy(input);
    match input_str.parse::<RedisCommand>() {
        Ok(RedisCommand::Ping) => Some("+PONG\r\n".to_string()),
        Ok(RedisCommand::Echo(message)) => Some(format!("${}\r\n{}\r\n", message.len(), message)),
        Ok(RedisCommand::Set { key, value, expiry }) => {
            // Acquire a lock and store the key-value pair with optional expiry
            let mut store = store.lock().unwrap();
            let expiry_time = expiry.map(|dur| SystemTime::now() + dur);
            store.insert(key, (value, expiry_time));
            Some("+OK\r\n".to_string())
        }
        Ok(RedisCommand::Get(key)) => {
            let mut store = store.lock().unwrap();
            if let Some((value, expiry)) = store.get(&key) {
                // Check for expiry time if it exists
                if let Some(expiry_time) = expiry {
                    if SystemTime::now() > *expiry_time {
                        store.remove(&key); // Key has expired, so remove it
                        return Some("$-1\r\n".to_string()); // Return null bulk string
                    }
                }
                Some(format!("${}\r\n{}\r\n", value.len(), value)) // Return value if not expired
            } else {
                Some("$-1\r\n".to_string()) // Key does not exist
            }
        }
        Ok(RedisCommand::Unknown) => Some("-ERR unknown command\r\n".to_string()),
        Err(_) => None,
    }
}

