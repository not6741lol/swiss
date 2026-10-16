# swiss

**One small binary, three small jobs.**

`swiss` bundles three everyday chores into a single static binary:

| Command | What it does |
|---|---|
| `swiss unlock` | Find out *which* process is holding a file (Windows), and optionally kill it |
| `swiss pdf` | Merge, split, rotate, strip metadata and inspect PDFs |
| `swiss brand` | Build a complete web branding set — favicons, web manifest, social image — from one image |

No runtime, no Node, no Python, no uploads. It's one file.

## Status

🚧 **Early.** The CLI surface is in place; the three jobs are being filled in milestone by milestone.

- [x] CLI skeleton
- [ ] `unlock` — Restart Manager integration
- [ ] `pdf` — merge / split / rotate / strip / info
- [ ] `brand` — favicon set + web manifest
- [ ] `brand` — Open Graph image
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

# Branding
swiss brand full logo.png --out public/ --title "not" --subtitle "vibecoder"
```

## Why "swiss"?

Swiss-army knife: small, portable, and it's the tool you reach for instead of installing five separate ones.

## License

MIT
