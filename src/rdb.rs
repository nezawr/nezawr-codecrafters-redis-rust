use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

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
        for (key, value) in key_value_pairs {
            store.insert(key.clone(), (value.clone(), None)); // Set expiry as None for this stage
            println!("Inserted key: {}, value: {}", key, value);
        }
    } else {
        println!("No key-value pair found in RDB file");
    }

    Ok(())
}

fn parse_rdb_key_value(buffer: &[u8]) -> Option<Vec<(String, String)>> {
    let mut keys_values = Vec::new();
    let mut pos = 9; // Start after the header "REDIS0011"

    while pos < buffer.len() {
        match buffer[pos] {
            0xFE => { // Database section marker
                println!("Found database section marker at position {}", pos);
                pos += 2;
                println!("wherei am at: {}:{}", pos, buffer[pos]);
                // Skip over hash table resize information (0xFB)
                if buffer[pos] == 0xFB {
                    pos += 4; // Skip the 4 bytes of hash table size info
                    println!("Skipped resize hash table info at position {}", pos);
                }
                println!("where i am at: {}:{}", pos, buffer[pos]);
                let length: usize = buffer[pos] as usize;
                let (key, new_pos) = read_string(&buffer, pos+1, length);
                let length: usize =  buffer[new_pos] as usize;
                let (value, new_pos) = read_string(&buffer, new_pos + 1, length);
                keys_values.push((key, value));
            }
            , _ => {
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

fn read_string(buffer: &[u8], pos: usize, length: usize) -> (String, usize) {
    // Convert the slice to a String. Adjust error handling as needed.
    let string = String::from_utf8(buffer[pos..pos + length].to_vec())
        .expect("Failed to convert buffer to String");

    // Return the String and the new position (pos + length)
    (string, pos + length)
}

// Decode length based on Redis size-encoding
fn decode_length(buffer: &[u8], pos: usize) -> Option<(usize, usize)> {
    let byte = buffer[pos];
    let length = match byte >> 6 {
        0b00 => (byte & 0x3F) as usize, // 6-bit length
        0b01 => {
            let next_byte = buffer.get(pos + 1)?;
            (((byte & 0x3F) as usize) << 8 | (*next_byte as usize)) as usize // 14-bit length
        },
        0b10 => {
            let length_bytes = &buffer[pos + 1..pos + 5];
            u32::from_be_bytes(length_bytes.try_into().ok()?) as usize // 32-bit length
        },
        _ => {
            println!("Unexpected length encoding at position {}", pos);
            return None;
        }
    };

    println!("Decoded length: {}, at position: {}", length, pos);
    Some((length, pos + 1)) // Return length and new position
}


pub fn print_rdb_file_contents(dir: &Option<String>, dbfilename: &Option<String>) -> io::Result<()> {
    let path = match (dir, dbfilename) {
        (Some(d), Some(f)) => Path::new(d).join(f),
        _ => return Ok(()), // If no file provided, skip printing
    };

    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to open file: {}", e);
            return Ok(()); // If file doesn't exist, skip
        }
    };

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;

    // Print the file contents in hex format
    println!("RDB File Contents (hex):");
    for (i, byte) in buffer.iter().enumerate() {
        print!("{:02X} ", byte);
        if (i + 1) % 16 == 0 {
            println!(); // New line every 16 bytes for readability
        }
    }
    println!(); // Final newline after printing
    Ok(())
}