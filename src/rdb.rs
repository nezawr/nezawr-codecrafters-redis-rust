use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub fn load_rdb_file(
    store: &Arc<Mutex<HashMap<String, (String, Option<SystemTime>)>>>,
    dir: &Option<String>,
    dbfilename: &Option<String>,
) -> io::Result<()> {
    let path = match (dir, dbfilename) {
        (Some(d), Some(f)) => Path::new(d).join(f),
        _ => return Ok(()), // If no file provided, skip loading
    };

    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return Ok(()), // If file doesn't exist, treat db as empty
    };

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Parse the RDB file and get all key-value pairs
    if let Some(key_value_pairs) = parse_rdb_key_value(&buffer) {
        let mut store = store.lock().unwrap();
        for (key, value, expiry) in key_value_pairs {
            if let Some(expiry_time) = expiry {
                if SystemTime::now() > expiry_time {
                    continue;
                }
            }

            store.insert(key.clone(), (value.clone(), None)); // Set expiry as None for this stage
            println!("Inserted key: {}, value: {}", key, value);
        }
    } else {
        println!("No key-value pair found in RDB file");
    }

    Ok(())
}

fn read_string(buffer: &[u8], pos: usize, length: usize) -> (String, usize) {
    // Convert the slice to a String. Adjust error handling as needed.
    let string = String::from_utf8(buffer[pos..pos + length].to_vec())
        .expect("Failed to convert buffer to String");

    // Return the String and the new position (pos + length)
    (string, pos + length)
}

fn parse_unix_time(buffer: &[u8], pos: usize) -> SystemTime {
    let timestamp = u64::from_le_bytes(buffer[pos..pos+8].try_into().unwrap());
    UNIX_EPOCH + Duration::from_millis(timestamp)
}

fn handle_keys_values(buffer: &[u8], pos: &mut usize, keys_values: &mut Vec<(String, String, Option<SystemTime>)>) {
    while *pos < buffer.len() && buffer[*pos] != 0xFF {
        let mut expiry = None;
        // Indicates that this key has an expire
        if buffer[*pos] == 0xFC {
            expiry = Some(parse_unix_time(buffer, *pos + 1));
            *pos += 9;
        }
        // The 1-byte flag that specifies the value’s type and encoding
        *pos += 1;
        let key_length = buffer[*pos] as usize;
        let (key, new_pos) = read_string(&buffer, *pos + 1, key_length);
        let value_length = buffer[new_pos] as usize;
        let (value, new_pos) = read_string(&buffer, new_pos + 1, value_length);
        keys_values.push((key, value, expiry));
        *pos = new_pos; // Update the position for the next iteration
    }
}

fn parse_rdb_key_value(buffer: &[u8]) -> Option<Vec<(String, String, Option<SystemTime>)>> {
    let mut keys_values = Vec::new();
    let mut pos = 9; // Start after the header "REDIS0011"

    while pos < buffer.len() {
        match buffer[pos] {
            0xFE => { // Database section marker
                pos += 2;
                // Skip over hash table resize information (0xFB)
                if buffer[pos] == 0xFB {
                    pos += 3; // Skip the 4 bytes of hash table size info
                }
                handle_keys_values(buffer, &mut pos, &mut keys_values);
                break;
            }
            _ => {
                pos += 1; // Skip any other unexpected sections
            }
        }
    }

    if keys_values.is_empty() {
        println!("No key-value pairs found after parsing the RDB file");
        None
    } else {
        Some(keys_values)
    }
}