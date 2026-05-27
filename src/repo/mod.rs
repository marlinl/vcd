pub mod github;
pub mod gitlab;

use std::process::{Command, Stdio};

use crate::error::{Result, VcdError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRepo {
    pub url: String,
    pub project: String,
    platform: RepoPlatform,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RepoPlatform {
    GitHub(github::GitHubRepo),
    GitLab(gitlab::GitLabRepo),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchPlan {
    Named(String),
}

impl GitRepo {
    pub fn parse(url: &str) -> Result<Self> {
        let url = url.trim();
        if url.is_empty() {
            return Err(invalid_repo("missing Git repository URL"));
        }

        if let Some(repo) = github::GitHubRepo::parse(url) {
            return Ok(Self {
                url: repo.clone_url.clone(),
                project: repo.project.clone(),
                platform: RepoPlatform::GitHub(repo),
            });
        }

        if let Some(repo) = gitlab::GitLabRepo::parse(url) {
            return Ok(Self {
                url: repo.clone_url.clone(),
                project: repo.project.clone(),
                platform: RepoPlatform::GitLab(repo),
            });
        }

        Err(invalid_repo(format!("unsupported Git URL '{url}'")))
    }

    /// Convert HTTPS URLs to SSH format for private repo access.
    /// SSH keys are mounted into the container, so SSH clone always works.
    pub fn ssh_clone_url(&self) -> String {
        https_to_ssh(&self.url)
    }

    pub fn branch_from_url_or_default(&self) -> Result<String> {
        let branch = match &self.platform {
            RepoPlatform::GitHub(repo) => repo.branch_from_url_or_default()?,
            RepoPlatform::GitLab(repo) => repo.branch_from_url_or_default()?,
        };
        validate_branch(&branch)?;
        Ok(branch)
    }

    pub fn platform_name(&self) -> &'static str {
        match &self.platform {
            RepoPlatform::GitHub(_) => "github",
            RepoPlatform::GitLab(_) => "gitlab",
        }
    }
}

impl BranchPlan {
    pub fn named(branch: &str) -> Result<Self> {
        validate_branch(branch)?;
        Ok(Self::Named(branch.to_string()))
    }
}

pub fn container_safe_name(value: &str) -> String {
    let mut name = String::with_capacity(value.len());
    let mut last_was_dash = false;

    for byte in value.bytes() {
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

    let trimmed = name.trim_matches('-');
    if trimmed.is_empty() {
        "project".to_string()
    } else {
        trimmed.to_string()
    }
}

pub(crate) fn project_name_from_path(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/').trim_end_matches(".git");
    let project = trimmed.rsplit('/').next()?.trim_end_matches(".git");
    if project.is_empty() {
        None
    } else {
        Some(container_safe_name(project))
    }
}

pub(crate) fn percent_encode_path(path: &str) -> String {
    let mut encoded = String::new();
    for byte in path.bytes() {
        let keep = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~');
        if keep {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

pub(crate) fn json_string_field(json: &str, field: &str) -> Option<String> {
    let needle = format!("\"{field}\"");
    let mut rest = json.split_once(&needle)?.1.trim_start();
    rest = rest.strip_prefix(':')?.trim_start();
    let value = rest.strip_prefix('"')?;
    parse_json_string_value(value)
}

pub(crate) fn json_string_field_in_object(json: &str, object: &str, field: &str) -> Option<String> {
    let needle = format!("\"{object}\"");
    let after_object = json.split_once(&needle)?.1;
    let object_start = after_object.find('{')?;
    let object_json = matching_json_object(&after_object[object_start..])?;
    json_string_field(object_json, field)
}

pub(crate) fn run_cli(program: &str, args: &[String], stage: &'static str) -> Result<String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|err| {
            VcdError::new(stage, format!("failed to execute {program}: {err}")).with_hint(format!(
                "请确认本机已安装并登录 {program}，或显式传入 branch"
            ))
        })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(VcdError::new(
            stage,
            format!("{program} exited with status {}", output.status),
        )
        .with_hint(format!(
            "请确认 {program} 已登录且有权限访问该仓库，或显式传入 branch。输出: {}",
            stderr.trim()
        )))
    }
}

pub(crate) fn validate_branch(branch: &str) -> Result<()> {
    let invalid = branch.starts_with('-')
        || branch.starts_with('/')
        || branch.ends_with('/')
        || branch.ends_with(".lock")
        || branch.contains("..")
        || branch.contains("//")
        || branch.contains("@{")
        || branch
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || "~^:?*[\\".contains(ch));

    if invalid {
        Err(VcdError::new(
            "Git 分支解析失败",
            format!("invalid branch name '{branch}'"),
        )
        .with_hint("请传入普通分支名，例如 main、dev、feature-login"))
    } else {
        Ok(())
    }
}

pub(crate) fn invalid_repo(message: impl Into<String>) -> VcdError {
    VcdError::new("Git 仓库地址非法", message)
        .with_hint("请传入常见 HTTPS 或 SSH Git URL，例如 https://github.com/user/project.git")
}

fn https_to_ssh(url: &str) -> String {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or(url);
    if rest == url {
        return url.to_string();
    }
    let Some((host, path)) = rest.split_once('/') else {
        return url.to_string();
    };
    let path = path.trim_end_matches('/');
    format!("git@{host}:{path}")
}

fn parse_json_string_value(value: &str) -> Option<String> {
    let mut output = String::new();
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            output.push(match ch {
                '"' => '"',
                '\\' => '\\',
                '/' => '/',
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(output);
        } else {
            output.push(ch);
        }
    }
    None
}

fn matching_json_object(value: &str) -> Option<&str> {
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (index, ch) in value.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(&value[..=index]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_github_url() {
        let repo = GitRepo::parse("https://github.com/user/project.git").unwrap();
        assert_eq!(repo.platform_name(), "github");
        assert_eq!(repo.project, "project");
    }

    #[test]
    fn detects_gitlab_mr_url() {
        let repo =
            GitRepo::parse("https://gitlab.example.com/team/sample-app/-/merge_requests/759")
                .unwrap();
        assert_eq!(repo.platform_name(), "gitlab");
        assert_eq!(repo.url, "https://gitlab.example.com/team/sample-app.git");
        assert_eq!(repo.project, "sample-app");
    }

    #[test]
    fn detects_gitlab_ssh_url() {
        let repo = GitRepo::parse("git@gitlab.example.com:team/sample-app.git").unwrap();
        assert_eq!(repo.platform_name(), "gitlab");
        assert_eq!(repo.project, "sample-app");
    }

    #[test]
    fn builds_named_branch_plan() {
        assert_eq!(
            BranchPlan::named("feature-a").unwrap(),
            BranchPlan::Named("feature-a".to_string())
        );
    }

    #[test]
    fn rejects_unsafe_branch() {
        assert!(BranchPlan::named("-bad").is_err());
        assert!(BranchPlan::named("bad branch").is_err());
        assert!(BranchPlan::named("bad..branch").is_err());
    }

    #[test]
    fn converts_https_to_ssh() {
        let repo = GitRepo::parse("https://gitlab.example.com/team/sample-app.git").unwrap();
        assert_eq!(
            repo.ssh_clone_url(),
            "git@gitlab.example.com:team/sample-app.git"
        );
    }

    #[test]
    fn keeps_ssh_url_unchanged() {
        let repo = GitRepo::parse("git@github.com:user/project.git").unwrap();
        assert_eq!(repo.ssh_clone_url(), "git@github.com:user/project.git");
    }

    #[test]
    fn percent_encodes_project_path() {
        assert_eq!(
            percent_encode_path("quick-n-dirty/click"),
            "quick-n-dirty%2Fclick"
        );
        assert_eq!(
            percent_encode_path("group/sub group/project"),
            "group%2Fsub%20group%2Fproject"
        );
    }

    #[test]
    fn parses_json_string_field() {
        let json = r#"{"iid":599,"source_branch":"feature/click-599","title":"sample"}"#;
        assert_eq!(
            json_string_field(json, "source_branch"),
            Some("feature/click-599".to_string())
        );
    }

    #[test]
    fn parses_escaped_json_string_field() {
        let json = r#"{"source_branch":"feature\/click-599"}"#;
        assert_eq!(
            json_string_field(json, "source_branch"),
            Some("feature/click-599".to_string())
        );
    }

    #[test]
    fn parses_string_field_inside_object() {
        let json = r#"{"head":{"label":"u:f","ref":"feature-a"},"base":{"ref":"main"}}"#;
        assert_eq!(
            json_string_field_in_object(json, "head", "ref"),
            Some("feature-a".to_string())
        );
    }
}
