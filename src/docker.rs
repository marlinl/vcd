use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::core::config::{self, VcdConfig};
use crate::error::{Result, VcdError};
use crate::repo::{self, BranchPlan, GitRepo};

const EMBEDDED_DOCKERFILE: &str = include_str!("../docker/Dockerfile");
const EMBEDDED_OPTIONAL_PACKAGES: &str = include_str!("../docker/optional-packages.txt");

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildFiles {
    pub dockerfile: PathBuf,
    pub optional_packages: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildRequest {
    pub config: VcdConfig,
    pub base_image: String,
    pub proxy_url: String,
    pub no_proxy: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerRequest {
    pub name: String,
    pub image: String,
    pub user: String,
    pub ssh_key_path: String,
    pub proxy_url: String,
    pub no_proxy: String,
}

pub fn default_dockerfile_path() -> Result<PathBuf> {
    Ok(config::default_config_dir()?.join("Dockerfile"))
}

pub fn write_build_files(config: &VcdConfig) -> Result<BuildFiles> {
    let (dockerfile, context) = configured_build_paths(config)?;
    fs::create_dir_all(&context).map_err(|err| {
        VcdError::new(
            "Docker 初始化失败",
            format!("failed to create {}: {err}", context.display()),
        )
    })?;

    let optional_packages = context.join("optional-packages.txt");
    write_embedded_file(&dockerfile, EMBEDDED_DOCKERFILE)?;
    write_embedded_file(&optional_packages, EMBEDDED_OPTIONAL_PACKAGES)?;

    Ok(BuildFiles {
        dockerfile,
        optional_packages,
    })
}

fn write_embedded_file(path: &Path, content: &str) -> Result<()> {
    fs::write(path, content).map_err(|err| {
        VcdError::new(
            "Docker 初始化失败",
            format!("failed to write embedded {}: {err}", path.display()),
        )
    })
}

pub fn validate_image_user(user: &str) -> Result<()> {
    if user.is_empty() || user.len() > 32 {
        return Err(invalid_user());
    }

    let mut chars = user.chars();
    let first = chars.next().ok_or_else(invalid_user)?;
    if !(first.is_ascii_lowercase() || first == '_') {
        return Err(invalid_user());
    }

    if chars.any(|ch| !(ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')) {
        return Err(invalid_user());
    }

    Ok(())
}

pub fn timestamp() -> Result<String> {
    let output = Command::new("date")
        .arg("+%Y%m%d%H%M%S")
        .output()
        .map_err(|err| {
            VcdError::new("系统命令失败", format!("failed to execute date: {err}"))
                .with_hint("请确认当前系统提供 date 命令")
        })?;

    if !output.status.success() {
        return Err(VcdError::new(
            "系统命令失败",
            format!("date exited with status {}", output.status),
        ));
    }

    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.len() != 14 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(VcdError::new(
            "系统命令失败",
            format!("unexpected timestamp from date: {value}"),
        ));
    }

    Ok(value)
}

pub fn default_container_id(user: &str) -> String {
    format!("vcd-{user}")
}

pub fn base_image_name(container_id: &str, timestamp: &str) -> String {
    format!("{container_id}:{timestamp}")
}

pub fn image_name_without_tag(image: &str) -> String {
    image
        .rsplit_once(':')
        .map(|(name, _)| name)
        .unwrap_or(image)
        .to_string()
}

pub fn local_base_alias(base_image: &str) -> String {
    let mut name = String::with_capacity(base_image.len());
    let mut last_was_dash = false;

    for byte in base_image.bytes() {
        let ch = byte.to_ascii_lowercase() as char;
        let is_name_char = ch.is_ascii_lowercase() || ch.is_ascii_digit();
        if is_name_char {
            name.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            name.push('-');
            last_was_dash = true;
        }
    }

    let name = name.trim_matches('-');
    if name.is_empty() {
        "vcd-base-image:local".to_string()
    } else {
        format!("vcd-base-{name}:local")
    }
}

pub fn prepare_local_base_image(base_image: &str, local_alias: &str) -> Result<()> {
    if !image_exists(base_image)? {
        let status = Command::new("docker")
            .args(["pull", base_image])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|err| {
                VcdError::new(
                    "基础镜像准备失败",
                    format!("failed to execute docker pull {base_image}: {err}"),
                )
            })?;

        if !status.success() {
            return Err(VcdError::new(
                "基础镜像准备失败",
                format!("docker pull {base_image} exited with status {status}"),
            )
            .with_hint("请先确认该基础镜像可以在当前终端中 docker pull 成功"));
        }
    }

    let status = Command::new("docker")
        .args(["tag", base_image, local_alias])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| {
            VcdError::new(
                "基础镜像准备失败",
                format!("failed to execute docker tag {base_image} {local_alias}: {err}"),
            )
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(VcdError::new(
            "基础镜像准备失败",
            format!("docker tag {base_image} {local_alias} exited with status {status}"),
        ))
    }
}

fn image_exists(image: &str) -> Result<bool> {
    let status = Command::new("docker")
        .args(["image", "inspect", image])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| {
            VcdError::new(
                "基础镜像准备失败",
                format!("failed to execute docker image inspect {image}: {err}"),
            )
        })?;

    Ok(status.success())
}

pub fn ensure_docker_ready() -> Result<()> {
    let status = Command::new("docker")
        .args(["version", "--format", "{{.Server.Version}}"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| {
            VcdError::new(
                "Docker CLI 不可用",
                format!("failed to execute docker: {err}"),
            )
            .with_hint("请先安装 Docker CLI，并确认 OrbStack 或 Docker Desktop 已启动")
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(VcdError::new(
            "Docker daemon 不可连接",
            format!("docker version exited with status {status}"),
        )
        .with_hint("请检查 OrbStack 或 Docker Desktop 是否已经启动，并确认当前用户可访问 Docker"))
    }
}

pub fn build_base_image(request: &BuildRequest) -> Result<()> {
    let args = build_args(request)?;
    let status = Command::new("docker")
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| {
            VcdError::new(
                "基础镜像构建失败",
                format!("failed to execute docker build: {err}"),
            )
            .with_hint("请确认 Docker CLI 已安装，并且 ~/.config/vcd/Dockerfile 存在")
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(VcdError::new(
            "基础镜像构建失败",
            format!("docker build exited with status {status}"),
        ))
        .map_err(|err| {
            err.with_hint(
                "如果卡在 docker.io/library/debian:trixie-slim，可先配置 Docker registry mirror，或用 VCD_BASE_IMAGE 指向本地已有/可访问的 Debian 镜像",
            )
        })
    }
}

pub fn build_args(request: &BuildRequest) -> Result<Vec<String>> {
    let (dockerfile, context) = configured_build_paths(&request.config)?;

    Ok(vec![
        "build".to_string(),
        "--build-arg".to_string(),
        format!("BASE_IMAGE={}", request.base_image),
        "--build-arg".to_string(),
        format!("USERNAME={}", request.config.user_name),
        "--build-arg".to_string(),
        format!("GIT_USERNAME={}", request.config.user_name),
        "--build-arg".to_string(),
        format!("GIT_EMAIL={}", request.config.user_email),
        "--build-arg".to_string(),
        format!(
            "SSH_KEY={}",
            ssh_key_file_name(&request.config.ssh_key_path)?
        ),
        "--build-arg".to_string(),
        format!("VCD_PROXY_URL={}", request.proxy_url),
        "--build-arg".to_string(),
        format!("VCD_NO_PROXY={}", request.no_proxy),
        "-t".to_string(),
        request.config.container_id.clone(),
        "-f".to_string(),
        dockerfile.display().to_string(),
        context.display().to_string(),
    ])
}

fn configured_build_paths(config: &VcdConfig) -> Result<(PathBuf, PathBuf)> {
    if config.container_docker_build.trim().is_empty() {
        return Err(VcdError::new(
            "配置读取失败",
            "missing required config key 'container.docker_build'",
        )
        .with_hint("请重新运行 vcd init <user> 生成 ~/.config/vcd/Dockerfile 和完整配置"));
    }

    let dockerfile = PathBuf::from(&config.container_docker_build);
    let context = dockerfile
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            VcdError::new(
                "配置读取失败",
                format!(
                    "container.docker_build must include a parent directory: {}",
                    dockerfile.display()
                ),
            )
            .with_hint("请重新运行 vcd init <user> 生成默认 Dockerfile 路径")
        })?;

    Ok((dockerfile, context))
}

pub fn ensure_image_exists(image: &str) -> Result<()> {
    if image_exists(image)? {
        Ok(())
    } else {
        Err(VcdError::new(
            "基础镜像不存在",
            format!("local image '{image}' was not found"),
        )
        .with_hint("请先运行 vcd init <user> 构建本地基础镜像"))
    }
}

pub fn container_name(user: &str, editor: &str, project: &str, timestamp: &str) -> String {
    format!(
        "vcd-{}-{}-{}-{}",
        repo::container_safe_name(user),
        repo::container_safe_name(editor),
        repo::container_safe_name(project),
        repo::container_safe_name(timestamp)
    )
}

pub fn ensure_container(request: &ContainerRequest) -> Result<()> {
    match container_status(&request.name)? {
        Some(status) if status == "running" => ensure_ssh_mount(request),
        Some(status) if status == "paused" => {
            docker_checked(
                &["unpause", request.name.as_str()],
                "容器启动失败",
                "docker unpause failed",
            )?;
            ensure_ssh_mount(request)
        }
        Some(_) => {
            docker_checked(
                &["start", request.name.as_str()],
                "容器启动失败",
                "docker start failed",
            )?;
            ensure_ssh_mount(request)
        }
        None => {
            ensure_host_ssh_key(&request.ssh_key_path)?;
            let codex_host = format!(
                "/Users/{}/.codex:/Users/{}/.codex",
                request.user, request.user
            );
            let claude_host = format!(
                "/Users/{}/.claude:/Users/{}/.claude",
                request.user, request.user
            );
            let mut args = vec![
                "run".to_string(),
                "-d".to_string(),
                "--name".to_string(),
                request.name.clone(),
                "-v".to_string(),
                codex_host,
                "-v".to_string(),
                claude_host,
                "-v".to_string(),
                ssh_file_mount(&request.user, &request.ssh_key_path)?,
            ];
            add_optional_ssh_mount(
                &mut args,
                &request.user,
                &format!("{}.pub", request.ssh_key_path),
            )?;
            add_optional_ssh_mount_from_parent(
                &mut args,
                &request.user,
                &request.ssh_key_path,
                "known_hosts",
            )?;
            add_optional_ssh_mount_from_parent(
                &mut args,
                &request.user,
                &request.ssh_key_path,
                "config",
            )?;
            inject_proxy_env(&mut args, &request.proxy_url, &request.no_proxy);
            args.extend([
                request.image.clone(),
                "sleep".to_string(),
                "infinity".to_string(),
            ]);

            docker_checked_strings(&args, "容器创建失败", "docker run failed")
        }
    }
}

fn inject_proxy_env(args: &mut Vec<String>, proxy_url: &str, no_proxy: &str) {
    if !proxy_url.is_empty() {
        for key in [
            "HTTP_PROXY",
            "HTTPS_PROXY",
            "ALL_PROXY",
            "http_proxy",
            "https_proxy",
            "all_proxy",
        ] {
            args.extend(["-e".to_string(), format!("{key}={proxy_url}")]);
        }
    }
    if !no_proxy.is_empty() {
        args.extend(["-e".to_string(), format!("NO_PROXY={no_proxy}")]);
        args.extend(["-e".to_string(), format!("no_proxy={no_proxy}")]);
    }
}

pub fn remove_container(container: &str) -> Result<()> {
    docker_checked(
        &["rm", "-f", container],
        "容器清理失败",
        "docker rm -f failed",
    )
}

pub fn prepare_repo(
    container: &str,
    user: &str,
    repo: &GitRepo,
    branch: &BranchPlan,
) -> Result<()> {
    let project_path = project_path(user, &repo.project);
    let clone_url = repo.ssh_clone_url();

    if repo_exists(container, &project_path)? {
        docker_exec_checked(
            container,
            &[
                "git",
                "-C",
                project_path.as_str(),
                "fetch",
                "origin",
                "--prune",
            ],
            "仓库更新失败",
            "git fetch failed",
        )?;
    } else {
        docker_exec_checked(
            container,
            &["git", "clone", clone_url.as_str(), project_path.as_str()],
            "仓库 clone 失败",
            "git clone failed",
        )?;
    }

    checkout_branch(container, &project_path, branch)
}

pub fn open_editor(container: &str, user: &str, editor: &str, project: &str) -> Result<()> {
    let project_path = project_path(user, project);
    docker_checked_interactive(
        &[
            "exec",
            "-it",
            "-w",
            project_path.as_str(),
            container,
            editor,
            ".",
        ],
        "编辑器启动失败",
        "docker exec failed",
    )
}

fn checkout_branch(container: &str, project_path: &str, branch: &BranchPlan) -> Result<()> {
    match branch {
        BranchPlan::Named(branch) => {
            if local_branch_exists(container, project_path, branch)? {
                docker_exec_checked(
                    container,
                    &["git", "-C", project_path, "checkout", branch.as_str()],
                    "Git 分支切换失败",
                    "git checkout failed",
                )?;
            } else {
                let remote = format!("origin/{branch}");
                docker_exec_checked(
                    container,
                    &[
                        "git",
                        "-C",
                        project_path,
                        "checkout",
                        "-B",
                        branch.as_str(),
                        remote.as_str(),
                    ],
                    "Git 分支切换失败",
                    "git checkout remote branch failed",
                )?;
            }

            docker_exec_checked(
                container,
                &[
                    "git",
                    "-C",
                    project_path,
                    "pull",
                    "--ff-only",
                    "origin",
                    branch.as_str(),
                ],
                "Git 分支更新失败",
                "git pull --ff-only failed",
            )
        }
        BranchPlan::TempFromMaster { base, branch } => {
            docker_exec_checked(
                container,
                &["git", "-C", project_path, "fetch", "origin", base.as_str()],
                "Git 分支更新失败",
                "git fetch origin master failed",
            )?;
            docker_exec_checked(
                container,
                &["git", "-C", project_path, "checkout", base.as_str()],
                "Git 分支切换失败",
                "git checkout master failed",
            )?;
            docker_exec_checked(
                container,
                &[
                    "git",
                    "-C",
                    project_path,
                    "pull",
                    "--ff-only",
                    "origin",
                    base.as_str(),
                ],
                "Git 分支更新失败",
                "git pull --ff-only origin master failed",
            )?;

            if local_branch_exists(container, project_path, branch)? {
                docker_exec_checked(
                    container,
                    &["git", "-C", project_path, "checkout", branch.as_str()],
                    "Git 分支切换失败",
                    "git checkout temp failed",
                )
            } else {
                docker_exec_checked(
                    container,
                    &["git", "-C", project_path, "checkout", "-b", branch.as_str()],
                    "Git 分支切换失败",
                    "git checkout -b temp failed",
                )
            }
        }
        BranchPlan::MergeRequest { iid } => {
            let refspec = format!("refs/merge-requests/{iid}/head:mr/{iid}");
            docker_exec_checked(
                container,
                &[
                    "git",
                    "-C",
                    project_path,
                    "fetch",
                    "origin",
                    refspec.as_str(),
                ],
                "MR 分支拉取失败",
                "git fetch merge request ref failed",
            )?;
            let local_branch = format!("mr/{iid}");
            docker_exec_checked(
                container,
                &["git", "-C", project_path, "checkout", local_branch.as_str()],
                "MR 分支切换失败",
                "git checkout mr branch failed",
            )
        }
    }
}

fn project_path(user: &str, project: &str) -> String {
    format!("/home/{user}/{project}")
}

fn container_ssh_file(user: &str, file: &str) -> String {
    format!("/home/{user}/.ssh/{file}")
}

fn ssh_key_file_name(path: &str) -> Result<String> {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            VcdError::new(
                "SSH 配置失败",
                format!("SSH key path must include a file name: {path}"),
            )
        })
}

fn ssh_file_mount(user: &str, path: &str) -> Result<String> {
    let key = PathBuf::from(path);
    let file = ssh_key_file_name(path)?;
    Ok(format!(
        "{}:{}:ro",
        key.display(),
        container_ssh_file(user, &file)
    ))
}

fn add_optional_ssh_mount(args: &mut Vec<String>, user: &str, path: &str) -> Result<()> {
    if Path::new(path).is_file() {
        args.push("-v".to_string());
        args.push(ssh_file_mount(user, path)?);
    }
    Ok(())
}

fn add_optional_ssh_mount_from_parent(
    args: &mut Vec<String>,
    user: &str,
    ssh_key_path: &str,
    file: &str,
) -> Result<()> {
    let Some(parent) = Path::new(ssh_key_path).parent() else {
        return Ok(());
    };
    let path = parent.join(file);
    if path.is_file() {
        add_optional_ssh_mount(args, user, &path.display().to_string())?;
    }
    Ok(())
}

pub fn ensure_host_ssh_key(ssh_key_path: &str) -> Result<()> {
    let key = PathBuf::from(ssh_key_path);
    if key.is_file() {
        Ok(())
    } else {
        Err(VcdError::new(
            "SSH 配置失败",
            format!("SSH key file was not found: {}", key.display()),
        )
        .with_hint(
            "请确认 vcd init 中填写的是宿主机 SSH 私钥的绝对路径，例如 /Users/me/.ssh/id_ed25519",
        ))
    }
}

fn ensure_ssh_mount(request: &ContainerRequest) -> Result<()> {
    ensure_host_ssh_key(&request.ssh_key_path)?;
    let expected = container_ssh_file(&request.user, &ssh_key_file_name(&request.ssh_key_path)?);
    let output = Command::new("docker")
        .args([
            "container",
            "inspect",
            "-f",
            "{{range .Mounts}}{{println .Destination}}{{end}}",
            request.name.as_str(),
        ])
        .output()
        .map_err(|err| {
            VcdError::new(
                "容器检查失败",
                format!("failed to execute docker container inspect: {err}"),
            )
        })?;

    if !output.status.success() {
        return Err(VcdError::new(
            "容器检查失败",
            format!(
                "docker container inspect exited with status {}",
                output.status
            ),
        ));
    }

    let mounts = String::from_utf8_lossy(&output.stdout);
    if mounts.lines().any(|line| line == expected) {
        Ok(())
    } else {
        Err(VcdError::new(
            "容器配置不匹配",
            format!(
                "container '{}' does not mount {}",
                request.name, expected
            ),
        )
        .with_hint(format!(
            "这是旧容器或 SSH key 配置变更后的容器；如无需要保留的容器内改动，请执行 docker rm -f {} 后重试",
            request.name
        )))
    }
}

fn container_status(container: &str) -> Result<Option<String>> {
    let output = Command::new("docker")
        .args(["container", "inspect", "-f", "{{.State.Status}}", container])
        .output()
        .map_err(|err| {
            VcdError::new(
                "容器检查失败",
                format!("failed to execute docker container inspect: {err}"),
            )
        })?;

    if output.status.success() {
        let status = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(Some(status))
    } else {
        Ok(None)
    }
}

fn repo_exists(container: &str, project_path: &str) -> Result<bool> {
    docker_exec_status(container, &["test", "-d", &format!("{project_path}/.git")])
}

fn local_branch_exists(container: &str, project_path: &str, branch: &str) -> Result<bool> {
    let ref_name = format!("refs/heads/{branch}");
    docker_exec_status(
        container,
        &[
            "git",
            "-C",
            project_path,
            "show-ref",
            "--verify",
            "--quiet",
            ref_name.as_str(),
        ],
    )
}

fn docker_exec_status(container: &str, command: &[&str]) -> Result<bool> {
    let status = Command::new("docker")
        .arg("exec")
        .arg(container)
        .args(command)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| {
            VcdError::new(
                "容器命令执行失败",
                format!("failed to execute docker exec: {err}"),
            )
        })?;

    Ok(status.success())
}

fn docker_exec_checked(
    container: &str,
    command: &[&str],
    stage: &'static str,
    message: &'static str,
) -> Result<()> {
    let mut args = Vec::with_capacity(command.len() + 2);
    args.push("exec");
    args.push(container);
    args.extend(command.iter().copied());
    docker_checked_interactive(&args, stage, message)
}

fn docker_checked(args: &[&str], stage: &'static str, message: &'static str) -> Result<()> {
    run_docker(args, stage, message, false)
}

fn docker_checked_strings(
    args: &[String],
    stage: &'static str,
    message: &'static str,
) -> Result<()> {
    run_docker(args, stage, message, false)
}

fn docker_checked_interactive(
    args: &[&str],
    stage: &'static str,
    message: &'static str,
) -> Result<()> {
    run_docker(args, stage, message, true)
}

fn run_docker<S: AsRef<std::ffi::OsStr>>(
    args: &[S],
    stage: &'static str,
    message: &'static str,
    inherit_stdin: bool,
) -> Result<()> {
    let mut command = Command::new("docker");
    command
        .args(args)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if inherit_stdin {
        command.stdin(Stdio::inherit());
    } else {
        command.stdin(Stdio::null());
    }

    let status = command
        .status()
        .map_err(|err| VcdError::new(stage, format!("{message}: {err}")))?;

    if status.success() {
        Ok(())
    } else {
        Err(VcdError::new(
            stage,
            format!("{message} with status {status}"),
        ))
    }
}

fn invalid_user() -> VcdError {
    VcdError::new("参数解析失败", "invalid user for vcd init").with_hint(
        "用户名称需为 1-32 位，只包含小写字母、数字、下划线或连字符，并以小写字母或下划线开头",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_user_for_docker_and_linux_useradd() {
        assert!(validate_image_user("jack").is_ok());
        assert!(validate_image_user("user_1").is_ok());
        assert!(validate_image_user("user-name").is_ok());
        assert!(validate_image_user("User").is_err());
        assert!(validate_image_user("1user").is_err());
        assert!(validate_image_user("user.name").is_err());
        assert!(validate_image_user("user/name").is_err());
    }

    #[test]
    fn builds_image_name() {
        assert_eq!(
            base_image_name("vcd-jack", "20260520103000"),
            "vcd-jack:20260520103000"
        );
    }

    #[test]
    fn builds_local_base_alias() {
        assert_eq!(
            local_base_alias("debian:trixie-slim"),
            "vcd-base-debian-trixie-slim:local"
        );
        assert_eq!(
            local_base_alias("registry.example.com/library/debian:trixie-slim"),
            "vcd-base-registry-example-com-library-debian-trixie-slim:local"
        );
    }

    #[test]
    fn builds_docker_build_args() {
        let args = build_args(&BuildRequest {
            config: VcdConfig {
                user_name: "jack".to_string(),
                user_email: "jack@example.com".to_string(),
                ssh_key_path: "/Users/jack/.ssh/id_rsa".to_string(),
                initialized_at: "20260520103000".to_string(),
                container_docker_build: "/Users/jack/.config/vcd/Dockerfile".to_string(),
                container_id: "vcd-jack:20260520103000".to_string(),
                proxy_url: "http://host.docker.internal:1087".to_string(),
                no_proxy: "localhost,127.0.0.1,::1,host.docker.internal,.local".to_string(),
            },
            base_image: "debian:trixie-slim".to_string(),
            proxy_url: "http://host.docker.internal:1087".to_string(),
            no_proxy: "localhost,127.0.0.1,::1,host.docker.internal,.local".to_string(),
        })
        .unwrap();

        assert_eq!(
            args,
            vec![
                "build",
                "--build-arg",
                "BASE_IMAGE=debian:trixie-slim",
                "--build-arg",
                "USERNAME=jack",
                "--build-arg",
                "GIT_USERNAME=jack",
                "--build-arg",
                "GIT_EMAIL=jack@example.com",
                "--build-arg",
                "SSH_KEY=id_rsa",
                "--build-arg",
                "VCD_PROXY_URL=http://host.docker.internal:1087",
                "--build-arg",
                "VCD_NO_PROXY=localhost,127.0.0.1,::1,host.docker.internal,.local",
                "-t",
                "vcd-jack:20260520103000",
                "-f",
                "/Users/jack/.config/vcd/Dockerfile",
                "/Users/jack/.config/vcd",
            ]
        );
    }

    #[test]
    fn rejects_build_without_configured_dockerfile() {
        let error = build_args(&BuildRequest {
            config: VcdConfig {
                user_name: "jack".to_string(),
                user_email: "jack@example.com".to_string(),
                ssh_key_path: "/Users/jack/.ssh/id_rsa".to_string(),
                initialized_at: "20260520103000".to_string(),
                container_docker_build: String::new(),
                container_id: "vcd-jack:20260520103000".to_string(),
                proxy_url: String::new(),
                no_proxy: String::new(),
            },
            base_image: "debian:trixie-slim".to_string(),
            proxy_url: String::new(),
            no_proxy: String::new(),
        })
        .unwrap_err();

        assert!(error.to_string().contains("container.docker_build"));
    }

    #[test]
    fn builds_container_name() {
        assert_eq!(
            container_name("jack", "codex", "project", "20260520103000"),
            "vcd-jack-codex-project-20260520103000"
        );
    }

    #[test]
    fn builds_project_path_under_container_home() {
        assert_eq!(project_path("jack", "project"), "/home/jack/project");
    }

    #[test]
    fn injects_proxy_env_vars() {
        let mut args = vec!["run".to_string()];
        inject_proxy_env(
            &mut args,
            "http://host.docker.internal:1087",
            "localhost,127.0.0.1",
        );
        assert!(args.contains(&"-e".to_string()));
        assert!(args.contains(&"HTTP_PROXY=http://host.docker.internal:1087".to_string()));
        assert!(args.contains(&"NO_PROXY=localhost,127.0.0.1".to_string()));
    }

    #[test]
    fn skips_proxy_env_when_empty() {
        let mut args = vec!["run".to_string()];
        inject_proxy_env(&mut args, "", "");
        assert_eq!(args, vec!["run".to_string()]);
    }
}
