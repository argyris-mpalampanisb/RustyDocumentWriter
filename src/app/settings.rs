//! User preferences (File ▸ Options), kept in `settings.txt` as `key = value`
//! lines next to the recent-files list. Unknown keys are ignored and missing
//! ones take their defaults, so old and new versions can share the file.

use super::theme;
use egui::Color32;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// Interface language: "system", or a code from `i18n::LANGS` ("en", "el").
    pub language: String,
    /// Theme used when not following the system.
    pub theme: String,
    /// Switch between `light_theme` and `dark_theme` with the system's appearance.
    pub follow_system: bool,
    pub light_theme: String,
    pub dark_theme: String,
    /// Custom accent colour instead of the theme's own.
    pub accent: Option<Color32>,
    /// Size of the whole interface (1.0 = 100%).
    pub ui_scale: f32,
    /// Show only the tab names; a tab click shows the ribbon until you click the document.
    pub ribbon_collapsed: bool,
    pub show_status_bar: bool,
    pub status_page: bool,
    pub status_words: bool,
    pub status_language: bool,
    pub status_accessibility: bool,
    pub status_view: bool,
    pub status_zoom: bool,
    pub caret_blink: bool,
    /// Caret width in points.
    pub caret_width: f32,
    /// Document zoom when the app starts.
    pub start_zoom: f32,
    pub show_ruler: bool,
    pub show_nav: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: "system".into(),
            theme: "Black".into(),
            follow_system: false,
            light_theme: "White".into(),
            dark_theme: "Black".into(),
            accent: None,
            ui_scale: 1.0,
            ribbon_collapsed: false,
            show_status_bar: true,
            status_page: true,
            status_words: true,
            status_language: true,
            status_accessibility: true,
            status_view: true,
            status_zoom: true,
            caret_blink: true,
            caret_width: 1.5,
            start_zoom: 1.2,
            show_ruler: false,
            show_nav: false,
        }
    }
}

/// Accent colours offered in Options (besides the theme's own).
pub const ACCENTS: &[(&str, Color32)] = &[
    ("Blue", Color32::from_rgb(0x18, 0x5A, 0xBD)),
    ("Sky", Color32::from_rgb(0x00, 0x78, 0xD4)),
    ("Teal", Color32::from_rgb(0x03, 0x83, 0x87)),
    ("Green", Color32::from_rgb(0x10, 0x7C, 0x10)),
    ("Olive", Color32::from_rgb(0x6B, 0x7D, 0x1A)),
    ("Orange", Color32::from_rgb(0xCA, 0x50, 0x10)),
    ("Red", Color32::from_rgb(0xC4, 0x2B, 0x1C)),
    ("Pink", Color32::from_rgb(0xBF, 0x00, 0x77)),
    ("Purple", Color32::from_rgb(0x88, 0x17, 0x98)),
    ("Violet", Color32::from_rgb(0x6B, 0x4F, 0xD8)),
    ("Slate", Color32::from_rgb(0x51, 0x5C, 0x6B)),
];

pub const SCALES: &[f32] = &[0.8, 0.9, 1.0, 1.1, 1.25, 1.5];

fn dir() -> Option<PathBuf> {
    std::env::var("LOCALAPPDATA").or_else(|_| std::env::var("HOME")).ok().map(|b| PathBuf::from(b).join("RustyDocumentWriter"))
}

fn hex(c: Color32) -> String {
    format!("{:02X}{:02X}{:02X}", c.r(), c.g(), c.b())
}

fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

impl Settings {
    pub fn to_text(&self) -> String {
        let b = |v: bool| if v { "true" } else { "false" };
        let lines = [
            ("language", self.language.clone()),
            ("theme", self.theme.clone()),
            ("follow_system", b(self.follow_system).into()),
            ("light_theme", self.light_theme.clone()),
            ("dark_theme", self.dark_theme.clone()),
            ("accent", self.accent.map(hex).unwrap_or_else(|| "theme".into())),
            ("ui_scale", format!("{}", self.ui_scale)),
            ("ribbon_collapsed", b(self.ribbon_collapsed).into()),
            ("show_status_bar", b(self.show_status_bar).into()),
            ("status_page", b(self.status_page).into()),
            ("status_words", b(self.status_words).into()),
            ("status_language", b(self.status_language).into()),
            ("status_accessibility", b(self.status_accessibility).into()),
            ("status_view", b(self.status_view).into()),
            ("status_zoom", b(self.status_zoom).into()),
            ("caret_blink", b(self.caret_blink).into()),
            ("caret_width", format!("{}", self.caret_width)),
            ("start_zoom", format!("{}", self.start_zoom)),
            ("show_ruler", b(self.show_ruler).into()),
            ("show_nav", b(self.show_nav).into()),
        ];
        let mut s = String::from("# RustyDocumentWriter settings (File > Options)\n");
        for (k, v) in lines {
            s.push_str(&format!("{k} = {v}\n"));
        }
        s
    }

    pub fn from_text(text: &str) -> Settings {
        let mut s = Settings::default();
        for line in text.lines() {
            let Some((k, v)) = line.split_once('=') else { continue };
            let (k, v) = (k.trim(), v.trim());
            let flag = matches!(v, "true" | "1" | "yes");
            let num = v.parse::<f32>().ok().filter(|n| n.is_finite());
            match k {
                "language" => s.language = v.into(),
                "theme" => s.theme = v.into(),
                "follow_system" => s.follow_system = flag,
                "light_theme" => s.light_theme = v.into(),
                "dark_theme" => s.dark_theme = v.into(),
                "accent" => s.accent = parse_hex(v),
                "ui_scale" => s.ui_scale = num.unwrap_or(1.0).clamp(0.5, 2.0),
                "ribbon_collapsed" => s.ribbon_collapsed = flag,
                "show_status_bar" => s.show_status_bar = flag,
                "status_page" => s.status_page = flag,
                "status_words" => s.status_words = flag,
                "status_language" => s.status_language = flag,
                "status_accessibility" => s.status_accessibility = flag,
                "status_view" => s.status_view = flag,
                "status_zoom" => s.status_zoom = flag,
                "caret_blink" => s.caret_blink = flag,
                "caret_width" => s.caret_width = num.unwrap_or(1.5).clamp(0.5, 4.0),
                "start_zoom" => s.start_zoom = num.unwrap_or(1.2).clamp(0.1, 5.0),
                "show_ruler" => s.show_ruler = flag,
                "show_nav" => s.show_nav = flag,
                _ => {}
            }
        }
        s
    }

    /// Load saved settings (or defaults). Picks up the theme from the older `theme.txt`.
    pub fn load() -> Settings {
        let Some(dir) = dir() else { return Settings::default() };
        match std::fs::read_to_string(dir.join("settings.txt")) {
            Ok(t) => Settings::from_text(&t),
            Err(_) => {
                let mut s = Settings::default();
                if let Ok(t) = std::fs::read_to_string(dir.join("theme.txt")) {
                    s.theme = t.trim().to_string();
                }
                s
            }
        }
    }

    pub fn save(&self) {
        if let Some(dir) = dir() {
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(dir.join("settings.txt"), self.to_text());
        }
    }

    /// The theme to show now (depends on the system's appearance when following it).
    pub fn effective_theme(&self, system_dark: Option<bool>) -> usize {
        let name = match (self.follow_system, system_dark) {
            (true, Some(true)) => &self.dark_theme,
            (true, Some(false)) => &self.light_theme,
            _ => &self.theme,
        };
        theme::find(name).unwrap_or(0)
    }

    /// Picking a theme: with "follow system" on, it fills the light or dark slot.
    pub fn choose_theme(&mut self, index: usize) {
        let p = &theme::PALETTES[index.min(theme::PALETTES.len() - 1)];
        if self.follow_system {
            if p.dark {
                self.dark_theme = p.name.into();
            } else {
                self.light_theme = p.name.into();
            }
        }
        self.theme = p.name.into();
    }

    /// Bring the UI in line with these settings (cheap when nothing changed).
    pub fn apply(&self, ctx: &egui::Context) {
        super::i18n::set(super::i18n::resolve(&self.language));
        let system_dark = ctx.system_theme().map(|t| t == egui::Theme::Dark);
        theme::set(ctx, self.effective_theme(system_dark), self.accent);
        if (ctx.zoom_factor() - self.ui_scale).abs() > 0.001 {
            ctx.set_zoom_factor(self.ui_scale);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_tolerates_junk() {
        let mut s = Settings::default();
        s.theme = "One Dark".into();
        s.accent = Some(Color32::from_rgb(0x12, 0x34, 0x56));
        s.ui_scale = 1.25;
        s.ribbon_collapsed = true;
        s.status_language = false;
        s.language = "el".into();
        assert_eq!(Settings::from_text(&s.to_text()), s);
        let odd = Settings::from_text("nonsense\nui_scale = 99\naccent = nothex\nfuture_key = 1\n");
        assert_eq!(odd.ui_scale, 2.0);
        assert_eq!(odd.accent, None);
    }

    #[test]
    fn follows_the_system() {
        let mut s = Settings { follow_system: true, ..Settings::default() };
        s.choose_theme(theme::find("Solarized Light").unwrap());
        assert_eq!(s.light_theme, "Solarized Light");
        assert_eq!(s.dark_theme, "Black");
        assert_eq!(s.effective_theme(Some(false)), theme::find("Solarized Light").unwrap());
        assert_eq!(s.effective_theme(Some(true)), theme::find("Black").unwrap());
    }
}
