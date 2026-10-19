//! The Brand tab: one image in, a whole web branding set out.
//!
//! The social card renders into memory on every edit, so the preview is the real
//! output rather than an approximation of it — the same function that writes the
//! file draws what you see.

use std::path::{Path, PathBuf};

use eframe::egui::{self, RichText};

use crate::theme;
use crate::ui::{self, Log};
use swiss::brand::{self, Fit};

/// Which of the three jobs is showing.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    Favicon,
    Card,
    Full,
}

/// Everything the Brand tab remembers between frames.
pub struct BrandTab {
    mode: Mode,

    // --- source
    image: String,
    fit: Fit,

    // --- icon set
    app: String,
    theme_colour: String,
    bg: String,
    out_dir: String,

    // --- social card
    title: String,
    subtitle: String,
    size: String,
    card_bg: String,
    accent: String,
    font: String,
    card_out: String,

    // --- results
    preview: Option<egui::TextureHandle>,
    preview_key: String,
    preview_error: Option<String>,
    preview_note: Option<String>,
    generated: Option<Vec<PathBuf>>,
    snippet: Option<String>,
}

/// `Fit` lives in the library crate and has no `Default` of its own, and the orphan
/// rule stops us adding one here, so the struct spells out its own defaults.
impl Default for BrandTab {
    fn default() -> Self {
        Self {
            mode: Mode::default(),
            image: String::new(),
            fit: Fit::Contain,
            app: String::new(),
            theme_colour: String::new(),
            bg: String::new(),
            out_dir: String::new(),
            title: String::new(),
            subtitle: String::new(),
            size: String::new(),
            card_bg: String::new(),
            accent: String::new(),
            font: String::new(),
            card_out: String::new(),
            preview: None,
            preview_key: String::new(),
            preview_error: None,
            preview_note: None,
            generated: None,
            snippet: None,
        }
    }
}

impl BrandTab {
    pub fn show(&mut self, ui: &mut egui::Ui, log: &mut Log, ctx: &egui::Context) {
        self.fill_defaults();

        ui::heading(ui, "brand");
        ui::hint(
            ui,
            "One image becomes the favicon set, a web manifest, the paste-in HTML and a 1200x630 social card. All rendered locally.",
        );

        ui.add_space(12.0);

        ui.horizontal(|ui| {
            for (mode, label) in [
                (Mode::Favicon, "favicons"),
                (Mode::Card, "social card"),
                (Mode::Full, "everything"),
            ] {
                if ui::pill(ui, label, self.mode == mode).clicked() {
                    self.mode = mode;
                    self.preview_key.clear();
                }
            }
        });

        ui.add_space(14.0);

        ui::card(ui, |ui| self.settings_ui(ui));

        ui.add_space(14.0);

        self.refresh_preview(ctx);

        ui::card(ui, |ui| self.preview_ui(ui, log));

        if self.generated.is_some() || self.snippet.is_some() {
            ui.add_space(14.0);
            ui::card(ui, |ui| self.output_ui(ui));
        }
    }

    /// Sensible starting values, applied once.
    fn fill_defaults(&mut self) {
        if self.theme_colour.is_empty() {
            // The site's own colours, so the first run already looks like home.
            self.theme_colour = "#0e0e10".to_string();
            self.bg = "#ffffff".to_string();
            self.card_bg = "#111013".to_string();
            self.accent = "#ff9f1c".to_string();
            self.size = "1200x630".to_string();
            self.out_dir = "public".to_string();
            self.card_out = "og.png".to_string();
        }
    }

    // ----------------------------------------------------------- settings

    fn settings_ui(&mut self, ui: &mut egui::Ui) {
        ui::heading(ui, "the image");
        ui.add_space(8.0);

        if picker(ui, &mut self.image, r"C:\path\to\logo.png", "browse…") {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Source image")
                .add_filter("Image", &["png", "jpg", "jpeg"])
                .pick_file()
            {
                self.image = path.display().to_string();
            }
        }

        ui.add_space(12.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("non-square source").size(12.0).color(theme::FAINT));
            for (fit, label) in [(Fit::Contain, "contain"), (Fit::Cover, "cover")] {
                if ui::pill(ui, label, self.fit == fit).clicked() {
                    self.fit = fit;
                }
            }
        });

        ui::hint(
            ui,
            "contain pads the square with transparency, cover crops the overflow",
        );

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(14.0);

        match self.mode {
            Mode::Favicon => self.favicon_settings(ui),
            Mode::Card => self.card_settings(ui),
            Mode::Full => {
                self.favicon_settings(ui);
                ui.add_space(16.0);
                ui.separator();
                ui.add_space(14.0);
                self.card_settings(ui);
            }
        }
    }

    fn favicon_settings(&mut self, ui: &mut egui::Ui) {
        ui::heading(ui, "icon set");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("app name").size(12.0).color(theme::FAINT));
            ui.add(
                egui::TextEdit::singleline(&mut self.app)
                    .hint_text("defaults to the image's file name")
                    .desired_width(280.0),
            );
        });

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("theme-color").size(12.0).color(theme::FAINT));
            ui::colour_field(ui, &mut self.theme_colour);
            ui.add_space(16.0);
            ui.label(RichText::new("background").size(12.0).color(theme::FAINT));
            ui::colour_field(ui, &mut self.bg);
        });

        ui.add_space(12.0);
        label(ui, "output directory");
        if picker(ui, &mut self.out_dir, "public", "choose…") {
            if let Some(dir) = rfd::FileDialog::new()
                .set_title("Where should the icon set go?")
                .pick_folder()
            {
                self.out_dir = dir.display().to_string();
            }
        }
    }

    fn card_settings(&mut self, ui: &mut egui::Ui) {
        ui::heading(ui, "social card");
        ui.add_space(10.0);

        label(ui, "headline");
        ui.add(
            egui::TextEdit::multiline(&mut self.title)
                .hint_text("one small binary")
                .desired_rows(1)
                .desired_width(ui.available_width()),
        );

        ui.add_space(10.0);
        label(ui, "subtitle");
        ui.add(
            egui::TextEdit::multiline(&mut self.subtitle)
                .hint_text("three small jobs")
                .desired_rows(1)
                .desired_width(ui.available_width()),
        );
        ui::hint(
            ui,
            "enter starts a new line; the card wraps and balances the rest",
        );

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("background").size(12.0).color(theme::FAINT));
            ui::colour_field(ui, &mut self.card_bg);
            ui.add_space(16.0);
            ui.label(RichText::new("accent").size(12.0).color(theme::FAINT));
            ui::colour_field(ui, &mut self.accent);
        });

        if self.mode == Mode::Full {
            ui.add_space(6.0);
            ui::hint(
                ui,
                "\"everything\" reuses theme-color as the card background, exactly like the command line",
            );
        }

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("size").size(12.0).color(theme::FAINT));
            ui.add(
                egui::TextEdit::singleline(&mut self.size)
                    .desired_width(110.0)
                    .font(egui::TextStyle::Monospace),
            );

            let fits = brand::parse_size(&self.size).is_ok();
            if !fits {
                ui.label(
                    RichText::new("WIDTHxHEIGHT")
                        .size(12.0)
                        .color(theme::ERR),
                );
            }
        });

        ui.add_space(12.0);
        label(ui, "font file (optional)");
        if picker(ui, &mut self.font, "defaults to the system UI font", "browse…") {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("A .ttf or .otf for the card text")
                .add_filter("Font", &["ttf", "otf"])
                .pick_file()
            {
                self.font = path.display().to_string();
            }
        }

        if self.mode == Mode::Card {
            ui.add_space(12.0);
            label(ui, "save as");
            if picker(ui, &mut self.card_out, "og.png", "save as…") {
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Where should the card go?")
                    .set_file_name("og.png")
                    .add_filter("PNG", &["png"])
                    .save_file()
                {
                    self.card_out = path.display().to_string();
                }
            }
        }
    }

    // ------------------------------------------------------------ preview

    /// Re-render whenever something the preview depends on has changed.
    fn refresh_preview(&mut self, ctx: &egui::Context) {
        let key = self.preview_key();

        if key.is_empty() {
            self.preview = None;
            self.preview_key.clear();
            self.preview_error = None;
            self.preview_note = None;
            return;
        }

        if key == self.preview_key && self.preview.is_some() {
            return;
        }

        match self.render_preview() {
            Ok(card) => {
                let image = card.image;
                let pixels = egui::ColorImage::from_rgba_unmultiplied(
                    [image.width() as usize, image.height() as usize],
                    image.as_raw(),
                );

                let texture =
                    ctx.load_texture("brand-preview", pixels, egui::TextureOptions::LINEAR);
                self.preview = Some(texture);
                self.preview_key = key;
                self.preview_error = None;
                self.preview_note = if card.truncated {
                    Some("copy is cut at 3 headline / 2 subtitle lines".to_string())
                } else {
                    None
                };
            }

            Err(err) => {
                self.preview = None;
                self.preview_key = key;
                self.preview_error = Some(format!("{err:#}"));
                self.preview_note = None;
            }
        }
    }

    /// A fingerprint of everything the preview is built from.
    fn preview_key(&self) -> String {
        if self.image.trim().is_empty() {
            return String::new();
        }

        match self.mode {
            Mode::Favicon => format!(
                "icon|{}|{}|{:?}|{}",
                self.image, self.bg, self.fit, self.theme_colour
            ),
            Mode::Card => format!(
                "card|{}|{}|{}|{}|{}|{}|{}|{:?}|{}",
                self.image,
                self.title,
                self.subtitle,
                self.size,
                self.card_bg,
                self.accent,
                self.font,
                self.fit,
                self.theme_colour
            ),
            Mode::Full => format!(
                "full|{}|{}|{}|{}|{}|{}|{}|{:?}|{}|{}",
                self.image,
                self.title,
                self.subtitle,
                self.size,
                self.theme_colour,
                self.accent,
                self.font,
                self.fit,
                self.bg,
                self.app
            ),
        }
    }

    fn render_preview(&self) -> anyhow::Result<brand::RenderedCard> {
        if self.mode == Mode::Favicon {
            let source = image::open(self.image.trim())?;
            let bg = brand::parse_color(&self.bg)?;
            return Ok(brand::RenderedCard {
                image: brand::squared(&source, 384, self.fit, Some(bg)),
                font_missing: false,
                truncated: false,
            });
        }

        let (width, height) = brand::parse_size(&self.size)?;

        // "everything" paints the card on the manifest's theme colour, which is
        // what the CLI does too — the two must not disagree.
        let bg = if self.mode == Mode::Full {
            &self.theme_colour
        } else {
            &self.card_bg
        };

        let font = self.font.trim();
        let font = (!font.is_empty()).then_some(font);

        let rendered = brand::render_card(
            self.image.trim(),
            &self.title,
            Some(self.subtitle.as_str()),
            width,
            height,
            bg,
            &self.accent,
            font,
            self.fit,
        )?;

        Ok(rendered)
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui, log: &mut Log) {
        ui::heading(ui, "preview");
        ui.add_space(10.0);

        if let Some(message) = &self.preview_error {
            ui.label(RichText::new(message).size(12.5).color(theme::ERR));
            return;
        }

        match &self.preview {
            None => {
                ui::hint(ui, "pick an image above and it will appear here as you type");
            }

            Some(texture) => {
                let natural = texture.size_vec2();
                let width = ui.available_width().min(620.0);
                let scale = width / natural.x;

                let shown = egui::vec2(width, natural.y * scale);
                ui.add(egui::Image::new(texture).fit_to_exact_size(shown));

                if self.mode != Mode::Favicon {
                    ui.add_space(6.0);
                    ui::hint(
                        ui,
                        &format!(
                            "shown at {:.0}% of {}x{}",
                            scale * 100.0,
                            natural.x as u32,
                            natural.y as u32
                        ),
                    );
                }

                if let Some(note) = &self.preview_note {
                    ui.add_space(4.0);
                    ui.label(RichText::new(note).size(12.0).color(theme::ACCENT));
                }
            }
        }

        ui.add_space(16.0);

        if ui::primary(ui, self.action_label(), self.can_run()).clicked() {
            self.run(log);
        }
    }

    fn action_label(&self) -> &'static str {
        match self.mode {
            Mode::Favicon => "build the icon set",
            Mode::Card => "render the card",
            Mode::Full => "build everything",
        }
    }

    fn can_run(&self) -> bool {
        !self.image.trim().is_empty() && self.preview_error.is_none()
    }

    // ------------------------------------------------------------- output

    /// Do the work, straight through the same functions the CLI calls.
    fn run(&mut self, log: &mut Log) {
        log.clear();
        self.generated = None;
        self.snippet = None;

        let image = self.image.trim().to_string();
        let fit = self.fit;

        match self.mode {
            Mode::Favicon => {
                let out = self.out_dir.trim().to_string();
                let name = self.app.trim();
                let name = (!name.is_empty()).then_some(name);

                log.info(format!("brand favicon {image} --out {out}"));

                match brand::favicons(
                    &image,
                    Path::new(&out),
                    name,
                    &self.theme_colour,
                    &self.bg,
                    fit,
                ) {
                    Ok(set) => {
                        for entry in &set.entries {
                            log.info(format!("  {}", entry.path.display()));
                        }
                        log.ok(format!(
                            "{} file(s) written to {}",
                            set.entries.len(),
                            set.dir.display()
                        ));
                        self.generated = Some(set.entries.iter().map(|e| e.path.clone()).collect());
                        self.snippet = Some(set.snippet);
                    }
                    Err(err) => log.err(format!("{err:#}")),
                }
            }

            Mode::Card => {
                let out = self.card_out.trim().to_string();
                let font = self.font.trim();
                let font = (!font.is_empty()).then_some(font);

                log.info(format!("brand og {image} --out {out}"));

                match self.card(&image, Path::new(&out), font) {
                    Ok(card) => {
                        if card.font_missing {
                            log.err("no font found — the card came out logo-only");
                        }
                        if card.truncated {
                            log.info("copy was cut at 3 headline / 2 subtitle lines");
                        }
                        log.ok(format!(
                            "social card : {} ({}x{})",
                            card.out.display(),
                            card.width,
                            card.height
                        ));
                        self.generated = Some(vec![card.out.clone()]);
                    }
                    Err(err) => log.err(format!("{err:#}")),
                }
            }

            Mode::Full => {
                let dir = PathBuf::from(self.out_dir.trim());
                let name = self.app.trim();
                let name = (!name.is_empty()).then_some(name);

                log.info(format!("brand full {image} --out {}", dir.display()));

                let favicons = brand::favicons(
                    &image,
                    &dir,
                    name,
                    &self.theme_colour,
                    &self.bg,
                    fit,
                );

                match favicons {
                    Ok(set) => {
                        for entry in &set.entries {
                            log.info(format!("  {}", entry.path.display()));
                        }
                        log.ok(format!("{} file(s) written to {}", set.entries.len(), set.dir.display()));
                        let mut written: Vec<PathBuf> =
                            set.entries.iter().map(|e| e.path.clone()).collect();

                        let social = dir.join("og.png");
                        let font = self.font.trim();
                        let font = (!font.is_empty()).then_some(font);

                        match self.card(&image, &social, font) {
                            Ok(card) => {
                                if card.font_missing {
                                    log.err("no font found — the card came out logo-only");
                                }
                                if card.truncated {
                                    log.info("copy was cut at 3 headline / 2 subtitle lines");
                                }
                                log.ok(format!(
                                    "social card : {} ({}x{})",
                                    card.out.display(),
                                    card.width,
                                    card.height
                                ));
                                written.push(card.out);
                                self.generated = Some(written);
                            }
                            Err(err) => {
                                log.err(format!("{err:#}"));
                                self.generated = Some(written);
                            }
                        }

                        self.snippet = Some(set.snippet);
                    }
                    Err(err) => log.err(format!("{err:#}")),
                }
            }
        }
    }

    /// The full-size card for this tab's settings, on the "full" size.
    fn card(
        &self,
        image: &str,
        out: &Path,
        font: Option<&str>,
    ) -> anyhow::Result<brand::Card> {
        let (width, height) = match self.mode {
            Mode::Full => (1200, 630),
            _ => brand::parse_size(&self.size)?,
        };

        let bg = if self.mode == Mode::Full {
            &self.theme_colour
        } else {
            &self.card_bg
        };

        brand::card(
            image,
            out,
            &self.title,
            Some(self.subtitle.as_str()),
            width,
            height,
            bg,
            &self.accent,
            font,
            self.fit,
        )
    }

    fn output_ui(&mut self, ui: &mut egui::Ui) {
        if let Some(files) = &self.generated {
            ui::heading(ui, &format!("{} file(s) written", files.len()));
            ui.add_space(8.0);

            for file in files {
                ui.label(
                    RichText::new(file.display().to_string())
                        .family(egui::FontFamily::Monospace)
                        .size(12.0)
                        .color(theme::MUTED),
                );
            }
        }

        if let Some(snippet) = &self.snippet {
            ui.add_space(16.0);

            ui.horizontal(|ui| {
                ui::heading(ui, "paste into <head>");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui::secondary(ui, "copy", true).clicked() {
                        ui.ctx().copy_text(snippet.clone());
                    }
                });
            });

            ui.add_space(8.0);
            ui::code_box(ui, snippet);
        }
    }

    // ------------------------------------------------------------ offscreen

    /// Offscreen-screenshot support: pick the sub-mode.
    #[cfg(debug_assertions)]
    pub fn choose(&mut self, mode: &str) {
        self.mode = match mode {
            "favicons" | "favicon" => Mode::Favicon,
            "card" | "social" => Mode::Card,
            _ => Mode::Full,
        };

        self.preview_key.clear();
    }

    /// Offscreen-screenshot support: pre-fill the source image. The card's headline
    /// and subtitle also get example text, so a seeded preview shows real type rather
    /// than an empty card.
    #[cfg(debug_assertions)]
    pub fn seed(&mut self, image: &str) {
        self.image = image.to_string();

        if self.title.trim().is_empty() {
            self.title = "one small binary".to_string();
        }

        if self.subtitle.trim().is_empty() {
            self.subtitle = "three small jobs".to_string();
        }

        self.preview_key.clear();
        self.preview_error = None;
    }

    /// Offscreen-screenshot support: press the main button.
    #[cfg(debug_assertions)]
    pub fn seed_run(&mut self, log: &mut Log) {
        self.run(log);
    }
}

// ------------------------------------------------------------------ helpers

/// A small label above a field.
fn label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(12.0).color(theme::FAINT));
}

/// A monospace path field with a browse button. Returns true when it was pressed.
fn picker(ui: &mut egui::Ui, value: &mut String, hint: &str, button: &str) -> bool {
    let mut pressed = false;

    ui.horizontal(|ui| {
        let width = (ui.available_width() - 122.0).max(150.0);

        ui.add(
            egui::TextEdit::singleline(value)
                .hint_text(hint)
                .desired_width(width)
                .font(egui::TextStyle::Monospace),
        );

        pressed = ui::secondary(ui, button, true).clicked();
    });

    pressed
}
