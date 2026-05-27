use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::core::config;
use crate::error::{Result, VcdError};

pub fn add(git_url: &str) -> Result<()> {
    let plugin_name = plugin_name_from_git_url(git_url)?;
    let root = plugin_root()?;

    fs::create_dir_all(&root).map_err(|err| {
        VcdError::new(
            "插件目录创建失败",
            format!("failed to create {}: {err}", root.display()),
        )
    })?;

    let target = root.join(&plugin_name);
    if target.exists() {
        return Err(VcdError::new(
            "插件添加失败",
            format!("plugin directory already exists: {}", target.display()),
        )
        .with_hint(
            "请检查该目录；如需重新添加，请手动删除或重命名后再执行 vcd plugin add <git-url>",
        ));
    }

    ensure_git_cli()?;
    clone_plugin(git_url.trim(), &target)?;

    println!("Plugin added: {plugin_name}");
    println!("Path: {}", display_user_path(&target));
    Ok(())
}

pub fn list() -> Result<()> {
    let root = plugin_root()?;
    let plugins = installed_plugins(&root)?;

    if plugins.is_empty() {
        println!("No plugins installed.");
        return Ok(());
    }

    for plugin in plugins {
        let name = plugin
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        println!("{name}\t{}", display_user_path(&plugin));
    }

    Ok(())
}

pub(crate) fn plugin_root() -> Result<PathBuf> {
    Ok(config::default_config_dir()?.join("plugins"))
}

fn installed_plugins(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let entries = fs::read_dir(root).map_err(|err| {
        VcdError::new(
            "插件目录读取失败",
            format!("failed to read {}: {err}", root.display()),
        )
    })?;

    let mut plugins = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| {
            VcdError::new(
                "插件目录读取失败",
                format!("failed to read entry in {}: {err}", root.display()),
            )
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| {
            VcdError::new(
                "插件目录读取失败",
                format!("failed to read metadata for {}: {err}", path.display()),
            )
        })?;

        if file_type.is_dir() {
            plugins.push(path);
        }
    }

    plugins.sort_by(|left, right| left.file_name().cmp(&right.file_name()));
    Ok(plugins)
}

fn ensure_git_cli() -> Result<()> {
    let status = Command::new("git")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| {
            VcdError::new("Git CLI 不存在", format!("failed to execute git: {err}"))
                .with_hint("请先安装 Git CLI，并确认当前终端可以执行 git --version")
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(VcdError::new(
            "Git CLI 不可用",
            format!("git --version exited with status {status}"),
        )
        .with_hint("请先确认当前终端可以执行 git --version"))
    }
}

fn clone_plugin(git_url: &str, target: &Path) -> Result<()> {
    let status = Command::new("git")
        .arg("clone")
        .arg(git_url)
        .arg(target)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| {
            VcdError::new(
                "Git clone 失败",
                format!("failed to execute git clone: {err}"),
            )
            .with_hint("请确认 Git CLI 已安装，并且当前终端有权限访问该仓库")
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(VcdError::new(
            "Git clone 失败",
            format!("git clone exited with status {status}"),
        )
        .with_hint("请确认仓库地址、网络连接和 Git/SSH 认证配置可用"))
    }
}

fn plugin_name_from_git_url(git_url: &str) -> Result<String> {
    let trimmed = git_url.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return Err(invalid_git_url("missing Git repository URL"));
    }

    if !(trimmed.starts_with("http://")
        || trimmed.starts_with("https://")
        || trimmed.starts_with("git@")
        || trimmed.starts_with("ssh://"))
    {
        return Err(invalid_git_url(format!("unsupported Git URL '{trimmed}'")));
    }

    let last = trimmed
        .rsplit(['/', ':'])
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| invalid_git_url(format!("cannot infer plugin name from '{trimmed}'")))?;
    let name = last.strip_suffix(".git").unwrap_or(last);

    validate_plugin_name(name)?;
    Ok(name.to_string())
}

pub(crate) fn validate_plugin_name(name: &str) -> Result<()> {
    let invalid = name.is_empty()
        || name == "."
        || name == ".."
        || name.starts_with('-')
        || name
            .chars()
            .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-'));

    if invalid {
        Err(invalid_git_url(format!(
            "cannot infer a safe plugin name from '{name}'"
        )))
    } else {
        Ok(())
    }
}

fn invalid_git_url(message: impl Into<String>) -> VcdError {
    VcdError::new("Git 仓库地址非法", message)
        .with_hint("请传入常见 HTTPS 或 SSH Git URL，例如 https://github.com/user/plugin.git")
}

fn display_user_path(path: &Path) -> String {
    let Some(home) = std::env::var_os("HOME") else {
        return path.display().to_string();
    };
    let home = PathBuf::from(home);
    match path.strip_prefix(&home) {
        Ok(relative) if relative.as_os_str().is_empty() => "~".to_string(),
        Ok(relative) => format!("~/{}", relative.display()),
        Err(_) => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn infers_plugin_name_from_https_url() {
        assert_eq!(
            plugin_name_from_git_url("https://github.com/user/vcd-plugin-example.git").unwrap(),
            "vcd-plugin-example"
        );
    }

    #[test]
    fn infers_plugin_name_from_ssh_url() {
        assert_eq!(
            plugin_name_from_git_url("git@github.com:user/vcd-plugin-example.git").unwrap(),
            "vcd-plugin-example"
        );
    }

    #[test]
    fn infers_plugin_name_with_trailing_slash() {
        assert_eq!(
            plugin_name_from_git_url("https://github.com/user/vcd-plugin-example.git/").unwrap(),
            "vcd-plugin-example"
        );
    }

    #[test]
    fn rejects_unsafe_plugin_name() {
        assert!(plugin_name_from_git_url("https://github.com/user/..git").is_err());
        assert!(plugin_name_from_git_url("https://github.com/user/-plugin.git").is_err());
        assert!(plugin_name_from_git_url("https://github.com/user/bad name.git").is_err());
    }

    #[test]
    fn lists_only_directories_in_sorted_order() {
        let root = temp_dir("plugin-list");
        fs::create_dir_all(root.join("beta")).unwrap();
        fs::create_dir_all(root.join("alpha")).unwrap();
        fs::write(root.join("not-a-plugin"), "").unwrap();

        let plugins = installed_plugins(&root).unwrap();
        let names: Vec<String> = plugins
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
            .collect();

        assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_plugin_root_is_empty() {
        let root = temp_dir("missing-plugin-list");
        assert_eq!(installed_plugins(&root).unwrap(), Vec::<PathBuf>::new());
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("vcd-{prefix}-{nanos}"))
    }
}
