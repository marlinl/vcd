use crate::error::{Result, VcdError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitRepo {
    pub url: String,
    pub project: String,
    pub mr_iid: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchPlan {
    Named(String),
    TempFromMaster { base: String, branch: String },
    MergeRequest { iid: String },
}

impl GitRepo {
    /// Convert HTTPS URLs to SSH format for private repo access.
    /// SSH keys are mounted into the container, so SSH clone always works.
    pub fn ssh_clone_url(&self) -> String {
        https_to_ssh(&self.url)
    }

    pub fn parse(url: &str) -> Result<Self> {
        let url = url.trim();
        if url.is_empty() {
            return Err(invalid_repo("missing Git repository URL"));
        }

        if let Some((base_url, project, mr_iid)) = parse_merge_request_url(url) {
            return Ok(Self {
                url: base_url,
                project,
                mr_iid: Some(mr_iid),
            });
        }

        if !(url.starts_with("http://")
            || url.starts_with("https://")
            || url.starts_with("git@")
            || url.starts_with("ssh://"))
        {
            return Err(invalid_repo(format!("unsupported Git URL '{url}'")));
        }

        let project = project_name(url)?;
        Ok(Self {
            url: url.to_string(),
            project,
            mr_iid: None,
        })
    }
}

impl BranchPlan {
    pub fn from_optional(branch: Option<&str>) -> Result<Self> {
        match branch.map(str::trim).filter(|branch| !branch.is_empty()) {
            Some(branch) => {
                validate_branch(branch)?;
                Ok(Self::Named(branch.to_string()))
            }
            None => Ok(Self::TempFromMaster {
                base: "master".to_string(),
                branch: "temp".to_string(),
            }),
        }
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

fn parse_merge_request_url(url: &str) -> Option<(String, String, String)> {
    let marker = "/-/merge_requests/";
    let mr_pos = url.find(marker)?;
    let (base, iid_with_suffix) = url.split_at(mr_pos);
    let iid = iid_with_suffix
        .strip_prefix(marker)?
        .split(&['/', '?', '#'])
        .next()?;

    if iid.is_empty() || !iid.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let base_trimmed = base.trim_end_matches('/');
    let last_segment = base_trimmed.rsplit('/').next()?;
    if last_segment.is_empty() {
        return None;
    }

    let clone_url = format!("{base_trimmed}.git");
    let project = container_safe_name(last_segment);

    Some((clone_url, project, iid.to_string()))
}

fn project_name(url: &str) -> Result<String> {
    let trimmed = url.trim_end_matches('/');
    let last = trimmed
        .rsplit(['/', ':'])
        .next()
        .filter(|part| !part.is_empty())
        .ok_or_else(|| invalid_repo(format!("cannot infer project name from '{url}'")))?;
    let project = last.strip_suffix(".git").unwrap_or(last);

    if project.is_empty() {
        return Err(invalid_repo(format!(
            "cannot infer project name from '{url}'"
        )));
    }

    Ok(container_safe_name(project))
}

fn validate_branch(branch: &str) -> Result<()> {
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

fn invalid_repo(message: impl Into<String>) -> VcdError {
    VcdError::new("Git 仓库地址非法", message)
        .with_hint("请传入常见 HTTPS 或 SSH Git URL，例如 https://github.com/user/project.git")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_project_from_https_url() {
        let repo = GitRepo::parse("https://github.com/user/project.git").unwrap();
        assert_eq!(repo.project, "project");
    }

    #[test]
    fn infers_project_from_ssh_url() {
        let repo = GitRepo::parse("git@github.com:user/project.git").unwrap();
        assert_eq!(repo.project, "project");
    }

    #[test]
    fn defaults_to_temp_from_master() {
        assert_eq!(
            BranchPlan::from_optional(None).unwrap(),
            BranchPlan::TempFromMaster {
                base: "master".to_string(),
                branch: "temp".to_string()
            }
        );
    }

    #[test]
    fn rejects_unsafe_branch() {
        assert!(BranchPlan::from_optional(Some("-bad")).is_err());
        assert!(BranchPlan::from_optional(Some("bad branch")).is_err());
        assert!(BranchPlan::from_optional(Some("bad..branch")).is_err());
    }

    #[test]
    fn parses_gitlab_mr_url() {
        let repo =
            GitRepo::parse("https://gitlab.example.com/team/sample-app/-/merge_requests/759")
                .unwrap();
        assert_eq!(repo.url, "https://gitlab.example.com/team/sample-app.git");
        assert_eq!(repo.project, "sample-app");
        assert_eq!(repo.mr_iid, Some("759".to_string()));
    }

    #[test]
    fn parses_gitlab_mr_url_with_trailing_slash() {
        let repo = GitRepo::parse("https://gitlab.com/group/project/-/merge_requests/42/").unwrap();
        assert_eq!(repo.url, "https://gitlab.com/group/project.git");
        assert_eq!(repo.project, "project");
        assert_eq!(repo.mr_iid, Some("42".to_string()));
    }

    #[test]
    fn parses_gitlab_mr_url_with_query_params() {
        let repo =
            GitRepo::parse("https://gitlab.com/group/project/-/merge_requests/1?view=parallel")
                .unwrap();
        assert_eq!(repo.url, "https://gitlab.com/group/project.git");
        assert_eq!(repo.mr_iid, Some("1".to_string()));
    }

    #[test]
    fn non_mr_url_has_no_mr_iid() {
        let repo = GitRepo::parse("https://github.com/user/project.git").unwrap();
        assert_eq!(repo.mr_iid, None);
        assert_eq!(repo.url, "https://github.com/user/project.git");
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
    fn converts_mr_url_to_ssh() {
        let repo =
            GitRepo::parse("https://gitlab.example.com/team/sample-app/-/merge_requests/759")
                .unwrap();
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
}
