use std::io::{self, Write};
use std::path::Path;

use crate::docker;
use crate::error::{Result, VcdError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitOptions {
    pub user_name: String,
    pub user_email: String,
    pub ssh_key_path: String,
    pub token_gitlab_host: String,
    pub token_gitlab: String,
    pub token_github: String,
}

pub fn prompt_init_options(user: &str, defaults: Option<&InitOptions>) -> Result<InitOptions> {
    docker::validate_image_user(user)?;

    let user_email = match defaults {
        Some(defaults) => prompt_with_default("User email", &defaults.user_email)?,
        None => prompt_required("User email")?,
    };
    validate_user_email(&user_email)?;

    let default_ssh_key_path = defaults
        .map(|defaults| defaults.ssh_key_path.clone())
        .unwrap_or_else(|| format!("/Users/{user}/.ssh/id_rsa"));
    let ssh_key_path = prompt_with_default("SSH key path", &default_ssh_key_path)?;
    validate_ssh_key_path(&ssh_key_path)?;

    let token_gitlab_host = prompt_with_default(
        "GitLab host (optional, exported as GITLAB_HOST)",
        defaults
            .map(|defaults| defaults.token_gitlab_host.as_str())
            .unwrap_or(""),
    )?;
    let token_gitlab = prompt_with_default(
        "GitLab API token (optional, exported as GITLAB_TOKEN)",
        defaults
            .map(|defaults| defaults.token_gitlab.as_str())
            .unwrap_or(""),
    )?;
    let token_github = prompt_with_default(
        "GitHub API token (optional, exported as GH_TOKEN)",
        defaults
            .map(|defaults| defaults.token_github.as_str())
            .unwrap_or(""),
    )?;

    Ok(InitOptions {
        user_name: user.to_string(),
        user_email,
        ssh_key_path,
        token_gitlab_host,
        token_gitlab,
        token_github,
    })
}

fn prompt_with_default(label: &str, default: &str) -> Result<String> {
    print!("{label} [{default}]: ");
    io::stdout().flush().map_err(prompt_error)?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(prompt_error)?;
    let value = input.trim();
    if value.is_empty() {
        Ok(default.to_string())
    } else {
        Ok(value.to_string())
    }
}

fn prompt_required(label: &str) -> Result<String> {
    print!("{label}: ");
    io::stdout().flush().map_err(prompt_error)?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(prompt_error)?;
    let value = input.trim();
    if value.is_empty() {
        Err(VcdError::new(
            "参数解析失败",
            format!("{label} cannot be empty"),
        ))
    } else {
        Ok(value.to_string())
    }
}

fn validate_user_email(value: &str) -> Result<()> {
    if value.is_empty() || !value.contains('@') {
        Err(VcdError::new(
            "参数解析失败",
            "User email must be a non-empty email address",
        ))
    } else {
        Ok(())
    }
}

fn validate_ssh_key_path(value: &str) -> Result<()> {
    let path = Path::new(value);
    let invalid = value.is_empty() || value.starts_with('-') || !path.is_absolute();
    if invalid {
        Err(VcdError::new(
            "参数解析失败",
            "SSH key path must be an absolute path, for example /Users/jack/.ssh/id_ed25519",
        ))
    } else {
        Ok(())
    }
}

fn prompt_error(err: io::Error) -> VcdError {
    VcdError::new(
        "参数解析失败",
        format!("failed to read interactive input: {err}"),
    )
}
