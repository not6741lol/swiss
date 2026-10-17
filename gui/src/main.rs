//! swiss-gui — the windowed half of swiss.
//!
//! The CLI is still the same 1.5 MB binary it always was. This is a second
//! front-end over the same core, for when clicking beats typing.

#[cfg(debug_assertions)]
mod autoshot;
mod brand;
mod pdf;
mod theme;
mod ui;
mod unlock;

use eframe::egui::{self, RichText};

use ui::Log;

fn main() -> eframe::Result<()> {
    #[cfg(debug_assertions)]
    let plan = autoshot::Plan::from_env();

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([1080.0, 760.0])
        .with_min_inner_size([880.0, 620.0]);

    // An offscreen run parks the window well past the edge of the desktop. It has to
    // stay *visible*, because Windows only delivers paint messages to windows that
    // are shown — a hidden window never renders a frame at all — but nothing is
    // placed anywhere a person could see it.
    #[cfg(debug_assertions)]
    let viewport = if plan.is_some() {
        viewport
            .with_position([20_000.0, 20_000.0])
            .with_taskbar(false)
    } else {
        viewport
    };

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "swiss",
        options,
        Box::new(move |cc| {
            theme::install(&cc.egui_ctx);

            #[cfg(debug_assertions)]
            let app = App {
                shot: plan,
                ..Default::default()
            };

            #[cfg(not(debug_assertions))]
            let app = App::default();

            Ok(Box::new(app))
        }),
    )
}

/// Which job you're looking at.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Unlock,
    Pdf,
    Brand,
}

impl Tab {
    /// Parse a tab name. Used by the offscreen screenshot mode.
    #[cfg(debug_assertions)]
    fn parse(name: &str) -> Option<Self> {
        match name {
            "unlock" => Some(Self::Unlock),
            "pdf" => Some(Self::Pdf),
            "brand" => Some(Self::Brand),
            _ => None,
        }
    }
}

#[derive(Default)]
struct App {
    tab: Tab,
    log: Log,
    unlock: unlock::UnlockTab,
    pdf: pdf::PdfTab,
    brand: brand::BrandTab,

    #[cfg(debug_assertions)]
    shot: Option<autoshot::Plan>,
    #[cfg(debug_assertions)]
    frame: usize,
    #[cfg(debug_assertions)]
    saved: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        #[cfg(debug_assertions)]
        {
            let ctx = ui.ctx().clone();
            self.drive(&ctx);
        }

        egui::Panel::top("top-bar")
            .frame(
                egui::Frame::new()
                    .fill(theme::SURFACE)
                    .inner_margin(egui::Margin::symmetric(16, 10)),
            )
            .show(ui, |ui| self.top_bar(ui));

        // The output pane is the window's answer to the CLI's stdout, so it gets
        // the terminal colour rather than the panel colour.
        egui::Panel::bottom("output")
            .resizable(true)
            .default_size(158.0)
            .min_size(76.0)
            .frame(
                egui::Frame::new()
                    .fill(theme::TERMINAL)
                    .inner_margin(egui::Margin::symmetric(16, 12)),
            )
            .show(ui, |ui| self.output(ui));

        let tab = self.tab;
        let ctx = ui.ctx().clone();

        egui::CentralPanel::no_frame()
            .frame(
                egui::Frame::new()
                    .fill(theme::BG)
                    .inner_margin(egui::Margin::symmetric(16, 16)),
            )
            .show(ui, |ui| {
                let area = egui::ScrollArea::vertical().auto_shrink([false, false]);

                #[cfg(debug_assertions)]
                let area = match &self.shot {
                    Some(plan) => area.vertical_scroll_offset(plan.scroll),
                    None => area,
                };

                area.show(ui, |ui| match tab {
                    Tab::Unlock => self.unlock.show(ui, &mut self.log, &ctx),
                    Tab::Pdf => self.pdf.show(ui, &mut self.log, &ctx),
                    Tab::Brand => self.brand.show(ui, &mut self.log, &ctx),
                });
            });
    }
}

impl App {
    /// An offscreen screenshot run, if one was asked for. Does nothing otherwise.
    #[cfg(debug_assertions)]
    fn drive(&mut self, ctx: &egui::Context) {
        let Some(plan) = self.shot.clone() else {
            return;
        };

        self.frame += 1;

        // Frame one is the first with a laid-out interface, so it's the earliest
        // point we can set the state we were asked for and have it be drawn.
        if self.frame == 1 {
            if let Some(tab) = plan.tab {
                self.tab = tab;
            }

            if let Some(mode) = &plan.mode {
                match self.tab {
                    Tab::Pdf => self.pdf.choose(mode),
                    Tab::Brand => self.brand.choose(mode),
                    Tab::Unlock => {}
                }
            }

            if let Some(seed) = &plan.seed {
                match self.tab {
                    Tab::Unlock => self.unlock.seed(seed),
                    Tab::Pdf => self.pdf.seed(seed),
                    Tab::Brand => self.brand.seed(seed),
                }
            }

            ctx.request_repaint();
            return;
        }

        if plan.run && self.frame == 3 {
            match self.tab {
                Tab::Unlock => self.unlock.seed_run(&mut self.log),
                Tab::Pdf => self.pdf.seed_run(&mut self.log),
                Tab::Brand => self.brand.seed_run(&mut self.log),
            }

            ctx.request_repaint();
            return;
        }

        // Fonts, textures and the first layout settle over a few frames; grabbing
        // too early catches a half-drawn frame.
        if self.frame >= 10 && (self.frame - 10) % 30 == 0 {
            let out = plan.out.clone();
            let close = ctx.clone();
            let saved = self.saved.clone();

            ctx.request_screenshot(move |image| {
                match autoshot::save(&out, &image) {
                    Ok(()) => println!("autoshot: wrote {}", out.display()),
                    Err(err) => eprintln!("autoshot: {err}"),
                }

                saved.store(true, std::sync::atomic::Ordering::SeqCst);
                close.send_viewport_cmd(egui::ViewportCommand::Close);
            });
        }

        // A queued capture only completes if another frame gets painted, and asking
        // for one is not optional — without it the app idles forever and the run
        // hangs. The deadline is the backstop in case nothing ever comes back.
        if self.frame > 900 {
            eprintln!("autoshot: nothing came back after 900 frames, giving up");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        if !self.saved.load(std::sync::atomic::Ordering::SeqCst) {
            ctx.request_repaint();
        }
    }

    /// The wordmark, the three tabs, and the version.
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, egui::CornerRadius::same(8), theme::ACCENT_FROM);
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                ">_",
                egui::FontId::new(12.0, egui::FontFamily::Monospace),
                theme::ON_ACCENT,
            );

            ui.label(
                RichText::new("swiss")
                    .family(theme::bold())
                    .size(17.0)
                    .color(theme::TEXT),
            );

            ui.add_space(22.0);

            for (tab, label) in [
                (Tab::Unlock, "Unlock"),
                (Tab::Pdf, "PDF"),
                (Tab::Brand, "Brand"),
            ] {
                if ui::pill(ui, label, self.tab == tab).clicked() {
                    self.tab = tab;
                }
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                        .size(12.0)
                        .color(theme::FAINT),
                );
            });
        });
    }

    /// Everything the CLI would have printed.
    fn output(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("output")
                    .family(theme::semibold())
                    .size(12.0)
                    .color(theme::FAINT),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui::secondary(ui, "clear", !self.log.is_empty()).clicked() {
                    self.log.clear();
                }
            });
        });

        ui.add_space(6.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| self.log.show(ui));
    }
}
