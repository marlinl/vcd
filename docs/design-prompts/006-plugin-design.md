# vcd plugin 设计文档

## 1. Context

`vcd plugin` 是 vcd 的插件管理命令。

当前只定义最小能力：

```bash
vcd plugin add <git-url>
vcd plugin list
```

该命令把指定 Git 仓库 clone 到本机 vcd 配置目录下：

```text
~/.config/vcd/plugins/
```

现阶段 `plugin add` 只负责下载插件仓库，`plugin list` 只负责列举本机已经下载的插件。它们不负责启用插件、不校验插件 manifest、不把插件自动注入 Docker container，也不改变 `vcd codex` 或 `vcd claude` 的启动行为。

## 2. Contract

### Command

```bash
vcd plugin add <git-url>
vcd plugin list
```

示例：

```bash
vcd plugin add https://github.com/user/vcd-plugin-example.git
vcd plugin add git@github.com:user/vcd-plugin-example.git
vcd plugin list
```

### Inputs

`git-url`：

- 支持常见 HTTPS Git URL。
- 支持常见 SSH Git URL。
- project name 从 URL 最后一段推导。
- `.git` 后缀会被移除。
- URL 末尾 `/` 应被忽略。

插件本地目录名使用推导出的 project name：

```text
https://github.com/user/vcd-plugin-example.git
-> ~/.config/vcd/plugins/vcd-plugin-example
```

### Prerequisites

本机必须具备：

- Git CLI。
- 对目标仓库的访问权限。
- 可写的 vcd 配置目录：

  ```text
  ~/.config/vcd/
  ```

如果 `~/.config/vcd/plugins/` 不存在，命令应自动创建。

### Outputs

`plugin add` 成功时输出插件已添加，并显示本地路径：

```text
Plugin added: vcd-plugin-example
Path: ~/.config/vcd/plugins/vcd-plugin-example
```

`plugin list` 成功时输出当前插件列表：

```text
vcd-plugin-a    ~/.config/vcd/plugins/vcd-plugin-a
vcd-plugin-b    ~/.config/vcd/plugins/vcd-plugin-b
```

如果插件目录不存在或目录为空，输出：

```text
No plugins installed.
```

失败时必须说明失败阶段，并给出可操作提示。

常见失败阶段：

- 参数解析失败。
- Git 仓库地址非法。
- Git CLI 不存在。
- 插件目录创建失败。
- 目标插件目录已存在。
- Git clone 失败。
- 插件目录读取失败。

## 3. Core Logic

### plugin add

1. 解析 CLI 参数：

   ```text
   command = plugin
   subcommand = add
   git_url = <git-url>
   ```

2. 校验并解析 Git URL。
3. 从 Git URL 推导 plugin name：
   - `https://github.com/user/plugin.git -> plugin`
   - `git@github.com:user/plugin.git -> plugin`
   - `https://github.com/user/plugin/ -> plugin`
4. 生成插件根目录：

   ```text
   ~/.config/vcd/plugins
   ```

5. 如果插件根目录不存在，创建该目录。
6. 生成目标 clone 目录：

   ```text
   ~/.config/vcd/plugins/<plugin-name>
   ```

7. 如果目标目录已存在，失败并提示用户该插件已经存在。
8. 检查 Git CLI 是否可执行：

   ```bash
   git --version
   ```

9. 执行 clone：

   ```bash
   git clone <git-url> ~/.config/vcd/plugins/<plugin-name>
   ```

10. clone 成功后输出插件名称和本地路径。

所有外部命令必须使用 `std::process::Command` 和参数数组构造，不要拼接 shell 字符串执行。

### plugin list

1. 解析 CLI 参数：

   ```text
   command = plugin
   subcommand = list
   ```

2. 生成插件根目录：

   ```text
   ~/.config/vcd/plugins
   ```

3. 如果插件根目录不存在，输出：

   ```text
   No plugins installed.
   ```

4. 读取插件根目录下的直接子目录。
5. 只列举目录，不列举普通文件。
6. 按插件目录名升序输出。
7. 如果没有任何插件目录，输出：

   ```text
   No plugins installed.
   ```

8. 如果存在插件目录，逐行输出插件名和路径：

   ```text
   <plugin-name>    ~/.config/vcd/plugins/<plugin-name>
   ```

## 4. Safety

`plugin add` 不应覆盖已有插件目录。

如果目标目录已经存在：

- 不执行 `git pull`。
- 不删除目录。
- 不覆盖用户修改。
- 返回明确错误，并提示用户手动处理目录或后续使用将来可能提供的更新命令。

`plugin add` 不应读取或输出敏感 token。SSH URL 的权限和认证交给本机 Git/SSH 配置处理。

插件仓库内容不应在 add 阶段执行。当前阶段只允许 clone，不运行插件中的脚本、install hook 或任意命令。

`plugin list` 必须保持只读：

- 不创建插件目录。
- 不修改插件仓库。
- 不执行插件仓库中的任何文件。
- 不读取或解析插件 manifest。

## 5. Corners

### Missing Git URL

缺少 `<git-url>` 时，参数解析失败，并提示正确用法：

```bash
vcd plugin add <git-url>
```

### Invalid Git URL

如果 URL 无法推导出非空 plugin name，应失败并提示传入有效 Git 仓库地址。

### Existing Plugin Directory

如果以下目录已经存在：

```text
~/.config/vcd/plugins/<plugin-name>
```

命令必须失败，避免覆盖已有插件或用户改动。

### Empty Plugin Directory

如果 `~/.config/vcd/plugins/` 存在但没有任何子目录，`vcd plugin list` 输出：

```text
No plugins installed.
```

### Non-directory Files

如果 `~/.config/vcd/plugins/` 下存在普通文件，`vcd plugin list` 忽略这些文件，只列举直接子目录。

### Clone Interrupted

如果 clone 中断并留下半成品目录，后续再次执行会因目录已存在而失败。错误提示应说明目标目录已存在，并提示用户检查后手动删除或重命名该目录。

### Private Repository

私有仓库认证由 Git 自身处理。vcd 不应在 `plugin add` 中要求或保存 token。

### Future Extension

后续可以在不改变当前语义的基础上增加：

- `vcd plugin remove <name>`
- `vcd plugin update <name>`
- 插件 manifest 校验。
- 将本机插件目录 mount 到项目容器。
