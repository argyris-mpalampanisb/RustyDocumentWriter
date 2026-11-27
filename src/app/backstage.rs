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
    Theme,
}

pub fn show(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    let full = ui.max_rect();
    let area = Rect::from_min_max(pos2(full.left(), ui.cursor().top()), full.right_bottom());
    let side = Rect::from_min_max(area.left_top(), pos2(area.left() + 220.0, area.bottom()));
    let content = Rect::from_min_max(pos2(side.right(), area.top()), area.right_bottom());
    let p = ui.painter().clone();
    p.rect_filled(side, 0.0, pal().backstage_side);
    p.rect_filled(content, 0.0, pal().backstage);
    let id = egui::Id::NULL.with("backstage-page");
    let mut page: Page = ui.data(|d| d.get_temp(id)).unwrap_or(Page::Home);

    let mut sui = ui.new_child(egui::UiBuilder::new().max_rect(side.shrink2(vec2(0.0, 10.0))));
    let mut item = |ui: &mut Ui, label: &str, icon: Option<Icon>, selected: bool| -> bool {
        let (rect, r) = ui.allocate_exact_size(vec2(side.width(), 40.0), Sense::click());
        if selected {
            ui.painter().rect_filled(rect, 0.0, pal().checked);
        } else if r.hovered() {
            ui.painter().rect_filled(rect, 0.0, pal().hover);
        }
        if let Some(ic) = icon {
            icons::paint(ui.painter(), ic, Rect::from_center_size(pos2(rect.left() + 28.0, rect.center().y), vec2(18.0, 18.0)), pal().text);
        }
        ui.painter().text(pos2(rect.left() + 52.0, rect.center().y), Align2::LEFT_CENTER, label, theme::ui_font(14.0), pal().text);
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
    sui.painter().hline((side.left() + 16.0)..=(side.right() - 16.0), sui.cursor().top(), Stroke::new(1.0, pal().sep));
    sui.add_space(8.0);
    if item(&mut sui, "Info", None, page == Page::Info) {
        page = Page::Info;
    }
    if item(&mut sui, "Theme", Some(Icon::DarkMode), page == Page::Theme) {
        page = Page::Theme;
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
        cmds.push(Cmd::Export);
        app.backstage = false;
    }
    if item(&mut sui, "Close", None, false) {
        cmds.push(Cmd::CloseDoc);
    }
    ui.data_mut(|d| d.insert_temp(id, page));

    let mut cui = ui.new_child(egui::UiBuilder::new().max_rect(content.shrink2(vec2(40.0, 30.0))));
    match page {
        Page::Home | Page::New => {
            cui.label(egui::RichText::new(if page == Page::Home { "Good day" } else { "New" }).size(26.0).color(pal().text));
            cui.add_space(16.0);
            let (rect, r) = cui.allocate_exact_size(vec2(150.0, 194.0), Sense::click());
            let pg = Rect::from_min_size(rect.min, vec2(150.0, 170.0));
            cui.painter().rect_filled(pg, 2.0, Color32::WHITE);
            if r.hovered() {
                cui.painter().rect_stroke(pg, 2.0, Stroke::new(2.0, pal().accent), StrokeKind::Outside);
            }
            cui.painter().text(pos2(rect.center().x, pg.bottom() + 14.0), Align2::CENTER_CENTER, "Blank document", theme::ui_font(13.0), pal().text);
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
            cui.label(egui::RichText::new("Open").size(26.0).color(pal().text));
            cui.add_space(16.0);
            if cui.add(egui::Button::new(egui::RichText::new("  Browse…  ").size(14.0)).min_size(vec2(140.0, 34.0))).clicked() {
                cmds.push(Cmd::Open);
            }
            cui.add_space(20.0);
            recent_list(app, &mut cui, cmds);
        }
        Page::Theme => theme_page(&mut cui),
        Page::Info => {
            cui.label(egui::RichText::new("Info").size(26.0).color(pal().text));
            cui.add_space(12.0);
            let path = app.doc.path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "Not saved yet".into());
            cui.label(egui::RichText::new(app.doc.title()).size(18.0).color(pal().text));
            cui.label(egui::RichText::new(path).color(pal().text_dim));
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
    let width = ui.available_width().min(700.0);
    // Header row: "Recent" on the left, "Clear all" on the right.
    let (hdr, _) = ui.allocate_exact_size(vec2(width, 22.0), Sense::hover());
    ui.painter().text(hdr.left_center(), Align2::LEFT_CENTER, "Recent", theme::ui_font(15.0), pal().text);
    if !app.recent.is_empty() {
        let galley = ui.painter().layout_no_wrap("Clear all".into(), theme::ui_font(12.5), pal().accent_text);
        let cr = Rect::from_min_size(pos2(hdr.right() - galley.size().x - 8.0, hdr.center().y - 10.0), vec2(galley.size().x + 8.0, 20.0));
        let r = ui.interact(cr, egui::Id::NULL.with("recent-clear"), Sense::click()).on_hover_text("Remove every document from the Recent list");
        if r.hovered() {
            ui.painter().rect_filled(cr, 3.0, pal().hover);
        }
        ui.painter().galley(pos2(cr.left() + 4.0, cr.center().y - galley.size().y / 2.0), galley, pal().accent_text);
        if r.clicked() {
            cmds.push(Cmd::ClearRecent);
        }
    }
    ui.add_space(6.0);
    if app.recent.is_empty() {
        ui.label(egui::RichText::new("Documents you open will show up here.").color(pal().text_dim));
        return;
    }
    egui::ScrollArea::vertical().id_salt("recent-scroll").auto_shrink([false, true]).show(ui, |ui| {
        for p in app.recent.clone() {
            recent_row(ui, &p, width, cmds);
        }
    });
}

fn recent_row(ui: &mut Ui, p: &std::path::Path, width: f32, cmds: &mut Vec<Cmd>) {
    let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    let dir = p.parent().map(|d| d.display().to_string()).unwrap_or_default();
    let (rect, r) = ui.allocate_exact_size(vec2(width, 44.0), Sense::click());
    // The remove button sits on top of the row, so it takes the click when hovered.
    let xr = Rect::from_center_size(pos2(rect.right() - 22.0, rect.center().y), vec2(26.0, 26.0));
    let xresp = ui.interact(xr, egui::Id::NULL.with(("recent-remove", p)), Sense::click()).on_hover_text("Remove from list");
    let row_hot = r.hovered() || xresp.hovered() || r.context_menu_opened();
    let painter = ui.painter_at(rect);
    if row_hot {
        painter.rect_filled(rect, 4.0, pal().hover);
    }
    icons::paint(&painter, Icon::Generic, Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(20.0, 24.0)), pal().accent_text);
    // Clip long names/paths before the remove button instead of running under it.
    let text_clip = Rect::from_min_max(rect.min, pos2(xr.left() - 6.0, rect.bottom()));
    let tp = ui.painter_at(text_clip);
    tp.text(pos2(rect.left() + 44.0, rect.top() + 14.0), Align2::LEFT_CENTER, &name, theme::ui_font(13.5), pal().text);
    tp.text(pos2(rect.left() + 44.0, rect.top() + 31.0), Align2::LEFT_CENTER, &dir, theme::ui_font(11.5), pal().text_dim);
    if row_hot {
        if xresp.hovered() {
            painter.rect_filled(xr, 4.0, pal().pressed);
        }
        let c = xr.center();
        let s = 4.5;
        let stroke = Stroke::new(1.4, if xresp.hovered() { pal().text } else { pal().text_dim });
        painter.line_segment([c + vec2(-s, -s), c + vec2(s, s)], stroke);
        painter.line_segment([c + vec2(-s, s), c + vec2(s, -s)], stroke);
    }
    if xresp.clicked() {
        cmds.push(Cmd::RemoveRecent(p.to_path_buf()));
    } else if r.clicked() {
        cmds.push(Cmd::OpenPath(p.to_path_buf()));
    }
    let r = r.on_hover_text(p.display().to_string());
    r.context_menu(|ui| {
        ui.set_min_width(180.0);
        if ui.button("Open").clicked() {
            cmds.push(Cmd::OpenPath(p.to_path_buf()));
        }
        if ui.button("Copy path").clicked() {
            ui.ctx().copy_text(p.display().to_string());
        }
        ui.separator();
        if ui.button("Remove from list").clicked() {
            cmds.push(Cmd::RemoveRecent(p.to_path_buf()));
        }
        if ui.button("Clear all recent documents").clicked() {
            cmds.push(Cmd::ClearRecent);
        }
    });
}

/// File ▸ Theme: every UI theme as a miniature window, grouped by where it comes from.
fn theme_page(ui: &mut Ui) {
    ui.label(egui::RichText::new("Theme").size(26.0).color(pal().text));
    ui.label(egui::RichText::new("Colours for the ribbon, menus and the area around the pages. Your documents are not changed.").color(pal().text_dim));
    ui.add_space(8.0);
    let mut pick = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for source in ["Word", "VS Code", "Zed"] {
            ui.add_space(10.0);
            ui.label(egui::RichText::new(source).size(15.0).color(pal().text));
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
                for (i, p) in theme::PALETTES.iter().enumerate().filter(|(_, p)| p.source == source) {
                    if theme_card(ui, p, i == theme::current()).clicked() {
                        pick = Some(i);
                    }
                }
            });
        }
        ui.add_space(20.0);
    });
    if let Some(i) = pick {
        theme::set(ui.ctx(), i);
        theme::save();
    }
}

/// A miniature of the app drawn in palette `t`.
fn theme_card(ui: &mut Ui, t: &theme::Palette, selected: bool) -> egui::Response {
    let (rect, r) = ui.allocate_exact_size(vec2(176.0, 132.0), Sense::click());
    let p = ui.painter();
    let win = Rect::from_min_size(rect.min, vec2(rect.width(), 104.0));
    p.rect_filled(win, 6.0, t.canvas);
    let title = Rect::from_min_size(win.min, vec2(win.width(), 14.0));
    p.rect_filled(title, egui::CornerRadius { nw: 6, ne: 6, sw: 0, se: 0 }, t.bg);
    p.circle_filled(pos2(title.left() + 8.0, title.center().y), 2.5, t.accent);
    p.hline((title.left() + 14.0)..=(title.left() + 50.0), title.center().y, Stroke::new(2.0, t.text_dim));
    let ribbon = Rect::from_min_size(pos2(win.left(), title.bottom()), vec2(win.width(), 28.0));
    p.rect_filled(ribbon, 0.0, t.ribbon);
    p.hline(ribbon.x_range(), ribbon.bottom(), Stroke::new(1.0, t.sep));
    // Tabs, one of them hovered, and a couple of "buttons".
    for (k, w) in [16.0, 20.0, 14.0, 18.0].into_iter().enumerate() {
        let x = ribbon.left() + 8.0 + k as f32 * 26.0;
        if k == 1 {
            p.rect_filled(Rect::from_min_size(pos2(x - 2.0, ribbon.top() + 3.0), vec2(w + 4.0, 8.0)), 2.0, t.hover);
        }
        p.hline(x..=(x + w), ribbon.top() + 7.0, Stroke::new(2.0, if k == 0 { t.accent_text } else { t.text }));
    }
    for k in 0..4 {
        let b = Rect::from_min_size(pos2(ribbon.left() + 8.0 + k as f32 * 16.0, ribbon.top() + 14.0), vec2(12.0, 10.0));
        if k == 2 {
            p.rect_filled(b.expand(1.0), 2.0, t.checked);
        }
        p.rect_filled(b.shrink(2.5), 1.0, t.icon);
    }
    let share = Rect::from_min_size(pos2(ribbon.right() - 34.0, ribbon.top() + 14.0), vec2(26.0, 10.0));
    p.rect_filled(share, 2.0, t.accent);
    p.hline((share.left() + 6.0)..=(share.right() - 6.0), share.center().y, Stroke::new(1.5, Color32::WHITE));
    // A page on the canvas.
    let page = Rect::from_min_max(pos2(win.center().x - 30.0, ribbon.bottom() + 8.0), pos2(win.center().x + 30.0, win.bottom()));
    p.rect_filled(page, 0.0, Color32::WHITE);
    for k in 0..4 {
        let y = page.top() + 10.0 + k as f32 * 6.0;
        p.hline((page.left() + 8.0)..=(page.right() - if k == 3 { 24.0 } else { 8.0 }), y, Stroke::new(1.5, Color32::from_gray(0x9A)));
    }
    let outline = if selected { pal().accent_text } else if r.hovered() { pal().border() } else { pal().sep };
    p.rect_stroke(win, 6.0, Stroke::new(if selected { 2.5 } else { 1.0 }, outline), StrokeKind::Outside);
    let label = if selected { format!("✓ {}", t.name) } else { t.name.to_string() };
    p.text(pos2(rect.left() + 2.0, win.bottom() + 15.0), Align2::LEFT_CENTER, label, theme::ui_font(13.0), if selected { pal().accent_text } else { pal().text });
    r.on_hover_text(format!("{} ({})", t.name, if t.dark { "dark" } else { "light" }))
}
