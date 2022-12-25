use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;

#[derive(Serialize, Deserialize)]
pub struct ApiKey {
    key: String,
    secret: String,
}

impl ApiKey {
    pub fn new(key: String, secret: String) -> Self {
        Self { key, secret }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn secret(&self) -> &str {
        &self.secret
    }

    pub fn read_json<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let file = File::open(path.as_ref())?;
        let apikey = serde_json::from_reader(file)?;
        Ok(apikey)
    }
}
