//! `swiss brand` — build a full web branding set from a single image.
//!
//! Everything is rendered locally: the favicon / app-icon set, a web manifest, an
//! HTML snippet, and a 1200x630 Open Graph card. No network, no external tools.

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use clap::{Args, Subcommand, ValueEnum};
use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use fontdue::{Font, FontSettings};
use image::imageops::{self, FilterType};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

#[derive(Debug, Args)]
pub struct BrandArgs {
    #[command(subcommand)]
    pub command: BrandCommand,
}

/// How a non-square source is fitted into the square icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Fit {
    /// Scale to fit inside the square, centred, leaving the padding transparent.
    Contain,
    /// Scale to fill the square, cropping whatever overflows.
    Cover,
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
        /// App name written into the web manifest. Defaults to the image file name.
        #[arg(long)]
        name: Option<String>,
        /// `theme-color` for the manifest and the HTML snippet.
        #[arg(long, default_value = "#111111")]
        theme: String,
        /// `background_color` for the manifest, and the fill behind the Apple touch icon.
        #[arg(long, default_value = "#ffffff")]
        bg: String,
        /// How a non-square source is fitted into the square icons.
        #[arg(long, value_enum, default_value_t = Fit::Contain)]
        fit: Fit,
    },

    /// Generate a social preview (Open Graph) image.
    Og {
        /// Source image.
        image: String,
        /// Big headline. Wraps automatically; `\n` starts a new line.
        #[arg(long)]
        title: Option<String>,
        /// Smaller line under the headline. Also wraps, and accepts `\n`.
        #[arg(long)]
        subtitle: Option<String>,
        /// Output file.
        #[arg(long, default_value = "og.png")]
        out: String,
        /// Card size, as WIDTHxHEIGHT.
        #[arg(long, default_value = "1200x630")]
        size: String,
        /// Card background colour.
        #[arg(long, default_value = "#111013")]
        bg: String,
        /// Accent colour, used for the bar down the left edge.
        #[arg(long, default_value = "#ff9f1c")]
        accent: String,
        /// A .ttf/.otf file for the text. Defaults to the system UI font.
        #[arg(long)]
        font: Option<String>,
        /// How a non-square source is fitted into the logo box.
        #[arg(long, value_enum, default_value_t = Fit::Contain)]
        fit: Fit,
    },

    /// Do everything: favicon set, manifest and social image.
    Full {
        /// Source image (ideally square).
        image: String,
        /// Output directory.
        #[arg(long, default_value = "public")]
        out: String,
        /// App name written into the web manifest. Defaults to the image file name.
        #[arg(long)]
        name: Option<String>,
        /// Big headline for the social image. Wraps automatically; `\n` starts a
        /// new line.
        #[arg(long)]
        title: Option<String>,
        /// Smaller line for the social image. Also wraps, and accepts `\n`.
        #[arg(long)]
        subtitle: Option<String>,
        /// A .ttf/.otf file for the social image text. Defaults to the system UI font.
        #[arg(long)]
        font: Option<String>,
        /// `theme-color` for the manifest, and the social card background.
        #[arg(long, default_value = "#111111")]
        theme: String,
        /// `background_color` for the manifest.
        #[arg(long, default_value = "#ffffff")]
        bg: String,
        /// Accent colour for the social card.
        #[arg(long, default_value = "#ff9f1c")]
        accent: String,
        /// How a non-square source is fitted into the square icons.
        #[arg(long, value_enum, default_value_t = Fit::Contain)]
        fit: Fit,
    },
}

// ------------------------------------------------------------------- reports

/// One file written by [`favicons`], with a label for it.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Where it was written.
    pub path: PathBuf,
    /// Short description, e.g. `16x16` or `16 / 32 / 48`.
    pub detail: String,
}

/// Everything [`favicons`] wrote.
#[derive(Debug, Clone)]
pub struct IconSet {
    /// The directory the set went into.
    pub dir: PathBuf,
    /// The app name used in the manifest.
    pub app: String,
    /// Every file, in the order they should be shown.
    pub entries: Vec<Entry>,
    /// The `<head>` block that goes with this set.
    pub snippet: String,
}

/// Everything [`card`] wrote.
#[derive(Debug, Clone)]
pub struct Card {
    /// Where the card was written.
    pub out: PathBuf,
    /// Card width in pixels.
    pub width: u32,
    /// Card height in pixels.
    pub height: u32,
    /// Set when a headline was requested but no font could be loaded, so the card
    /// came out logo-only.
    pub font_missing: bool,
    /// Set when the copy was longer than the card's line budget and was cut with
    /// an ellipsis.
    pub truncated: bool,
}

/// A rendered card, plus anything the caller ought to know about it.
pub struct RenderedCard {
    /// The pixels.
    pub image: RgbaImage,
    /// Set when there was a headline to draw but no usable font to draw it with,
    /// so the card came out logo-only.
    pub font_missing: bool,
    /// Set when the copy was cut to fit the card's line budget.
    pub truncated: bool,
}

// ------------------------------------------------------------------ front-end

pub fn run(args: BrandArgs) -> Result<()> {
    match args.command {
        BrandCommand::Favicon {
            image,
            out,
            name,
            theme,
            bg,
            fit,
        } => {
            let set = favicons(&image, Path::new(&out), name.as_deref(), &theme, &bg, fit)?;
            print_set(&set);
            Ok(())
        }

        BrandCommand::Og {
            image,
            title,
            subtitle,
            out,
            size,
            bg,
            accent,
            font,
            fit,
        } => {
            let (width, height) = parse_size(&size)?;
            let shown = title.as_deref().unwrap_or("");
            let card = card(
                &image,
                Path::new(&out),
                shown,
                subtitle.as_deref(),
                width,
                height,
                &bg,
                &accent,
                font.as_deref(),
                fit,
            )?;
            warn_about(&card);
            println!(
                "Social card : {} ({}x{})",
                card.out.display(),
                card.width,
                card.height
            );
            Ok(())
        }

        BrandCommand::Full {
            image,
            out,
            name,
            title,
            subtitle,
            font,
            theme,
            bg,
            accent,
            fit,
        } => {
            let dir = Path::new(&out);
            let set = favicons(&image, dir, name.as_deref(), &theme, &bg, fit)?;
            print_set(&set);

            let shown = title.as_deref().unwrap_or("");
            let social = card(
                &image,
                &dir.join("og.png"),
                shown,
                subtitle.as_deref(),
                1200,
                630,
                &theme,
                &accent,
                font.as_deref(),
                fit,
            )?;

            warn_about(&social);
            println!(
                "\nSocial card : {} ({}x{})",
                social.out.display(),
                social.width,
                social.height
            );
            Ok(())
        }
    }
}

/// The `swiss brand favicon` report, as the terminal shows it.
fn print_set(set: &IconSet) {
    println!("Icons -> {}\n", set.dir.display());

    for entry in &set.entries {
        let name = entry
            .path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        println!("  {:<28}{}", name, entry.detail);
    }

    println!("\nPaste into <head>:\n");
    println!("{}", set.snippet);
}

/// Say so when a card could not show everything it was given.
fn warn_about(card: &Card) {
    if card.font_missing {
        eprintln!("note: no system font found, drawing the card without text (pass --font)");
    }
    if card.truncated {
        eprintln!(
            "note: copy was cut to 3 headline / 2 subtitle lines; shorten it or raise --size"
        );
    }
}

// --------------------------------------------------------------------- work

/// Write the whole icon set, the manifest and the HTML snippet.
pub fn favicons(
    image: &str,
    dir: &Path,
    name: Option<&str>,
    theme: &str,
    bg: &str,
    fit: Fit,
) -> Result<IconSet> {
    let source = load_image(image)?;
    let theme = parse_color(theme)?;
    let bg = parse_color(bg)?;

    fs::create_dir_all(dir).with_context(|| format!("could not create `{}`", dir.display()))?;

    let app = name.map(str::to_string).unwrap_or_else(|| default_name(image));

    let mut entries = Vec::new();

    // The well-known set browsers and phones actually look for.
    let icons: [(&str, u32, bool); 5] = [
        ("favicon-16x16.png", 16, false),
        ("favicon-32x32.png", 32, false),
        ("apple-touch-icon.png", 180, true),
        ("android-chrome-192x192.png", 192, false),
        ("android-chrome-512x512.png", 512, false),
    ];

    for (file, size, opaque) in icons {
        // iOS composites transparent icons onto black, so the Apple icon gets a real fill.
        let padding = if opaque { Some(bg) } else { None };
        let icon = squared(&source, size, fit, padding);
        let path = dir.join(file);
        icon.save_with_format(&path, ImageFormat::Png)
            .with_context(|| format!("could not write `{}`", path.display()))?;
        entries.push(Entry {
            path,
            detail: format!("{size}x{size}"),
        });
    }

    // A single .ico with 16/32/48 frames, for old tabs, bookmarks and legacy Windows.
    let frames: Vec<(u32, RgbaImage)> = [16u32, 32, 48]
        .iter()
        .map(|size| (*size, squared(&source, *size, fit, None)))
        .collect();
    let ico = dir.join("favicon.ico");
    write_ico(&frames, &ico)?;
    entries.push(Entry {
        path: ico,
        detail: "16 / 32 / 48".to_string(),
    });

    let manifest = dir.join("manifest.webmanifest");
    fs::write(
        &manifest,
        manifest_json(&app, &short_name(&app), &hex(theme), &hex(bg)),
    )
    .with_context(|| format!("could not write `{}`", manifest.display()))?;
    entries.push(Entry {
        path: manifest,
        detail: app.clone(),
    });

    Ok(IconSet {
        dir: dir.to_path_buf(),
        app,
        entries,
        snippet: snippet(&hex(theme)),
    })
}

/// Render the Open Graph card and write it to `out`.
#[allow(clippy::too_many_arguments)]
pub fn card(
    image: &str,
    out: &Path,
    title: &str,
    subtitle: Option<&str>,
    width: u32,
    height: u32,
    bg: &str,
    accent: &str,
    font_path: Option<&str>,
    fit: Fit,
) -> Result<Card> {
    let rendered = render_card(
        image, title, subtitle, width, height, bg, accent, font_path, fit,
    )?;

    if let Some(parent) = out.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create `{}`", parent.display()))?;
        }
    }

    rendered
        .image
        .save_with_format(out, ImageFormat::Png)
        .with_context(|| format!("could not write `{}`", out.display()))?;

    Ok(Card {
        out: out.to_path_buf(),
        width,
        height,
        font_missing: rendered.font_missing,
        truncated: rendered.truncated,
    })
}

/// Draw the Open Graph card into a fresh image. Nothing is written to disk, which
/// is what lets a UI show the result as you edit the fields.
#[allow(clippy::too_many_arguments)]
pub fn render_card(
    image: &str,
    title: &str,
    subtitle: Option<&str>,
    width: u32,
    height: u32,
    bg: &str,
    accent: &str,
    font_path: Option<&str>,
    fit: Fit,
) -> Result<RenderedCard> {
    let source = load_image(image)?;
    let background = parse_color(bg)?;
    let accent = parse_color(accent)?;
    let fonts = load_fonts(font_path)?;

    let mut canvas = RgbaImage::from_pixel(width, height, background);

    // Accent bar down the left edge.
    let bar = (((width as f32) * 0.013).round().clamp(6.0, 32.0) as u32).min(width);
    for y in 0..height {
        for x in 0..bar {
            canvas.put_pixel(x, y, accent);
        }
    }

    // `\n` is much easier to type in a shell than a literal newline, so accept
    // both spellings.
    let title = unescape(title);
    let title = title.trim();
    let subtitle = unescape(subtitle.unwrap_or(""));
    let subtitle = subtitle.trim();
    let has_text = !title.is_empty() || !subtitle.is_empty();

    // With no headline, the subtitle is the headline: it would look odd as a
    // small grey line beside a large logo.
    let (title, subtitle) = if title.is_empty() && !subtitle.is_empty() {
        (subtitle, "")
    } else {
        (title, subtitle)
    };

    let on_dark = luminance(background) < 140.0;
    let primary = if on_dark {
        Rgba([246, 244, 240, 255])
    } else {
        Rgba([22, 22, 26, 255])
    };
    let muted = blend(primary, background, 0.45);

    let (font_missing, truncated) = match &fonts {
        // Logo on the left, text on the right, the pair centred as one group.
        Some(fonts) if has_text => {
            let plan = lay_out(fonts, title, subtitle, width, height, bar);

            let logo = squared(&source, plan.logo_size, fit, None);
            let logo_y = (height.saturating_sub(plan.logo_size) / 2) as i64;
            imageops::overlay(&mut canvas, &logo, plan.logo_left, logo_y);

            let mut cursor = ((height as f32 - plan.height) / 2.0).max(plan.pad);
            if plan.title.height > 0.0 {
                let drawn = plan.title.draw(
                    &mut canvas,
                    &fonts.display,
                    plan.text_left,
                    cursor,
                    primary,
                );
                cursor += drawn + plan.gap;
            }
            if plan.subtitle.height > 0.0 {
                plan.subtitle
                    .draw(&mut canvas, fonts.body(), plan.text_left, cursor, muted);
            }

            (false, plan.truncated)
        }

        // Nothing to say, or no font available: just present the logo.
        _ => {
            let box_size = ((width.min(height) as f32) * 0.62).round() as u32;
            let logo = squared(&source, box_size, fit, None);
            let x = (width.saturating_sub(box_size) / 2) as i64;
            let y = (height.saturating_sub(box_size) / 2) as i64;
            imageops::overlay(&mut canvas, &logo, x, y);

            (fonts.is_none() && has_text, false)
        }
    };

    Ok(RenderedCard {
        image: canvas,
        font_missing,
        truncated,
    })
}

// ---------------------------------------------------------------- image helpers

fn load_image(path: &str) -> Result<DynamicImage> {
    image::ImageReader::open(path)
        .with_context(|| format!("could not open `{path}`"))?
        .with_guessed_format()
        .with_context(|| format!("could not read `{path}`"))?
        .decode()
        .with_context(|| format!("`{path}` is not a PNG or JPEG this build can read"))
}

/// Fit `source` into a `size` x `size` RGBA square.
///
/// `padding` fills the canvas behind a contained image; `None` leaves it transparent.
pub fn squared(
    source: &DynamicImage,
    size: u32,
    fit: Fit,
    padding: Option<Rgba<u8>>,
) -> RgbaImage {
    match fit {
        Fit::Contain => {
            // `resize` preserves the aspect ratio and fits inside the box.
            let scaled = source.resize(size, size, FilterType::Lanczos3).to_rgba8();
            let mut canvas = match padding {
                Some(colour) => RgbaImage::from_pixel(size, size, colour),
                None => RgbaImage::new(size, size),
            };
            let x = (size.saturating_sub(scaled.width()) / 2) as i64;
            let y = (size.saturating_sub(scaled.height()) / 2) as i64;
            imageops::overlay(&mut canvas, &scaled, x, y);
            canvas
        }
        Fit::Cover => {
            let (w, h) = (source.width().max(1) as f32, source.height().max(1) as f32);
            let scale = (size as f32 / w).max(size as f32 / h);
            let new_w = ((w * scale).ceil() as u32).max(size);
            let new_h = ((h * scale).ceil() as u32).max(size);
            let scaled = source.resize_exact(new_w, new_h, FilterType::Lanczos3).to_rgba8();
            let x = (new_w - size) / 2;
            let y = (new_h - size) / 2;
            imageops::crop_imm(&scaled, x, y, size, size).to_image()
        }
    }
}

/// Write a multi-frame `.ico`. Each frame is stored as PNG, which every current
/// browser and Windows Vista+ understands.
fn write_ico(frames: &[(u32, RgbaImage)], out: &Path) -> Result<()> {
    let mut encoded: Vec<(u32, Vec<u8>)> = Vec::with_capacity(frames.len());

    for (size, frame) in frames {
        let mut bytes = Vec::new();
        frame
            .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
            .with_context(|| format!("could not encode a {size}x{size} icon frame"))?;
        encoded.push((*size, bytes));
    }

    let mut file = Vec::new();
    file.extend_from_slice(&0u16.to_le_bytes()); // reserved
    file.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    file.extend_from_slice(&(encoded.len() as u16).to_le_bytes());

    // Directory entries, then the payloads.
    let mut offset = 6u32 + 16 * encoded.len() as u32;

    for (size, bytes) in &encoded {
        let dimension = if *size >= 256 { 0u8 } else { *size as u8 };
        file.push(dimension); // width (0 means 256)
        file.push(dimension); // height
        file.push(0); // palette size
        file.push(0); // reserved
        file.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        file.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        file.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        file.extend_from_slice(&offset.to_le_bytes());
        offset += bytes.len() as u32;
    }

    for (_, bytes) in &encoded {
        file.extend_from_slice(bytes);
    }

    fs::write(out, file).with_context(|| format!("could not write `{}`", out.display()))?;
    Ok(())
}

// ------------------------------------------------------------------ text helpers

/// Font files that usually ship with the OS, paired (display, body).
const SYSTEM_FONT_PAIRS: [(&str, &str); 4] = [
    (
        r"C:\Windows\Fonts\segoeuib.ttf",
        r"C:\Windows\Fonts\segoeui.ttf",
    ),
    (
        r"C:\Windows\Fonts\arialbd.ttf",
        r"C:\Windows\Fonts\arial.ttf",
    ),
    (
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ),
    (
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ),
];

/// The system UI font, if one of the usual suspects exists.
pub fn system_font() -> Option<PathBuf> {
    SYSTEM_FONT_PAIRS
        .iter()
        .flat_map(|(display, body)| [display, body])
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

/// The system display/body pair. When no regular weight sits beside the bold one,
/// the bold face is used for both, so a missing sibling is not an error.
fn system_fonts() -> Option<(PathBuf, PathBuf)> {
    for (display, body) in SYSTEM_FONT_PAIRS {
        let display = PathBuf::from(display);
        if !display.is_file() {
            continue;
        }

        let body = PathBuf::from(body);
        let body = if body.is_file() {
            body
        } else {
            display.clone()
        };
        return Some((display, body));
    }

    None
}

/// The two faces a card draws with: headings in `display`, supporting copy in
/// `body`. When the caller passes one font file, it is used for both.
struct Fonts {
    display: Font,
    body: Option<Font>,
}

impl Fonts {
    fn body(&self) -> &Font {
        self.body.as_ref().unwrap_or(&self.display)
    }
}

fn try_font(path: &Path) -> Option<Font> {
    let bytes = fs::read(path).ok()?;
    Font::from_bytes(bytes, FontSettings::default()).ok()
}

/// Load the display and body faces, falling back to the system pair.
fn load_fonts(explicit: Option<&str>) -> Result<Option<Fonts>> {
    if let Some(path) = explicit {
        let Some(display) = load_font(Some(path))? else {
            return Ok(None);
        };
        return Ok(Some(Fonts {
            display,
            body: None,
        }));
    }

    let Some((display_path, body_path)) = system_fonts() else {
        return Ok(None);
    };
    let Some(display) = try_font(&display_path) else {
        return Ok(None);
    };
    let body = try_font(&body_path);

    Ok(Some(Fonts { display, body }))
}

/// Load a `.ttf`/`.otf`, falling back to the system font.
pub fn load_font(explicit: Option<&str>) -> Result<Option<Font>> {
    let path = match explicit {
        Some(path) => PathBuf::from(path),
        None => match system_font() {
            Some(path) => path,
            None => return Ok(None),
        },
    };

    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if explicit.is_some() => {
            return Err(anyhow!("could not read font `{}`: {err}", path.display()))
        }
        Err(_) => return Ok(None),
    };

    match Font::from_bytes(bytes, FontSettings::default()) {
        Ok(font) => Ok(Some(font)),
        Err(err) if explicit.is_some() => {
            Err(anyhow!("could not parse font `{}`: {err}", path.display()))
        }
        Err(_) => Ok(None),
    }
}

/// Wrapped lines sit this far apart, as a multiple of the font size. Display type
/// wants tighter leading than body copy; 1.12 reads as one block without the
/// descenders of a line colliding with the caps of the next.
const TITLE_SPACING: f32 = 1.12;

/// Supporting copy wants a little more air between lines than a headline.
const SUBTITLE_SPACING: f32 = 1.25;

/// A social card is glanced at, not read: cap the lines and mark the cut.
const MAX_TITLE_LINES: usize = 3;
const MAX_SUBTITLE_LINES: usize = 2;

/// Auto-fitting never goes below this fraction of the base size. In practice the
/// line cap does the work at 1200x630; this only matters on short canvases, where
/// a full block would otherwise be taller than the card.
const MIN_SCALE: f32 = 0.55;

/// Turn the two-character `\n` escape into a real line break, and drop carriage
/// returns so pasted Windows text does not leave stray glyphs.
fn unescape(text: &str) -> String {
    text.replace("\\r\\n", "\n")
        .replace("\\n", "\n")
        .replace('\r', "")
}

/// Width of `text` at `px`, from advance widths so it matches what gets drawn.
fn measure(font: &Font, text: &str, px: f32) -> f32 {
    text.chars()
        .map(|character| font.metrics(character, px).advance_width)
        .sum()
}

/// Greedy word wrapping, honouring hard breaks and splitting words that are
/// wider than the column.
fn wrap(font: &Font, text: &str, px: f32, width: f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();

    for paragraph in text.split('\n') {
        let mut line = String::new();

        for word in paragraph.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };

            if measure(font, &candidate, px) <= width {
                line = candidate;
                continue;
            }

            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }

            // A single word wider than the column: break it by characters rather
            // than let it run off the card.
            if measure(font, word, px) > width {
                let mut chunk = String::new();
                for character in word.chars() {
                    let mut next = chunk.clone();
                    next.push(character);
                    if !chunk.is_empty() && measure(font, &next, px) > width {
                        lines.push(std::mem::take(&mut chunk));
                    }
                    chunk.push(character);
                }
                line = chunk;
            } else {
                line = word.to_string();
            }
        }

        if !line.is_empty() {
            lines.push(line);
        }
    }

    lines
}

/// Narrow the column to the smallest width that keeps the same number of lines.
/// This is `text-wrap: balance`: a two-line headline splits into two full lines
/// instead of leaving a lonely word on the second.
fn balanced(font: &Font, text: &str, px: f32, width: f32, max_lines: usize) -> Vec<String> {
    let lines = wrap(font, text, px, width);
    if lines.len() < 2 || lines.len() > max_lines {
        return lines;
    }

    let count = lines.len();
    let widest_word = text
        .split_whitespace()
        .map(|word| measure(font, word, px))
        .fold(0.0_f32, f32::max)
        .min(width);

    let (mut lo, mut hi) = (widest_word, width);
    for _ in 0..12 {
        let mid = (lo + hi) / 2.0;
        if wrap(font, text, px, mid).len() <= count {
            hi = mid;
        } else {
            lo = mid;
        }
    }

    let balanced = wrap(font, text, px, hi);
    if balanced.len() == count {
        balanced
    } else {
        lines
    }
}

/// Keep at most `max` lines, marking the cut with an ellipsis.
fn cap(
    font: &Font,
    mut lines: Vec<String>,
    px: f32,
    width: f32,
    max: usize,
) -> (Vec<String>, bool) {
    if lines.len() <= max {
        return (lines, false);
    }

    lines.truncate(max);
    let last = lines.last_mut().expect("max is at least one");
    loop {
        let candidate = format!("{}…", last.trim_end());
        if measure(font, &candidate, px) <= width {
            *last = candidate;
            break;
        }
        match last.rfind(' ') {
            Some(space) => last.truncate(space),
            None => {
                *last = candidate;
                break;
            }
        }
    }

    (lines, true)
}

/// Lay out lines that have already been broken. Soft wrapping is off so fontdue
/// cannot second-guess the wrapper; hard breaks still split on `\n`.
fn layout_lines(font: &Font, lines: &[String], px: f32, spacing: f32) -> Layout {
    let natural = font
        .horizontal_line_metrics(px)
        .map(|metrics| metrics.new_line_size / px)
        .filter(|ratio| ratio.is_finite() && *ratio > 0.1)
        .unwrap_or(1.0);

    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings {
        max_width: None,
        line_height: spacing / natural,
        wrap_hard_breaks: true,
        ..Default::default()
    });
    layout.append(&[font], &TextStyle::new(&lines.join("\n"), px, 0));
    layout
}

/// Top and bottom of the actual ink, so blocks can be centred precisely.
fn ink_bounds(layout: &Layout) -> Option<(f32, f32)> {
    let mut top = f32::MAX;
    let mut bottom = f32::MIN;
    for glyph in layout.glyphs() {
        // A hard break still produces a glyph, and its `.notdef` box would
        // otherwise show up at the end of the line and inflate the ink bounds.
        if glyph.parent.is_control() || glyph.width == 0 || glyph.height == 0 {
            continue;
        }
        top = top.min(glyph.y);
        bottom = bottom.max(glyph.y + glyph.height as f32);
    }

    (top <= bottom).then_some((top, bottom))
}

fn ink_height(layout: &Layout) -> f32 {
    ink_bounds(layout)
        .map(|(top, bottom)| bottom - top)
        .unwrap_or(0.0)
}

/// One paragraph, already wrapped and laid out.
struct Prepared {
    layout: Layout,
    /// Height of the inked pixels, not the line box.
    height: f32,
    /// Width of the longest line, used to centre the logo + text group.
    width: f32,
}

impl Prepared {
    /// Draw with the ink starting exactly at `top`. Returns the ink height.
    fn draw(
        &self,
        canvas: &mut RgbaImage,
        font: &Font,
        left: f32,
        top: f32,
        colour: Rgba<u8>,
    ) -> f32 {
        let Some((ink_top, ink_bottom)) = ink_bounds(&self.layout) else {
            return 0.0;
        };

        for glyph in self.layout.glyphs() {
            if glyph.parent.is_control() {
                continue;
            }

            let (metrics, coverage) = font.rasterize_config(glyph.key);
            if metrics.width == 0 || metrics.height == 0 {
                continue;
            }

            let mut mask = RgbaImage::new(metrics.width as u32, metrics.height as u32);
            for (index, alpha) in coverage.iter().enumerate() {
                if *alpha == 0 {
                    continue;
                }
                let scaled = (*alpha as u16 * colour.0[3] as u16 / 255) as u8;
                mask.put_pixel(
                    (index % metrics.width) as u32,
                    (index / metrics.width) as u32,
                    Rgba([colour.0[0], colour.0[1], colour.0[2], scaled]),
                );
            }

            let x = (left + glyph.x) as i64;
            let y = (top + glyph.y - ink_top) as i64;
            imageops::overlay(canvas, &mask, x, y);
        }

        ink_bottom - ink_top
    }
}

fn prepare(font: &Font, lines: &[String], px: f32, spacing: f32) -> Prepared {
    let layout = layout_lines(font, lines, px, spacing);
    let height = ink_height(&layout);
    let width = lines
        .iter()
        .map(|line| measure(font, line, px))
        .fold(0.0_f32, f32::max);

    Prepared {
        layout,
        height,
        width,
    }
}

/// A wrapped, fitted pair of paragraphs.
struct Fitted {
    title: Prepared,
    subtitle: Prepared,
    gap: f32,
    height: f32,
    truncated: bool,
}

/// Where everything on the card goes.
struct CardLayout {
    pad: f32,
    logo_size: u32,
    logo_left: i64,
    text_left: f32,
    title: Prepared,
    subtitle: Prepared,
    gap: f32,
    height: f32,
    truncated: bool,
}

/// Wrap, balance and size both paragraphs at one scale.
#[allow(clippy::too_many_arguments)]
fn fit_at(
    fonts: &Fonts,
    title: &str,
    subtitle: &str,
    max_text: f32,
    base_title: f32,
    base_subtitle: f32,
    scale: f32,
) -> Fitted {
    let title_px = base_title * scale;
    let subtitle_px = base_subtitle * scale;

    let (title_lines, cut_title) = if title.is_empty() {
        (Vec::new(), false)
    } else {
        cap(
            &fonts.display,
            balanced(&fonts.display, title, title_px, max_text, MAX_TITLE_LINES),
            title_px,
            max_text,
            MAX_TITLE_LINES,
        )
    };

    let (subtitle_lines, cut_subtitle) = if subtitle.is_empty() {
        (Vec::new(), false)
    } else {
        cap(
            fonts.body(),
            balanced(
                fonts.body(),
                subtitle,
                subtitle_px,
                max_text,
                MAX_SUBTITLE_LINES,
            ),
            subtitle_px,
            max_text,
            MAX_SUBTITLE_LINES,
        )
    };

    let title_block = prepare(&fonts.display, &title_lines, title_px, TITLE_SPACING);
    let subtitle_block = prepare(fonts.body(), &subtitle_lines, subtitle_px, SUBTITLE_SPACING);
    let gap = if title_block.height > 0.0 && subtitle_block.height > 0.0 {
        title_px * 0.5
    } else {
        0.0
    };
    let height = title_block.height + gap + subtitle_block.height;

    Fitted {
        title: title_block,
        subtitle: subtitle_block,
        gap,
        height,
        truncated: cut_title || cut_subtitle,
    }
}

/// Wrap, balance, fit and place the copy. The order matters: balance before the
/// line cap (so the cut is made on good breaks), then shrink if the block is
/// still too tall for the canvas.
fn lay_out(
    fonts: &Fonts,
    title: &str,
    subtitle: &str,
    width: u32,
    height: u32,
    bar: u32,
) -> CardLayout {
    // Type and logo scale with the short side, so a portrait or a very wide
    // canvas keeps the same proportions instead of growing one dimension only.
    // The logo also gives ground as the card approaches square, because a big
    // square logo beside a narrower column is what forces short titles onto a
    // fourth line.
    let short = width.min(height) as f32;
    let pad_x = ((width as f32) * 0.075).round().max(24.0);
    let pad_y = ((height as f32) * 0.14).round().max(18.0);
    let aspect = width as f32 / height as f32;
    let logo_ratio = 0.22 + 0.14 * ((aspect - 1.0) / 0.6).clamp(0.0, 1.0);
    let logo_size = (short * logo_ratio).round().max(1.0) as u32;

    let left_edge = bar as f32 + pad_x;
    let right_edge = width as f32 - pad_x;
    let max_text = (right_edge - left_edge - logo_size as f32 - pad_x).max(40.0);

    let base_title = short * 0.112;
    let base_subtitle = short * 0.052;
    let budget = (height as f32 - pad_y * 2.0).max(40.0);

    let mut scale = 1.0_f32;
    let mut fitted = fit_at(
        fonts,
        title,
        subtitle,
        max_text,
        base_title,
        base_subtitle,
        scale,
    );

    // Too tall: shrink until it fits, or until the floor.
    while fitted.height > budget && scale > MIN_SCALE {
        scale *= 0.94;
        fitted = fit_at(
            fonts,
            title,
            subtitle,
            max_text,
            base_title,
            base_subtitle,
            scale,
        );
    }

    // The block fits but a line was cut. A small reduction often shows the
    // whole thing, which beats an ellipsis; the 15% ceiling keeps that from
    // turning the headline noticeably small. If it cannot fit even then, keep
    // the full-size version and let the warning stand.
    if fitted.truncated {
        let floor = (scale * 0.85).max(MIN_SCALE);
        while scale > floor {
            scale *= 0.95;
            let trial = fit_at(
                fonts,
                title,
                subtitle,
                max_text,
                base_title,
                base_subtitle,
                scale,
            );
            if !trial.truncated {
                fitted = trial;
                break;
            }
        }
    }

    // Centre the logo and the text as one group, so a short headline does not
    // leave the pair stranded against the left edge.
    let text_width = fitted.title.width.max(fitted.subtitle.width);
    let group = logo_size as f32 + pad_x + text_width;
    let available = right_edge - left_edge;
    let logo_left = left_edge + ((available - group) / 2.0).max(0.0);
    let text_left = logo_left + logo_size as f32 + pad_x;

    CardLayout {
        pad: pad_y,
        logo_size,
        logo_left: logo_left as i64,
        text_left,
        title: fitted.title,
        subtitle: fitted.subtitle,
        gap: fitted.gap,
        height: fitted.height,
        truncated: fitted.truncated,
    }
}

// ------------------------------------------------------------------ colour helpers

/// Parse `#rgb`, `#rgba`, `#rrggbb` or `#rrggbbaa`.
pub fn parse_color(spec: &str) -> Result<Rgba<u8>> {
    let hex = spec.trim().trim_start_matches('#');

    let mut digits = Vec::with_capacity(hex.len());
    for character in hex.chars() {
        match character.to_digit(16) {
            Some(digit) => digits.push(digit as u8),
            None => bail!("`{spec}` is not a hex colour like #ff9f1c"),
        }
    }

    let pair = |hi: u8, lo: u8| hi * 16 + lo;

    let (r, g, b, a) = match digits.len() {
        3 => (digits[0] * 17, digits[1] * 17, digits[2] * 17, 255),
        4 => (
            digits[0] * 17,
            digits[1] * 17,
            digits[2] * 17,
            digits[3] * 17,
        ),
        6 => (
            pair(digits[0], digits[1]),
            pair(digits[2], digits[3]),
            pair(digits[4], digits[5]),
            255,
        ),
        8 => (
            pair(digits[0], digits[1]),
            pair(digits[2], digits[3]),
            pair(digits[4], digits[5]),
            pair(digits[6], digits[7]),
        ),
        _ => bail!("`{spec}` is not a hex colour like #ff9f1c"),
    };

    Ok(Rgba([r, g, b, a]))
}

fn hex(colour: Rgba<u8>) -> String {
    format!(
        "#{:02x}{:02x}{:02x}",
        colour.0[0], colour.0[1], colour.0[2]
    )
}

fn luminance(colour: Rgba<u8>) -> f32 {
    0.2126 * colour.0[0] as f32 + 0.7152 * colour.0[1] as f32 + 0.0722 * colour.0[2] as f32
}

/// Linear blend from `from` towards `to`.
fn blend(from: Rgba<u8>, to: Rgba<u8>, amount: f32) -> Rgba<u8> {
    let channel = |a: u8, b: u8| {
        (a as f32 + (b as f32 - a as f32) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    };

    Rgba([
        channel(from.0[0], to.0[0]),
        channel(from.0[1], to.0[1]),
        channel(from.0[2], to.0[2]),
        255,
    ])
}

// ------------------------------------------------------------------ misc helpers

/// Parse `1200x630` into a width and height.
pub fn parse_size(spec: &str) -> Result<(u32, u32)> {
    let (width, height) = spec
        .split_once(['x', 'X'])
        .ok_or_else(|| anyhow!("expected a size like 1200x630, got `{spec}`"))?;

    let width: u32 = width
        .trim()
        .parse()
        .with_context(|| format!("bad width in `{spec}`"))?;
    let height: u32 = height
        .trim()
        .parse()
        .with_context(|| format!("bad height in `{spec}`"))?;

    // Below this the logo and text column stop fitting side by side; above it a
    // "social card" is a billboard and the RGBA buffer gets very large.
    const MIN_SIDE: u32 = 240;
    const MAX_SIDE: u32 = 8192;

    if width < MIN_SIDE || height < MIN_SIDE {
        bail!("`{spec}` is too small; each side must be at least {MIN_SIDE}px");
    }
    if width > MAX_SIDE || height > MAX_SIDE {
        bail!("`{spec}` is too large; each side must be at most {MAX_SIDE}px");
    }

    Ok((width, height))
}

fn default_name(image: &str) -> String {
    let stem = Path::new(image)
        .file_stem()
        .map(|name| name.to_string_lossy().replace(['-', '_'], " "))
        .unwrap_or_default();

    let cleaned = stem.trim();
    let mut characters = cleaned.chars();

    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => "App".to_string(),
    }
}

fn short_name(app: &str) -> String {
    app.chars().take(12).collect()
}

fn manifest_json(app: &str, short: &str, theme: &str, bg: &str) -> String {
    format!(
        concat!(
            "{{\n",
            "  \"name\": \"{}\",\n",
            "  \"short_name\": \"{}\",\n",
            "  \"icons\": [\n",
            "    {{ \"src\": \"/android-chrome-192x192.png\", \"sizes\": \"192x192\", \"type\": \"image/png\" }},\n",
            "    {{ \"src\": \"/android-chrome-512x512.png\", \"sizes\": \"512x512\", \"type\": \"image/png\" }},\n",
            "    {{ \"src\": \"/apple-touch-icon.png\", \"sizes\": \"180x180\", \"type\": \"image/png\" }}\n",
            "  ],\n",
            "  \"theme_color\": \"{}\",\n",
            "  \"background_color\": \"{}\",\n",
            "  \"display\": \"standalone\"\n",
            "}}\n"
        ),
        escape_json(app),
        escape_json(short),
        theme,
        bg
    )
}

fn escape_json(value: &str) -> String {
    let mut out = String::with_capacity(value.len());

    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", control as u32))
            }
            other => out.push(other),
        }
    }

    out
}

/// The `<head>` block that goes with a generated icon set.
fn snippet(theme: &str) -> String {
    [
        r#"  <link rel="icon" href="/favicon.ico" sizes="any">"#.to_string(),
        r#"  <link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png">"#
            .to_string(),
        r#"  <link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png">"#
            .to_string(),
        r#"  <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png">"#
            .to_string(),
        r#"  <link rel="manifest" href="/manifest.webmanifest">"#.to_string(),
        format!(r#"  <meta name="theme-color" content="{theme}">"#),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A font to lay text out with. The system font is preferred, so the real
    /// default path is exercised; the bundled Inter is the fallback that keeps
    /// the test deterministic on a machine with no fonts installed.
    fn test_font() -> Font {
        if let Some(path) = system_font() {
            if let Ok(Some(font)) = load_font(path.to_str()) {
                return font;
            }
        }

        Font::from_bytes(
            include_bytes!("../gui/assets/Inter_400Regular.ttf").as_slice(),
            FontSettings::default(),
        )
        .expect("the bundled test font should parse")
    }

    fn test_fonts() -> Fonts {
        Fonts {
            display: test_font(),
            body: None,
        }
    }

    #[test]
    fn wrapped_lines_are_spaced_in_ems_not_the_fonts_natural_leading() {
        let font = test_font();
        let px = 100.0;

        // A hard break keeps this about leading rather than about where the
        // wrapper decides to split. Comparing the same glyph on both lines
        // removes rasterisation differences from the measurement.
        let layout = layout_lines(
            &font,
            &["o".to_string(), "o".to_string()],
            px,
            TITLE_SPACING,
        );

        let mut same = layout.glyphs().iter().filter(|glyph| glyph.parent == 'o');
        let first = same.next().expect("first line should have an `o`");
        let second = same.next().expect("second line should have an `o`");
        let delta = second.y - first.y;

        let natural = font
            .horizontal_line_metrics(px)
            .map(|metrics| metrics.new_line_size / px)
            .unwrap_or(1.0);
        let ratio = delta / px;

        assert!(
            (TITLE_SPACING - 0.06..=TITLE_SPACING + 0.06).contains(&ratio),
            "lines are {ratio:.2}em apart ({delta:.1}px at {px}px); expected about \
             {TITLE_SPACING:.2}em. The font's natural leading is {natural:.2}em, so \
             a raw multiplier of {TITLE_SPACING:.2} would have produced {:.2}em",
            natural * TITLE_SPACING
        );
    }

    #[test]
    fn shell_escaped_and_real_newlines_both_break() {
        assert_eq!(unescape(r"one\ntwo"), "one\ntwo");
        assert_eq!(unescape("one\r\ntwo"), "one\ntwo");
        assert_eq!(unescape("one\\n"), "one\n");
    }

    #[test]
    fn explicit_newlines_are_kept_by_the_wrapper() {
        let font = test_font();
        let lines = wrap(&font, "one\ntwo three", 48.0, 10_000.0);
        assert_eq!(lines, ["one", "two three"]);
    }

    #[test]
    fn balanced_wrapping_evens_out_a_two_line_headline() {
        let font = test_font();
        let px = 64.0;
        let width = 700.0;
        let text = "ship a small binary that does one thing well";

        let greedy = wrap(&font, text, px, width);
        assert!(
            greedy.len() >= 2,
            "test text should need at least two lines"
        );

        let lines = balanced(&font, text, px, width, MAX_TITLE_LINES);
        assert_eq!(lines.len(), greedy.len());

        let widths: Vec<f32> = lines.iter().map(|line| measure(&font, line, px)).collect();
        let widest = widths.iter().cloned().fold(0.0_f32, f32::max);
        let narrowest = widths.iter().cloned().fold(f32::MAX, f32::min);

        assert!(
            widest / narrowest < 1.5,
            "lines are unbalanced: {lines:?} ({widths:?})"
        );
    }

    #[test]
    fn long_copy_is_cut_at_the_line_cap_with_an_ellipsis() {
        let font = test_font();
        let px = 64.0;
        let width = 700.0;
        let text = "one two three four five six seven eight nine ten eleven twelve \
                    thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty";

        let (lines, cut) = cap(
            &font,
            balanced(&font, text, px, width, MAX_TITLE_LINES),
            px,
            width,
            MAX_TITLE_LINES,
        );

        assert!(cut);
        assert_eq!(lines.len(), MAX_TITLE_LINES);
        assert!(lines.last().expect("capped lines").ends_with('…'));
        for line in &lines {
            assert!(
                measure(&font, line, px) <= width + 0.5,
                "`{line}` overflows the column"
            );
        }
    }

    #[test]
    fn a_long_card_uses_its_line_budget_and_still_fits() {
        let fonts = test_fonts();
        let plan = lay_out(
            &fonts,
            "one two three four five six seven eight nine ten eleven twelve thirteen \
             fourteen fifteen sixteen",
            "one two three four five six seven eight nine ten eleven twelve",
            1200,
            630,
            16,
        );

        let budget = 630.0 - plan.pad * 2.0;
        assert!(
            plan.height <= budget,
            "block is {}px tall but the card only budgets {budget}px",
            plan.height
        );
        assert!(plan.truncated, "the long copy should be marked as cut");
    }

    #[test]
    fn a_full_block_shrinks_to_fit_a_short_canvas() {
        let fonts = test_fonts();
        let plan = lay_out(
            &fonts,
            "one two three four five six seven eight nine ten eleven twelve thirteen",
            "one two three four five six seven eight",
            100,
            80,
            6,
        );

        let budget = 80.0 - plan.pad * 2.0;
        assert!(
            plan.height <= budget + 0.5,
            "block is {}px tall but the card only budgets {budget}px",
            plan.height
        );
    }

    #[test]
    fn sizes_are_bounded() {
        assert_eq!(parse_size("1200x630").expect("valid size"), (1200, 630));
        assert!(parse_size("4x4").is_err(), "tiny cards panic the renderer");
        assert!(parse_size("9000x630").is_err(), "absurd size");
        assert!(parse_size("0x630").is_err(), "zero side");
        assert!(parse_size("nope").is_err(), "not a size");
    }
}
