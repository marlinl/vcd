# VCD

VCD（Vibe Coding Dev）是一个面向 vibe coding 工作流的 Rust CLI 工具。

它解决的问题很具体：在本机终端里用一条命令，把 Git 仓库放进本地 Docker 容器，进入项目目录，并启动 `codex` 或 `claude` 这类 AI 编程工具。

典型用法：

```bash
vcd codex https://github.com/user/project.git
```

执行后，VCD 会完成：

- 读取本地配置。
- 检查 Docker CLI 和 Docker daemon。
- 使用已构建好的本地 vcd 镜像启动临时容器。
- 在容器内 clone 或更新目标仓库。
- 切换到目标项目目录。
- 在该目录启动指定编辑器命令，例如 `codex .`。
- 编辑器退出后清理本次临时容器。

VCD 不是 IDE，也不试图管理完整开发环境。它只把“启动容器、准备仓库、进入项目、启动 AI 编程工具”这条命令行路径做顺。

## 功能特性

- 支持 `codex` 和 `claude` 两种编辑器入口。
- 支持 HTTPS 和常见 SSH Git 仓库 URL。
- 自动推导项目目录名，并处理 `.git` 后缀。
- 自动构建本地 vcd Docker 镜像。
- 支持自定义 Dockerfile 路径。
- 支持配置 SSH 私钥路径。
- 每次打开项目使用带时间戳的一次性容器，退出后自动删除。
- 配置文件使用简单的 `key=value` 文本格式，默认位于 `~/.config/vcd/config`。

## 前置条件

当前项目目前只支持 Apple Silicon Mac（macOS ARM64 / `aarch64-apple-darwin`）：

- 已启动 OrbStack、Docker Desktop 或其他可用 Docker daemon。
- 当前终端可以访问 `docker` 命令。
- 宿主机已有可用 SSH 私钥。
- 容器镜像内需要安装目标编辑器命令，默认 Dockerfile 会安装 `codex` 和 `claude`。

## 安装

从源码构建：

```bash
cargo build --release
```

运行本地二进制：

```bash
./target/release/vcd --help
```

安装到 Cargo bin 目录：

```bash
cargo install --path .
vcd --help
```

### 通过 Homebrew 安装

Homebrew 不支持直接执行 `brew install github.com/vcd/vcd` 这种裸 GitHub 地址。当前仓库内提供了 Formula，可以把当前仓库作为 tap 安装：

```bash
brew tap vcd/vcd https://github.com/vcd/vcd
brew install vcd
```

Formula 下载 GitHub Release 中已经编译好的 `vcd-aarch64-apple-darwin.tar.gz`，并把其中的 `vcd` 复制到 Homebrew 的 `bin` 目录；安装阶段不需要 Rust，也不声明 Docker 依赖。确认 `vcd` 可用：

```bash
vcd --version
which vcd
```

如果 `which vcd` 没有指向 Homebrew 目录，请先确认 Homebrew shell 环境已加载：

```bash
eval "$(/opt/homebrew/bin/brew shellenv)"
```

Intel Mac 上 Homebrew 路径通常是 `/usr/local/bin/brew`。

发布新版本时，需要先在 GitHub 打 tag，例如：

```bash
git tag v0.1.0
git push origin v0.1.0
```

推送 tag 后，GitHub Actions 会自动创建 GitHub Release，并上传 release asset：

```text
vcd-aarch64-apple-darwin.tar.gz
vcd-aarch64-apple-darwin.tar.gz.sha256
```

该压缩包内需要直接包含可执行文件：

```text
vcd
```

复制 `.sha256` 文件里的 hash，更新 [Formula/vcd.rb](Formula/vcd.rb) 里的 `url` 和 `sha256`。

## 快速开始

### 1. 初始化

首次使用需要初始化本地配置并构建基础镜像：

```bash
vcd init <user>
```

`<user>` 会作为容器内 Linux 用户名，也会作为 Git `user.name`。它必须满足：

- 长度 1-32。
- 只能包含小写字母、数字、下划线或连字符。
- 第一个字符必须是小写字母或下划线。

初始化会交互式询问：

- `User email`：写入 Git `user.email`。
- `SSH key path`：宿主机 SSH 私钥绝对路径，默认是 `/Users/<user>/.ssh/id_rsa`。

初始化成功后会写入：

```text
~/.config/vcd/config
~/.config/vcd/Dockerfile
~/.config/vcd/optional-packages.txt
```

并构建一个本地 vcd 镜像，例如：

```text
vcd-jack:20260522120000
```

`vcd init <user>` 只允许首次初始化。如果配置文件里已经存在非空 `initialized_at`，后续修改配置应使用 `vcd config set` 和 `vcd rebuild`。

### 2. 打开项目

使用 Codex：

```bash
vcd codex https://github.com/user/project.git
```

使用 Claude：

```bash
vcd claude https://github.com/user/project.git
```

指定分支：

```bash
vcd codex https://github.com/user/project.git feature-a
```

如果不指定分支，VCD 会在容器内更新 `master`，然后创建或复用本地 `temp` 分支。

项目会放在容器内：

```text
/home/<user>/<project>
```

注意：项目容器是临时容器。编辑器退出或打开流程失败后，VCD 会执行清理。容器内未提交、未推送或未同步的改动会丢失。

### 3. 修改配置

支持的配置修改命令：

```bash
vcd config set user.name jack
vcd config set user.email jack@example.com
vcd config set ssh.key_path /Users/jack/.ssh/id_ed25519
vcd config set container.docker_build /Users/jack/.config/vcd/Dockerfile
vcd config set container.id vcd-jack:20260522120000
vcd config set proxy.url http://host.docker.internal:1087
vcd config set proxy.no_proxy localhost,127.0.0.1,::1,host.docker.internal,.local
```

修改会影响镜像或容器行为时，执行：

```bash
vcd rebuild
```

切换用户并重新构建：

```bash
vcd rebuild <user>
```

## 配置文件

默认配置路径：

```text
~/.config/vcd/config
```

当前配置结构：

```text
user.name=<user-name>
user.email=<email>
ssh.key_path=<absolute-ssh-private-key-path>
initialized_at=<timestamp>
container.docker_build=<absolute-dockerfile-path>
container.id=<current-vcd-image-name>
proxy.url=<proxy-url-or-empty>
proxy.no_proxy=<no-proxy-domains>
```

示例：

```text
user.name=jack
user.email=jack@example.com
ssh.key_path=/Users/jack/.ssh/id_ed25519
initialized_at=20260522120000
container.docker_build=/Users/jack/.config/vcd/Dockerfile
container.id=vcd-jack:20260522120000
proxy.url=http://host.docker.internal:1087
proxy.no_proxy=localhost,127.0.0.1,::1,host.docker.internal,.local
```

字段说明：

- `user.name`：当前 vcd 用户名、容器内 Linux 用户名、Git `user.name`。
- `user.email`：Git `user.email`。
- `ssh.key_path`：宿主机 SSH 私钥绝对路径。
- `initialized_at`：首次初始化时间戳，用于防止重复 `init`。
- `container.docker_build`：用于构建 vcd 镜像的 Dockerfile 路径。
- `container.id`：完整本地 Docker image name/tag。`open` 阶段直接使用该值，不再拼接其他字段。
- `proxy.url`：容器运行时代理地址，空字符串表示禁用。修改后无需 rebuild，下次启动容器立即生效。
- `proxy.no_proxy`：不走代理的域名列表。

## 环境变量

默认 Debian base image：

```text
debian:trixie-slim
```

如果 Docker Hub 不可访问，可以指定本地或镜像源中的 Debian 镜像：

```bash
VCD_BASE_IMAGE=<local-or-mirrored-debian-image> vcd init <user>
```

默认构建代理：

```text
VCD_PROXY_URL=http://host.docker.internal:1087
VCD_NO_PROXY=localhost,127.0.0.1,::1,host.docker.internal,.local
```

通过环境变量覆盖代理设置（仅在 `vcd init` 或 `vcd rebuild` 时生效）：

```bash
VCD_PROXY_URL=http://host.docker.internal:7897 vcd rebuild
```

关闭代理环境变量：

```bash
VCD_PROXY_URL= vcd rebuild
```

也可以通过配置命令动态修改代理，立即生效，无需重建镜像：

```bash
vcd config set proxy.url http://host.docker.internal:7897
vcd config set proxy.no_proxy localhost,127.0.0.1,::1,host.docker.internal,.local,git.example.com
```

禁用代理：

```bash
vcd config set proxy.url ""
```

## 本地开发

常用校验命令：

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all
cargo build --release
```

GitHub Actions 会在 `master` 分支每次 push 和 pull request 时执行同样的 CI 流程，并把 release 二进制复制到 CI 工作区的 `bin/vcd`，打包成 `vcd-aarch64-apple-darwin.tar.gz` 后上传为 artifact。推送 `v*` tag 时，还会把该 tarball 和 sha256 文件发布到 GitHub Release。

项目结构：

```text
src/
  main.rs          CLI 入口、参数解析和命令分发
  core/
    init.rs        首次初始化和基础镜像构建流程
    rebuild.rs     基础镜像重建流程
    config.rs      配置模型、读写、解析、config set 流程
    open.rs        项目容器和编辑器启动流程
    prompt.rs      init/rebuild 共享交互输入
  docker.rs        Docker CLI 封装、镜像构建、容器操作
  repo.rs          Git URL、项目名和分支解析
  error.rs         面向用户的错误类型
```

`docker/` 目录中的 `Dockerfile` 和 `optional-packages.txt` 会通过 `include_str!` 嵌入 Rust 二进制。发布后的二进制不依赖运行目录旁边存在 `docker/` 目录。

## 许可证

本项目使用 Apache License 2.0 开源协议，详见 [LICENSE](LICENSE)。
