# swiss

[![release](https://img.shields.io/github/v/release/not6741lol/swiss?label=release&color=blue)](https://github.com/not6741lol/swiss/releases)
[![Made with Rust](https://img.shields.io/badge/Made%20with-Rust-orange?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![license](https://img.shields.io/github/license/not6741lol/swiss?color=blue)](LICENSE)

> [!NOTE]
> This project was made by **not6741** with the help of **DeepSeek**, written in **Rust**.
> It uses the **MIT license** and is **fully FOSS** (free and open-source software).

**One small binary, three small jobs.**

`swiss` bundles three everyday chores into a single static binary:

| Command | What it does |
|---|---|
| `swiss unlock` | Find out *which* process is holding a file (Windows), and optionally kill it |
| `swiss pdf` | Merge, split, rotate, strip metadata and inspect PDFs |
| `swiss brand` | Build a complete web branding set — favicons, web manifest, social image — from one image |

No runtime, no Node, no Python, no uploads. It's one file, about 1.5 MB.

And for when clicking beats typing there's **`swiss-gui`** — the same three jobs in a
window. It's a separate binary, so the command line stays exactly as small as it was.

## Status

🎉 **v0.2.0 is out** — all three jobs, the new `swiss-gui` window, and Windows builds of
both are on the [releases page](https://github.com/not6741lol/swiss/releases).

- [x] CLI skeleton
- [x] `unlock` — Restart Manager integration
- [x] `pdf` — merge / split / rotate / strip / info
- [x] `brand` — favicon set + web manifest
- [x] `brand` — Open Graph image
- [x] `swiss-gui` — the same three jobs in a window
- [x] Prebuilt release binaries

## Install

Grab the binary from [Releases](https://github.com/not6741lol/swiss/releases) — one file, no installer, no runtime. `swiss-gui.exe` sits beside it if you'd rather have a window.

Or build it yourself:

```bash
cargo install --git https://github.com/not6741lol/swiss
```

The window isn't part of `cargo install`, so it needs a clone:

```bash
git clone https://github.com/not6741lol/swiss
cd swiss
cargo build --release --workspace          # both binaries land in target/release
```

## Usage

```bash
# What's locking this folder?
swiss unlock C:\dev\myproject

# ...and close it
swiss unlock C:\dev\myproject --kill

# PDFs
swiss pdf merge out.pdf cover.pdf body.pdf appendix.pdf
swiss pdf split big.pdf --pages 1-3,7 --out pages/
swiss pdf rotate scan.pdf --deg 90
swiss pdf strip report.pdf

# Branding: one image in, a full icon set plus a social card out
swiss brand full logo.png --out public/ --name "not" --title "not" --subtitle "vibecoder"
```

### `swiss brand`

Feed it one image (PNG or JPEG) and it writes the whole set:

| File | Purpose |
|---|---|
| `favicon.ico` | 16 / 32 / 48 in one file, for old tabs, bookmarks and legacy Windows |
| `favicon-16x16.png`, `favicon-32x32.png` | modern tab icons |
| `apple-touch-icon.png` | iOS home screen — filled with `--bg`, since iOS pads transparent icons with black |
| `android-chrome-192x192.png`, `android-chrome-512x512.png` | Android home screen |
| `manifest.webmanifest` | PWA manifest, wired to the icons above |
| `og.png` | 1200×630 social card (`full` only) |

It prints the `<head>` snippet to paste in. Everything is rendered locally — the social card's
text uses your system UI font, and `--font` points it at a specific `.ttf`/`.otf` if you'd rather.

### `swiss-gui`

Same three jobs, one tab each, over the identical core the CLI calls — so the live preview
of a social card really is the file about to be written, not an approximation of it. Drag a
file anywhere onto the window to point the current tab at it, and the pane along the bottom
shows the same lines the command line would have printed.

## Why "swiss"?

Swiss-army knife: small, portable, and it's the tool you reach for instead of installing five separate ones.

## License

MIT — see [LICENSE](LICENSE).

`swiss-gui` bundles Inter and JetBrains Mono so its interface can match the website's
typography. Both are under the SIL Open Font License 1.1, and the licence texts ship
alongside them in [`gui/assets/`](gui/assets).

---

<sub>Made by **not**. More projects and build notes: **[not6741.lol](https://not6741.lol/)**</sub>
