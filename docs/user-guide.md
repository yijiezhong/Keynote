# Keynote .key 编辑工具 —— 使用手册
> **实现状态（Rust 阶段 1）**：本文是 kn 的目标命令参考；Rust 版当前只实现 `list` / `verify` / `info`，其余命令正在迁移。


**工具**：Rust `kn`（阶段 1 只读；目标 56 条命令）+ `keynote_add_page.py`（新增页流程）

**适用**：macOS 26 / Keynote 15.4 生成的 `.key`（v14_5 schema 兼容 M6.5–M15.4）

**原则**：普通命令只改副本、原件不动；`add-slide` / `new` / `insert-image-paste` 会调用 Keynote 本体

---

## 一、快速上手

### 1. 看一份 deck 里有什么

```bash
./bin/kn outline 你的文件.key      # 逐页标题/正文/备注
./bin/kn types   你的文件.key      # 对象类型统计
./bin/kn verify  你的文件.key      # 结构完整性
```

### 2. 改一页的标题/正文

```bash
./bin/kn set-text 你的文件.key 输出.key 3 title "新的标题"
./bin/kn set-text 输出.key 输出2.key 3 body "新的正文"
```

页码从 1 开始。改完用 Keynote 打开 `输出2.key` 确认。

### 3. 改文字颜色（页级精确）

```bash
# 整段改成品红（新建 CharacterStyle，不影响其他页）
./bin/kn set-text-color 你的文件.key 输出.key 3 title "#FF00FF"

# 只改其中几个字
./bin/kn set-text-color 输出.key 输出2.key 3 title "#FF0000" --substr "关键词"
```

---

## 二、这个工具能做什么、不能做什么

**能改**（都用 Keynote 打开 + 导出 PNG + 像素比对验证过）：

- 文字内容、文字颜色（整页/局部/批量）、字号/粗体/斜体、备注（追加/改写）
- 表格：改数字、增删行列、**边框**、**底色/斑马纹**、**合并单元格**、行高列宽
- 图表数值、图片素材、**插入新图片（剪贴板路线）**、超链接、构建动画、页间转场
- 删页、删形状、**跨页复制形状**（普通形状与文本框两条路径都通）、打组拆组、形状几何
- 新增页、换版式（走 Keynote 本身完成）

**不能做**（已系统验证过边界）：

- **文件层新增页**：Keynote 的文档模型不认，打开后该页显示为黑块
- **文件层插入图片**：直写 Data 文件会被 Keynote 拒开（11 个假设已排除），只能走剪贴板 + Command-V
- **表格删列**：结构耦合多，命令可跑但 Keynote 偶尔拒开，改稿前先备份

**判断依据**：以上每一条都用「Keynote 打开 + 导出 PNG + 逐像素比对」验证过，
不是推测。做不到的部分会明确报错，不会静默产生坏文件。

## 三、命令速查

### 查看与分析

| 命令 | 用途 |
|---|---|
| `list <key>` | 列出 Zip 条目 |
| `text <key> [关键字]` | 纯字节层搜索文字（不依赖 schema） |
| `types <key>` | 按类型统计对象 |
| `texts <key> [关键字]` | 列出真实文字及所在对象 |
| `outline <key>` | 逐页导出标题/正文/备注 |
| `tables <key>` | 列出表格（ident/行列/表名） |
| `charts <key>` | 列出图表与数值网格 |
| `images <key>` | 列出图片（ident/位置/尺寸/素材文件） |
| `links <key>` | 列出超链接 |
| `builds-list <key>` | 列出构建动画 |
| `transitions <key>` | 列出页间转场 |
| `styles <key> [关键字]` | 列出样式库 |
| `colors <key> [关键字] [--all]` | 列出颜色实例 |
| `verify <key>` | 校验结构完整性 |
| `audit <key>` | 综合体检（备注/跳过/动画/表格） |
| `check-notes <key> [字段...]` | 备注合规体检 |

### 文字与备注

| 命令 | 用途 |
|---|---|
| `set-text <src> <dst> <页> <title\|body> <文字>` | 整段重设标题/正文 |
| `note <src> <dst> <页> <文字>` | 备注末尾追加 |
| `set-note <src> <dst> <页> <文字> [--replace 旧串]` | 改写备注（整段/子串） |
| `edit <src> <dst> <iwa\|-> <旧串> <新串> [--resize]` | 字节级替换（等长/变长） |

### 颜色

| 命令 | 用途 |
|---|---|
| `set-text-color <src> <dst> <页> <title\|body> <#RRGGBB> [--substr X] [--range a:b] [--para N]` | 页级精确改字色 |
| `set-bg <src> <dst> <页> <#RRGGBB>` | 改页纯色背景 |
| `set-shape <src> <dst> <页> <形状路径> <#RRGGBB>` | 改形状填充 |
| `set-color <src> <dst> <序号> <#RRGGBB> [--alpha a]` | 改某个颜色实例 |
| `set-style <src> <dst> <样式ident> <属性=值>` | 批量改样式（影响所有引用它的文本） |

### 表格

| 命令 | 用途 |
|---|---|
| `set-cell <src> <dst> <表格ident> <行> <列> <值>` | 改数字格 |
| `add-row <src> <dst> <表格ident> <源行> [--at N]` | 插入一行（复制源行） |
| `del-row <src> <dst> <表格ident> <行号>` | 删除一行 |
| `add-col <src> <dst> <表格ident> [列位置]` | 加一列（空文本格） |
| `del-col <src> <dst> <表格ident> <列号>` | 删除一列（实验性） |
| `set-table-border <src> <dst> <表格ident> [color=#RRGGBB width=4]` | 改边框颜色/粗细 |
| `set-table-fill <src> <dst> <表格ident> [--body #RGB] [--header #RGB] [--band #RGB\|--no-band] [--header-rows N]` | 改底色 / 斑马纹 / 表头行数 |
| `merge-cells <src> <dst> <表格ident> <r1> <c1> <r2> <c2>` | 合并单元格（文字自动串接） |
| `set-table-size <src> <dst> <表格ident> [row-height=50] [col-width=300]` | 行高 / 列宽 |

### 图表 / 图片 / 链接 / 动画

| 命令 | 用途 |
|---|---|
| `set-chart <src> <dst> <图表ident> <行> <列> <值>` | 改图表数值 |
| `set-image <src> <dst> <图片ident> <新图片>` | 替换图片素材 |
| `set-link <src> <dst> <链接ident> <URL>` | 改超链接 |
| `add-build <src> <dst> <页> <对象路径> [effect]` | 加构建动画 |
| `set-build <src> <dst> <块ident> [duration=2.5] [automatic=true]` | 改动画参数 |
| `set-transition <src> <dst> <页> <effect> [duration]` | 改页间转场 |

### 结构编辑

| 命令 | 用途 |
|---|---|
| `del-slide <src> <dst> <页码表达式>` | 删页（`3` / `1-3,7` / `all`） |
| `dup-slide <src> <dst> <页>` | 复制页 |
| `del-shape <src> <dst> <页> <形状路径>` | 删形状（级联清理独占子对象） |
| `dup-shape <src> <dst> <页> <形状路径> [--offset dx,dy]` | 复制形状 |
| `group <src> <dst> <页> <路径1,路径2>` | 打组 |
| `ungroup <src> <dst> <页> <组路径>` | 拆组 |
| `skip <src> <dst> <页码表达式> [--off]` | 跳过/取消跳过放映 |

### 从零创建

```bash
kn new 新课程.key --title "标题" --body "第一条"
kn new 新课程.key --force          # 覆盖已有文件
```

默认主题「基本（白色）」、首屏版式「标题与项目符号」；`--theme` / `--layout` 可覆盖（例如 `--theme V9 --layout 标题项目符号`）。
创建由 Keynote 本体完成，保存后自动关闭。

### 新增页（走 Keynote）

`kn add-slide` 默认就把这一步交给 Keynote 本体（AppleScript），渲染 100% 正确，
之后仍可继续用 kn 的全部文件层命令编辑新页：

```bash
kn add-slide 你的文件.key 输出.key "标题项目符号" \
  --title "标题" --body "正文" [--after N] [--replace N]
```

- `--replace N`：换版式（新增目标版式的页 → 迁移文本 → 删掉原页）
- `--file-level`：纯文件层实现，仅供研究（页可见但内容不渲染，原因见
  页可见但内容不渲染），不要用于交付
- 也可以直接调用底层脚本：`python3 keynote_add_page.py …`（参数相同）


---

## 四、五个典型场景

### 场景 1：批量改某几页的标题并改色

```bash
./bin/kn set-text     模板.key out1.key 3 title "第一部分"
./bin/kn set-text-color out1.key out2.key 3 title "#C82505"
```

### 场景 2：改表格里的一个数字

```bash
./bin/kn tables 报表.key          # 找到表格 ident 和行列数
./bin/kn set-cell 报表.key out.key 7031011 2 2 777
```

### 场景 3：换一张图

```bash
./bin/kn images 图册.key          # 找到图片 ident
./bin/kn set-image 图册.key out.key 7034303 新图.jpg
```

### 场景 4：把三个形状打成一组

```bash
./bin/kn group 模板.key out.key 1 "0,1,2"
./bin/kn ungroup out.key out2.key 1 "0"
```

> **⚠ 不要把标题/正文占位符打进组**：实测把标题文本框和别的形状一起 group 之后，
> 标题会失去占位符身份，样式（包括改过的文字色）都不再生效。
> 打组前先用 `outline` 确认哪几个对象是普通形状。

### 场景 5：给某页加一个进入动画

```bash
./bin/kn builds-list 动画.key     # 看现有动画作参考
./bin/kn add-build 动画.key out.key 2 0 apple:bc-appear
```

---

### 场景 6：给表格加底色 / 斑马纹

```bash
./bin/kn tables report.key                          # 先拿表格 ident 和行列数
./bin/kn set-table-fill report.key out.key 7031011 \
    --header "#1F3864" --body "#FFFFFF" --header-rows 1
```

- 只给 `--body`、不给 `--band` 时，命令会**自动关闭斑马纹**——否则
  `banded_fill` 的优先级高于单元格底色，你看到的会是「底色 × 条纹」交替。
- 想保留条纹就同时给 `--band "#F2F2F7"`；想彻底关掉用 `--no-band`。
- 表格原本没有表头行时，`--header` 不会生效，要配 `--header-rows 1`。

### 场景 7：合并单元格

```bash
./bin/kn merge-cells report.key out.key 7031011 0 0 1 1
```

参数是「左上角行、列，右下角行、列」，**行列都从 0 开始**。
合并后原区域四格的文字会自动串成一行（列间 `\t`、行间 `\n`），
例如 `H1\tH2\n2\t4`——这是 Keynote 自己的行为，工具照抄。

> 想核对效果：把同一份 deck 在 Keynote 里手工合并一次，导出 PNG 与我们生成的对比，
> 两者应当逐像素相同（可用逐像素比对验证）。

## 五、常见问题

**Q：改完 Keynote 打不开怎么办？**

先确认是不是 Keynote 本身进入了异常状态：**退出 Keynote 再重开**，然后重新打开文件。
如果仍然打不开，说明改动触碰了未知结构，用 `verify` 看段解析是否通过，并回退到上一个正常版本。

**Q：为什么 `set-text-color` 有时看不到效果？**

`--substr` 找不到匹配时不会报错但也不生效。先用 `./bin/kn texts 文件.key` 确认文字内容。

**Q：形状路径 `2` 和 `3/1` 是什么意思？**

顶层对象从 0 开始编号，`3/1` 表示第 3 个对象（群组）里的第 1 个子对象。

**Q：改了表格的列，Keynote 打不开？**

`del-col` 目前是实验性的（列数与计算引擎、列宽重分配等多处结构耦合），建议改用「删除整列内容 + 隐藏」的方式绕过。

**Q：设了表格底色，为什么还能看到条纹？**

`banded_fill`（斑马纹）的优先级高于 `body_cell_style`。只给 `--body` 时命令会自动关掉
斑马纹；如果同时给了 `--band`，就会保留条纹并以它为准。

**Q：合并单元格后，被合并格子的文字去哪了？**

会按 Keynote 自己的规则串接进左上角：列间 `\t`、行间 `\n`。
所以 `H1/H2/2/4` 合并后左上角显示两行 `H1H2` 与 `24`。

**Q：能不能批量处理多个文件？**

可以，命令都是无状态的，用 shell 循环即可：

```bash
for f in *.key; do
  ./bin/kn set-text "$f" "out/${f}" 1 title "统一标题"
done
```

---

## 六、安全边界

| 项 | 说明 |
|---|---|
| **原件** | 所有命令都是 `<src> <dst>` 模式，只读源文件、写新文件 |
| **验证** | 每次改完用 Keynote 打开确认，接受 Keynote 自己的渲染结果 |
| **备份** | 交付前保留原件，改稿链式操作时每步都留中间文件 |
| **不可行** | 文件层新增页（Keynote 文档模型不认）、文件层插入图片（要加真实 Data 文件才拒开，已排除 11 个假设） |
| **实验性** | 表格删列（结构耦合多，Keynote 偶尔拒开）、文件层 `insert-image`（实验性，实用走 `insert-image-paste`） |
| **需要权限** | `insert-image-paste` 需要「辅助功能」+「自动化 Keynote」 |

---

## 七、当前能力总结

| 类别 | 能力 | 验证状态 |
|---|---|---|
| 文字 | 改内容、改颜色（整段/局部/批量）、字号/粗体/斜体 | ✅ e2e 品红 434 |
| 备注 | 追加、改写 | ✅ Keynote 读回 |
| 表格 | 改值、增删行列、边框、底色/斑马纹、合并、行高列宽 | ✅ e2e 边框 1010 / 底色 75116 / 合并与手工样本逐像素一致 |
| 图表 | 数值读写 | ✅ 渲染差异 9540 采样点 |
| 图片 | 列出、替换、插入（剪贴板路线） | ✅ e2e 绿色 7500 |
| 超链接 | 列出、改写 | ✅ 写回确认 |
| 动画 | 列出、新增、改参数 | ⚠ 结构正确，放映未逐一手验 |
| 结构 | 删页、删形状、跨页复制形状、打组拆组、几何 | ✅ 两条复制路径均验证通过 |
| 新增页 | 按版式新增、换版式 | ✅ 走 Keynote |
| 从零创建 | `kn new`，默认苹果官方主题 / 标题与项目符号 | ✅ 回归 + e2e |

---

**工具位置**：`./bin/kn`（薄入口 `python/keynote_iwa.py` + `python/kn/` 包）
**测试**：需自备 `samples/*.key`；开发测试脚本不在公开仓库中。
