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

    let config_for_load  = Arc::clone(&config);
    {
        let config_lock = config_for_load.lock().unwrap();
        load_rdb_file(&store, &config_lock.dir, &config_lock.dbfilename)
            .expect("Failed to load RDB file");
    }

    {
        let config_lock = config.lock().unwrap();

        // If the server is configured as a replica, initiate a connection to the master
        if let Some(replica_of) = config_lock.replica.clone() {
            let parts: Vec<&str> = replica_of.split_whitespace().collect();
            if parts.len() == 2 {
                let master_host = parts[0].to_string(); // Clone as `String` for 'static lifetime
                let master_port = parts[1].to_string(); // Clone as `String` for 'static lifetime
                let replica_port = config_lock.port.clone(); // Use the replica's own port

                tokio::spawn(async move {
                    if let Err(e) = perform_replica_handshake(master_host, master_port, replica_port).await {
                        eprintln!("Handshake failed: {:?}", e);
                    }
                });
            } else {
                eprintln!("Invalid replica configuration: {}", replica_of);
            }
        }
    }

    let listener;
    {
        let config_lock = config.lock().unwrap();
        listener = TcpListener::bind(format!("127.0.0.1:{}", config_lock.port)).await?;
        println!("Server listening on port {}", config_lock.port);
    }


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


/// Handles the replica-to-master handshake process
async fn perform_replica_handshake(master_host: String, master_port: String, replica_port: String) -> tokio::io::Result<()> {
    match tokio::net::TcpStream::connect(format!("{}:{}", master_host, master_port)).await {
        Ok(mut master_stream) => {
            println!("Connected to master at {}:{}", master_host, master_port);

            // 1. Send PING
            let ping_command = "*1\r\n$4\r\nPING\r\n";
            master_stream.write_all(ping_command.as_bytes()).await?;
            println!("Sent PING command to master");

            // Wait for +PONG
            let mut buffer = vec![0; 512];
            let n = master_stream.read(&mut buffer).await?;
            if &buffer[..n] != b"+PONG\r\n" {
                eprintln!("Unexpected response to PING: {:?}", &buffer[..n]);
                return Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, "Unexpected PING response"));
            }
            println!("Received PONG from master");

            // 2. Send REPLCONF listening-port <PORT>
            let replconf_listening_port = format!(
                "*3\r\n$8\r\nREPLCONF\r\n$14\r\nlistening-port\r\n${}\r\n{}\r\n",
                replica_port.len(),
                replica_port
            );
            master_stream.write_all(replconf_listening_port.as_bytes()).await?;
            println!("Sent REPLCONF listening-port command");

            // Wait for +OK
            let n = master_stream.read(&mut buffer).await?;
            if &buffer[..n] != b"+OK\r\n" {
                eprintln!("Unexpected response to REPLCONF listening-port: {:?}", &buffer[..n]);
                return Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, "Unexpected REPLCONF listening-port response"));
            }
            println!("Received OK for REPLCONF listening-port");

            // 3. Send REPLCONF capa psync2
            let replconf_capa_psync2 = "*3\r\n$8\r\nREPLCONF\r\n$4\r\ncapa\r\n$6\r\npsync2\r\n";
            master_stream.write_all(replconf_capa_psync2.as_bytes()).await?;
            println!("Sent REPLCONF capa psync2 command");

            // Wait for +OK
            let n = master_stream.read(&mut buffer).await?;
            if &buffer[..n] != b"+OK\r\n" {
                eprintln!("Unexpected response to REPLCONF capa psync2: {:?}", &buffer[..n]);
                return Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, "Unexpected REPLCONF capa psync2 response"));
            }
            println!("Received OK for REPLCONF capa psync2");

            // 4. Send PSYNC ? -1
            let psync_command = "*3\r\n$5\r\nPSYNC\r\n$1\r\n?\r\n$2\r\n-1\r\n";
            master_stream.write_all(psync_command.as_bytes()).await?;
            println!("Sent PSYNC command");

            // Wait for +FULLRESYNC response
            let n = master_stream.read(&mut buffer).await?;
            let response = String::from_utf8_lossy(&buffer[..n]);
            if response.starts_with("+FULLRESYNC") {
                println!("Received FULLRESYNC response: {}", response);
            } else {
                eprintln!("Unexpected response to PSYNC: {}", response);
                return Err(tokio::io::Error::new(tokio::io::ErrorKind::Other, "Unexpected PSYNC response"));
            }

            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to connect to master: {:?}", e);
            Err(e)
        }
    }
}