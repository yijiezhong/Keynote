# kn 交接说明

## 交接摘要

- 项目：Apple Keynote `.key` 命令行工具
- 语言：Rust 2021
- 分支：`main`
- 代码基线：`51bf02b`
- 当前状态：56 条命令已实现；全量回归完成；工作区干净
- GitHub：尚未配置 remote，尚未 push

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

56 条命令分为：

| 分组 | 数量 |
|---|---:|
| 查看与分析 | 17 |
| 文字 / 备注 | 6 |
| 颜色 / 形状 | 9 |
| 表格 | 9 |
| 图表 / 图片 / 链接 / 动画 | 8 |
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
| `src/main.rs` | 56 条命令的 CLI 分发与帮助 |
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
| 文档命令清单 | 56/56 |
| 文档断链 | 0 |
| 指南 PDF 页数 | 中文 8 页；英文 9 页 |
| 格式报告 PDF 页数 | 中文 24 页；英文 25 页 |

回归中发现并已修复：

- `set-table-fill --no-band` 参数未实现；
- `dup-shape` 复制表格会生成无效文件，现在明确拒绝表格与占位符；
- 最终 `set-shape` 用私有纯色形状样本验证通过：PNG 中检测到 8,415 个绿色采样点，颜色为 `(0,170,0)`。

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

## GitHub 交接

当前没有 remote，尚未 push。

```bash
git remote add origin <GitHub 仓库 URL>
git push -u origin main
```

提交身份已配置为：

```text
kn <noreply@example.com>
```

如果交接后需要打版本标签，可在确认 GitHub remote 后执行：

```bash
git tag -a v3.0.0 -m "kn 3.0.0"
git push origin main --tags
```

## 后续建议

1. 配置 GitHub remote 并完成首次 push。
2. 在 GitHub 上确认默认分支为 `main`。
3. 如需长期回归，补充一个可公开的纯色形状 fixture 到测试集。
4. 交付前按指南执行：

```bash
./bin/kn verify 输出.key
./bin/kn audit 输出.key
```

并用 Keynote 打开输出确认。
