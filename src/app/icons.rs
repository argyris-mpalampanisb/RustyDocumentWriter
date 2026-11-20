//! Procedurally drawn ribbon icons in a "line + accent" style.

use egui::{pos2, vec2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Stroke, StrokeKind};

pub const BLUE: Color32 = Color32::from_rgb(0x4F, 0xA3, 0xE8);
pub const RED: Color32 = Color32::from_rgb(0xE8, 0x4A, 0x3F);
pub const GREEN: Color32 = Color32::from_rgb(0x5C, 0xB8, 0x5C);
pub const YELLOW: Color32 = Color32::from_rgb(0xF5, 0xD3, 0x2E);
pub const ORANGE: Color32 = Color32::from_rgb(0xF0, 0x9A, 0x37);
pub const PURPLE: Color32 = Color32::from_rgb(0xA8, 0x7B, 0xE0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Paste, Cut, Copy, FormatPainter,
    Bold, Italic, Underline, Strike, Subscript, Superscript, TextEffects, Highlight, FontColor,
    GrowFont, ShrinkFont, ChangeCase, ClearFormat,
    Bullets, Numbering, Multilevel, IndentDec, IndentInc, Sort, Pilcrow,
    AlignLeft, AlignCenter, AlignRight, Justify, LineSpacing, Shading, Borders,
    Find, Replace, Select,
    CoverPage, BlankPage, PageBreak, Table, Pictures, Shapes, Icons3D, SmartArt, Chart, Screenshot, Video,
    Link, Bookmark, CrossRef, Comment, Header, Footer, PageNumber, TextBox, QuickParts, WordArt, DropCap,
    Signature, DateTime, Object, Equation, Symbol,
    Margins, Orientation, Size, Columns, Breaks, LineNumbers, Hyphenation,
    Position, WrapText, BringForward, SendBackward, SelectionPane, Align, Group, Rotate,
    Themes, Colors, Fonts, ParagraphSpacing, Effects, SetDefault, Watermark, PageColor, PageBorders,
    Toc, AddText, Update, Footnote, Endnote, NextFootnote, ShowNotes, Citation, ManageSources, Bibliography,
    Caption, TableOfFigures, MarkEntry, Index, MarkCitation,
    Envelopes, Labels, StartMerge, Recipients, EditRecipients, Highlight2, AddressBlock, Greeting, MergeField,
    Rules, MatchFields, UpdateLabels, Preview, FinishMerge,
    Spelling, Thesaurus, WordCount, ReadAloud, Accessibility, Language, NewComment, DeleteComment, Prev, Next,
    TrackChanges, Accept, Reject, Compare, Protect, HideInk,
    ReadMode, PrintLayout, WebLayout, Outline, Draft, Focus, Immersive, Vertical, SideToSide,
    Ruler, Gridlines, NavPane, Zoom, Zoom100, OnePage, MultiPage, PageWidth, NewWindow, ArrangeAll, Split,
    SwitchWindows, Macros, Properties, Help, Contact, Training, DarkMode,
    Pdf, Share, Review, Run, Generic,
}

fn s(c: Color32) -> Stroke {
    Stroke::new(1.3, c)
}

fn page(p: &Painter, r: Rect, fg: Color32) {
    // A document page with a folded corner.
    let fold = r.width() * 0.28;
    let pts = vec![
        r.left_top(),
        pos2(r.right() - fold, r.top()),
        pos2(r.right(), r.top() + fold),
        r.right_bottom(),
        r.left_bottom(),
        r.left_top(),
    ];
    p.add(egui::Shape::line(pts, s(fg)));
    p.line_segment([pos2(r.right() - fold, r.top()), pos2(r.right() - fold, r.top() + fold)], s(fg));
    p.line_segment([pos2(r.right() - fold, r.top() + fold), pos2(r.right(), r.top() + fold)], s(fg));
}

fn lines(p: &Painter, r: Rect, n: usize, fg: Color32, align: u8) {
    let step = r.height() / n as f32;
    for i in 0..n {
        let y = r.top() + step * (i as f32 + 0.5);
        let short = i % 2 == 1;
        let w = if short { r.width() * 0.65 } else { r.width() };
        let x0 = match align {
            1 => r.center().x - w / 2.0,
            2 => r.right() - w,
            _ => r.left(),
        };
        let w = if align == 3 { r.width() } else { w };
        let x0 = if align == 3 { r.left() } else { x0 };
        p.line_segment([pos2(x0, y), pos2(x0 + w, y)], s(fg));
    }
}

fn letter(p: &Painter, c: Pos2, text: &str, size: f32, color: Color32) {
    p.text(c, egui::Align2::CENTER_CENTER, text, FontId::proportional(size), color);
}

fn bar(p: &Painter, r: Rect, color: Color32) {
    p.rect_filled(Rect::from_min_size(pos2(r.left() + 1.0, r.bottom() - 3.5), vec2(r.width() - 2.0, 3.5)), CornerRadius::ZERO, color);
}

pub fn paint(p: &Painter, icon: Icon, r: Rect, fg: Color32) {
    let big = r.height() > 24.0;
    let c = r.center();
    let w = r.width();
    let h = r.height();
    let inset = |f: f32| r.shrink(w.min(h) * f);
    use Icon::*;
    match icon {
        Paste => {
            let b = Rect::from_min_max(pos2(r.left() + w * 0.12, r.top() + h * 0.12), pos2(r.right() - w * 0.22, r.bottom() - h * 0.05));
            p.rect_stroke(b, 2.0, s(fg), StrokeKind::Middle);
            p.rect_filled(Rect::from_center_size(pos2(b.center().x, b.top()), vec2(b.width() * 0.45, h * 0.14)), 1.5, fg);
            let pg = Rect::from_min_max(pos2(c.x - w * 0.02, c.y - h * 0.08), pos2(r.right() - w * 0.04, r.bottom()));
            p.rect_filled(pg, 0.0, Color32::from_gray(0x2b));
            page(p, pg, BLUE);
        }
        Cut => {
            p.circle_stroke(pos2(c.x - w * 0.2, r.bottom() - h * 0.22), w * 0.15, s(fg));
            p.circle_stroke(pos2(c.x + w * 0.2, r.bottom() - h * 0.22), w * 0.15, s(fg));
            p.line_segment([pos2(c.x - w * 0.1, r.bottom() - h * 0.33), pos2(c.x + w * 0.22, r.top() + h * 0.05)], s(fg));
            p.line_segment([pos2(c.x + w * 0.1, r.bottom() - h * 0.33), pos2(c.x - w * 0.22, r.top() + h * 0.05)], s(fg));
        }
        Copy => {
            page(p, Rect::from_min_size(r.left_top() + vec2(w * 0.1, h * 0.05), vec2(w * 0.55, h * 0.65)), fg);
            let b = Rect::from_min_size(r.left_top() + vec2(w * 0.35, h * 0.3), vec2(w * 0.55, h * 0.65));
            p.rect_filled(b, 0.0, Color32::from_gray(0x2b));
            page(p, b, fg);
        }
        FormatPainter => {
            p.rect_stroke(Rect::from_min_size(r.left_top() + vec2(w * 0.1, h * 0.08), vec2(w * 0.7, h * 0.3)), 1.5, s(fg), StrokeKind::Middle);
            p.line_segment([pos2(c.x + w * 0.35, r.top() + h * 0.23), pos2(c.x + w * 0.42, c.y)], s(fg));
            p.line_segment([pos2(c.x + w * 0.42, c.y), pos2(c.x, c.y + h * 0.05)], s(fg));
            p.rect_filled(Rect::from_min_size(pos2(c.x - w * 0.06, c.y + h * 0.05), vec2(w * 0.12, h * 0.4)), 1.0, ORANGE);
        }
        Bold => letter(p, c, "B", h * 0.85, fg),
        Italic => letter(p, c, "I", h * 0.85, fg),
        Underline => {
            letter(p, c - vec2(0.0, h * 0.08), "U", h * 0.75, fg);
            p.line_segment([pos2(r.left() + w * 0.2, r.bottom() - 1.5), pos2(r.right() - w * 0.2, r.bottom() - 1.5)], s(fg));
        }
        Strike => {
            letter(p, c, "ab", h * 0.75, fg);
            p.line_segment([pos2(r.left(), c.y + 1.0), pos2(r.right(), c.y + 1.0)], s(fg));
        }
        Subscript => {
            letter(p, c - vec2(w * 0.1, h * 0.05), "x", h * 0.8, fg);
            letter(p, pos2(r.right() - w * 0.15, r.bottom() - h * 0.18), "2", h * 0.45, BLUE);
        }
        Superscript => {
            letter(p, c - vec2(w * 0.1, -h * 0.05), "x", h * 0.8, fg);
            letter(p, pos2(r.right() - w * 0.15, r.top() + h * 0.2), "2", h * 0.45, BLUE);
        }
        TextEffects => {
            letter(p, c, "A", h * 0.95, BLUE);
        }
        Highlight => {
            p.line_segment([pos2(r.left() + w * 0.25, r.bottom() - h * 0.35), pos2(r.right() - w * 0.15, r.top() + h * 0.05)], Stroke::new(3.0, fg));
            bar(p, r, YELLOW);
        }
        FontColor => {
            letter(p, c - vec2(0.0, h * 0.12), "A", h * 0.75, fg);
            bar(p, r, RED);
        }
        GrowFont => {
            letter(p, c - vec2(w * 0.12, 0.0), "A", h * 0.8, fg);
            letter(p, pos2(r.right() - w * 0.12, r.top() + h * 0.2), "^", h * 0.5, fg);
        }
        ShrinkFont => {
            letter(p, c - vec2(w * 0.12, 0.0), "A", h * 0.65, fg);
            letter(p, pos2(r.right() - w * 0.12, r.top() + h * 0.25), "ˇ", h * 0.6, fg);
        }
        ChangeCase => letter(p, c, "Aa", h * 0.72, fg),
        ClearFormat => {
            letter(p, c - vec2(w * 0.1, h * 0.05), "A", h * 0.8, fg);
            p.rect_filled(Rect::from_min_size(pos2(c.x + w * 0.05, c.y + h * 0.05), vec2(w * 0.38, h * 0.28)), 2.0, PURPLE);
        }
        Bullets | Numbering | Multilevel => {
            for i in 0..3 {
                let y = r.top() + h * (0.2 + 0.3 * i as f32);
                let indent = if icon == Multilevel { i as f32 * w * 0.12 } else { 0.0 };
                let mx = r.left() + w * 0.12 + indent;
                match icon {
                    Bullets => {
                        p.circle_filled(pos2(mx, y), 1.8, BLUE);
                    }
                    _ => letter(p, pos2(mx, y), &(i + 1).to_string(), h * 0.32, BLUE),
                }
                p.line_segment([pos2(mx + w * 0.18, y), pos2(r.right(), y)], s(fg));
            }
        }
        IndentDec | IndentInc => {
            lines(p, inset(0.08), 4, fg, 2);
            let y = c.y;
            let (a, b) = if icon == IndentInc { (r.left(), r.left() + w * 0.3) } else { (r.left() + w * 0.3, r.left()) };
            p.arrow(pos2(a, y), vec2(b - a, 0.0), Stroke::new(1.5, BLUE));
        }
        Sort => {
            letter(p, pos2(c.x - w * 0.18, r.top() + h * 0.28), "A", h * 0.4, fg);
            letter(p, pos2(c.x - w * 0.18, r.bottom() - h * 0.28), "Z", h * 0.4, fg);
            p.arrow(pos2(c.x + w * 0.25, r.top() + h * 0.1), vec2(0.0, h * 0.8), Stroke::new(1.3, BLUE));
        }
        Pilcrow => letter(p, c, "¶", h * 0.9, fg),
        AlignLeft => lines(p, inset(0.08), 4, fg, 0),
        AlignCenter => lines(p, inset(0.08), 4, fg, 1),
        AlignRight => lines(p, inset(0.08), 4, fg, 2),
        Justify => lines(p, inset(0.08), 4, fg, 3),
        LineSpacing => {
            lines(p, Rect::from_min_max(pos2(r.left() + w * 0.4, r.top() + h * 0.1), r.right_bottom() - vec2(0.0, h * 0.1)), 4, fg, 3);
            p.arrow(pos2(r.left() + w * 0.15, c.y), vec2(0.0, -h * 0.42), Stroke::new(1.2, BLUE));
            p.arrow(pos2(r.left() + w * 0.15, c.y), vec2(0.0, h * 0.42), Stroke::new(1.2, BLUE));
        }
        Shading | PageColor => {
            let b = Rect::from_min_max(pos2(r.left() + w * 0.15, r.top() + h * 0.1), pos2(r.right() - w * 0.25, r.bottom() - h * 0.3));
            p.add(egui::Shape::convex_polygon(vec![pos2(b.left(), c.y - h * 0.05), pos2(b.center().x, b.top()), pos2(b.right(), c.y - h * 0.05), pos2(b.center().x, b.bottom())], Color32::TRANSPARENT, s(fg)));
            bar(p, r, if icon == Shading { YELLOW } else { ORANGE });
        }
        Borders | Table | PageBorders => {
            let b = inset(0.1);
            p.rect_stroke(b, 0.0, s(fg), StrokeKind::Middle);
            if icon != PageBorders {
                p.line_segment([pos2(b.left(), b.center().y), pos2(b.right(), b.center().y)], s(fg));
                p.line_segment([pos2(b.center().x, b.top()), pos2(b.center().x, b.bottom())], s(fg));
            }
            if icon == Table {
                p.rect_filled(Rect::from_min_max(b.left_top(), pos2(b.right(), b.top() + b.height() * 0.3)), 0.0, BLUE.gamma_multiply(0.6));
                p.line_segment([pos2(b.left() + b.width() / 3.0, b.top()), pos2(b.left() + b.width() / 3.0, b.bottom())], s(fg));
                p.line_segment([pos2(b.right() - b.width() / 3.0, b.top()), pos2(b.right() - b.width() / 3.0, b.bottom())], s(fg));
            }
            if icon == PageBorders {
                p.rect_stroke(b.shrink(w * 0.12), 0.0, Stroke::new(1.3, ORANGE), StrokeKind::Middle);
            }
        }
        Find | Zoom => {
            p.circle_stroke(c - vec2(w * 0.1, h * 0.1), w * 0.28, Stroke::new(1.5, fg));
            p.line_segment([c + vec2(w * 0.1, h * 0.1), r.right_bottom() - vec2(w * 0.08, h * 0.08)], Stroke::new(2.2, fg));
        }
        Replace => {
            letter(p, pos2(r.left() + w * 0.3, r.top() + h * 0.3), "b", h * 0.5, fg);
            letter(p, pos2(r.right() - w * 0.3, r.bottom() - h * 0.3), "c", h * 0.5, BLUE);
            p.arrow(pos2(r.left() + w * 0.2, c.y + h * 0.1), vec2(w * 0.3, h * 0.2), Stroke::new(1.2, BLUE));
        }
        Select => {
            let pts = vec![pos2(c.x - w * 0.2, r.top() + h * 0.08), pos2(c.x - w * 0.2, r.bottom() - h * 0.15), pos2(c.x - w * 0.05, c.y + h * 0.15), pos2(c.x + w * 0.2, c.y + h * 0.15)];
            p.add(egui::Shape::convex_polygon(pts, Color32::TRANSPARENT, s(fg)));
        }
        CoverPage | BlankPage | Header | Footer | PageNumber | Toc | Footnote | Endnote | Caption | TableOfFigures | Index | Bibliography => {
            let b = Rect::from_center_size(c, vec2(w * 0.68, h * 0.9));
            page(p, b, fg);
            let inner = b.shrink2(vec2(b.width() * 0.18, b.height() * 0.18));
            match icon {
                CoverPage => {
                    p.rect_filled(Rect::from_min_max(inner.left_top(), pos2(inner.right(), inner.top() + inner.height() * 0.45)), 0.0, BLUE);
                }
                Header => {
                    p.rect_filled(Rect::from_min_size(inner.left_top(), vec2(inner.width(), inner.height() * 0.2)), 0.0, ORANGE);
                    lines(p, Rect::from_min_max(pos2(inner.left(), inner.top() + inner.height() * 0.4), inner.right_bottom()), 3, fg, 0);
                }
                Footer => {
                    lines(p, Rect::from_min_max(inner.left_top(), pos2(inner.right(), inner.bottom() - inner.height() * 0.4)), 3, fg, 0);
                    p.rect_filled(Rect::from_min_max(pos2(inner.left(), inner.bottom() - inner.height() * 0.2), inner.right_bottom()), 0.0, ORANGE);
                }
                PageNumber => letter(p, inner.center(), "#", h * 0.4, BLUE),
                Toc | Index | Bibliography => lines(p, inner, 4, fg, 0),
                Footnote | Endnote => {
                    letter(p, inner.center() - vec2(0.0, h * 0.1), "ab", h * 0.3, fg);
                    letter(p, inner.right_top() + vec2(0.0, h * 0.05), "1", h * 0.25, RED);
                }
                Caption | TableOfFigures => {
                    p.rect_stroke(Rect::from_min_size(inner.left_top(), vec2(inner.width(), inner.height() * 0.5)), 0.0, s(BLUE), StrokeKind::Middle);
                    lines(p, Rect::from_min_max(pos2(inner.left(), inner.top() + inner.height() * 0.65), inner.right_bottom()), 2, fg, 0);
                }
                _ => {}
            }
        }
        PageBreak | Breaks => {
            let b = Rect::from_center_size(c, vec2(w * 0.6, h * 0.9));
            p.line_segment([b.left_top(), b.right_top()], s(fg));
            p.line_segment([b.left_top(), pos2(b.left(), b.top() + h * 0.3)], s(fg));
            p.line_segment([b.right_top(), pos2(b.right(), b.top() + h * 0.3)], s(fg));
            p.line_segment([b.left_bottom(), b.right_bottom()], s(fg));
            p.line_segment([b.left_bottom(), pos2(b.left(), b.bottom() - h * 0.3)], s(fg));
            p.line_segment([b.right_bottom(), pos2(b.right(), b.bottom() - h * 0.3)], s(fg));
            let mut x = b.left() - w * 0.1;
            while x < b.right() + w * 0.1 {
                p.line_segment([pos2(x, c.y), pos2(x + w * 0.08, c.y)], s(BLUE));
                x += w * 0.14;
            }
        }
        Pictures => {
            let b = inset(0.08);
            p.rect_stroke(b, 2.0, s(fg), StrokeKind::Middle);
            p.add(egui::Shape::convex_polygon(vec![pos2(b.left() + 2.0, b.bottom() - 2.0), pos2(b.left() + b.width() * 0.4, b.center().y), pos2(b.left() + b.width() * 0.65, b.bottom() - 2.0)], BLUE, Stroke::NONE));
            p.circle_filled(pos2(b.right() - b.width() * 0.25, b.top() + b.height() * 0.3), w * 0.07, YELLOW);
        }
        Shapes => {
            p.rect_stroke(Rect::from_min_size(r.left_top() + vec2(w * 0.08, h * 0.35), vec2(w * 0.5, h * 0.55)), 0.0, s(fg), StrokeKind::Middle);
            p.circle_filled(pos2(c.x + w * 0.15, c.y - h * 0.12), w * 0.27, BLUE);
        }
        Icons3D | Effects => {
            let pts = vec![pos2(c.x, r.top() + h * 0.08), pos2(r.right() - w * 0.1, c.y - h * 0.15), pos2(r.right() - w * 0.1, c.y + h * 0.25), pos2(c.x, r.bottom() - h * 0.05), pos2(r.left() + w * 0.1, c.y + h * 0.25), pos2(r.left() + w * 0.1, c.y - h * 0.15)];
            p.add(egui::Shape::closed_line(pts, Stroke::new(1.4, BLUE)));
            p.line_segment([pos2(c.x, c.y + h * 0.05), pos2(c.x, r.bottom() - h * 0.05)], s(BLUE));
        }
        SmartArt => {
            p.rect_filled(Rect::from_center_size(pos2(c.x, r.top() + h * 0.25), vec2(w * 0.4, h * 0.25)), 2.0, GREEN);
            p.rect_filled(Rect::from_center_size(pos2(c.x - w * 0.22, r.bottom() - h * 0.25), vec2(w * 0.35, h * 0.25)), 2.0, GREEN);
            p.rect_filled(Rect::from_center_size(pos2(c.x + w * 0.22, r.bottom() - h * 0.25), vec2(w * 0.35, h * 0.25)), 2.0, GREEN);
        }
        Chart => {
            for (i, (hh, col)) in [(0.45, BLUE), (0.8, BLUE), (0.6, fg)].iter().enumerate() {
                let x = r.left() + w * (0.15 + 0.27 * i as f32);
                p.rect_filled(Rect::from_min_max(pos2(x, r.bottom() - h * hh), pos2(x + w * 0.2, r.bottom())), 0.0, *col);
            }
        }
        Screenshot => {
            p.rect_stroke(inset(0.15), 0.0, Stroke::new(1.2, fg), StrokeKind::Middle);
            p.circle_stroke(c, w * 0.15, s(fg));
            letter(p, r.right_bottom() - vec2(w * 0.12, h * 0.15), "+", h * 0.5, GREEN);
        }
        Video => {
            p.rect_stroke(inset(0.12), 2.0, s(fg), StrokeKind::Middle);
            p.add(egui::Shape::convex_polygon(vec![c - vec2(w * 0.1, h * 0.15), c + vec2(w * 0.18, 0.0), c + vec2(-w * 0.1, h * 0.15)], fg, Stroke::NONE));
        }
        Link => {
            p.rect_stroke(Rect::from_center_size(c - vec2(w * 0.17, 0.0), vec2(w * 0.5, h * 0.35)), h * 0.17, Stroke::new(1.5, fg), StrokeKind::Middle);
            p.rect_stroke(Rect::from_center_size(c + vec2(w * 0.17, 0.0), vec2(w * 0.5, h * 0.35)), h * 0.17, Stroke::new(1.5, BLUE), StrokeKind::Middle);
        }
        Bookmark => {
            let b = Rect::from_center_size(c, vec2(w * 0.5, h * 0.8));
            p.add(egui::Shape::closed_line(vec![b.left_top(), b.right_top(), b.right_bottom(), pos2(b.center().x, b.bottom() - h * 0.2), b.left_bottom()], s(fg)));
        }
        CrossRef => {
            page(p, Rect::from_center_size(c - vec2(w * 0.1, 0.0), vec2(w * 0.55, h * 0.85)), fg);
            p.arrow(pos2(c.x, c.y + h * 0.1), vec2(w * 0.4, 0.0), Stroke::new(1.4, RED));
        }
        Comment | NewComment | DeleteComment | ShowNotes => {
            let b = Rect::from_min_max(r.left_top() + vec2(w * 0.08, h * 0.12), r.right_bottom() - vec2(w * 0.08, h * 0.3));
            p.rect_stroke(b, 2.0, s(fg), StrokeKind::Middle);
            p.line_segment([pos2(b.left() + w * 0.2, b.bottom()), pos2(b.left() + w * 0.15, r.bottom() - h * 0.08)], s(fg));
            if icon == NewComment {
                letter(p, b.center(), "+", h * 0.45, GREEN);
            }
            if icon == DeleteComment {
                letter(p, b.center(), "×", h * 0.45, RED);
            }
        }
        TextBox => {
            p.rect_stroke(inset(0.1), 0.0, s(BLUE), StrokeKind::Middle);
            letter(p, c, "A", h * 0.55, fg);
        }
        QuickParts => {
            p.rect_stroke(Rect::from_min_size(r.left_top() + vec2(w * 0.1, h * 0.1), vec2(w * 0.8, h * 0.35)), 1.0, s(fg), StrokeKind::Middle);
            p.rect_filled(Rect::from_min_size(r.left_top() + vec2(w * 0.1, h * 0.55), vec2(w * 0.8, h * 0.35)), 1.0, BLUE);
        }
        WordArt => {
            p.add(egui::Shape::line(vec![pos2(r.left() + w * 0.15, r.bottom() - h * 0.1), pos2(c.x, r.top() + h * 0.1), pos2(r.right() - w * 0.15, r.bottom() - h * 0.1)], Stroke::new(1.6, BLUE)));
            p.line_segment([pos2(c.x - w * 0.18, c.y + h * 0.12), pos2(c.x + w * 0.18, c.y + h * 0.12)], Stroke::new(1.6, BLUE));
        }
        DropCap => {
            letter(p, pos2(r.left() + w * 0.25, c.y - h * 0.1), "A", h * 0.6, fg);
            lines(p, Rect::from_min_max(pos2(c.x, r.top() + h * 0.1), r.right_bottom() - vec2(0.0, h * 0.1)), 4, fg, 0);
        }
        Signature => {
            p.add(egui::Shape::line(vec![pos2(r.left() + w * 0.1, c.y + h * 0.1), pos2(r.left() + w * 0.3, c.y - h * 0.2), pos2(c.x, c.y + h * 0.15), pos2(r.right() - w * 0.1, c.y - h * 0.25)], s(BLUE)));
            p.line_segment([pos2(r.left() + w * 0.1, r.bottom() - h * 0.15), pos2(r.right() - w * 0.1, r.bottom() - h * 0.15)], s(fg));
        }
        DateTime => {
            let b = inset(0.1);
            p.rect_stroke(b, 2.0, s(fg), StrokeKind::Middle);
            p.rect_filled(Rect::from_min_size(b.left_top(), vec2(b.width(), b.height() * 0.25)), 0.0, RED);
            p.circle_stroke(b.center() + vec2(0.0, h * 0.1), w * 0.18, s(BLUE));
        }
        Object => {
            p.rect_stroke(Rect::from_min_size(r.left_top() + vec2(w * 0.1, h * 0.1), vec2(w * 0.55, h * 0.55)), 0.0, s(fg), StrokeKind::Middle);
            p.rect_stroke(Rect::from_min_size(r.left_top() + vec2(w * 0.35, h * 0.35), vec2(w * 0.55, h * 0.55)), 0.0, s(BLUE), StrokeKind::Middle);
        }
        Equation => letter(p, c, "π", h * 0.95, fg),
        Symbol => letter(p, c, "Ω", h * 0.9, fg),
        Margins => {
            let b = Rect::from_center_size(c, vec2(w * 0.7, h * 0.9));
            p.rect_stroke(b, 0.0, s(fg), StrokeKind::Middle);
            p.rect_stroke(b.shrink(w * 0.1), 0.0, Stroke::new(1.0, BLUE), StrokeKind::Middle);
        }
        Orientation => {
            page(p, Rect::from_min_size(r.left_top() + vec2(w * 0.1, h * 0.05), vec2(w * 0.5, h * 0.75)), fg);
            p.rect_stroke(Rect::from_min_size(r.left_top() + vec2(w * 0.35, h * 0.5), vec2(w * 0.6, h * 0.42)), 0.0, s(BLUE), StrokeKind::Middle);
        }
        Size => {
            page(p, Rect::from_center_size(c + vec2(w * 0.08, 0.0), vec2(w * 0.55, h * 0.85)), fg);
            p.arrow(pos2(r.left() + w * 0.1, c.y), vec2(0.0, -h * 0.4), Stroke::new(1.2, BLUE));
            p.arrow(pos2(r.left() + w * 0.1, c.y), vec2(0.0, h * 0.4), Stroke::new(1.2, BLUE));
        }
        Columns => {
            let b = inset(0.1);
            lines(p, Rect::from_min_max(b.left_top(), pos2(b.center().x - w * 0.06, b.bottom())), 5, fg, 3);
            lines(p, Rect::from_min_max(pos2(b.center().x + w * 0.06, b.top()), b.right_bottom()), 5, BLUE, 3);
        }
        LineNumbers => {
            for i in 0..3 {
                let y = r.top() + h * (0.2 + 0.3 * i as f32);
                letter(p, pos2(r.left() + w * 0.15, y), &(i + 1).to_string(), h * 0.3, BLUE);
                p.line_segment([pos2(r.left() + w * 0.35, y), pos2(r.right(), y)], s(fg));
            }
        }
        Hyphenation => letter(p, c, "bc-", h * 0.6, fg),
        Position | WrapText => {
            let b = inset(0.08);
            lines(p, b, 5, fg, 3);
            p.rect_filled(Rect::from_center_size(b.center(), vec2(w * 0.4, h * 0.38)), 0.0, BLUE);
        }
        BringForward | SendBackward => {
            let back = Rect::from_min_size(r.left_top() + vec2(w * 0.1, h * 0.1), vec2(w * 0.55, h * 0.55));
            let front = Rect::from_min_size(r.left_top() + vec2(w * 0.35, h * 0.35), vec2(w * 0.55, h * 0.55));
            if icon == BringForward {
                p.rect_filled(back, 0.0, Color32::from_gray(0x80));
                p.rect_filled(front, 0.0, BLUE);
            } else {
                p.rect_filled(front, 0.0, Color32::from_gray(0x80));
                p.rect_filled(back, 0.0, BLUE);
            }
        }
        SelectionPane => {
            p.rect_stroke(inset(0.1), 0.0, s(fg), StrokeKind::Middle);
            p.rect_filled(Rect::from_min_max(pos2(c.x + w * 0.1, r.top() + h * 0.1), r.right_bottom() - vec2(w * 0.1, h * 0.1)), 0.0, ORANGE);
        }
        Align | Group | Rotate => {
            p.rect_stroke(Rect::from_min_size(r.left_top() + vec2(w * 0.15, h * 0.15), vec2(w * 0.3, h * 0.3)), 0.0, s(fg), StrokeKind::Middle);
            p.rect_stroke(Rect::from_min_size(r.left_top() + vec2(w * 0.5, h * 0.5), vec2(w * 0.35, h * 0.35)), 0.0, s(fg), StrokeKind::Middle);
        }
        Themes => {
            letter(p, c - vec2(0.0, h * 0.1), "Aa", h * 0.55, fg);
            for (i, col) in [BLUE, ORANGE, GREEN, RED].iter().enumerate() {
                p.rect_filled(Rect::from_min_size(pos2(r.left() + w * (0.1 + 0.2 * i as f32), r.bottom() - h * 0.2), vec2(w * 0.18, h * 0.15)), 0.0, *col);
            }
        }
        Colors => {
            p.rect_filled(Rect::from_min_max(r.left_top() + vec2(w * 0.1, h * 0.1), c), 0.0, Color32::from_rgb(0x0E, 0x28, 0x41));
            p.rect_filled(Rect::from_min_max(pos2(c.x, r.top() + h * 0.1), pos2(r.right() - w * 0.1, c.y)), 0.0, Color32::from_gray(0xE8));
            p.rect_filled(Rect::from_min_max(pos2(r.left() + w * 0.1, c.y), pos2(c.x, r.bottom() - h * 0.1)), 0.0, Color32::from_rgb(0x15, 0x60, 0x82));
            p.rect_filled(Rect::from_min_max(c, r.right_bottom() - vec2(w * 0.1, h * 0.1)), 0.0, Color32::from_rgb(0xE9, 0x71, 0x32));
        }
        Fonts => {
            p.rect_filled(inset(0.08), 0.0, Color32::WHITE);
            letter(p, c, "A", h * 0.8, Color32::BLACK);
        }
        ParagraphSpacing => lines(p, inset(0.12), 5, fg, 0),
        SetDefault | Accept => {
            p.circle_stroke(c, w * 0.38, s(GREEN));
            p.add(egui::Shape::line(vec![c + vec2(-w * 0.18, 0.0), c + vec2(-w * 0.04, h * 0.14), c + vec2(w * 0.2, -h * 0.15)], Stroke::new(1.6, GREEN)));
        }
        Reject => {
            page(p, Rect::from_center_size(c, vec2(w * 0.6, h * 0.85)), fg);
            letter(p, c + vec2(w * 0.1, h * 0.1), "×", h * 0.6, RED);
        }
        Watermark => {
            page(p, Rect::from_center_size(c, vec2(w * 0.7, h * 0.9)), fg);
            letter(p, c, "A", h * 0.45, RED);
        }
        AddText | Update | Rules | MatchFields | UpdateLabels | Preview | Highlight2 => {
            page(p, Rect::from_center_size(c, vec2(w * 0.6, h * 0.85)), fg);
            lines(p, Rect::from_center_size(c + vec2(0.0, h * 0.08), vec2(w * 0.3, h * 0.35)), 3, BLUE, 0);
        }
        NextFootnote | Prev | Next => {
            letter(p, c - vec2(w * 0.1, 0.0), "ab", h * 0.5, fg);
            p.arrow(c + vec2(w * 0.05, h * 0.25), vec2(w * 0.35, 0.0), Stroke::new(1.3, BLUE));
        }
        Citation | ManageSources | MarkCitation | MarkEntry => {
            page(p, Rect::from_center_size(c, vec2(w * 0.6, h * 0.85)), fg);
            p.add(egui::Shape::line(vec![c + vec2(-w * 0.15, 0.0), c + vec2(-w * 0.02, h * 0.13), c + vec2(w * 0.18, -h * 0.13)], Stroke::new(1.6, GREEN)));
        }
        Envelopes => {
            let b = Rect::from_center_size(c, vec2(w * 0.8, h * 0.55));
            p.rect_stroke(b, 0.0, s(fg), StrokeKind::Middle);
            p.add(egui::Shape::line(vec![b.left_top(), b.center(), b.right_top()], s(fg)));
        }
        Labels => {
            page(p, Rect::from_center_size(c - vec2(w * 0.08, 0.0), vec2(w * 0.55, h * 0.8)), fg);
            p.rect_stroke(Rect::from_center_size(c + vec2(w * 0.15, h * 0.15), vec2(w * 0.5, h * 0.3)), 0.0, s(BLUE), StrokeKind::Middle);
        }
        StartMerge | Recipients | EditRecipients | AddressBlock | Greeting | MergeField | FinishMerge => {
            page(p, Rect::from_center_size(c, vec2(w * 0.6, h * 0.85)), fg);
            p.rect_stroke(Rect::from_center_size(c + vec2(w * 0.12, h * 0.15), vec2(w * 0.4, h * 0.28)), 0.0, s(BLUE), StrokeKind::Middle);
        }
        Spelling => {
            letter(p, c - vec2(0.0, h * 0.15), "abc", h * 0.4, fg);
            p.add(egui::Shape::line(vec![c + vec2(-w * 0.15, h * 0.15), c + vec2(-w * 0.03, h * 0.3), c + vec2(w * 0.2, h * 0.02)], Stroke::new(1.6, GREEN)));
        }
        Thesaurus => {
            p.rect_stroke(inset(0.12), 1.0, s(fg), StrokeKind::Middle);
            p.line_segment([pos2(c.x, r.top() + h * 0.12), pos2(c.x, r.bottom() - h * 0.12)], s(fg));
        }
        WordCount => letter(p, c, "123", h * 0.42, BLUE),
        ReadAloud => {
            letter(p, c - vec2(w * 0.1, 0.0), "A", h * 0.7, fg);
            p.circle_stroke(c + vec2(w * 0.1, -h * 0.2), w * 0.12, s(BLUE));
        }
        Accessibility => {
            page(p, Rect::from_center_size(c, vec2(w * 0.6, h * 0.85)), fg);
            p.circle_filled(c + vec2(w * 0.1, h * 0.05), w * 0.1, BLUE);
        }
        Language => {
            p.rect_stroke(Rect::from_min_size(r.left_top() + vec2(w * 0.08, h * 0.08), vec2(w * 0.6, h * 0.55)), 1.0, s(fg), StrokeKind::Middle);
            p.circle_stroke(c + vec2(w * 0.15, h * 0.15), w * 0.22, s(BLUE));
        }
        TrackChanges | Compare | Protect | HideInk => {
            page(p, Rect::from_center_size(c - vec2(w * 0.05, 0.0), vec2(w * 0.6, h * 0.85)), fg);
            let col = match icon {
                Protect => ORANGE,
                HideInk => RED,
                _ => BLUE,
            };
            p.line_segment([c + vec2(-w * 0.1, h * 0.25), c + vec2(w * 0.3, -h * 0.15)], Stroke::new(2.0, col));
        }
        ReadMode | Immersive => {
            p.add(egui::Shape::line(vec![pos2(c.x, r.top() + h * 0.2), pos2(r.left() + w * 0.08, r.top() + h * 0.12), pos2(r.left() + w * 0.08, r.bottom() - h * 0.12), pos2(c.x, r.bottom() - h * 0.05), pos2(r.right() - w * 0.08, r.bottom() - h * 0.12), pos2(r.right() - w * 0.08, r.top() + h * 0.12), pos2(c.x, r.top() + h * 0.2), pos2(c.x, r.bottom() - h * 0.05)], s(fg)));
        }
        PrintLayout | Draft | Outline | Focus | OnePage | Vertical => {
            let b = Rect::from_center_size(c, vec2(w * 0.6, h * 0.88));
            p.rect_stroke(b, 0.0, s(fg), StrokeKind::Middle);
            lines(p, b.shrink2(vec2(b.width() * 0.2, b.height() * 0.2)), 4, fg, 0);
        }
        WebLayout => {
            let b = Rect::from_center_size(c - vec2(w * 0.05, 0.0), vec2(w * 0.6, h * 0.85));
            p.rect_stroke(b, 0.0, s(fg), StrokeKind::Middle);
            p.circle_stroke(r.right_bottom() - vec2(w * 0.25, h * 0.25), w * 0.2, s(BLUE));
        }
        SideToSide | MultiPage => {
            p.rect_stroke(Rect::from_min_size(r.left_top() + vec2(w * 0.05, h * 0.15), vec2(w * 0.42, h * 0.7)), 0.0, s(fg), StrokeKind::Middle);
            p.rect_stroke(Rect::from_min_size(r.left_top() + vec2(w * 0.53, h * 0.15), vec2(w * 0.42, h * 0.7)), 0.0, s(fg), StrokeKind::Middle);
        }
        Ruler | Gridlines | NavPane => {
            p.rect_stroke(inset(0.15), 0.0, s(fg), StrokeKind::Middle);
        }
        Zoom100 => {
            page(p, Rect::from_center_size(c, vec2(w * 0.6, h * 0.85)), fg);
            p.rect_filled(Rect::from_center_size(c + vec2(w * 0.1, h * 0.2), vec2(w * 0.5, h * 0.25)), 1.0, BLUE);
            letter(p, c + vec2(w * 0.1, h * 0.2), "100", h * 0.2, Color32::WHITE);
        }
        PageWidth => {
            p.rect_stroke(inset(0.15), 0.0, s(fg), StrokeKind::Middle);
            p.arrow(c, vec2(w * 0.35, 0.0), s(BLUE));
            p.arrow(c, vec2(-w * 0.35, 0.0), s(BLUE));
        }
        NewWindow | ArrangeAll | Split | SwitchWindows => {
            p.rect_stroke(Rect::from_min_size(r.left_top() + vec2(w * 0.1, h * 0.15), vec2(w * 0.7, h * 0.6)), 1.0, s(fg), StrokeKind::Middle);
            p.line_segment([pos2(r.left() + w * 0.1, r.top() + h * 0.3), pos2(r.left() + w * 0.8, r.top() + h * 0.3)], s(fg));
            p.rect_stroke(Rect::from_min_size(r.left_top() + vec2(w * 0.35, h * 0.4), vec2(w * 0.55, h * 0.45)), 1.0, s(BLUE), StrokeKind::Middle);
        }
        Macros => {
            page(p, Rect::from_center_size(c, vec2(w * 0.6, h * 0.85)), fg);
            p.rect_filled(Rect::from_center_size(c + vec2(w * 0.12, h * 0.15), vec2(w * 0.35, h * 0.3)), 2.0, RED);
        }
        Properties => {
            page(p, Rect::from_center_size(c, vec2(w * 0.6, h * 0.85)), fg);
            letter(p, c + vec2(w * 0.15, h * 0.18), "S", h * 0.35, Color32::from_rgb(0x03, 0x83, 0x87));
        }
        Help => {
            p.circle_stroke(c, w * 0.4, Stroke::new(1.5, BLUE));
            letter(p, c, "?", h * 0.6, BLUE);
        }
        Contact => {
            p.circle_stroke(c - vec2(0.0, h * 0.15), w * 0.18, s(fg));
            p.add(egui::Shape::line(vec![pos2(r.left() + w * 0.15, r.bottom() - h * 0.05), pos2(c.x, c.y + h * 0.1), pos2(r.right() - w * 0.15, r.bottom() - h * 0.05)], s(fg)));
        }
        Training => {
            p.add(egui::Shape::closed_line(vec![pos2(c.x, r.top() + h * 0.2), pos2(r.right() - w * 0.05, c.y - h * 0.08), pos2(c.x, c.y + h * 0.15), pos2(r.left() + w * 0.05, c.y - h * 0.08)], s(fg)));
            p.line_segment([pos2(c.x - w * 0.22, c.y + h * 0.02), pos2(c.x - w * 0.22, c.y + h * 0.3)], s(fg));
        }
        DarkMode => {
            p.circle_stroke(c, w * 0.35, s(fg));
            p.circle_filled(c + vec2(w * 0.15, -h * 0.12), w * 0.3, Color32::from_gray(0x2b));
        }
        Pdf => {
            page(p, Rect::from_center_size(c, vec2(w * 0.6, h * 0.85)), fg);
            p.rect_filled(Rect::from_min_size(r.left_top() + vec2(w * 0.1, h * 0.1), vec2(w * 0.45, h * 0.28)), 1.0, RED);
        }
        Share => {
            p.rect_stroke(Rect::from_min_size(r.left_top() + vec2(w * 0.1, h * 0.35), vec2(w * 0.6, h * 0.55)), 1.0, s(fg), StrokeKind::Middle);
            p.arrow(c, vec2(w * 0.35, -h * 0.35), s(fg));
        }
        Review | Run => {
            page(p, Rect::from_center_size(c, vec2(w * 0.6, h * 0.85)), fg);
            p.rect_filled(Rect::from_min_size(r.left_top() + vec2(w * 0.1, h * 0.1), vec2(w * 0.35, h * 0.2)), 1.0, RED);
            p.circle_filled(c + vec2(w * 0.12, h * 0.15), w * 0.12, YELLOW);
        }
        Generic => {
            page(p, Rect::from_center_size(c, vec2(w * 0.6, h * 0.85)), fg);
        }
    }
    let _ = big;
}
