---
type: feature
status: implemented
created: 2026-11-14
---

# Add command

`minegr add [CONFIG_DIR] <SLUG>` installs a Modrinth plugin or mod and records the exact file in `minegr.toml`. The directory defaults to `.`.

```text
minegr add ./server simple-voice-chat --channel release
minegr add simple-voice-chat
```

## Behaviour

`--channel` / `-c` sets the most permissive permitted release channel. `release` accepts only releases, `beta` accepts beta and release versions, and `alpha` accepts all three. The default is alpha.

Versions must match both the configured Minecraft version and a compatible loader. Fabric accepts Fabric versions; Paper accepts Paper, Spigot, and Bukkit versions. The newest matching publication is selected by its parsed timestamp, with version ID breaking ties deterministically.

When `[minegr].application_type` is `Server`, selection excludes versions whose Modrinth environment is `client_only` or `singleplayer_only`. Other environments remain eligible, including `client_only_server_optional`, which supports optional server functionality. `unknown`, missing, and unrecognized environment metadata do not establish incompatibility and remain eligible. `Instance` configurations retain their existing environment selection. Filtering happens before choosing the newest version, so a newer incompatible release does not hide an older compatible one. See [Modrinth's environment definitions](https://modrinth.com/news/article/new-environments/#new-system).

The primary file is selected, falling back to the first file when no primary is marked. Its filename must name a JAR directly inside the addon directory. Installation targets `mods/` for Fabric and `plugins/` for Paper. These directories must not be symlinks, including dangling symlinks. Vanilla rejects addons.

The command displays metadata progress and separate downloading, hashing, and copying progress bars. It downloads into private temporary storage, checks the API byte count and SHA-512, then stages an adjacent temporary file and publishes without overwriting an existing path.

After installation, the command appends an `[[addons]]` record and saves the configuration. The record contains the API filename as `name`, the exact `version_id`, an algorithm-qualified `hash`, and the allowed channel as `version_type`. The saved channel is an update policy and can differ from the installed version's actual channel. Other API metadata is fetched as needed rather than duplicated in configuration.

## Rules

Before loading configuration, `add` acquires an exclusive advisory lock on the server root directory and holds it through installation and configuration saving. Another `add` targeting the same directory fails immediately while the lock is held, including when invoked through a different path to that directory. The lock is released when the command exits, including on failure. Programs that do not honor this advisory lock can still modify the configuration.

Before contacting Modrinth or installing files, configuration loading rejects a symlink, non-regular file, or `minegr.toml` owned by another effective user. The opened file is validated before reading; symlinks are not followed and special files cannot block the open. Configuration saving also checks an existing destination before staging and again before publication, refusing to replace an unsafe path.

The addon directory is checked for symlinks before metadata requests and again before staging the verified JAR.

Missing compatible versions, invalid filenames or hashes, HTTP errors, size or hash mismatches, and existing destination paths fail with contextual errors. Already configured filenames or version IDs are rejected. Failed downloads and staging operations discard temporary files.

If the JAR was published but directory synchronization or configuration saving fails, the error identifies that installation already happened. The installed file is retained; resolve the error before retrying, because installation will not overwrite it.

Dependency installation, automatic updates, and restoration of missing pinned addons are not part of this command. A pinned hash verifies downloaded bytes but cannot recover files deleted upstream without a retained copy.

## Verification

`cargo test --test add` runs the real CLI against temporary directories and a local HTTPS Modrinth fixture, without contacting upstream services. These tests require `python3` and `openssl`; the fixture generates its own temporary CA and verifies TLS normally without modifying the system trust store.

## Related

- [Configuration](../../architecture/configuration.md)
- [Artifact acquisition](../../architecture/artifact-acquisition.md)
- [Addon configuration proposal](../../proposals/addon-configuration.md)
