# kn 工具操作指南
> **实现状态（Rust 3.0）**：56 条命令已全部迁移到 Rust；本文保留为命令参考。


> 版本：v1.1
>
> 更新：2026-10-21
>
> 适用：macOS 26 / Keynote 15.4（v14_5 schema 兼容 M6.5–M15.4）

本指南覆盖 kn 当前全部能力：56 条命令。普通命令遵循「只改副本、源文件只读」；
`add-slide` / `new` / `insert-image-paste` 会调用 Keynote 本体。

> 约定：Keynote `.key` 的生成与操作优先用本工具，不要自己写 AppleScript；
> kn 内部按需调用 Keynote 本体（`add-slide`、`insert-image-paste`、`new`）。

---

## 一、工具概览

| 项 | 说明 |
|---|---|
| 入口 | `./bin/kn`（薄入口 `python/keynote_iwa.py`）；`pip install -e .` 后也可直接用 `kn` |
| 数据层 | Zip → IWA 段 → Snappy → protobuf，纯文件层读写 |
| 依赖 | Python 3.9+、protobuf、`kn_schema`（v14_5）；Keynote 用于新增页、图片粘贴、从零创建 |
| 输出 | 普通命令写新文件，源文件只读 |
| 验证 | 结构 `verify` / 综合 `audit`；开发测试需自备样本 |

## 二、安装与运行

```bash
cd ~/PARA/8.Code/Keynote

./bin/kn outline samples/table.key      # 直接运行
./bin/kn verify  samples/table.key

# 可选：安装命令入口（本机 PEP 668 限制，需要 --break-system-packages）
pip install -e . --break-system-packages
kn outline samples/table.key
```

退出码：

| 码 | 含义 |
|---|---|
| 0 | 成功 |
| 1 | 内部错误 / 结构不支持 / 写入校验失败 |
| 2 | 用法错误、文件缺失、加密、iWork '09、页码越界等 |

## 三、命令总览

| 域 | 命令 |
|---|---|
| 查看与分析（17） | `list` `text` `types` `texts` `outline` `tables` `charts` `images` `links` `builds` `builds-list` `transitions` `styles` `colors` `verify` `audit` `check-notes` |
| 文字 / 备注（6） | `set-text` `set-text-color` `set-text-style` `note` `set-note` `edit` |
| 颜色 / 形状（9） | `set-bg` `set-shape` `set-color` `set-style` `set-shape-geometry` `group` `ungroup` `del-shape` `dup-shape` |
| 表格（9） | `set-cell` `add-row` `del-row` `add-col` `del-col` `set-table-border` `set-table-fill` `set-table-size` `merge-cells` |
| 图表 / 图片 / 链接 / 动画（8） | `set-chart` `set-image` `insert-image` `insert-image-paste` `set-link` `add-build` `set-build` `set-transition` |
| 页面结构（5） | `add-slide` `change-layout` `del-slide` `dup-slide` `skip` |
| 批量（1） | `batch` |
| 从零创建（1） | `new` |

## 四、查看与分析

全部只读，不改文件。

```bash
./bin/kn outline 文件.key          # 逐页标题/正文/备注
./bin/kn texts   文件.key [关键字]  # 真实文字及所在对象
./bin/kn types   文件.key          # 按类型统计对象
./bin/kn tables  文件.key          # 表格 ident / 行列 / 表名
./bin/kn charts  文件.key          # 图表与数值网格
./bin/kn images  文件.key          # 图片 ident / 位置 / 尺寸 / 素材文件
./bin/kn links   文件.key          # 超链接
./bin/kn builds-list 文件.key      # 构建动画
./bin/kn transitions 文件.key      # 页间转场
./bin/kn styles  文件.key [关键字]  # 样式库
./bin/kn colors  文件.key [关键字] [--all]
./bin/kn verify  文件.key          # 结构完整性（段/对象解析）
./bin/kn audit   文件.key [--fix] [--out 报告.json]
./bin/kn check-notes 文件.key [字段...]  # 备注合规
```

提示：所有写命令的 ident / 形状路径都先用这些命令查出来，不要猜。

## 五、文字与备注

```bash
# 整段重设标题或正文
./bin/kn set-text 输入.key 输出.key 3 title "新标题"
./bin/kn set-text 输入.key 输出.key 3 body  "新正文"

# 字号 / 粗体 / 斜体 / 颜色（可只改子串）
./bin/kn set-text-style 输入.key 输出.key 3 title --size 72 --bold --color "#C82505"
./bin/kn set-text-style 输入.key 输出.key 3 title --italic --substr "关键字"

# 页级精确改字色
./bin/kn set-text-color 输入.key 输出.key 3 title "#FF00FF"
./bin/kn set-text-color 输入.key 输出.key 3 body  "#00A000" --substr "关键字"
./bin/kn set-text-color 输入.key 输出.key 3 title "#FF00FF" --range 1:3
./bin/kn set-text-color 输入.key 输出.key 3 title "#FF00FF" --para-index 2
./bin/kn set-text-color 输入.key 输出.key 3 title "#FF00FF" --para   # 整段模式

# 备注
./bin/kn note     输入.key 输出.key 3 "追加的备注"
./bin/kn set-note 输入.key 输出.key 3 "新备注" --replace "旧子串"

# 字节级替换（危险操作，先备份）
./bin/kn edit 输入.key 输出.key Index/Document.iwa "旧串" "新串" [--resize] [--all]
```

文字选择器：`title` / `body` / 形状路径（如 `2`、`3/1`）。

## 六、颜色与形状

```bash
./bin/kn set-bg    输入.key 输出.key 3 "#EEEEEE"
./bin/kn set-shape 输入.key 输出.key 3 2 "#C82505"       # 形状路径 2
./bin/kn set-color 输入.key 输出.key 7031018 "#FF00FF" --alpha 0.6
./bin/kn set-style 输入.key 输出.key 7031016 font_size=48
./bin/kn set-style 输入.key 输出.key 7031016 font_color=#FF0000 bold=true
./bin/kn set-shape-geometry 输入.key 输出.key 3 2 x=100 y=200 w=600 h=120 angle=15
./bin/kn group    输入.key 输出.key 3 "2,3"
./bin/kn ungroup  输入.key 输出.key 3 2
./bin/kn del-shape 输入.key 输出.key 3 2
./bin/kn dup-shape 输入.key 输出.key 3 2 --offset 0,250
./bin/kn dup-shape 输入.key 输出.key 3 2 --to 4        # 跨页复制
```

形状路径：顶层对象按顺序编号（从 0 开始），群组内用 `父/子`（如 `3/1`）。

## 七、表格

先 `tables` 查表格 ident，再操作。**注意行列基**：`set-cell` / `add-row` / `del-row` /
`add-col` / `del-col` 从 **1** 开始；`merge-cells` / `set-chart` 从 **0** 开始。

```bash
./bin/kn tables 文件.key

# 改单元格（行列 1 基）
./bin/kn set-cell 输入.key 输出.key 7031011 2 2 777

# 行 / 列（1 基）
./bin/kn add-row 输入.key 输出.key 7031011 1 --at 2
./bin/kn del-row 输入.key 输出.key 7031011 3
./bin/kn add-col 输入.key 输出.key 7031011 3
./bin/kn del-col 输入.key 输出.key 7031011 3        # 实验性，改完务必 Keynote 打开确认

# 边框 / 底色 / 尺寸
./bin/kn set-table-border 输入.key 输出.key 7031011 color="#EE2222" width=4
./bin/kn set-table-fill   输入.key 输出.key 7031011 --body "#FF00FF" --no-band
./bin/kn set-table-fill   输入.key 输出.key 7031011 --header "#EEF2F6" --header-rows 1
./bin/kn set-table-size   输入.key 输出.key 7031011 row-height=80 col-width=180

# 合并（0 基，左上到右下）
./bin/kn merge-cells 输入.key 输出.key 7031011 0 0 1 1
```

`set-cell` 只改数字格；文本格请用 `edit` 或先用 Keynote 改成文本。删列与斑马纹属于结构改写，
验收以 Keynote 打开为准。

## 八、图表 / 图片 / 链接 / 动画

```bash
./bin/kn set-chart 输入.key 输出.key 7031018 0 0 100      # 行列 0 基
./bin/kn set-image 输入.key 输出.key 7031019 新图片.png
./bin/kn set-link  输入.key 输出.key 7031020 "https://example.com"

# 插图：实用路线是剪贴板粘贴（需要辅助功能 + 自动化 Keynote 权限）
./bin/kn insert-image-paste 输入.key 输出.key 3 图片.png
./bin/kn insert-image       输入.key 输出.key 3 图片.png [x] [y] [width]   # 实验性

# 动画 / 转场
./bin/kn add-build 输入.key 输出.key 3 2
./bin/kn set-build 输入.key 输出.key 7035366 duration=2.5 automatic=true
./bin/kn set-transition 输入.key 输出.key 3 apple:dissolve 1.5
```

## 九、页面结构

页码表达式支持 `3`、`1-3,7`、`all`。

```bash
# 新增页：默认交给 Keynote 本体生成（渲染正确）
./bin/kn add-slide 输入.key 输出.key "标题项目符号" \
  --title "标题" --body "正文" --after 3
./bin/kn add-slide 输入.key 输出.key "空白无页码" --replace 5

# 换版式
./bin/kn change-layout 输入.key 输出.key 5 "标题项目符号"

# 删页 / 复制页
./bin/kn del-slide 输入.key 输出.key "3,7-9"
./bin/kn dup-slide 输入.key 输出.key 5

# 跳过放映
./bin/kn skip 输入.key 输出.key "2,4"
./bin/kn skip 输入.key 输出.key "2,4" --off
```

`--file-level` 是纯文件层研究实现（页可见但内容不渲染），不要用于交付；
交付一律用默认的 Keynote 路径。`new slide` 会短暂打开 Keynote，需要自动化权限。

## 十、批量处理

```bash
./bin/kn batch 输入目录 输出目录 set-text 1 title "统一标题"
./bin/kn batch 输入目录 输出目录 set-text-color 1 title "#C82505"
./bin/kn batch 输入目录 输出目录 skip 3
```

对目录下每个 `.key` 执行同一子命令，输出写到目标目录并保持文件名。

## 十一、验证与测试

```bash
cd python
# 开发测试需自备 samples/*.key；公开仓库不包含真实样本
```

单文件验证：

```bash
./bin/kn verify 文件.key          # 结构完整性
./bin/kn audit  文件.key          # 备注/跳过/动画/表格综合体检
```

验收纪律：结构对不等于渲染对。任何交付前用 Keynote 打开确认；涉及颜色/图片/表格样式时，
以导出 PNG/PDF 的像素或页数为最终判据。

## 十二、退出码与故障排查

| 现象 | 原因 | 处理 |
|---|---|---|
| `未找到 protobuf schema` | `kn_schema` 未安装 | 检查 `~/Library/Python/3.15/lib/python/site-packages/kn_schema/` |
| 加密文件报错 | 文档带密码 | Keynote 里「移除密码」后另存 |
| iWork '09 被拒 | 旧格式 `index.apxl` | 在 Keynote 里另存为当前版本；只读 `outline` 可看 |
| `insert-image-paste` 失败 | 缺辅助功能 / 自动化权限 | 系统设置里给终端授权 |
| `add-slide` 失败 | Keynote 未安装 / 权限被拒 | 打开 Keynote、允许自动化 |
| `new` 报主题不可用 | V9 主题不存在 | 在 Keynote 里创建 V9 主题，或用 `--theme` 指定已有主题 |
| `new` 报找不到版式 | 主题里没有该版式 | 检查 `--layout` 名称 |
| `export` 报输出已存在 | 默认 PDF 已存在 | 加 `--force` 或 `--out` |
| `--no-save` 报错 | 文档未在 Keynote 打开 | 先打开文档，或去掉 `--no-save` |
| `pip install -e .` 被 PEP 668 拦 | externally-managed | 加 `--break-system-packages`，或直接用 `./bin/kn` |

## 十三、安全边界与已知限制

- **普通命令只改副本**：`<src> <dst>`，源文件只读；交付前保留原件。
- **文件层新增页**：仅研究保留（`--file-level`），交付走 `add-slide` 的 Keynote 路径。
- **`insert-image` 实验性**：实用请用 `insert-image-paste`。
- **删列 / 斑马纹**：结构改动较多，Keynote 打开确认后再交付。
- **版本**：仅在 macOS 26 / Keynote 15.4 验证；更旧 Keynote 未逐一验证。
- **性能**：100 页全量切换按 15–18ms/页估算约 2 秒；日常只设差异页。
- **菜单**：依赖安装时的 Python 绝对路径；首次使用需在脚本编辑器启用脚本菜单。
- **并发 / 撤销**：多人同时切换同一文件、逐页 `set skipped` 的撤销行为未做专项验证。
- **菜单栏方案**：Swift / Rust 菜单栏 app 受 macOS 26 `NSStatusItem` 位置随机影响，
  无法稳定交付；SwiftBar 备选方案已于 2026-10-21 废弃。CLI 与 DSH 插件仍是主入口。

## 十四、从零创建

```bash
./bin/kn new 新课程.key --title "标题" --body "第一条"
./bin/kn new 新课程.key --theme V9 --layout "标题项目符号" --force
```

默认主题「基本（白色）」、首屏版式「标题与项目符号」；创建由 Keynote 本体完成（主题与版式必须由
Keynote 生成），保存后自动关闭。默认不覆盖已有文件，`--force` 才覆盖。

## 十五、附录：常用组合

```bash
# 批量改标题并统一字色
for f in *.key; do
  ./bin/kn set-text "$f" "out/$f" 1 title "课程标题"
  ./bin/kn set-text-color "out/$f" "out2/$f" 1 title "#1D4ED8"
done

# 交付前体检
./bin/kn verify 交付.key && ./bin/kn audit 交付.key
```
