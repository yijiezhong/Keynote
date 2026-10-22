# kn 交接说明

## 交接摘要

- 项目：Apple Keynote `.key` 命令行工具
- 语言：Rust 2021
- 分支：`main`
- 当前提交：以 `git log -1 --oneline` 为准
- v3.0.0 release 基线：`d2c8d44`
- 当前发布版本：`v3.0.1`（57 条命令，brew 资产已对齐）
- universal release job：`5e8363e`
- 当前状态：57 条命令已实现；全量回归完成；工作区干净
- GitHub：`https://github.com/yijiezhong/Keynote`（public）

## 快速验收

```bash
cd Keynote
cargo build --release
./bin/kn help
./bin/kn verify <某个 .key>
```

`./bin/kn` 在 `target/release/kn` 不存在时会自动执行 release 构建。

依赖：

- Rust 1.80+
- `protoc`（生成 Keynote protobuf 类型）
- macOS Keynote（仅 Keynote 路径命令需要）

## 当前能力

57 条命令分为：

| 分组 | 数量 |
|---|---:|
| 查看与分析 | 17 |
| 文字 / 备注 | 6 |
| 颜色 / 形状 | 9 |
| 表格 | 9 |
| 图表 / 图片 / 链接 / 动画 | 9 |
| 页面结构 | 5 |
| 批量 | 1 |
| 从零创建 | 1 |

写命令统一使用 `<src> <dst>`，源文件不被覆盖。

坐标约定需要交接时特别说明：

- 页码：1 基
- 表格行列：`set-cell` / 行 / 列 / `set-table-size` 为 1 基
- `merge-cells`：0 基
- 形状路径：0 基，例如 `3/1`
- 图表行列：0 基

## 关键实现位置

| 文件 | 职责 |
|---|---|
| `src/iwa.rs` | IWA / Snappy / protobuf 字节层；写回使用真实 Snappy，literal 仅作兜底 |
| `src/write.rs` | 文档加载、对象定位、段重打包、Zip 写回 |
| `src/apple.rs` | 需要 Keynote 本体的页面/图片/表格底色操作 |
| `src/table.rs` | 表格读写、行列、边框、底色、纯文件层合并 |
| `src/shape.rs` | 形状填充、几何、删除/复制、打组拆组 |
| `src/main.rs` | 57 条命令的 CLI 分发与帮助 |
| `bin/kn` | 自动构建入口 |

## 回归结果

本轮全量回归使用公开样例和私有纯色形状样例：

| 项目 | 结果 |
|---|---:|
| 只读命令调用 | 106 次通过 |
| `check-notes` 缺备注诊断 | 11 次按设计返回退出码 1 |
| 写命令检查 | 75 项通过，0 项失败 |
| Keynote 打开验证 | 39/39 |
| `cargo test --release` | 1/1 |
| 文档命令清单 | 57/57 |
| 文档断链 | 0 |
| 指南 PDF 页数 | 中文 8 页；英文 9 页 |
| 格式报告 PDF 页数 | 中文 24 页；英文 25 页 |

回归中发现并已修复：

- `set-table-fill --no-band` 参数未实现；
- `dup-shape` 复制表格会生成无效文件，现在明确拒绝表格与占位符；
- 最终 `set-shape` 用私有纯色形状样本验证通过：PNG 中检测到 8,415 个绿色采样点，颜色为 `(0,170,0)`。
- 交接验收时发现 `image-info` 已实现但未列入 help 与文档，命令数由 56 更正为 57；
- 同时修掉 help 里 `set-table-border` / `merge-cells` / `set-table-fill` 的 3 行重复。

## 交接验收记录

本次交接在 `2026-10-22` 于本机逐项核对，结果如下：

| 验收项 | 方法 | 结果 |
|---|---|---|
| 工作区与远端一致 | `git status` / `git rev-parse origin/main` | 干净；本地 `main` = `origin/main` |
| 交接包完整性 | `shasum -a 256 -c` | 通过 |
| 交接包内容 | 检查包内条目 | 不含 `.git` / `target` / `.key` |
| 命令入口覆盖 | help 与 `src/main.rs` 的 `Some("...")` 对照 | 61 个入口一一对应，无重复、无遗漏 |
| 文档命令清单 | 中英命令表合计 | 均为 57 |
| 文档断链 | 相对链接逐个解析 | 0 |
| 只读命令可用 | 用 `REA/samples` 私有样本跑 `verify` / `audit` / `images` / `image-info` | 均正常 |
| 单元测试 | `cargo test --release` | 1/1 |
| PDF 页数 | `mdls kMDItemNumberOfPages` | 指南 8/9；格式报告 24/25 |
| 远端同步 | `git ls-remote` + `git push` | 远端 `main` 已更新到本次交接提交 |
| Release 资产 | GitHub API 列资产 | `v3.0.1` 共 6 个资产（3 tar.gz + 3 sha256），非草稿 |
| 发布二进制版本 | 下载 `v3.0.1` universal 资产跑 `kn help` | 显示 57 条；`lipo -archs` = x86_64 arm64 |
| 发布资产哈希 | 本地 `shasum` 与 release `.sha256` 比对 | 一致（`ebadf614…6ca5`） |
| Tap formula 一致性 | tap 内 formula 的 `sha256` 与资产比对 | 一致；`test` 断言已改为 `"57 条命令"` |
| brew 端到端 | `brew update` / `reinstall` / `test` / `audit --strict` | 全部通过，装到 `kn 3.0.1` |
| 交接包可独立构建 | 解压后在空 `target` 下 `cargo build --release` | 成功（约 23 秒），产物 `kn help` = 57 条、61 入口 |

## 已知限制

- `set-shape` 只支持纯色填充；渐变/图片填充会明确报错。
- `del-col` 属于实验性结构改写，交付前必须用 Keynote 复核。
- 以下命令需要 Keynote 与自动化权限：
  - `new`
  - `add-slide`
  - `change-layout`
  - `insert-image`
  - `insert-image-paste`
  - `del-slide`
  - `dup-slide`
  - `set-table-fill`
- `merge-cells` 已改为纯文件层实现，不再依赖 Keynote 菜单。
- `dup-shape` 支持图片和普通形状；表格和占位符会拒绝。
- `del-shape` 当前只支持顶层 drawable 路径。
- 公开仓库不包含真实 `.key` 样本；回归使用本地私有样本。
- 更旧 Keynote 版本未逐一验证；主要在 macOS 26 / Keynote 15.4 验证。

## 文档

| 文档 | 路径 |
|---|---|
| 英文 README | `README.md` |
| 中文 README | `README.zh-CN.md` |
| 中文指南 Markdown | `docs/kn-guide.zh-CN.md` |
| 英文指南 Markdown | `docs/kn-guide.en.md` |
| 中文指南 PDF | `docs/kn-guide.zh-CN.pdf` |
| 英文指南 PDF | `docs/kn-guide.en.pdf` |
| 中文格式报告 PDF | `docs/AppleKeynoteFormatReport.pdf` |
| 英文格式报告 PDF | `docs/AppleKeynoteFormatReport.en.pdf` |

## Homebrew 发布

- Tap：`https://github.com/yijiezhong/keynote_cli`
- Formula：`Formula/kn.rb`
- 当前 Release：`https://github.com/yijiezhong/Keynote/releases/tag/v3.0.1`
- 历史 Release：`https://github.com/yijiezhong/Keynote/releases/tag/v3.0.0`
- 工作流：`.github/workflows/release.yml`
- 工作流会为后续 tag 自动生成 arm64、x86_64 和 universal 三种资产
- 当前预编译资产（`v3.0.1`）：
  - `kn-v3.0.1-universal.tar.gz`（formula 使用，sha256 `ebadf614…6ca5`）
  - `kn-v3.0.1-arm64.tar.gz`
  - `kn-v3.0.1-x86_64.tar.gz`

> **版本对齐状态（2026-10-22）**：`v3.0.1` 的三个预编译资产与仓库源码一致，
> 都是 **57 条命令**（`kn help` 首行显示「57 条命令」）。
> `v3.0.0` 的旧资产仍是 56 条命令的二进制，已被 formula 弃用，仅作历史留存。

安装：

```bash
brew tap yijiezhong/keynote_cli https://github.com/yijiezhong/keynote_cli.git
brew install yijiezhong/keynote_cli/kn
```

注意：tap 仓库名是 `keynote_cli`，不是 `homebrew-keynote_cli`，所以 `brew tap` 要带显式 URL。

已实测（`2026-10-22`，本机在装 `kn 3.0.0` 的基础上升级）：

```text
brew update                                   → 通过（tap 拉取到 v3.0.1 formula）
brew reinstall yijiezhong/keynote_cli/kn      → 通过（装到 3.0.1）
brew test  yijiezhong/keynote_cli/kn          → 通过（退出码 0）
brew audit --strict yijiezhong/keynote_cli/kn → 通过（退出码 0）
kn help                                       → 「57 条命令」
```

## 源码归档

交接源码包由 `git archive` 从对应提交生成：

```text
kn-handover-20261022.tar.gz
kn-handover-20261022.sha256
```

包内不含 `.git`、`target` 和私有 `.key` 样本；解压后需先 `cargo build --release`。

包由当前 `HEAD` 用 `git archive` 生成，校验方式：

```bash
shasum -a 256 -c kn-handover-20261022.sha256
tar -tzf kn-handover-20261022.tar.gz | grep -E '(^|/)\.git/|(^|/)target/|\.key$'   # 应无输出
```

## GitHub 交接

已配置 remote 并完成首次 push：

```text
origin  https://github.com/yijiezhong/Keynote.git
branch  main -> origin/main
```

提交身份已配置为：

```text
kn <noreply@example.com>
```

`v3.0.0` 与 `v3.0.1` 已打 tag 并发布。后续新版本流程：

```bash
# 1. 更新 Cargo.toml 版本并提交
git tag -a vX.Y.Z -m "kn X.Y.Z"
git push origin main --tags
# 2. GitHub Actions 自动构建 arm64 / x86_64 / universal
# 3. 更新 tap formula 的 Release URL 与 SHA256
```

> **本机网络提示**：GitHub 的 443 直连不可用，但本机 `127.0.0.1:1087` 的 HTTP 代理可用。
> 本次交接的 push 用下面这条命令完成：
>
> ```bash
> git -c http.proxy=http://127.0.0.1:1087 push origin main
> ```
>
> 若代理端口变化，先确认 xray/V2rayU 的 HTTP 入站端口再替换。

交接时已核对：默认分支 `main`、仓库 public、许可 MIT、open issues 0。
`2026-10-22` 的交接验收提交已推送到 `origin/main`，`v3.0.1` 已打 tag 并发布。

## 后续建议

1. ~~在 GitHub 上确认默认分支为 `main`~~ —— 已确认（`default_branch: main`）。
2. ~~发布 `v3.0.1` 对齐 brew 资产~~ —— 已完成：CI 出三种资产，tap formula 的
   `url` / `sha256` / `test` 断言均已更新，本机 `brew reinstall` 装到 `kn 3.0.1`。
3. 如需长期回归，补充一个可公开的纯色形状 fixture 到测试集。
4. 发布新版本时更新 `Cargo.toml`、打新 tag，并同步更新 tap formula 的 URL/SHA。
5. 交付前按指南执行：

```bash
./bin/kn verify 输出.key
./bin/kn audit 输出.key
```

并用 Keynote 打开输出确认。
