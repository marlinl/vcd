use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::docker;
use crate::error::{Result, VcdError};

const DEFAULT_PROXY_URL: &str = "http://host.docker.internal:1087";
const DEFAULT_NO_PROXY: &str = "localhost,127.0.0.1,::1,host.docker.internal,.local";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VcdConfig {
    pub user_name: String,
    pub user_email: String,
    pub ssh_key_path: String,
    pub initialized_at: String,
    pub container_docker_build: String,
    pub container_id: String,
    pub proxy_url: String,
    pub no_proxy: String,
    pub token_gitlab_host: String,
    pub token_gitlab: String,
    pub token_github: String,
}

pub fn default_config_dir() -> Result<PathBuf> {
    let home = env::var_os("HOME").ok_or_else(|| {
        VcdError::new("配置写入失败", "HOME environment variable is not set")
            .with_hint("请在有用户 HOME 的终端环境中运行 vcd init <user>")
    })?;

    Ok(PathBuf::from(home).join(".config").join("vcd"))
}

pub fn default_config_path() -> Result<PathBuf> {
    Ok(default_config_dir()?.join("config"))
}

pub fn write_config(path: &Path, config: &VcdConfig) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        VcdError::new(
            "配置写入失败",
            format!("invalid config path: {}", path.display()),
        )
    })?;

    fs::create_dir_all(parent).map_err(|err| {
        VcdError::new(
            "配置写入失败",
            format!("failed to create {}: {err}", parent.display()),
        )
    })?;

    fs::write(path, serialize(config)).map_err(|err| {
        VcdError::new(
            "配置写入失败",
            format!("failed to write {}: {err}", path.display()),
        )
    })
}

pub fn read_config(path: &Path) -> Result<VcdConfig> {
    let content = fs::read_to_string(path).map_err(|err| {
        VcdError::new(
            "配置读取失败",
            format!("failed to read {}: {err}", path.display()),
        )
        .with_hint("请先运行 vcd init <user> 生成本地配置")
    })?;

    parse(&content)
}

pub fn read_initialized_at(path: &Path) -> Result<Option<String>> {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(VcdError::new(
                "配置读取失败",
                format!("failed to read {}: {err}", path.display()),
            ));
        }
    };

    Ok(content.lines().find_map(|line| {
        let (key, value) = line.split_once('=')?;
        if key == "initialized_at" && !value.is_empty() {
            Some(value.to_string())
        } else {
            None
        }
    }))
}

pub fn set(key: &str, value: &str) -> Result<()> {
    let config_path = default_config_path()?;
    let mut config = read_config(&config_path)?;

    match key {
        "user.name" => {
            docker::validate_image_user(value)?;
            config.user_name = value.to_string();
            config.container_id = docker::base_image_name(
                &docker::default_container_id(value),
                &config.initialized_at,
            );
        }
        "user.email" => {
            validate_email(value)?;
            config.user_email = value.to_string();
        }
        "ssh.key_path" => {
            validate_ssh_key_path(value)?;
            docker::ensure_host_ssh_key(value)?;
            config.ssh_key_path = value.to_string();
        }
        "container.docker_build" => {
            validate_non_empty(key, value)?;
            config.container_docker_build = value.to_string();
        }
        "container.id" => {
            validate_non_empty(key, value)?;
            config.container_id = value.to_string();
        }
        "proxy.url" => {
            config.proxy_url = value.to_string();
        }
        "proxy.no_proxy" => {
            config.no_proxy = value.to_string();
        }
        "token.gitlab-host" => {
            config.token_gitlab_host = value.to_string();
        }
        "token.gitlab" => {
            config.token_gitlab = value.to_string();
        }
        "token.github" => {
            config.token_github = value.to_string();
        }
        _ => return Err(unsupported_key(key)),
    }

    write_config(&config_path, &config)?;
    println!("Updated {key}");
    println!("Run vcd rebuild to rebuild the base image with this value.");
    Ok(())
}

pub fn list() -> Result<()> {
    let config_path = default_config_path()?;
    let config = read_config(&config_path)?;
    print!("{}", serialize(&config));
    Ok(())
}

pub fn serialize(config: &VcdConfig) -> String {
    format!(
        "user.name={}\nuser.email={}\nssh.key_path={}\ninitialized_at={}\ncontainer.docker_build={}\ncontainer.id={}\nproxy.url={}\nproxy.no_proxy={}\ntoken.gitlab-host={}\ntoken.gitlab={}\ntoken.github={}\n",
        config.user_name,
        config.user_email,
        config.ssh_key_path,
        config.initialized_at,
        config.container_docker_build,
        config.container_id,
        config.proxy_url,
        config.no_proxy,
        config.token_gitlab_host,
        config.token_gitlab,
        config.token_github
    )
}

fn parse(content: &str) -> Result<VcdConfig> {
    let mut user_name = None;
    let mut user_email = None;
    let mut ssh_key_path = None;
    let mut base_image = None;
    let mut initialized_at = None;
    let mut container_docker_build = None;
    let mut container_id = None;
    let mut proxy_url = None;
    let mut no_proxy = None;
    let mut token_gitlab_host = None;
    let mut token_gitlab = None;
    let mut token_github = None;

    for line in content.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        match key {
            "user.name" | "user" => user_name = Some(value.to_string()),
            "user.email" | "git_email" => user_email = Some(value.to_string()),
            "ssh.key_path" => ssh_key_path = Some(value.to_string()),
            "ssh_key" => {
                let user = user_name.clone().unwrap_or_default();
                ssh_key_path = Some(format!("/Users/{user}/.ssh/{value}"));
            }
            "base_image" => base_image = Some(value.to_string()),
            "container.docker_build" | "dockerfile_path" => {
                container_docker_build = Some(value.to_string());
            }
            "container.id" => container_id = Some(value.to_string()),
            "initialized_at" => initialized_at = Some(value.to_string()),
            "proxy.url" => proxy_url = Some(value.to_string()),
            "proxy.no_proxy" => no_proxy = Some(value.to_string()),
            "token.gitlab-host" => token_gitlab_host = Some(value.to_string()),
            "token.gitlab" => token_gitlab = Some(value.to_string()),
            "token.github" => token_github = Some(value.to_string()),
            _ => {}
        }
    }

    let user_name = required_config_value(user_name, "user.name")?;
    let initialized_at = required_config_value(initialized_at, "initialized_at")?;
    let container_id = container_id.or(base_image).unwrap_or_else(|| {
        docker::base_image_name(&docker::default_container_id(&user_name), &initialized_at)
    });

    Ok(VcdConfig {
        user_name,
        user_email: required_config_value(user_email, "user.email")?,
        ssh_key_path: required_config_value(ssh_key_path, "ssh.key_path")?,
        initialized_at,
        container_docker_build: container_docker_build.unwrap_or_default(),
        container_id,
        proxy_url: proxy_url.unwrap_or_else(|| DEFAULT_PROXY_URL.to_string()),
        no_proxy: no_proxy.unwrap_or_else(|| DEFAULT_NO_PROXY.to_string()),
        token_gitlab_host: token_gitlab_host.unwrap_or_default(),
        token_gitlab: token_gitlab.unwrap_or_default(),
        token_github: token_github.unwrap_or_default(),
    })
}

fn required_config_value(value: Option<String>, key: &str) -> Result<String> {
    match value {
        Some(value) if !value.is_empty() => Ok(value),
        _ => Err(VcdError::new(
            "配置读取失败",
            format!("missing required config key '{key}'"),
        )
        .with_hint("请重新运行 vcd init <user> 生成完整配置")),
    }
}

fn validate_non_empty(key: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        Err(VcdError::new(
            "配置修改失败",
            format!("{key} cannot be empty"),
        ))
    } else {
        Ok(())
    }
}

fn validate_email(value: &str) -> Result<()> {
    if value.is_empty() || !value.contains('@') {
        Err(VcdError::new(
            "配置修改失败",
            "user.email must be a non-empty email address",
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
            "配置修改失败",
            "ssh.key_path must be an absolute SSH private key path",
        ))
    } else {
        Ok(())
    }
}

fn unsupported_key(key: &str) -> VcdError {
    VcdError::new("配置修改失败", format!("unsupported config key '{key}'")).with_hint(
        "当前支持: user.name, user.email, ssh.key_path, container.docker_build, container.id, proxy.url, proxy.no_proxy, token.gitlab-host, token.gitlab, token.github",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_config() {
        let output = serialize(&VcdConfig {
            user_name: "jack".to_string(),
            user_email: "jack@example.com".to_string(),
            ssh_key_path: "/Users/jack/.ssh/id_rsa".to_string(),
            initialized_at: "20260520103000".to_string(),
            container_docker_build: "/Users/jack/.config/vcd/Dockerfile".to_string(),
            container_id: "vcd-jack:20260520103000".to_string(),
            proxy_url: "http://host.docker.internal:1087".to_string(),
            no_proxy: "localhost,127.0.0.1,::1,host.docker.internal,.local".to_string(),
            token_gitlab_host: "gitlab.example.com".to_string(),
            token_gitlab: "glpat-example".to_string(),
            token_github: "ghp_example".to_string(),
        });

        assert_eq!(
            output,
            "user.name=jack\nuser.email=jack@example.com\nssh.key_path=/Users/jack/.ssh/id_rsa\ninitialized_at=20260520103000\ncontainer.docker_build=/Users/jack/.config/vcd/Dockerfile\ncontainer.id=vcd-jack:20260520103000\nproxy.url=http://host.docker.internal:1087\nproxy.no_proxy=localhost,127.0.0.1,::1,host.docker.internal,.local\ntoken.gitlab-host=gitlab.example.com\ntoken.gitlab=glpat-example\ntoken.github=ghp_example\n"
        );
    }

    #[test]
    fn parses_config() {
        let config = parse(
            "user.name=jack\nuser.email=jack@example.com\nssh.key_path=/Users/jack/.ssh/id_rsa\ninitialized_at=now\ncontainer.docker_build=/Users/jack/.config/vcd/Dockerfile\ncontainer.id=vcd-jack:now\nproxy.url=http://host.docker.internal:7897\nproxy.no_proxy=localhost,127.0.0.1\ntoken.gitlab-host=gitlab.example.com\ntoken.gitlab=glpat-example\ntoken.github=ghp_example\n",
        )
        .unwrap();

        assert_eq!(
            config,
            VcdConfig {
                user_name: "jack".to_string(),
                user_email: "jack@example.com".to_string(),
                ssh_key_path: "/Users/jack/.ssh/id_rsa".to_string(),
                initialized_at: "now".to_string(),
                container_docker_build: "/Users/jack/.config/vcd/Dockerfile".to_string(),
                container_id: "vcd-jack:now".to_string(),
                proxy_url: "http://host.docker.internal:7897".to_string(),
                no_proxy: "localhost,127.0.0.1".to_string(),
                token_gitlab_host: "gitlab.example.com".to_string(),
                token_gitlab: "glpat-example".to_string(),
                token_github: "ghp_example".to_string(),
            }
        );
    }

    #[test]
    fn parses_old_config_without_dockerfile_path() {
        let config = parse(
            "user=jack\ngit_username=Jack User\ngit_email=jack@example.com\nssh_key=id_rsa\nbase_image=vcd-jack:20260520103000\ninitialized_at=now\n",
        )
        .unwrap();

        assert_eq!(config.container_docker_build, "");
        assert_eq!(config.container_id, "vcd-jack:20260520103000");
        assert_eq!(config.ssh_key_path, "/Users/jack/.ssh/id_rsa");
        assert_eq!(config.token_gitlab_host, "");
        assert_eq!(config.token_gitlab, "");
        assert_eq!(config.token_github, "");
    }

    #[test]
    fn reads_initialized_at_from_config() {
        let path = env::temp_dir().join(format!(
            "vcd-config-test-{}",
            crate::docker::timestamp().unwrap()
        ));
        fs::write(&path, "initialized_at=20260522120000\n").unwrap();

        assert_eq!(
            read_initialized_at(&path).unwrap(),
            Some("20260522120000".to_string())
        );

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn missing_config_has_no_initialized_at() {
        let path = env::temp_dir().join(format!(
            "vcd-missing-config-test-{}",
            crate::docker::timestamp().unwrap()
        ));

        assert_eq!(read_initialized_at(&path).unwrap(), None);
    }

    #[test]
    fn rejects_empty_user_name() {
        assert!(validate_non_empty("user.name", "").is_err());
    }

    #[test]
    fn validates_email() {
        assert!(validate_email("jack@example.com").is_ok());
        assert!(validate_email("jack").is_err());
    }

    #[test]
    fn validates_ssh_key_path() {
        assert!(validate_ssh_key_path("/Users/jack/.ssh/id_ed25519").is_ok());
        assert!(validate_ssh_key_path("id_ed25519").is_err());
    }
}
