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

    if let Some((key, value)) = parse_rdb_key_value(&buffer) {
        let mut store = store.lock().unwrap();
        store.insert(key, (value, None)); // Set expiry as None for this stage
    }

    Ok(())
}

fn parse_rdb_key_value(buffer: &[u8]) -> Option<(String, String)> {
    if buffer.starts_with(b"REDIS0011") {
        let mut pos = 9; // Start after the "REDIS0011" header
        
        while pos < buffer.len() {
            match buffer[pos] {
                0xFE => { // Database section marker
                    pos += 2; 
                },
                0x00 => { // Start of a key-value entry
                    pos += 1; 
                    
                    if let Some((key, new_pos)) = read_string(buffer, pos) {
                        pos = new_pos;
                        if let Some((value, _new_pos)) = read_string(buffer, pos) {
                            if !key.is_empty() && !value.is_empty() {
                                return Some((key, value));
                            }
                        }
                    }
                    break;
                },
                0xFF => break, 
                _ => pos += 1,
            }
        }
    }
    None
}



// Helper function to read a size-encoded string from the buffer
fn read_string(buffer: &[u8], pos: usize) -> Option<(String, usize)> {
    if pos >= buffer.len() {
        return None;
    }

    let size = buffer[pos] as usize;
    let start = pos + 1;
    let end = start + size;

    if end <= buffer.len() {
        if let Ok(string) = String::from_utf8(buffer[start..end].to_vec()) {
            return Some((string, end));
        }
    }

    None
}

