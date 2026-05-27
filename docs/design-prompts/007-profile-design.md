# vcd profile 设计文档

## 1. Context

`vcd profile` 是 vcd 的插件组合配置命令。

当前定义最小能力：

```bash
vcd profile <profile-name> add <plugin-name>
vcd profile <profile-name>
```

profile 用于表达某个工作场景需要哪些 plugin。一个 profile 可以关联多个 plugin。

现阶段 `profile` 只负责维护本机配置关系，不负责启用插件、不校验插件 manifest、不把插件自动注入 Docker container，也不改变 `vcd codex` 或 `vcd claude` 的启动行为。

profile 数据存放在本机 vcd 配置目录下：

```text
~/.config/vcd/profiles/
```

plugin 名称对应 `vcd plugin add` 下载后的本地目录名：

```text
~/.config/vcd/plugins/<plugin-name>
```

## 2. Contract

### Command

```bash
vcd profile <profile-name> add <plugin-name>
vcd profile <profile-name>
```

示例：

```bash
vcd profile backend add rust-tools
vcd profile backend add openai-tools
vcd profile backend
```

### Inputs

`profile-name`：

- 不能为空。
- 只能包含 ASCII 字母、数字、`.`、`_`、`-`。
- 不能是 `.` 或 `..`。
- 不能以 `-` 开头。

`plugin-name`：

- 不能为空。
- 必须满足和 `vcd plugin add` 推导出的插件目录名一致的安全规则。
- 必须已经存在于：

  ```text
  ~/.config/vcd/plugins/<plugin-name>
  ```

### Storage

每个 profile 使用一个文本文件保存：

```text
~/.config/vcd/profiles/<profile-name>
```

文件内容每行一个 plugin name：

```text
rust-tools
openai-tools
```

写入时应去重，并按 plugin name 升序保存，保证输出稳定。

### Outputs

`profile add` 成功时输出：

```text
Profile updated: backend
Plugin added: rust-tools
```

如果该 plugin 已经关联到 profile，输出：

```text
Profile unchanged: backend
Plugin already added: rust-tools
```

`vcd profile <profile-name>` 成功时输出该 profile 关联的 plugin：

```text
Profile: backend
Plugins:
rust-tools
openai-tools
```

如果 profile 存在但没有关联任何 plugin，输出：

```text
Profile: backend
Plugins: none
```

如果 profile 不存在，失败并提示先添加插件：

```bash
vcd profile <profile-name> add <plugin-name>
```

失败时必须说明失败阶段，并给出可操作提示。

常见失败阶段：

- 参数解析失败。
- profile name 非法。
- plugin name 非法。
- plugin 不存在。
- profile 目录创建失败。
- profile 读取失败。
- profile 写入失败。

## 3. Core Logic

### profile add

1. 解析 CLI 参数：

   ```text
   command = profile
   profile_name = <profile-name>
   subcommand = add
   plugin_name = <plugin-name>
   ```

2. 校验 `profile-name`。
3. 校验 `plugin-name`。
4. 检查 plugin 目录是否存在：

   ```text
   ~/.config/vcd/plugins/<plugin-name>
   ```

5. 如果 plugin 不存在，失败并提示先执行：

   ```bash
   vcd plugin add <git-url>
   ```

6. 生成 profile 根目录：

   ```text
   ~/.config/vcd/profiles
   ```

7. 如果 profile 根目录不存在，创建该目录。
8. 读取 profile 文件：

   ```text
   ~/.config/vcd/profiles/<profile-name>
   ```

   如果文件不存在，按空列表处理。

9. 把 `plugin-name` 加入列表。
10. 对列表去重并按名称升序排序。
11. 重写 profile 文件。
12. 输出更新结果。

### profile show

1. 解析 CLI 参数：

   ```text
   command = profile
   profile_name = <profile-name>
   ```

2. 校验 `profile-name`。
3. 读取 profile 文件：

   ```text
   ~/.config/vcd/profiles/<profile-name>
   ```

4. 如果文件不存在，失败并提示先添加插件。
5. 解析文件中的 plugin name。
6. 去掉空行。
7. 去重并按名称升序输出。

## 4. Safety

`profile add` 只修改对应 profile 文件，不修改 plugin 仓库内容。

`profile show` 必须保持只读：

- 不创建 profile 目录。
- 不创建 profile 文件。
- 不修改 plugin 目录。
- 不执行插件仓库中的任何文件。
- 不读取或解析插件 manifest。

profile 文件只能写入 plugin name，不写入 token、Git URL 或其他敏感信息。

所有路径必须使用 `Path`/`PathBuf` 构造，不要拼接 shell 字符串。

## 5. Corners

### Missing Profile Name

缺少 `<profile-name>` 时，参数解析失败，并提示正确用法：

```bash
vcd profile <profile-name>
vcd profile <profile-name> add <plugin-name>
```

### Missing Plugin Name

缺少 `<plugin-name>` 时，参数解析失败，并提示正确用法：

```bash
vcd profile <profile-name> add <plugin-name>
```

### Invalid Profile Name

如果 `profile-name` 不能安全映射为本地文件名，应失败并提示使用简单名称，例如：

```text
backend
frontend
rust-tools
```

### Missing Plugin

如果以下目录不存在：

```text
~/.config/vcd/plugins/<plugin-name>
```

`profile add` 必须失败，避免 profile 指向未安装插件。

### Duplicate Plugin

如果同一个 plugin 已经关联到 profile，再次 add 不应重复写入。命令应成功返回，并明确说明没有修改。

### Multiple Plugins

同一个 profile 可以关联多个 plugin。输出时按 plugin name 升序显示，避免因添加顺序不同导致结果不稳定。

### Missing Profile

执行：

```bash
vcd profile <profile-name>
```

如果 profile 文件不存在，应失败并提示先使用 `add` 创建关联。

### Future Extension

后续可以在不改变当前语义的基础上增加：

- `vcd profile list`
- `vcd profile <profile-name> remove <plugin-name>`
- `vcd profile <profile-name> clear`
- 打开项目时指定 profile。
- 将 profile 关联的插件 mount 到项目容器。
