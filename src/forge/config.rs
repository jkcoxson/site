// Jackson Coxson

use std::{io::Read, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default)]
pub struct ForgeConfig {
    // General options
    pub content_type: Option<String>, // set as the content-type header for downloads
    #[serde(default)]
    pub alt_names: Vec<String>, // alternative names that will match as the file
    #[serde(default)]
    pub ignore: Vec<String>, // ignores specific files
    pub password: Option<String>,     // password for the files
    #[serde(default = "d_false")]
    pub zip: bool, // zip any file downloaded
    #[serde(default = "d_false")]
    pub zip_parent: bool, // zip this entire folder for download
    #[serde(default = "d_false")]
    pub parented: bool, // move these files to the parent folder
    #[serde(default = "d_false")]
    pub hidden: bool, // hide these files from the browser (can still be downloaded)

    // Media options
    pub convert_to: Option<String>, // converts compatible media to the specified format
    pub resize_to: Option<String>,  // resizes images to desired resolution (YxY)
}

/// Attempts to load a toml from the requested path
pub fn load(path: &PathBuf) -> Result<ForgeConfig, std::io::Error> {
    let mut buffer = String::new();
    std::fs::File::open(path)?.read_to_string(&mut buffer)?;
    let config: ForgeConfig = match toml::from_str(&buffer) {
        Ok(c) => c,
        Err(e) => {
            return Err(std::io::Error::new(std::io::ErrorKind::Other, e));
        }
    };
    Ok(config)
}

fn d_false() -> bool {
    false
}
