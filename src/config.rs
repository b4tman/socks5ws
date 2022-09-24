use std::{env, fs, io::Read, path::PathBuf};

use serde_derive::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct Config {
    /// Bind on address address. eg. `127.0.0.1:1080`
    pub listen_addr: String,
    /// Request timeout
    pub request_timeout: u64,
    /// Authentication
    #[serde(default)]
    pub auth: Option<PasswordAuth>,
    /// Avoid useless roundtrips if we don't need the Authentication layer
    #[serde(default)]
    pub skip_auth: bool,
    /// Enable dns-resolving
    #[serde(default = "default_true")]
    pub dns_resolve: bool,
    /// Enable command execution
    #[serde(default = "default_true")]
    pub execute_command: bool,
    /// Enable UDP support
    #[serde(default = "default_true")]
    pub allow_udp: bool,
}

fn default_true() -> bool {
    true
}

/// Password authentication data
#[derive(Clone, Deserialize, Debug)]
pub struct PasswordAuth {
    pub username: String,
    pub password: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            listen_addr: "127.0.0.1:1080".into(),
            request_timeout: 120,
            auth: None,
            skip_auth: false,
            dns_resolve: true,
            execute_command: true,
            allow_udp: true,
        }
    }
}

impl Config {
    const FILENAME: &'static str = "config.toml";
    fn read(filename: &str) -> Result<Self, &str> {
        let mut file = fs::File::open(filename).map_err(|_| "can't open config")?;
        let mut data = vec![];
        file.read_to_end(&mut data)
            .map_err(|_| "can't read config")?;
        toml::from_slice(&data).map_err(|_| "can't parse config")
    }
    fn file_location() -> Result<PathBuf, &'static str> {
        let mut res = env::current_exe().map_err(|_| "can't get current exe path")?;
        res.pop();
        res.push(Config::FILENAME);
        Ok(res)
    }
    pub fn get() -> Self {
        let path = Config::file_location();
        if path.is_err() {
            log::error!("Error: {}, using default config", path.err().unwrap());
            return Config::default();
        }

        let path = path.unwrap();
        let cfg = Config::read(path.to_str().unwrap());
        match cfg {
            Err(e) => {
                log::error!("Error: {e}, using default config");
                Config::default()
            }
            Ok(cfg) => cfg,
        }
    }
}
