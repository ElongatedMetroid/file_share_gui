use std::fs;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub server: Option<Server>,
    pub client: Option<Client>,
}

#[derive(Deserialize, PartialEq)]
pub struct Server {
    thread_count: usize,
    ips: Vec<String>,

    max_share_size_without_file: Option<u64>,
    max_file_size: Option<u64>,

    return_on_success: Option<String>,
    return_on_help: Option<String>,
}

#[derive(Deserialize, PartialEq)]
pub struct Client {
    server: String,

    retry_delay: u64,
    retry_amount: usize,
}

impl Config {
    pub fn build(path: &str) -> Result<Config, Box<dyn std::error::Error>> {
        Ok(toml::from_str(&fs::read_to_string(path)?)?)
    }
    pub fn server(self) -> Result<Server, Box<dyn std::error::Error>> {
        if let Some(server) = self.server {
            return Ok(server);
        } 

        Err("Server configuration is empty".into())
    }
    pub fn client(self) -> Result<Client, Box<dyn std::error::Error>> {
        if let Some(client) = self.client {
            return Ok(client);
        } 

        Err("Client configuration is empty".into())
    }
}

impl Server {
    pub fn thread_count(&self) -> usize {
        self.thread_count
    }
    pub fn ip(&self) -> &str {
        &self.ips[0]
    }
    pub fn ip_backups(&self) -> &Vec<String> {
        &self.ips
    }
}

impl Client {
    pub fn server(&self) -> &str {
        &self.server
    }
    pub fn retry_amount(&self) -> usize {
        self.retry_amount - 1
    }
    pub fn retry_delay(&self) -> u64 {
        self.retry_delay
    }
}