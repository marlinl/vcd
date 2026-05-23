# vcd doctor 设计文档

## 1. Context

`vcd doctor` 是环境检查命令，用于在用户执行 `vcd init`、`vcd rebuild` 或打开项目之前，确认本机环境是否适合运行 vcd。

vcd 的核心使用场景依赖 macOS、OrbStack 和 Docker CLI。`doctor` 不负责修复环境，也不执行会改变系统状态的操作；它只做只读检查，并给出明确的安装或修复提示。

当前 `doctor` 的主要目标：

- 检查 OrbStack 是否已安装。
- 检查 Docker CLI 是否可用。
- 读取 vcd config。

只要 OrbStack app 已安装即可认为 OrbStack 检查通过。`doctor` 不检查 Docker daemon 是否已经启动，也不检查 GitHub 网络连通性。

## 2. Contract

### Command

```bash
vcd doctor
```

### Outputs

成功时输出每个检查项的通过状态：

```text
[ok] OrbStack installed
[ok] Docker CLI available
[ok] vcd config readable
```

失败时必须说明失败阶段，并给出可操作提示。

OrbStack 未安装时：

```text
[fail] OrbStack not installed
Hint: install OrbStack with: brew install --cask orbstack
```

Docker CLI 不存在时：

```text
[fail] Docker CLI not found
Hint: install and start OrbStack, then reopen your terminal
```

config 不存在或无法读取时：

```text
[fail] vcd config unreadable
Hint: run vcd init <user>
```

## 3. Core Logic

### Check OrbStack

1. 检查 OrbStack app 是否存在：

   ```text
   /Applications/OrbStack.app
   ```

2. 如果不存在，失败并提示：

   ```bash
   brew install --cask orbstack
   ```

3. 如果存在，输出通过。

### Check Docker CLI

1. 使用固定参数执行：

   ```bash
   docker --version
   ```

2. 命令不存在或返回失败时，提示安装并启动 OrbStack。

3. 不执行 `docker info`，不要求 Docker daemon 当前可连接。

### Read Config

1. 读取默认配置：

   ```text
   ~/.config/vcd/config
   ```

2. 解析当前字段：

   ```text
   user.name
   user.email
   ssh.key_path
   initialized_at
   container.docker_build
   container.id
   proxy.url
   proxy.no_proxy
   token.gitlab-host
   token.gitlab
   token.github
   ```

3. 读取失败时提示执行：

   ```bash
   vcd init <user>
   ```

## 4. Safety

`doctor` 必须保持只读：

- 不创建、修改或删除文件。
- 不修改 Docker container、image、volume 或 network。
- 不修改 vcd config。
- 不输出 SSH 私钥内容。
- 不把 config 值拼接成 shell 字符串执行。

所有外部命令必须使用 `std::process::Command` 和参数数组构造。

`ssh.key_path` 只能作为配置解析字段存在，不读取文件内容。

## 5. Corners

### OrbStack Installed But Not Running

OrbStack app 存在即通过 OrbStack 检查。`doctor` 不检查 OrbStack 是否已启动。

### Docker CLI From Other Provider

如果 Docker CLI 可用，但不是 OrbStack 提供，`doctor` 仍可以通过。vcd 默认面向 OrbStack，但实现上依赖 Docker CLI。

### Missing Config

未初始化时，OrbStack 和 Docker CLI 检查仍可继续；config 检查失败，并提示执行 `vcd init <user>`。
