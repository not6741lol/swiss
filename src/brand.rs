//! `swiss brand` — build a full web branding set from a single image.

use anyhow::Result;
use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct BrandArgs {
    #[command(subcommand)]
    pub command: BrandCommand,
}

#[derive(Debug, Subcommand)]
pub enum BrandCommand {
    /// Generate the favicon / app-icon set plus a web manifest.
    Favicon {
        /// Source image (ideally square).
        image: String,
        /// Output directory.
        #[arg(long, default_value = "public")]
        out: String,
    },

    /// Generate a social preview (Open Graph) image.
    Og {
        /// Source image.
        image: String,
        /// Big headline.
        #[arg(long)]
        title: Option<String>,
        /// Smaller line under the headline.
        #[arg(long)]
        subtitle: Option<String>,
        /// Output file.
        #[arg(long, default_value = "og.png")]
        out: String,
    },

    /// Do everything: favicon set, manifest and social image.
    Full {
        /// Source image (ideally square).
        image: String,
        /// Output directory.
        #[arg(long, default_value = "public")]
        out: String,
        /// Big headline for the social image.
        #[arg(long)]
        title: Option<String>,
        /// Smaller line for the social image.
        #[arg(long)]
        subtitle: Option<String>,
    },
}

pub fn run(args: BrandArgs) -> Result<()> {
    let what = match args.command {
        BrandCommand::Favicon { image, out } => format!("favicon set for {image} into {out}"),
        BrandCommand::Og {
            image,
            title,
            subtitle,
            out,
        } => {
            let title = title.unwrap_or_else(|| "(no title)".into());
            let subtitle = subtitle.unwrap_or_else(|| "(no subtitle)".into());
            format!("Open Graph image for {image} (title: {title}, subtitle: {subtitle}) into {out}")
        }
        BrandCommand::Full { image, out, title, .. } => {
            let title = title.unwrap_or_else(|| "(no title)".into());
            format!("full branding set for {image} (title: {title}) into {out}")
        }
    };

    anyhow::bail!("`swiss brand` is not implemented yet — parsed: {what}")
}
