mod core;
mod docker;
mod error;
mod repo;

use std::env;

use error::{Result, VcdError};

const USAGE: &str = "\
vcd 0.1.0

Usage:
  vcd init <user>
  vcd rebuild [user]
  vcd config set <key> <value>
  vcd config list
  vcd doctor
  vcd plugin add <git-url>
  vcd plugin list
  vcd profile <profile-name> add <plugin-name>
  vcd profile <profile-name>
  vcd <codex|claude> <git-url> [branch] [-pf|--profile <profile-name>]
  vcd --help
  vcd --version

Commands:
  init <user>    Configure vcd user, Git/SSH settings, and build a local base image.
  rebuild        Rebuild the local base image and update the current config.
  config set     Update an existing vcd config value.
  config list    Print the current vcd config.
  doctor         Check OrbStack, Docker CLI, and vcd config.
  plugin add     Clone a plugin repository into the local vcd plugin directory.
  plugin list    List locally installed vcd plugins.
  profile add    Associate an installed plugin with a profile.
  profile        Show plugins associated with a profile.
  codex          Open a Git project with codex in the local vcd Docker container.
  claude         Open a Git project with claude in the local vcd Docker container.
";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Init {
        user: String,
    },
    Rebuild {
        user: Option<String>,
    },
    ConfigSet {
        key: String,
        value: String,
    },
    ConfigList,
    Doctor,
    PluginAdd {
        git_url: String,
    },
    PluginList,
    ProfileAdd {
        profile_name: String,
        plugin_name: String,
    },
    ProfileShow {
        profile_name: String,
    },
    Open {
        editor: String,
        repo_url: String,
        branch: Option<String>,
        profile: Option<String>,
    },
    Help,
    Version,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    match parse(env::args().skip(1))? {
        Command::Help => {
            print!("{USAGE}");
            Ok(())
        }
        Command::Version => {
            println!("vcd {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Init { user } => core::init::run(&user),
        Command::Rebuild { user } => core::rebuild::run(user.as_deref()),
        Command::ConfigSet { key, value } => core::config::set(&key, &value),
        Command::ConfigList => core::config::list(),
        Command::Doctor => core::doctor::run(),
        Command::PluginAdd { git_url } => core::plugin::add(&git_url),
        Command::PluginList => core::plugin::list(),
        Command::ProfileAdd {
            profile_name,
            plugin_name,
        } => core::profile::add(&profile_name, &plugin_name),
        Command::ProfileShow { profile_name } => core::profile::show(&profile_name),
        Command::Open {
            editor,
            repo_url,
            branch,
            profile,
        } => core::open::run(&editor, &repo_url, branch.as_deref(), profile.as_deref()),
    }
}

fn parse<I>(args: I) -> Result<Command>
where
    I: IntoIterator<Item = String>,
{
    let args: Vec<String> = args.into_iter().collect();

    match args.as_slice() {
        [] => Err(parse_error("missing command")),
        [flag] if flag == "--help" || flag == "-h" => Ok(Command::Help),
        [flag] if flag == "--version" || flag == "-V" => Ok(Command::Version),
        [command, user] if command == "init" => Ok(Command::Init { user: user.clone() }),
        [command] if command == "init" => Err(parse_error("missing user for 'init'")),
        [command] if command == "doctor" => Ok(Command::Doctor),
        [command] if command == "rebuild" => Ok(Command::Rebuild { user: None }),
        [command, user] if command == "rebuild" => Ok(Command::Rebuild {
            user: Some(user.clone()),
        }),
        [command, subcommand, key, value] if command == "config" && subcommand == "set" => {
            Ok(Command::ConfigSet {
                key: key.clone(),
                value: value.clone(),
            })
        }
        [command, subcommand] if command == "config" && subcommand == "list" => {
            Ok(Command::ConfigList)
        }
        [command] if command == "config" => Err(parse_error("missing subcommand for 'config'")),
        [command, subcommand, ..] if command == "config" => Err(parse_error(format!(
            "unsupported config command 'config {subcommand}'"
        ))),
        [command, subcommand, git_url] if command == "plugin" && subcommand == "add" => {
            Ok(Command::PluginAdd {
                git_url: git_url.clone(),
            })
        }
        [command, subcommand] if command == "plugin" && subcommand == "add" => {
            Err(parse_error("missing git-url for 'plugin add'"))
        }
        [command, subcommand] if command == "plugin" && subcommand == "list" => {
            Ok(Command::PluginList)
        }
        [command] if command == "plugin" => Err(parse_error("missing subcommand for 'plugin'")),
        [command, subcommand, ..] if command == "plugin" => Err(parse_error(format!(
            "unsupported plugin command 'plugin {subcommand}'"
        ))),
        [command, profile_name, subcommand, plugin_name]
            if command == "profile" && subcommand == "add" =>
        {
            Ok(Command::ProfileAdd {
                profile_name: profile_name.clone(),
                plugin_name: plugin_name.clone(),
            })
        }
        [command, profile_name, subcommand] if command == "profile" && subcommand == "add" => {
            Err(parse_error(format!(
                "missing plugin-name for 'profile {profile_name} add'"
            )))
        }
        [command, profile_name] if command == "profile" => Ok(Command::ProfileShow {
            profile_name: profile_name.clone(),
        }),
        [command] if command == "profile" => Err(parse_error("missing profile-name for 'profile'")),
        [command, profile_name, subcommand, ..] if command == "profile" => Err(parse_error(
            format!("unsupported profile command 'profile {profile_name} {subcommand}'"),
        )),
        [editor, repo_url, rest @ ..] => parse_open_command(editor, repo_url, rest),
        [command, ..] => Err(parse_error(format!("unsupported command '{command}'"))),
    }
}

fn parse_open_command(editor: &str, repo_url: &str, rest: &[String]) -> Result<Command> {
    let mut branch = None;
    let mut profile = None;
    let mut index = 0;

    while index < rest.len() {
        let value = &rest[index];
        match value.as_str() {
            "-pf" | "--profile" => {
                if profile.is_some() {
                    return Err(parse_error("profile can only be provided once"));
                }
                let Some(profile_name) = rest.get(index + 1) else {
                    return Err(parse_error("missing profile-name for profile flag"));
                };
                if profile_name.starts_with('-') {
                    return Err(parse_error("missing profile-name for profile flag"));
                }
                profile = Some(profile_name.clone());
                index += 2;
            }
            flag if flag.starts_with('-') => {
                return Err(parse_error(format!("unsupported open option '{flag}'")));
            }
            _ => {
                if branch.is_some() {
                    return Err(parse_error("branch can only be provided once"));
                }
                branch = if value.is_empty() {
                    None
                } else {
                    Some(value.clone())
                };
                index += 1;
            }
        }
    }

    Ok(Command::Open {
        editor: editor.to_string(),
        repo_url: repo_url.to_string(),
        branch,
        profile,
    })
}

fn parse_error(message: impl Into<String>) -> VcdError {
    VcdError::new("参数解析失败", message).with_hint(concat!(
        "当前支持:\n",
        "  vcd init <user>\n",
        "  vcd rebuild [user]\n",
        "  vcd config set <key> <value>\n",
        "  vcd config list\n",
        "  vcd doctor\n",
        "  vcd plugin add <git-url>\n",
        "  vcd plugin list\n",
        "  vcd profile <profile-name> add <plugin-name>\n",
        "  vcd profile <profile-name>\n",
        "  vcd <codex|claude> <git-url> [branch] [-pf|--profile <profile-name>]",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_init() {
        let command = parse(["init".to_string(), "jack".to_string()]).unwrap();
        assert_eq!(
            command,
            Command::Init {
                user: "jack".to_string()
            }
        );
    }

    #[test]
    fn rejects_missing_init_user() {
        let error = parse(["init".to_string()]).unwrap_err();
        assert!(error.to_string().contains("missing user"));
    }

    #[test]
    fn parses_rebuild_without_user() {
        let command = parse(["rebuild".to_string()]).unwrap();
        assert_eq!(command, Command::Rebuild { user: None });
    }

    #[test]
    fn parses_rebuild_with_user() {
        let command = parse(["rebuild".to_string(), "jack".to_string()]).unwrap();
        assert_eq!(
            command,
            Command::Rebuild {
                user: Some("jack".to_string())
            }
        );
    }

    #[test]
    fn parses_config_set() {
        let command = parse([
            "config".to_string(),
            "set".to_string(),
            "user.name".to_string(),
            "jack".to_string(),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::ConfigSet {
                key: "user.name".to_string(),
                value: "jack".to_string(),
            }
        );
    }

    #[test]
    fn parses_config_list() {
        let command = parse(["config".to_string(), "list".to_string()]).unwrap();
        assert_eq!(command, Command::ConfigList);
    }

    #[test]
    fn parses_doctor() {
        let command = parse(["doctor".to_string()]).unwrap();
        assert_eq!(command, Command::Doctor);
    }

    #[test]
    fn parses_plugin_add() {
        let command = parse([
            "plugin".to_string(),
            "add".to_string(),
            "https://github.com/user/vcd-plugin-example.git".to_string(),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::PluginAdd {
                git_url: "https://github.com/user/vcd-plugin-example.git".to_string(),
            }
        );
    }

    #[test]
    fn parses_plugin_list() {
        let command = parse(["plugin".to_string(), "list".to_string()]).unwrap();
        assert_eq!(command, Command::PluginList);
    }

    #[test]
    fn rejects_missing_plugin_add_url() {
        let error = parse(["plugin".to_string(), "add".to_string()]).unwrap_err();
        assert!(error.to_string().contains("missing git-url"));
    }

    #[test]
    fn parses_profile_add() {
        let command = parse([
            "profile".to_string(),
            "backend".to_string(),
            "add".to_string(),
            "rust-tools".to_string(),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::ProfileAdd {
                profile_name: "backend".to_string(),
                plugin_name: "rust-tools".to_string(),
            }
        );
    }

    #[test]
    fn parses_profile_show() {
        let command = parse(["profile".to_string(), "backend".to_string()]).unwrap();
        assert_eq!(
            command,
            Command::ProfileShow {
                profile_name: "backend".to_string(),
            }
        );
    }

    #[test]
    fn rejects_missing_profile_name() {
        let error = parse(["profile".to_string()]).unwrap_err();
        assert!(error.to_string().contains("missing profile-name"));
    }

    #[test]
    fn rejects_missing_profile_add_plugin_name() {
        let error = parse([
            "profile".to_string(),
            "backend".to_string(),
            "add".to_string(),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("missing plugin-name"));
    }

    #[test]
    fn parses_editor_repo_without_branch() {
        let command = parse([
            "codex".to_string(),
            "https://github.com/user/project.git".to_string(),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::Open {
                editor: "codex".to_string(),
                repo_url: "https://github.com/user/project.git".to_string(),
                branch: None,
                profile: None,
            }
        );
    }

    #[test]
    fn parses_editor_repo_with_branch() {
        let command = parse([
            "claude".to_string(),
            "https://github.com/user/project.git".to_string(),
            "feature-a".to_string(),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::Open {
                editor: "claude".to_string(),
                repo_url: "https://github.com/user/project.git".to_string(),
                branch: Some("feature-a".to_string()),
                profile: None,
            }
        );
    }

    #[test]
    fn parses_editor_repo_with_profile_after_repo() {
        let command = parse([
            "codex".to_string(),
            "https://github.com/user/project.git".to_string(),
            "-pf".to_string(),
            "backend".to_string(),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::Open {
                editor: "codex".to_string(),
                repo_url: "https://github.com/user/project.git".to_string(),
                branch: None,
                profile: Some("backend".to_string()),
            }
        );
    }

    #[test]
    fn parses_editor_repo_with_branch_and_profile() {
        let command = parse([
            "claude".to_string(),
            "https://github.com/user/project.git".to_string(),
            "feature-a".to_string(),
            "--profile".to_string(),
            "frontend".to_string(),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::Open {
                editor: "claude".to_string(),
                repo_url: "https://github.com/user/project.git".to_string(),
                branch: Some("feature-a".to_string()),
                profile: Some("frontend".to_string()),
            }
        );
    }

    #[test]
    fn parses_editor_repo_with_profile_before_branch() {
        let command = parse([
            "codex".to_string(),
            "https://github.com/user/project.git".to_string(),
            "--profile".to_string(),
            "backend".to_string(),
            "feature-a".to_string(),
        ])
        .unwrap();

        assert_eq!(
            command,
            Command::Open {
                editor: "codex".to_string(),
                repo_url: "https://github.com/user/project.git".to_string(),
                branch: Some("feature-a".to_string()),
                profile: Some("backend".to_string()),
            }
        );
    }

    #[test]
    fn rejects_missing_open_profile_value() {
        let error = parse([
            "codex".to_string(),
            "https://github.com/user/project.git".to_string(),
            "--profile".to_string(),
        ])
        .unwrap_err();
        assert!(error.to_string().contains("missing profile-name"));
    }

    #[test]
    fn rejects_duplicate_open_profile() {
        let error = parse([
            "codex".to_string(),
            "https://github.com/user/project.git".to_string(),
            "-pf".to_string(),
            "backend".to_string(),
            "--profile".to_string(),
            "frontend".to_string(),
        ])
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("profile can only be provided once"));
    }
}
