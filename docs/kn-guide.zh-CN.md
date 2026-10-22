# kn 使用与操作指南

> 适用：macOS 26 / Keynote 15.4（v14_5 schema，兼容 M6.5–M15.4）
>
> 状态：Rust 3.0，57 条命令已完成
>
> 原则：普通命令只读源文件、写新文件；需要 Keynote 文档模型的命令会短暂调用 Keynote

`kn` 是 Keynote `.key` 的读写工具。大部分操作直接处理 IWA / Snappy / protobuf 数据；
`new`、`add-slide`、`change-layout`、`insert-image`、`insert-image-paste`、
`del-slide`、`dup-slide` 以及表格底色/合并等操作会调用 Keynote 本体。

---

## 一、安装与运行

```bash
cd 你的项目目录
./bin/kn help
./bin/kn outline 文件.key
```

`./bin/kn` 是薄入口：`target/release/kn` 不存在时，会先执行 `cargo build --release`。

手动构建：

```bash
cargo build --release
./bin/kn verify 文件.key
```

依赖：

- Rust 1.80+（edition 2021）
- `protoc`（生成 Keynote protobuf 类型）
- macOS Keynote（仅 Keynote 路径命令需要）

不依赖外部运行时 schema 包。

### 退出码

| 码 | 含义 |
|---|---|
| 0 | 成功 |
| 1 | 内部错误 / 结构不支持 / 写入校验失败 |
| 2 | 用法错误、文件缺失、页码越界、参数非法 |

---

## 二、5 分钟上手

```bash
# 看结构、文字、页面
./bin/kn outline 文件.key
./bin/kn texts   文件.key
./bin/kn verify  文件.key

# 改标题、正文
./bin/kn set-text 输入.key 输出.key 3 title "新标题"
./bin/kn set-text 输入.key 输出.key 3 body  "新正文"

# 改字色 / 字号
./bin/kn set-text-color 输入.key 输出.key 3 title "#FF00FF"
./bin/kn set-text-style 输入.key 输出.key 3 title --size 48 --bold

# 改表格数字
./bin/kn tables 文件.key
./bin/kn set-cell 输入.key 输出.key 7031011 2 2 777

# 交付前体检
./bin/kn audit 输出.key
```

页码、表格行列均从 1 开始；形状路径与 `merge-cells` 从 0 开始，详见各节。

---

## 三、命令总览

| 域 | 数量 | 命令 |
|---|---:|---|
| 查看与分析 | 17 | `list` `verify` `text` `texts` `types` `outline` `tables` `charts` `images` `links` `builds` `builds-list` `transitions` `styles` `colors` `audit` `check-notes` |
| 文字 / 备注 | 6 | `set-text` `set-text-color` `set-text-style` `note` `set-note` `edit` |
| 颜色 / 形状 | 9 | `set-bg` `set-shape` `set-color` `set-style` `set-shape-geometry` `group` `ungroup` `del-shape` `dup-shape` |
| 表格 | 9 | `set-cell` `add-row` `del-row` `add-col` `del-col` `set-table-border` `set-table-fill` `set-table-size` `merge-cells` |
| 图表 / 图片 / 链接 / 动画 | 9 | `set-chart` `set-image` `image-info` `insert-image` `insert-image-paste` `set-link` `add-build` `set-build` `set-transition` |
| 页面结构 | 5 | `add-slide` `change-layout` `del-slide` `dup-slide` `skip` |
| 批量 | 1 | `batch` |
| 从零创建 | 1 | `new` |

补充说明：

- `info` 是 `verify` 的别名；`skip-many` 是 `skip` 的表达式入口；
  `note` 是 `append-note` 的别名；`builds-list` 是 `builds` 的别名。
- 写命令几乎都是 `<src> <dst>`，不会覆盖源文件。
- 使用 ident 的命令，先用对应查看命令查 ident，不要猜。

---

## 四、查看与分析

```bash
./bin/kn list     文件.key
./bin/kn verify   文件.key
./bin/kn text     文件.key [关键字]
./bin/kn texts    文件.key [关键字]
./bin/kn types    文件.key
./bin/kn outline  文件.key
./bin/kn tables   文件.key
./bin/kn charts   文件.key
./bin/kn images   文件.key
./bin/kn links    文件.key
./bin/kn builds   文件.key
./bin/kn builds-list 文件.key
./bin/kn transitions 文件.key
./bin/kn styles   文件.key [关键字]
./bin/kn colors   文件.key
./bin/kn audit    文件.key
./bin/kn check-notes 文件.key [--require 文本]
```

说明：

- `text` 是字节层字符串搜索；`texts` 按 `TSWP.StorageArchive` 列真实文字。
- `colors` 列出内联 RGB 颜色实例，序号可直接给 `set-color`。
- `check-notes` 默认检查备注中是否含「内容依据」「设计依据」，也可用
  `--require` 指定一个必需文本。
- `audit` 做结构、identifier 重复、页面与备注的轻量体检。

---

## 五、文字与备注

```bash
# 整段重设标题/正文
./bin/kn set-text 输入.key 输出.key 3 title "新标题"
./bin/kn set-text 输入.key 输出.key 3 body  "新正文"

# 字号、粗体、斜体、颜色；可只作用于某个子串
./bin/kn set-text-style 输入.key 输出.key 3 title --size 48 --bold
./bin/kn set-text-style 输入.key 输出.key 3 title --italic --color "#C82505"
./bin/kn set-text-style 输入.key 输出.key 3 title --substr "关键词" --color "#FF0000"

# 页级精确改字色（整段）
./bin/kn set-text-color 输入.key 输出.key 3 title "#FF00FF"
./bin/kn set-text-color 输入.key 输出.key 3 body  "#00A000"

# 备注：追加 / 整段替换
./bin/kn note     输入.key 输出.key 3 "追加的备注"
./bin/kn set-note 输入.key 输出.key 3 "新的整段备注"

# 字节级替换：iwa 名写 - 表示全部 .iwa
./bin/kn edit 输入.key 输出.key - "旧串" "新串" --resize
```

- `title` / `body` 是页面占位符选择器。
- `set-text-color` 只支持整段，不提供 `--substr` / `--range`。
- `set-note` 是整段替换；需要保留原文时用 `note` 追加。
- `edit` 不加 `--resize` 时只允许等长替换；`--resize` 会重算 protobuf 各层长度。

---

## 六、颜色、样式与形状

```bash
./bin/kn set-bg   输入.key 输出.key 3 "#EEEEEE"
./bin/kn set-shape 输入.key 输出.key 3 2 "#C82505"
./bin/kn set-color 输入.key 输出.key 1943 "#FF00FF" --alpha 0.6
./bin/kn set-style 输入.key 输出.key 7031016 font_size=48
./bin/kn set-style 输入.key 输出.key 7031016 bold=true

./bin/kn set-shape-geometry 输入.key 输出.key 3 2 x=100 y=200 w=600 h=120 angle=15
./bin/kn dup-shape 输入.key 输出.key 3 3 --offset 40,40
./bin/kn del-shape 输入.key 输出.key 3 2

./bin/kn group   输入.key 输出.key 3 "2,3"
./bin/kn ungroup 输入.key 输出.key 3 2
```

- **形状路径从 0 开始**：`2` 表示第 3 个顶层对象；`3/1` 表示第 4 个顶层群组里的
  第 2 个子对象。
- `set-shape` 只处理纯色填充；渐变/图片填充会明确报错。
- `dup-shape` 支持图片与普通形状；表格对象请用表格命令，占位符请复制整页。
- `set-style` 一次只改一个属性：`font_size=48`、`bold=true`、`italic=false`、
  `font_color=#FF0000`。
- 打组不要包含标题/正文占位符，否则占位符身份会丢失；先用 `outline` 确认对象。

---

## 七、表格

先用 `tables` 找到表格 ident：

```bash
./bin/kn tables 文件.key
```

### 单元格与行列

```bash
# set-cell / add-row / del-row / add-col / del-col：行列从 1 开始
./bin/kn set-cell 输入.key 输出.key 7031011 2 2 777
./bin/kn add-row  输入.key 输出.key 7031011 1 --at 2
./bin/kn del-row  输入.key 输出.key 7031011 3
./bin/kn add-col  输入.key 输出.key 7031011 --at 3
./bin/kn del-col  输入.key 输出.key 7031011 3

# 行高 / 列宽
./bin/kn set-table-size 输入.key 输出.key 7031011 --row-height 80 --col-width 180
```

`del-col` 结构耦合较多，属于实验性能力；改完务必用 Keynote 打开确认。

### 边框、底色、合并

```bash
./bin/kn set-table-border 输入.key 输出.key 7031011 color="#EE2222" width=4
./bin/kn set-table-fill   输入.key 输出.key 7031011 --body "#FF00FF" --no-band
./bin/kn set-table-fill   输入.key 输出.key 7031011 --header "#EEF2F6" --header-rows 1
./bin/kn set-table-fill   输入.key 输出.key 7031011 --band "#F2F2F7"

# merge-cells：行列从 0 开始，左上到右下
./bin/kn merge-cells 输入.key 输出.key 7031011 0 0 1 1
```

- `set-table-fill` 通过 Keynote 设置单元格背景，需要 Keynote 与自动化权限。
- `set-table-border` 写入表格样式变体；Keynote 打开后复核边框。
- `merge-cells` 的行列从 0 开始，和 `set-cell` 的 1 基不同。

---

## 八、图表 / 图片 / 链接 / 动画 / 转场

```bash
# 图表：行列从 0 开始
./bin/kn charts    文件.key
./bin/kn set-chart 输入.key 输出.key 7031018 0 0 100

# 图片：列出、查看、替换、插入
./bin/kn images 文件.key
./bin/kn image-info 文件.key 7034153
./bin/kn set-image 输入.key 输出.key 7031019 新图片.png
./bin/kn insert-image 输入.key 输出.key 3 新图片.png
./bin/kn insert-image-paste 输入.key 输出.key 3 新图片.png

# 超链接
./bin/kn links    文件.key
./bin/kn set-link 输入.key 输出.key 7031020 "https://example.com"

# 动画 / 转场
./bin/kn builds-list 文件.key
./bin/kn add-build   输入.key 输出.key 3 2
./bin/kn set-build   输入.key 输出.key 7035366 duration=2.5 automatic=true
./bin/kn transitions 文件.key
./bin/kn set-transition 输入.key 输出.key 3 apple:dissolve 1.5
```

- `insert-image` 与 `insert-image-paste` 都由 Keynote 完成；后者走剪贴板与 Command-V，
  需要「辅助功能」和「自动化 Keynote」权限。
- `set-image` 替换已有图片的 `Data/` 素材，并同步 natural/original 尺寸。
- `image-info` 显示单张图片的 data 字节数与编码尺寸，换图前先用它确认目标。
- `add-build` 的对象路径从 0 开始，需要文件里已有可复制的构建模板。

---

## 九、页面结构与新建

```bash
# 新增页（由 Keynote 生成）
./bin/kn add-slide 输入.key 输出.key "标题与项目符号" \
  --title "标题" --body "正文" --after 3

# 换版式 / 删页 / 复制页
./bin/kn change-layout 输入.key 输出.key 5 "标题与项目符号"
./bin/kn del-slide 输入.key 输出.key "3,7-9"
./bin/kn dup-slide 输入.key 输出.key 5

# 跳过放映：支持 3 / 1-3,7 / all
./bin/kn skip 输入.key 输出.key "2,4"
./bin/kn skip 输入.key 输出.key "2,4" --off
```

`add-slide` 的 `--replace N` 会把第 N 页换成目标版式并迁移标题/正文。
版式名必须存在于当前主题；V9 等自定义主题请使用主题里实际的版式名。

### 从零创建

```bash
./bin/kn new 新文件.key --title "标题" --body "第一条"
./bin/kn new 新文件.key --theme "Basic White" --layout "Title & Bullets"
./bin/kn new 新文件.key --force
```

默认主题为「基本（白色）」、首屏版式为「标题与项目符号」。创建由 Keynote 完成，
保存后自动关闭；默认不覆盖已有文件。

### 批量

```bash
./bin/kn batch 输入目录 输出目录 set-text 1 title "统一标题"
./bin/kn batch 输入目录 输出目录 set-text-style 1 title --color "#C82505"
./bin/kn batch 输入目录 输出目录 skip 3
```

对目录下每个 `.key` 执行同一子命令，结果写入输出目录并保持文件名。

---

## 十、典型场景

### 1. 批量改标题并统一字色

```bash
for f in *.key; do
  ./bin/kn set-text "$f" "out/$f" 1 title "课程标题"
  ./bin/kn set-text-color "out/$f" "out2/$f" 1 title "#1D4ED8"
done
```

### 2. 改表格数字

```bash
./bin/kn tables 报表.key
./bin/kn set-cell 报表.key out.key 7031011 2 2 777
```

### 3. 换图

```bash
./bin/kn images 图册.key
./bin/kn set-image 图册.key out.key 7034303 新图.jpg
```

### 4. 给对象加入场动画

```bash
./bin/kn builds-list 动画.key
./bin/kn add-build 动画.key out.key 2 0 apple:bc-appear
```

### 5. 表格底色与合并

```bash
./bin/kn set-table-fill report.key out.key 7031011 \
  --header "#1F3864" --body "#FFFFFF" --header-rows 1
./bin/kn merge-cells out.key out2.key 7031011 0 0 1 1
```

> 先用 `tables` 查表格 ident；写入后跑 `verify`，再在 Keynote 中打开输出确认，并保留中间副本。

---

## 十一、验证与排错

```bash
./bin/kn verify 文件.key
./bin/kn audit  文件.key
./bin/kn check-notes 文件.key --require "内容依据"
```

结构正确不等于渲染正确。交付前必须用 Keynote 打开输出；涉及颜色、图片、表格样式时，
以 Keynote 导出的 PNG / PDF 页数与像素为准。

| 现象 | 可能原因 | 处理 |
|---|---|---|
| 打不开输出 | 结构被未知字段影响 | 用 `verify` 检查，回退上一版，退出 Keynote 后重开 |
| `insert-image-paste` 失败 | 缺辅助功能 / 自动化权限 | 在系统设置给终端或 DSH 授权 |
| `add-slide` / `new` 失败 | Keynote 未安装、权限被拒、主题/版式名不存在 | 打开 Keynote，核对主题与版式名 |
| 加密文件报错 | 文档带密码 | 在 Keynote 里移除密码后另存 |
| iWork '09 被拒 | 旧格式 `index.apxl` | 在 Keynote 中另存为当前格式 |
| 输出已存在 | `new` 默认不覆盖 | 加 `--force` 或换输出路径 |

---

## 十二、安全边界

- **源文件只读**：写命令使用 `<src> <dst>`；交付前保留原件。
- **Keynote 路径**：`new`、`add-slide`、`change-layout`、`insert-image`、
  `insert-image-paste`、`del-slide`、`dup-slide`、表格底色/合并会短暂打开 Keynote。
- **实验性**：`del-col`、`set-table-border`、`set-table-fill` 涉及结构/样式改写，
  必须用 Keynote 复核；`set-shape` 只支持纯色填充。
- **版本**：主要在 macOS 26 / Keynote 15.4 验证；更旧版本未逐一验证。
- **并发 / 撤销**：多人同时改写同一文件、Keynote 的逐页撤销行为未做专项验证。
- **发布**：公开仓库不包含真实 `.key` 样本或个人路径。

---

## 十三、附：格式分析报告

`docs/AppleKeynoteFormatReport.pdf` 是 24 页的 `.key` 文件格式分析报告。报告中的示例
文件名为通用样例名。

## 许可证

MIT。详见仓库根目录 [MIT 许可证](../LICENSE)。
