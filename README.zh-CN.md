# kn

[English](README.md) | 简体中文

读取与编辑 Apple Keynote `.key` 文件的 Rust 命令行工具。

`kn` 的大部分操作直接处理 IWA / zip 容器；只有在 Keynote 文档模型强制要求时才调用
Keynote 本体（新建文稿、新增页、插入图片，以及部分表格样式操作）。

## 状态

**Rust 3.0 —— 56 条命令已全部迁移。**

- 不依赖外部运行时 schema 包；
- 公开仓库只包含 Rust 实现；
- 写操作统一使用 `<src> <dst>`，源文件保持不变；
- 需要 Keynote 的命令仍需安装 macOS Keynote，并授予自动化权限。

完整说明见：[简体中文 PDF](docs/kn-guide.zh-CN.pdf) · [English PDF](docs/kn-guide.en.pdf)。

## 快速上手

```bash
./bin/kn help
./bin/kn outline 你的文件.key
./bin/kn set-text 输入.key 输出.key 1 title "新标题"
./bin/kn verify 输出.key
```

`./bin/kn` 在缺少二进制时会在首次运行自动构建。

## 构建

```bash
cargo build --release
./bin/kn list 你的文件.key
```

依赖：

- Rust 1.80+（edition 2021）
- `protoc`（`prost-build` 生成 Keynote protobuf 模块）
- macOS Keynote：`new`、`add-slide`、`change-layout`、`insert-image`、
  `insert-image-paste`、`del-slide`、`dup-slide`，以及 Keynote 路径的表格底色/合并

## 命令分组

| 分组 | 数量 | 命令 |
|---|---:|---|
| 查看与分析 | 17 | `list` `verify` `text` `texts` `types` `outline` `tables` `charts` `images` `links` `builds` `builds-list` `transitions` `styles` `colors` `audit` `check-notes` |
| 文字 / 备注 | 6 | `set-text` `set-text-color` `set-text-style` `note` `set-note` `edit` |
| 颜色 / 形状 | 9 | `set-bg` `set-shape` `set-color` `set-style` `set-shape-geometry` `group` `ungroup` `del-shape` `dup-shape` |
| 表格 | 9 | `set-cell` `add-row` `del-row` `add-col` `del-col` `set-table-border` `set-table-fill` `set-table-size` `merge-cells` |
| 图表 / 图片 / 链接 / 动画 | 8 | `set-chart` `set-image` `insert-image` `insert-image-paste` `set-link` `add-build` `set-build` `set-transition` |
| 页面结构 | 5 | `add-slide` `change-layout` `del-slide` `dup-slide` `skip` |
| 批量 | 1 | `batch` |
| 从零创建 | 1 | `new` |

`info` 是 `verify` 的别名；`skip-many` 是 `skip` 的另一种入口。
`del-slide` 支持页码表达式：`3`、`1-3,7`、`all`。

## 文档

| 文档 | 简体中文 | English |
|---|---|---|
| kn 使用与操作指南 | [PDF](docs/kn-guide.zh-CN.pdf) · [Markdown](docs/kn-guide.zh-CN.md) | [PDF](docs/kn-guide.en.pdf) · [Markdown](docs/kn-guide.en.md) |
| 格式分析报告 | [PDF](docs/AppleKeynoteFormatReport.pdf) · [Markdown](docs/AppleKeynoteFormatReport.md) | [PDF](docs/AppleKeynoteFormatReport.en.pdf) · [Markdown](docs/AppleKeynoteFormatReport.en.md) |

## 安全边界

大多数命令读取源 `.key`、写出新文件。交付前请保留原件，并用 Keynote 打开输出确认。
`del-col` 等结构改写属于实验性能力，务必在 Keynote 中复核后再交付。

## 许可证

MIT，见 [MIT 许可证](LICENSE)。
