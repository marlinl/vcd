use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::core::{config, plugin};
use crate::error::{Result, VcdError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub plugins: Vec<String>,
}

pub fn add(profile_name: &str, plugin_name: &str) -> Result<()> {
    validate_profile_name(profile_name)?;
    plugin::validate_plugin_name(plugin_name).map_err(|_| invalid_plugin_name(plugin_name))?;

    let plugin_path = plugin::plugin_root()?.join(plugin_name);
    if !plugin_path.is_dir() {
        return Err(VcdError::new(
            "plugin 不存在",
            format!("plugin directory not found: {}", plugin_path.display()),
        )
        .with_hint("请先执行 vcd plugin add <git-url> 添加该插件"));
    }

    let root = profile_root()?;
    fs::create_dir_all(&root).map_err(|err| {
        VcdError::new(
            "profile 目录创建失败",
            format!("failed to create {}: {err}", root.display()),
        )
    })?;

    let path = profile_path(&root, profile_name);
    let mut plugins = read_profile_plugins_optional(&path)?;
    let inserted = plugins.insert(plugin_name.to_string());
    write_profile_plugins(&path, &plugins)?;

    if inserted {
        println!("Profile updated: {profile_name}");
        println!("Plugin added: {plugin_name}");
    } else {
        println!("Profile unchanged: {profile_name}");
        println!("Plugin already added: {plugin_name}");
    }

    Ok(())
}

pub fn show(profile_name: &str) -> Result<()> {
    validate_profile_name(profile_name)?;

    let root = profile_root()?;
    let path = profile_path(&root, profile_name);
    let plugins = read_profile_plugins_existing(&path, profile_name)?;
    let profile = Profile {
        name: profile_name.to_string(),
        plugins: plugins.into_iter().collect(),
    };

    print_profile(&profile);
    Ok(())
}

pub fn load(profile_name: &str) -> Result<Profile> {
    validate_profile_name(profile_name)?;

    let root = profile_root()?;
    let path = profile_path(&root, profile_name);
    let plugins = read_profile_plugins_existing(&path, profile_name)?;
    let plugin_root = plugin::plugin_root()?;
    let mut validated = Vec::with_capacity(plugins.len());

    for plugin_name in plugins {
        plugin::validate_plugin_name(&plugin_name)
            .map_err(|_| invalid_plugin_name(&plugin_name))?;
        let plugin_path = plugin_root.join(&plugin_name);
        if !plugin_path.is_dir() {
            return Err(VcdError::new(
                "profile plugin 不存在",
                format!("plugin directory not found: {}", plugin_path.display()),
            )
            .with_hint("请先执行 vcd plugin add <git-url> 添加该插件"));
        }
        validated.push(plugin_name);
    }

    Ok(Profile {
        name: profile_name.to_string(),
        plugins: validated,
    })
}

pub fn print_profile(profile: &Profile) {
    println!("Profile: {}", profile.name);
    if profile.plugins.is_empty() {
        println!("Plugins: none");
    } else {
        println!("Plugins:");
        for plugin in &profile.plugins {
            println!("{plugin}");
        }
    }
}

pub(crate) fn profile_root() -> Result<PathBuf> {
    Ok(config::default_config_dir()?.join("profiles"))
}

fn profile_path(root: &Path, profile_name: &str) -> PathBuf {
    root.join(profile_name)
}

fn read_profile_plugins_optional(path: &Path) -> Result<BTreeSet<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(parse_profile_plugins(&content)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(BTreeSet::new()),
        Err(err) => Err(VcdError::new(
            "profile 读取失败",
            format!("failed to read {}: {err}", path.display()),
        )),
    }
}

fn read_profile_plugins_existing(path: &Path, profile_name: &str) -> Result<BTreeSet<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(parse_profile_plugins(&content)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Err(VcdError::new(
            "profile 读取失败",
            format!("profile '{profile_name}' does not exist"),
        )
        .with_hint(format!(
            "请先执行 vcd profile {profile_name} add <plugin-name>"
        ))),
        Err(err) => Err(VcdError::new(
            "profile 读取失败",
            format!("failed to read {}: {err}", path.display()),
        )),
    }
}

fn parse_profile_plugins(content: &str) -> BTreeSet<String> {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn write_profile_plugins(path: &Path, plugins: &BTreeSet<String>) -> Result<()> {
    let content = plugins
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\n");
    let content = if content.is_empty() {
        String::new()
    } else {
        format!("{content}\n")
    };

    fs::write(path, content).map_err(|err| {
        VcdError::new(
            "profile 写入失败",
            format!("failed to write {}: {err}", path.display()),
        )
    })
}

pub(crate) fn validate_profile_name(name: &str) -> Result<()> {
    let invalid = name.is_empty()
        || name == "."
        || name == ".."
        || name.starts_with('-')
        || name
            .chars()
            .any(|ch| !(ch.is_ascii_alphanumeric() || ch == '.' || ch == '_' || ch == '-'));

    if invalid {
        Err(VcdError::new(
            "profile name 非法",
            format!("invalid profile name '{name}'"),
        )
        .with_hint("请使用简单 profile 名称，例如 backend、frontend、rust-tools"))
    } else {
        Ok(())
    }
}

fn invalid_plugin_name(name: &str) -> VcdError {
    VcdError::new("plugin name 非法", format!("invalid plugin name '{name}'"))
        .with_hint("请传入已经通过 vcd plugin add 安装的 plugin 目录名")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn validates_profile_name() {
        assert!(validate_profile_name("backend").is_ok());
        assert!(validate_profile_name("rust-tools").is_ok());
        assert!(validate_profile_name(".").is_err());
        assert!(validate_profile_name("..").is_err());
        assert!(validate_profile_name("-backend").is_err());
        assert!(validate_profile_name("bad name").is_err());
    }

    #[test]
    fn parses_profile_plugins_with_dedup_and_sort() {
        let plugins = parse_profile_plugins("beta\n\nalpha\nbeta\n");
        let values: Vec<String> = plugins.into_iter().collect();

        assert_eq!(values, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn writes_profile_plugins_sorted() {
        let root = temp_dir("profile-write");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("backend");
        let plugins = BTreeSet::from(["beta".to_string(), "alpha".to_string()]);

        write_profile_plugins(&path, &plugins).unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), "alpha\nbeta\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_optional_profile_reads_as_empty() {
        let root = temp_dir("profile-missing");
        let path = root.join("backend");

        assert_eq!(
            read_profile_plugins_optional(&path).unwrap(),
            BTreeSet::<String>::new()
        );
    }

    fn temp_dir(prefix: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("vcd-{prefix}-{nanos}"))
    }
}
