# VCD

VCD 是一个让 vibe coding 在容器里编程开发的命令行工具。

它主要解决本地环境污染和版本冲突问题：项目代码、依赖安装、AI 编程工具运行都放在 Docker 容器中，本机只负责发起命令。这样可以让不同项目使用相对隔离的开发环境，避免 vibe coding 过程受到本机语言版本、系统依赖、工具链版本的影响。

VCD 当前主要面向 macOS + OrbStack + Docker CLI 使用。

## 安装

### Homebrew 安装：

```bash
brew tap marlinl/tap
brew install vcd
```

### 源码安装：

```bash
git clone https://github.com/marlinl/vcd.git
cd vcd
cargo install --path .
```

运行前需要：

- macOS。
- OrbStack 已启动。
- Docker CLI 可用。
- 宿主机有可用 SSH 私钥。

## 使用

查看帮助：

```bash
vcd --help
```

检查运行环境：

```bash
vcd doctor
```

首次初始化：

```bash
vcd init <user>
```

打开仓库：

```bash
vcd <codex|claude> <git-url> [branch]
```

修改配置：

```bash
vcd config set <key> <value>
```

查看配置：

```bash
vcd config list
```

重建容器基础镜像：

```bash
vcd rebuild [user]
```

常用配置项：

```text
user.name
user.email
ssh.key_path
container.docker_build
container.id
proxy.url
proxy.no_proxy
token.gitlab-host
token.gitlab
token.github
```

`token.gitlab-host` 会在打开项目容器时作为 `GITLAB_HOST` 注入容器，供 `glab` 使用。`token.gitlab` 会作为 `GITLAB_TOKEN` 注入容器。`token.github` 会作为 `GH_TOKEN` 注入容器，供 `gh` 使用。

## 许可

Apache License 2.0。详见 [LICENSE](LICENSE)。
