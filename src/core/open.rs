use crate::docker;
use crate::error::{Result, VcdError};
use crate::repo;

use super::{config, plugin, profile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Editor {
    name: &'static str,
    command: &'static str,
}

impl Editor {
    fn name(self) -> &'static str {
        self.name
    }

    fn command(self) -> &'static str {
        self.command
    }
}

pub fn run(
    editor_name: &str,
    repo_url: &str,
    branch: Option<&str>,
    profile_name: Option<&str>,
) -> Result<()> {
    let editor = resolve_editor(editor_name)?;
    let repo = repo::GitRepo::parse(repo_url)?;
    let profile = match profile_name {
        Some(name) => Some(profile::load(name)?),
        None => None,
    };
    let plugin_root = profile
        .as_ref()
        .filter(|profile| !profile.plugins.is_empty())
        .map(|_| plugin::plugin_root().map(|path| path.display().to_string()))
        .transpose()?;
    let config_path = config::default_config_path()?;
    let config = config::read_config(&config_path)?;
    let branch = resolve_branch(branch, &repo, &config)?;
    let timestamp = docker::timestamp()?;
    let container =
        docker::container_name(&config.user_name, editor.name(), &repo.project, &timestamp);
    docker::ensure_docker_ready()?;
    docker::ensure_image_exists(&config.container_id)?;
    docker::ensure_container(&docker::ContainerRequest {
        name: container.clone(),
        image: config.container_id.clone(),
        user: config.user_name.clone(),
        ssh_key_path: config.ssh_key_path.clone(),
        plugin_root,
        proxy_url: config.proxy_url.clone(),
        no_proxy: config.no_proxy.clone(),
        token_gitlab_host: config.token_gitlab_host.clone(),
        token_gitlab: config.token_gitlab.clone(),
        token_github: config.token_github.clone(),
    })?;

    let open_result = (|| {
        docker::prepare_repo(&container, &config.user_name, &repo, &branch)?;
        let profile_plugins = profile
            .as_ref()
            .map(|profile| profile.plugins.as_slice())
            .unwrap_or(&[]);
        if let Some(profile) = &profile {
            docker::install_profile_plugins(
                &container,
                &config.user_name,
                editor.name(),
                &profile.plugins,
            )?;
            profile::print_profile(profile);
        }
        docker::open_editor(&docker::EditorRequest {
            container: container.clone(),
            user: config.user_name.clone(),
            editor: editor.command().to_string(),
            project: repo.project.clone(),
            plugins: profile_plugins.to_vec(),
            token_gitlab_host: config.token_gitlab_host.clone(),
            token_gitlab: config.token_gitlab.clone(),
            token_github: config.token_github.clone(),
        })
    })();
    let cleanup_result = docker::remove_container(&container);

    match (open_result, cleanup_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(err), Ok(())) => Err(err),
        (Ok(()), Err(err)) => Err(err),
        (Err(err), Err(cleanup_err)) => {
            eprintln!("{cleanup_err}");
            Err(err)
        }
    }
}

fn resolve_editor(name: &str) -> Result<Editor> {
    match name {
        "codex" => Ok(Editor {
            name: "codex",
            command: "codex",
        }),
        "claude" => Ok(Editor {
            name: "claude",
            command: "claude",
        }),
        _ => Err(
            VcdError::new("编辑器解析失败", format!("unsupported editor '{name}'"))
                .with_hint("当前只支持: vcd <codex|claude> <git-url> [branch]"),
        ),
    }
}

fn resolve_branch(
    branch: Option<&str>,
    repo: &repo::GitRepo,
    config: &config::VcdConfig,
) -> Result<repo::BranchPlan> {
    if let Some(name) = branch.map(str::trim).filter(|b| !b.is_empty()) {
        return repo::BranchPlan::from_optional(Some(name));
    }
    if repo.mr_iid.is_some() {
        let source_branch = repo.gitlab_mr_source_branch(&config.token_gitlab)?;
        println!("GitLab MR source branch: {}", source_branch);
        repo::BranchPlan::from_optional(Some(&source_branch))
    } else {
        repo::BranchPlan::from_optional(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_supported_editors() {
        assert_eq!(resolve_editor("codex").unwrap().command(), "codex");
        assert_eq!(resolve_editor("claude").unwrap().command(), "claude");
    }

    #[test]
    fn rejects_unsupported_editor() {
        assert!(resolve_editor("vim").is_err());
    }
}
