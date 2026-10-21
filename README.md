# kn

A Rust CLI for reading and editing Apple Keynote `.key` files.

## Status

**Phase 1.5 — read + basic text write.** The Rust implementation currently provides:

```bash
kn list   <deck.key>     # list zip entries
kn verify <deck.key>     # IWA / segments / objects / slides statistics
kn info   <deck.key>     # alias of verify
kn text   <deck.key> [keyword]
kn texts  <deck.key> [keyword]
kn types  <deck.key>
kn outline <deck.key>
kn set-text   <src.key> <dst.key> <page> <title|body> <text>
kn tables <deck.key>
kn set-table-size <src> <dst> <ident> [--row-height H] [--col-width W]
kn set-cell <src.key> <dst.key> <table-ident> <row> <col> <value>
kn skip <src.key> <dst.key> <page|1-3,7> [--off]
kn skip-many <src.key> <dst.key> <expr> [--off]
kn set-note   <src.key> <dst.key> <page> <text>
kn append-note <src.key> <dst.key> <page> <text>
kn insert-image-paste <src> <dst> <page> <image>
kn change-layout <src.key> <dst.key> <page> <layout>
kn add-slide <src.key> <dst.key> <layout> [--title T] [--body B] [--after N] [--replace N]
kn new    <new.key> [--theme T] [--layout L] [--title T] [--body B]
kn help
```

Write and edit commands (`outline`, `set-text`, `set-table-fill`, `new`, `add-slide`, ...)
are being ported from the previous Python implementation. The public repository no longer
ships the Python version; it is kept privately only as a comparison oracle.

## Build

```bash
cargo build --release
./bin/kn list your.key
```

`./bin/kn` builds the release binary on first use. Build requirements:

- Rust 1.80+ (edition 2021)
- `protoc` (used by `prost-build` to generate the Keynote protobuf modules)
- A `.key` file; a macOS Keynote installation is only needed once write/edit commands land.

## Roadmap

1. Read-only CLI: `list`, `verify`, `info`, `text`, `texts`, `types`, `outline`.
2. File-level write: ident index, segment re-pack, atomic save; `set-text` / `set-note` / `append-note` / `set-cell` / `skip` / `skip-many` (current).
3. `new`, `add-slide`, `change-layout` and `insert-image-paste` (Keynote via AppleScript) are ported and usable.
3. Table / chart / image / shape / slide commands.
4. `new` / `add-slide` / `insert-image-paste` (Keynote via AppleScript behind the Rust CLI).
5. Swift menu bar shell (`swift/kn-menu/`) wired to the Rust CLI.

## Swift menu bar

`swift/kn-menu/` contains a Swift/AppKit menu bar shell. It is built with `swiftc` and calls
`bin/kn`; set `KN_BIN` if the binary lives elsewhere. macOS 26 has a known `NSStatusItem`
position issue — see `swift/kn-menu/README.md`.

## Documentation

The command reference in `docs/` describes the target CLI. It is kept as the migration
specification; the Rust phase-1 binary implements only the read-only subset above.

## License

MIT. See [LICENSE](LICENSE).
