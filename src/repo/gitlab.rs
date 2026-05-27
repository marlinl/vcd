use crate::error::{Result, VcdError};

use super::{
    json_string_field, percent_encode_path, project_name_from_path, run_cli, validate_branch,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitLabRepo {
    pub clone_url: String,
    pub project: String,
    host: String,
    project_path: String,
    mr_iid: Option<String>,
}

impl GitLabRepo {
    pub fn parse(url: &str) -> Option<Self> {
        parse_merge_request_url(url)
            .or_else(|| parse_http_url(url))
            .or_else(|| parse_git_at_url(url))
            .or_else(|| parse_ssh_url(url))
    }

    pub fn branch_from_url_or_default(&self) -> Result<String> {
        match &self.mr_iid {
            Some(iid) => self.mr_source_branch(iid),
            None => self.default_branch(),
        }
    }

    fn mr_source_branch(&self, iid: &str) -> Result<String> {
        let endpoint = format!(
            "projects/{}/merge_requests/{iid}",
            percent_encode_path(&self.project_path)
        );
        let output = self.glab_api(&endpoint)?;
        let branch = json_string_field(&output, "source_branch").ok_or_else(|| {
            VcdError::new(
                "GitLab MR 解析失败",
                "missing source_branch in glab response",
            )
            .with_hint("请确认传入的是 GitLab merge request URL，或显式传入 MR 源分支名")
        })?;
        validate_branch(&branch)?;
        Ok(branch)
    }

    fn default_branch(&self) -> Result<String> {
        let endpoint = format!("projects/{}", percent_encode_path(&self.project_path));
        let output = self.glab_api(&endpoint)?;
        let branch = json_string_field(&output, "default_branch").ok_or_else(|| {
            VcdError::new(
                "GitLab 默认分支解析失败",
                "missing default_branch in glab response",
            )
            .with_hint("请确认 glab 已登录且有权限访问该仓库，或显式传入 branch")
        })?;
        validate_branch(&branch)?;
        Ok(branch)
    }

    fn glab_api(&self, endpoint: &str) -> Result<String> {
        run_cli(
            "glab",
            &[
                "api".to_string(),
                "--hostname".to_string(),
                self.host.clone(),
                endpoint.to_string(),
            ],
            "GitLab 分支解析失败",
        )
    }
}

fn parse_merge_request_url(url: &str) -> Option<GitLabRepo> {
    let marker = "/-/merge_requests/";
    let mr_pos = url.find(marker)?;
    let (project_url, iid_with_suffix) = url.split_at(mr_pos);
    let iid = iid_with_suffix
        .strip_prefix(marker)?
        .split(&['/', '?', '#'])
        .next()?;
    if iid.is_empty() || !iid.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }

    let mut repo = parse_http_url(project_url.trim_end_matches('/'))?;
    repo.mr_iid = Some(iid.to_string());
    Some(repo)
}

fn parse_http_url(url: &str) -> Option<GitLabRepo> {
    let (scheme, rest) = url.split_once("://")?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let (host, path) = rest.split_once('/')?;
    if host.eq_ignore_ascii_case("github.com") {
        return None;
    }
    build_repo(
        format!("{scheme}://{host}/{}", normalize_path(path)?),
        host.to_string(),
        normalize_path(path)?,
        None,
    )
}

fn parse_git_at_url(url: &str) -> Option<GitLabRepo> {
    let rest = url.strip_prefix("git@")?;
    let (host, path) = rest.split_once(':')?;
    if host.eq_ignore_ascii_case("github.com") {
        return None;
    }
    let path = normalize_path(path)?;
    build_repo(
        format!("git@{host}:{path}.git"),
        host.to_string(),
        path,
        None,
    )
}

fn parse_ssh_url(url: &str) -> Option<GitLabRepo> {
    let rest = url.strip_prefix("ssh://")?;
    let (user_host, path) = rest.split_once('/')?;
    let host = user_host
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(user_host);
    if host.eq_ignore_ascii_case("github.com") {
        return None;
    }
    let path = normalize_path(path)?;
    build_repo(
        format!("ssh://{user_host}/{path}.git"),
        host.to_string(),
        path,
        None,
    )
}

fn normalize_path(path: &str) -> Option<String> {
    let path = path
        .split(&['?', '#'])
        .next()?
        .trim_matches('/')
        .trim_end_matches(".git");
    if path.is_empty() || path.contains("/-/") {
        None
    } else {
        Some(path.to_string())
    }
}

fn build_repo(
    clone_url: String,
    host: String,
    project_path: String,
    mr_iid: Option<String>,
) -> Option<GitLabRepo> {
    let project = project_name_from_path(&project_path)?;
    let clone_url = if clone_url.ends_with(".git") {
        clone_url
    } else {
        format!("{clone_url}.git")
    };
    Some(GitLabRepo {
        clone_url,
        project,
        host,
        project_path,
        mr_iid,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gitlab_ce_mr_url() {
        let repo =
            GitLabRepo::parse("https://git.leyantech.com/quick-n-dirty/click/-/merge_requests/599")
                .unwrap();
        assert_eq!(
            repo.clone_url,
            "https://git.leyantech.com/quick-n-dirty/click.git"
        );
        assert_eq!(repo.project, "click");
        assert_eq!(repo.host, "git.leyantech.com");
        assert_eq!(repo.project_path, "quick-n-dirty/click");
        assert_eq!(repo.mr_iid, Some("599".to_string()));
    }

    #[test]
    fn parses_gitlab_saas_repo_url() {
        let repo = GitLabRepo::parse("https://gitlab.com/group/project.git").unwrap();
        assert_eq!(repo.clone_url, "https://gitlab.com/group/project.git");
        assert_eq!(repo.host, "gitlab.com");
        assert_eq!(repo.project_path, "group/project");
        assert_eq!(repo.mr_iid, None);
    }

    #[test]
    fn parses_gitlab_ssh_url() {
        let repo = GitLabRepo::parse("git@gitlab.example.com:team/sample-app.git").unwrap();
        assert_eq!(repo.clone_url, "git@gitlab.example.com:team/sample-app.git");
        assert_eq!(repo.project_path, "team/sample-app");
    }

    #[test]
    fn ignores_github_url() {
        assert!(GitLabRepo::parse("https://github.com/user/project.git").is_none());
        assert!(GitLabRepo::parse("git@github.com:user/project.git").is_none());
    }
}
