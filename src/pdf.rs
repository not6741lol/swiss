//! `swiss pdf` — merge, split, rotate, strip metadata and inspect PDFs.
//!
//! Pure Rust, via `lopdf`. No external binaries, nothing uploaded anywhere.

use std::path::Path;

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand};
use lopdf::{Dictionary, Document, Object, ObjectId};

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
        /// Pages to extract, e.g. `1-3,7`. Defaults to every page.
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
        /// Output file. Defaults to overwriting the input.
        #[arg(long)]
        out: Option<String>,
    },

    /// Remove metadata (title, author, producer, XMP, ...).
    Strip {
        /// Input PDF.
        input: String,
        /// Output file. Defaults to overwriting the input.
        #[arg(long)]
        out: Option<String>,
    },

    /// Print page count, file size and metadata.
    Info {
        /// Input PDF.
        input: String,
    },
}

pub fn run(args: PdfArgs) -> Result<()> {
    match args.command {
        PdfCommand::Merge { out, inputs } => merge(&inputs, &out),
        PdfCommand::Split { input, pages, out } => split(&input, pages.as_deref(), &out),
        PdfCommand::Rotate { input, deg, out } => rotate(&input, deg, out.as_deref()),
        PdfCommand::Strip { input, out } => strip(&input, out.as_deref()),
        PdfCommand::Info { input } => info(&input),
    }
}

/// Reserve the next free object id in a document.
fn next_id(doc: &mut Document) -> ObjectId {
    doc.max_id += 1;
    (doc.max_id, 0)
}

fn merge(inputs: &[String], out: &str) -> Result<()> {
    if inputs.len() < 2 {
        bail!("merging needs at least two input files");
    }

    let mut merged = Document::with_version("1.5");
    let mut page_ids: Vec<ObjectId> = Vec::new();
    let mut highest: u32 = 0;

    for input in inputs {
        let mut doc =
            Document::load(input).with_context(|| format!("could not read `{input}`"))?;

        // Shift every object id so documents never collide, then keep the pages.
        doc.renumber_objects_with(highest + 1);

        for page_id in doc.get_pages().values() {
            page_ids.push(*page_id);
        }
        if let Some(max) = doc.objects.keys().map(|(id, _)| *id).max() {
            highest = highest.max(max);
        }

        merged.objects.extend(doc.objects);
    }

    merged.max_id = highest;

    // One fresh /Pages node that every page will point at.
    let pages_id = next_id(&mut merged);

    for page_id in &page_ids {
        let page = merged
            .get_object_mut(*page_id)
            .map_err(|err| anyhow!("page object {page_id:?}: {err}"))?
            .as_dict_mut()
            .map_err(|err| anyhow!("page object {page_id:?} is not a dictionary: {err}"))?;
        page.set(b"Parent", Object::Reference(pages_id));
    }

    let kids: Vec<Object> = page_ids.iter().map(|id| Object::Reference(*id)).collect();

    let mut pages = Dictionary::default();
    pages.set(b"Type", Object::Name(b"Pages".to_vec()));
    pages.set(b"Kids", Object::Array(kids.clone()));
    pages.set(b"Count", Object::Integer(kids.len() as i64));
    merged.objects.insert(pages_id, Object::Dictionary(pages));

    let mut catalog = Dictionary::default();
    catalog.set(b"Type", Object::Name(b"Catalog".to_vec()));
    catalog.set(b"Pages", Object::Reference(pages_id));
    let catalog_id = next_id(&mut merged);
    merged.objects.insert(catalog_id, Object::Dictionary(catalog));

    merged.trailer.set(b"Root", Object::Reference(catalog_id));
    merged.trailer.remove(b"Info");

    // Drop the per-document catalogs/page trees we replaced.
    merged.prune_objects();
    let _ = merged.compress();
    merged
        .save(out)
        .with_context(|| format!("could not write `{out}`"))?;

    println!(
        "Merged {} file(s) — {} page(s) total — into {out}",
        inputs.len(),
        page_ids.len()
    );
    Ok(())
}

fn split(input: &str, pages: Option<&str>, out_dir: &str) -> Result<()> {
    let probe = Document::load(input).with_context(|| format!("could not read `{input}`"))?;
    let all: Vec<u32> = probe.get_pages().keys().copied().collect();
    let total = all.len() as u32;

    let selected = match pages {
        Some(spec) => parse_pages(spec, total)?,
        None => all.clone(),
    };

    std::fs::create_dir_all(out_dir).with_context(|| format!("could not create `{out_dir}`"))?;

    let stem = Path::new(input)
        .file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "page".to_string());

    for page in &selected {
        // Reload each time so every output starts from a clean document.
        let mut single = Document::load(input).with_context(|| format!("could not read `{input}`"))?;

        let drop: Vec<u32> = all.iter().copied().filter(|number| number != page).collect();
        single.delete_pages(&drop);
        single.prune_objects();
        single.renumber_objects();
        let _ = single.compress();

        let target = Path::new(out_dir).join(format!("{stem}-{page:03}.pdf"));
        single
            .save(&target)
            .with_context(|| format!("could not write `{}`", target.display()))?;
        println!("wrote {}", target.display());
    }

    println!("\n{} page(s) written to {out_dir}", selected.len());
    Ok(())
}

fn rotate(input: &str, deg: i32, out: Option<&str>) -> Result<()> {
    let dest = out.unwrap_or(input);

    let mut doc = Document::load(input).with_context(|| format!("could not read `{input}`"))?;
    let page_ids: Vec<ObjectId> = doc.get_pages().values().copied().collect();
    let count = page_ids.len();

    for page_id in page_ids {
        let page = doc
            .get_object_mut(page_id)
            .map_err(|err| anyhow!("page object {page_id:?}: {err}"))?
            .as_dict_mut()
            .map_err(|err| anyhow!("page object {page_id:?} is not a dictionary: {err}"))?;

        let current = page
            .get(b"Rotate")
            .ok()
            .and_then(|value| value.as_i64().ok())
            .unwrap_or(0);
        let normalised = ((current + deg as i64) % 360 + 360) % 360;

        if normalised == 0 {
            page.remove(b"Rotate");
        } else {
            page.set(b"Rotate", Object::Integer(normalised));
        }
    }

    let _ = doc.compress();
    doc.save(dest)
        .with_context(|| format!("could not write `{dest}`"))?;

    println!("Rotated {count} page(s) by {deg}° -> {dest}");
    Ok(())
}

fn strip(input: &str, out: Option<&str>) -> Result<()> {
    let dest = out.unwrap_or(input);

    let mut doc = Document::load(input).with_context(|| format!("could not read `{input}`"))?;

    // The document info dictionary holds title/author/producer/creator/dates.
    doc.trailer.remove(b"Info");

    // XMP metadata lives as a stream referenced from the catalog.
    if let Ok(catalog) = doc.catalog_mut() {
        catalog.remove(b"Metadata");
    }

    let _ = doc.compress();
    doc.save(dest)
        .with_context(|| format!("could not write `{dest}`"))?;

    println!("Stripped metadata from {input} -> {dest}");
    Ok(())
}

fn info(input: &str) -> Result<()> {
    let doc = Document::load(input).with_context(|| format!("could not read `{input}`"))?;

    let bytes = std::fs::metadata(input).map(|meta| meta.len()).unwrap_or(0);

    println!("file      : {input}");
    println!("size      : {}", human_size(bytes));
    println!("version   : {}", doc.version);
    println!("pages     : {}", doc.get_pages().len());
    println!("encrypted : {}", doc.trailer.has(b"Encrypt"));

    if let Ok(object) = doc.trailer.get(b"Info") {
        if let Ok(metadata) = object.as_dict() {
            let mut printed_header = false;
            for key in [
                "Title",
                "Author",
                "Subject",
                "Keywords",
                "Creator",
                "Producer",
                "CreationDate",
                "ModDate",
            ] {
                if let Some(text) = metadata.get(key.as_bytes()).ok().and_then(printable) {
                    if !printed_header {
                        println!();
                        printed_header = true;
                    }
                    println!("{:<13}: {text}", key.to_lowercase());
                }
            }
        }
    }

    Ok(())
}

/// Turn `1-3,7` into `[1, 2, 3, 7]`, clamped to the document.
fn parse_pages(spec: &str, total: u32) -> Result<Vec<u32>> {
    let mut pages = Vec::new();

    for chunk in spec.split(',') {
        let chunk = chunk.trim();
        if chunk.is_empty() {
            continue;
        }

        match chunk.split_once('-') {
            Some((start, end)) => {
                let start: u32 = start
                    .trim()
                    .parse()
                    .with_context(|| format!("bad page range `{chunk}`"))?;
                let end: u32 = end
                    .trim()
                    .parse()
                    .with_context(|| format!("bad page range `{chunk}`"))?;
                let (start, end) = if start <= end { (start, end) } else { (end, start) };
                for page in start..=end {
                    if (1..=total).contains(&page) {
                        pages.push(page);
                    }
                }
            }
            None => {
                let page: u32 = chunk
                    .parse()
                    .with_context(|| format!("bad page number `{chunk}`"))?;
                if (1..=total).contains(&page) {
                    pages.push(page);
                }
            }
        }
    }

    pages.sort_unstable();
    pages.dedup();

    if pages.is_empty() {
        bail!("no valid pages in `{spec}` (the document has {total})");
    }
    Ok(pages)
}

fn printable(object: &Object) -> Option<String> {
    match object {
        Object::String(bytes, _) => Some(String::from_utf8_lossy(bytes).trim().to_string()),
        Object::Name(bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
        Object::Integer(value) => Some(value.to_string()),
        Object::Real(value) => Some(value.to_string()),
        _ => None,
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];

    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
