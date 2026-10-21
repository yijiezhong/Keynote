# kn-menu (Swift)

A small AppKit menu bar shell that calls `kn` for the current Keynote document.

## Build

```bash
cd swift/kn-menu
swiftc -O -o kn-menu main.swift -framework AppKit
```

## Run

```bash
KN_BIN=/path/to/Keynote/bin/kn ./kn-menu
```

`KN_BIN` defaults to `~/PARA/8.Code/Keynote/bin/kn`.

## Status

The Swift shell is wired to the Rust CLI (56 commands). The menu exposes read-only
inspections (`outline`, `text`, `tables`, `links`, `colors`, `styles`, `charts`,
`images`, `verify`), page operations (`add-slide`, `del-slide`, `dup-slide`), and
common writes (`set-text` title, `append-note`, `skip`). `全部命令帮助` shows the
full CLI help.

All file-changing menu actions write a new copy (`-del.key`, `-title.key`, …); the
source document is never overwritten by the menu shell.

## Known limitation

On macOS 26, `NSStatusItem` position is unstable (visible / collapsed / off-screen), and
hiding cannot be reliably reversed. This is a system behavior; the app is therefore
distributed as a source-level shell rather than a signed .app.
