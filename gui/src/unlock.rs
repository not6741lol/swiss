//! The Unlock tab: find what's holding a file, and end it.

use eframe::egui::{self, RichText};

use crate::theme;
use crate::ui::{self, Log};
use swiss::unlock::{self, Holder};

/// Unlock tab state.
#[derive(Default)]
pub struct UnlockTab {
    /// The file or folder being interrogated.
    path: String,
    /// What the last check found. `None` means we haven't checked yet.
    holders: Option<Vec<Holder>>,
    /// Why the last check failed, if it did.
    error: Option<String>,
    /// The user has acknowledged that closing a process loses unsaved work.
    confirmed: bool,
}

impl UnlockTab {
    pub fn show(&mut self, ui: &mut egui::Ui, log: &mut Log, ctx: &egui::Context) {
        ui::heading(ui, "unlock");
        ui::hint(
            ui,
            "Windows won't tell you which program is holding a file. This asks the Restart Manager.",
        );

        ui.add_space(14.0);

        if !unlock::SUPPORTED {
            ui::card(ui, |ui| {
                ui.label(
                    RichText::new("This one is Windows-only — it relies on the Restart Manager API.")
                        .color(theme::ACCENT)
                        .size(13.0),
                );
            });
            ui.add_space(14.0);
        }

        ui::card(ui, |ui| self.picker(ui, log));

        ui.add_space(14.0);

        ui::card(ui, |ui| self.results(ui, log));

        self.accept_drop(ctx, log);
    }

    /// Offscreen-screenshot support: pre-fill the target.
    #[cfg(debug_assertions)]
    pub fn seed(&mut self, path: &str) {
        self.path = path.to_string();
        self.holders = None;
        self.error = None;
        self.confirmed = false;
    }

    /// Offscreen-screenshot support: press the main button.
    #[cfg(debug_assertions)]
    pub fn seed_run(&mut self, log: &mut Log) {
        self.query(log);
    }

    /// Choose the target.
    fn picker(&mut self, ui: &mut egui::Ui, log: &mut Log) {
        ui::heading(ui, "what's locked?");

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            let browse = 104.0;
            ui.add(
                egui::TextEdit::singleline(&mut self.path)
                    .hint_text(r"C:\path\to\the\file")
                    .desired_width((ui.available_width() - browse).max(160.0))
                    .font(egui::TextStyle::Monospace),
            );

            if ui::secondary(ui, "browse…", true).clicked() {
                if let Some(picked) = rfd::FileDialog::new()
                    .set_title("Pick a file")
                    .pick_file()
                {
                    self.path = picked.display().to_string();
                    self.holders = None;
                    self.error = None;
                    self.confirmed = false;
                }
            }
        });

        ui.add_space(4.0);
        ui::hint(ui, "dragging a file anywhere onto the window works too");

        ui.add_space(12.0);

        let ready = !self.path.trim().is_empty();
        if ui::primary(ui, "find what's holding it", ready).clicked() {
            self.query(log);
        }
    }

    /// The list of holders, and the button that ends them.
    fn results(&mut self, ui: &mut egui::Ui, log: &mut Log) {
        // A failed check isn't the same as "nothing checked yet" — the output pane has
        // the detail, but the card shouldn't imply the file came back clean.
        if let Some(error) = self.error.clone() {
            ui::heading(ui, "result");
            ui.add_space(8.0);
            ui.label(RichText::new(error).color(theme::ERR).size(13.0));
            return;
        }

        let holders = self.holders.clone();

        match holders {
            None => {
                ui::heading(ui, "result");
                ui.add_space(8.0);
                ui::hint(ui, "nothing checked yet");
            }

            Some(list) if list.is_empty() => {
                ui::heading(ui, "result");
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Nothing has it open. You're fine.")
                        .color(theme::OK)
                        .size(13.5),
                );
            }

            Some(list) => {
                ui::heading(
                    ui,
                    &format!(
                        "{} process{} holding it",
                        list.len(),
                        if list.len() == 1 { "" } else { "es" }
                    ),
                );

                ui.add_space(10.0);

                egui::Grid::new("holders")
                    .num_columns(3)
                    .spacing([20.0, 7.0])
                    .show(ui, |ui| {
                        for holder in &list {
                            ui.label(
                                RichText::new(holder.pid.to_string())
                                    .family(egui::FontFamily::Monospace)
                                    .size(12.5)
                                    .color(theme::FAINT),
                            );
                            ui.label(
                                RichText::new(&holder.name)
                                    .family(egui::FontFamily::Monospace)
                                    .size(12.5)
                                    .color(theme::TEXT),
                            );
                            ui.label(
                                RichText::new(holder.type_label())
                                    .size(12.0)
                                    .color(theme::MUTED),
                            );
                            ui.end_row();
                        }
                    });

                ui.add_space(14.0);

                ui.checkbox(
                    &mut self.confirmed,
                    RichText::new("I know this closes them and any unsaved work goes with it")
                        .size(12.5)
                        .color(theme::MUTED),
                );

                ui.add_space(10.0);

                let label = format!(
                    "end {} process{}",
                    list.len(),
                    if list.len() == 1 { "" } else { "es" }
                );

                if ui::primary(ui, &label, self.confirmed).clicked() {
                    self.end(&list, log);
                }
            }
        }
    }

    /// Run a check, replacing the output.
    fn query(&mut self, log: &mut Log) {
        log.clear();
        self.probe(log);
    }

    /// Run a check, appending the outcome.
    fn probe(&mut self, log: &mut Log) {
        let path = self.path.trim().to_string();
        log.info(format!("unlock {path}"));

        match unlock::who_holds(&path) {
            Ok(lock) => {
                self.error = None;

                if lock.holders.is_empty() {
                    log.ok(format!("nothing is holding `{}`", lock.path));
                } else {
                    log.info(format!(
                        "{} process(es) are holding `{}`",
                        lock.holders.len(),
                        lock.path
                    ));
                    for holder in &lock.holders {
                        log.info(format!(
                            "  pid {:>7}  {:<32}  {}",
                            holder.pid,
                            holder.name,
                            holder.type_label()
                        ));
                    }
                }

                self.holders = Some(lock.holders);
                self.confirmed = false;
            }

            Err(err) => {
                log.err(format!("{err:#}"));
                self.error = Some(format!("{err:#}"));
                self.holders = None;
            }
        }
    }

    /// Close everything holding the file, then check again.
    fn end(&mut self, holders: &[Holder], log: &mut Log) {
        let mut killed = 0usize;
        let mut failed = 0usize;

        for holder in holders {
            match unlock::terminate(holder.pid) {
                Ok(()) => {
                    killed += 1;
                    log.ok(format!("terminated {} (pid {})", holder.name, holder.pid));
                }
                Err(err) => {
                    failed += 1;
                    log.err(format!("could not terminate pid {}: {err:#}", holder.pid));
                }
            }
        }

        if failed == 0 {
            log.ok(format!("done — {killed} terminated"));
        } else {
            log.err(format!(
                "done — {killed} terminated, {failed} failed (try running as administrator)"
            ));
        }

        self.confirmed = false;
        self.probe(log);
    }

    /// A file dropped anywhere on the window becomes the target.
    fn accept_drop(&mut self, ctx: &egui::Context, log: &mut Log) {
        let dropped = ctx.input(|input| input.raw.dropped_files.clone());
        let Some(path) = dropped.first().map(|file| file.path().to_path_buf()) else {
            return;
        };

        self.path = path.display().to_string();
        self.query(log);
    }
}
