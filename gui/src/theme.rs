//! The look of the app: not6741.lol, in a window.
//!
//! Every colour below is lifted from the site's `site.css` custom properties, and
//! the type is the same Inter + JetBrains Mono pair the site loads, so the app and
//! the website cannot drift apart.

use eframe::egui::{
    self, Color32, Context, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin,
    Shadow, Spacing, Stroke, TextStyle, Visuals,
};
use std::collections::BTreeMap;

// ------------------------------------------------------------------- palette
// Straight from site.css `:root`.

/// `--bg`
pub const BG: Color32 = Color32::from_rgb(0x0e, 0x0e, 0x10);
/// A shade under `--bg`, for the output pane — the site's terminal colour.
pub const TERMINAL: Color32 = Color32::from_rgb(0x0b, 0x0b, 0x0d);
/// `--surface`
pub const SURFACE: Color32 = Color32::from_rgb(0x17, 0x17, 0x1a);
/// `--surface-2`
pub const SURFACE_2: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x22);
/// `--border`
pub const BORDER: Color32 = Color32::from_rgb(0x29, 0x29, 0x30);
/// `--border-strong`
pub const BORDER_STRONG: Color32 = Color32::from_rgb(0x3a, 0x3a, 0x43);
/// `--text`
pub const TEXT: Color32 = Color32::from_rgb(0xf3, 0xf3, 0xf5);
/// `--muted`
pub const MUTED: Color32 = Color32::from_rgb(0xa4, 0xa4, 0xad);
/// `--faint`
pub const FAINT: Color32 = Color32::from_rgb(0x75, 0x75, 0x7f);
/// `--accent`
pub const ACCENT: Color32 = Color32::from_rgb(0xf0, 0xa1, 0x3a);

/// The warm end of the site's `--grad`, used to fill anything you can press.
pub const ACCENT_FROM: Color32 = Color32::from_rgb(0xff, 0x8a, 0x1e);

/// Ink colour for text sitting on the accent.
pub const ON_ACCENT: Color32 = Color32::from_rgb(0x1a, 0x11, 0x04);

/// Something worked.
pub const OK: Color32 = Color32::from_rgb(0x6b, 0xd3, 0x94);
/// Something did not.
pub const ERR: Color32 = Color32::from_rgb(0xff, 0x74, 0x74);

/// Corner radius for a control (the site's `--radius-sm`).
pub const RADIUS: u8 = 11;
/// Corner radius for a panel (the site's `--radius`).
pub const RADIUS_PANEL: u8 = 16;

// ---------------------------------------------------------------------- type

/// Inter, regular.
pub const INTER: &str = "inter";
/// Inter, semibold — headings and buttons.
pub const INTER_SEMIBOLD: &str = "inter-semibold";
/// Inter, bold — the wordmark.
pub const INTER_BOLD: &str = "inter-bold";
/// JetBrains Mono. Anything the terminal would have printed.
pub const MONO: &str = "jetbrains-mono";

/// Semibold, for headings and buttons.
pub fn semibold() -> FontFamily {
    FontFamily::Name(INTER_SEMIBOLD.into())
}

/// Bold, for the wordmark.
pub fn bold() -> FontFamily {
    FontFamily::Name(INTER_BOLD.into())
}

// ------------------------------------------------------------------- install

/// Install the bundled fonts and the visual style.
pub fn install(ctx: &Context) {
    use std::sync::Arc;

    let mut fonts = FontDefinitions::default();

    for (name, bytes) in [
        (
            INTER,
            include_bytes!("../assets/Inter_400Regular.ttf").as_slice(),
        ),
        (
            INTER_SEMIBOLD,
            include_bytes!("../assets/Inter_600SemiBold.ttf").as_slice(),
        ),
        (
            INTER_BOLD,
            include_bytes!("../assets/Inter_700Bold.ttf").as_slice(),
        ),
        (
            MONO,
            include_bytes!("../assets/JetBrainsMono_400Regular.ttf").as_slice(),
        ),
    ] {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }

    // Keep egui's bundled faces behind ours. Inter has no arrows, box-drawing or
    // emoji, and the interface uses all three (list reordering, log markers), so
    // without a fallback those glyphs would silently render as nothing.
    let bundled = fonts.families.clone();

    fonts
        .families
        .insert(FontFamily::Proportional, stack(&bundled, INTER));
    fonts
        .families
        .insert(FontFamily::Monospace, stack(&bundled, MONO));
    fonts.families.insert(semibold(), stack(&bundled, INTER_SEMIBOLD));
    fonts.families.insert(bold(), stack(&bundled, INTER_BOLD));

    ctx.set_fonts(fonts);

    // There is no longer one global style: egui keeps one per theme, so set them
    // all and pin the app to dark.
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.all_styles_mut(|style| {
        style.visuals = visuals();
        style.spacing = spacing();
        style.text_styles = text_styles();
    });
}

/// Our face first, then whatever egui bundles, so no glyph can go missing.
fn stack(bundled: &BTreeMap<FontFamily, Vec<String>>, first: &str) -> Vec<String> {
    let mut list = vec![first.to_owned()];

    if let Some(rest) = bundled.get(&FontFamily::Proportional) {
        list.extend(rest.iter().filter(|name| name.as_str() != first).cloned());
    }

    list
}

/// A dark theme built out of the site's palette.
fn visuals() -> Visuals {
    let mut v = Visuals::dark();

    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.faint_bg_color = SURFACE;
    v.extreme_bg_color = Color32::from_rgb(0x0a, 0x0a, 0x0c);
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.window_corner_radius = CornerRadius::same(RADIUS_PANEL);
    v.menu_corner_radius = CornerRadius::same(RADIUS);
    v.window_shadow = Shadow {
        offset: [0, 18],
        blur: 40,
        spread: 0,
        color: Color32::from_black_alpha(170),
    };
    v.popup_shadow = v.window_shadow;

    v.override_text_color = Some(TEXT);
    v.hyperlink_color = ACCENT;
    v.warn_fg_color = ACCENT;
    v.error_fg_color = ERR;
    v.selection.bg_fill = Color32::from_rgba_unmultiplied(0xf0, 0xa1, 0x3a, 70);
    v.selection.stroke = Stroke::new(1.0, ACCENT);

    let radius = CornerRadius::same(RADIUS);

    v.widgets.noninteractive.bg_fill = SURFACE;
    v.widgets.noninteractive.weak_bg_fill = SURFACE;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.noninteractive.corner_radius = radius;

    v.widgets.inactive.bg_fill = SURFACE_2;
    v.widgets.inactive.weak_bg_fill = SURFACE_2;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.inactive.corner_radius = radius;

    let lifted = Color32::from_rgb(0x27, 0x27, 0x2e);
    v.widgets.hovered.bg_fill = lifted;
    v.widgets.hovered.weak_bg_fill = lifted;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.hovered.corner_radius = radius;

    v.widgets.active.bg_fill = ACCENT_FROM;
    v.widgets.active.weak_bg_fill = ACCENT_FROM;
    v.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT);
    v.widgets.active.fg_stroke = Stroke::new(1.0, ON_ACCENT);
    v.widgets.active.corner_radius = radius;

    v.widgets.open.bg_fill = SURFACE_2;
    v.widgets.open.weak_bg_fill = SURFACE_2;
    v.widgets.open.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    v.widgets.open.fg_stroke = Stroke::new(1.0, TEXT);
    v.widgets.open.corner_radius = radius;

    v
}

fn spacing() -> Spacing {
    let mut s = Spacing::default();
    s.item_spacing = egui::vec2(10.0, 10.0);
    s.button_padding = egui::vec2(14.0, 7.0);
    s.interact_size.y = 30.0;
    s.window_margin = Margin::same(14);
    s.scroll = egui::style::ScrollStyle::solid();
    s
}

fn text_styles() -> BTreeMap<TextStyle, FontId> {
    [
        (TextStyle::Small, FontId::new(12.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(14.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(14.0, semibold())),
        (TextStyle::Heading, FontId::new(21.0, semibold())),
        (TextStyle::Monospace, FontId::new(13.0, FontFamily::Monospace)),
    ]
    .into_iter()
    .collect()
}
