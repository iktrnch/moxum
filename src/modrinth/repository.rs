use serde::{Deserialize, Serialize};

/// A Modrinth release channel, encoded using its lowercase API name.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::Display, clap::ValueEnum,
)]
#[serde(rename_all = "lowercase")]
#[strum(serialize_all = "lowercase")]
pub enum ReleaseChannel {
    Release,
    Beta,
    Alpha,
}

impl ReleaseChannel {
    /// Checks whether this channel policy permits a version's release channel.
    pub fn allows(self, version_type: Self) -> bool {
        match self {
            Self::Release => version_type == Self::Release,
            Self::Beta => version_type != Self::Alpha,
            Self::Alpha => true,
        }
    }
}

/// The client and server environments advertised by a Modrinth version.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Environment {
    ClientAndServer,
    ClientOnly,
    ClientOnlyServerOptional,
    SingleplayerOnly,
    ServerOnly,
    ServerOnlyClientOptional,
    DedicatedServerOnly,
    ClientOrServer,
    ClientOrServerPrefersBoth,
    #[default]
    #[serde(other)]
    Unknown,
}

impl Environment {
    /// Allows dedicated servers unless the metadata explicitly excludes them.
    pub(super) fn supports_server(&self) -> bool {
        !matches!(self, Self::ClientOnly | Self::SingleplayerOnly)
    }

    /// Allows clients unless the environment requires a dedicated server.
    pub(super) fn _support_client(&self) -> bool {
        !matches!(self, Self::DedicatedServerOnly)
    }
}

/// Metadata used to select one compatible Modrinth version.
#[derive(Debug, Deserialize)]
pub(super) struct Version {
    pub game_versions: Vec<String>,
    pub id: String,
    pub loaders: Vec<String>,
    pub files: Vec<VersionFile>,
    pub version_type: ReleaseChannel,
    pub date_published: chrono::DateTime<chrono::Utc>,
    /// Missing or unrecognized metadata does not establish an incompatibility.
    #[serde(default)]
    pub environment: Environment,
}

/// One downloadable file belonging to a Modrinth version.
#[derive(Debug, Deserialize)]
pub(super) struct VersionFile {
    pub hashes: Hashes,
    pub url: String,
    pub filename: String,
    pub primary: bool,
    pub size: u64,
}

/// Upstream content hash used to verify an addon.
#[derive(Debug, Deserialize)]
pub(super) struct Hashes {
    pub sha512: String,
}
