//! Command used to add a plugin/mod to the server configuration.
use std::{
    fs::{File, TryLockError},
    path::Path,
};

use anyhow::Context;

use crate::{
    config::Config,
    modrinth::{get_addon, repository::ReleaseChannel},
};

/// Installs a compatible Modrinth addon and records it in minegr.toml.
#[derive(Debug, clap::Args)]
#[command(allow_missing_positional = true)]
pub struct AddArgs {
    /// Modrinth project slug or ID.
    #[arg(index = 2, value_name = "SLUG")]
    pub slug: String,
    /// Most permissive channel to allow; defaults to alpha, which also permits beta and release.
    #[arg(long, short, value_name = "CHANNEL", value_enum)]
    pub channel: Option<ReleaseChannel>,
}

/// Downloads an addon, verifies it, and saves its pinned record in the selected configuration.
/// Holds an exclusive directory lock from configuration loading through installation and saving.
pub fn execute(args: &AddArgs, config_dir: &Path) -> anyhow::Result<()> {
    // Lock the directory because atomic saves replace the configuration file's inode.
    let config_lock =
        File::open(config_dir).context("Failed to open the configuration directory for locking")?;
    match config_lock.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => anyhow::bail!(
            "Another configuration update is already in progress for {}",
            config_dir.display()
        ),
        Err(TryLockError::Error(error)) => {
            return Err(error).context("Failed to lock the configuration directory");
        }
    }
    let mut config = Config::try_load(config_dir)?;
    let addons_dir = config.minecraft.platform.get_addons_dir(config_dir)?;
    let addon = get_addon(&addons_dir, &config, &args.slug, args.channel)?;
    let name = addon.name.clone();
    config.addons.push(addon);
    config.try_save(config_dir).with_context(|| {
        format!(
            "Installed {name}, but saving its record in {}/minegr.toml failed",
            config_dir.display()
        )
    })?;

    eprintln!("  Added {name}");
    eprintln!(
        "  Configuration: {}",
        config_dir.join("minegr.toml").display()
    );
    Ok(())
}
