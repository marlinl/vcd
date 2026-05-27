---
id: F-20260527-001
title: GitHub/GitLab URL 分支解析与 repo 工具模块拆分
status: partial
source:
  - internal: "打开 GitLab MR URL 时 vcd 创建 mr/<iid> 临时分支，未使用 MR 实际源分支；同时 repo 相关逻辑集中在单文件中，不利于扩展 GitHub/GitLab 差异"
created: 2026-05-27
owner: vcd
affected_design_prompts:
  - 003-open-project-design
---

# F-20260527-001 GitHub/GitLab URL 分支解析与 repo 工具模块拆分

## 1. Summary

`vcd <codex|claude> <git-url> [branch]` 需要根据传入 URL 类型解析应 checkout 的分支。GitLab MR URL 应使用 MR 的 source branch；GitHub PR URL 应使用 PR 的 head branch；普通 repo URL 没有显式 branch 时应使用项目 default branch。

同时将 `src/repo.rs` 拆分为 `src/repo/` 目录：

- `mod.rs`：统一 URL 解析、平台识别和公共类型
- `gitlab.rs`：通过 `glab` 处理 GitLab CE/EE URL
- `github.rs`：通过 `gh` 处理 GitHub URL

## 2. Background

当前 vcd 已支持 GitLab MR URL，例如：

```bash
vcd claude https://gitlab.example.com/group/project/-/merge_requests/42
```

旧逻辑会拉取：

```text
refs/merge-requests/<iid>/head:mr/<iid>
```

然后 checkout 到本地 `mr/<iid>` 分支。这会偏离用户在 GitLab 上实际工作的源分支名，影响后续提交、push、工具上下文和 AI editor 对分支的理解。

GitHub/GitLab 都已有官方 CLI：

- GitHub：`gh`
- GitLab：`glab`

vcd 镜像中已经安装 `gh` 和 `glab`，本机 open 阶段也可以通过这些 CLI 查询 URL 对应的分支信息。

## 3. Problem

1. **GitLab MR 分支不正确**：打开 MR URL 时创建 `mr/<iid>`，没有 checkout 到 MR source branch。
2. **普通仓库默认分支写死**：未传 branch 时仍默认使用 `master`，不适配 `main` 或自定义 default branch。
3. **repo 逻辑不可扩展**：Git URL 解析、MR 解析、分支策略都集中在 `src/repo.rs`，后续支持 GitHub PR、GitLab CE/EE 差异时不清晰。

## 4. Goal

- 新增 `src/repo/` 模块目录：
  - `mod.rs`
  - `gitlab.rs`
  - `github.rs`
- `mod.rs` 负责识别 URL 是 GitHub 还是 GitLab。
- GitLab 支持：
  - GitLab SaaS
  - GitLab CE/EE 私有域名
  - MR URL：通过 `glab` 获取 `source_branch`
  - 普通 repo URL：通过 `glab` 获取 default branch
- GitHub 支持：
  - PR URL：通过 `gh` 获取 `headRefName`
  - 普通 repo URL：通过 `gh` 获取 default branch
- 如果用户显式传入 `[branch]`，始终优先使用用户传入的 branch。
- 如果 URL 没有关联 PR/MR branch，则使用项目 default branch。

## 5. Non-goal

- 不实现 GitHub Enterprise 的完整 host 配置。
- 不实现 fork MR/PR 的跨 remote checkout。
- 不替代 `gh`/`glab` 的认证流程。
- 不新增真实 GitHub/GitLab 网络集成测试。

## 6. Impact Map

| Design Prompt | Role | Impact Level | Affected 4C Area | What Changes |
|---|---|---|---|---|
| 003-open-project-design | Primary | L2 | Contract | open 命令未传 branch 时不再固定 `master`；GitLab MR/GitHub PR URL 根据平台 CLI 解析实际分支 |
| 003-open-project-design | Primary | L2 | Core Logic | Parse Inputs 后新增 repo platform branch resolution；Prepare Repository 使用解析出的 named branch |
| 003-open-project-design | Primary | L1 | Corners | 新增 `gh`/`glab` 不存在、未认证、私有 GitLab CE/EE 无权限、default branch 获取失败等失败路径 |

## 7. Technical Details

### Module Layout

```text
src/repo/
  mod.rs       # 公共类型、平台识别、URL 解析、BranchPlan
  gitlab.rs    # GitLab CE/EE 和 SaaS URL 解析；glab 查询 branch/default branch
  github.rs    # GitHub URL 解析；gh 查询 PR branch/default branch
```

### URL Platform Resolution

`mod.rs` 根据 URL host 和 URL 形态识别平台：

- `github.com` 或 `https://github.com/...` -> GitHub
- 包含 `/-/merge_requests/<iid>` -> GitLab
- 其他 HTTPS/SSH Git URL 默认按 GitLab-compatible 处理，以支持 GitLab CE/EE 私有域名

### Branch Resolution

显式 branch：

```bash
vcd claude <url> feature-a
```

直接使用 `feature-a`。

GitLab MR URL：

```bash
glab api projects/<encoded-project-path>/merge_requests/<iid>
```

读取：

```text
source_branch
```

GitHub PR URL：

```bash
gh api repos/<owner>/<repo>/pulls/<number>
```

读取：

```text
head.ref
```

普通 GitLab repo URL：

```bash
glab api projects/<encoded-project-path>
```

读取：

```text
default_branch
```

普通 GitHub repo URL：

```bash
gh api repos/<owner>/<repo>
```

读取：

```text
default_branch
```

### Failure Behavior

如果 `gh` 或 `glab` 不存在、未登录、token 无权限、API 响应缺字段，应在创建 Docker container 前失败，并提示：

- 安装或登录对应 CLI。
- 显式传入 branch 作为临时绕过方案。

### Testing

单元测试覆盖：

- GitHub repo URL 解析。
- GitHub PR URL 解析。
- GitLab CE/EE MR URL 解析。
- GitLab 普通 repo URL 解析。
- project path URL encoding。
- JSON 字段解析。
- 显式 branch 优先。

不在单元测试中依赖真实 `gh`/`glab` 网络调用。
