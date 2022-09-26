use std::{
    env, fs,
    path::{Path, PathBuf},
};

use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Config {
    /// Bind on address address. eg. `127.0.0.1:1080`
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,
    /// Request timeout
    #[serde(default = "default_timeout")]
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

fn default_timeout() -> u64 {
    60
}

fn default_listen_addr() -> String {
    "127.0.0.1:1080".into()
}

/// Password authentication data
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct PasswordAuth {
    pub username: String,
    pub password: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            listen_addr: default_listen_addr(),
            request_timeout: default_timeout(),
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
    fn read<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let data = fs::read(path).map_err(|e| format!("can't read config: {:?}", e))?;
        toml::from_slice(&data).map_err(|e| format!("can't parse config: {:?}", e))
    }
    fn write<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let data = toml::to_vec(&self).map_err(|e| format!("can't serialize config: {:?}", e))?;
        fs::write(path, &data).map_err(|e| format!("can't write config: {:?}", e))
    }
    fn file_location() -> Result<PathBuf, String> {
        let res = env::current_exe()
            .map_err(|e| format!("can't get current exe path: {:?}", e))?
            .with_file_name(Config::FILENAME);
        Ok(res)
    }
    pub fn get() -> Self {
        let path = Config::file_location();
        if let Err(e) = path {
            log::error!("Error: {e}, using default config");
            return Config::default();
        }

        let path = path.unwrap();
        let cfg = Config::read(path);
        match cfg {
            Err(e) => {
                log::error!("Error: {e}, using default config");
                Config::default()
            }
            Ok(cfg) => cfg,
        }
    }
    pub fn save(&self) {
        let path = Config::file_location();
        if let Err(ref e) = path {
            log::error!("save error: {}", &e);
        }
        let path = path.unwrap();

        let res = self.write(&path);
        if let Err(e) = res {
            log::error!("save error: {e}");
        } else {
            log::info!("config saved to: {}", path.to_str().unwrap());
        }
    }
}
