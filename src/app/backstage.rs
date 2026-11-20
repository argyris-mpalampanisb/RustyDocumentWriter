//! The File ("backstage") view.

use super::icons::{self, Icon};
use super::theme::{self, *};
use super::{App, Cmd};
use egui::{pos2, vec2, Align2, Color32, Rect, Sense, Stroke, StrokeKind, Ui};

#[derive(Clone, Copy, PartialEq)]
enum Page {
    Home,
    New,
    Open,
    Info,
}

pub fn show(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    let full = ui.max_rect();
    let area = Rect::from_min_max(pos2(full.left(), ui.cursor().top()), full.right_bottom());
    let side = Rect::from_min_max(area.left_top(), pos2(area.left() + 220.0, area.bottom()));
    let content = Rect::from_min_max(pos2(side.right(), area.top()), area.right_bottom());
    let p = ui.painter().clone();
    p.rect_filled(side, 0.0, BACKSTAGE_SIDE);
    p.rect_filled(content, 0.0, BACKSTAGE);
    let id = egui::Id::NULL.with("backstage-page");
    let mut page: Page = ui.data(|d| d.get_temp(id)).unwrap_or(Page::Home);

    let mut sui = ui.new_child(egui::UiBuilder::new().max_rect(side.shrink2(vec2(0.0, 10.0))));
    let mut item = |ui: &mut Ui, label: &str, icon: Option<Icon>, selected: bool| -> bool {
        let (rect, r) = ui.allocate_exact_size(vec2(side.width(), 40.0), Sense::click());
        if selected {
            ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(0x33, 0x33, 0x33));
        } else if r.hovered() {
            ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(0x2B, 0x2B, 0x2B));
        }
        if let Some(ic) = icon {
            icons::paint(ui.painter(), ic, Rect::from_center_size(pos2(rect.left() + 28.0, rect.center().y), vec2(18.0, 18.0)), TEXT);
        }
        ui.painter().text(pos2(rect.left() + 52.0, rect.center().y), Align2::LEFT_CENTER, label, theme::ui_font(14.0), TEXT);
        r.clicked()
    };
    if item(&mut sui, "Back", Some(Icon::Prev), false) {
        app.backstage = false;
    }
    sui.add_space(8.0);
    if item(&mut sui, "Home", Some(Icon::Generic), page == Page::Home) {
        page = Page::Home;
    }
    if item(&mut sui, "New", Some(Icon::BlankPage), page == Page::New) {
        page = Page::New;
    }
    if item(&mut sui, "Open", Some(Icon::Envelopes), page == Page::Open) {
        page = Page::Open;
    }
    sui.add_space(8.0);
    sui.painter().hline((side.left() + 16.0)..=(side.right() - 16.0), sui.cursor().top(), Stroke::new(1.0, SEP));
    sui.add_space(8.0);
    if item(&mut sui, "Info", None, page == Page::Info) {
        page = Page::Info;
    }
    if item(&mut sui, "Save", None, false) {
        cmds.push(Cmd::Save);
        app.backstage = false;
    }
    if item(&mut sui, "Save As", None, false) {
        cmds.push(Cmd::SaveAs);
        app.backstage = false;
    }
    if item(&mut sui, "Print", None, false) {
        cmds.push(Cmd::Unsupported("Print"));
    }
    if item(&mut sui, "Export", None, false) {
        cmds.push(Cmd::Unsupported("Export to PDF"));
    }
    if item(&mut sui, "Close", None, false) {
        cmds.push(Cmd::CloseDoc);
    }
    ui.data_mut(|d| d.insert_temp(id, page));

    let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(content.shrink2(vec2(40.0, 30.0))));
    match page {
        Page::Home | Page::New => {
            cui.label(egui::RichText::new(if page == Page::Home { "Good day" } else { "New" }).size(26.0).color(TEXT));
            cui.add_space(16.0);
            let (rect, r) = cui.allocate_exact_size(vec2(150.0, 194.0), Sense::click());
            let pg = Rect::from_min_size(rect.min, vec2(150.0, 170.0));
            cui.painter().rect_filled(pg, 2.0, Color32::WHITE);
            if r.hovered() {
                cui.painter().rect_stroke(pg, 2.0, Stroke::new(2.0, ACCENT), StrokeKind::Outside);
            }
            cui.painter().text(pos2(rect.center().x, pg.bottom() + 14.0), Align2::CENTER_CENTER, "Blank document", theme::ui_font(13.0), TEXT);
            if r.clicked() {
                cmds.push(Cmd::New);
                app.backstage = false;
            }
            if page == Page::Home {
                cui.add_space(24.0);
                recent_list(app, &mut cui, cmds);
            }
        }
        Page::Open => {
            cui.label(egui::RichText::new("Open").size(26.0).color(TEXT));
            cui.add_space(16.0);
            if cui.add(egui::Button::new(egui::RichText::new("  Browse…  ").size(14.0)).min_size(vec2(140.0, 34.0))).clicked() {
                cmds.push(Cmd::Open);
            }
            cui.add_space(20.0);
            recent_list(app, &mut cui, cmds);
        }
        Page::Info => {
            cui.label(egui::RichText::new("Info").size(26.0).color(TEXT));
            cui.add_space(12.0);
            let path = app.doc.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Not saved yet".into());
            cui.label(egui::RichText::new(app.doc.title()).size(18.0).color(TEXT));
            cui.label(egui::RichText::new(path).color(TEXT_DIM));
            cui.add_space(16.0);
            let pages = app.layout.as_ref().map(|l| l.pages.len()).unwrap_or(0);
            egui::Grid::new("info").num_columns(2).spacing([30.0, 8.0]).show(&mut cui, |ui| {
                ui.label("Pages");
                ui.label(pages.to_string());
                ui.end_row();
                ui.label("Words");
                ui.label(app.word_count().to_string());
                ui.end_row();
                ui.label("Parts in package");
                ui.label(app.doc.package.parts.len().to_string());
                ui.end_row();
                ui.label("Compatibility mode");
                ui.label(match app.doc.settings.compat_mode {
                    15 => "Mode 15 (current)".to_string(),
                    14 => "Mode 14".to_string(),
                    12 => "Mode 12".to_string(),
                    n => format!("Legacy ({n})"),
                });
                ui.end_row();
                ui.label("Styles");
                ui.label(app.doc.styles.map.len().to_string());
                ui.end_row();
            });
            if !app.doc.warnings.is_empty() {
                cui.add_space(12.0);
                for w in &app.doc.warnings {
                    cui.colored_label(Color32::from_rgb(0xF0, 0xA0, 0x40), w);
                }
            }
        }
    }
}

fn recent_list(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    ui.label(egui::RichText::new("Recent").size(15.0).color(TEXT));
    ui.add_space(6.0);
    if app.recent.is_empty() {
        ui.label(egui::RichText::new("Documents you open will show up here.").color(TEXT_DIM));
    }
    for p in app.recent.clone() {
        let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let dir = p.parent().map(|d| d.display().to_string()).unwrap_or_default();
        let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width().min(700.0), 44.0), Sense::click());
        if r.hovered() {
            ui.painter().rect_filled(rect, 4.0, Color32::from_rgb(0x33, 0x33, 0x33));
        }
        icons::paint(ui.painter(), Icon::Generic, Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(20.0, 24.0)), ACCENT_TEXT);
        ui.painter().text(pos2(rect.left() + 44.0, rect.top() + 14.0), Align2::LEFT_CENTER, name, theme::ui_font(13.5), TEXT);
        ui.painter().text(pos2(rect.left() + 44.0, rect.top() + 31.0), Align2::LEFT_CENTER, dir, theme::ui_font(11.5), TEXT_DIM);
        if r.clicked() {
            cmds.push(Cmd::OpenPath(p.clone()));
        }
    }
}
