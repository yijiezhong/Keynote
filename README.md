# kn

[简体中文](README.zh-CN.md) | English

A Rust CLI for reading and editing Apple Keynote `.key` files.

`kn` works directly on the IWA/zip container for most operations, and uses Keynote
itself only where the document model requires it (creating a deck, adding slides,
inserting images, and some table styling).

## Status

**Rust 3.0 — all 56 commands ported.**

- No external runtime schema package is required.
- The public repository contains the Rust source only.
- Write operations use `<src> <dst>` and leave the source document untouched.
- Keynote-dependent commands still require macOS Keynote and Automation permission.

See the full guide in [English](docs/kn-guide.en.md) or
[简体中文](docs/kn-guide.zh-CN.md).

## Quick start

```bash
./bin/kn help
./bin/kn outline deck.key
./bin/kn set-text in.key out.key 1 title "New title"
./bin/kn verify out.key
```

`./bin/kn` builds the release binary on first use when it is missing.

## Build

```bash
cargo build --release
./bin/kn list your.key
```

Requirements:

- Rust 1.80+ (edition 2021)
- `protoc` for `prost-build` code generation
- macOS Keynote for `new`, `add-slide`, `change-layout`, `insert-image`,
  `insert-image-paste`, `del-slide`, `dup-slide`, and the Keynote-backed table
  fill/merge paths

## Command groups

| Group | Count | Commands |
|---|---:|---|
| Inspect | 17 | `list` `verify` `text` `texts` `types` `outline` `tables` `charts` `images` `links` `builds` `builds-list` `transitions` `styles` `colors` `audit` `check-notes` |
| Text / notes | 6 | `set-text` `set-text-color` `set-text-style` `note` `set-note` `edit` |
| Color / shape | 9 | `set-bg` `set-shape` `set-color` `set-style` `set-shape-geometry` `group` `ungroup` `del-shape` `dup-shape` |
| Table | 9 | `set-cell` `add-row` `del-row` `add-col` `del-col` `set-table-border` `set-table-fill` `set-table-size` `merge-cells` |
| Chart / image / link / animation | 8 | `set-chart` `set-image` `insert-image` `insert-image-paste` `set-link` `add-build` `set-build` `set-transition` |
| Page structure | 5 | `add-slide` `change-layout` `del-slide` `dup-slide` `skip` |
| Batch | 1 | `batch` |
| Create | 1 | `new` |

`info` is an alias of `verify`; `skip-many` is an alias-form of `skip`.
`del-slide` accepts page expressions such as `3`, `1-3,7`, and `all`.

## Swift menu bar

`swift/kn-menu/` contains an AppKit menu bar shell that calls `bin/kn`. It exposes
read-only inspections, page operations, title/note/skip writes, and full CLI help.
See [swift/kn-menu/README.md](swift/kn-menu/README.md) for build and `KN_BIN` notes.

## Documentation

- [kn 使用与操作指南 — 简体中文 PDF](docs/kn-guide.zh-CN.pdf)
- [kn Guide — English PDF](docs/kn-guide.en.pdf)
- [kn Guide — English Markdown](docs/kn-guide.en.md)
- [kn 使用与操作指南 — 简体中文 Markdown](docs/kn-guide.zh-CN.md)
- [Format analysis report — 中文](docs/AppleKeynoteFormatReport.pdf)
- [Format analysis report — English](docs/AppleKeynoteFormatReport.en.pdf)
- [Format analysis source — 中文](docs/AppleKeynoteFormatReport.md)
- [Format analysis source — English](docs/AppleKeynoteFormatReport.en.md)

## Safety

Most commands read the source `.key` and write a new file. Keep the original until
the output has been opened and checked in Keynote. Structural table edits such as
`del-col` are experimental; verify the result in Keynote before delivery.

## License

MIT. See [LICENSE](LICENSE).
