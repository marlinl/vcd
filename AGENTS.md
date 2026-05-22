# AGENTS.md


## Context

`vcd` 是一个 Rust CLI 项目，用于支持 vibe coding 工作流。

主要使用环境：

- macOS
- OrbStack
- Docker CLI
- AI 编程编辑器，例如 `codex`

核心场景是用户在本机终端执行：

```bash
vcd codex https://github.com/user/project
```

然后 `vcd` 自动完成以下准备工作：

- 识别目标 Git 仓库。
- 启动或复用本地 Docker 容器。
- 在容器内下载或更新仓库。
- 进入容器内对应项目目录。
- 启动指定 AI 编程编辑器。
- 保持当前终端为交互式会话。

这个项目的重点不是构建完整 IDE，而是把「启动容器、准备仓库、进入项目、启动 AI 编辑器」这条命令行路径做顺。

## Contract

### CLI 输入

主命令格式：

```bash
vcd <editor> <repo-url>
```

当前核心示例：

```bash
vcd codex https://github.com/user/project
```

建议支持：

```bash
vcd --help
vcd --version
vcd config set user.name abc
```

`vcd init <user>` 只用于首次初始化。如果 `~/.config/vcd/config` 中已经存在非空 `initialized_at`，后续不允许再次 `init`；应通过 `vcd config set <key> <value>` 修改已有配置。

### 运行前置条件

- 用户本机已安装 Docker CLI。
- 用户本机已启动 OrbStack 或其他 Docker daemon。
- 基础本地镜像可用，或实现中能给出清晰的镜像缺失提示。
- 容器内具备基本命令行能力，例如 shell、git，以及目标 AI 编辑器所需命令。

### 成功结果

命令成功后，用户应处在一个可交互终端中：

- 终端已经进入 Docker 容器。
- 当前目录是目标仓库目录。
- 指定 AI 编辑器已经在该目录启动。

### 失败结果

失败时必须说明失败发生在哪个阶段，并给出可操作提示。

常见阶段：

- 参数解析失败。
- Git 仓库地址非法。
- Docker CLI 不存在。
- Docker daemon 不可连接，可能是 OrbStack 未启动。
- 基础镜像不存在或无法启动。
- 容器创建、启动或进入失败。
- 仓库 clone 或更新失败。
- 编辑器命令不存在或启动失败。

### 实现约束

- 使用稳定版 Rust。
- CLI 参数解析优先遵循现有代码；新实现可使用 `clap`。
- 错误处理可使用 `thiserror` 或 `anyhow`，但错误信息必须面向用户可读。
- 调用外部命令时使用 `std::process::Command` 或项目已有封装。
- Docker 命令使用参数数组构造，不要拼接成 shell 字符串执行。
- 路径处理使用 `Path`/`PathBuf`。
- 交互式命令必须正确继承 stdin/stdout/stderr。

推荐模块结构如下；如果现有代码已经不同，优先遵循现有结构。

```text
src/
  main.rs          # CLI 入口、参数解析和命令分发
  core/
    init.rs        # init 命令流程
    rebuild.rs     # rebuild 命令流程
    config.rs      # 配置读写、config set 流程和配置字段校验
    open.rs        # open 命令流程，包含编辑器解析
    prompt.rs      # init/rebuild 共享交互输入
  repo.rs          # Git URL 解析和项目名推导
  docker.rs        # Docker/OrbStack 命令封装
  error.rs         # 错误类型
```

## Core Logic

### 主流程

`vcd codex https://github.com/user/project` 的核心逻辑：

1. 解析 CLI 参数，得到 `editor = codex` 和 `repo_url = https://github.com/user/project`。
2. 校验并解析仓库地址，推导项目名，例如 `project`。
3. 检查 Docker CLI 是否可执行。
4. 检查 Docker daemon 是否可连接；失败时提示检查 OrbStack 状态。
5. 根据编辑器名和项目名生成稳定容器名，例如 `vcd-codex-project`。
6. 判断容器是否已存在。
7. 如果容器不存在，使用基础镜像创建并启动容器。
8. 如果容器已存在但未运行，启动容器。
9. 在容器内准备项目目录，例如 `/workspace/project`。
10. 如果仓库未 clone，则执行 clone。
11. 如果仓库已存在，则执行安全的更新策略。
12. 进入容器并切换到项目目录。
13. 启动编辑器命令，例如 `codex`。
14. 将控制权交给交互式终端。

### 仓库处理

- 支持常见 HTTPS Git URL。
- 项目名从 URL 最后一段推导。
- 需要处理 `.git` 后缀，例如 `project.git` 应推导为 `project`。
- 已存在仓库时不要直接删除用户改动。
- 更新策略应谨慎，优先避免覆盖容器内未提交修改。

### 容器处理

- 默认面向 OrbStack，但实现上尽量只依赖 Docker CLI。
- 容器名称应稳定，便于复用。
- 项目路径建议固定在 `/workspace/<project>`。
- 容器内执行命令时要确保工作目录正确。
- 用户应能通过当前终端继续与编辑器交互。

### 编辑器处理

- `<editor>` 是扩展点，不应只为 `codex` 写死全部逻辑。
- `codex` 是当前优先支持的编辑器。
- 启动编辑器前应确认容器内项目已准备好。
- 编辑器启动失败时，应说明是容器内缺少命令还是命令返回失败。

## Corners

### 输入边界

- 缺少 `<editor>`。
- 缺少 `<repo-url>`。
- URL 不是 Git 仓库地址。
- URL 带 `.git` 后缀。
- URL 末尾带 `/`。
- 仓库名包含不适合用于容器名的字符。

### Docker 边界

- 本机没有安装 Docker CLI。
- OrbStack 未启动。
- Docker daemon 无权限访问。
- 基础镜像不存在。
- 容器同名但来源不是当前项目。
- 容器存在但处于 exited、paused 或异常状态。
- 交互式终端没有正确绑定。

### Git 边界

- 仓库不存在或无权限访问。
- 网络失败。
- clone 中断后留下半成品目录。
- 已存在目录但不是 Git 仓库。
- 已存在仓库有未提交修改。
- 默认分支变更或远端不可达。

### 编辑器边界

- 容器内不存在 `codex` 命令。
- 编辑器需要认证或首次初始化。
- 编辑器启动后退出码非零。
- 编辑器需要 TTY，但当前命令未正确分配 TTY。

### 测试与验证边界

代码变更后优先运行：

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
```

涉及 Docker 行为时，至少人工验证：

```bash
vcd codex https://github.com/user/project
```

验收点：

- 容器能启动或复用。
- 仓库能在容器内正确准备。
- 当前终端保持交互式。
- 进入后的当前目录是目标项目目录。
- `codex` 在目标项目目录中启动。
- 失败路径能返回明确、可操作的错误信息。

单元测试不应强依赖真实 Docker daemon。真实 Docker 验证应放在集成测试或人工验证中。
