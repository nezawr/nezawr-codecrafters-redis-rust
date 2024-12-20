use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::SystemTime;
use crate::config::ServerConfig;
use crate::command::RedisCommand;

pub fn handle_command(
    input: &[u8],
    store: &Arc<Mutex<HashMap<String, (String, Option<SystemTime>)>>>,
    config: &Arc<Mutex<ServerConfig>>,
) -> Option<String> {
    let input_str = String::from_utf8_lossy(input);
    match input_str.parse::<RedisCommand>() {
        Ok(RedisCommand::Keys(_)) => {
            let store = store.lock().unwrap();
            let keys: Vec<&String> = store.keys().collect();
            println!("Keys found in store: {:?}", keys); // Debugging output
            let mut response = format!("*{}\r\n", keys.len());
            for key in keys {
                response.push_str(&format!("${}\r\n{}\r\n", key.len(), key));
            }
            Some(response)
        },
        Ok(RedisCommand::Ping) => Some("+PONG\r\n".to_string()),
        Ok(RedisCommand::Echo(message)) => Some(format!("${}\r\n{}\r\n",
            message.len(), message)),
        Ok(RedisCommand::Set { key, value, expiry }) => {
            let mut store = store.lock().unwrap();
            let expiry_time = expiry.map(|dur| SystemTime::now() + dur);
            store.insert(key, (value, expiry_time));
            Some("+OK\r\n".to_string())
        }
        Ok(RedisCommand::Get(key)) => {
            let mut store = store.lock().unwrap();
            if let Some((value, expiry)) = store.get(&key) {
                if let Some(expiry_time) = expiry {
                    if SystemTime::now() > *expiry_time {
                        store.remove(&key);
                        return Some("$-1\r\n".to_string());
                    }
                }
                Some(format!("${}\r\n{}\r\n", value.len(), value))
            } else {
                Some("$-1\r\n".to_string())
            }
        }

        Ok(RedisCommand::ConfigGet(param)) => {
            let config = config.lock().unwrap();
            match param.as_str() {
                "dir" => match &config.dir {
                    Some(dir) => Some(format!("*2\r\n$3\r\ndir\r\n${}\r\n{}\r\n", dir.len(), dir)),
                    None => Some("ERR dir not configured\r\n".to_string()),
                }
                "dbfilename" => match &config.dbfilename {
                    Some(dbfilename) => Some(format!("*2\r\n$10\r\ndbfilename\r\n${}\r\n{}\r\n", dbfilename.len(), dbfilename)),
                    None => Some("-ERR dbfilename not configured\r\n".to_string()),
                },
                _ => Some("-ERR unknown parameter\r\n".to_string()),

            }
        }

        Ok(RedisCommand::Info) => {
            let config = config.lock().unwrap();

            if config.replica.is_some() {
                // Slave role
                Some("$10\r\nrole:slave\r\n".to_string())
            } else {
                // Master role
                let master_replid = "8371b4fb1155b71f4a04d3e1bc3e18c4a990aeeb"; // 40-char string
                let master_repl_offset = 0; // Offset is 0

                // Correct length calculation for the bulk string
                let content = format!(
                    "\r\nrole:master\r\nmaster_replid:{}\r\nmaster_repl_offset:{}\r\n",
                    master_replid, master_repl_offset
                );

                Some(format!("$89{}", content))
            }
        }

        Ok(RedisCommand::Replconf) => {
            Some("+OK\r\n".to_string())
        }

        Ok(RedisCommand::Psync { repl_id, offset }) => {
            let master_repl_id = "8371b4fb1155b71f4a04d3e1bc3e18c4a990aeeb";
            let master_offset = 0;
            Some(format!(
                "+FULLRESYNC {} {}\r\n",
                master_repl_id, master_offset
            ))
        }

        Ok(RedisCommand::Unknown) =>
            Some("-ERR unknown command\r\n".to_string()),
        Err(_) => None,
    }
}