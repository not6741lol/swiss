//! Offscreen screenshots, for development only.
//!
//! Opening a window just to look at it is disruptive: it steals focus and covers
//! whatever the person was doing, and doing that repeatedly is worse. Setting
//! `SWISS_SHOT=<file>` instead renders the interface into a **hidden** window,
//! saves a PNG and exits, so a layout change can be checked without anything
//! appearing on screen.
//!
//! This whole module is `#[cfg(debug_assertions)]`, so release builds contain none
//! of it. Driven by environment variables:
//!
//! * `SWISS_SHOT` — where to write the PNG. Setting it turns the mode on.
//! * `SWISS_TAB`  — `unlock`, `pdf` or `brand`.
//! * `SWISS_MODE` — the sub-mode within a tab, e.g. `info` or `merge`.
//! * `SWISS_SEED` — a path to pre-fill the tab's field with.
//! * `SWISS_RUN`  — press the tab's main button before capturing.
//! * `SWISS_SCROLL` — how far down the page to scroll, in points.

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::Tab;

/// What an offscreen run should do.
#[derive(Clone)]
pub struct Plan {
    /// Where the PNG goes.
    pub out: PathBuf,
    /// Which tab to show.
    pub tab: Option<Tab>,
    /// Which sub-mode to show inside that tab.
    pub mode: Option<String>,
    /// A path to pre-fill the tab's field with.
    pub seed: Option<String>,
    /// Whether to press the tab's main button before capturing.
    pub run: bool,
    /// How far down the page to scroll before capturing.
    pub scroll: f32,
}

impl Plan {
    /// Read the plan out of the environment, if there is one.
    pub fn from_env() -> Option<Self> {
        Some(Self {
            out: PathBuf::from(std::env::var("SWISS_SHOT").ok()?),
            tab: std::env::var("SWISS_TAB")
                .ok()
                .as_deref()
                .and_then(Tab::parse),
            mode: std::env::var("SWISS_MODE").ok(),
            seed: std::env::var("SWISS_SEED").ok(),
            run: std::env::var("SWISS_RUN").is_ok(),
            scroll: std::env::var("SWISS_SCROLL")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(0.0),
        })
    }
}

/// Write a rendered frame out as a PNG.
pub fn save(path: &Path, image: &egui::ColorImage) -> Result<(), String> {
    let [width, height] = image.size;

    let bytes: Vec<u8> = image
        .pixels
        .iter()
        .flat_map(|pixel| pixel.to_array())
        .collect();

    let raw = image::RgbaImage::from_raw(width as u32, height as u32, bytes)
        .ok_or_else(|| "pixel buffer did not match the image size".to_string())?;

    raw.save(path).map_err(|err| err.to_string())
}
