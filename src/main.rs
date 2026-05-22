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
  vcd <codex|claude> <git-url> [branch]
  vcd --help
  vcd --version

Commands:
  init <user>    Configure vcd user, Git/SSH settings, and build a local base image.
  rebuild        Rebuild the local base image and update the current config.
  config set     Update an existing vcd config value.
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
    Open {
        editor: String,
        repo_url: String,
        branch: Option<String>,
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
        Command::Open {
            editor,
            repo_url,
            branch,
        } => core::open::run(&editor, &repo_url, branch.as_deref()),
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
        [command] if command == "config" => Err(parse_error("missing subcommand for 'config'")),
        [command, subcommand, ..] if command == "config" => Err(parse_error(format!(
            "unsupported config command 'config {subcommand}'"
        ))),
        [editor, repo_url] => Ok(Command::Open {
            editor: editor.clone(),
            repo_url: repo_url.clone(),
            branch: None,
        }),
        [editor, repo_url, branch] => Ok(Command::Open {
            editor: editor.clone(),
            repo_url: repo_url.clone(),
            branch: if branch.is_empty() {
                None
            } else {
                Some(branch.clone())
            },
        }),
        [command, ..] => Err(parse_error(format!("unsupported command '{command}'"))),
    }
}

fn parse_error(message: impl Into<String>) -> VcdError {
    VcdError::new("参数解析失败", message).with_hint(
        "当前支持: vcd init <user>、vcd rebuild [user]、vcd config set <key> <value> 或 vcd <codex|claude> <git-url> [branch]",
    )
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
            }
        );
    }
}
