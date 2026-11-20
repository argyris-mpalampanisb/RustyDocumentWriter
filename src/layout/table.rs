//! Table layout.

use super::para::{self, PageNums};
use super::{Ctx, Flow, Frag, Item, Story};
use crate::model::{CellItem, RowItem, Table};
use crate::props::{on_off, pt_attr, Border, CellCond, Color, ListCounters, TableCtx, TblLook};
use crate::xml::Element;

#[derive(Clone, Default)]
struct Borders {
    top: Option<Border>,
    left: Option<Border>,
    bottom: Option<Border>,
    right: Option<Border>,
    inside_h: Option<Border>,
    inside_v: Option<Border>,
}

fn read_borders(e: &Element, b: &mut Borders, tc: &crate::props::ThemeCtx) {
    for c in e.elements() {
        let v = Some(Border::parse(c, tc));
        match c.name.as_str() {
            "w:top" => b.top = v,
            "w:left" | "w:start" => b.left = v,
            "w:bottom" => b.bottom = v,
            "w:right" | "w:end" => b.right = v,
            "w:insideH" => b.inside_h = v,
            "w:insideV" => b.inside_v = v,
            _ => {}
        }
    }
}

fn read_margins(e: &Element, m: &mut [f32; 4]) {
    for c in e.elements() {
        let v = c.attr("w:w").and_then(crate::props::twips).map(|t| t / 20.0);
        if let Some(v) = v {
            if c.attr("w:type").is_some_and(|t| t != "dxa" && t != "nil") && c.attr("w:type") != Some("auto") {
                // pct margins are rare; ignore.
            }
            match c.name.as_str() {
                "w:top" => m[0] = v,
                "w:left" | "w:start" => m[1] = v,
                "w:bottom" => m[2] = v,
                "w:right" | "w:end" => m[3] = v,
                _ => {}
            }
        }
    }
}

struct TblInfo {
    style_id: Option<String>,
    ind: f32,
    jc: String,
    borders: Borders,
    mar: [f32; 4],
    look: TblLook,
    row_band: usize,
    col_band: usize,
    width: Option<(f32, String)>,
    shading: Option<Color>,
}

fn tbl_info(ctx: &Ctx, t: &Table) -> TblInfo {
    let tc = ctx.doc.theme_ctx();
    let style_id = t
        .tblpr
        .as_ref()
        .and_then(|p| p.child("w:tblStyle"))
        .and_then(|s| s.attr("w:val"))
        .map(str::to_string)
        .filter(|s| ctx.doc.styles.get(s).is_some())
        .or_else(|| ctx.doc.styles.default_table.clone());
    let mut info = TblInfo {
        style_id: style_id.clone(),
        ind: 0.0,
        jc: "left".into(),
        borders: Borders::default(),
        mar: [0.0, 5.4, 0.0, 5.4],
        look: TblLook::default(),
        row_band: 1,
        col_band: 1,
        width: None,
        shading: None,
    };
    let mut layers: Vec<&Element> = ctx.res.table_style_tblpr(style_id.as_deref());
    if let Some(p) = &t.tblpr {
        layers.push(p);
    }
    for p in layers {
        for c in p.elements() {
            match c.name.as_str() {
                "w:tblInd" => info.ind = pt_attr(c, "w:w").unwrap_or(0.0),
                "w:jc" => info.jc = c.attr("w:val").unwrap_or("left").to_string(),
                "w:tblBorders" => read_borders(c, &mut info.borders, &tc),
                "w:tblCellMar" => read_margins(c, &mut info.mar),
                "w:tblLook" => info.look = TblLook::parse(c),
                "w:tblStyleRowBandSize" => info.row_band = c.attr("w:val").and_then(|v| v.parse().ok()).unwrap_or(1),
                "w:tblStyleColBandSize" => info.col_band = c.attr("w:val").and_then(|v| v.parse().ok()).unwrap_or(1),
                "w:tblW" => {
                    let ty = c.attr("w:type").unwrap_or("dxa").to_string();
                    let raw = c.attr("w:w").unwrap_or("0");
                    let v = if ty == "pct" {
                        if let Some(p) = raw.strip_suffix('%') { p.parse::<f32>().unwrap_or(0.0) * 50.0 } else { raw.parse::<f32>().unwrap_or(0.0) }
                    } else {
                        crate::props::twips(raw).unwrap_or(0.0) / 20.0
                    };
                    info.width = Some((v, ty));
                }
                "w:shd" => info.shading = tc.shading(c),
                _ => {}
            }
        }
    }
    info
}

#[derive(Clone)]
struct CellProps {
    span: usize,
    vmerge: Option<bool>, // Some(true)=restart, Some(false)=continue
    borders: Borders,
    shading: Option<Color>,
    mar: [f32; 4],
    valign: String,
}

fn cell_props(ctx: &Ctx, tcpr: Option<&Element>, info: &TblInfo, tctx: &TableCtx) -> CellProps {
    let tc = ctx.doc.theme_ctx();
    let mut cp = CellProps { span: 1, vmerge: None, borders: Borders::default(), shading: info.shading, mar: info.mar, valign: "top".into() };
    // Conditional table-level borders (e.g. firstRow tblPr) feed into inside/outside.
    let mut layers = ctx.res.table_style_tcpr(tctx);
    if let Some(p) = tcpr {
        layers.push(p);
    }
    for p in layers {
        for c in p.elements() {
            match c.name.as_str() {
                "w:gridSpan" => cp.span = c.attr("w:val").and_then(|v| v.parse().ok()).unwrap_or(1).max(1),
                "w:vMerge" => cp.vmerge = Some(c.attr("w:val") == Some("restart")),
                "w:tcBorders" => read_borders(c, &mut cp.borders, &tc),
                "w:shd" => cp.shading = tc.shading(c),
                "w:tcMar" => read_margins(c, &mut cp.mar),
                "w:vAlign" => cp.valign = c.attr("w:val").unwrap_or("top").to_string(),
                _ => {}
            }
        }
    }
    cp
}

#[derive(Clone)]
struct CellBox {
    col: usize,
    span: usize,
    frag: Frag,
    cp: CellProps,
    rows_spanned: usize,
    continuation: bool,
    edges: [Option<Border>; 4], // top, left, bottom, right
}

#[derive(Clone)]
struct RowBox {
    /// Content offset caused by the row's top border (cell content sits below it).
    border_off: f32,
    cells: Vec<CellBox>,
    height: f32,
    exact: bool,
    cant_split: bool,
    header: bool,
}

struct Grid {
    xs: Vec<f32>, // cumulative x of grid lines, relative to table left
    left: f32,    // table left offset relative to available area
}

fn compute_grid(t: &Table, info: &TblInfo, avail: f32, compat: u32, first_left_mar: f32) -> Grid {
    let mut widths: Vec<f32> = t.grid.iter().map(|w| *w as f32 / 20.0).collect();
    let max_cells = t.rows().map(|r| r.cells().map(|c| c.tcpr.as_ref().and_then(|p| p.child("w:gridSpan")).and_then(|g| g.attr("w:val")).and_then(|v| v.parse::<usize>().ok()).unwrap_or(1)).sum::<usize>()).max().unwrap_or(1);
    if widths.is_empty() || widths.iter().sum::<f32>() <= 0.0 {
        let n = max_cells.max(1);
        widths = vec![avail / n as f32; n];
    }
    while widths.len() < max_cells {
        widths.push(widths.last().copied().unwrap_or(72.0));
    }
    let sum: f32 = widths.iter().sum();
    if let Some((w, ty)) = &info.width {
        let target = match ty.as_str() {
            "pct" => Some(avail * w / 5000.0),
            "dxa" if *w > 0.0 => None,
            _ => None,
        };
        if let Some(target) = target {
            if sum > 0.0 && target > 0.0 {
                for x in widths.iter_mut() {
                    *x *= target / sum;
                }
            }
        }
    }
    let total: f32 = widths.iter().sum();
    let mut xs = vec![0.0];
    for w in &widths {
        xs.push(xs.last().unwrap() + w);
    }
    let left = match info.jc.as_str() {
        "center" => (avail - total) / 2.0,
        "right" | "end" => avail - total,
        _ => info.ind - if compat < 15 { first_left_mar } else { 0.0 },
    };
    Grid { xs, left }
}

fn layout_rows(ctx: &Ctx, t: &Table, info: &TblInfo, grid: &Grid, story: &Story, path: &[usize], counters: &mut ListCounters, pn: Option<&PageNums>) -> Vec<RowBox> {
    let rows: Vec<(usize, &crate::model::Row)> = t.rows.iter().enumerate().filter_map(|(i, r)| if let RowItem::Row(r) = r { Some((i, r)) } else { None }).collect();
    let nrows = rows.len();
    let ncols = grid.xs.len() - 1;
    let header_rows = rows.iter().take_while(|(_, r)| r.trpr.as_ref().and_then(|p| p.child("w:tblHeader")).is_some_and(on_off)).count();
    let mut out: Vec<RowBox> = Vec::new();
    for (ri, (row_idx, row)) in rows.iter().enumerate() {
        let trpr = row.trpr.as_ref();
        let grid_before = trpr.and_then(|p| p.child("w:gridBefore")).and_then(|g| g.attr("w:val")).and_then(|v| v.parse::<usize>().ok()).unwrap_or(0);
        let (min_h, exact) = trpr
            .and_then(|p| p.child("w:trHeight"))
            .map(|h| (pt_attr(h, "w:val").unwrap_or(0.0), h.attr("w:hRule") == Some("exact")))
            .unwrap_or((0.0, false));
        let cant_split = trpr.and_then(|p| p.child("w:cantSplit")).is_some_and(on_off);
        let header = ri < header_rows;
        let mut col = grid_before;
        let mut cells = Vec::new();
        let ncells = row.cells().count();
        for (ci, item) in row.cells.iter().enumerate() {
            let CellItem::Cell(cell) = item else { continue };
            let span_guess = cell.tcpr.as_ref().and_then(|p| p.child("w:gridSpan")).and_then(|g| g.attr("w:val")).and_then(|v| v.parse::<usize>().ok()).unwrap_or(1);
            let logical_col = cells.len();
            let cond = CellCond::compute(&info.look, ri, nrows, logical_col, ncells, header_rows, info.row_band, info.col_band);
            let tctx = TableCtx { style_id: info.style_id.clone(), cond };
            let cp = cell_props(ctx, cell.tcpr.as_ref(), info, &tctx);
            let span = cp.span.max(span_guess).min(ncols.saturating_sub(col).max(1));
            let c0 = col.min(ncols - 1);
            let c1 = (col + span).min(ncols);
            let cell_w = grid.xs[c1] - grid.xs[c0];
            let content_w = (cell_w - cp.mar[1] - cp.mar[3]).max(1.0);
            let continuation = cp.vmerge == Some(false);
            let mut cpath = path.to_vec();
            cpath.push(*row_idx);
            cpath.push(ci);
            let saved_bg = ctx.cell_bg.replace(cp.shading);
            let frag = if continuation { Frag::default() } else { para::layout_blocks(ctx, &cell.blocks, content_w, story, &cpath, Some(&tctx), counters, pn) };
            *ctx.cell_bg.borrow_mut() = saved_bg;
            // Resolve the cell's four edges.
            let tb = &info.borders;
            let is_first_row = ri == 0;
            let is_last_row = ri + 1 == nrows;
            let edges = [
                cp.borders.top.clone().or_else(|| if is_first_row { tb.top.clone() } else { tb.inside_h.clone() }),
                cp.borders.left.clone().or_else(|| if c0 == 0 { tb.left.clone() } else { tb.inside_v.clone() }),
                cp.borders.bottom.clone().or_else(|| if is_last_row { tb.bottom.clone() } else { tb.inside_h.clone() }),
                cp.borders.right.clone().or_else(|| if c1 >= ncols { tb.right.clone() } else { tb.inside_v.clone() }),
            ];
            cells.push(CellBox { col: c0, span: c1 - c0, frag, cp, rows_spanned: 1, continuation, edges });
            col += span;
        }
        let bw = |b: &Option<Border>| b.as_ref().filter(|b| b.is_visible()).map(|b| b.width).unwrap_or(0.0);
        let top_b = cells.iter().map(|c| bw(&c.edges[0])).fold(0.0, f32::max);
        let bot_b = cells.iter().map(|c| bw(&c.edges[2])).fold(0.0, f32::max);
        // Half of the top and bottom border widths are added to the row height.
        let border_extra = (top_b + bot_b) / 2.0;
        let content_h = cells.iter().filter(|c| !c.continuation && c.cp.vmerge.is_none()).map(|c| c.frag.height + c.cp.mar[0] + c.cp.mar[2]).fold(0.0, f32::max) + border_extra;
        let mut height = if exact { min_h } else { content_h.max(min_h) };
        if height <= 0.0 {
            height = 1.0;
        }
        out.push(RowBox { border_off: top_b / 2.0, cells, height, exact, cant_split, header });
    }
    // Vertical merges: compute spans and grow last row if needed.
    let n = out.len();
    for r in 0..n {
        for ci in 0..out[r].cells.len() {
            if out[r].cells[ci].cp.vmerge != Some(true) {
                continue;
            }
            let col = out[r].cells[ci].col;
            let mut span = 1;
            while r + span < n && out[r + span].cells.iter().any(|c| c.col == col && c.continuation) {
                span += 1;
            }
            out[r].cells[ci].rows_spanned = span;
            let need = out[r].cells[ci].frag.height + out[r].cells[ci].cp.mar[0] + out[r].cells[ci].cp.mar[2];
            let have: f32 = (r..r + span).map(|k| out[k].height).sum();
            if need > have && !out[r + span - 1].exact {
                out[r + span - 1].height += need - have;
            }
            // The merged cell's bottom edge comes from the last continuation cell.
            if let Some(last) = out[r + span - 1].cells.iter().find(|c| c.col == col) {
                let b = last.edges[2].clone();
                out[r].cells[ci].edges[2] = b;
            }
        }
    }
    out
}

fn draw_border(items: &mut Vec<Item>, b: &Option<Border>, x: f32, y: f32, w: f32, h: f32, horizontal: bool) {
    let Some(b) = b.as_ref().filter(|b| b.is_visible()) else { return };
    let c = b.color.unwrap_or(Color::BLACK);
    if b.style == "double" {
        let t = b.width / 3.0;
        if horizontal {
            items.push(Item::Rect { x, y: y - b.width / 2.0, w, h: t, color: c });
            items.push(Item::Rect { x, y: y + b.width / 2.0 - t, w, h: t, color: c });
        } else {
            items.push(Item::Rect { x: x - b.width / 2.0, y, w: t, h, color: c });
            items.push(Item::Rect { x: x + b.width / 2.0 - t, y, w: t, h, color: c });
        }
        return;
    }
    if horizontal {
        items.push(Item::Rect { x: x - b.width / 2.0, y: y - b.width / 2.0, w: w + b.width, h: b.width, color: c });
    } else {
        items.push(Item::Rect { x: x - b.width / 2.0, y: y - b.width / 2.0, w: b.width, h: h + b.width, color: c });
    }
}

/// Draw a row at (x0, y). `heights` provides heights of subsequent rows for vertical merges.
fn draw_row(row: &RowBox, grid: &Grid, x0: f32, y: f32, following: &[f32], items: &mut Vec<Item>, lines: &mut Vec<super::LineInfo>, floats: &mut Vec<(Item, bool)>) {
    let mut borders = Vec::new();
    for c in &row.cells {
        if c.continuation {
            continue;
        }
        let cx = x0 + grid.xs[c.col];
        let cw = grid.xs[c.col + c.span] - grid.xs[c.col];
        let mut ch = row.height;
        for k in 1..c.rows_spanned {
            ch += following.get(k - 1).copied().unwrap_or(0.0);
        }
        if let Some(s) = c.cp.shading {
            items.push(Item::Rect { x: cx, y, w: cw, h: ch, color: s });
        }
        let inner = ch - c.cp.mar[0] - c.cp.mar[2];
        let dy = match c.cp.valign.as_str() {
            "center" => ((inner - c.frag.height) / 2.0).max(0.0),
            "bottom" => (inner - c.frag.height).max(0.0),
            _ => 0.0,
        };
        let mut fr = c.frag.clone();
        fr.translate(cx + c.cp.mar[1], y + c.cp.mar[0] + dy + row.border_off);
        if row.exact {
            // Exact height rows clip their content.
            let limit = y + ch;
            fr.items.retain(|it| match it {
                Item::Glyphs { y, .. } => *y <= limit + 2.0,
                Item::Rect { y, .. } | Item::Image { y, .. } | Item::Placeholder { y, .. } => *y < limit,
            });
        }
        items.extend(fr.items);
        lines.extend(fr.lines);
        floats.extend(fr.floats);
        borders.push((c.edges.clone(), cx, y, cw, ch));
    }
    for (e, cx, cy, cw, ch) in borders {
        draw_border(items, &e[0], cx, cy, cw, 0.0, true);
        draw_border(items, &e[2], cx, cy + ch, cw, 0.0, true);
        draw_border(items, &e[1], cx, cy, 0.0, ch, false);
        draw_border(items, &e[3], cx + cw, cy, 0.0, ch, false);
    }
}

fn first_left_margin(ctx: &Ctx, t: &Table, info: &TblInfo) -> f32 {
    t.rows()
        .next()
        .and_then(|r| r.cells().next())
        .and_then(|c| c.tcpr.as_ref())
        .and_then(|p| p.child("w:tcMar"))
        .map(|m| {
            let mut mm = info.mar;
            read_margins(m, &mut mm);
            mm[1]
        })
        .unwrap_or_else(|| {
            let _ = ctx;
            info.mar[1]
        })
}

pub fn layout_table_frag(ctx: &Ctx, t: &Table, width: f32, story: &Story, path: &[usize], counters: &mut ListCounters, pn: Option<&PageNums>) -> Frag {
    let info = tbl_info(ctx, t);
    let grid = compute_grid(t, &info, width, ctx.doc.settings.compat_mode, first_left_margin(ctx, t, &info));
    let rows = layout_rows(ctx, t, &info, &grid, story, path, counters, pn);
    let mut fr = Frag::default();
    let mut y = 0.0;
    for (i, r) in rows.iter().enumerate() {
        let following: Vec<f32> = rows[i + 1..].iter().map(|r| r.height).collect();
        draw_row(r, &grid, grid.left, y, &following, &mut fr.items, &mut fr.lines, &mut fr.floats);
        y += r.height;
        fr.breaks.push(y);
    }
    fr.height = y;
    fr
}

fn split_row(row: &mut RowBox, avail: f32) -> Option<RowBox> {
    // Split each cell's content at the last break that fits.
    let mut rest = row.clone();
    let mut any = false;
    let mut rest_h: f32 = 0.0;
    for (c, rc) in row.cells.iter_mut().zip(rest.cells.iter_mut()) {
        if c.continuation {
            continue;
        }
        let limit = avail - c.cp.mar[0] - c.cp.mar[2];
        let b = c.frag.breaks.iter().copied().filter(|b| *b <= limit + 0.01).fold(0.0, f32::max);
        if b > 0.0 {
            any = true;
        }
        let tail = c.frag.split_off(b);
        rc.frag = tail;
        rest_h = rest_h.max(rc.frag.height + rc.cp.mar[0] + rc.cp.mar[2]);
        rc.rows_spanned = 1;
        c.rows_spanned = 1;
    }
    if !any {
        return None;
    }
    row.height = avail;
    rest.height = rest_h.max(1.0);
    // The split edge has no border unless the table defines inside borders; keep simple.
    Some(rest)
}

/// Number of rows starting at `i` that must stay together (vertical merges).
fn unit_len(rows: &[RowBox], i: usize) -> usize {
    let mut end = i + 1;
    let mut k = i;
    while k < end && k < rows.len() {
        for c in &rows[k].cells {
            if c.cp.vmerge == Some(true) {
                end = end.max(k + c.rows_spanned);
            }
        }
        k += 1;
    }
    end.min(rows.len()) - i
}

/// Height needed at the start of a table before it may break: the header rows
/// plus the first body row (and the rows vertically merged with it).
pub fn leading_height(flow: &Flow, t: &Table) -> f32 {
    let ctx = flow.ctx;
    let info = tbl_info(ctx, t);
    let grid = compute_grid(t, &info, flow.col_w(), ctx.doc.settings.compat_mode, first_left_margin(ctx, t, &info));
    let mut counters = flow.counters.clone();
    let rows = layout_rows(ctx, t, &info, &grid, &Story::Body, &[usize::MAX], &mut counters, None);
    let nh = rows.iter().take_while(|r| r.header).count();
    let first = nh.min(rows.len().saturating_sub(1));
    let n = if rows.is_empty() { 0 } else { unit_len(&rows, first) };
    rows[..(first + n).min(rows.len())].iter().map(|r| r.height).sum()
}

pub fn place_table(flow: &mut Flow, t: &Table, path: Vec<usize>) {
    let ctx = flow.ctx;
    let info = tbl_info(ctx, t);
    let col_w = flow.col_w();
    let grid = compute_grid(t, &info, col_w, ctx.doc.settings.compat_mode, first_left_margin(ctx, t, &info));
    let pn = PageNums { page: flow.page_number, total: 0, fmt: flow.sect.pg_num_fmt.clone() };
    let mut counters = std::mem::take(&mut flow.counters);
    let mut rows = layout_rows(ctx, t, &info, &grid, &Story::Body, &path, &mut counters, Some(&pn));
    flow.counters = counters;
    let headers: Vec<RowBox> = rows.iter().take_while(|r| r.header).cloned().collect();
    flow.y += flow.pending_after;
    flow.pending_after = 0.0;
    // Never leave only header rows at the bottom of a page: the headers plus the
    // first body unit must fit, otherwise the table starts on the next page.
    if !flow.at_top && !rows.is_empty() {
        let nh = headers.len().min(rows.len() - 1);
        let n = unit_len(&rows, nh);
        let lead: f32 = rows[..nh + n].iter().map(|r| r.height).sum();
        let page_h = flow.body_bottom - flow.col_top;
        if lead > flow.avail() + 0.01 && lead <= page_h && (nh > 0 || n > 1 || rows[nh].cant_split) {
            flow.next_column(true);
        }
    }
    let mut i = 0;
    let mut on_new_page = false;
    let mut guard = 0;
    while i < rows.len() {
        guard += 1;
        if guard > 10_000 {
            break;
        }
        if on_new_page && !headers.is_empty() && !rows[i].header {
            for h in &headers {
                let x0 = flow.col_x() + grid.left;
                let y = flow.y;
                let page = flow.pages.last_mut().unwrap();
                let mut fl = Vec::new();
                draw_row(h, &grid, x0, y, &[], &mut page.items, &mut page.lines, &mut fl);
                flow.y += h.height;
            }
            on_new_page = false;
        }
        let avail = flow.avail();
        let h = rows[i].height;
        // Rows joined by vertical merges move to the next page together.
        let unit = unit_len(&rows, i);
        if unit > 1 && !flow.at_top {
            let uh: f32 = rows[i..i + unit].iter().map(|r| r.height).sum();
            if uh > avail + 0.01 && uh <= flow.body_bottom - flow.col_top {
                flow.next_column(true);
                on_new_page = true;
                continue;
            }
        }
        if h > avail + 0.01 {
            let page_h = flow.body_bottom - flow.col_top;
            let can_split = !rows[i].cant_split || h > page_h;
            if can_split && !rows[i].exact && avail > 12.0 {
                let mut row = rows[i].clone();
                if let Some(rest) = split_row(&mut row, avail) {
                    let x0 = flow.col_x() + grid.left;
                    let y = flow.y;
                    let page = flow.pages.last_mut().unwrap();
                    let mut fl = Vec::new();
                    draw_row(&row, &grid, x0, y, &[], &mut page.items, &mut page.lines, &mut fl);
                    rows[i] = rest;
                    flow.next_column(true);
                    on_new_page = true;
                    continue;
                }
            }
            if !flow.at_top {
                flow.next_column(true);
                on_new_page = true;
                continue;
            }
        }
        let following: Vec<f32> = rows[i + 1..].iter().map(|r| r.height).collect();
        let x0 = flow.col_x() + grid.left;
        let y = flow.y;
        let page = flow.pages.last_mut().unwrap();
        let mut fl = Vec::new();
        draw_row(&rows[i], &grid, x0, y, &following, &mut page.items, &mut page.lines, &mut fl);
        for (it, _) in fl {
            page.items.push(it);
        }
        flow.y += h;
        flow.at_top = false;
        i += 1;
    }
    flow.last_border_group = None;
}
