use std::str::FromStr;
use std::time::Duration;


#[derive(Debug)]
pub enum RedisCommand {
    Ping,
    Echo(String),
    Set { key: String, value: String, expiry: Option<Duration> },
    Get(String),
    ConfigGet(String),
    Keys(String),
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
            ["*2", "$4", "KEYS", "$1", pattern] if *pattern == "*" => {
                Ok(RedisCommand::Keys(pattern.to_string())) // Handle KEYS "*"
            }
            // Updated SET with PX pattern
            ["*5", "$3", "SET", key_len, key, value_len, value, px_len, px, expiry_len, expiry]
                if key_len.starts_with('$')
                    && value_len.starts_with('$')
                    && px_len.starts_with('$')
                    && expiry_len.starts_with('$')
                    && px.to_lowercase() == "px" =>
            {
                let expiry_ms = expiry.parse::<u64>().ok();
                let expiry = expiry_ms.map(Duration::from_millis);
                Ok(RedisCommand::Set { key: key.to_string(),
                    value: value.to_string(), expiry })
            }
            ["*3", "$3", "SET", key_len, key, value_len, value]
                if key_len.starts_with('$') && value_len.starts_with('$') =>
            {
                Ok(RedisCommand::Set { key: key.to_string(),
                    value: value.to_string(), expiry: None })
            }
            ["*2", "$3", "GET", key_len, key]
                if key_len.starts_with('$') => {
                    Ok(RedisCommand::Get(key.to_string()))
            }
            ["*3", "$6", "CONFIG", "$3", "GET", param_len, param]
                if param_len.starts_with('$') => {
                Ok(RedisCommand::ConfigGet(param.to_string()))
            }
            _ => {
                Ok(RedisCommand::Unknown)
            }
        }
    }
}