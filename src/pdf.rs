//! `swiss pdf` — merge, split, rotate, strip metadata and inspect PDFs.
//!
//! Pure Rust, via `lopdf`. No external binaries, nothing uploaded anywhere.
//!
//! Each operation has two halves: a function that does the work and returns a
//! report, and a line in [`run`] that prints it.

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

// ------------------------------------------------------------------- reports

/// What [`merge`] did.
#[derive(Debug, Clone)]
pub struct MergeReport {
    /// How many files went in.
    pub inputs: usize,
    /// How many pages came out.
    pub pages: usize,
    /// Where the merged file was written.
    pub out: String,
}

/// What [`split`] did.
#[derive(Debug, Clone)]
pub struct SplitReport {
    /// The directory the pages were written to.
    pub out_dir: String,
    /// One path per page, in page order.
    pub written: Vec<String>,
}

/// What [`rotate`] did.
#[derive(Debug, Clone)]
pub struct RotateReport {
    /// How many pages were touched.
    pub pages: usize,
    /// The rotation applied, in degrees.
    pub degrees: i32,
    /// Where the result was written.
    pub out: String,
}

/// What [`strip`] did.
#[derive(Debug, Clone)]
pub struct StripReport {
    /// The file that was read.
    pub input: String,
    /// Where the stripped file was written.
    pub out: String,
}

/// Everything [`info`] learned about a document.
#[derive(Debug, Clone)]
pub struct PdfInfo {
    /// The file that was read.
    pub path: String,
    /// Size on disk, in bytes.
    pub bytes: u64,
    /// PDF version from the header, e.g. `1.5`.
    pub version: String,
    /// Page count.
    pub pages: usize,
    /// Whether the trailer carries an `/Encrypt` entry.
    pub encrypted: bool,
    /// Title, author, producer and friends, in the order they should be shown.
    pub metadata: Vec<(String, String)>,
}

// ------------------------------------------------------------------ front-end

pub fn run(args: PdfArgs) -> Result<()> {
    match args.command {
        PdfCommand::Merge { out, inputs } => {
            let report = merge(&inputs, &out)?;
            println!(
                "Merged {} file(s) — {} page(s) total — into {}",
                report.inputs, report.pages, report.out
            );
            Ok(())
        }

        PdfCommand::Split { input, pages, out } => {
            let report = split(&input, pages.as_deref(), &out)?;
            for file in &report.written {
                println!("wrote {file}");
            }
            println!(
                "\n{} page(s) written to {}",
                report.written.len(),
                report.out_dir
            );
            Ok(())
        }

        PdfCommand::Rotate { input, deg, out } => {
            let report = rotate(&input, deg, out.as_deref())?;
            println!(
                "Rotated {} page(s) by {}° -> {}",
                report.pages, report.degrees, report.out
            );
            Ok(())
        }

        PdfCommand::Strip { input, out } => {
            let report = strip(&input, out.as_deref())?;
            println!("Stripped metadata from {} -> {}", report.input, report.out);
            Ok(())
        }

        PdfCommand::Info { input } => {
            let found = info(&input)?;
            println!("file      : {}", found.path);
            println!("size      : {}", human_size(found.bytes));
            println!("version   : {}", found.version);
            println!("pages     : {}", found.pages);
            println!("encrypted : {}", found.encrypted);

            if !found.metadata.is_empty() {
                println!();
                for (key, value) in &found.metadata {
                    println!("{:<13}: {value}", key.to_lowercase());
                }
            }
            Ok(())
        }
    }
}

// --------------------------------------------------------------------- work

/// Merge `inputs` into `out`, in the order given.
pub fn merge(inputs: &[String], out: &str) -> Result<MergeReport> {
    if inputs.len() < 2 {
        bail!("merging needs at least two input files");
    }

    let mut merged = Document::with_version("1.5");
    let mut page_ids: Vec<ObjectId> = Vec::new();
    let mut highest: u32 = 0;

    for input in inputs {
        let mut doc = Document::load(input).with_context(|| format!("could not read `{input}`"))?;

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

    Ok(MergeReport {
        inputs: inputs.len(),
        pages: page_ids.len(),
        out: out.to_string(),
    })
}

/// Write one file per selected page into `out_dir`.
pub fn split(input: &str, pages: Option<&str>, out_dir: &str) -> Result<SplitReport> {
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

    let mut written = Vec::with_capacity(selected.len());

    for page in &selected {
        // Reload each time so every output starts from a clean document.
        let mut single =
            Document::load(input).with_context(|| format!("could not read `{input}`"))?;

        let drop: Vec<u32> = all.iter().copied().filter(|number| number != page).collect();
        single.delete_pages(&drop);
        single.prune_objects();
        single.renumber_objects();
        let _ = single.compress();

        let target = Path::new(out_dir).join(format!("{stem}-{page:03}.pdf"));
        single
            .save(&target)
            .with_context(|| format!("could not write `{}`", target.display()))?;
        written.push(target.display().to_string());
    }

    Ok(SplitReport {
        out_dir: out_dir.to_string(),
        written,
    })
}

/// Rotate every page by `deg`, writing to `out` (or over the input).
pub fn rotate(input: &str, deg: i32, out: Option<&str>) -> Result<RotateReport> {
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

    Ok(RotateReport {
        pages: count,
        degrees: deg,
        out: dest.to_string(),
    })
}

/// Remove the info dictionary and any XMP metadata stream.
pub fn strip(input: &str, out: Option<&str>) -> Result<StripReport> {
    let dest = out.unwrap_or(input);

    let mut doc = Document::load(input).with_context(|| format!("could not read `{input}`"))?;

    // `/Info` is an indirect reference in almost every real PDF, so the object has
    // to go as well — dropping only the trailer entry would leave the metadata
    // bytes sitting in the file for anyone to read back.
    let info_id = match doc.trailer.get(b"Info") {
        Ok(Object::Reference(id)) => Some(*id),
        _ => None,
    };
    doc.trailer.remove(b"Info");
    if let Some(id) = info_id {
        doc.objects.remove(&id);
    }

    // XMP metadata is a stream referenced from the catalog, so same again.
    let xmp_id = doc.catalog().ok().and_then(|catalog| {
        match catalog.get(b"Metadata") {
            Ok(Object::Reference(id)) => Some(*id),
            _ => None,
        }
    });
    if let Ok(catalog) = doc.catalog_mut() {
        catalog.remove(b"Metadata");
    }
    if let Some(id) = xmp_id {
        doc.objects.remove(&id);
    }

    // Drop anything else the removals left unreachable.
    doc.prune_objects();
    let _ = doc.compress();
    doc.save(dest)
        .with_context(|| format!("could not write `{dest}`"))?;

    Ok(StripReport {
        input: input.to_string(),
        out: dest.to_string(),
    })
}

/// Read the page count, size, version and metadata of a document.
pub fn info(input: &str) -> Result<PdfInfo> {
    let doc = Document::load(input).with_context(|| format!("could not read `{input}`"))?;

    let bytes = std::fs::metadata(input).map(|meta| meta.len()).unwrap_or(0);

    let metadata = read_metadata(&doc);

    Ok(PdfInfo {
        path: input.to_string(),
        bytes,
        version: doc.version.clone(),
        pages: doc.get_pages().len(),
        encrypted: doc.trailer.has(b"Encrypt"),
        metadata,
    })
}

/// Just the page count — handy for a UI that needs to bound a page range.
pub fn page_count(input: &str) -> Result<usize> {
    let doc = Document::load(input).with_context(|| format!("could not read `{input}`"))?;
    Ok(doc.get_pages().len())
}

// ------------------------------------------------------------------ helpers

/// Reserve the next free object id in a document.
fn next_id(doc: &mut Document) -> ObjectId {
    doc.max_id += 1;
    (doc.max_id, 0)
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

/// Read the `/Info` dictionary, following the indirect reference real files use.
fn read_metadata(doc: &Document) -> Vec<(String, String)> {
    let Ok(entry) = doc.trailer.get(b"Info") else {
        return Vec::new();
    };

    // `/Info` is nearly always `n 0 R` rather than an inline dictionary, so the
    // object has to be resolved before any of its keys can be read.
    let dict = match entry {
        Object::Reference(id) => match doc.get_object(*id).ok().and_then(|obj| obj.as_dict().ok()) {
            Some(dict) => dict,
            None => return Vec::new(),
        },
        other => match other.as_dict() {
            Ok(dict) => dict,
            Err(_) => return Vec::new(),
        },
    };

    const KEYS: [&str; 8] = [
        "Title",
        "Author",
        "Subject",
        "Keywords",
        "Creator",
        "Producer",
        "CreationDate",
        "ModDate",
    ];

    KEYS.iter()
        .filter_map(|key| {
            dict.get(key.as_bytes())
                .ok()
                .and_then(printable)
                .map(|text| (key.to_string(), text))
        })
        .collect()
}

fn printable(object: &Object) -> Option<String> {
    match object {
        Object::String(bytes, _) => Some(decode_text(bytes)),
        Object::Name(bytes) => Some(String::from_utf8_lossy(bytes).to_string()),
        Object::Integer(value) => Some(value.to_string()),
        Object::Real(value) => Some(value.to_string()),
        _ => None,
    }
}

/// PDF text strings are either PDFDocEncoding or UTF-16BE behind a byte-order mark.
fn decode_text(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16_lossy(&units).trim().to_string()
    } else {
        String::from_utf8_lossy(bytes).trim().to_string()
    }
}

/// Format a byte count as `1.5 MB` and friends.
pub fn human_size(bytes: u64) -> String {
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
