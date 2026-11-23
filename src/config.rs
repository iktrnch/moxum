use std::{
    fs::{self, File, Metadata},
    io::{Read, Write},
    os::unix::fs::MetadataExt,
    path::Path,
};

use anyhow::Context;
use rustix::fs::{Mode, OFlags};
use serde::{Deserialize, Serialize};

use crate::modrinth::repository::ReleaseChannel;
pub use crate::platform::Platform;
use crate::platform::properties::Properties;

#[derive(Debug, strum::Display, strum::EnumString, strum::VariantNames, Deserialize, Serialize)]
#[strum(serialize_all = "lowercase")]
pub enum ApplicationType {
    Server,
    Instance,
}

/// A pinned addon file and the channel policy used for future updates.
#[derive(Debug, Deserialize, Serialize)]
pub struct Addon {
    /// Installed filename supplied by Modrinth.
    pub name: String,
    pub version_id: String,
    /// Algorithm-qualified checksum, such as sha512:<hex>.
    pub hash: String,
    /// Most permissive allowed channel, which may differ from the installed release's channel.
    pub version_type: ReleaseChannel,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Moxum {
    pub application_type: ApplicationType,
    pub version: u8,
    pub uuid: String,
    pub name: String,
}

impl Moxum {
    pub fn new(name: String) -> Self {
        Moxum {
            application_type: ApplicationType::Server,
            version: 1,
            uuid: uuid::Uuid::new_v4().to_string(),
            name,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Minecraft {
    pub platform: Platform,
    pub version: String,
    pub checksum: String,
    pub eula: bool,
}

impl Minecraft {
    pub fn new(platform: Platform, version: String, checksum: String, eula: bool) -> Self {
        Minecraft {
            platform,
            version,
            checksum,
            eula,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Java {
    pub executable: String,
    pub jvm_args: String,
    pub server_args: String,
}

impl Default for Java {
    fn default() -> Self {
        Java {
            executable: "java".to_string(),
            jvm_args: "-Xms2G -Xmx2G".to_string(),
            server_args: "nogui".to_string(),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub moxum: Moxum,
    pub minecraft: Minecraft,
    pub properties: Properties,
    pub java: Java,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub addons: Vec<Addon>,
}

impl Config {
    /// Loads an owned, regular configuration file without following symlinks.
    pub fn try_load(config_dir: &Path) -> anyhow::Result<Self> {
        let path = config_dir.join("minegr.toml");
        validate_config_metadata(
            &fs::symlink_metadata(&path).context("Failed to inspect config file")?,
            &path,
        )?;
        let mut file = File::from(
            rustix::fs::open(
                &path,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .context("Failed to open config file without following symlinks")?,
        );
        validate_config_metadata(
            &file.metadata().context("Failed to inspect opened config file")?,
            &path,
        )?;
        let mut config_str = String::new();
        file.read_to_string(&mut config_str)
            .context("Failed to read config file")?;
        toml::from_str(&config_str).context("Failed to parse config file")
    }

    /// Atomically saves minegr.toml inside the selected directory with owner-only permissions.
    /// Refuses to replace a symlink, non-regular file, or file owned by another user.
    pub fn try_save(&self, path: &Path) -> anyhow::Result<()> {
        let destination = path.join("moxum.toml");
        validate_config_destination(&destination)?;
        let text = toml::to_string_pretty(self).context("Failed to serialize the configuration")?;
        let mut staged = tempfile::NamedTempFile::new_in(path)
            .context("Failed to create a temporary configuration file")?;
        staged
            .write_all(text.as_bytes())
            .context("Failed to write the temporary configuration")?;
        staged
            .as_file()
            .sync_all()
            .context("Failed to synchronize the temporary configuration")?;
        validate_config_destination(&destination)?;
        staged
            .persist(&destination)
            .map_err(|error| error.error)
            .context("Failed to publish minegr.toml")?;
        std::fs::File::open(path)
            .context("Failed to open the configuration directory for synchronization")?
            .sync_all()
            .context("moxum.toml was saved, but synchronizing its directory failed")?;
        Ok(())
    }
}

/// Rejects configuration metadata that is not a regular file owned by the effective user.
fn validate_config_metadata(metadata: &Metadata, path: &Path) -> anyhow::Result<()> {
    anyhow::ensure!(
        metadata.is_file(),
        "Configuration must be a regular file, not a symlink or special file: {}",
        path.display()
    );
    anyhow::ensure!(
        metadata.uid() == rustix::process::geteuid().as_raw(),
        "Configuration is owned by another user: {}",
        path.display()
    );
    Ok(())
}

/// Validates an existing save destination while allowing initial configuration creation.
fn validate_config_destination(path: &Path) -> anyhow::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => validate_config_metadata(&metadata, path),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("Failed to inspect configuration save destination"),
    }
}
