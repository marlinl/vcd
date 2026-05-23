# vcd codex / claude 打开项目设计文档

## 1. Context

`vcd <codex|claude> <git-url> [branch]` 是 vcd 的核心项目打开命令。

它依赖以下命令提前准备好的本地 base image：

```bash
vcd init <user>
vcd rebuild
vcd config set <key> <value>
```

命令会启动一个带时间戳名称的一次性 Docker container，在容器内准备 Git 仓库，切换到目标分支，然后在项目根目录启动选定的 AI coding tool。编辑器退出或打开流程失败后，vcd 会尝试删除本次 container。

除普通 Git 仓库 URL 外，命令也支持 GitLab merge request URL。传入 MR URL 且未显式传入 `[branch]` 时，vcd 会自动拉取 MR ref 并 checkout 到本地 `mr/<iid>` 分支。

当前支持的 editor：

```text
codex
claude
```

容器内项目目录固定为：

```text
/home/<user>/<project>
```

该命令面向交互式终端使用。启动 editor 时必须使用 `docker exec -it` 并继承 stdin/stdout/stderr，否则 `codex` 或 `claude` 可能无法正常交互。

## 2. Contract

### Command

```bash
vcd <codex|claude> <git-url> [branch]
```

示例：

```bash
vcd codex https://github.com/user/project.git
vcd claude https://github.com/user/project.git feature-a
vcd codex https://gitlab.com/group/project/-/merge_requests/42
```

### Inputs

`editor`：

- 只能是 `codex` 或 `claude`。

`git-url`：

- 支持常见 HTTPS Git URL。
- 支持常见 SSH Git URL。
- 支持 GitLab merge request URL，例如 `https://gitlab.com/group/project/-/merge_requests/42`。
- project name 从 URL 最后一段推导。
- `.git` 后缀会被移除。
- GitLab MR URL 会被解析为仓库 clone URL 和 MR IID，例如：
  - clone URL: `https://gitlab.com/group/project.git`
  - project name: `project`
  - MR IID: `42`
- GitLab MR URL 支持末尾 `/`、query string 和 fragment。

`branch`：

- 可选。
- 如果传入，vcd 切到该分支。
- 如果传入 GitLab MR URL 且未传 `branch`，vcd 自动 checkout 对应 MR 分支。
- 如果既传入 GitLab MR URL 又传入 `branch`，显式 `branch` 优先，MR URL 只用于推导 clone URL 和 project name。
- 如果省略且 URL 不是 GitLab MR URL，vcd 切到 `master` 并更新，然后创建或复用本地 `temp` 分支。

### Prerequisites

用户必须已经执行过：

```bash
vcd init <user>
```

或者本地已经存在有效配置：

```text
~/.config/vcd/config
```

配置必须包含：

```text
user.name=<user>
user.email=<email>
ssh.key_path=<ssh-key-path>
initialized_at=<timestamp>
container.docker_build=~/.config/vcd/Dockerfile
container.id=<local-vcd-image-name>
```

可选配置：

```text
token.gitlab-host=<gitlab-host-or-empty>
token.gitlab=<gitlab-api-token-or-empty>
token.github=<github-api-token-or-empty>
```

Docker 必须已安装并可访问。

`container.id` 指向的本地 vcd image 必须存在于本地 Docker images。

如果配置已存在但需要修改用户、邮箱、SSH key path 或 Dockerfile path，应使用：

```bash
vcd config set <key> <value>
vcd rebuild
```

镜像内必须包含：

- git
- zsh
- 选定的 editor command
- editor 所需的认证或配置目录

### Outputs

成功后，当前终端会附着到容器内运行的 editor 进程。

editor 的工作目录是：

```text
/home/<user>/<project>
```

如果配置中存在 token，项目容器会在创建时动态注入：

```text
GITLAB_HOST=<token.gitlab-host>
GITLAB_TOKEN=<token.gitlab>
GH_TOKEN=<token.github>
```

选择 `codex` 时执行：

```bash
docker exec -it -w /home/<user>/<project> <container> codex .
```

选择 `claude` 时执行：

```bash
docker exec -it -w /home/<user>/<project> <container> claude .
```

## 3. Core Logic

### Parse Inputs

1. 解析命令参数：

   ```text
   editor = codex | claude
   repo_url = <git-url>
   branch = optional
   ```

2. 解析 editor：
   - `codex -> codex`
   - `claude -> claude`
3. 解析 Git repo URL，识别普通仓库 URL 或 GitLab MR URL。
4. 从 URL 推导 project name：
   - `https://github.com/user/project.git -> project`
   - `git@github.com:user/project.git -> project`
   - `https://gitlab.com/group/project/-/merge_requests/42 -> project`
5. 如果 URL 是 GitLab MR URL，提取 MR IID：
   - `https://gitlab.com/group/project/-/merge_requests/42 -> 42`
6. 把 project name 规范化成适合容器名的格式。
7. 解析 branch plan：
   - 传入 branch -> named branch plan
   - 未传 branch 且 URL 是 GitLab MR URL -> merge-request branch plan
   - 未传 branch 且 URL 是普通仓库 URL -> temp-from-master plan

### Read Config

1. 读取：

   ```text
   ~/.config/vcd/config
   ```

2. 提取：
   - user.name
   - user.email
   - ssh.key_path
   - initialized_at
   - container.docker_build
   - container.id
3. 如果配置缺失或不完整，给出明确的 config-read 错误。

### Prepare Docker

1. 检查 Docker daemon：

   ```bash
   docker version --format {{.Server.Version}}
   ```

2. 确认本地 vcd image 存在：

   ```bash
   docker image inspect <container.id>
   ```

3. 生成带时间戳的 container name：

   ```text
   vcd-{user}-{editor}-{project}-{timestamp}
   ```

4. 检查 container 状态：

   ```bash
   docker container inspect -f {{.State.Status}} <container>
   ```

5. 如果 container 不存在，创建它：

   ```bash
   docker run -d \
     --name <container> \
     -v /Users/<user>/.codex:/Users/<user>/.codex \
     -v /Users/<user>/.claude:/Users/<user>/.claude \
     -v <ssh.key_path>:/home/<user>/.ssh/<ssh-key-file>:ro \
     <container.id> \
     sleep infinity
   ```

   如果宿主机存在以下 SSH 辅助文件，也会挂载到容器内对应路径：

   ```text
   <ssh.key_path>.pub
   ~/.ssh/known_hosts
   ~/.ssh/config
   ```

6. 如果极少数情况下同名 container 已存在但已停止，执行 start。
7. 如果极少数情况下同名 container 存在但 paused，执行 unpause。
8. 如果极少数情况下同名 container 已 running，确认 SSH key mount 存在。

### Prepare Repository

项目直接放在容器用户 home 目录下，避免普通用户写入根目录路径。

1. 如果 `/home/<user>/<project>/.git` 存在：

   ```bash
   docker exec <container> git -C /home/<user>/<project> fetch origin --prune
   ```

2. 否则执行 clone。

   HTTPS URL 会转换为 SSH clone URL，以复用容器内挂载的 SSH key；SSH URL 保持不变。

   ```bash
   docker exec <container> git clone <ssh-clone-url> /home/<user>/<project>
   ```

### Branch Behavior

传入 branch 时：

1. 如果本地 branch 存在：

   ```bash
   git -C /home/<user>/<project> checkout <branch>
   ```

2. 否则从远端创建或重置本地 branch：

   ```bash
   git -C /home/<user>/<project> checkout -B <branch> origin/<branch>
   ```

3. 更新分支：

   ```bash
   git -C /home/<user>/<project> pull --ff-only origin <branch>
   ```

传入 GitLab MR URL 且未传 branch 时：

1. 拉取对应 MR ref 到本地 `mr/<iid>` 分支：

   ```bash
   git -C /home/<user>/<project> fetch origin refs/merge-requests/<iid>/head:mr/<iid>
   ```

2. 切到本地 MR 分支：

   ```bash
   git -C /home/<user>/<project> checkout mr/<iid>
   ```

未传 branch 时：

1. 拉取 master：

   ```bash
   git -C /home/<user>/<project> fetch origin master
   ```

2. 切到 master：

   ```bash
   git -C /home/<user>/<project> checkout master
   ```

3. 更新 master：

   ```bash
   git -C /home/<user>/<project> pull --ff-only origin master
   ```

4. 如果本地 `temp` 存在，切到它：

   ```bash
   git -C /home/<user>/<project> checkout temp
   ```

5. 否则从当前 master 创建 `temp`：

   ```bash
   git -C /home/<user>/<project> checkout -b temp
   ```

### Launch Editor

交互式启动选定 editor：

```bash
docker exec -it -w /home/<user>/<project> <container> <editor> .
```

该进程必须继承终端 stdin/stdout/stderr。

### Cleanup Container

`prepare repo`、`launch editor` 或 editor 本身退出后，执行：

```bash
docker rm -f <container>
```

container 是一次性的；退出后容器内未同步到远端或宿主机的修改会被删除。

## 4. Corners

### Unsupported Editor

除 `codex` 和 `claude` 外的 editor 都应在 editor resolution 阶段失败。

错误信息需要说明当前支持：

```bash
vcd codex <git-url> [branch]
vcd claude <git-url> [branch]
```

### Missing Config

如果配置不存在，应在接触 Docker 前失败。

提示用户先执行：

```bash
vcd init <user>
```

### Missing Base Image

如果 `container.id` 指向的本地 vcd image 在本地不存在，应在创建容器前失败。

提示用户执行：

```bash
vcd rebuild
```

如果本地尚未初始化，则先执行 `vcd init <user>`。

### Existing Container Uses Old Image

container name 带本次启动时间戳，正常情况下不会复用旧容器，并且打开流程结束后会自动删除本次容器。

这样可以避免失败容器阻塞后续启动。

### Dirty Repository

当前更新分支使用 `pull --ff-only`。

如果仓库有本地修改，或无法 fast-forward，Git 应失败而不是覆盖用户工作。

错误应归类为 Git branch update failure。

### Missing Remote Branch

如果传入的 branch 在 origin 上不存在，从 `origin/<branch>` checkout 会失败。

应把错误归类为 branch switch failure。

### Merge Request URL

当前自动 checkout 只支持 GitLab MR URL 形态：

```text
https://<host>/<group>/<project>/-/merge_requests/<iid>
```

其中 `<iid>` 必须是数字。GitHub pull request URL 等其他平台 URL 不会进入 MR 自动 checkout 流程。

如果 MR ref 不存在、无权限访问或远端不支持 `refs/merge-requests/<iid>/head`，`git fetch` 会失败。

应把错误归类为 MR branch fetch failure。

### Default Branch Is Not master

未传 branch 时，当前逻辑假设远端存在 `master`。

如果仓库只有 `main`，默认分支流程会失败。

未来可以通过以下方式发现默认分支：

```bash
git remote show origin
```

或：

```bash
git symbolic-ref refs/remotes/origin/HEAD
```

### Half-Cloned Repository

如果 clone 被中断，留下 `/home/<user>/<project>` 但没有 `.git`，当前逻辑会再次 clone，可能因为目录已存在而失败。

未来应该识别这种半成品目录，并给出明确恢复提示。

### Authentication

私有 Git 仓库和 editor 启动可能需要认证。

当前 container 挂载：

```text
/Users/<user>/.codex
/Users/<user>/.claude
```

Git SSH 凭据尚未显式挂载。

如果需要支持 SSH 私有仓库，后续应挂载 `.ssh` 或使用更安全的 credential forwarding 策略。

### Interactive TTY

editor 启动必须使用 `docker exec -it`。

如果当前终端不是交互式终端，editor 可能启动失败或表现异常。

### Container Name Collisions

container name 带时间戳生成：

```text
vcd-{user}-{editor}-{project}-{timestamp}
```

这避免失败容器阻塞下一次启动，但同一秒内重复启动同一项目仍可能发生冲突。

未来可以把 host 或 org 信息加入 container name。
