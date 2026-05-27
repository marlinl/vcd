use crate::error::{Result, VcdError};

use super::{json_string_field, json_string_field_in_object, project_name_from_path, run_cli};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubRepo {
    pub clone_url: String,
    pub project: String,
    owner: String,
    repo: String,
    pr_number: Option<String>,
}

impl GitHubRepo {
    pub fn parse(url: &str) -> Option<Self> {
        parse_pull_request_url(url)
            .or_else(|| parse_http_url(url))
            .or_else(|| parse_git_at_url(url))
            .or_else(|| parse_ssh_url(url))
    }

    pub fn branch_from_url_or_default(&self) -> Result<String> {
        match &self.pr_number {
            Some(number) => self.pull_request_branch(number),
            None => self.default_branch(),
        }
    }

    fn pull_request_branch(&self, number: &str) -> Result<String> {
        let output = run_cli(
            "gh",
            &[
                "api".to_string(),
                format!("repos/{}/{}/pulls/{number}", self.owner, self.repo),
            ],
            "GitHub PR 分支解析失败",
        )?;
        json_string_field_in_object(&output, "head", "ref").ok_or_else(|| {
            VcdError::new("GitHub PR 分支解析失败", "missing head.ref in gh response")
                .with_hint("请确认传入的是 GitHub pull request URL，或显式传入 PR 源分支名")
        })
    }

    fn default_branch(&self) -> Result<String> {
        let output = run_cli(
            "gh",
            &[
                "api".to_string(),
                format!("repos/{}/{}", self.owner, self.repo),
            ],
            "GitHub 默认分支解析失败",
        )?;
        json_string_field(&output, "default_branch").ok_or_else(|| {
            VcdError::new(
                "GitHub 默认分支解析失败",
                "missing default_branch in gh response",
            )
            .with_hint("请确认 gh 已登录且有权限访问该仓库，或显式传入 branch")
        })
    }
}

fn parse_pull_request_url(url: &str) -> Option<GitHubRepo> {
    let repo = parse_http_url(url)?;
    let path = github_path_from_http_url(url)?;
    let parts: Vec<&str> = path.split('/').collect();
    let pull_index = parts.iter().position(|part| *part == "pull")?;
    let number = parts.get(pull_index + 1)?;
    if number.is_empty() || !number.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    Some(GitHubRepo {
        pr_number: Some((*number).to_string()),
        ..repo
    })
}

fn parse_http_url(url: &str) -> Option<GitHubRepo> {
    let path = github_path_from_http_url(url)?;
    let (owner, repo) = owner_repo_from_path(&path)?;
    build_repo(owner, repo, None)
}

fn github_path_from_http_url(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let (host, path) = rest.split_once('/')?;
    if !host.eq_ignore_ascii_case("github.com") {
        return None;
    }
    Some(
        path.split(&['?', '#'])
            .next()?
            .trim_matches('/')
            .to_string(),
    )
}

fn parse_git_at_url(url: &str) -> Option<GitHubRepo> {
    let rest = url.strip_prefix("git@github.com:")?;
    let (owner, repo) = owner_repo_from_path(rest)?;
    build_repo(owner, repo, None)
}

fn parse_ssh_url(url: &str) -> Option<GitHubRepo> {
    let rest = url.strip_prefix("ssh://git@github.com/")?;
    let (owner, repo) = owner_repo_from_path(rest)?;
    build_repo(owner, repo, None)
}

fn owner_repo_from_path(path: &str) -> Option<(String, String)> {
    let path = path.trim_matches('/').trim_end_matches(".git");
    let mut parts = path.split('/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.trim_end_matches(".git").to_string();
    if owner.is_empty() || repo.is_empty() {
        None
    } else {
        Some((owner, repo))
    }
}

fn build_repo(owner: String, repo: String, pr_number: Option<String>) -> Option<GitHubRepo> {
    let project = project_name_from_path(&repo)?;
    Some(GitHubRepo {
        clone_url: format!("https://github.com/{owner}/{repo}.git"),
        project,
        owner,
        repo,
        pr_number,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_repo_url() {
        let repo = GitHubRepo::parse("https://github.com/user/project.git").unwrap();
        assert_eq!(repo.clone_url, "https://github.com/user/project.git");
        assert_eq!(repo.project, "project");
        assert_eq!(repo.owner, "user");
        assert_eq!(repo.repo, "project");
        assert_eq!(repo.pr_number, None);
    }

    #[test]
    fn parses_github_pr_url() {
        let repo = GitHubRepo::parse("https://github.com/user/project/pull/42").unwrap();
        assert_eq!(repo.clone_url, "https://github.com/user/project.git");
        assert_eq!(repo.pr_number, Some("42".to_string()));
    }

    #[test]
    fn parses_github_ssh_url() {
        let repo = GitHubRepo::parse("git@github.com:user/project.git").unwrap();
        assert_eq!(repo.clone_url, "https://github.com/user/project.git");
        assert_eq!(repo.owner, "user");
        assert_eq!(repo.repo, "project");
    }

    #[test]
    fn ignores_gitlab_url() {
        assert!(GitHubRepo::parse("https://gitlab.com/group/project.git").is_none());
        assert!(GitHubRepo::parse("git@gitlab.com:group/project.git").is_none());
    }
}
