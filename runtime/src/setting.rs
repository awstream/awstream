//! A flexible client/server runtime setting in TOML.

use std::fs::File;
use std::io::Read;
use std::io::Result;
use toml;

/// The runtime setting.
#[derive(Deserialize)]
pub struct Setting {
    /// Server's IP address.
    pub server: String,

    /// Data connection port.
    pub port: u16,

    /// Path to the profile.
    pub profile_path: String,

    /// Path to source (video).
    pub source_path: String,

    /// Path to stat (per frame stat).
    pub stat_path: String,
}

impl Setting {
    /// Initialize from a file.
    pub fn init(path: &str) -> Result<Setting> {
        let file = format!("{}/{}", env!("CARGO_MANIFEST_DIR"), path);
        let mut file = File::open(file)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(toml::from_str(&contents).unwrap())
    }
}
