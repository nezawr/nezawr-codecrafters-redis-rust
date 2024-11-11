mod config;
mod command;
mod handler;
mod rdb;

use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use crate::config::ServerConfig;
use crate::handler::handle_command;
use crate::rdb::load_rdb_file;

#[tokio::main]
async fn main() -> tokio::io::Result<()> {
    let config: Arc<Mutex<ServerConfig>> = Arc::new(Mutex::new(ServerConfig::new_from_args()));
    let store: Arc<Mutex<HashMap<String, (String, Option<SystemTime>)>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Load RDB file into the store if it exists
    {
        let config = config.lock().unwrap();
        load_rdb_file(&store, &config.dir, &config.dbfilename).expect("Failed to load RDB file");
    }

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

        let config = Arc::clone(&config);
        let store = Arc::clone(&store);

        tokio::spawn(async move {
            let mut buf = vec![0; 512];
            loop {
                match stream.read(&mut buf).await {
                    Ok(0) => break, // Client closed connection
                    Ok(n) => {
                        if let Some(response) =
                            handle_command(&buf[..n], &store, &config)
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
