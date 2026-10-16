# swiss

**One small binary, three small jobs.**

`swiss` bundles three everyday chores into a single static binary:

| Command | What it does |
|---|---|
| `swiss unlock` | Find out *which* process is holding a file (Windows), and optionally kill it |
| `swiss pdf` | Merge, split, rotate, strip metadata and inspect PDFs |
| `swiss brand` | Build a complete web branding set — favicons, web manifest, social image — from one image |

No runtime, no Node, no Python, no uploads. It's one file, about 1.5 MB.

## Status

🚧 **All three jobs work.** Prebuilt binaries are still to come.

- [x] CLI skeleton
- [x] `unlock` — Restart Manager integration
- [x] `pdf` — merge / split / rotate / strip / info
- [x] `brand` — favicon set + web manifest
- [x] `brand` — Open Graph image
- [ ] Prebuilt release binaries

## Install

```bash
cargo install --git https://github.com/not6741lol/swiss
```

Prebuilt binaries will land in [Releases](https://github.com/not6741lol/swiss/releases).

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

## Why "swiss"?

Swiss-army knife: small, portable, and it's the tool you reach for instead of installing five separate ones.

## License

MIT
