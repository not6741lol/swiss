//! The PDF tab: merge, split, rotate, strip and inspect.

use std::path::Path;

use eframe::egui::{self, RichText};

use crate::theme;
use crate::ui::{self, Log};
use swiss::pdf;

/// Which of the five jobs is showing.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Mode {
    #[default]
    Merge,
    Split,
    Rotate,
    Strip,
    Info,
}

/// Everything the PDF tab remembers between frames.
#[derive(Default)]
pub struct PdfTab {
    mode: Mode,
    merge: Merge,
    split: Split,
    rotate: Rotate,
    strip: Strip,
    info: Info,
}

#[derive(Default)]
struct Merge {
    inputs: Vec<String>,
    out: String,
}

#[derive(Default)]
struct Split {
    input: String,
    pages: String,
    dir: String,
    /// Page count of `input`, once we've looked.
    total: Option<usize>,
}

struct Rotate {
    input: String,
    degrees: i32,
    out: String,
}

impl Default for Rotate {
    fn default() -> Self {
        Self {
            input: String::new(),
            degrees: 90,
            out: String::new(),
        }
    }
}

#[derive(Default)]
struct Strip {
    input: String,
    out: String,
}

#[derive(Default)]
struct Info {
    input: String,
    found: Option<pdf::PdfInfo>,
}

impl PdfTab {
    pub fn show(&mut self, ui: &mut egui::Ui, log: &mut Log, ctx: &egui::Context) {
        ui::heading(ui, "pdf");
        ui::hint(
            ui,
            "Merge, split, rotate, strip metadata and inspect. Pure Rust via lopdf — nothing is uploaded.",
        );

        ui.add_space(12.0);

        ui.horizontal(|ui| {
            for (mode, label) in [
                (Mode::Merge, "merge"),
                (Mode::Split, "split"),
                (Mode::Rotate, "rotate"),
                (Mode::Strip, "strip"),
                (Mode::Info, "info"),
            ] {
                if ui::pill(ui, label, self.mode == mode).clicked() {
                    self.mode = mode;
                }
            }
        });

        ui.add_space(14.0);

        match self.mode {
            Mode::Merge => ui::card(ui, |ui| self.merge_ui(ui, log)),
            Mode::Split => ui::card(ui, |ui| self.split_ui(ui, log)),
            Mode::Rotate => ui::card(ui, |ui| self.rotate_ui(ui, log)),
            Mode::Strip => ui::card(ui, |ui| self.strip_ui(ui, log)),
            Mode::Info => ui::card(ui, |ui| self.info_ui(ui, log)),
        }

        self.accept_drop(ctx, log);
    }

    // -------------------------------------------------------------- merge

    fn merge_ui(&mut self, ui: &mut egui::Ui, log: &mut Log) {
        ui::heading(ui, "merge");
        ui::hint(ui, "Files are joined in the order below — reorder before you run it.");

        ui.add_space(10.0);

        if self.merge.inputs.is_empty() {
            ui::hint(ui, "no files yet — add some, or drag PDFs onto the window");
        } else {
            // Drag-and-drop and buttons only report an intent here; the list can't
            // be edited while it's being iterated.
            let mut remove = None;
            let mut swap = None;
            let count = self.merge.inputs.len();

            for (index, path) in self.merge.inputs.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{:>2}", index + 1))
                            .family(egui::FontFamily::Monospace)
                            .size(12.0)
                            .color(theme::FAINT),
                    );

                    ui.label(
                        RichText::new(file_name(path))
                            .size(12.5)
                            .color(theme::TEXT),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if small_button(ui, "×", true).clicked() {
                            remove = Some(index);
                        }
                        if small_button(ui, "↓", index + 1 < count).clicked() {
                            swap = Some((index, index + 1));
                        }
                        if small_button(ui, "↑", index > 0).clicked() {
                            swap = Some((index, index - 1));
                        }
                    });
                });
            }

            if let Some(index) = remove {
                self.merge.inputs.remove(index);
            }
            if let Some((a, b)) = swap {
                self.merge.inputs.swap(a, b);
            }
        }

        ui.add_space(12.0);

        if ui::secondary(ui, "add files…", true).clicked() {
            if let Some(picked) = rfd::FileDialog::new()
                .set_title("PDFs to merge")
                .add_filter("PDF", &["pdf"])
                .pick_files()
            {
                self.merge
                    .inputs
                    .extend(picked.iter().map(|path| path.display().to_string()));
            }
        }

        ui.add_space(14.0);
        label(ui, "save as");
        if picker(ui, &mut self.merge.out, r"C:\path\to\merged.pdf", "save as…") {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Where should the merged file go?")
                .set_file_name("merged.pdf")
                .add_filter("PDF", &["pdf"])
                .save_file()
            {
                self.merge.out = path.display().to_string();
            }
        }

        ui.add_space(14.0);

        let ready = self.merge.inputs.len() >= 2 && !self.merge.out.trim().is_empty();
        let verb = format!("merge {} files", self.merge.inputs.len());

        if ui::primary(ui, &verb, ready).clicked() {
            self.merge_now(log);
        }

        if self.merge.inputs.len() == 1 {
            ui.add_space(8.0);
            ui::hint(ui, "merging needs at least two files");
        }
    }

    fn merge_now(&mut self, log: &mut Log) {
        log.clear();
        let inputs = self.merge.inputs.clone();
        let out = self.merge.out.trim().to_string();

        log.info(format!("pdf merge {out} <- {} file(s)", inputs.len()));
        log.result(pdf::merge(&inputs, &out).map(|report| {
            format!(
                "merged {} file(s) — {} page(s) total — into {}",
                report.inputs, report.pages, report.out
            )
        }));
    }

    // -------------------------------------------------------------- split

    fn split_ui(&mut self, ui: &mut egui::Ui, log: &mut Log) {
        ui::heading(ui, "split");
        ui::hint(ui, "One file per page, into a directory.");

        ui.add_space(10.0);

        label(ui, "file");
        if picker(ui, &mut self.split.input, r"C:\path\to\file.pdf", "browse…") {
            if let Some(path) = open_dialog("Pick a PDF") {
                self.split.input = path;
                self.read_split_total();
            }
        }

        ui.add_space(10.0);
        label(ui, "pages");
        ui.add(
            egui::TextEdit::singleline(&mut self.split.pages)
                .hint_text("1-3,7 — leave empty for every page")
                .desired_width(ui.available_width())
                .font(egui::TextStyle::Monospace),
        );

        ui.add_space(4.0);
        match self.split.total {
            Some(total) => ui::hint(ui, &format!("this file has {total} page(s)")),
            None => ui::hint(ui, "browse for a file to see how many pages it has"),
        }

        ui.add_space(10.0);
        label(ui, "output directory");
        if picker(ui, &mut self.split.dir, r"C:\path\to\out", "choose…") {
            if let Some(dir) = rfd::FileDialog::new()
                .set_title("Where should the pages go?")
                .pick_folder()
            {
                self.split.dir = dir.display().to_string();
            }
        }

        if self.split.dir.trim().is_empty() {
            ui.add_space(4.0);
            ui::hint(ui, "empty means the current directory");
        }

        ui.add_space(14.0);

        let ready = !self.split.input.trim().is_empty();
        if ui::primary(ui, "split it", ready).clicked() {
            self.split_now(log);
        }
    }

    fn split_now(&mut self, log: &mut Log) {
        log.clear();

        let input = self.split.input.trim().to_string();
        let spec = self.split.pages.trim();
        let pages = (!spec.is_empty()).then_some(spec);
        let dir = self.split.dir.trim();
        let dir = if dir.is_empty() { "." } else { dir };

        log.info(format!("pdf split {input}"));

        // Each page has to be written somewhere, so report them all.
        match pdf::split(&input, pages, dir) {
            Ok(report) => {
                for file in &report.written {
                    log.info(format!("wrote {file}"));
                }
                log.ok(format!(
                    "{} page(s) written to {}",
                    report.written.len(),
                    report.out_dir
                ));
            }
            Err(err) => log.err(format!("{err:#}")),
        }
    }

    fn read_split_total(&mut self) {
        self.split.total = pdf::page_count(&self.split.input).ok();
    }

    // ------------------------------------------------------------- rotate

    fn rotate_ui(&mut self, ui: &mut egui::Ui, log: &mut Log) {
        ui::heading(ui, "rotate");
        ui::hint(ui, "Turns every page by the same amount.");

        ui.add_space(10.0);

        label(ui, "file");
        if picker(ui, &mut self.rotate.input, r"C:\path\to\file.pdf", "browse…") {
            if let Some(path) = open_dialog("Pick a PDF") {
                self.rotate.input = path;
            }
        }

        ui.add_space(10.0);
        label(ui, "by");
        ui.horizontal(|ui| {
            for degrees in [90, 180, 270] {
                if ui::pill(ui, &format!("{degrees}°"), self.rotate.degrees == degrees).clicked() {
                    self.rotate.degrees = degrees;
                }
            }
        });

        ui.add_space(10.0);
        label(ui, "save to");
        if picker(ui, &mut self.rotate.out, "leave empty to overwrite the original", "save as…") {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Where should the rotated file go?")
                .set_file_name("rotated.pdf")
                .add_filter("PDF", &["pdf"])
                .save_file()
            {
                self.rotate.out = path.display().to_string();
            }
        }

        ui.add_space(14.0);

        let ready = !self.rotate.input.trim().is_empty();
        if ui::primary(ui, "rotate it", ready).clicked() {
            self.rotate_now(log);
        }
    }

    fn rotate_now(&mut self, log: &mut Log) {
        log.clear();

        let input = self.rotate.input.trim().to_string();
        let out = self.rotate.out.trim();
        let out = (!out.is_empty()).then_some(out);

        log.info(format!("pdf rotate {input} --deg {}", self.rotate.degrees));
        log.result(pdf::rotate(&input, self.rotate.degrees, out).map(|report| {
            format!(
                "rotated {} page(s) by {}° -> {}",
                report.pages, report.degrees, report.out
            )
        }));
    }

    // -------------------------------------------------------------- strip

    fn strip_ui(&mut self, ui: &mut egui::Ui, log: &mut Log) {
        ui::heading(ui, "strip");
        ui::hint(
            ui,
            "Removes title, author, producer, dates and any XMP block — the objects are deleted, not just unlinked.",
        );

        ui.add_space(10.0);

        label(ui, "file");
        if picker(ui, &mut self.strip.input, r"C:\path\to\file.pdf", "browse…") {
            if let Some(path) = open_dialog("Pick a PDF") {
                self.strip.input = path;
            }
        }

        ui.add_space(10.0);
        label(ui, "save to");
        if picker(ui, &mut self.strip.out, "leave empty to overwrite the original", "save as…") {
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Where should the stripped file go?")
                .set_file_name("stripped.pdf")
                .add_filter("PDF", &["pdf"])
                .save_file()
            {
                self.strip.out = path.display().to_string();
            }
        }

        ui.add_space(14.0);

        let ready = !self.strip.input.trim().is_empty();
        if ui::primary(ui, "strip metadata", ready).clicked() {
            self.strip_now(log);
        }
    }

    fn strip_now(&mut self, log: &mut Log) {
        log.clear();

        let input = self.strip.input.trim().to_string();
        let out = self.strip.out.trim();
        let out = (!out.is_empty()).then_some(out);

        log.info(format!("pdf strip {input}"));
        log.result(pdf::strip(&input, out).map(|report| {
            format!("stripped metadata from {} -> {}", report.input, report.out)
        }));
    }

    // --------------------------------------------------------------- info

    fn info_ui(&mut self, ui: &mut egui::Ui, log: &mut Log) {
        ui::heading(ui, "info");
        ui::hint(ui, "Page count, size, version and whatever metadata is still in there.");

        ui.add_space(10.0);

        label(ui, "file");
        if picker(ui, &mut self.info.input, r"C:\path\to\file.pdf", "browse…") {
            if let Some(path) = open_dialog("Pick a PDF") {
                self.info.input = path;
                self.info.found = None;
            }
        }

        ui.add_space(14.0);

        let ready = !self.info.input.trim().is_empty();
        if ui::primary(ui, "read it", ready).clicked() {
            self.info_now(log);
        }

        let Some(found) = self.info.found.clone() else {
            return;
        };

        ui.add_space(16.0);
        ui.separator();
        ui.add_space(12.0);

        egui::Grid::new("pdf-facts")
            .num_columns(2)
            .spacing([26.0, 7.0])
            .show(ui, |ui| {
                let facts = [
                    ("file", found.path.clone()),
                    ("size", pdf::human_size(found.bytes)),
                    ("version", found.version.clone()),
                    ("pages", found.pages.to_string()),
                    ("encrypted", found.encrypted.to_string()),
                ];

                for (key, value) in facts {
                    ui.label(RichText::new(key).size(12.0).color(theme::FAINT));
                    ui.label(
                        RichText::new(value)
                            .family(egui::FontFamily::Monospace)
                            .size(12.5)
                            .color(theme::TEXT),
                    );
                    ui.end_row();
                }
            });

        ui.add_space(14.0);

        if found.metadata.is_empty() {
            ui::hint(ui, "no metadata — nothing left to leak");
        } else {
            ui::heading(ui, "metadata");
            ui.add_space(8.0);

            // Values can be long (a producer string, a user-agent), so they get
            // their own line instead of a grid cell that would push the card wide.
            for (key, value) in &found.metadata {
                ui.label(
                    RichText::new(key.to_lowercase())
                        .size(11.5)
                        .color(theme::FAINT),
                );
                ui.label(
                    RichText::new(value)
                        .family(egui::FontFamily::Monospace)
                        .size(12.0)
                        .color(theme::MUTED),
                );
                ui.add_space(6.0);
            }
        }
    }

    fn info_now(&mut self, log: &mut Log) {
        log.clear();

        let input = self.info.input.trim().to_string();
        log.info(format!("pdf info {input}"));

        match pdf::info(&input) {
            Ok(found) => {
                log.ok(format!(
                    "{} page(s), {}",
                    found.pages,
                    pdf::human_size(found.bytes)
                ));
                self.info.found = Some(found);
            }
            Err(err) => {
                log.err(format!("{err:#}"));
                self.info.found = None;
            }
        }
    }

    // ------------------------------------------------------------ offscreen

    /// Offscreen-screenshot support: pick the sub-mode.
    #[cfg(debug_assertions)]
    pub fn choose(&mut self, mode: &str) {
        self.mode = match mode {
            "merge" => Mode::Merge,
            "split" => Mode::Split,
            "rotate" => Mode::Rotate,
            "strip" => Mode::Strip,
            _ => Mode::Info,
        };
    }

    /// Offscreen-screenshot support: pre-fill the mode's fields. Several paths can
    /// be given at once for merge, separated by `;`.
    #[cfg(debug_assertions)]
    pub fn seed(&mut self, seed: &str) {
        let paths: Vec<String> = seed
            .split(';')
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_string)
            .collect();

        let Some(first) = paths.first().cloned() else {
            return;
        };

        match self.mode {
            Mode::Merge => {
                for path in paths {
                    if !self.merge.inputs.contains(&path) {
                        self.merge.inputs.push(path);
                    }
                }
                if self.merge.out.trim().is_empty() {
                    self.merge.out = beside(&first, "merged.pdf");
                }
            }

            Mode::Split => {
                self.split.input = first;
                self.read_split_total();
            }

            Mode::Rotate => {
                self.rotate.input = first.clone();
                if self.rotate.out.trim().is_empty() {
                    self.rotate.out = beside(&first, "rotated.pdf");
                }
            }

            Mode::Strip => {
                self.strip.input = first.clone();
                if self.strip.out.trim().is_empty() {
                    self.strip.out = beside(&first, "stripped.pdf");
                }
            }

            Mode::Info => {
                self.info.input = first;
                self.info.found = None;
            }
        }
    }

    /// Offscreen-screenshot support: press the mode's main button.
    #[cfg(debug_assertions)]
    pub fn seed_run(&mut self, log: &mut Log) {
        match self.mode {
            Mode::Merge => self.merge_now(log),
            Mode::Split => self.split_now(log),
            Mode::Rotate => self.rotate_now(log),
            Mode::Strip => self.strip_now(log),
            Mode::Info => self.info_now(log),
        }
    }

    // --------------------------------------------------------------- drops

    /// Dropped files fill in whichever mode is showing.
    fn accept_drop(&mut self, ctx: &egui::Context, log: &mut Log) {
        let dropped = ctx.input(|input| input.raw.dropped_files.clone());
        if dropped.is_empty() {
            return;
        }

        let paths: Vec<String> = dropped
            .iter()
            .map(|file| file.path().display().to_string())
            .collect();

        match self.mode {
            Mode::Merge => {
                log.clear();
                log.info(format!("added {} file(s) to the merge", paths.len()));
                self.merge.inputs.extend(paths);
            }

            Mode::Split => {
                self.split.input = paths[0].clone();
                self.read_split_total();
            }

            Mode::Rotate => self.rotate.input = paths[0].clone(),
            Mode::Strip => self.strip.input = paths[0].clone(),

            Mode::Info => {
                self.info.input = paths[0].clone();
                self.info.found = None;
            }
        }
    }
}

// ------------------------------------------------------------------ helpers

/// The file's own name, which reads better than a full path in a list.
fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

/// A tiny square button for list controls.
fn small_button(ui: &mut egui::Ui, glyph: &str, enabled: bool) -> egui::Response {
    let button = egui::Button::new(RichText::new(glyph).size(12.5))
        .fill(theme::SURFACE_2)
        .stroke(egui::Stroke::new(1.0, theme::BORDER))
        .corner_radius(egui::CornerRadius::same(7))
        .min_size(egui::vec2(26.0, 24.0));

    ui.add_enabled(enabled, button)
}

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

/// A single-file picker, as a display string.
fn open_dialog(title: &str) -> Option<String> {
    rfd::FileDialog::new()
        .set_title(title)
        .add_filter("PDF", &["pdf"])
        .pick_file()
        .map(|path| path.display().to_string())
}

/// A sibling of `path` under a different name. Used by the offscreen screenshot mode.
#[cfg(debug_assertions)]
fn beside(path: &str, name: &str) -> String {
    Path::new(path).with_file_name(name).display().to_string()
}
