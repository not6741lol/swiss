//! Small pieces shared by every tab.

use eframe::egui::{self, Color32, Margin, RichText, Stroke, Ui};

use crate::theme;

// ---------------------------------------------------------------------- output

/// How a line of output should read.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Level {
    /// Neutral chatter.
    Info,
    /// Something worked.
    Ok,
    /// Something did not.
    Err,
}

/// One line of output — the terminal equivalent of a `println!`.
pub struct LogLine {
    /// How it should read.
    pub level: Level,
    /// What it says.
    pub text: String,
}

/// A rolling output buffer, standing in for the CLI's stdout.
#[derive(Default)]
pub struct Log {
    lines: Vec<LogLine>,
}

impl Log {
    /// Throw everything away.
    pub fn clear(&mut self) {
        self.lines.clear();
    }

    /// True when nothing has been written yet.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// Note something neutral.
    pub fn info(&mut self, text: impl Into<String>) {
        self.push(Level::Info, text);
    }

    /// Note that something worked.
    pub fn ok(&mut self, text: impl Into<String>) {
        self.push(Level::Ok, text);
    }

    /// Note that something failed.
    pub fn err(&mut self, text: impl Into<String>) {
        self.push(Level::Err, text);
    }

    /// Record the outcome of a fallible operation, flattening the `anyhow` chain
    /// so a context line and its cause both show up. Returns whether it worked.
    pub fn result(&mut self, outcome: anyhow::Result<impl Into<String>>) -> bool {
        match outcome {
            Ok(text) => {
                self.ok(text);
                true
            }
            Err(err) => {
                self.err(format!("{err:#}"));
                false
            }
        }
    }

    fn push(&mut self, level: Level, text: impl Into<String>) {
        self.lines.push(LogLine {
            level,
            text: text.into(),
        });
    }

    /// Draw the buffer in monospace, like a terminal.
    pub fn show(&self, ui: &mut Ui) {
        if self.lines.is_empty() {
            ui.label(
                RichText::new("nothing yet")
                    .family(egui::FontFamily::Monospace)
                    .size(12.5)
                    .color(theme::FAINT),
            );
            return;
        }

        for line in &self.lines {
            let (marker, colour) = match line.level {
                Level::Info => ("›", theme::MUTED),
                Level::Ok => ("✓", theme::OK),
                Level::Err => ("✗", theme::ERR),
            };

            ui.label(
                RichText::new(format!("{marker} {}", line.text))
                    .family(egui::FontFamily::Monospace)
                    .size(12.5)
                    .color(colour),
            );
        }
    }
}

// -------------------------------------------------------------------- widgets

/// A section heading, in the site's lowercase style.
pub fn heading(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text.to_lowercase())
            .family(theme::semibold())
            .size(16.0)
            .color(theme::TEXT),
    );
}

/// A quiet line of explanation under a heading.
pub fn hint(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(12.5)
            .color(theme::FAINT)
            .family(egui::FontFamily::Proportional),
    );
}

/// A panel, like the site's `.panel`. Always spans the full available width, so a
/// short card never looks like a stray box floating in the middle of the window.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    const MARGIN: f32 = 16.0;
    let width = ui.available_width();

    egui::Frame::new()
        .fill(theme::SURFACE)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_PANEL))
        .inner_margin(Margin::same(MARGIN as i8))
        .show(ui, |ui| {
            ui.set_min_width((width - MARGIN * 2.0 - 2.0).max(0.0));
            add(ui)
        })
        .inner
}

/// The primary action: filled with the warm end of the site's gradient.
///
/// A disabled primary goes flat and grey rather than staying amber, so "this
/// doesn't do anything yet" is obvious before the click rather than after it.
pub fn primary(ui: &mut Ui, text: &str, enabled: bool) -> egui::Response {
    let (fill, ink) = if enabled {
        (theme::ACCENT_FROM, theme::ON_ACCENT)
    } else {
        (theme::SURFACE_2, theme::FAINT)
    };

    let button = egui::Button::new(
        RichText::new(text)
            .family(theme::semibold())
            .size(14.0)
            .color(ink),
    )
    .fill(fill)
    .stroke(Stroke::NONE)
    .corner_radius(egui::CornerRadius::same(theme::RADIUS));

    ui.add_enabled(enabled, button)
}

/// A small selectable pill, used to switch between the modes within a tab.
pub fn pill(ui: &mut Ui, label: &str, selected: bool) -> egui::Response {
    let text = RichText::new(label)
        .family(theme::semibold())
        .size(13.0)
        .color(if selected {
            theme::ON_ACCENT
        } else {
            theme::MUTED
        });

    let button = egui::Button::new(text)
        .fill(if selected {
            theme::ACCENT_FROM
        } else {
            theme::SURFACE_2
        })
        .stroke(if selected {
            Stroke::NONE
        } else {
            Stroke::new(1.0, theme::BORDER)
        })
        .corner_radius(egui::CornerRadius::same(9));

    ui.add(button)
}

/// A restrained secondary action.
pub fn secondary(ui: &mut Ui, text: &str, enabled: bool) -> egui::Response {
    let button = egui::Button::new(RichText::new(text).size(14.0))
        .fill(theme::SURFACE_2)
        .stroke(Stroke::new(1.0, theme::BORDER_STRONG))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS));

    ui.add_enabled(enabled, button)
}

/// A monospace, read-only box for text worth copying.
pub fn code_box(ui: &mut Ui, text: &str) {
    egui::Frame::new()
        .fill(theme::BG)
        .stroke(Stroke::new(1.0, theme::BORDER))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.label(
                RichText::new(text)
                    .family(egui::FontFamily::Monospace)
                    .size(12.0)
                    .color(theme::MUTED),
            );
        });
}

/// A colour swatch you can type a hex value into. Returns true when it changed.
pub fn colour_field(ui: &mut Ui, value: &mut String) -> bool {
    let swatch = swiss::brand::parse_color(value)
        .ok()
        .map(|c| Color32::from_rgb(c.0[0], c.0[1], c.0[2]))
        .unwrap_or(theme::ERR);

    let mut changed = false;

    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(30.0, 26.0), egui::Sense::hover());
        ui.painter().rect_filled(
            rect,
            egui::CornerRadius::same(theme::RADIUS),
            swatch,
        );
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(theme::RADIUS),
            Stroke::new(1.0, theme::BORDER_STRONG),
            egui::StrokeKind::Inside,
        );

        changed = ui
            .add(
                egui::TextEdit::singleline(value)
                    .desired_width(96.0)
                    .font(egui::TextStyle::Monospace),
            )
            .changed();
    });

    changed
}
