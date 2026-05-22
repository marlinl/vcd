# vcd init 设计文档

## 1. Context

`vcd init <user>` 是 vcd 的首次初始化命令。

它负责生成本地配置、释放 Docker 构建文件，并构建第一版本地基础镜像。`init` 只允许执行一次：如果 `~/.config/vcd/config` 中已经存在非空 `initialized_at`，后续再次执行 `vcd init <user>` 必须失败。

后续修改配置应使用：

```bash
vcd config set <key> <value>
vcd rebuild
```

`docker/Dockerfile` 和 `docker/optional-packages.txt` 在仓库中维护，并通过 `include_str!` 嵌入 Rust 二进制。运行时由 vcd 写到配置目录。

默认运行时文件：

```text
~/.config/vcd/config
~/.config/vcd/Dockerfile
~/.config/vcd/optional-packages.txt
```

## 2. Contract

### Command

```bash
vcd init <user>
```

### Inputs

`user` 是当前 vcd 用户名，同时会传给 Dockerfile 创建容器内用户。

校验规则：

- 长度 1-32。
- 只能包含小写 ASCII 字母、数字、`_`、`-`。
- 第一个字符必须是小写 ASCII 字母或 `_`。

`init` 会交互式询问：

- User email，必填且必须包含 `@`。
- SSH 私钥绝对路径，默认值为 `/Users/<user>/.ssh/id_rsa`。

SSH key 必须存在于：

```text
<ssh-key-path>
```

### Environment

默认 base image：

```text
debian:trixie-slim
```

可通过环境变量覆盖：

```bash
VCD_BASE_IMAGE=<local-or-mirrored-debian-image> vcd init <user>
```

默认代理构建参数：

```text
VCD_PROXY_URL=http://host.docker.internal:1087
VCD_NO_PROXY=localhost,127.0.0.1,::1,host.docker.internal,.local
```

### Outputs

成功初始化会写入配置：

```text
user.name=<user>
user.email=<email>
ssh.key_path=/Users/<user>/.ssh/id_rsa
initialized_at=<timestamp>
container.docker_build=~/.config/vcd/Dockerfile
container.id=vcd-<user>:<timestamp>
```

并构建本地 Docker image：

```text
vcd-<user>:<timestamp>
```

## 3. Core Logic

当前实现是两个阶段：先写必要文件，再构建镜像。

### Phase 1: 写配置和构建文件

1. 解析 CLI 参数，得到 `user`。
2. 检查 `~/.config/vcd/config` 是否已有非空 `initialized_at`。
3. 如果已经初始化，失败并提示使用 `vcd config set <key> <value>`。
4. 校验 `user`。
5. 交互式读取 user email 和 SSH key path。
6. 校验 SSH key path 文件存在。
7. 生成时间戳：

   ```bash
   date +%Y%m%d%H%M%S
   ```

8. 生成目标镜像名：

   ```text
   vcd-<user>:<timestamp>
   ```

9. 写入 `~/.config/vcd/config`。
10. 从刚写入的 config 读取 `container.docker_build`。
11. 把内嵌 `Dockerfile` 写到 `container.docker_build`。
12. 把内嵌 `optional-packages.txt` 写到 Dockerfile 同目录。

### Phase 2: 构建镜像

1. 解析 requested base image：
   - 有 `VCD_BASE_IMAGE` 时使用它。
   - 否则使用 `debian:trixie-slim`。
2. 为 requested base image 生成 local alias，例如：

   ```text
   debian:trixie-slim -> vcd-base-debian-trixie-slim:local
   ```

3. 检查 Docker daemon：

   ```bash
   docker version --format {{.Server.Version}}
   ```

4. 准备 base image：
   - `docker image inspect <base-image>`
   - 如果不存在，执行 `docker pull <base-image>`
   - 执行 `docker tag <base-image> <local-base-alias>`
5. 根据 config 中的 `container.docker_build` 推导 Docker build context，即 Dockerfile 所在目录。
6. 执行 Docker build：

   ```bash
   docker build \
     --build-arg BASE_IMAGE=<local-base-alias> \
     --build-arg USERNAME=<user> \
     --build-arg GIT_USERNAME=<git-username> \
     --build-arg GIT_EMAIL=<git-email> \
     --build-arg SSH_KEY=<ssh-key-file> \
     --build-arg VCD_PROXY_URL=<proxy-url> \
     --build-arg VCD_NO_PROXY=<no-proxy> \
     -t vcd-<user>:<timestamp> \
     -f <container.docker_build> \
     <dockerfile-parent-dir>
   ```

## 4. Corners

### Already Initialized

如果 config 中存在非空 `initialized_at`，`init` 必须失败。

提示应指向：

```bash
vcd config set <key> <value>
```

### Config Write Timing

当前流程会在 Docker build 前写 config 和 Docker 构建文件。

因此如果后续 Docker build 失败，config 可能已经写入，但 `container.id` 指向的镜像可能尚未构建成功。用户应修复 Docker/base image 问题后执行：

```bash
vcd rebuild
```

### Docker Unavailable

如果 Docker CLI 不存在或 Docker daemon 不可连接，应在构建阶段失败。

提示应指向 OrbStack / Docker Desktop 是否启动、当前用户是否能访问 Docker。

### Base Image Pull Failure

如果 base image 不存在且 pull 失败，应停止构建。

用户可以通过 `VCD_BASE_IMAGE` 指向本地已有镜像或镜像代理。

### Released Binary Without docker Directory

运行时不能依赖当前目录存在 `docker/`。

`init` 必须使用二进制内嵌资源释放出的 Dockerfile 和 optional package 文件。

### Dockerfile Changes

修改 `docker/Dockerfile` 或 `docker/optional-packages.txt` 后，必须重新编译发布二进制。

旧二进制会继续使用它编译时内嵌的 Docker 文件。

### UID/GID

`init` 不传宿主机 UID/GID。

Dockerfile 只负责创建默认容器用户，并保证：

- home directory 存在。
- 默认 shell 是 zsh。
- 用户具备 sudo 权限。
