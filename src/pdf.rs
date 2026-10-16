//! `swiss pdf` — merge, split, rotate, strip metadata and inspect PDFs.

use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct PdfArgs {
    #[command(subcommand)]
    pub command: PdfCommand,
}

#[derive(Debug, Subcommand)]
pub enum PdfCommand {
    /// Merge two or more PDFs into one.
    Merge {
        /// Output file.
        out: String,
        /// Input PDFs, in order.
        #[arg(required = true)]
        inputs: Vec<String>,
    },

    /// Split a PDF, one file per page (or only the pages you pick).
    Split {
        /// Input PDF.
        input: String,
        /// Pages to extract, e.g. `1-3,7`.
        #[arg(long)]
        pages: Option<String>,
        /// Output directory.
        #[arg(long, default_value = ".")]
        out: String,
    },

    /// Rotate every page.
    Rotate {
        /// Input PDF.
        input: String,
        /// Rotation in degrees (90, 180 or 270).
        #[arg(long, default_value_t = 90)]
        deg: i32,
    },

    /// Remove metadata (title, author, producer, ...).
    Strip {
        /// Input PDF.
        input: String,
    },

    /// Print page count, file size and metadata.
    Info {
        /// Input PDF.
        input: String,
    },
}

pub fn run(args: PdfArgs) -> Result<()> {
    let what = match args.command {
        PdfCommand::Merge { out, inputs } => {
            format!("merge {} file(s) into {out}", inputs.len())
        }
        PdfCommand::Split { input, pages, out } => match pages {
            Some(pages) => format!("split {input} (pages {pages}) into {out}"),
            None => format!("split {input} into {out}"),
        },
        PdfCommand::Rotate { input, deg } => format!("rotate {input} by {deg} degrees"),
        PdfCommand::Strip { input } => format!("strip metadata from {input}"),
        PdfCommand::Info { input } => format!("show info for {input}"),
    };

    anyhow::bail!("`swiss pdf` is not implemented yet — parsed: {what}")
}
