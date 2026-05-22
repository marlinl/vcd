# vcd config 设计文档

## 1. Context

`vcd config` 是初始化后的配置修改命令。

`vcd init <user>` 只用于首次初始化。如果 `~/.config/vcd/config` 中已经存在非空 `initialized_at`，再次执行 `init` 会失败。后续要修改用户、邮箱、SSH key 路径、Dockerfile 路径或当前容器标识，应使用 `vcd config set`，然后按需要执行 `vcd rebuild`。

当前只实现 `set` 子命令。

实现上，`src/core/config.rs` 是配置相关用例的统一入口。它同时包含：

- `VcdConfig` 数据结构。
- 默认配置路径解析。
- 配置文件读写、解析和序列化。
- `vcd config set` 的字段更新、校验和落盘流程。

`init`、`rebuild`、`open` 和 Docker 构建逻辑都通过 `core::config` 读取或写入本地配置；项目根目录不再单独维护 `src/config.rs`。

## 2. Contract

### Command

```bash
vcd config set <key> <value>
```

示例：

```bash
vcd config set user.name jack
vcd config set user.email jack@example.com
vcd config set ssh.key_path /Users/jack/.ssh/id_ed25519
vcd config set container.docker_build /Users/jack/.config/vcd/Dockerfile
vcd config set container.id vcd-jack:20260522120000
```

### Config Structure

配置文件固定使用点分 key。当前结构定义为：

```text
user.name=<user-name>
user.email=<email>
ssh.key_path=<absolute-ssh-private-key-path>
initialized_at=<timestamp>
container.docker_build=<absolute-dockerfile-path>
container.id=<current-vcd-image-name>
```

示例：

```text
user.name=jack
user.email=jack@example.com
ssh.key_path=/Users/jack/.ssh/id_ed25519
initialized_at=20260522120000
container.docker_build=/Users/jack/.config/vcd/Dockerfile
container.id=vcd-jack:20260522120000
```

字段语义：

`user.name`：

- 当前 vcd 用户名，同时作为容器内 Linux 用户名和 Git `user.name`。
- 不能为空。
- 必须满足 Docker/Linux 用户名约束：1-32 位，只包含小写字母、数字、`_`、`-`，并以小写字母或 `_` 开头。

`user.email`：

- 用户邮箱，同时作为 Git `user.email`。
- 不能为空，且必须包含 `@`。

`ssh.key_path`：

- 宿主机 SSH 私钥的绝对路径。
- 不能为空，必须是绝对路径，且文件必须存在。
- 容器内会把该 key 挂载到 `/home/<user.name>/.ssh/<file-name>`，Dockerfile 中的 `SSH_KEY` build arg 使用路径最后一段文件名。

`initialized_at`：

- 首次初始化时间戳。
- 用于判断是否已经初始化；`vcd init <user>` 看到非空 `initialized_at` 后必须拒绝再次初始化。
- 不参与 `open` 阶段的镜像名拼接。

`container.docker_build`：

- Dockerfile 的绝对路径。
- 不能为空。
- 后续 `vcd rebuild` 会按这个路径写 Dockerfile，并把 `optional-packages.txt` 写到 Dockerfile 同目录。

`container.id`：

- 当前 vcd 使用的完整本地 Docker image name/tag。
- 不能为空。
- `open` 阶段直接使用该值执行 `docker image inspect` 和 `docker run`，不再和 `initialized_at` 拼接。
- 示例：`vcd-jack:20260522120000`。

### Requirements

执行 `config set` 前必须已经存在有效配置：

```text
~/.config/vcd/config
```

配置至少需要能被当前 parser 读取，包含：

```text
user.name
user.email
ssh.key_path
initialized_at
container.docker_build
container.id
```

旧格式兼容读取：

- `user` 会映射为 `user.name`。
- `git_email` 会映射为 `user.email`。
- `ssh_key` 会按旧规则转换为 `/Users/<user.name>/.ssh/<ssh_key>`。
- `dockerfile_path` 会映射为 `container.docker_build`。
- `base_image` 会原样映射为 `container.id`。

新写入的 config 必须只使用点分 key。

## 3. Core Logic

1. 解析 CLI 参数：

   ```text
   command = config
   subcommand = set
   key = <key>
   value = <value>
   ```

2. 读取默认 config：

   ```text
   ~/.config/vcd/config
   ```

3. 按 key 更新内存中的配置字段。
4. 执行对应校验：
   - `user.name` 非空。
   - `user.email` 非空且包含 `@`。
   - `ssh.key_path` 是绝对路径，并且宿主机 SSH key 存在。
   - `container.docker_build` 非空。
   - `container.id` 非空。
5. 重写 `~/.config/vcd/config`。
6. 输出更新成功，并提示执行：

   ```bash
   vcd rebuild
   ```

## 4. Corners

### Missing Config

如果 config 不存在，应失败并提示先执行：

```bash
vcd init <user>
```

### Unsupported Key

不支持的 key 应失败，并提示当前支持：

```text
user.name
user.email
ssh.key_path
container.docker_build
container.id
```

### Invalid SSH Key

`ssh.key_path` 必须是宿主机绝对路径。

错误示例：

```text
id_rsa
../id_rsa
-i
```

### Rebuild Required

`config set` 只修改配置文件，不会自动重建镜像。

由于用户、邮箱、SSH key 路径、Dockerfile 路径和完整镜像名都会影响后续构建或容器行为，命令成功后应提示用户按需要执行：

```bash
vcd rebuild
```

### User Field

当前用户字段是 `user.name`。修改后应重新执行 `vcd rebuild`，因为容器内 Linux 用户、Git `user.name`、镜像 tag 和容器 home 路径都会受影响。
