use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};

use sha2::{Digest, Sha512};

const ARTIFACT: &[u8] = b"Moxum environment regression artifact";

/// Owns a loopback HTTPS fixture and its temporary certificate authority.
struct ModrinthFixture {
    root: tempfile::TempDir,
    child: Child,
    proxy: String,
}

impl ModrinthFixture {
    /// Starts the fixture and waits for its listener to become ready.
    fn start() -> Self {
        let root = tempfile::tempdir().unwrap();
        let mut child = Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/modrinth_proxy.py"
            ))
            .arg(root.path())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut proxy = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut proxy)
            .unwrap();
        assert!(
            proxy.starts_with("http://127.0.0.1:"),
            "Fixture did not start"
        );
        Self {
            root,
            child,
            proxy: proxy.trim().to_owned(),
        }
    }

    /// Supplies version metadata and runs the real CLI with fixture-only TLS trust.
    fn add(&self, root: &Path, versions: &str) -> Output {
        fs::write(self.root.path().join("versions.json"), versions).unwrap();
        Command::new(env!("CARGO_BIN_EXE_moxum"))
            .arg("add")
            .arg(root)
            .arg("example-addon")
            .env("HTTPS_PROXY", &self.proxy)
            .env("https_proxy", &self.proxy)
            .env("ALL_PROXY", &self.proxy)
            .env("all_proxy", &self.proxy)
            .env("NO_PROXY", "")
            .env("no_proxy", "")
            .env("SSL_CERT_FILE", self.root.path().join("ca.pem"))
            .env("SSL_CERT_DIR", self.root.path())
            .output()
            .unwrap()
    }
}

impl Drop for ModrinthFixture {
    /// Stops and reaps the fixture when a test finishes, including on assertion failure.
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Describes one compatible Fabric artifact with a supplied Modrinth environment.
fn version(environment: &str, id: &str, published: &str) -> String {
    let hash: String = Sha512::digest(ARTIFACT)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!(
        r#"{{
        "game_versions": ["1.20.1"], "id": "{id}", "loaders": ["fabric"],
        "version_type": "release", "date_published": "{published}",
        "environment": "{environment}",
        "files": [{{"hashes": {{"sha512": "{hash}"}},
            "url": "https://api.modrinth.com/artifact.jar", "filename": "example.jar",
            "primary": true, "size": {size}}}]
    }}"#,
        size = ARTIFACT.len()
    )
}

/// Checks the installed bytes and the exact version pinned by a successful CLI invocation.
fn assert_installed(root: &Path, output: &Output, version_id: &str) {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(root.join("mods/example.jar")).unwrap(), ARTIFACT);
    let config: toml::Value =
        toml::from_str(&fs::read_to_string(root.join("moxum.toml")).unwrap()).unwrap();
    assert_eq!(config["addons"][0]["version_id"].as_str(), Some(version_id));
}

/// Writes a minimal server configuration without contacting upstream services.
fn write_config(root: &Path, platform: &str) -> Vec<u8> {
    let text = format!(
        r#"[moxum]
application_type = "Server"
version = 1
uuid = "018f0000-0000-7000-8000-000000000000"
name = "Review regression"

[minecraft]
platform = "{platform}"
version = "1.20.1"
checksum = ""
eula = true

[properties]

[java]
executable = "java"
jvm_args = "-Xms2G -Xmx2G"
server_args = "nogui"
"#
    );
    fs::write(root.join("moxum.toml"), &text).unwrap();
    text.into_bytes()
}

/// Runs the real add command with upstream traffic directed to a closed local port.
fn add(root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_moxum"))
        .arg("add")
        .arg(root)
        .arg("example-addon")
        .env("HTTPS_PROXY", "http://127.0.0.1:0")
        .env("https_proxy", "http://127.0.0.1:0")
        .env("ALL_PROXY", "http://127.0.0.1:0")
        .env("all_proxy", "http://127.0.0.1:0")
        .env("NO_PROXY", "")
        .env("no_proxy", "")
        .output()
        .unwrap()
}

/// Rejects a competing update before reading or rewriting its configuration.
#[test]
fn add_rejects_concurrent_configuration_updates() {
    let root = tempfile::tempdir().unwrap();
    let original = write_config(root.path(), "Vanilla");
    let lock = File::open(root.path()).unwrap();
    lock.lock().unwrap();

    let output = add(root.path());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        error.contains("Another configuration update is already in progress"),
        "{error}"
    );
    assert_eq!(fs::read(root.path().join("moxum.toml")).unwrap(), original);

    drop(lock);
    let output = add(root.path());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(error.contains("Vanilla does not support addons"), "{error}");
}

/// Rejects existing and dangling addon-directory symlinks before upstream requests.
#[test]
fn add_rejects_symlinked_addon_directories() {
    for (platform, directory) in [("Fabric", "mods"), ("Paper", "plugins")] {
        for dangling in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let outside = tempfile::tempdir().unwrap();
            let target = if dangling {
                outside.path().join("missing")
            } else {
                outside.path().to_path_buf()
            };
            let original = write_config(root.path(), platform);
            let addons_dir = root.path().join(directory);
            symlink(&target, &addons_dir).unwrap();

            let output = add(root.path());
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(!output.status.success());
            assert!(
                error.contains("Addon directory must not be a symlink"),
                "{error}"
            );
            assert_eq!(fs::read(root.path().join("moxum.toml")).unwrap(), original);
            assert!(fs::symlink_metadata(&addons_dir).unwrap().is_symlink());
            assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
        }
    }
}

/// A dedicated server must not install client-only or singleplayer-only releases.
#[test]
fn add_rejects_client_environments_for_servers() {
    let fixture = ModrinthFixture::start();
    for environment in ["client_only", "singleplayer_only"] {
        let root = tempfile::tempdir().unwrap();
        let original = write_config(root.path(), "Fabric");
        let versions = format!(
            "[{}]",
            version(environment, "selected", "2026-11-01T00:00:00Z")
        );

        let output = fixture.add(root.path(), &versions);
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "Installed {environment}: {error}");
        assert!(error.contains("No compatible version"), "{error}");
        assert!(!root.path().join("mods").exists());
        assert_eq!(fs::read(root.path().join("moxum.toml")).unwrap(), original);
    }
}

/// Server-capable and unknown environments remain installable, including optional server support.
#[test]
fn add_accepts_server_capable_and_unknown_environments() {
    let fixture = ModrinthFixture::start();
    for environment in [
        "client_and_server",
        "client_only_server_optional",
        "server_only",
        "server_only_client_optional",
        "dedicated_server_only",
        "client_or_server",
        "client_or_server_prefers_both",
        "unknown",
        "future_environment",
    ] {
        let root = tempfile::tempdir().unwrap();
        write_config(root.path(), "Fabric");
        let versions = format!(
            "[{}]",
            version(environment, "selected", "2026-11-01T00:00:00Z")
        );

        let output = fixture.add(root.path(), &versions);
        assert_installed(root.path(), &output, "selected");
    }
}

/// The server-only restriction does not change addon selection for Instance configurations.
#[test]
fn add_preserves_client_environments_for_instances() {
    let fixture = ModrinthFixture::start();
    for environment in ["client_only", "singleplayer_only"] {
        let root = tempfile::tempdir().unwrap();
        let config = String::from_utf8(write_config(root.path(), "Fabric"))
            .unwrap()
            .replace(
                "application_type = \"Server\"",
                "application_type = \"Instance\"",
            );
        fs::write(root.path().join("moxum.toml"), config).unwrap();
        let versions = format!(
            "[{}]",
            version(environment, "selected", "2026-11-01T00:00:00Z")
        );

        let output = fixture.add(root.path(), &versions);
        assert_installed(root.path(), &output, "selected");
    }
}

/// Filters incompatible environments before choosing the newest publication.
#[test]
fn add_selects_older_server_version_over_newer_client_version() {
    let fixture = ModrinthFixture::start();
    let root = tempfile::tempdir().unwrap();
    write_config(root.path(), "Fabric");
    let versions = format!(
        "[{},{}]",
        version("client_only", "newer-client", "2026-11-02T00:00:00Z"),
        version("server_only", "older-server", "2026-11-01T00:00:00Z")
    );

    let output = fixture.add(root.path(), &versions);
    assert_installed(root.path(), &output, "older-server");
}

/// Metadata without an environment remains usable as an unknown environment.
#[test]
fn add_accepts_missing_environment_metadata() {
    let fixture = ModrinthFixture::start();
    let root = tempfile::tempdir().unwrap();
    write_config(root.path(), "Fabric");
    let metadata = version("unknown", "selected", "2026-11-01T00:00:00Z")
        .replace("\"environment\": \"unknown\",", "");

    let output = fixture.add(root.path(), &format!("[{metadata}]"));
    assert_installed(root.path(), &output, "selected");
}
