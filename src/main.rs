//! swiss — one small binary, three small jobs.
//!
//! * `swiss unlock` — find (and optionally kill) whatever is locking a file.
//! * `swiss pdf`    — merge, split, rotate, strip metadata, inspect.
//! * `swiss brand`  — build a full web branding set from one image.

use anyhow::Result;
use clap::{Parser, Subcommand};

use swiss::{brand, pdf, unlock};

/// One small binary, three small jobs.
#[derive(Debug, Parser)]
#[command(
    name = "swiss",
    version,
    about = "One small binary, three small jobs.",
    long_about = "swiss bundles three everyday chores into a single tiny binary:\n  unlock  find (and kill) whatever is locking a file\n  pdf     merge, split, rotate, strip and inspect PDFs\n  brand   build a full web branding set from one image",
    propagate_version = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Find (and optionally kill) whatever is locking a file or folder.
    Unlock(unlock::UnlockArgs),

    /// Wrangle PDFs: merge, split, rotate, strip metadata, inspect.
    Pdf(pdf::PdfArgs),

    /// Build a full web branding set (favicons, manifest, social image) from one image.
    Brand(brand::BrandArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Unlock(args) => unlock::run(args),
        Command::Pdf(args) => pdf::run(args),
        Command::Brand(args) => brand::run(args),
    }
}
