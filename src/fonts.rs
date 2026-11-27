//! System font discovery and metrics.
//!
//! DOCX line heights follow the GDI text metrics of the font:
//!   line height = usWinAscent + usWinDescent + max(0, hhea.lineGap − ((winAscent + winDescent) − (hhea.ascender − hhea.descender)))
//! (the last term is GDI's tmExternalLeading). We reproduce exactly that so
//! line heights — and therefore page breaks — match the reference.

use ab_glyph::{Font as _, FontArc, FontRef, GlyphId};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::{Arc, Mutex, OnceLock};

/// FxHash (as in rustc): the glyph and kerning caches are hit for every
/// character laid out, and SipHash's DoS resistance is pure overhead for
/// keys that are chars and glyph ids. It took a fifth of all layout time.
#[derive(Default)]
struct FxHasher(u64);

impl Hasher for FxHasher {
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b as u64);
        }
    }
    fn write_u8(&mut self, n: u8) {
        self.write_u64(n as u64);
    }
    fn write_u16(&mut self, n: u16) {
        self.write_u64(n as u64);
    }
    fn write_u32(&mut self, n: u32) {
        self.write_u64(n as u64);
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0.rotate_left(5) ^ n).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_usize(&mut self, n: usize) {
        self.write_u64(n as u64);
    }
    fn finish(&self) -> u64 {
        self.0
    }
}

const FAST_GLYPHS: usize = 0x800;

type FxMap<K, V> = HashMap<K, V, BuildHasherDefault<FxHasher>>;

#[derive(Clone, Debug)]
struct FaceEntry {
    path: PathBuf,
    index: u32,
    families: Vec<String>, // lowercased: legacy (ID 1), typographic (ID 16), full name (ID 4)
    bold: bool,
    italic: bool,
    weight: u16,
    /// Font compiled into the executable (see `EMBEDDED_FONTS`).
    embedded: Option<&'static [u8]>,
}

/// Fonts bundled into the binary so documents render identically everywhere.
///
/// Liberation Sans (SIL OFL 1.1) is metric-compatible with Arial: every glyph has
/// Arial's advance width, so text set in Arial breaks into exactly the same lines
/// and pages. It is used for "Arial" (and Helvetica) whenever the real font isn't
/// installed. Arial itself is proprietary and cannot be redistributed.
pub const EMBEDDED_FONTS: &[(&str, &[u8])] = &[
    ("LiberationSans-Regular.ttf", include_bytes!("../assets/fonts/LiberationSans-Regular.ttf")),
    ("LiberationSans-Bold.ttf", include_bytes!("../assets/fonts/LiberationSans-Bold.ttf")),
    ("LiberationSans-Italic.ttf", include_bytes!("../assets/fonts/LiberationSans-Italic.ttf")),
    ("LiberationSans-BoldItalic.ttf", include_bytes!("../assets/fonts/LiberationSans-BoldItalic.ttf")),
];

pub struct LoadedFont {
    pub id: usize,
    pub family: String,
    pub font: FontArc,
    pub upm: f32,
    pub win_ascent: f32,
    pub win_descent: f32,
    pub ext_leading: f32,
    pub underline_pos: f32,
    pub underline_thickness: f32,
    pub strike_pos: f32,
    pub strike_thickness: f32,
    pub symbol: bool,
    pub bold: bool,
    pub italic: bool,
    /// Symbol-encoded (3,0) cmap, which ab_glyph doesn't consult.
    symbol_map: HashMap<u32, u16>,
    /// Glyph and advance for U+0000..U+07FF (Latin, Greek, Cyrillic, Hebrew,
    /// Arabic) without taking a lock; the rest go through `advance_cache`.
    fast_glyphs: Box<[OnceLock<(GlyphId, f32)>; FAST_GLYPHS]>,
    advance_cache: Mutex<FxMap<char, (GlyphId, f32)>>,
    data: &'static [u8],
    face_index: u32,
    kern_cache: Mutex<FxMap<(u16, u16), f32>>,
}

impl LoadedFont {
    /// Glyph and advance in font units, with symbol-font PUA remapping.
    pub fn glyph(&self, c: char) -> (GlyphId, f32) {
        if let Some(slot) = self.fast_glyphs.get(c as usize) {
            return *slot.get_or_init(|| self.lookup_glyph(c));
        }
        if let Some(v) = self.advance_cache.lock().unwrap().get(&c) {
            return *v;
        }
        let v = self.lookup_glyph(c);
        self.advance_cache.lock().unwrap().insert(c, v);
        v
    }

    fn lookup_glyph(&self, c: char) -> (GlyphId, f32) {
        let mut g = self.font.glyph_id(c);
        if g.0 == 0 && self.symbol {
            let cp = c as u32;
            let cands = [cp, cp | 0xF000, cp.wrapping_sub(0xF000)];
            if let Some(id) = cands.iter().find_map(|k| self.symbol_map.get(k)) {
                g = GlyphId(*id);
            }
        }
        (g, self.font.h_advance_unscaled(g))
    }

    /// Pair kerning in font units (OpenType GPOS 'kern' feature, falling back to the legacy 'kern' table).
    pub fn kern(&self, a: GlyphId, b: GlyphId) -> f32 {
        if let Some(v) = self.kern_cache.lock().unwrap().get(&(a.0, b.0)) {
            return *v;
        }
        let v = self.gpos_kern(a, b).unwrap_or_else(|| self.font.kern_unscaled(a, b));
        self.kern_cache.lock().unwrap().insert((a.0, b.0), v);
        v
    }

    fn gpos_kern(&self, a: GlyphId, b: GlyphId) -> Option<f32> {
        use ttf_parser::gpos::{PairAdjustment, PositioningSubtable};
        let face = ttf_parser::Face::parse(self.data, self.face_index).ok()?;
        let gpos = face.tables().gpos?;
        let (ta, tb) = (ttf_parser::GlyphId(a.0), ttf_parser::GlyphId(b.0));
        let kern_tag = ttf_parser::Tag::from_bytes(b"kern");
        let mut seen = Vec::new();
        for feature in gpos.features {
            if feature.tag != kern_tag {
                continue;
            }
            for li in feature.lookup_indices {
                if seen.contains(&li) {
                    continue;
                }
                seen.push(li);
                let Some(lookup) = gpos.lookups.get(li) else { continue };
                for si in 0..lookup.subtables.len() {
                    let Some(PositioningSubtable::Pair(pa)) = lookup.subtables.get::<PositioningSubtable>(si) else { continue };
                    let Some(ci) = pa.coverage().get(ta) else { continue };
                    let rec = match pa {
                        PairAdjustment::Format1 { sets, .. } => sets.get(ci).and_then(|s| s.get(tb)),
                        PairAdjustment::Format2 { classes, matrix, .. } => matrix.get((classes.0.get(ta), classes.1.get(tb))),
                    };
                    if let Some((v1, _)) = rec {
                        return Some(v1.x_advance as f32);
                    }
                }
            }
        }
        if seen.is_empty() { None } else { Some(0.0) }
    }

    pub fn has_glyph(&self, c: char) -> bool {
        self.glyph(c).0 .0 != 0
    }

    /// Advance in points for a font size in points.
    pub fn advance(&self, c: char, size: f32) -> f32 {
        self.glyph(c).1 * size / self.upm
    }

    /// GDI-style ascent (pt) for the size.
    pub fn ascent(&self, size: f32) -> f32 {
        self.win_ascent * size / self.upm
    }
    pub fn descent(&self, size: f32) -> f32 {
        self.win_descent * size / self.upm
    }
    pub fn leading(&self, size: f32) -> f32 {
        self.ext_leading * size / self.upm
    }
    /// "Single" line height.
    pub fn line_height(&self, size: f32) -> f32 {
        (self.win_ascent + self.win_descent + self.ext_leading) * size / self.upm
    }
}

pub struct FontDb {
    entries: Vec<FaceEntry>,
    by_family: HashMap<String, Vec<usize>>,
    loaded: Mutex<HashMap<usize, Arc<LoadedFont>>>,
    resolve_cache: Mutex<HashMap<(String, bool, bool), FontMatch>>,
    fallback_cache: Mutex<FxMap<(char, bool, bool), Option<FontMatch>>>,
}

#[derive(Clone)]
pub struct FontMatch {
    pub font: Arc<LoadedFont>,
    pub synth_bold: bool,
    pub synth_italic: bool,
}

/// Metric-compatible or visually-closest substitutes, in priority order.
fn substitutes(family: &str) -> &'static [&'static str] {
    match family {
        "aptos" | "aptos narrow" | "aptos display" | "aptos serif" | "aptos mono" => {
            &["Aptos", "Calibri", "Carlito", "Segoe UI", "Arial", "Liberation Sans", "DejaVu Sans"]
        }
        "calibri" | "calibri light" => &["Carlito", "Segoe UI", "Arial", "Liberation Sans", "DejaVu Sans"],
        "cambria" | "cambria math" => &["Caladea", "Cambria", "Times New Roman", "Liberation Serif", "DejaVu Serif"],
        "arial" | "helvetica" | "arial nova" => &["Liberation Sans", "Arimo", "Helvetica", "DejaVu Sans"],
        "times new roman" | "times" => &["Liberation Serif", "Tinos", "Times", "DejaVu Serif"],
        "courier new" | "courier" => &["Liberation Mono", "Cousine", "Consolas", "DejaVu Sans Mono"],
        "symbol" => &["Symbol", "Segoe UI Symbol", "OpenSymbol"],
        "wingdings" => &["Wingdings", "Segoe UI Symbol", "OpenSymbol"],
        "georgia" => &["Gelasio", "Times New Roman", "Liberation Serif"],
        "verdana" | "tahoma" => &["DejaVu Sans", "Segoe UI", "Arial"],
        "consolas" => &["Courier New", "Liberation Mono", "DejaVu Sans Mono"],
        "ms mincho" | "ms gothic" | "yu gothic" | "yu mincho" | "simsun" | "mingliu" | "dengxian" | "malgun gothic" => {
            &["Yu Gothic", "MS Gothic", "Microsoft YaHei", "Malgun Gothic", "Noto Sans CJK JP", "Noto Sans CJK SC"]
        }
        _ => &[],
    }
}

const GLYPH_FALLBACKS: &[&str] = &[
    "Segoe UI", "Arial", "Segoe UI Symbol", "Segoe UI Emoji", "Cambria Math", "Microsoft YaHei", "Yu Gothic", "Malgun Gothic",
    "Nirmala UI", "Leelawadee UI", "Ebrima", "Gadugi", "Microsoft Himalaya", "Mongolian Baiti", "Microsoft Yi Baiti",
    "Segoe UI Historic", "Noto Sans", "Noto Sans CJK SC", "DejaVu Sans", "Liberation Sans",
];

/// The bytes of a font file, read once per process and shared by every face in
/// it (a .ttc such as Noto Sans CJK holds the Japanese, Chinese and Korean faces
/// in one 30 MB file). Loaded fonts live as long as the program anyway, so the
/// buffer is intentionally never freed; that is what lets ab_glyph and
/// ttf-parser borrow it without copying.
fn font_file(path: &Path) -> Option<&'static [u8]> {
    static FILES: OnceLock<Mutex<HashMap<PathBuf, &'static [u8]>>> = OnceLock::new();
    let files = FILES.get_or_init(Default::default);
    if let Some(b) = files.lock().unwrap().get(path) {
        return Some(b);
    }
    let bytes: &'static [u8] = Box::leak(std::fs::read(path).ok()?.into_boxed_slice());
    // Two threads may race to read the same file; keep the first copy.
    Some(*files.lock().unwrap().entry(path.to_path_buf()).or_insert(bytes))
}

fn font_dirs() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if cfg!(windows) {
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        v.push(PathBuf::from(windir).join("Fonts"));
        if let Ok(l) = std::env::var("LOCALAPPDATA") {
            v.push(PathBuf::from(&l).join("Microsoft\\Windows\\Fonts"));
            v.push(PathBuf::from(&l).join("Microsoft\\FontCache\\4\\CloudFonts"));
        }
    } else if cfg!(target_os = "macos") {
        v.push("/System/Library/Fonts".into());
        v.push("/Library/Fonts".into());
        if let Ok(h) = std::env::var("HOME") {
            v.push(PathBuf::from(h).join("Library/Fonts"));
        }
    } else {
        v.push("/usr/share/fonts".into());
        v.push("/usr/local/share/fonts".into());
        if let Ok(h) = std::env::var("HOME") {
            v.push(PathBuf::from(&h).join(".fonts"));
            v.push(PathBuf::from(&h).join(".local/share/fonts"));
        }
    }
    // Fonts shipped next to the executable (e.g. Aptos, Carlito).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(d) = exe.parent() {
            v.push(d.join("fonts"));
        }
    }
    v.push(PathBuf::from("fonts"));
    v
}

fn collect_font_files(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth < 4 {
                collect_font_files(&p, out, depth + 1);
            }
        } else if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
            if matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc" | "otc") {
                out.push(p);
            }
        }
    }
}

fn cache_path() -> Option<PathBuf> {
    let base = std::env::var("LOCALAPPDATA").or_else(|_| std::env::var("XDG_CACHE_HOME")).or_else(|_| std::env::var("HOME").map(|h| format!("{h}/.cache"))).ok()?;
    Some(PathBuf::from(base).join("RustyDocumentWriter").join("fontindex-v2.tsv"))
}

fn file_stamp(p: &Path) -> Option<(u64, u64)> {
    let m = std::fs::metadata(p).ok()?;
    let t = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
    Some((m.len(), t))
}

fn names_of(face: &ttf_parser::Face) -> Vec<String> {
    let mut fams: Vec<(u16, u16, String)> = Vec::new();
    for n in face.names() {
        if matches!(n.name_id, 1 | 4 | 16 | 21) {
            if let Some(s) = n.to_string() {
                fams.push((n.name_id, n.language_id, s));
            }
        }
    }
    let mut out: Vec<String> = Vec::new();
    // English (0x409) first, but keep localized names too (documents may use either).
    fams.sort_by_key(|(id, lang, _)| (*id != 1, *lang != 0x409));
    for (_, _, s) in fams {
        let l = s.trim().to_lowercase();
        if !l.is_empty() && !out.contains(&l) {
            out.push(l);
        }
    }
    out
}

fn scan_file(path: &Path) -> Vec<FaceEntry> {
    let Ok(data) = std::fs::read(path) else { return Vec::new() };
    scan_bytes(&data, path, None)
}

fn scan_bytes(data: &[u8], path: &Path, embedded: Option<&'static [u8]>) -> Vec<FaceEntry> {
    let data = data.to_vec();
    let n = ttf_parser::fonts_in_collection(&data).unwrap_or(1);
    let mut out = Vec::new();
    for i in 0..n {
        let Ok(face) = ttf_parser::Face::parse(&data, i) else { continue };
        let families = names_of(&face);
        if families.is_empty() {
            continue;
        }
        let weight = face.weight().to_number();
        out.push(FaceEntry {
            path: path.to_path_buf(),
            index: i,
            families,
            bold: face.is_bold() || weight >= 600,
            italic: face.is_italic() || face.is_oblique(),
            weight,
            embedded,
        });
    }
    out
}

impl FontDb {
    pub fn load_system() -> FontDb {
        let mut files = Vec::new();
        for d in font_dirs() {
            collect_font_files(&d, &mut files, 0);
        }
        files.sort();
        files.dedup();

        // Load cache.
        let mut cached: HashMap<PathBuf, ((u64, u64), Vec<FaceEntry>)> = HashMap::new();
        let cp = cache_path();
        if let Some(cp) = &cp {
            if let Ok(s) = std::fs::read_to_string(cp) {
                for line in s.lines() {
                    let f: Vec<&str> = line.split('\t').collect();
                    if f.len() < 8 {
                        continue;
                    }
                    let path = PathBuf::from(f[0]);
                    let stamp = (f[1].parse().unwrap_or(0), f[2].parse().unwrap_or(0));
                    let e = FaceEntry {
                        path: path.clone(),
                        index: f[3].parse().unwrap_or(0),
                        families: f[7].split('|').map(str::to_string).collect(),
                        bold: f[4] == "1",
                        italic: f[5] == "1",
                        weight: f[6].parse().unwrap_or(400),
                        embedded: None,
                    };
                    let slot = cached.entry(path).or_insert((stamp, Vec::new()));
                    if !(f[7].is_empty()) {
                        slot.1.push(e);
                    }
                }
            }
        }

        let mut entries = Vec::new();
        let mut cache_out = String::new();
        let mut changed = false;
        for f in &files {
            let Some(stamp) = file_stamp(f) else { continue };
            let faces = match cached.get(f) {
                Some((s, faces)) if *s == stamp => faces.clone(),
                _ => {
                    changed = true;
                    scan_file(f)
                }
            };
            if faces.is_empty() {
                cache_out.push_str(&format!("{}\t{}\t{}\t0\t0\t0\t0\t\n", f.display(), stamp.0, stamp.1));
            }
            for e in &faces {
                cache_out.push_str(&format!(
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    f.display(),
                    stamp.0,
                    stamp.1,
                    e.index,
                    e.bold as u8,
                    e.italic as u8,
                    e.weight,
                    e.families.join("|")
                ));
            }
            entries.extend(faces);
        }
        if changed || cached.len() != files.len() {
            if let Some(cp) = &cp {
                let _ = std::fs::create_dir_all(cp.parent().unwrap());
                let _ = std::fs::write(cp, cache_out);
            }
        }

        // Bundled fonts come last: an installed copy of the same family wins ties.
        for (name, bytes) in EMBEDDED_FONTS {
            entries.extend(scan_bytes(bytes, Path::new(&format!("<embedded>/{name}")), Some(bytes)));
        }
        let mut by_family: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, e) in entries.iter().enumerate() {
            for f in &e.families {
                by_family.entry(f.clone()).or_default().push(i);
            }
        }
        FontDb {
            entries,
            by_family,
            loaded: Mutex::new(HashMap::new()),
            resolve_cache: Mutex::new(HashMap::new()),
            fallback_cache: Mutex::new(FxMap::default()),
        }
    }

    pub fn family_names(&self) -> Vec<String> {
        // Display names: use the first (legacy, English) family of each entry.
        let mut v: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for e in &self.entries {
            if let Some(f) = e.families.first() {
                if seen.insert(f.clone()) {
                    v.push(f.clone());
                }
            }
        }
        v.sort();
        v
    }

    pub fn display_family(&self, lower: &str) -> String {
        // Title-case the family name for display.
        lower
            .split(' ')
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn has_family(&self, family: &str) -> bool {
        self.by_family.contains_key(&family.trim().to_lowercase())
    }

    fn load(&self, idx: usize) -> Option<Arc<LoadedFont>> {
        if let Some(f) = self.loaded.lock().unwrap().get(&idx) {
            return Some(f.clone());
        }
        let e = self.entries.get(idx)?;
        let data = match e.embedded {
            Some(b) => b,
            None => font_file(&e.path)?,
        };
        let (upm, wa, wd, el, ul, st, symbol, symbol_map) = {
            let face = ttf_parser::Face::parse(data, e.index).ok()?;
            let upm = face.units_per_em() as f32;
            let hhea = &face.tables().hhea;
            let (mut wa, mut wd) = match face.tables().os2 {
                Some(os2) => (os2.windows_ascender() as f32, (os2.windows_descender() as f32).abs()),
                None => (hhea.ascender as f32, (hhea.descender as f32).abs()),
            };
            if wa + wd <= 0.0 {
                wa = hhea.ascender as f32;
                wd = (hhea.descender as f32).abs();
            }
            let hhea_h = hhea.ascender as f32 - hhea.descender as f32;
            let ext = (hhea.line_gap as f32 - ((wa + wd) - hhea_h)).max(0.0);
            let ul = face.underline_metrics().map(|m| (m.position as f32, m.thickness as f32)).unwrap_or((-upm * 0.1, upm * 0.05));
            let st = face.strikeout_metrics().map(|m| (m.position as f32, m.thickness as f32)).unwrap_or((upm * 0.26, upm * 0.05));
            let mut symbol_map = HashMap::new();
            if let Some(cmap) = face.tables().cmap {
                for sub in cmap.subtables {
                    if sub.platform_id == ttf_parser::PlatformId::Windows && sub.encoding_id == 0 {
                        sub.codepoints(|cp| {
                            if let Some(g) = sub.glyph_index(cp) {
                                symbol_map.insert(cp, g.0);
                            }
                        });
                    }
                }
            }
            let symbol = !symbol_map.is_empty();
            (upm, wa, wd, ext, ul, st, symbol, symbol_map)
        };
        let fv = FontRef::try_from_slice_and_index(data, e.index).ok()?;
        let lf = Arc::new(LoadedFont {
            id: idx,
            family: e.families.first().cloned().unwrap_or_default(),
            font: FontArc::new(fv),
            upm,
            win_ascent: wa,
            win_descent: wd,
            ext_leading: el,
            underline_pos: ul.0,
            underline_thickness: ul.1.max(1.0),
            strike_pos: st.0,
            strike_thickness: st.1.max(1.0),
            symbol,
            bold: e.bold,
            italic: e.italic,
            symbol_map,
            data,
            face_index: e.index,
            kern_cache: Mutex::new(FxMap::default()),
            fast_glyphs: Box::new(std::array::from_fn(|_| OnceLock::new())),
            advance_cache: Mutex::new(FxMap::default()),
        });
        self.loaded.lock().unwrap().insert(idx, lf.clone());
        Some(lf)
    }

    pub fn get_loaded(&self, id: usize) -> Option<Arc<LoadedFont>> {
        self.load(id)
    }

    fn best_in_family(&self, family: &str, bold: bool, italic: bool) -> Option<(usize, bool, bool)> {
        let cands = self.by_family.get(family)?;
        let target_w: i32 = if bold { 700 } else { 400 };
        let mut best: Option<(i32, usize)> = None;
        for &i in cands {
            let e = &self.entries[i];
            // Prefer faces whose legacy family name equals the request: those are what GDI enumerates.
            let legacy = e.families.first().is_some_and(|f| f == family);
            let mut score = (e.weight as i32 - target_w).abs();
            if e.italic != italic {
                score += 1000;
            }
            if e.bold != bold {
                score += 300;
            }
            if !legacy {
                score += 50;
            }
            if best.is_none_or(|(s, _)| score < s) {
                best = Some((score, i));
            }
        }
        let (_, i) = best?;
        let e = &self.entries[i];
        Some((i, bold && !e.bold, italic && !e.italic))
    }

    /// Resolve a font family (as named in the document) to a loaded face.
    pub fn resolve(&self, family: &str, bold: bool, italic: bool) -> FontMatch {
        let key = (family.trim().to_lowercase(), bold, italic);
        if let Some(m) = self.resolve_cache.lock().unwrap().get(&key) {
            return m.clone();
        }
        let mut tries: Vec<String> = vec![key.0.clone()];
        tries.extend(substitutes(&key.0).iter().map(|s| s.to_lowercase()));
        tries.extend(["segoe ui", "arial", "liberation sans", "dejavu sans", "noto sans"].iter().map(|s| s.to_string()));
        let mut result = None;
        for t in &tries {
            if let Some((i, sb, si)) = self.best_in_family(t, bold, italic) {
                if let Some(f) = self.load(i) {
                    result = Some(FontMatch { font: f, synth_bold: sb, synth_italic: si });
                    break;
                }
            }
        }
        let m = result.unwrap_or_else(|| {
            // Absolutely anything that loads.
            for i in 0..self.entries.len() {
                if let Some(f) = self.load(i) {
                    return FontMatch { font: f, synth_bold: bold, synth_italic: italic };
                }
            }
            panic!("No usable fonts found on this system");
        });
        self.resolve_cache.lock().unwrap().insert(key, m.clone());
        m
    }

    /// Find a font that can render `c` when the requested one cannot.
    pub fn fallback_for(&self, c: char, bold: bool, italic: bool) -> Option<FontMatch> {
        let key = (c, bold, italic);
        if let Some(m) = self.fallback_cache.lock().unwrap().get(&key) {
            return m.clone();
        }
        let mut found = None;
        for fam in GLYPH_FALLBACKS {
            let m = self.resolve(fam, bold, italic);
            if m.font.family.eq_ignore_ascii_case(fam) && m.font.has_glyph(c) {
                found = Some(m);
                break;
            }
        }
        self.fallback_cache.lock().unwrap().insert(key, found.clone());
        found
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
