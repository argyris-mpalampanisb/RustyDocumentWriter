//! Dark ("Black") colour palette and UI fonts.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId, TextStyle};
use std::sync::Arc;

pub const BG: Color32 = Color32::from_rgb(0x0A, 0x0A, 0x0A); // window / title bar / canvas
pub const RIBBON: Color32 = Color32::from_rgb(0x29, 0x29, 0x29); // ribbon panel
pub const HOVER: Color32 = Color32::from_rgb(0x3D, 0x3D, 0x3D);
pub const PRESSED: Color32 = Color32::from_rgb(0x4A, 0x4A, 0x4A);
pub const CHECKED: Color32 = Color32::from_rgb(0x45, 0x45, 0x45);
pub const SEP: Color32 = Color32::from_rgb(0x47, 0x47, 0x47);
pub const TEXT: Color32 = Color32::from_rgb(0xF0, 0xF0, 0xF0);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0xC8, 0xC8, 0xC8);
pub const TEXT_DISABLED: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);
pub const ICON: Color32 = Color32::from_rgb(0xD6, 0xD6, 0xD6);
pub const ACCENT: Color32 = Color32::from_rgb(0x2B, 0x88, 0xD8);
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(0x8C, 0xC8, 0xFF);
pub const FIELD: Color32 = Color32::from_rgb(0x1F, 0x1F, 0x1F);
pub const BACKSTAGE_SIDE: Color32 = Color32::from_rgb(0x1B, 0x1B, 0x1B);
pub const BACKSTAGE: Color32 = Color32::from_rgb(0x26, 0x26, 0x26);

pub const ICON_FAMILY: &str = "icons";

fn read_first(paths: &[&str]) -> Option<Vec<u8>> {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    for p in paths {
        let full = if p.contains(['/', '\\']) { p.to_string() } else { format!("{windir}\\Fonts\\{p}") };
        if let Ok(b) = std::fs::read(&full) {
            return Some(b);
        }
    }
    None
}

/// Install fonts & visuals. Returns true if an icon font (Segoe Fluent / MDL2) is available.
pub fn install(ctx: &egui::Context) -> bool {
    let mut fonts = FontDefinitions::default();
    // Segoe UI on Windows; elsewhere the bundled Liberation Sans keeps the UI consistent.
    let embedded = |name: &str| rusty_document_writer::fonts::EMBEDDED_FONTS.iter().find(|(n, _)| *n == name).map(|(_, b)| b.to_vec());
    if let Some(b) = read_first(&["segoeui.ttf"]).or_else(|| embedded("LiberationSans-Regular.ttf")) {
        fonts.font_data.insert("ui".into(), Arc::new(FontData::from_owned(b)));
        fonts.families.get_mut(&FontFamily::Proportional).unwrap().insert(0, "ui".into());
    }
    if let Some(b) = read_first(&["segoeuib.ttf"]).or_else(|| embedded("LiberationSans-Bold.ttf")) {
        fonts.font_data.insert("ui-bold".into(), Arc::new(FontData::from_owned(b)));
        fonts.families.insert(FontFamily::Name("bold".into()), vec!["ui-bold".into(), "ui".into()]);
    } else {
        fonts.families.insert(FontFamily::Name("bold".into()), fonts.families[&FontFamily::Proportional].clone());
    }
    // Symbols used in labels (¶, Ω, π, arrows...).
    if let Some(b) = read_first(&["seguisym.ttf"]) {
        fonts.font_data.insert("sym".into(), Arc::new(FontData::from_owned(b)));
        fonts.families.get_mut(&FontFamily::Proportional).unwrap().push("sym".into());
    }
    let icon = read_first(&["SegoeIcons.ttf", "segmdl2.ttf"]);
    let has_icons = icon.is_some();
    if let Some(b) = icon {
        fonts.font_data.insert("icons".into(), Arc::new(FontData::from_owned(b)));
        fonts.families.insert(FontFamily::Name(ICON_FAMILY.into()), vec!["icons".into()]);
    } else {
        fonts.families.insert(FontFamily::Name(ICON_FAMILY.into()), fonts.families[&FontFamily::Proportional].clone());
    }
    ctx.set_fonts(fonts);

    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = Color32::from_rgb(0x2B, 0x2B, 0x2B);
    visuals.extreme_bg_color = FIELD;
    visuals.selection.bg_fill = ACCENT;
    visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(0x3A, 0x3A, 0x3A);
    visuals.widgets.hovered.weak_bg_fill = HOVER;
    visuals.widgets.active.weak_bg_fill = PRESSED;
    visuals.window_corner_radius = 8.into();
    visuals.menu_corner_radius = 6.into();
    ctx.set_visuals(visuals);

    let mut style = (*ctx.global_style()).clone();
    style.text_styles = [
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Heading, FontId::new(17.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace)),
    ]
    .into();
    style.spacing.item_spacing = egui::vec2(6.0, 4.0);
    style.spacing.button_padding = egui::vec2(8.0, 3.0);
    ctx.set_global_style(style);
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
    has_icons
}

pub fn ui_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

pub fn bold_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("bold".into()))
}

pub fn icon_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(ICON_FAMILY.into()))
}
