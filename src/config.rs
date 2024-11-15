use std::env;

#[derive(Debug)]
pub struct ServerConfig {
    pub dir: Option<String>,
    pub dbfilename: Option<String>,
    pub port: String
}

impl ServerConfig {
    pub fn new_from_args() -> Self {
        let args: Vec<String> = env::args().collect();
    let dir = args.iter().position(|x| x == "--dir")
        .and_then(|pos| args.get(pos + 1))
        .map(|s| s.to_string());

    let dbfilename = args.iter().position(|x| x == "--dbfilename")
        .and_then(|pos| args.get(pos + 1))
        .map(|s| s.to_string());

    let port = args.iter().position(|x| x == "--port")
        .and_then(|pos| args.get(pos + 1))
        .map(|s| s.to_string())
        .unwrap_or_else(|| "6379".to_string());

        ServerConfig { dir, dbfilename, port }
    }
}