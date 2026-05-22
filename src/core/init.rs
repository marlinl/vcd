use std::env;
use std::path::{Path, PathBuf};

use crate::core::prompt::{prompt_init_options, InitOptions};
use crate::docker;
use crate::error::{Result, VcdError};

use super::config;

pub fn run(user: &str) -> Result<()> {
    ensure_not_initialized()?;
    let options = prompt_init_options(user, None)?;
    build_and_write_config(&options)
}

fn ensure_not_initialized() -> Result<()> {
    let config_path = config::default_config_path()?;
    if let Some(initialized_at) = config::read_initialized_at(&config_path)? {
        Err(VcdError::new(
            "初始化失败",
            format!(
                "vcd has already been initialized at {initialized_at}: {}",
                config_path.display()
            ),
        )
        .with_hint(
            "请使用 vcd config set <key> <value> 修改已有配置，例如 vcd config set user.name abc",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn build_and_write_config(options: &InitOptions) -> Result<()> {
    docker::validate_image_user(&options.user_name)?;
    docker::ensure_host_ssh_key(&options.ssh_key_path)?;

    let requested_base_image =
        env::var("VCD_BASE_IMAGE").unwrap_or_else(|_| "debian:trixie-slim".to_string());
    let local_base_image = docker::local_base_alias(&requested_base_image);
    let proxy_url = env::var("VCD_PROXY_URL")
        .unwrap_or_else(|_| "http://host.docker.internal:1087".to_string());
    let no_proxy = env::var("VCD_NO_PROXY")
        .unwrap_or_else(|_| "localhost,127.0.0.1,::1,host.docker.internal,.local".to_string());

    let config_path = config::default_config_path()?;
    let previous_config = config::read_config(&config_path).ok();
    let initialized_at = match previous_config.as_ref() {
        Some(config) => config.initialized_at.clone(),
        None => docker::timestamp()?,
    };
    let image_tag = match previous_config.as_ref() {
        Some(_) => docker::timestamp()?,
        None => initialized_at.clone(),
    };
    let image_name = previous_config
        .as_ref()
        .filter(|config| config.user_name == options.user_name)
        .map(|config| docker::image_name_without_tag(&config.container_id))
        .unwrap_or_else(|| docker::default_container_id(&options.user_name));
    let container_id = docker::base_image_name(&image_name, &image_tag);
    let dockerfile_path = configured_dockerfile_path(&config_path)?;
    let next_config = config::VcdConfig {
        user_name: options.user_name.clone(),
        user_email: options.user_email.clone(),
        ssh_key_path: options.ssh_key_path.clone(),
        initialized_at,
        container_docker_build: dockerfile_path.display().to_string(),
        container_id,
        proxy_url: proxy_url.clone(),
        no_proxy: no_proxy.clone(),
    };
    config::write_config(&config_path, &next_config)?;

    let build_config = config::read_config(&config_path)?;
    let build_files = docker::write_build_files(&build_config)?;

    println!(
        "Configured vcd user '{}' at {}",
        options.user_name,
        config_path.display()
    );
    println!("Dockerfile: {}", build_files.dockerfile.display());
    println!(
        "Optional packages: {}",
        build_files.optional_packages.display()
    );
    println!("Building base image {}", build_config.container_id);
    println!("Base Debian image: {requested_base_image}");
    println!("Local base alias: {local_base_image}");
    if proxy_url.is_empty() {
        println!("Proxy: disabled");
    } else {
        println!("Proxy: {proxy_url}");
    }
    docker::ensure_docker_ready()?;
    docker::prepare_local_base_image(&requested_base_image, &local_base_image)?;
    docker::build_base_image(&docker::BuildRequest {
        config: build_config.clone(),
        base_image: local_base_image,
        proxy_url,
        no_proxy,
    })?;

    println!(
        "Built base image {} from {}",
        build_config.container_id, build_config.container_docker_build
    );
    Ok(())
}

fn configured_dockerfile_path(config_path: &Path) -> Result<PathBuf> {
    match config::read_config(config_path) {
        Ok(config) if !config.container_docker_build.trim().is_empty() => {
            Ok(PathBuf::from(config.container_docker_build))
        }
        _ => docker::default_dockerfile_path(),
    }
}
