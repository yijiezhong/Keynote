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

The Swift shell is wired to the Rust CLI, but only commands currently implemented by the
Rust binary will work. In phase 1 that means `verify`; `outline` / `text` / `add-slide`
menu items will start working as those commands are ported.

## Known limitation

On macOS 26, `NSStatusItem` position is unstable (visible / collapsed / off-screen), and
hiding cannot be reliably reversed. This is a system behavior; the app is therefore
distributed as a source-level shell rather than a signed .app.
