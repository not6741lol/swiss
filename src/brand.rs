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
        /// Big headline.
        #[arg(long)]
        title: Option<String>,
        /// Smaller line under the headline.
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
        /// Big headline for the social image.
        #[arg(long)]
        title: Option<String>,
        /// Smaller line for the social image.
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
}

/// A rendered card, plus anything the caller ought to know about it.
pub struct RenderedCard {
    /// The pixels.
    pub image: RgbaImage,
    /// Set when there was a headline to draw but no usable font to draw it with,
    /// so the card came out logo-only.
    pub font_missing: bool,
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
            warn_missing_font(&card);
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

            warn_missing_font(&social);
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

/// Say so when a card wanted text but had no font to draw it with.
fn warn_missing_font(card: &Card) {
    if card.font_missing {
        eprintln!("note: no system font found, drawing the card without text (pass --font)");
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
    let font = load_font(font_path)?;

    let mut canvas = RgbaImage::from_pixel(width, height, background);

    // Accent bar down the left edge.
    let bar = ((width as f32) * 0.013).round().clamp(6.0, 32.0) as u32;
    for y in 0..height {
        for x in 0..bar {
            canvas.put_pixel(x, y, accent);
        }
    }

    let title = title.trim();
    let subtitle = subtitle.map(str::trim).filter(|text| !text.is_empty());

    let on_dark = luminance(background) < 140.0;
    let primary = if on_dark {
        Rgba([246, 244, 240, 255])
    } else {
        Rgba([22, 22, 26, 255])
    };
    let muted = blend(primary, background, 0.45);

    let mut font_missing = false;

    match (font.as_ref(), title.is_empty()) {
        // Logo on the left, text on the right.
        (Some(font), false) => {
            let pad = ((width as f32) * 0.075).round().max(28.0) as u32;
            let box_size = ((height as f32) * 0.42).round() as u32;

            let logo = squared(&source, box_size, fit, None);
            let logo_x = (bar + pad) as i64;
            let logo_y = (height.saturating_sub(box_size) / 2) as i64;
            imageops::overlay(&mut canvas, &logo, logo_x, logo_y);

            let text_left = (bar + pad + box_size + pad) as f32;
            let text_width = (width as f32 - text_left - pad as f32).max(120.0);

            let title_px = (height as f32) * 0.112;
            let subtitle_px = (height as f32) * 0.052;
            let gap = title_px * 0.5;

            let title_height = ink_height(font, title, title_px, text_width);
            let subtitle_height = subtitle
                .map(|text| ink_height(font, text, subtitle_px, text_width))
                .unwrap_or(0.0);
            let block = title_height + if subtitle.is_some() { gap + subtitle_height } else { 0.0 };

            let mut cursor = ((height as f32 - block) / 2.0).max(pad as f32);
            cursor += draw_text(
                &mut canvas,
                font,
                title,
                title_px,
                text_left,
                cursor,
                text_width,
                primary,
            );

            if let Some(subtitle) = subtitle {
                cursor += gap;
                draw_text(
                    &mut canvas,
                    font,
                    subtitle,
                    subtitle_px,
                    text_left,
                    cursor,
                    text_width,
                    muted,
                );
            }
        }

        // Nothing to say, or no font available: just present the logo.
        _ => {
            font_missing = font.is_none() && !title.is_empty();

            let box_size = ((width.min(height) as f32) * 0.62).round() as u32;
            let logo = squared(&source, box_size, fit, None);
            let x = (width.saturating_sub(box_size) / 2) as i64;
            let y = (height.saturating_sub(box_size) / 2) as i64;
            imageops::overlay(&mut canvas, &logo, x, y);
        }
    }

    Ok(RenderedCard {
        image: canvas,
        font_missing,
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

/// The system UI font, if one of the usual suspects exists.
pub fn system_font() -> Option<PathBuf> {
    const CANDIDATES: [&str; 8] = [
        r"C:\Windows\Fonts\segoeuib.ttf",
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\arialbd.ttf",
        r"C:\Windows\Fonts\arial.ttf",
        "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ];

    CANDIDATES
        .iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
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

fn build_layout(font: &Font, text: &str, px: f32, max_width: f32) -> Layout {
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.reset(&LayoutSettings {
        max_width: Some(max_width),
        line_height: 1.2,
        ..Default::default()
    });
    layout.append(&[font], &TextStyle::new(text, px, 0));
    layout
}

/// Top and bottom of the actual ink, so blocks can be centred precisely.
fn ink_bounds(font: &Font, text: &str, px: f32, max_width: f32) -> Option<(f32, f32)> {
    let layout = build_layout(font, text, px, max_width);

    let mut top = f32::MAX;
    let mut bottom = f32::MIN;
    for glyph in layout.glyphs() {
        if glyph.width == 0 || glyph.height == 0 {
            continue;
        }
        top = top.min(glyph.y);
        bottom = bottom.max(glyph.y + glyph.height as f32);
    }

    (top <= bottom).then_some((top, bottom))
}

fn ink_height(font: &Font, text: &str, px: f32, max_width: f32) -> f32 {
    ink_bounds(font, text, px, max_width)
        .map(|(top, bottom)| bottom - top)
        .unwrap_or(0.0)
}

/// Draw wrapped text with its ink starting exactly at `top`. Returns the ink height.
#[allow(clippy::too_many_arguments)]
fn draw_text(
    canvas: &mut RgbaImage,
    font: &Font,
    text: &str,
    px: f32,
    left: f32,
    top: f32,
    max_width: f32,
    colour: Rgba<u8>,
) -> f32 {
    let layout = build_layout(font, text, px, max_width);

    let Some((ink_top, ink_bottom)) = ink_bounds(font, text, px, max_width) else {
        return 0.0;
    };

    for glyph in layout.glyphs() {
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

    if width == 0 || height == 0 {
        bail!("`{spec}` has a zero side");
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
