use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use serde_derive::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Config {
    /// Bind on address address. eg. `127.0.0.1:1080`
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,
    /// Our external IP address to be sent in reply packets (required for UDP)
    #[serde(default)]
    pub public_addr: Option<std::net::IpAddr>,
    /// Request timeout
    #[serde(default = "default_timeout")]
    pub request_timeout: u64,
    /// Authentication
    #[serde(default)]
    pub auth: Option<PasswordAuth>,
    /// Avoid useless roundtrips if we don't need the Authentication layer
    #[serde(default)]
    pub skip_auth: bool,
    /// Enable UDP support
    #[serde(default)]
    pub allow_udp: bool,
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
            public_addr: None,
            request_timeout: default_timeout(),
            auth: None,
            skip_auth: false,
            allow_udp: false,
        }
    }
}

impl Config {
    const FILENAME: &'static str = "config.toml";
    fn read<P: AsRef<Path>>(path: P) -> Result<Self> {
        let data = fs::read_to_string(path).context("can't read config")?;
        toml::from_str(&data).context("can't parse config")
    }
    fn write<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let data = toml::to_string_pretty(&self).context("can't serialize config")?;
        fs::write(path, data).context("can't write config")
    }
    fn file_location() -> Result<PathBuf> {
        let res = env::current_exe()
            .context("can't get current exe path")?
            .with_file_name(Config::FILENAME);
        Ok(res)
    }
    pub fn get() -> Self {
        let path = Config::file_location();
        if let Err(e) = path {
            log::error!(r#"Error: "{e}", using default config"#);
            return Config::default();
        }

        let path = path.unwrap();
        let cfg = Config::read(path);
        match cfg {
            Err(e) => {
                log::error!(r#"Error: "{e}", using default config"#);
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
            log::error!("save error: {}", &e);
        } else {
            log::info!(r#"config saved to: "{}""#, path.to_str().unwrap());
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.allow_udp && self.public_addr.is_none() {
            return Err(anyhow!("Can't allow UDP if public-addr is not set"));
        }
        if self.skip_auth && self.auth.is_some() {
            return Err(anyhow!(
                "Can't use skip-auth flag and authentication altogether."
            ));
        }

        Ok(())
    }
}
