//! Text themes: one-click presets that combine character formatting
//! (bold, italic, caps, underline and strike styles, colour, highlight)
//! with the classic effects (outline, shadow, emboss, engrave).
//!
//! Every preset writes ordinary `w:rPr` properties, so a themed document
//! opens unchanged in any DOCX word processor.

use crate::edit;
use crate::props::RPR_ORDER;
use crate::xml::Element;

#[derive(Clone, Copy, Debug, Default)]
pub struct TextTheme {
    pub name: &'static str,
    pub color: &'static str,
    pub highlight: Option<&'static str>,
    pub bold: bool,
    pub italic: bool,
    pub caps: bool,
    pub small_caps: bool,
    pub underline: Option<&'static str>,
    pub underline_color: Option<&'static str>,
    pub strike: bool,
    pub dstrike: bool,
    pub outline: bool,
    pub shadow: bool,
    pub emboss: bool,
    pub imprint: bool,
}

const BASE: TextTheme = TextTheme {
    name: "",
    color: "000000",
    highlight: None,
    bold: false,
    italic: false,
    caps: false,
    small_caps: false,
    underline: None,
    underline_color: None,
    strike: false,
    dstrike: false,
    outline: false,
    shadow: false,
    emboss: false,
    imprint: false,
};

pub const THEMES: &[TextTheme] = &[
    TextTheme { name: "Headline", color: "0F4761", bold: true, shadow: true, ..BASE },
    TextTheme { name: "Hollow", color: "156082", bold: true, outline: true, ..BASE },
    TextTheme { name: "Neon", color: "3FE0FF", highlight: Some("black"), bold: true, outline: true, ..BASE },
    TextTheme { name: "Embossed", color: "404040", bold: true, emboss: true, ..BASE },
    TextTheme { name: "Engraved", color: "404040", bold: true, imprint: true, ..BASE },
    TextTheme { name: "Marquee", color: "C00000", bold: true, caps: true, underline: Some("double"), ..BASE },
    TextTheme { name: "Signature", color: "7030A0", bold: true, italic: true, underline: Some("wave"), ..BASE },
    TextTheme { name: "Whisper", color: "595959", italic: true, small_caps: true, ..BASE },
    TextTheme { name: "Struck Out", color: "C00000", dstrike: true, ..BASE },
    TextTheme { name: "Highlighter", color: "000000", highlight: Some("yellow"), bold: true, underline: Some("thick"), underline_color: Some("E97132"), ..BASE },
    TextTheme { name: "Typewriter", color: "262626", caps: true, underline: Some("dotted"), ..BASE },
    TextTheme { name: "Stamp", color: "C00000", bold: true, caps: true, outline: true, shadow: true, ..BASE },
];

impl TextTheme {
    /// Apply the theme to a run's properties. Every property the theme
    /// controls is written explicitly (on or off), so the result looks the
    /// same whatever paragraph style the text sits in, and applying another
    /// theme replaces this one instead of piling up.
    pub fn apply(&self, rpr: &mut Element) {
        for (name, on) in [
            ("w:b", self.bold),
            ("w:i", self.italic),
            ("w:caps", self.caps),
            ("w:smallCaps", self.small_caps),
            ("w:strike", self.strike),
            ("w:dstrike", self.dstrike),
            ("w:outline", self.outline),
            ("w:shadow", self.shadow),
            ("w:emboss", self.emboss),
            ("w:imprint", self.imprint),
        ] {
            if on {
                edit::set_toggle(rpr, name, true);
            } else if name == "w:b" || name == "w:i" {
                // Bold/italic are commonly inherited (headings), so switch them off explicitly.
                edit::set_toggle(rpr, name, false);
            } else {
                // The rest are rarely inherited: just drop direct formatting, keeping the XML tidy.
                rpr.remove_child(name);
            }
        }
        edit::set_color(rpr, Some(self.color));
        edit::set_highlight(rpr, self.highlight);
        match self.underline {
            Some(u) => {
                let mut el = Element::new("w:u").with_attr("w:val", u);
                if let Some(c) = self.underline_color {
                    el = el.with_attr("w:color", c);
                }
                rpr.set_child(el, RPR_ORDER);
            }
            None => rpr.remove_child("w:u"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(rpr: &Element) -> Vec<String> {
        rpr.elements().map(|c| format!("{}={}", c.name, c.attr("w:val").unwrap_or("on"))).collect()
    }

    #[test]
    fn effects_exclude_each_other() {
        let mut r = Element::new("w:rPr");
        edit::set_effect(&mut r, "w:outline", true);
        edit::set_effect(&mut r, "w:shadow", true);
        edit::set_effect(&mut r, "w:emboss", true);
        assert_eq!(names(&r), ["w:outline=0", "w:shadow=0", "w:emboss=on"]);
        edit::set_effect(&mut r, "w:caps", true);
        edit::set_effect(&mut r, "w:smallCaps", true);
        assert!(names(&r).contains(&"w:caps=0".to_string()));
    }

    #[test]
    fn themes_replace_each_other_and_keep_schema_order() {
        let mut r = Element::new("w:rPr");
        edit::set_size(&mut r, 14.0);
        THEMES.iter().find(|t| t.name == "Stamp").unwrap().apply(&mut r);
        THEMES.iter().find(|t| t.name == "Whisper").unwrap().apply(&mut r);
        let n = names(&r);
        assert!(!n.iter().any(|s| s.starts_with("w:outline") || s.starts_with("w:shadow") || s.starts_with("w:caps") || s.starts_with("w:u=")));
        assert!(n.contains(&"w:smallCaps=on".to_string()) && n.contains(&"w:b=0".to_string()));
        // Children must follow the CT_RPr sequence or strict consumers reject the file.
        let order: Vec<usize> = r.elements().map(|c| RPR_ORDER.iter().position(|o| *o == c.name).unwrap()).collect();
        assert!(order.windows(2).all(|w| w[0] < w[1]), "{n:?}");
    }

    #[test]
    fn underline_style_keeps_colour() {
        let mut r = Element::new("w:rPr");
        edit::set_underline_color(&mut r, Some("C00000"));
        edit::set_underline_style(&mut r, Some("wave"));
        let u = r.child("w:u").unwrap();
        assert_eq!((u.attr("w:val"), u.attr("w:color")), (Some("wave"), Some("C00000")));
        edit::set_underline_style(&mut r, None);
        assert_eq!(r.child("w:u").unwrap().attr("w:val"), Some("none"));
    }
}
