//! Title bar, tab strip, ribbon and status bar — dark ribbon theme.

use super::icons::{self, Icon};
use super::theme::{self, *};
use super::{App, Cmd, Tab};
use egui::{pos2, vec2, Align2, Popup, Color32, CornerRadius, CursorIcon, Id, Rect, Response, Sense, Stroke, StrokeKind, Ui};
use rusty_document_writer::props::Align;

const TITLE_H: f32 = 40.0;
const TABS_H: f32 = 32.0;
const CONTENT_H: f32 = 76.0;
const LABEL_H: f32 = 18.0;
pub const STATUS_H: f32 = 26.0;

// ---------------------------------------------------------------------------
// Widgets

fn paint_bg(ui: &Ui, r: &Response, rect: Rect, checked: bool, enabled: bool) {
    let p = ui.painter();
    if !enabled {
        return;
    }
    if r.is_pointer_button_down_on() {
        p.rect_filled(rect, 4.0, pal().pressed);
    } else if r.hovered() {
        p.rect_filled(rect, 4.0, pal().hover);
    } else if checked {
        p.rect_filled(rect, 4.0, pal().checked);
    }
    if checked {
        p.rect_stroke(rect, 4.0, Stroke::new(1.0, pal().border()), StrokeKind::Inside);
    }
}

fn chevron(ui: &Ui, center: egui::Pos2, color: Color32) {
    let p = ui.painter();
    p.add(egui::Shape::line(vec![center + vec2(-3.0, -1.5), center + vec2(0.0, 1.5), center + vec2(3.0, -1.5)], Stroke::new(1.1, color)));
}

fn text_width(ui: &Ui, text: &str, size: f32) -> f32 {
    ui.painter().layout_no_wrap(text.to_string(), theme::ui_font(size), pal().text).size().x
}

/// Large ribbon button: 32px icon above a (possibly two-line) label.
pub fn big(ui: &mut Ui, icon: Icon, label: &str, dropdown: bool, enabled: bool) -> Response {
    let lines: Vec<&str> = label.split('\n').collect();
    let tw = lines.iter().map(|l| text_width(ui, l, 12.0)).fold(0.0, f32::max) + if dropdown && lines.len() == 1 { 0.0 } else { 0.0 };
    let w = (tw + 10.0).max(46.0);
    let (rect, r) = ui.allocate_exact_size(vec2(w, CONTENT_H), Sense::click());
    paint_bg(ui, &r, rect, false, enabled);
    let fg = if enabled { pal().icon } else { pal().text_disabled };
    let ir = Rect::from_center_size(pos2(rect.center().x, rect.top() + 22.0), vec2(30.0, 30.0));
    icons::paint(ui.painter(), icon, ir, fg);
    let tc = if enabled { pal().text } else { pal().text_disabled };
    let mut y = rect.top() + 46.0;
    for (i, l) in lines.iter().enumerate() {
        let last = i + 1 == lines.len();
        let extra = if dropdown && last && lines.len() > 1 { 7.0 } else { 0.0 };
        ui.painter().text(pos2(rect.center().x - extra, y), Align2::CENTER_CENTER, *l, theme::ui_font(12.0), tc);
        if dropdown && last && lines.len() > 1 {
            let lw = text_width(ui, l, 12.0);
            chevron(ui, pos2(rect.center().x + lw / 2.0, y + 1.0), tc);
        }
        y += 15.0;
    }
    if dropdown && lines.len() == 1 {
        chevron(ui, pos2(rect.center().x, y + 1.0), tc);
    }
    if enabled { r.on_hover_text(label.replace('\n', " ")) } else { r }
}

/// Small ribbon button: 16px icon + label (stacked three per column).
pub fn small(ui: &mut Ui, icon: Icon, label: &str, dropdown: bool, enabled: bool) -> Response {
    let tw = text_width(ui, label, 12.5);
    let w = 28.0 + tw + if dropdown { 14.0 } else { 6.0 };
    let (rect, r) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    paint_bg(ui, &r, rect, false, enabled);
    let fg = if enabled { pal().icon } else { pal().text_disabled };
    icons::paint(ui.painter(), icon, Rect::from_center_size(pos2(rect.left() + 13.0, rect.center().y), vec2(16.0, 16.0)), fg);
    let tc = if enabled { pal().text } else { pal().text_disabled };
    ui.painter().text(pos2(rect.left() + 26.0, rect.center().y), Align2::LEFT_CENTER, label, theme::ui_font(12.5), tc);
    if dropdown {
        chevron(ui, pos2(rect.right() - 8.0, rect.center().y), tc);
    }
    r
}

/// Square icon-only button with tooltip (formatting buttons).
pub fn icon_btn(ui: &mut Ui, icon: Icon, tip: &str, checked: bool, dropdown: bool) -> Response {
    let w = if dropdown { 36.0 } else { 26.0 };
    let (rect, r) = ui.allocate_exact_size(vec2(w, 24.0), Sense::click());
    paint_bg(ui, &r, rect, checked, true);
    let ir = Rect::from_center_size(pos2(rect.left() + 13.0, rect.center().y), vec2(16.0, 16.0));
    icons::paint(ui.painter(), icon, ir, pal().icon);
    if dropdown {
        chevron(ui, pos2(rect.right() - 6.0, rect.center().y), pal().icon);
    }
    r.on_hover_text(tip)
}

fn group(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    let r = ui.vertical(|ui| {
        let inner = ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing = vec2(2.0, 1.0);
            ui.set_min_height(CONTENT_H);
            add(ui);
        });
        let w = inner.response.rect.width().max(text_width(ui, label, 11.5) + 8.0);
        let (lr, _) = ui.allocate_exact_size(vec2(w, LABEL_H), Sense::hover());
        ui.painter().text(lr.center(), Align2::CENTER_CENTER, label, theme::ui_font(11.5), pal().text_dim);
    });
    let rect = r.response.rect;
    ui.add_space(5.0);
    let x = ui.cursor().left();
    ui.painter().vline(x, (rect.top() + 4.0)..=(rect.bottom() - 4.0), Stroke::new(1.0, pal().sep));
    ui.add_space(6.0);
}

fn column(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = vec2(1.0, 1.0);
        ui.add_space(1.0);
        add(ui);
    });
}

fn popup(resp: &Response, add: impl FnOnce(&mut Ui)) {
    egui::Popup::from_toggle_button_response(resp).show(|ui| {
        ui.set_min_width(160.0);
        add(ui);
    });
}

fn menu_item(ui: &mut Ui, text: &str) -> bool {
    ui.add(egui::Button::new(text).frame(false).min_size(vec2(ui.available_width(), 24.0))).clicked()
}

// ---------------------------------------------------------------------------
// Title bar

fn window_button(ui: &mut Ui, rect: Rect, glyph: &str, fallback: &str, close: bool, icon_font: bool) -> Response {
    let r = ui.interact(rect, Id::NULL.with(("winbtn", glyph)), Sense::click());
    if r.hovered() {
        ui.painter().rect_filled(rect, 0.0, if close { Color32::from_rgb(0xC4, 0x2B, 0x1C) } else { pal().hover });
    }
    if icon_font {
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, glyph, theme::icon_font(10.0), pal().text);
    } else {
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, fallback, theme::ui_font(14.0), pal().text);
    }
    r
}

fn qat_button(ui: &mut Ui, x: f32, rect_y: f32, glyph: &str, fallback: Icon, enabled: bool, icon_font: bool, tip: &str) -> (Response, f32) {
    let rect = Rect::from_min_size(pos2(x, rect_y), vec2(30.0, 30.0));
    let r = ui.interact(rect, Id::NULL.with(("qat", glyph)), Sense::click());
    if r.hovered() && enabled {
        ui.painter().rect_filled(rect, 4.0, pal().hover);
    }
    let c = if enabled { pal().text } else { pal().text_disabled };
    if icon_font {
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, glyph, theme::icon_font(15.0), c);
    } else {
        icons::paint(ui.painter(), fallback, rect.shrink(7.0), c);
    }
    (r.on_hover_text(tip), x + 30.0)
}

pub fn window_chrome(app: &mut App, ui: &mut Ui, title: &str, cmds: &mut Vec<Cmd>) {
    let full = ui.max_rect();
    let (bar, bar_resp) = ui.allocate_exact_size(vec2(full.width(), TITLE_H), Sense::click_and_drag());
    let ctx = ui.ctx().clone();
    if bar_resp.drag_started() {
        ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    if bar_resp.double_clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
    let p = ui.painter().clone();
    p.rect_filled(bar, 0.0, pal().bg);
    // App logo: a stylised "R" page.
    let logo = Rect::from_min_size(pos2(bar.left() + 14.0, bar.center().y - 9.0), vec2(18.0, 18.0));
    p.rect_filled(logo, 3.0, Color32::from_rgb(0x6B, 0x4F, 0xD8));
    p.rect_filled(Rect::from_min_max(pos2(logo.center().x, logo.top()), logo.right_bottom()), CornerRadius { nw: 0, ne: 3, sw: 0, se: 3 }, Color32::from_rgb(0x8E, 0x74, 0xF0));
    p.text(logo.center(), Align2::CENTER_CENTER, "R", theme::bold_font(12.0), Color32::WHITE);

    let mut x = logo.right() + 14.0;
    p.text(pos2(x, bar.center().y), Align2::LEFT_CENTER, "AutoSave", theme::ui_font(13.0), pal().text);
    x += text_width(ui, "AutoSave", 13.0) + 8.0;
    // AutoSave toggle (off: requires cloud storage).
    let tr = Rect::from_min_size(pos2(x, bar.center().y - 10.0), vec2(40.0, 20.0));
    p.rect_stroke(tr, 10.0, Stroke::new(1.0, pal().text), StrokeKind::Inside);
    p.circle_filled(pos2(tr.left() + 10.0, tr.center().y), 6.0, pal().text);
    ui.interact(tr, Id::NULL.with("autosave"), Sense::hover()).on_hover_text("AutoSave needs cloud storage — use Save (Ctrl+S)");
    x = tr.right() + 10.0;
    let y = bar.center().y - 15.0;
    let icf = app.icon_font;
    let (r, nx) = qat_button(ui, x, y, "\u{E74E}", Icon::Generic, true, icf, "Save (Ctrl+S)");
    if r.clicked() {
        cmds.push(Cmd::Save);
    }
    let (r, nx) = qat_button(ui, nx, y, "\u{E7A7}", Icon::Prev, app.can_undo(), icf, "Undo (Ctrl+Z)");
    if r.clicked() {
        cmds.push(Cmd::Undo);
    }
    chevron(ui, pos2(nx + 2.0, bar.center().y), pal().text_disabled);
    let (r, nx) = qat_button(ui, nx + 8.0, y, "\u{E7A6}", Icon::Next, app.can_redo(), icf, "Redo (Ctrl+Y)");
    if r.clicked() {
        cmds.push(Cmd::Redo);
    }
    chevron(ui, pos2(nx + 10.0, bar.center().y), pal().text);
    let title_x = nx + 34.0;
    p.text(pos2(title_x, bar.center().y), Align2::LEFT_CENTER, title, theme::ui_font(13.0), pal().text);

    // Search box (centred) — opens Find & Replace.
    let sw = (full.width() * 0.22).clamp(200.0, 430.0);
    let sx = (full.center().x - sw / 2.0).max(title_x + text_width(ui, title, 13.0) + 20.0);
    let sr = Rect::from_min_size(pos2(sx, bar.center().y - 15.0), vec2(sw, 30.0));
    if sr.right() < full.right() - 160.0 {
        let resp = ui.interact(sr, Id::NULL.with("searchbox"), Sense::click());
        p.rect_filled(sr, 6.0, pal().field);
        p.rect_stroke(sr, 6.0, Stroke::new(1.0, if resp.hovered() { pal().border() } else { pal().sep }), StrokeKind::Inside);
        if icf {
            p.text(pos2(sr.left() + 18.0, sr.center().y), Align2::CENTER_CENTER, "\u{E721}", theme::icon_font(13.0), pal().text_dim);
        } else {
            icons::paint(&p, Icon::Find, Rect::from_center_size(pos2(sr.left() + 18.0, sr.center().y), vec2(14.0, 14.0)), pal().text_dim);
        }
        p.text(pos2(sr.left() + 36.0, sr.center().y), Align2::LEFT_CENTER, "Search", theme::ui_font(13.0), pal().text_dim);
        if resp.clicked() {
            cmds.push(Cmd::Find);
        }
    }

    // Window buttons.
    let bw = 46.0;
    let close = Rect::from_min_size(pos2(bar.right() - bw, bar.top()), vec2(bw, TITLE_H));
    let maxr = close.translate(vec2(-bw, 0.0));
    let minr = maxr.translate(vec2(-bw, 0.0));
    if window_button(ui, minr, "\u{E921}", "–", false, icf).clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
    }
    if window_button(ui, maxr, if maximized { "\u{E923}" } else { "\u{E922}" }, "□", false, icf).clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
    }
    if window_button(ui, close, "\u{E8BB}", "✕", true, icf).clicked() {
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

/// Resize handles for the undecorated window.
pub fn resize_edges(ctx: &egui::Context) {
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false)) {
        return;
    }
    let screen = ctx.content_rect();
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let m = 5.0;
    let l = pos.x < screen.left() + m;
    let r = pos.x > screen.right() - m;
    let t = pos.y < screen.top() + m;
    let b = pos.y > screen.bottom() - m;
    use egui::viewport::ResizeDirection as D;
    let dir = match (l, r, t, b) {
        (true, _, true, _) => Some((D::NorthWest, CursorIcon::ResizeNorthWest)),
        (_, true, true, _) => Some((D::NorthEast, CursorIcon::ResizeNorthEast)),
        (true, _, _, true) => Some((D::SouthWest, CursorIcon::ResizeSouthWest)),
        (_, true, _, true) => Some((D::SouthEast, CursorIcon::ResizeSouthEast)),
        (true, ..) => Some((D::West, CursorIcon::ResizeWest)),
        (_, true, ..) => Some((D::East, CursorIcon::ResizeEast)),
        (_, _, true, _) => Some((D::North, CursorIcon::ResizeNorth)),
        (_, _, _, true) => Some((D::South, CursorIcon::ResizeSouth)),
        _ => None,
    };
    if let Some((d, c)) = dir {
        ctx.set_cursor_icon(c);
        if ctx.input(|i| i.pointer.primary_pressed()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(d));
        }
    }
}

// ---------------------------------------------------------------------------
// Tabs

pub fn tabs_row(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    let full = ui.max_rect();
    let (row, _) = ui.allocate_exact_size(vec2(full.width(), TABS_H), Sense::hover());
    let p = ui.painter().clone();
    let mut x = row.left() + 6.0;
    let mut draw_tab = |ui: &mut Ui, label: &str, selected: bool| -> Response {
        let font = if selected { theme::bold_font(13.5) } else { theme::ui_font(13.5) };
        let w = p.layout_no_wrap(label.to_string(), font.clone(), pal().text).size().x + 16.0;
        let rect = Rect::from_min_size(pos2(x, row.top() + 2.0), vec2(w, TABS_H - 4.0));
        x += w + 4.0;
        let r = ui.interact(rect, Id::NULL.with(("tab", label)), Sense::click());
        if r.hovered() && !selected {
            p.rect_filled(rect, 4.0, pal().hover);
        }
        p.text(rect.center(), Align2::CENTER_CENTER, label, font, pal().text);
        if selected {
            let tw = p.layout_no_wrap(label.to_string(), theme::bold_font(13.5), pal().text).size().x;
            p.rect_filled(Rect::from_center_size(pos2(rect.center().x, rect.bottom() - 2.0), vec2(tw, 2.5)), 1.0, pal().text);
        }
        r
    };
    if draw_tab(ui, "File", false).clicked() {
        app.backstage = true;
    }
    for t in Tab::ALL {
        let collapsed = app.settings.ribbon_collapsed;
        // While collapsed, the selected tab is only underlined when its ribbon is showing.
        let r = draw_tab(ui, t.label(), app.tab == t && (!collapsed || app.ribbon_peek));
        if r.double_clicked() {
            // Word: double-clicking a tab collapses or pins the ribbon.
            cmds.push(Cmd::ToggleRibbon);
        } else if r.clicked() {
            if collapsed {
                app.ribbon_peek = !(app.ribbon_peek && app.tab == t);
            }
            app.tab = t;
        }
    }
    // "Share" button: saves a copy (no cloud integration).
    let label = "Share";
    let sw = text_width(ui, label, 13.0) + 44.0;
    let sr = Rect::from_min_size(pos2(row.right() - sw - 12.0, row.top() + 3.0), vec2(sw, TABS_H - 6.0));
    let r = ui.interact(sr, Id::NULL.with("share"), Sense::click());
    p.rect_filled(sr, 4.0, if r.hovered() { pal().accent.lerp_to_gamma(Color32::WHITE, 0.12) } else { pal().accent });
    icons::paint(&p, Icon::Share, Rect::from_center_size(pos2(sr.left() + 15.0, sr.center().y), vec2(14.0, 14.0)), Color32::WHITE);
    p.text(pos2(sr.left() + 28.0, sr.center().y), Align2::LEFT_CENTER, label, theme::ui_font(13.0), Color32::WHITE);
    if r.on_hover_text("Save a copy to share").clicked() {
        cmds.push(Cmd::SaveAs);
    }
}

// ---------------------------------------------------------------------------
// Ribbon

pub fn ribbon(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    let full = ui.max_rect();
    let h = CONTENT_H + LABEL_H + 10.0;
    let (outer, _) = ui.allocate_exact_size(vec2(full.width(), h + 8.0), Sense::hover());
    let panel = Rect::from_min_max(pos2(outer.left() + 10.0, outer.top() + 2.0), pos2(outer.right() - 10.0, outer.bottom() - 6.0));
    ui.painter().rect_filled(panel, 8.0, pal().ribbon);
    let inner = panel.shrink2(vec2(10.0, 4.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::left_to_right(egui::Align::Min)));
    child.set_clip_rect(panel);
    match app.tab {
        Tab::Home => home(app, &mut child, cmds),
        Tab::Insert => insert(app, &mut child, cmds),
        Tab::Design => design(app, &mut child, cmds),
        Tab::Layout => layout_tab(app, &mut child, cmds),
        Tab::References => references(&mut child, cmds),
        Tab::Mailings => mailings(&mut child, cmds),
        Tab::Review => review(&mut child, cmds),
        Tab::View => view(app, &mut child, cmds),
        Tab::Help => help(&mut child, cmds),
    }
    // Collapse / pin button.
    let pin = Rect::from_center_size(pos2(panel.right() - 14.0, panel.bottom() - 12.0), vec2(20.0, 16.0));
    let r = ui.interact(pin, Id::NULL.with("ribbon-pin"), Sense::click());
    if r.hovered() {
        ui.painter().rect_filled(pin, 3.0, pal().hover);
    }
    let c = pin.center();
    if app.settings.ribbon_collapsed {
        // A pin: keep the ribbon open.
        let p = ui.painter();
        p.circle_stroke(c + vec2(0.0, -2.0), 3.0, Stroke::new(1.2, pal().text_dim));
        p.line_segment([c + vec2(0.0, 1.0), c + vec2(0.0, 6.0)], Stroke::new(1.2, pal().text_dim));
    } else {
        // An up-chevron: collapse.
        let s = Stroke::new(1.3, pal().text_dim);
        ui.painter().line_segment([c + vec2(-4.0, 2.0), c + vec2(0.0, -2.0)], s);
        ui.painter().line_segment([c + vec2(0.0, -2.0), c + vec2(4.0, 2.0)], s);
    }
    let tip = if app.settings.ribbon_collapsed { "Keep the ribbon open (Ctrl+F1)" } else { "Collapse the ribbon (Ctrl+F1)" };
    if r.on_hover_text(tip).clicked() {
        cmds.push(Cmd::ToggleRibbon);
    }
}

/// Height of the status bar, or 0 when it's hidden.
pub fn status_h(app: &App) -> f32 {
    if app.settings.show_status_bar { STATUS_H } else { 0.0 }
}

const THEME_COLORS: [[&str; 10]; 1] = [["FFFFFF", "000000", "E8E8E8", "0E2841", "156082", "E97132", "196B24", "0F9ED5", "A02B93", "4EA72E"]];
const STD_COLORS: [&str; 10] = ["C00000", "FF0000", "FFC000", "FFFF00", "92D050", "00B050", "00B0F0", "0070C0", "002060", "7030A0"];

fn color_grid(ui: &mut Ui, automatic: &str) -> Option<Option<String>> {
    let mut out = None;
    if menu_item(ui, automatic) {
        out = Some(None);
    }
    ui.label(egui::RichText::new("Theme Colors").small().color(pal().text_dim));
    for row in THEME_COLORS {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for hex in row {
                if swatch(ui, hex) {
                    out = Some(Some(hex.to_string()));
                }
            }
        });
    }
    // Tints of the theme colours.
    for t in [0.8f32, 0.6, 0.4] {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            for hex in THEME_COLORS[0] {
                let c = rusty_document_writer::props::Color::from_hex(hex).unwrap().tint(1.0 - t * 0.5);
                if swatch(ui, &c.to_hex()) {
                    out = Some(Some(c.to_hex()));
                }
            }
        });
    }
    ui.label(egui::RichText::new("Standard Colors").small().color(pal().text_dim));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for hex in STD_COLORS {
            if swatch(ui, hex) {
                out = Some(Some(hex.to_string()));
            }
        }
    });
    out
}

// ---------------------------------------------------------------------------
// Underline styles & text effects

const UNDERLINES: [(&str, &str); 11] = [
    ("single", "Single"),
    ("words", "Words only"),
    ("double", "Double"),
    ("thick", "Thick"),
    ("dotted", "Dotted"),
    ("dash", "Dashed"),
    ("dotDash", "Dot-dash"),
    ("dotDotDash", "Dot-dot-dash"),
    ("wave", "Wave"),
    ("wavyHeavy", "Heavy wave"),
    ("wavyDouble", "Double wave"),
];

/// Paint an underline (or strike) sample of `style` from `x0` to `x1` at height `y`.
fn paint_line_style(p: &egui::Painter, x0: f32, x1: f32, y: f32, style: &str, color: Color32) {
    let st = |w: f32| Stroke::new(w, color);
    match style {
        "double" => {
            p.hline(x0..=x1, y - 1.5, st(1.0));
            p.hline(x0..=x1, y + 1.5, st(1.0));
        }
        "thick" => {
            p.hline(x0..=x1, y, st(2.5));
        }
        "dotted" | "dash" | "dotDash" | "dotDotDash" => {
            let pattern: &[f32] = match style {
                "dotted" => &[1.5, 2.0],
                "dash" => &[5.0, 3.0],
                "dotDash" => &[5.0, 2.5, 1.5, 2.5],
                _ => &[5.0, 2.0, 1.5, 2.0, 1.5, 2.0],
            };
            let (mut x, mut i) = (x0, 0);
            while x < x1 {
                if i % 2 == 0 {
                    p.hline(x..=(x + pattern[i % pattern.len()]).min(x1), y, st(1.2));
                }
                x += pattern[i % pattern.len()];
                i += 1;
            }
        }
        "wave" | "wavyHeavy" | "wavyDouble" => {
            let rows: &[f32] = if style == "wavyDouble" { &[-2.0, 2.0] } else { &[0.0] };
            for dy in rows {
                let pts: Vec<egui::Pos2> = (0..=((x1 - x0) as usize)).map(|i| pos2(x0 + i as f32, y + dy + 1.5 * (i as f32 * 0.9).sin())).collect();
                p.add(egui::Shape::line(pts, st(if style == "wavyHeavy" { 2.0 } else { 1.0 })));
            }
        }
        "words" => {
            for (a, b) in [(0.0, 0.3), (0.4, 0.65), (0.75, 1.0)] {
                p.hline((x0 + (x1 - x0) * a)..=(x0 + (x1 - x0) * b), y, st(1.0));
            }
        }
        _ => {
            p.hline(x0..=x1, y, st(1.0));
        }
    }
}

fn underline_menu(ui: &mut Ui, current: Option<&str>, cmds: &mut Vec<Cmd>) {
    for (val, label) in UNDERLINES {
        let (rect, r) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
        let sel = current == Some(val);
        if sel || r.hovered() {
            ui.painter().rect_filled(rect, 3.0, if sel { pal().checked } else { pal().hover });
        }
        let p = ui.painter();
        paint_line_style(p, rect.left() + 10.0, rect.left() + 80.0, rect.center().y + 3.0, val, pal().text);
        p.text(pos2(rect.left() + 94.0, rect.center().y), Align2::LEFT_CENTER, label, theme::ui_font(13.0), pal().text);
        if r.clicked() {
            cmds.push(Cmd::UnderlineStyle(val));
            ui.close();
        }
    }
    ui.separator();
    ui.label(egui::RichText::new("Underline Color").small().color(pal().text_dim));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for hex in ["000000", "0E2841", "156082", "E97132", "196B24"].iter().chain(STD_COLORS.iter()) {
            if swatch(ui, hex) {
                cmds.push(Cmd::UnderlineColor(Some(hex.to_string())));
                ui.close();
            }
        }
    });
    if menu_item(ui, "Automatic Color") {
        cmds.push(Cmd::UnderlineColor(None));
    }
    if menu_item(ui, "No Underline") {
        cmds.push(Cmd::UnderlineStyle("none"));
    }
}

fn hex32(hex: &str) -> Color32 {
    let c = rusty_document_writer::props::Color::from_hex(hex).unwrap_or(rusty_document_writer::props::Color::BLACK);
    Color32::from_rgb(c.0, c.1, c.2)
}

/// A gallery tile previewing a text theme with the egui painter (an approximation of the page rendering).
fn theme_tile(ui: &mut Ui, t: &rusty_document_writer::texttheme::TextTheme) -> bool {
    let (rect, r) = ui.allocate_exact_size(vec2(96.0, 58.0), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, Color32::WHITE);
    if r.hovered() {
        p.rect_stroke(rect, 3.0, Stroke::new(2.0, pal().accent), StrokeKind::Outside);
    }
    let text = if t.caps { "ABC" } else if t.small_caps { "Aʙᴄ" } else { "Abc" };
    let font = if t.bold { theme::bold_font(22.0) } else { theme::ui_font(22.0) };
    let col = hex32(t.color);
    let c = pos2(rect.center().x, rect.top() + 22.0);
    let size = p.layout_no_wrap(text.to_string(), font.clone(), col).size();
    let tr = Rect::from_center_size(c, size);
    if let Some(h) = t.highlight.and_then(rusty_document_writer::props::highlight_color) {
        p.rect_filled(tr.expand2(vec2(5.0, 0.0)), 0.0, Color32::from_rgb(h.0, h.1, h.2));
    }
    let bg = t.highlight.and_then(rusty_document_writer::props::highlight_color).map(|h| Color32::from_rgb(h.0, h.1, h.2)).unwrap_or(Color32::WHITE);
    let draw = |off: egui::Vec2, color: Color32| {
        p.text(c + off, Align2::CENTER_CENTER, text, font.clone(), color);
    };
    if t.emboss || t.imprint {
        let s = if t.emboss { 1.2 } else { -1.2 };
        draw(vec2(s, s), Color32::from_gray(0x50));
        draw(vec2(-s, -s), Color32::WHITE);
        draw(vec2(0.0, 0.0), col.lerp_to_gamma(Color32::WHITE, 0.72));
    } else {
        if t.shadow {
            draw(vec2(1.6, 1.6), Color32::from_gray(0xA0));
        }
        if t.outline {
            for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0), (-0.7, -0.7), (0.7, 0.7), (-0.7, 0.7), (0.7, -0.7)] {
                draw(vec2(dx, dy), col);
            }
            draw(vec2(0.0, 0.0), bg);
        } else {
            draw(vec2(0.0, 0.0), col);
        }
    }
    if let Some(u) = t.underline {
        paint_line_style(p, tr.left(), tr.right(), tr.bottom() - 1.0, u, t.underline_color.map(hex32).unwrap_or(col));
    }
    if t.strike || t.dstrike {
        paint_line_style(p, tr.left(), tr.right(), tr.center().y + 1.0, if t.dstrike { "double" } else { "single" }, col);
    }
    p.text(pos2(rect.center().x, rect.bottom() - 9.0), Align2::CENTER_CENTER, t.name, theme::ui_font(11.0), Color32::from_gray(0x40));
    r.on_hover_text(t.name).clicked()
}

fn text_effects_menu(ui: &mut Ui, tb: &super::Toolbar, cmds: &mut Vec<Cmd>) {
    use rusty_document_writer::texttheme::THEMES;
    ui.set_min_width(310.0);
    ui.label(egui::RichText::new("Text Themes").small().color(pal().text_dim));
    egui::Grid::new("text-themes").spacing([4.0, 4.0]).show(ui, |ui| {
        for (i, t) in THEMES.iter().enumerate() {
            if theme_tile(ui, t) {
                cmds.push(Cmd::TextTheme(i));
                ui.close();
            }
            if i % 3 == 2 {
                ui.end_row();
            }
        }
    });
    ui.separator();
    ui.label(egui::RichText::new("Effects").small().color(pal().text_dim));
    for (name, label, on) in [
        ("w:outline", "Outline", tb.outline),
        ("w:shadow", "Shadow", tb.shadow),
        ("w:emboss", "Emboss", tb.emboss),
        ("w:imprint", "Engrave", tb.imprint),
        ("w:smallCaps", "Small Caps    Ctrl+Shift+K", tb.small_caps),
        ("w:caps", "All Caps    Ctrl+Shift+A", tb.caps),
        ("w:dstrike", "Double Strikethrough", tb.dstrike),
    ] {
        if ui.add(egui::Button::selectable(on, format!("{}  {label}", if on { "✔" } else { "   " })).frame(false).min_size(vec2(ui.available_width(), 24.0))).clicked() {
            cmds.push(Cmd::Effect(name));
        }
    }
    ui.separator();
    if menu_item(ui, "Remove Text Effects") {
        cmds.push(Cmd::ClearEffects);
    }
}

// ---------------------------------------------------------------------------
// 3D shape library

const SHAPE_COLORS: [&str; 12] = ["156082", "0F9ED5", "196B24", "4EA72E", "E97132", "FFC000", "C00000", "A02B93", "7030A0", "0E2841", "404040", "FFFFFF"];

/// (Re)build the gallery thumbnails when the library is first shown or the shape colour changes.
fn ensure_shape_thumbs(app: &mut App, ctx: &egui::Context) {
    use rusty_document_writer::shapes::{tint_rgba, SHAPES};
    if app.shape_thumbs.as_ref().is_some_and(|(c, _)| *c == app.shape_color) {
        return;
    }
    let color = app.shape_color.as_deref().and_then(rusty_document_writer::props::Color::from_hex);
    let thumbs = SHAPES
        .iter()
        .filter_map(|s| {
            let img = image::load_from_memory(s.png).ok()?.thumbnail(112, 112);
            let mut rgba = img.to_rgba8();
            if let Some(c) = color {
                tint_rgba(&mut rgba, c);
            }
            let ci = egui::ColorImage::from_rgba_unmultiplied([rgba.width() as usize, rgba.height() as usize], &rgba);
            Some(ctx.load_texture(format!("shape-{}", s.id), ci, egui::TextureOptions::LINEAR))
        })
        .collect();
    app.shape_thumbs = Some((app.shape_color.clone(), thumbs));
}

fn shapes_menu(ui: &mut Ui, app: &App, cmds: &mut Vec<Cmd>) {
    use rusty_document_writer::shapes::SHAPES;
    ui.set_min_width(400.0);
    ui.label(egui::RichText::new("3D Shapes").small().color(pal().text_dim));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        // "Silver": the neutral render, untinted.
        let silver = color_chip(ui, Color32::from_gray(0xA6), app.shape_color.is_none()).on_hover_text("Silver");
        if silver.clicked() {
            cmds.push(Cmd::ShapeColor(None));
        }
        for hex in SHAPE_COLORS {
            if color_chip(ui, hex32(hex), app.shape_color.as_deref() == Some(hex)).clicked() {
                cmds.push(Cmd::ShapeColor(Some(hex.to_string())));
            }
        }
    });
    ui.add_space(4.0);
    let Some((_, thumbs)) = &app.shape_thumbs else { return };
    egui::Grid::new("shape-gallery").spacing([4.0, 4.0]).show(ui, |ui| {
        for (i, (shape, tex)) in SHAPES.iter().zip(thumbs).enumerate() {
            let (rect, r) = ui.allocate_exact_size(vec2(62.0, 62.0), Sense::click());
            let p = ui.painter();
            p.rect_filled(rect, 4.0, if r.hovered() { Color32::from_gray(0xF4) } else { Color32::WHITE });
            if r.hovered() {
                p.rect_stroke(rect, 4.0, Stroke::new(2.0, pal().accent), StrokeKind::Outside);
            }
            let sz = tex.size_vec2();
            let k = 54.0 / sz.x.max(sz.y);
            p.image(tex.id(), Rect::from_center_size(rect.center(), sz * k), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            if r.on_hover_text(shape.name).clicked() {
                cmds.push(Cmd::InsertShape(i));
                ui.close();
            }
            if i % 6 == 5 {
                ui.end_row();
            }
        }
    });
    ui.add_space(2.0);
    ui.label(egui::RichText::new("Shapes are inserted as pictures, so they look the same in any word processor.").small().color(pal().text_dim));
}

fn color_chip(ui: &mut Ui, c: Color32, selected: bool) -> Response {
    let (rect, r) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::click());
    let p = ui.painter();
    p.circle_filled(rect.center(), 8.0, c);
    p.circle_stroke(rect.center(), 8.0, Stroke::new(0.8, Color32::from_gray(0x70)));
    if selected || r.hovered() {
        p.circle_stroke(rect.center(), 10.0, Stroke::new(1.6, if selected { pal().text } else { pal().accent }));
    }
    r
}

fn swatch(ui: &mut Ui, hex: &str) -> bool {
    let c = rusty_document_writer::props::Color::from_hex(hex).unwrap_or(rusty_document_writer::props::Color::BLACK);
    let (rect, r) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
    ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(c.0, c.1, c.2));
    ui.painter().rect_stroke(rect, 0.0, Stroke::new(if r.hovered() { 2.0 } else { 0.5 }, if r.hovered() { pal().accent_text } else { Color32::from_gray(0x60) }), StrokeKind::Inside);
    r.on_hover_text(format!("#{hex}")).clicked()
}

fn home(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    group(ui, "Clipboard", |ui| {
        if big(ui, Icon::Paste, "Paste", true, true).clicked() {
            cmds.push(Cmd::Paste);
        }
        column(ui, |ui| {
            if icon_btn(ui, Icon::Cut, "Cut (Ctrl+X)", false, false).clicked() {
                cmds.push(Cmd::Cut);
            }
            if icon_btn(ui, Icon::Copy, "Copy (Ctrl+C)", false, false).clicked() {
                cmds.push(Cmd::Copy);
            }
            if icon_btn(ui, Icon::FormatPainter, "Format Painter", false, false).clicked() {
                cmds.push(Cmd::Unsupported("Format Painter"));
            }
        });
    });
    group(ui, "Font", |ui| {
        column(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                let font_label = app.tb.font.clone();
                egui::ComboBox::from_id_salt("font-family").width(150.0).height(400.0).selected_text(font_label).show_ui(ui, |ui| {
                    let theme = &app.doc.theme;
                    for (name, tag) in [(theme.major_latin.clone(), " (Headings)"), (theme.minor_latin.clone(), " (Body)")] {
                        if !name.is_empty() && ui.selectable_label(false, format!("{name}{tag}")).clicked() {
                            cmds.push(Cmd::FontFamily(name));
                        }
                    }
                    ui.separator();
                    for f in &app.font_names {
                        if ui.selectable_label(false, f).clicked() {
                            cmds.push(Cmd::FontFamily(f.clone()));
                        }
                    }
                });
                let size_text = if app.tb.size.fract() == 0.0 { format!("{}", app.tb.size as i32) } else { format!("{:.1}", app.tb.size) };
                egui::ComboBox::from_id_salt("font-size").width(46.0).height(400.0).selected_text(size_text).show_ui(ui, |ui| {
                    for s in [8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 48.0, 72.0] {
                        if ui.selectable_label((app.tb.size - s).abs() < 0.01, format!("{s}")).clicked() {
                            cmds.push(Cmd::FontSize(s));
                        }
                    }
                });
                if icon_btn(ui, Icon::GrowFont, "Increase Font Size (Ctrl+])", false, false).clicked() {
                    cmds.push(Cmd::GrowFont);
                }
                if icon_btn(ui, Icon::ShrinkFont, "Decrease Font Size (Ctrl+[)", false, false).clicked() {
                    cmds.push(Cmd::ShrinkFont);
                }
                let r = icon_btn(ui, Icon::ChangeCase, "Change Case", false, true);
                popup(&r, |ui| {
                    for (i, t) in ["UPPERCASE", "lowercase", "Capitalize Each Word"].iter().enumerate() {
                        if menu_item(ui, t) {
                            cmds.push(Cmd::ChangeCase(i as u8));
                        }
                    }
                });
                if icon_btn(ui, Icon::ClearFormat, "Clear All Formatting", false, false).clicked() {
                    cmds.push(Cmd::ClearFormat);
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 1.0;
                if icon_btn(ui, Icon::Bold, "Bold (Ctrl+B)", app.tb.bold, false).clicked() {
                    cmds.push(Cmd::Toggle("w:b"));
                }
                if icon_btn(ui, Icon::Italic, "Italic (Ctrl+I)", app.tb.italic, false).clicked() {
                    cmds.push(Cmd::Toggle("w:i"));
                }
                // Split button: the icon toggles the underline, the chevron opens the style menu.
                let r = icon_btn(ui, Icon::Underline, "Underline (Ctrl+U)", app.tb.underline, true);
                let on_chevron = r.interact_pointer_pos().or(r.hover_pos()).is_some_and(|p| p.x > r.rect.right() - 12.0);
                if r.clicked() && !on_chevron {
                    cmds.push(Cmd::Underline);
                }
                egui::Popup::from_response(&r).open_memory((r.clicked() && on_chevron).then_some(egui::SetOpenCommand::Toggle)).show(|ui| {
                    ui.set_min_width(200.0);
                    underline_menu(ui, app.tb.underline_style.as_deref(), cmds);
                });
                if icon_btn(ui, Icon::Strike, "Strikethrough", app.tb.strike, false).clicked() {
                    cmds.push(Cmd::Toggle("w:strike"));
                }
                if icon_btn(ui, Icon::Subscript, "Subscript (Ctrl+=)", app.tb.sub, false).clicked() {
                    cmds.push(Cmd::VertAlign("subscript"));
                }
                if icon_btn(ui, Icon::Superscript, "Superscript (Ctrl+Shift++)", app.tb.sup, false).clicked() {
                    cmds.push(Cmd::VertAlign("superscript"));
                }
                ui.add_space(4.0);
                let tb = &app.tb;
                let any_fx = tb.outline || tb.shadow || tb.emboss || tb.imprint;
                let r = icon_btn(ui, Icon::TextEffects, "Text Effects and Themes", any_fx, true);
                popup(&r, |ui| text_effects_menu(ui, tb, cmds));
                let r = icon_btn(ui, Icon::Highlight, "Text Highlight Color", false, true);
                popup(&r, |ui| {
                    let hl = ["yellow", "green", "cyan", "magenta", "blue", "red", "darkBlue", "darkCyan", "darkGreen", "darkMagenta", "darkRed", "darkYellow", "darkGray", "lightGray", "black"];
                    egui::Grid::new("hlgrid").spacing([3.0, 3.0]).show(ui, |ui| {
                        for (i, n) in hl.iter().enumerate() {
                            let c = rusty_document_writer::props::highlight_color(n).unwrap();
                            if swatch(ui, &c.to_hex()) {
                                cmds.push(Cmd::Highlight(Some(n.to_string())));
                            }
                            if i % 5 == 4 {
                                ui.end_row();
                            }
                        }
                    });
                    if menu_item(ui, "No Color") {
                        cmds.push(Cmd::Highlight(None));
                    }
                });
                let r = icon_btn(ui, Icon::FontColor, "Font Color", false, true);
                popup(&r, |ui| {
                    if let Some(c) = color_grid(ui, "Automatic") {
                        cmds.push(Cmd::FontColor(c));
                    }
                });
            });
        });
    });
    group(ui, "Paragraph", |ui| {
        column(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 1.0;
                if icon_btn(ui, Icon::Bullets, "Bullets", app.tb.list == Some(true), true).clicked() {
                    cmds.push(Cmd::Bullets);
                }
                if icon_btn(ui, Icon::Numbering, "Numbering", app.tb.list == Some(false), true).clicked() {
                    cmds.push(Cmd::Numbering);
                }
                if icon_btn(ui, Icon::Multilevel, "Multilevel List", false, true).clicked() {
                    cmds.push(Cmd::Numbering);
                }
                ui.add_space(4.0);
                if icon_btn(ui, Icon::IndentDec, "Decrease Indent", false, false).clicked() {
                    cmds.push(Cmd::IndentDec);
                }
                if icon_btn(ui, Icon::IndentInc, "Increase Indent", false, false).clicked() {
                    cmds.push(Cmd::IndentInc);
                }
                ui.add_space(4.0);
                if icon_btn(ui, Icon::Sort, "Sort", false, false).clicked() {
                    cmds.push(Cmd::Unsupported("Sort"));
                }
                if icon_btn(ui, Icon::Pilcrow, "Show/Hide ¶ (Ctrl+*)", app.show_marks, false).clicked() {
                    cmds.push(Cmd::ShowMarks);
                }
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 1.0;
                let a = app.tb.align;
                if icon_btn(ui, Icon::AlignLeft, "Align Left (Ctrl+L)", a == Some(Align::Left), false).clicked() {
                    cmds.push(Cmd::Align("left"));
                }
                if icon_btn(ui, Icon::AlignCenter, "Center (Ctrl+E)", a == Some(Align::Center), false).clicked() {
                    cmds.push(Cmd::Align("center"));
                }
                if icon_btn(ui, Icon::AlignRight, "Align Right (Ctrl+R)", a == Some(Align::Right), false).clicked() {
                    cmds.push(Cmd::Align("right"));
                }
                if icon_btn(ui, Icon::Justify, "Justify (Ctrl+J)", a == Some(Align::Justify), false).clicked() {
                    cmds.push(Cmd::Align("both"));
                }
                ui.add_space(4.0);
                let r = icon_btn(ui, Icon::LineSpacing, "Line and Paragraph Spacing", false, true);
                popup(&r, |ui| {
                    for m in [1.0f32, 1.08, 1.15, 1.5, 2.0, 2.5, 3.0] {
                        let checked = (app.tb.line - m).abs() < 0.02;
                        if ui.selectable_label(checked, format!("{m}")).clicked() {
                            cmds.push(Cmd::LineSpacing(m));
                        }
                    }
                    ui.separator();
                    if menu_item(ui, "Remove Space After Paragraph") {
                        cmds.push(Cmd::SpaceAfter(0.0));
                    }
                    if menu_item(ui, "Add Space Before Paragraph") {
                        cmds.push(Cmd::SpaceBefore(12.0));
                    }
                });
                let r = icon_btn(ui, Icon::Shading, "Shading", false, true);
                popup(&r, |ui| {
                    if let Some(c) = color_grid(ui, "No Color") {
                        cmds.push(Cmd::ParaShading(c));
                    }
                });
                let r = icon_btn(ui, Icon::Borders, "Borders", false, true);
                popup(&r, |ui| {
                    if menu_item(ui, "Outside Borders") {
                        cmds.push(Cmd::ParaBorder(true));
                    }
                    if menu_item(ui, "No Border") {
                        cmds.push(Cmd::ParaBorder(false));
                    }
                });
            });
        });
    });
    group(ui, "Styles", |ui| {
        let styles: Vec<(&str, &str)> = vec![("Normal", "Normal"), ("NoSpacing", "No Spacing"), ("Heading1", "Heading 1"), ("Heading2", "Heading 2"), ("Title", "Title"), ("Subtitle", "Subtitle"), ("Quote", "Quote")];
        let avail = (ui.available_width() - 220.0).max(140.0);
        let tile_w = 92.0;
        let n = ((avail / (tile_w + 3.0)) as usize).clamp(2, styles.len());
        ui.vertical(|ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 3.0;
                for (id, name) in styles.iter().take(n) {
                    let exists = app.doc.styles.get(id).is_some() || *id == "Normal";
                    let (rect, r) = ui.allocate_exact_size(vec2(tile_w, 54.0), Sense::click());
                    let sel = app.tb.style == *id || (app.tb.style.is_empty() && *id == "Normal");
                    let p = ui.painter();
                    p.rect_filled(rect, 2.0, Color32::WHITE);
                    if sel || r.hovered() {
                        p.rect_stroke(rect, 2.0, Stroke::new(2.0, if sel { Color32::from_gray(0x90) } else { pal().accent }), StrokeKind::Outside);
                    }
                    // Preview using the style's resolved formatting.
                    let rp = app.doc.resolver().run(id, None, None);
                    let col = rp.color.map(|c| Color32::from_rgb(c.0, c.1, c.2)).unwrap_or(Color32::BLACK);
                    let mk = |size: f32| if rp.bold { theme::bold_font(size) } else { theme::ui_font(size) };
                    let mut size = (rp.size * 1.15).clamp(10.0, 22.0);
                    // Shrink large previews (headings, title) so the whole name fits the tile.
                    let tw = p.layout_no_wrap(name.to_string(), mk(size), col).size().x;
                    let room = tile_w - 14.0;
                    if tw > room {
                        size = (size * room / tw).max(9.0);
                    }
                    let font = mk(size);
                    let clip = rect.shrink(4.0);
                    p.with_clip_rect(clip).text(pos2(rect.left() + 8.0, rect.center().y), Align2::LEFT_CENTER, *name, font, col);
                    let r = r.on_hover_text(*name);
                    if r.clicked() && exists {
                        cmds.push(Cmd::Style(id.to_string()));
                    }
                }
                let (rect, r) = ui.allocate_exact_size(vec2(16.0, 54.0), Sense::click());
                if r.hovered() {
                    ui.painter().rect_filled(rect, 2.0, pal().hover);
                }
                ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, pal().sep), StrokeKind::Inside);
                chevron(ui, rect.center() + vec2(0.0, 4.0), pal().text);
                ui.painter().hline((rect.center().x - 3.0)..=(rect.center().x + 3.0), rect.center().y - 1.0, Stroke::new(1.0, pal().text));
                popup(&r, |ui| {
                    let mut all: Vec<(String, String)> = app
                        .doc
                        .styles
                        .map
                        .values()
                        .filter(|s| s.kind == rusty_document_writer::props::StyleKind::Paragraph && (s.q_format || !s.hidden))
                        .map(|s| (s.id.clone(), app.doc.styles.display_name(&s.id)))
                        .collect();
                    all.sort_by(|a, b| a.1.cmp(&b.1));
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        for (id, name) in all {
                            if ui.selectable_label(app.tb.style == id, name).clicked() {
                                cmds.push(Cmd::Style(id));
                            }
                        }
                    });
                });
            });
        });
    });
    group(ui, "Editing", |ui| {
        column(ui, |ui| {
            if small(ui, Icon::Find, "Find", true, true).clicked() {
                cmds.push(Cmd::Find);
            }
            if small(ui, Icon::Replace, "Replace", false, true).clicked() {
                cmds.push(Cmd::Find);
            }
            if small(ui, Icon::Select, "Select", true, true).clicked() {
                cmds.push(Cmd::SelectAll);
            }
        });
    });
    group(ui, "Voice", |ui| {
        if big(ui, Icon::ReadAloud, "Dictate", true, false).clicked() {}
    });
}

fn insert(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    group(ui, "Pages", |ui| {
        if big(ui, Icon::CoverPage, "Cover\nPage", true, false).clicked() {}
        if big(ui, Icon::BlankPage, "Blank\nPage", false, true).clicked() {
            cmds.push(Cmd::BlankPage);
        }
        if big(ui, Icon::PageBreak, "Page\nBreak", false, true).clicked() {
            cmds.push(Cmd::PageBreak);
        }
    });
    group(ui, "Tables", |ui| {
        let r = big(ui, Icon::Table, "Table", true, true);
        popup(&r, |ui| {
            let id = Id::NULL.with("tblpick");
            let mut hover: (usize, usize) = ui.data(|d| d.get_temp(id)).unwrap_or((0, 0));
            ui.label(if hover.0 > 0 { format!("{}x{} Table", hover.1, hover.0) } else { "Insert Table".into() });
            let mut new_hover = (0, 0);
            let mut clicked = None;
            for r in 1..=8 {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for c in 1..=10 {
                        let (rect, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
                        let on = r <= hover.0 && c <= hover.1;
                        ui.painter().rect_filled(rect, 1.0, if on { Color32::from_rgb(0xFF, 0xC8, 0x6E) } else { Color32::WHITE });
                        ui.painter().rect_stroke(rect, 1.0, Stroke::new(1.0, Color32::from_gray(0x80)), StrokeKind::Inside);
                        if resp.hovered() {
                            new_hover = (r, c);
                        }
                        if resp.clicked() {
                            clicked = Some((r, c));
                        }
                    }
                });
            }
            if new_hover.0 > 0 {
                hover = new_hover;
            }
            ui.data_mut(|d| d.insert_temp(id, hover));
            if let Some((r, c)) = clicked {
                cmds.push(Cmd::InsertTable(r, c));
                ui.close();
            }
            ui.separator();
            if menu_item(ui, "Insert Table… (3 x 3)") {
                cmds.push(Cmd::InsertTable(3, 3));
            }
        });
    });
    group(ui, "Illustrations", |ui| {
        if big(ui, Icon::Pictures, "Pictures", true, true).clicked() {
            cmds.push(Cmd::InsertPicture);
        }
        let r = big(ui, Icon::Shapes, "Shapes", true, true);
        if Popup::is_id_open(ui.ctx(), Popup::default_response_id(&r)) || r.clicked() {
            ensure_shape_thumbs(app, ui.ctx());
        }
        // Stays open while picking a colour; choosing a shape closes it.
        Popup::from_toggle_button_response(&r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| shapes_menu(ui, app, cmds));
        for (ic, l) in [(Icon::Icons3D, "Icons"), (Icon::SmartArt, "SmartArt"), (Icon::Chart, "Chart"), (Icon::Screenshot, "Screenshot")] {
            if big(ui, ic, l, matches!(ic, Icon::Shapes | Icon::Screenshot), false).clicked() {}
        }
    });
    group(ui, "Links", |ui| {
        column(ui, |ui| {
            for (ic, l) in [(Icon::Link, "Link"), (Icon::Bookmark, "Bookmark"), (Icon::CrossRef, "Cross-reference")] {
                if small(ui, ic, l, false, true).clicked() {
                    cmds.push(Cmd::Unsupported(l_static(l)));
                }
            }
        });
    });
    group(ui, "Comments", |ui| {
        if big(ui, Icon::Comment, "Comment", false, true).clicked() {
            cmds.push(Cmd::Unsupported("Comments"));
        }
    });
    group(ui, "Header & Footer", |ui| {
        for (ic, l) in [(Icon::Header, "Header"), (Icon::Footer, "Footer")] {
            if big(ui, ic, l, true, true).clicked() {
                cmds.push(Cmd::Unsupported("Header & Footer gallery (double-click a header to edit it)"));
            }
        }
        let r = big(ui, Icon::PageNumber, "Page\nNumber", true, true);
        page_number_menu(app, &r, cmds);
    });
    group(ui, "Text", |ui| {
        if big(ui, Icon::TextBox, "Text\nBox", true, false).clicked() {}
        column(ui, |ui| {
            small(ui, Icon::QuickParts, "Quick Parts", true, false);
            let r = small(ui, Icon::WordArt, "WordArt", true, true);
            super::wordart_ui::menu(app, ui, &r, cmds);
            small(ui, Icon::DropCap, "Drop Cap", true, false);
        });
        column(ui, |ui| {
            small(ui, Icon::Signature, "Signature Line", true, false);
            if small(ui, Icon::DateTime, "Date & Time", false, true).clicked() {
                cmds.push(Cmd::InsertDateTime);
            }
            small(ui, Icon::Object, "Object", true, false);
        });
    });
    group(ui, "Symbols", |ui| {
        if big(ui, Icon::Equation, "Equation", true, false).clicked() {}
        let r = big(ui, Icon::Symbol, "Symbol", true, true);
        popup(&r, |ui| {
            let syms = "€£¥©®™±≠≤≥÷×∞µαβγδπΩ∑√∫°§¶†‡•…–—«»“”‘’←→↑↓⇒✓✗☐☑★☆♥";
            egui::Grid::new("symgrid").spacing([2.0, 2.0]).show(ui, |ui| {
                for (i, c) in syms.chars().enumerate() {
                    if ui.add(egui::Button::new(egui::RichText::new(c.to_string()).size(16.0)).min_size(vec2(26.0, 26.0))).clicked() {
                        cmds.push(Cmd::InsertSymbol(c));
                        ui.close();
                    }
                    if i % 10 == 9 {
                        ui.end_row();
                    }
                }
            });
        });
    });
}

fn l_static(s: &str) -> &'static str {
    match s {
        "Link" => "Link",
        "Bookmark" => "Bookmark",
        "Cross-reference" => "Cross-reference",
        _ => "This feature",
    }
}

fn design(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    group(ui, "Document Formatting", |ui| {
        if big(ui, Icon::Themes, "Themes", true, false).clicked() {}
        // Style set preview tiles.
        ui.vertical(|ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                let n = (((ui.available_width() - 420.0) / 78.0) as i32).clamp(2, 9);
                for i in 0..n {
                    let (rect, r) = ui.allocate_exact_size(vec2(74.0, 58.0), Sense::click());
                    let p = ui.painter();
                    p.rect_filled(rect, 1.0, Color32::WHITE);
                    if r.hovered() || i == 0 {
                        p.rect_stroke(rect, 1.0, Stroke::new(2.0, if i == 0 { Color32::from_gray(0x90) } else { pal().accent }), StrokeKind::Outside);
                    }
                    let col = [Color32::BLACK, Color32::from_rgb(0x0F, 0x47, 0x61), Color32::from_rgb(0x15, 0x60, 0x82)][i as usize % 3];
                    p.text(rect.left_top() + vec2(6.0, 12.0), Align2::LEFT_CENTER, "Title", theme::ui_font(13.0), Color32::BLACK);
                    p.text(rect.left_top() + vec2(6.0, 28.0), Align2::LEFT_CENTER, "Heading 1", theme::ui_font(9.5), col);
                    for k in 0..3 {
                        p.hline((rect.left() + 6.0)..=(rect.right() - 6.0), rect.top() + 40.0 + k as f32 * 5.0, Stroke::new(1.0, Color32::from_gray(0xA0)));
                    }
                }
            });
        });
        column(ui, |ui| {
            ui.add_space(4.0);
            big(ui, Icon::Colors, "Colors", true, false);
        });
        big(ui, Icon::Fonts, "Fonts", true, false);
        column(ui, |ui| {
            small(ui, Icon::ParagraphSpacing, "Paragraph Spacing", true, false);
            small(ui, Icon::Effects, "Effects", true, false);
            small(ui, Icon::SetDefault, "Set as Default", false, false);
        });
    });
    group(ui, "Page Background", |ui| {
        big(ui, Icon::Watermark, "Watermark", true, false);
        let r = big(ui, Icon::PageColor, "Page\nColor", true, true);
        popup(&r, |ui| {
            if let Some(c) = color_grid(ui, "No Color") {
                cmds.push(Cmd::PageColor(c));
            }
        });
        big(ui, Icon::PageBorders, "Page\nBorders", false, false);
    });
    let _ = app;
}

fn inch_spinner(ui: &mut Ui, label: &str, icon: Icon, value_pt: f32, on_change: impl FnOnce(f32)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        icons::paint(ui.painter(), icon, Rect::from_min_size(ui.cursor().min + vec2(0.0, 4.0), vec2(16.0, 16.0)), pal().icon);
        ui.add_space(18.0);
        ui.add_sized(vec2(56.0, 22.0), egui::Label::new(egui::RichText::new(label).size(12.5)));
        let mut v = (value_pt / 72.0 * 100.0).round() / 100.0;
        let r = ui.add_sized(vec2(70.0, 22.0), egui::DragValue::new(&mut v).speed(0.01).range(-10.0..=22.0).suffix("\"").fixed_decimals(2));
        if r.changed() {
            on_change(v * 72.0);
        }
    });
}

fn pt_spinner(ui: &mut Ui, label: &str, icon: Icon, value_pt: f32, on_change: impl FnOnce(f32)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        icons::paint(ui.painter(), icon, Rect::from_min_size(ui.cursor().min + vec2(0.0, 4.0), vec2(16.0, 16.0)), pal().icon);
        ui.add_space(18.0);
        ui.add_sized(vec2(44.0, 22.0), egui::Label::new(egui::RichText::new(label).size(12.5)));
        let mut v = value_pt;
        let r = ui.add_sized(vec2(70.0, 22.0), egui::DragValue::new(&mut v).speed(1.0).range(0.0..=1584.0).suffix(" pt").fixed_decimals(0));
        if r.changed() {
            on_change(v);
        }
    });
}

fn layout_tab(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    group(ui, "Page Setup", |ui| {
        let r = big(ui, Icon::Margins, "Margins", true, true);
        popup(&r, |ui| {
            for (name, t, rr, b, l) in [("Normal  (1\" all)", 72.0, 72.0, 72.0, 72.0), ("Narrow  (0.5\" all)", 36.0, 36.0, 36.0, 36.0), ("Moderate  (1\" / 0.75\")", 72.0, 54.0, 72.0, 54.0), ("Wide  (1\" / 2\")", 72.0, 144.0, 72.0, 144.0)] {
                if menu_item(ui, name) {
                    cmds.push(Cmd::Margins(t, rr, b, l));
                }
            }
        });
        let r = big(ui, Icon::Orientation, "Orientation", true, true);
        popup(&r, |ui| {
            if menu_item(ui, "Portrait") {
                cmds.push(Cmd::Orientation(false));
            }
            if menu_item(ui, "Landscape") {
                cmds.push(Cmd::Orientation(true));
            }
        });
        let r = big(ui, Icon::Size, "Size", true, true);
        popup(&r, |ui| {
            for (name, w, h) in [("Letter  8.5\" x 11\"", 612.0, 792.0), ("Legal  8.5\" x 14\"", 612.0, 1008.0), ("Executive  7.25\" x 10.5\"", 522.0, 756.0), ("A4  21 cm x 29.7 cm", 595.3, 841.9), ("A5  14.8 cm x 21 cm", 419.6, 595.3), ("B5 (JIS)  18.2 cm x 25.7 cm", 515.9, 728.5)] {
                if menu_item(ui, name) {
                    cmds.push(Cmd::PageSize(w, h));
                }
            }
        });
        let r = big(ui, Icon::Columns, "Columns", true, true);
        popup(&r, |ui| {
            for (name, n) in [("One", 1), ("Two", 2), ("Three", 3)] {
                if menu_item(ui, name) {
                    cmds.push(Cmd::Columns(n));
                }
            }
        });
        column(ui, |ui| {
            let r = small(ui, Icon::Breaks, "Breaks", true, true);
            popup(&r, |ui| {
                if menu_item(ui, "Page") {
                    cmds.push(Cmd::PageBreak);
                }
                if menu_item(ui, "Column") {
                    cmds.push(Cmd::ColumnBreak);
                }
                ui.separator();
                if menu_item(ui, "Section Break: Next Page") {
                    cmds.push(Cmd::SectionBreak("nextPage"));
                }
                if menu_item(ui, "Section Break: Continuous") {
                    cmds.push(Cmd::SectionBreak("continuous"));
                }
            });
            small(ui, Icon::LineNumbers, "Line Numbers", true, false);
            small(ui, Icon::Hyphenation, "Hyphenation", true, false);
        });
    });
    group(ui, "Paragraph", |ui| {
        column(ui, |ui| {
            ui.label(egui::RichText::new("Indent").size(12.5));
            let l = app.tb.ind_left;
            let rr = app.tb.ind_right;
            inch_spinner(ui, "Left:", Icon::IndentInc, l, |v| cmds.push(Cmd::IndentLeft(v)));
            inch_spinner(ui, "Right:", Icon::IndentDec, rr, |v| cmds.push(Cmd::IndentRight(v)));
        });
        ui.add_space(8.0);
        column(ui, |ui| {
            ui.label(egui::RichText::new("Spacing").size(12.5));
            let b = app.tb.before;
            let a = app.tb.after;
            pt_spinner(ui, "Before:", Icon::LineSpacing, b, |v| cmds.push(Cmd::SpaceBefore(v)));
            pt_spinner(ui, "After:", Icon::LineSpacing, a, |v| cmds.push(Cmd::SpaceAfter(v)));
        });
    });
    group(ui, "Arrange", |ui| {
        for (ic, l) in [(Icon::Position, "Position"), (Icon::WrapText, "Wrap\nText"), (Icon::BringForward, "Bring\nForward"), (Icon::SendBackward, "Send\nBackward")] {
            big(ui, ic, l, true, false);
        }
        big(ui, Icon::SelectionPane, "Selection\nPane", false, false);
        column(ui, |ui| {
            small(ui, Icon::Align, "Align", true, false);
            small(ui, Icon::Group, "Group", true, false);
            small(ui, Icon::Rotate, "Rotate", true, false);
        });
    });
}

fn references(ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    group(ui, "Table of Contents", |ui| {
        if big(ui, Icon::Toc, "Table of\nContents", true, true).clicked() {
            cmds.push(Cmd::Unsupported("Table of Contents generation"));
        }
        column(ui, |ui| {
            small(ui, Icon::AddText, "Add Text", true, false);
            small(ui, Icon::Update, "Update Table", false, false);
        });
    });
    group(ui, "Footnotes", |ui| {
        if big(ui, Icon::Footnote, "Insert\nFootnote", false, true).clicked() {
            cmds.push(Cmd::Unsupported("Insert Footnote"));
        }
        column(ui, |ui| {
            small(ui, Icon::Endnote, "Insert Endnote", false, false);
            small(ui, Icon::NextFootnote, "Next Footnote", true, false);
            small(ui, Icon::ShowNotes, "Show Notes", false, false);
        });
    });
    group(ui, "Citations & Bibliography", |ui| {
        big(ui, Icon::Citation, "Insert\nCitation", true, false);
        column(ui, |ui| {
            small(ui, Icon::ManageSources, "Manage Sources", false, false);
            small(ui, Icon::Generic, "Style: APA", true, false);
            small(ui, Icon::Bibliography, "Bibliography", true, false);
        });
    });
    group(ui, "Captions", |ui| {
        big(ui, Icon::Caption, "Insert\nCaption", false, false);
        column(ui, |ui| {
            small(ui, Icon::TableOfFigures, "Insert Table of Figures", false, false);
            small(ui, Icon::Update, "Update Table", false, false);
            small(ui, Icon::CrossRef, "Cross-reference", false, false);
        });
    });
    group(ui, "Index", |ui| {
        big(ui, Icon::MarkEntry, "Mark\nEntry", false, false);
        column(ui, |ui| {
            small(ui, Icon::Index, "Insert Index", false, false);
            small(ui, Icon::Update, "Update Index", false, false);
        });
    });
    group(ui, "Table of Authorities", |ui| {
        big(ui, Icon::MarkCitation, "Mark\nCitation", false, false);
    });
}

fn mailings(ui: &mut Ui, _cmds: &mut Vec<Cmd>) {
    group(ui, "Create", |ui| {
        big(ui, Icon::Envelopes, "Envelopes", false, false);
        big(ui, Icon::Labels, "Labels", false, false);
    });
    group(ui, "Start Mail Merge", |ui| {
        big(ui, Icon::StartMerge, "Start Mail\nMerge", true, false);
        big(ui, Icon::Recipients, "Select\nRecipients", true, false);
        big(ui, Icon::EditRecipients, "Edit\nRecipient List", false, false);
    });
    group(ui, "Write & Insert Fields", |ui| {
        for (ic, l) in [(Icon::Highlight2, "Highlight\nMerge Fields"), (Icon::AddressBlock, "Address\nBlock"), (Icon::Greeting, "Greeting\nLine"), (Icon::MergeField, "Insert Merge\nField")] {
            big(ui, ic, l, false, false);
        }
        column(ui, |ui| {
            small(ui, Icon::Rules, "Rules", true, false);
            small(ui, Icon::MatchFields, "Match Fields", false, false);
            small(ui, Icon::UpdateLabels, "Update Labels", false, false);
        });
    });
    group(ui, "Preview Results", |ui| {
        big(ui, Icon::Preview, "Preview\nResults", false, false);
    });
    group(ui, "Finish", |ui| {
        big(ui, Icon::FinishMerge, "Finish &\nMerge", true, false);
    });
}

fn review(ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    group(ui, "Proofing", |ui| {
        big(ui, Icon::Spelling, "Spelling &\nGrammar", false, false);
        column(ui, |ui| {
            small(ui, Icon::Thesaurus, "Thesaurus", false, false);
            if small(ui, Icon::WordCount, "Word Count", false, true).clicked() {
                cmds.push(Cmd::WordCount);
            }
        });
    });
    group(ui, "Speech", |ui| {
        big(ui, Icon::ReadAloud, "Read\nAloud", false, false);
    });
    group(ui, "Accessibility", |ui| {
        big(ui, Icon::Accessibility, "Check\nAccessibility", true, false);
    });
    group(ui, "Language", |ui| {
        big(ui, Icon::Language, "Language", true, false);
    });
    group(ui, "Comments", |ui| {
        column(ui, |ui| {
            small(ui, Icon::NewComment, "New Comment", false, false);
            small(ui, Icon::DeleteComment, "Delete", true, false);
            small(ui, Icon::Prev, "Previous", false, false);
        });
        column(ui, |ui| {
            small(ui, Icon::Next, "Next", false, false);
            small(ui, Icon::Comment, "Show Comments", false, false);
        });
    });
    group(ui, "Tracking", |ui| {
        big(ui, Icon::TrackChanges, "Track\nChanges", true, false);
        column(ui, |ui| {
            small(ui, Icon::Generic, "Simple Markup", true, false);
            small(ui, Icon::Generic, "Show Markup", true, false);
            small(ui, Icon::Generic, "Reviewing Pane", true, false);
        });
    });
    group(ui, "Changes", |ui| {
        big(ui, Icon::Accept, "Accept", true, false);
        big(ui, Icon::Reject, "Reject", true, false);
    });
    group(ui, "Compare", |ui| {
        big(ui, Icon::Compare, "Compare", true, false);
    });
    group(ui, "Protect", |ui| {
        big(ui, Icon::Protect, "Protect", true, false);
    });
}

fn view(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    group(ui, "Views", |ui| {
        big(ui, Icon::ReadMode, "Read\nMode", false, false);
        let r = big(ui, Icon::PrintLayout, "Print\nLayout", false, true);
        ui.painter().rect_stroke(r.rect, 4.0, Stroke::new(1.0, pal().border()), StrokeKind::Inside);
        big(ui, Icon::WebLayout, "Web\nLayout", false, false);
        column(ui, |ui| {
            small(ui, Icon::Outline, "Outline", false, false);
            small(ui, Icon::Draft, "Draft", false, false);
        });
    });
    group(ui, "Dark Mode", |ui| {
        if big(ui, Icon::DarkMode, "Switch\nModes", false, true).on_hover_text("Show the page in dark colours").clicked() {
            cmds.push(Cmd::DarkPage);
        }
        let r = big(ui, Icon::Themes, "App\nTheme", true, true).on_hover_text("Colours for the ribbon and menus (also under File ▸ Theme)");
        egui::Popup::from_toggle_button_response(&r).show(|ui| {
            ui.set_min_width(220.0);
            egui::ScrollArea::vertical().max_height(480.0).show(ui, |ui| {
                for source in ["Word", "VS Code", "Zed"] {
                    ui.label(egui::RichText::new(source).small().color(pal().text_dim));
                    for (i, p) in theme::PALETTES.iter().enumerate().filter(|(_, p)| p.source == source) {
                        let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width().max(200.0), 24.0), Sense::click());
                        let painter = ui.painter();
                        if resp.hovered() {
                            painter.rect_filled(rect, 3.0, pal().hover);
                        }
                        // Swatch: ribbon, canvas and accent of that theme.
                        let sw = Rect::from_min_size(pos2(rect.left() + 6.0, rect.center().y - 7.0), vec2(30.0, 14.0));
                        painter.rect_filled(sw, 2.0, p.ribbon);
                        painter.rect_filled(Rect::from_min_max(pos2(sw.left() + 12.0, sw.top()), sw.right_bottom()), 0.0, p.canvas);
                        painter.rect_filled(Rect::from_min_size(pos2(sw.right() - 8.0, sw.top() + 4.0), vec2(5.0, 6.0)), 1.0, p.accent);
                        painter.rect_stroke(sw, 2.0, Stroke::new(1.0, pal().sep), StrokeKind::Outside);
                        let on = i == theme::current();
                        painter.text(pos2(sw.right() + 10.0, rect.center().y), Align2::LEFT_CENTER, p.name, theme::ui_font(13.0), if on { pal().accent_text } else { pal().text });
                        if on {
                            painter.text(pos2(rect.right() - 10.0, rect.center().y), Align2::RIGHT_CENTER, "✓", theme::ui_font(13.0), pal().accent_text);
                        }
                        if resp.clicked() {
                            app.change_settings(|s| s.choose_theme(i));
                        }
                    }
                    ui.add_space(4.0);
                }
            });
        });
    });
    group(ui, "Show", |ui| {
        column(ui, |ui| {
            let mut ruler = app.show_ruler;
            if ui.checkbox(&mut ruler, "Ruler").changed() {
                cmds.push(Cmd::ToggleRuler);
            }
            let mut grid = false;
            ui.add_enabled(false, egui::Checkbox::new(&mut grid, "Gridlines"));
            let mut nav = app.show_nav;
            if ui.checkbox(&mut nav, "Navigation Pane").changed() {
                cmds.push(Cmd::ToggleNav);
            }
        });
    });
    group(ui, "Zoom", |ui| {
        let r = big(ui, Icon::Zoom, "Zoom", false, true);
        popup(&r, |ui| {
            for z in [0.5f32, 0.75, 1.0, 1.2, 1.5, 2.0, 3.0] {
                if ui.selectable_label((app.zoom - z).abs() < 0.01, format!("{}%", (z * 100.0) as i32)).clicked() {
                    cmds.push(Cmd::Zoom(z));
                }
            }
        });
        if big(ui, Icon::Zoom100, "100%", false, true).clicked() {
            cmds.push(Cmd::Zoom(1.0));
        }
        column(ui, |ui| {
            if small(ui, Icon::OnePage, "One Page", false, true).clicked() {
                cmds.push(Cmd::ZoomOnePage);
            }
            small(ui, Icon::MultiPage, "Multiple Pages", false, false);
            if small(ui, Icon::PageWidth, "Page Width", false, true).clicked() {
                cmds.push(Cmd::ZoomPageWidth);
            }
        });
    });
    group(ui, "Window", |ui| {
        column(ui, |ui| {
            small(ui, Icon::NewWindow, "New Window", false, false);
            small(ui, Icon::ArrangeAll, "Arrange All", false, false);
            small(ui, Icon::Split, "Split", false, false);
        });
        big(ui, Icon::SwitchWindows, "Switch\nWindows", true, false);
    });
    group(ui, "Macros", |ui| {
        big(ui, Icon::Macros, "Macros", true, false);
    });
}

fn help(ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    group(ui, "Help", |ui| {
        if big(ui, Icon::Help, "Help", false, true).clicked() {
            cmds.push(Cmd::About);
        }
        if big(ui, Icon::Contact, "Contact\nSupport", false, true).clicked() {
            cmds.push(Cmd::About);
        }
        big(ui, Icon::Training, "Show\nTraining", false, false);
    });
}

// ---------------------------------------------------------------------------
// Insert → Page Number gallery

fn page_number_menu(app: &App, r: &Response, cmds: &mut Vec<Cmd>) {
    use rusty_document_writer::pagenum::{self, Place};
    egui::Popup::from_toggle_button_response(r).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        let id = Id::NULL.with("pagenum-place");
        let mut place: Place = ui.data(|d| d.get_temp(id)).unwrap_or(Place::Bottom);
        ui.set_width(300.0);
        ui.horizontal(|ui| {
            for (p, label) in [(Place::Bottom, "Bottom of Page"), (Place::Top, "Top of Page")] {
                if ui.selectable_label(place == p, label).clicked() {
                    place = p;
                }
            }
        });
        ui.data_mut(|d| d.insert_temp(id, place));
        ui.separator();
        egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
            for st in pagenum::STYLES {
                let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 64.0), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(rect, 3.0, pal().hover);
                }
                let tile = Rect::from_min_size(rect.min + vec2(4.0, 4.0), vec2(rect.width() - 8.0, 40.0));
                paint_page_number_preview(ui, tile, st, place);
                ui.painter().text(pos2(tile.left(), tile.bottom() + 9.0), Align2::LEFT_CENTER, st.name, theme::ui_font(12.0), pal().text);
                if resp.clicked() {
                    cmds.push(Cmd::PageNumber(st.id, place));
                    ui.close();
                }
            }
        });
        ui.separator();
        ui.label(egui::RichText::new("Number format").size(11.5).color(pal().text_dim));
        let cur = app.doc.page_number_format();
        ui.horizontal_wrapped(|ui| {
            for (fmt, label) in pagenum::FORMATS {
                if ui.selectable_label(cur == *fmt, *label).clicked() {
                    cmds.push(Cmd::PageNumberFormat(fmt));
                }
            }
        });
        ui.separator();
        let has = app.doc.has_page_numbers();
        if ui.add_enabled(has, egui::Button::new("Remove Page Numbers").frame(false).min_size(vec2(ui.available_width(), 24.0))).clicked() {
            cmds.push(Cmd::RemovePageNumbers);
            ui.close();
        }
    });
}

/// A miniature page edge showing where and how the number will appear.
fn paint_page_number_preview(ui: &Ui, tile: Rect, st: &rusty_document_writer::pagenum::PnStyle, place: rusty_document_writer::pagenum::Place) {
    use rusty_document_writer::pagenum::{Align as PA, Place};
    let p = ui.painter();
    p.rect_filled(tile, 2.0, Color32::WHITE);
    let margin = 18.0;
    let inner = tile.shrink2(vec2(margin, 0.0));
    let accent = Color32::from_rgb(0x15, 0x60, 0x82);
    let rule = Color32::from_gray(0xA6);
    let ink = Color32::from_gray(0x20);
    // Body text hint on the side away from the number.
    let (num_y, text_y0) = match place {
        Place::Bottom => (tile.bottom() - 11.0, tile.top() + 6.0),
        Place::Top => (tile.top() + 11.0, tile.bottom() - 14.0),
    };
    for i in 0..2 {
        let y = text_y0 + i as f32 * 5.0;
        let w = if i == 1 { inner.width() * 0.6 } else { inner.width() };
        p.hline(inner.left()..=(inner.left() + w), y, Stroke::new(2.0, Color32::from_gray(0xE2)));
    }
    let (label, color, font) = match st.id {
        "brackets" => ("[1]", ink, theme::ui_font(10.0)),
        "thick_line" => ("1", accent, theme::bold_font(10.0)),
        _ => ("1", ink, theme::ui_font(10.0)),
    };
    let (x, align) = match st.align {
        PA::Left => (inner.left(), Align2::LEFT_CENTER),
        PA::Center => (inner.center().x, Align2::CENTER_CENTER),
        PA::Right => (inner.right(), Align2::RIGHT_CENTER),
    };
    p.text(pos2(x, num_y), align, label, font, color);
    // Decorative rules on the side facing the body text.
    let toward_body = if place == Place::Bottom { -8.0 } else { 8.0 };
    match st.id {
        "thin_line" => {
            // Inset like the real rule (1" of 6.5" text width per side), swelling toward the middle.
            let inset = inner.width() * (1.0 / 6.5);
            let (x0, x1, y) = (inner.left() + inset, inner.right() - inset, num_y + toward_body);
            let n = 24;
            let pt = |i: usize, sign: f32| {
                let t = i as f32 / n as f32;
                let half = (0.3 + 1.1 * (std::f32::consts::PI * t).sin()) / 2.0;
                pos2(x0 + (x1 - x0) * t, y + sign * half)
            };
            let mut poly: Vec<egui::Pos2> = (0..=n).map(|i| pt(i, -1.0)).collect();
            poly.extend((0..=n).rev().map(|i| pt(i, 1.0)));
            p.add(egui::Shape::convex_polygon(poly, rule, Stroke::NONE));
        }
        "thick_line" => {
            p.hline(inner.x_range(), num_y + toward_body, Stroke::new(3.0, accent));
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Status bar

pub fn status_bar(app: &mut App, ui: &mut Ui, cmds: &mut Vec<Cmd>) {
    let full = ui.max_rect();
    let bar = Rect::from_min_max(pos2(full.left(), full.bottom() - STATUS_H), full.right_bottom());
    let p = ui.painter().clone();
    p.rect_filled(bar, 0.0, pal().bg);
    // Right-click anywhere on the bar to choose what it shows, as in Word.
    let bar_resp = ui.interact(bar, Id::NULL.with("status-bar"), Sense::click());
    bar_resp.context_menu(|ui| {
        let mut s = app.settings.clone();
        ui.label(egui::RichText::new("Customize Status Bar").small().color(pal().text_dim));
        ui.checkbox(&mut s.status_page, "Page Number");
        ui.checkbox(&mut s.status_words, "Word Count");
        ui.checkbox(&mut s.status_language, "Language");
        ui.checkbox(&mut s.status_accessibility, "Accessibility Checker");
        ui.checkbox(&mut s.status_view, "View Shortcuts");
        ui.checkbox(&mut s.status_zoom, "Zoom Slider");
        ui.separator();
        if ui.button("Hide the status bar").clicked() {
            s.show_status_bar = false;
            ui.close();
        }
        if s != app.settings {
            app.change_settings(|x| *x = s);
        }
    });
    let y = bar.center().y;
    let pages = app.layout.as_ref().map(|l| l.pages.len()).unwrap_or(1);
    let cur_page = app.locate(&app.caret).map(|(p, _)| p + 1).unwrap_or(1);
    let words = app.word_count();
    let mut x = bar.left() + 12.0;
    let st = &app.settings;
    let shown = [st.status_page, st.status_words, st.status_language, st.status_accessibility];
    let items = [format!("Page {cur_page} of {pages}"), format!("{} words", words), "English (United States)".into(), "Accessibility: Good to go".into()];
    for (i, t) in items.iter().enumerate().filter(|(i, _)| shown[*i]) {
        let rect = Rect::from_min_size(pos2(x - 4.0, bar.top() + 2.0), vec2(text_width(ui, t, 12.0) + 8.0, STATUS_H - 4.0));
        let r = ui.interact(rect, Id::NULL.with(("status", i)), Sense::click());
        if r.hovered() {
            p.rect_filled(rect, 2.0, pal().hover);
        }
        if i == 1 && r.clicked() {
            cmds.push(Cmd::WordCount);
        }
        p.text(pos2(x, y), Align2::LEFT_CENTER, t, theme::ui_font(12.0), pal().text_dim);
        x += text_width(ui, t, 12.0) + 24.0;
    }
    // Right side: zoom.
    let (show_zoom, show_view) = (app.settings.status_zoom, app.settings.status_view);
    let mut rx = bar.right() - 12.0;
    if show_zoom {
        let pct = format!("{}%", (app.zoom * 100.0).round() as i32);
        p.text(pos2(rx, y), Align2::RIGHT_CENTER, &pct, theme::ui_font(12.0), pal().text_dim);
        rx -= 50.0;
        let plus = Rect::from_center_size(pos2(rx, y), vec2(16.0, 16.0));
        if ui.interact(plus, Id::NULL.with("zplus"), Sense::click()).clicked() {
            cmds.push(Cmd::Zoom(((app.zoom * 10.0).round() + 1.0) / 10.0));
        }
        p.text(plus.center(), Align2::CENTER_CENTER, "+", theme::ui_font(15.0), pal().text_dim);
        let slider = Rect::from_min_max(pos2(rx - 120.0, y - 6.0), pos2(rx - 12.0, y + 6.0));
        p.hline(slider.x_range(), y, Stroke::new(1.0, pal().text_dim));
        p.vline(slider.center().x, (y - 4.0)..=(y + 4.0), Stroke::new(1.0, pal().text_dim));
        // Map 10%..100% to the left half and 100%..500% to the right half.
        let t = if app.zoom <= 1.0 { (app.zoom - 0.1) / 0.9 * 0.5 } else { 0.5 + (app.zoom - 1.0) / 4.0 * 0.5 };
        let kx = slider.left() + t.clamp(0.0, 1.0) * slider.width();
        p.rect_filled(Rect::from_center_size(pos2(kx, y), vec2(4.0, 12.0)), 1.0, pal().text);
        let sr = ui.interact(slider.expand(4.0), Id::NULL.with("zslider"), Sense::click_and_drag());
        if let (true, Some(pos)) = (sr.dragged() || sr.clicked(), sr.interact_pointer_pos()) {
            let t = ((pos.x - slider.left()) / slider.width()).clamp(0.0, 1.0);
            let z = if t <= 0.5 { 0.1 + t / 0.5 * 0.9 } else { 1.0 + (t - 0.5) / 0.5 * 4.0 };
            cmds.push(Cmd::Zoom((z * 100.0).round() / 100.0));
        }
        let minus = Rect::from_center_size(pos2(slider.left() - 12.0, y), vec2(16.0, 16.0));
        if ui.interact(minus, Id::NULL.with("zminus"), Sense::click()).clicked() {
            cmds.push(Cmd::Zoom(((app.zoom * 10.0).round() - 1.0).max(1.0) / 10.0));
        }
        p.text(minus.center(), Align2::CENTER_CENTER, "–", theme::ui_font(15.0), pal().text_dim);
        rx = minus.left() - 14.0;
    }
    if !show_view {
        return;
    }
    // View buttons.
    for (i, ic) in [Icon::WebLayout, Icon::PrintLayout, Icon::ReadMode].iter().enumerate() {
        let r = Rect::from_center_size(pos2(rx - 14.0, y), vec2(26.0, 20.0));
        if i == 1 {
            p.rect_filled(r, 2.0, pal().checked);
        }
        icons::paint(&p, *ic, r.shrink2(vec2(6.0, 3.0)), pal().text_dim);
        rx -= 30.0;
    }
    let focus = "Focus";
    p.text(pos2(rx - 8.0, y), Align2::RIGHT_CENTER, focus, theme::ui_font(12.0), pal().text_dim);
    icons::paint(&p, Icon::Focus, Rect::from_center_size(pos2(rx - 18.0 - text_width(ui, focus, 12.0), y), vec2(14.0, 14.0)), pal().text_dim);
}
