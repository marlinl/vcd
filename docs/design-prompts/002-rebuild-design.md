# vcd rebuild 设计文档

## 1. Context

`vcd rebuild [user]` 是本地基础镜像的重新构建命令。

它用于以下场景：

- vcd 二进制升级后，需要使用新二进制内嵌的 Dockerfile 重新构建镜像。
- `docker/optional-packages.txt` 的勾选内容变化后，需要生成新基础镜像。
- 用户通过 `VCD_BASE_IMAGE` 切换 base image。
- 当前配置里的 `container.id` 指向的本地 vcd image 不存在或过旧。
- 用户希望在 rebuild 时显式切换当前 vcd user。

`rebuild` 复用 `init` 的 build pipeline，但不受 `initialized_at` 的一次性初始化限制。

## 2. Contract

### Commands

```bash
vcd rebuild
vcd rebuild <user>
```

### Inputs

`vcd rebuild` 不带参数时，从当前配置读取：

```text
user.name
user.email
ssh.key_path
container.docker_build
container.id
```

`vcd rebuild <user>` 带参数时，使用传入 user，重新询问 user email 和 SSH key path。

`user` 的格式与 `vcd init <user>` 相同：

- 长度 1-32。
- 只能包含小写 ASCII 字母、数字、`_`、`-`。
- 第一个字符必须是小写 ASCII 字母或 `_`。

### Outputs

成功后创建一个新的本地镜像：

```text
vcd-<user>:<timestamp>
```

并更新：

```text
~/.config/vcd/config
~/.config/vcd/Dockerfile
~/.config/vcd/optional-packages.txt
```

配置格式：

```text
user.name=<user>
user.email=<email>
ssh.key_path=<ssh-key-path>
initialized_at=<timestamp>
container.docker_build=<existing-or-default-dockerfile-path>
container.id=vcd-<user>:<timestamp>
```

### Non-Goals

`rebuild` 不负责：

- 删除旧镜像。
- 重建已有项目容器。
- 迁移运行中的容器。
- 修改容器内 Git 仓库。
- 启动 `codex` 或 `claude`。

## 3. Core Logic

### User Resolution

执行：

```bash
vcd rebuild
```

流程：

1. 定位 `~/.config/vcd/config`。
2. 读取配置。
3. 提取 `user.name`、`user.email`、`ssh.key_path`、`container.docker_build` 和 `container.id`。
4. 校验 `user.name` 和 SSH key path 文件。
5. 沿用已有非空 `container.docker_build`。

执行：

```bash
vcd rebuild <user>
```

流程：

1. 使用命令行传入的 `user`。
2. 询问 user email 和 SSH key path。
3. 校验 `user` 和 SSH key path 文件。
4. 如果已有 config 且 `container.docker_build` 非空，继续沿用它；否则使用默认 `~/.config/vcd/Dockerfile`。

### Rebuild Flow

完成 user resolution 后，调用和 `init` 相同的 build-and-write-config 逻辑：

1. 复用已有 `initialized_at`，没有已有 config 时生成初始化时间戳。
2. 为本次构建生成新的完整镜像名并写入 `container.id`，例如 `vcd-<user>:<build-timestamp>`。
3. 写入新的 `~/.config/vcd/config`。
4. 从 config 读取 `container.docker_build`。
5. 写入 Dockerfile 和 `optional-packages.txt`。
6. 解析 `VCD_BASE_IMAGE` 或默认 `debian:trixie-slim`。
7. 准备 local base alias。
8. 执行 `docker build`。

Docker build 的完整契约见 `001-init-design.md`。

### Config Write Timing

当前实现会在 Docker build 前写 config。

如果 Docker build 失败，config 可能已经写入，但 `container.id` 指向的 image 尚未构建成功。用户应修复问题后再次执行：

```bash
vcd rebuild
```

## 4. Corners

### Missing Config

`vcd rebuild` 不带 user 时，如果配置不存在或缺少必要字段，应失败并提示：

```bash
vcd init <user>
```

`vcd rebuild <user>` 可以在没有已有 config 时运行，因为 user 和 Git/SSH 配置会在交互流程中重新填写。

### Custom Dockerfile Path

如果用户通过以下命令修改了 Dockerfile 路径：

```bash
vcd config set container.docker_build <path>
```

后续 `rebuild` 应沿用该路径，并把 `optional-packages.txt` 写到 Dockerfile 同目录。

### Build Failure

如果 Docker build 失败：

- 错误归类到 image build 阶段。
- 输出 Docker exit status。
- config 可能已经更新，但对应 image tag 尚未构建成功。

### User Switch

`vcd rebuild <user>` 成功后会把当前配置切换到新 user。

这会影响后续：

```bash
vcd codex <git-url>
vcd claude <git-url>
```

因为 container name、挂载路径和 base image 都和 user 有关。

### Existing Containers

项目打开命令使用带时间戳的一次性 container，并在退出后清理。

`rebuild` 不会主动处理已有 container。

### Old Images

`rebuild` 会留下旧的 `vcd-<user>:...` 镜像。

当前不自动 prune，因为旧镜像可能仍被已有容器引用，也可能用于排障回滚。

### Base Image Change

用户可以执行：

```bash
VCD_BASE_IMAGE=<image> vcd rebuild
```

成功后 config 指向基于新 base image 构建出来的新 vcd image。

### Concurrent Rebuilds

并发 rebuild 可能竞争：

- `~/.config/vcd`
- `~/.config/vcd/config`
- local base alias tag

当前不定义锁机制。后续如果需要支持并发，应增加 lock file。

### Timestamp Collision

image tag 使用秒级时间戳。

同一 user 在同一秒内 rebuild 两次可能生成相同 tag。当前本地场景可接受，后续可以改成更高精度时间戳。
