# kn Guide

> Applies to macOS 26 / Keynote 15.4 (v14_5 schema, compatible with M6.5–M15.4)
>
> Status: Rust 3.0 — all 56 commands ported
>
> Principle: normal commands read the source and write a new file; commands that
> need Keynote's document model briefly launch Keynote

`kn` reads and edits Apple Keynote `.key` files. Most operations work directly on
the IWA / Snappy / protobuf container. `new`, `add-slide`, `change-layout`,
`insert-image`, `insert-image-paste`, `del-slide`, `dup-slide`, and the
Keynote-backed table fill/merge paths use Keynote itself.

---

## 1. Install and run

```bash
cd /path/to/project
./bin/kn help
./bin/kn outline deck.key
```

`./bin/kn` builds `target/release/kn` on first use when the binary is missing.

Manual build:

```bash
cargo build --release
./bin/kn verify deck.key
```

Requirements:

- Rust 1.80+ (edition 2021)
- `protoc` for Keynote protobuf code generation
- macOS Keynote for the Keynote-backed commands listed above

No external runtime schema package is required.

### Exit codes

| Code | Meaning |
|---|---|
| 0 | Success |
| 1 | Internal error / unsupported structure / write-back validation failed |
| 2 | Usage error, missing file, page out of range, invalid argument |

---

## 2. Five-minute quick start

```bash
# Inspect
./bin/kn outline deck.key
./bin/kn texts   deck.key
./bin/kn verify  deck.key

# Title and body
./bin/kn set-text in.key out.key 3 title "New title"
./bin/kn set-text in.key out.key 3 body  "New body"

# Text color and style
./bin/kn set-text-color in.key out.key 3 title "#FF00FF"
./bin/kn set-text-style in.key out.key 3 title --size 48 --bold

# Table cell
./bin/kn tables deck.key
./bin/kn set-cell in.key out.key 7031011 2 2 777

# Final check
./bin/kn audit out.key
```

Pages and table rows/columns are 1-based. Shape paths and `merge-cells` coordinates
are 0-based; see the relevant sections.

---

## 3. Command overview

| Area | Count | Commands |
|---|---:|---|
| Inspect | 17 | `list` `verify` `text` `texts` `types` `outline` `tables` `charts` `images` `links` `builds` `builds-list` `transitions` `styles` `colors` `audit` `check-notes` |
| Text / notes | 6 | `set-text` `set-text-color` `set-text-style` `note` `set-note` `edit` |
| Color / shape | 9 | `set-bg` `set-shape` `set-color` `set-style` `set-shape-geometry` `group` `ungroup` `del-shape` `dup-shape` |
| Table | 9 | `set-cell` `add-row` `del-row` `add-col` `del-col` `set-table-border` `set-table-fill` `set-table-size` `merge-cells` |
| Chart / image / link / animation | 8 | `set-chart` `set-image` `insert-image` `insert-image-paste` `set-link` `add-build` `set-build` `set-transition` |
| Page structure | 5 | `add-slide` `change-layout` `del-slide` `dup-slide` `skip` |
| Batch | 1 | `batch` |
| Create | 1 | `new` |

`info` is an alias of `verify`; `skip-many` is another entry point for `skip`.
`del-slide` accepts page expressions such as `3`, `1-3,7`, and `all`.

Write commands use `<src> <dst>` and do not overwrite the source. For commands that
take an ident, query it first with the matching inspection command.

---

## 4. Inspect

```bash
./bin/kn list      deck.key
./bin/kn verify    deck.key
./bin/kn text      deck.key [keyword]
./bin/kn texts     deck.key [keyword]
./bin/kn types     deck.key
./bin/kn outline   deck.key
./bin/kn tables    deck.key
./bin/kn charts    deck.key
./bin/kn images    deck.key
./bin/kn links     deck.key
./bin/kn builds    deck.key
./bin/kn builds-list deck.key
./bin/kn transitions deck.key
./bin/kn styles    deck.key [keyword]
./bin/kn colors    deck.key
./bin/kn audit     deck.key
./bin/kn check-notes deck.key [--require text]
```

Notes:

- `text` searches raw strings; `texts` reads `TSWP.StorageArchive` text objects.
- `colors` lists inline RGB color instances. Its index is the `set-color` selector.
- `check-notes` defaults to requiring `内容依据` and `设计依据`; `--require` sets one
  required substring.
- `audit` performs lightweight structure, identifier, page, and note checks.

---

## 5. Text and notes

```bash
# Whole title/body
./bin/kn set-text in.key out.key 3 title "New title"
./bin/kn set-text in.key out.key 3 body  "New body"

# Size, bold, italic, color; optionally a substring
./bin/kn set-text-style in.key out.key 3 title --size 48 --bold
./bin/kn set-text-style in.key out.key 3 title --italic --color "#C82505"
./bin/kn set-text-style in.key out.key 3 title --substr "keyword" --color "#FF0000"

# Whole-run text color
./bin/kn set-text-color in.key out.key 3 title "#FF00FF"
./bin/kn set-text-color in.key out.key 3 body  "#00A000"

# Notes: append or replace the whole note
./bin/kn note     in.key out.key 3 "appended note"
./bin/kn set-note in.key out.key 3 "replacement note"

# Raw replacement; use "-" to search all .iwa files
./bin/kn edit in.key out.key - "old" "new" --resize
```

- `title` / `body` select the page placeholders.
- `set-text-color` is whole-run only; it has no `--substr` / `--range` flags.
- `set-note` replaces the whole note. Use `note` when existing text must be kept.
- `edit` without `--resize` requires equal byte length; `--resize` recomputes nested
  protobuf lengths.

---

## 6. Color, styles, and shapes

```bash
./bin/kn set-bg    in.key out.key 3 "#EEEEEE"
./bin/kn set-shape in.key out.key 3 2 "#C82505"
./bin/kn set-color in.key out.key 1943 "#FF00FF" --alpha 0.6
./bin/kn set-style in.key out.key 7031016 font_size=48
./bin/kn set-style in.key out.key 7031016 bold=true

./bin/kn set-shape-geometry in.key out.key 3 2 x=100 y=200 w=600 h=120 angle=15
./bin/kn dup-shape in.key out.key 3 3 --offset 40,40
./bin/kn del-shape in.key out.key 3 2

./bin/kn group   in.key out.key 3 "2,3"
./bin/kn ungroup in.key out.key 3 2
```

- **Shape paths are 0-based**: `2` is the third top-level object; `3/1` is the
  second child of the fourth top-level group.
- `set-shape` only handles a solid fill. Gradient or image fills produce an error.
- `dup-shape` supports images and ordinary shapes; use table commands for tables and duplicate a page for placeholders.
- `set-style` changes one property at a time: `font_size=48`, `bold=true`,
  `italic=false`, `font_color=#FF0000`.
- Do not group title/body placeholders: grouping removes their placeholder identity.
  Confirm object paths with `outline` first.

---

## 7. Tables

Find the table ident first:

```bash
./bin/kn tables deck.key
```

### Cells and rows/columns

```bash
# set-cell / add-row / del-row / add-col / del-col: 1-based rows and columns
./bin/kn set-cell in.key out.key 7031011 2 2 777
./bin/kn add-row  in.key out.key 7031011 1 --at 2
./bin/kn del-row  in.key out.key 7031011 3
./bin/kn add-col  in.key out.key 7031011 --at 3
./bin/kn del-col  in.key out.key 7031011 3

# Row height / column width
./bin/kn set-table-size in.key out.key 7031011 --row-height 80 --col-width 180
```

`del-col` is experimental because column structure is coupled to several tables;
always reopen the output in Keynote.

### Borders, fills, merges

```bash
./bin/kn set-table-border in.key out.key 7031011 color="#EE2222" width=4
./bin/kn set-table-fill   in.key out.key 7031011 --body "#FF00FF" --no-band
./bin/kn set-table-fill   in.key out.key 7031011 --header "#EEF2F6" --header-rows 1
./bin/kn set-table-fill   in.key out.key 7031011 --band "#F2F2F7"

# merge-cells: 0-based rows/columns, top-left to bottom-right
./bin/kn merge-cells in.key out.key 7031011 0 0 1 1
```

- `set-table-fill` sets cell background colors through Keynote and needs Automation
  permission.
- `set-table-border` writes a table-style variation; reopen the output in Keynote.
- `merge-cells` coordinates are 0-based, unlike the 1-based `set-cell`.

---

## 8. Charts, images, links, animation, transitions

```bash
# Charts: 0-based rows/columns
./bin/kn charts    deck.key
./bin/kn set-chart in.key out.key 7031018 0 0 100

# Images
./bin/kn images deck.key
./bin/kn set-image in.key out.key 7031019 new.png
./bin/kn insert-image in.key out.key 3 new.png
./bin/kn insert-image-paste in.key out.key 3 new.png

# Links
./bin/kn links    deck.key
./bin/kn set-link in.key out.key 7031020 "https://example.com"

# Animation and transition
./bin/kn builds-list deck.key
./bin/kn add-build   in.key out.key 3 2
./bin/kn set-build   in.key out.key 7035366 duration=2.5 automatic=true
./bin/kn transitions deck.key
./bin/kn set-transition in.key out.key 3 apple:dissolve 1.5
```

- `insert-image` and `insert-image-paste` are performed by Keynote. The paste path
  needs Accessibility and Keynote Automation permissions.
- `set-image` replaces the `Data/` payload and updates natural/original dimensions.
- `add-build` uses a 0-based drawable path and requires an existing build template
  in the document.

---

## 9. Pages and creation

```bash
# Add a slide (generated by Keynote)
./bin/kn add-slide in.key out.key "Title & Bullets" \
  --title "Title" --body "Body" --after 3

# Change layout / delete / duplicate
./bin/kn change-layout in.key out.key 5 "Title & Bullets"
./bin/kn del-slide in.key out.key "3,7-9"
./bin/kn dup-slide in.key out.key 5

# Skip slides: 3 / 1-3,7 / all
./bin/kn skip in.key out.key "2,4"
./bin/kn skip in.key out.key "2,4" --off
```

`add-slide --replace N` replaces slide N with the target layout and migrates its
title/body. The layout name must exist in the current theme.

### Create from scratch

```bash
./bin/kn new new.key --title "Title" --body "First point"
./bin/kn new new.key --theme "Basic White" --layout "Title & Bullets"
./bin/kn new new.key --force
```

The defaults are the built-in Basic White theme and Title & Bullets layout. Keynote
creates the document and closes it after saving. Existing files are not overwritten
unless `--force` is given.

### Batch

```bash
./bin/kn batch in-dir out-dir set-text 1 title "Unified title"
./bin/kn batch in-dir out-dir set-text-style 1 title --color "#C82505"
./bin/kn batch in-dir out-dir skip 3
```

Each `.key` in the input directory runs the same subcommand; results keep their
file names in the output directory.

---

## 10. Common workflows

### Batch title and color

```bash
for f in *.key; do
  ./bin/kn set-text "$f" "out/$f" 1 title "Course title"
  ./bin/kn set-text-color "out/$f" "out2/$f" 1 title "#1D4ED8"
done
```

### Table cell

```bash
./bin/kn tables report.key
./bin/kn set-cell report.key out.key 7031011 2 2 777
```

### Replace an image

```bash
./bin/kn images gallery.key
./bin/kn set-image gallery.key out.key 7034303 new.jpg
```

### Add an entrance build

```bash
./bin/kn builds-list animation.key
./bin/kn add-build animation.key out.key 2 0 apple:bc-appear
```

### Table fill and merge

```bash
./bin/kn set-table-fill report.key out.key 7031011 \
  --header "#1F3864" --body "#FFFFFF" --header-rows 1
./bin/kn merge-cells out.key out2.key 7031011 0 0 1 1
```

> Find the table ident with `tables`; after writing, run `verify` and open the output in Keynote. Keep the intermediate copy.

---

## 11. Verification and troubleshooting

```bash
./bin/kn verify deck.key
./bin/kn audit deck.key
./bin/kn check-notes deck.key --require "内容依据"
```

A structurally valid file is not necessarily rendered correctly. Always open the
output in Keynote before delivery; for colors, images, and table styles, use
Keynote-exported PNG/PDF page counts and pixels as the final check.

| Symptom | Likely cause | Fix |
|---|---|---|
| Output will not open | Unknown structure touched | Run `verify`, revert to last good version, quit and reopen Keynote |
| `insert-image-paste` fails | Missing Accessibility/Automation permission | Grant permission to the terminal or DSH |
| `add-slide` / `new` fails | Keynote missing, permission denied, theme/layout not found | Open Keynote and check theme/layout names |
| Encrypted file error | Password-protected document | Remove the password in Keynote and save a copy |
| iWork '09 rejected | Legacy `index.apxl` format | Save as a current Keynote document |
| Output already exists | `new` does not overwrite by default | Use `--force` or another output path |

---

## 12. Safety boundaries

- **Source is read-only**: write commands use `<src> <dst>`; keep the original.
- **Keynote paths**: `new`, `add-slide`, `change-layout`, `insert-image`,
  `insert-image-paste`, `del-slide`, `dup-slide`, table fill/merge briefly open
  Keynote.
- **Experimental**: `del-col`, `set-table-border`, `set-table-fill` involve
  structural/style rewrites; verify in Keynote. `set-shape` only supports solid fills.
- **Version scope**: mainly validated on macOS 26 / Keynote 15.4; older versions were
  not tested exhaustively.
- **Concurrency/undo**: simultaneous edits and Keynote's per-slide undo behavior were
  not specifically validated.
- **Distribution**: the public repository does not include real `.key` samples or
  personal paths.

---

## 13. Format report

`docs/AppleKeynoteFormatReport.pdf` is a 24-page analysis of the `.key` format.
Example file names are generic.

## License

MIT. See [MIT License](../LICENSE).
