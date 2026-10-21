# dsh-keynote

把本仓库的 Rust `kn` CLI 暴露成 DeepSeek Harness 的 `keynote_kn` 工具。

> **阶段 1（只读）**：当前只实现 `list` / `verify` / `info`；写回与编辑命令正在从 Python 迁移到 Rust。

## 准备

```bash
cd /path/to/Keynote
cargo build --release
```

## 它做什么

注册一个 Host 工具 `keynote_kn`，执行：

```bash
<projectRoot>/bin/kn <args...>
```

示例：

```json
{ "args": ["list", "deck.key"] }
{ "args": ["verify", "deck.key"] }
{ "args": ["help"] }
```

参数是**数组**，不经过 shell。只读命令标记为可并发。

## 配置

| 键 | 默认 | 说明 |
|---|---|---|
| `projectRoot` | `~/PARA/8.Code/Keynote`（或 `KN_PROJECT_ROOT`） | kn 仓库根 |
| `timeoutMs` | `180000` | 单次命令超时 |
| `maxOutputChars` | `40000` | 输出上限 |

## 安装

```bash
plugin_manager install_bundle  target=/path/to/Keynote/plugins/dsh-keynote
```

## 限制

- 目前只读；`outline`、`set-text`、`new`、`add-slide` 等命令在 Rust 迁移完成后才可用。
- 需要本机已安装 Rust 工具链并完成 `cargo build --release`。
