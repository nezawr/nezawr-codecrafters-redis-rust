use std::env;

#[derive(Debug)]
pub struct ServerConfig {
    pub dir: Option<String>,
    pub dbfilename: Option<String>,
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

        ServerConfig { dir, dbfilename }
    }
}