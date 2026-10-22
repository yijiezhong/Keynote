# kn

[简体中文](README.zh-CN.md) | English

A Rust CLI for reading and editing Apple Keynote `.key` files.

Repository: <https://github.com/yijiezhong/Keynote>

`kn` works directly on the IWA/zip container for most operations, and uses Keynote
itself only where the document model requires it (creating a deck, adding slides,
inserting images, and some table styling).

## Status

**Rust 3.0 — all 57 commands ported.**

- No external runtime schema package is required.
- The public repository contains the Rust source only.
- Write operations use `<src> <dst>` and leave the source document untouched.
- Keynote-dependent commands still require macOS Keynote and Automation permission.

See the full guide: [English PDF](docs/kn-guide.en.pdf) · [简体中文 PDF](docs/kn-guide.zh-CN.pdf).

## Quick start

```bash
./bin/kn help
./bin/kn outline deck.key
./bin/kn set-text in.key out.key 1 title "New title"
./bin/kn verify out.key
```

`./bin/kn` builds the release binary on first use when it is missing.

## Homebrew

The public tap installs a prebuilt macOS binary:

```bash
brew tap yijiezhong/keynote_cli https://github.com/yijiezhong/keynote_cli.git
brew install yijiezhong/keynote_cli/kn
```

The tap repository is `keynote_cli` rather than `homebrew-*`, so the explicit
tap URL is required.

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

## Documentation

| Document | 简体中文 | English |
|---|---|---|
| kn Guide | [PDF](docs/kn-guide.zh-CN.pdf) · [Markdown](docs/kn-guide.zh-CN.md) | [PDF](docs/kn-guide.en.pdf) · [Markdown](docs/kn-guide.en.md) |
| Format analysis report | [PDF](docs/AppleKeynoteFormatReport.pdf) · [Markdown](docs/AppleKeynoteFormatReport.md) | [PDF](docs/AppleKeynoteFormatReport.en.pdf) · [Markdown](docs/AppleKeynoteFormatReport.en.md) |
| Handover | [中文](HANDOVER.md) | — |

## Safety

Most commands read the source `.key` and write a new file. Keep the original until
the output has been opened and checked in Keynote. Structural table edits such as
`del-col` are experimental; verify the result in Keynote before delivery.

## License

MIT. See [MIT License](LICENSE).
