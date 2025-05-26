use dotenv::dotenv;
use std::env;

#[derive(Debug)]
pub struct Config {
    pub private_key: String,
    pub rpc_url: String,
}

impl Config {
    pub fn load() -> Self {
        dotenv().ok(); // Load .env file, if present
        let private_key = env::var("PRIVATE_KEY").expect("PRIVATE_KEY must be set in .env");
        let rpc_url = env::var("RPC_URL").expect("RPC_URL must be set in .env");
        Config {
            private_key,
            rpc_url,
        }
    }
}