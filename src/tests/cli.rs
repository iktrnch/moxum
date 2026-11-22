use super::*;
use clap::CommandFactory;

/// Uses the current directory when a command has no positional directory.
#[test]
fn config_dir_defaults_to_current_directory_for_every_command() {
    for arguments in [
        vec!["minegr", "daemon"],
        vec!["minegr", "init"],
        vec!["minegr", "add", "simple-voice-chat"],
    ] {
        let cli = Cli::try_parse_from(arguments).unwrap();
        assert_eq!(cli.config_dir, PathBuf::from("."));
        assert!(cli.command.is_some());
        if let Some(Commands::Add(args)) = cli.command {
            assert_eq!(args.slug, "simple-voice-chat");
            assert_eq!(args.channel, None);
        }
    }
}

/// Accepts the global positional directory before or after every command.
#[test]
fn every_command_accepts_a_config_directory() {
    for directory in ["./server", "/tmp/minegr server", "instances/survival/"] {
        for arguments in [
            vec!["minegr", "daemon", directory],
            vec!["minegr", directory, "daemon"],
            vec!["minegr", "init", directory],
            vec!["minegr", directory, "init"],
            vec!["minegr", "add", directory, "simple-voice-chat"],
            vec!["minegr", directory, "add", "simple-voice-chat"],
        ] {
            let cli = Cli::try_parse_from(arguments).unwrap();
            assert_eq!(cli.config_dir, PathBuf::from(directory));
            assert!(cli.command.is_some());
            if let Some(Commands::Add(args)) = cli.command {
                assert_eq!(args.slug, "simple-voice-chat");
            }
        }
    }
}

/// Treats removed command names as directory arguments rather than supported subcommands.
#[test]
fn removed_commands_are_not_subcommands() {
    for command in [
        "backup", "console", "logs", "restart", "start", "status", "stop", "sync",
    ] {
        let cli = Cli::try_parse_from(["minegr", command]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.config_dir, PathBuf::from(command));
    }
}

/// Requires an addon slug whether the configuration directory is omitted or supplied globally.
#[test]
fn add_requires_a_slug() {
    for arguments in [vec!["minegr", "add"], vec!["minegr", "./server", "add"]] {
        let error = Cli::try_parse_from(arguments)
            .err()
            .expect("add requires an addon slug");
        assert_eq!(error.kind(), clap::error::ErrorKind::MissingRequiredArgument);
    }
}

/// Accepts the channel flag and its short form without consuming the addon slug as a directory.
#[test]
fn add_accepts_release_channels() {
    for flag in ["--channel", "-c"] {
        for channel in ["release", "beta", "alpha"] {
            let cli =
                Cli::try_parse_from(["minegr", "add", flag, channel, "simple-voice-chat"])
                    .unwrap();
            let Some(Commands::Add(args)) = cli.command else {
                panic!("expected add command");
            };
            assert_eq!(cli.config_dir, PathBuf::from("."));
            assert_eq!(args.slug, "simple-voice-chat");
            assert_eq!(args.channel.unwrap().to_string(), channel);
        }
    }
}

/// Parses every supported initialization option.
#[test]
fn init_accepts_all_options() {
    let matches = Cli::command()
        .try_get_matches_from([
            "minegr",
            "init",
            "./server",
            "--name",
            "Survival",
            "--minecraft-version",
            "1.20.1",
            "--platform",
            "fabric",
            "--accept-eula",
        ])
        .unwrap();
    let init = matches.subcommand_matches("init").unwrap();

    assert_eq!(
        init.get_one::<PathBuf>("config_dir"),
        Some(&PathBuf::from("./server"))
    );
    assert_eq!(
        init.get_one::<String>("name").map(String::as_str),
        Some("Survival")
    );
    assert_eq!(
        init.get_one::<String>("minecraft_version")
            .map(String::as_str),
        Some("1.20.1")
    );
    assert_eq!(
        init.get_one::<String>("platform").map(String::as_str),
        Some("fabric")
    );
    assert!(init.get_flag("accept_eula"));
}

/// Rejects platforms outside the supported set.
#[test]
fn init_rejects_unknown_platforms() {
    let error = Cli::try_parse_from(["minegr", "init", "--platform", "forge"])
        .err()
        .expect("unsupported platforms must be rejected");
    assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
}

/// Leaves every init option unset when no flags are supplied.
#[test]
fn init_options_are_optional() {
    let cli = Cli::try_parse_from(["minegr", "init"]).unwrap();
    let Some(Commands::Init(args)) = cli.command else {
        panic!("expected init command");
    };
    assert_eq!(cli.config_dir, PathBuf::from("."));
    assert_eq!(args.name, None);
    assert_eq!(args.minecraft_version, None);
    assert_eq!(args.platform, None);
    assert_eq!(args.accept_eula, None);
}

/// Accepts each of the three supported platforms.
#[test]
fn init_accepts_supported_platforms() {
    for platform in ["vanilla", "paper", "fabric"] {
        let cli = Cli::try_parse_from(["minegr", "init", "--platform", platform]).unwrap();
        let Some(Commands::Init(args)) = cli.command else {
            panic!("expected init command");
        };
        assert_eq!(args.platform.as_deref(), Some(platform));
    }
}

/// Accepts the directory before or after flags with explicit values.
#[test]
fn init_directory_can_appear_before_or_after_flags() {
    for arguments in [
        ["minegr", "init", "./server", "--name", "Survival"],
        ["minegr", "init", "--name", "Survival", "./server"],
    ] {
        let cli = Cli::try_parse_from(arguments).unwrap();
        let Some(Commands::Init(args)) = cli.command else {
            panic!("expected init command");
        };
        assert_eq!(cli.config_dir, PathBuf::from("./server"));
        assert_eq!(args.name.as_deref(), Some("Survival"));
    }
}

/// Rejects legacy config-file flags while reserving add's short flag for release channels.
#[test]
fn config_flags_are_no_longer_accepted() {
    for command in ["daemon", "init"] {
        for flag in ["--config", "-c"] {
            for arguments in [
                ["minegr", flag, "minegr.toml", command],
                ["minegr", command, flag, "minegr.toml"],
            ] {
                let error = Cli::try_parse_from(arguments)
                    .err()
                    .expect("config-file flags must be rejected");
                assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
            }
        }
    }

    for arguments in [
        ["minegr", "--config", "minegr.toml", "add", "simple-voice-chat"],
        ["minegr", "add", "simple-voice-chat", "--config", "minegr.toml"],
        ["minegr", "-c", "minegr.toml", "add", "simple-voice-chat"],
    ] {
        let error = Cli::try_parse_from(arguments)
            .err()
            .expect("config-file flags must be rejected");
        assert_eq!(error.kind(), clap::error::ErrorKind::UnknownArgument);
    }
}

/// Rejects a config filename as the value of add's channel option.
#[test]
fn add_rejects_config_file_as_channel() {
    for flag in ["--channel", "-c"] {
        let error =
            Cli::try_parse_from(["minegr", "add", "simple-voice-chat", flag, "minegr.toml"])
                .err()
                .expect("a config filename is not a release channel");
        assert_eq!(error.kind(), clap::error::ErrorKind::InvalidValue);
    }
}
