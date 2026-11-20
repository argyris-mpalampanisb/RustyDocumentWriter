//! A small, lossless-enough XML DOM tailored to OOXML.
//!
//! OOXML never uses DTDs or custom entities, so a compact hand-written parser
//! is both faster and more predictable than a general purpose one. Element
//! names are stored with their prefix (e.g. `w:p`), attribute order is kept,
//! and unknown content is preserved so documents round-trip cleanly.

use std::fmt::Write as _;

#[derive(Clone, Debug, PartialEq)]
pub enum Node {
    Element(Element),
    Text(String),
    CData(String),
    Comment(String),
    Pi(String),
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<Node>,
}

#[derive(Debug)]
pub struct XmlError(pub String);

impl std::fmt::Display for XmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "XML error: {}", self.0)
    }
}
impl std::error::Error for XmlError {}

/// Strip a namespace prefix: `w:p` -> `p`.
pub fn local(name: &str) -> &str {
    match name.rfind(':') {
        Some(i) => &name[i + 1..],
        None => name,
    }
}

impl Element {
    pub fn new(name: impl Into<String>) -> Self {
        Element { name: name.into(), attrs: Vec::new(), children: Vec::new() }
    }

    pub fn with_attr(mut self, k: &str, v: impl Into<String>) -> Self {
        self.set_attr(k, v);
        self
    }

    pub fn with_child(mut self, e: Element) -> Self {
        self.children.push(Node::Element(e));
        self
    }

    pub fn with_text(mut self, t: impl Into<String>) -> Self {
        self.children.push(Node::Text(t.into()));
        self
    }

    pub fn local(&self) -> &str {
        local(&self.name)
    }

    pub fn is(&self, qname: &str) -> bool {
        self.name == qname
    }

    /// Attribute lookup by exact qualified name (`w:val`).
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
    }

    /// Attribute lookup ignoring the prefix (`val` matches `w:val`).
    pub fn attr_local(&self, lname: &str) -> Option<&str> {
        self.attrs.iter().find(|(k, _)| local(k) == lname).map(|(_, v)| v.as_str())
    }

    pub fn set_attr(&mut self, name: &str, value: impl Into<String>) {
        let value = value.into();
        if let Some(slot) = self.attrs.iter_mut().find(|(k, _)| k == name) {
            slot.1 = value;
        } else {
            self.attrs.push((name.to_string(), value));
        }
    }

    pub fn remove_attr(&mut self, name: &str) {
        self.attrs.retain(|(k, _)| k != name);
    }

    pub fn elements(&self) -> impl Iterator<Item = &Element> {
        self.children.iter().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    pub fn elements_mut(&mut self) -> impl Iterator<Item = &mut Element> {
        self.children.iter_mut().filter_map(|n| match n {
            Node::Element(e) => Some(e),
            _ => None,
        })
    }

    pub fn child(&self, qname: &str) -> Option<&Element> {
        self.elements().find(|e| e.name == qname)
    }

    pub fn child_mut(&mut self, qname: &str) -> Option<&mut Element> {
        self.elements_mut().find(|e| e.name == qname)
    }

    pub fn child_local(&self, lname: &str) -> Option<&Element> {
        self.elements().find(|e| e.local() == lname)
    }

    pub fn children_named<'a>(&'a self, qname: &'a str) -> impl Iterator<Item = &'a Element> + 'a {
        self.elements().filter(move |e| e.name == qname)
    }

    /// Find descendant (depth-first) by local name.
    pub fn find_local(&self, lname: &str) -> Option<&Element> {
        for e in self.elements() {
            if e.local() == lname {
                return Some(e);
            }
            if let Some(f) = e.find_local(lname) {
                return Some(f);
            }
        }
        None
    }

    pub fn find_all_local<'a>(&'a self, lname: &str, out: &mut Vec<&'a Element>) {
        for e in self.elements() {
            if e.local() == lname {
                out.push(e);
            }
            e.find_all_local(lname, out);
        }
    }

    /// Get or create a child, inserting it at the position dictated by `order`
    /// (OOXML schemas are sequence-sensitive; consumers reject out-of-order children).
    pub fn ensure_child(&mut self, qname: &str, order: &[&str]) -> &mut Element {
        if let Some(i) = self.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == qname)) {
            if let Node::Element(e) = &mut self.children[i] {
                return e;
            }
            unreachable!()
        }
        let idx = insert_index(self, qname, order);
        self.children.insert(idx, Node::Element(Element::new(qname)));
        match &mut self.children[idx] {
            Node::Element(e) => e,
            _ => unreachable!(),
        }
    }

    /// Replace (or insert, respecting `order`) a child element.
    pub fn set_child(&mut self, el: Element, order: &[&str]) {
        if let Some(i) = self.children.iter().position(|n| matches!(n, Node::Element(e) if e.name == el.name)) {
            self.children[i] = Node::Element(el);
            return;
        }
        let idx = insert_index(self, &el.name, order);
        self.children.insert(idx, Node::Element(el));
    }

    pub fn remove_child(&mut self, qname: &str) {
        self.children.retain(|n| !matches!(n, Node::Element(e) if e.name == qname));
    }

    /// Concatenated text of all descendant text nodes.
    pub fn text(&self) -> String {
        let mut s = String::new();
        fn rec(e: &Element, s: &mut String) {
            for c in &e.children {
                match c {
                    Node::Text(t) | Node::CData(t) => s.push_str(t),
                    Node::Element(e) => rec(e, s),
                    _ => {}
                }
            }
        }
        rec(self, &mut s);
        s
    }

    pub fn to_xml(&self) -> String {
        let mut s = String::with_capacity(256);
        write_element(self, &mut s);
        s
    }
}

fn insert_index(parent: &Element, qname: &str, order: &[&str]) -> usize {
    let my_rank = order.iter().position(|o| *o == qname);
    let Some(my_rank) = my_rank else {
        return parent.children.len();
    };
    for (i, n) in parent.children.iter().enumerate() {
        if let Node::Element(e) = n {
            if let Some(r) = order.iter().position(|o| *o == e.name) {
                if r > my_rank {
                    return i;
                }
            }
        }
    }
    parent.children.len()
}

// ---------------------------------------------------------------------------
// Parsing

pub struct Document {
    pub decl: Option<String>,
    pub root: Element,
}

pub fn parse(bytes: &[u8]) -> Result<Document, XmlError> {
    // Strip UTF-8 BOM; handle UTF-16 (rare in OOXML but legal).
    let text: std::borrow::Cow<str> = if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        String::from_utf8_lossy(&bytes[3..])
    } else if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let le = bytes[0] == 0xFF;
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) })
            .collect();
        std::borrow::Cow::Owned(String::from_utf16_lossy(&units))
    } else {
        String::from_utf8_lossy(bytes)
    };
    parse_str(&text)
}

pub fn parse_str(s: &str) -> Result<Document, XmlError> {
    let mut p = Parser { s: s.as_bytes(), src: s, i: 0 };
    let mut decl = None;
    let mut stack: Vec<Element> = Vec::new();
    let mut root: Option<Element> = None;

    while p.i < p.s.len() {
        if p.s[p.i] == b'<' {
            if p.starts_with("<?") {
                let end = p.find("?>")?;
                let body = &p.src[p.i + 2..end];
                if body.starts_with("xml ") || body == "xml" {
                    decl = Some(format!("<?{}?>", body));
                } else if let Some(top) = stack.last_mut() {
                    top.children.push(Node::Pi(body.to_string()));
                }
                p.i = end + 2;
            } else if p.starts_with("<!--") {
                let end = p.find("-->")?;
                let body = &p.src[p.i + 4..end];
                if let Some(top) = stack.last_mut() {
                    top.children.push(Node::Comment(body.to_string()));
                }
                p.i = end + 3;
            } else if p.starts_with("<![CDATA[") {
                let end = p.find("]]>")?;
                let body = &p.src[p.i + 9..end];
                if let Some(top) = stack.last_mut() {
                    top.children.push(Node::CData(body.to_string()));
                }
                p.i = end + 3;
            } else if p.starts_with("<!") {
                // DOCTYPE: skip (handles simple internal subsets).
                let mut depth = 0i32;
                while p.i < p.s.len() {
                    match p.s[p.i] {
                        b'[' => depth += 1,
                        b']' => depth -= 1,
                        b'>' if depth <= 0 => break,
                        _ => {}
                    }
                    p.i += 1;
                }
                p.i += 1;
            } else if p.starts_with("</") {
                p.i += 2;
                let name = p.name()?;
                p.skip_ws();
                p.expect(b'>')?;
                let el = stack.pop().ok_or_else(|| XmlError(format!("unexpected </{}>", name)))?;
                if el.name != name {
                    return Err(XmlError(format!("mismatched </{}>, expected </{}>", name, el.name)));
                }
                match stack.last_mut() {
                    Some(parent) => parent.children.push(Node::Element(el)),
                    None => root = Some(el),
                }
            } else {
                p.i += 1;
                let name = p.name()?;
                let mut el = Element::new(name);
                loop {
                    p.skip_ws();
                    if p.i >= p.s.len() {
                        return Err(XmlError("unexpected EOF in tag".into()));
                    }
                    match p.s[p.i] {
                        b'/' => {
                            p.i += 1;
                            p.expect(b'>')?;
                            match stack.last_mut() {
                                Some(parent) => parent.children.push(Node::Element(el)),
                                None => root = Some(el),
                            }
                            break;
                        }
                        b'>' => {
                            p.i += 1;
                            stack.push(el);
                            break;
                        }
                        _ => {
                            let k = p.name()?;
                            p.skip_ws();
                            p.expect(b'=')?;
                            p.skip_ws();
                            let q = p.s[p.i];
                            if q != b'"' && q != b'\'' {
                                return Err(XmlError("expected quote".into()));
                            }
                            p.i += 1;
                            let start = p.i;
                            while p.i < p.s.len() && p.s[p.i] != q {
                                p.i += 1;
                            }
                            let raw = &p.src[start..p.i];
                            p.i += 1;
                            el.attrs.push((k.to_string(), unescape(raw, true)));
                        }
                    }
                }
                if root.is_some() && stack.is_empty() {
                    break;
                }
            }
        } else {
            let start = p.i;
            while p.i < p.s.len() && p.s[p.i] != b'<' {
                p.i += 1;
            }
            let raw = &p.src[start..p.i];
            if let Some(top) = stack.last_mut() {
                let t = unescape(raw, false);
                // Merge adjacent text nodes.
                if let Some(Node::Text(prev)) = top.children.last_mut() {
                    prev.push_str(&t);
                } else {
                    top.children.push(Node::Text(t));
                }
            }
        }
    }
    let root = root.ok_or_else(|| XmlError("no root element".into()))?;
    Ok(Document { decl, root })
}

struct Parser<'a> {
    s: &'a [u8],
    src: &'a str,
    i: usize,
}

impl<'a> Parser<'a> {
    fn starts_with(&self, pat: &str) -> bool {
        self.s[self.i..].starts_with(pat.as_bytes())
    }
    fn find(&self, pat: &str) -> Result<usize, XmlError> {
        self.src[self.i..].find(pat).map(|o| o + self.i).ok_or_else(|| XmlError(format!("unterminated, expected {}", pat)))
    }
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\r' | b'\n') {
            self.i += 1;
        }
    }
    fn expect(&mut self, c: u8) -> Result<(), XmlError> {
        if self.i < self.s.len() && self.s[self.i] == c {
            self.i += 1;
            Ok(())
        } else {
            Err(XmlError(format!("expected '{}' at byte {}", c as char, self.i)))
        }
    }
    fn name(&mut self) -> Result<&'a str, XmlError> {
        let start = self.i;
        while self.i < self.s.len() {
            let c = self.s[self.i];
            if c.is_ascii_whitespace() || matches!(c, b'=' | b'>' | b'/' | b'<' | b'"' | b'\'') {
                break;
            }
            self.i += 1;
        }
        if self.i == start {
            return Err(XmlError(format!("expected name at byte {}", start)));
        }
        Ok(&self.src[start..self.i])
    }
}

fn unescape(raw: &str, attr: bool) -> String {
    if !raw.contains('&') && !raw.contains('\r') && !(attr && raw.contains(['\n', '\t'])) {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '&' => {
                if let Some(end) = raw[i..].find(';') {
                    let ent = &raw[i + 1..i + end];
                    let rep = match ent {
                        "amp" => Some('&'),
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "quot" => Some('"'),
                        "apos" => Some('\''),
                        _ if ent.starts_with("#x") || ent.starts_with("#X") => {
                            u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32)
                        }
                        _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
                        _ => None,
                    };
                    if let Some(r) = rep {
                        out.push(r);
                        while let Some(&(j, _)) = chars.peek() {
                            if j <= i + end {
                                chars.next();
                            } else {
                                break;
                            }
                        }
                        continue;
                    }
                }
                out.push('&');
            }
            '\r' => {
                if matches!(chars.peek(), Some((_, '\n'))) {
                    continue;
                }
                out.push(if attr { ' ' } else { '\n' });
            }
            '\n' | '\t' if attr => out.push(' '),
            _ => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Writing

pub fn escape_text(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\r' => out.push_str("&#13;"),
            _ => out.push(c),
        }
    }
}

pub fn escape_attr(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\n' => out.push_str("&#10;"),
            '\r' => out.push_str("&#13;"),
            '\t' => out.push_str("&#9;"),
            _ => out.push(c),
        }
    }
}

pub fn write_element(e: &Element, out: &mut String) {
    out.push('<');
    out.push_str(&e.name);
    for (k, v) in &e.attrs {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        escape_attr(v, out);
        out.push('"');
    }
    if e.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    for c in &e.children {
        match c {
            Node::Element(ch) => write_element(ch, out),
            Node::Text(t) => escape_text(t, out),
            Node::CData(t) => {
                let _ = write!(out, "<![CDATA[{}]]>", t);
            }
            Node::Comment(t) => {
                let _ = write!(out, "<!--{}-->", t);
            }
            Node::Pi(t) => {
                let _ = write!(out, "<?{}?>", t);
            }
        }
    }
    out.push_str("</");
    out.push_str(&e.name);
    out.push('>');
}

pub const STANDALONE_DECL: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#;

pub fn serialize_document(root: &Element, decl: Option<&str>) -> Vec<u8> {
    let mut s = String::with_capacity(4096);
    s.push_str(decl.unwrap_or(STANDALONE_DECL));
    s.push_str("\r\n");
    write_element(root, &mut s);
    s.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_basic() {
        let src = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="x"><w:body><w:p><w:r><w:t xml:space="preserve"> a &amp; b &lt;c&gt; &#x3A9;</w:t></w:r></w:p><!--c--></w:body></w:document>"#;
        let d = parse_str(src).unwrap();
        let t = d.root.find_local("t").unwrap();
        assert_eq!(t.text(), " a & b <c> Ω");
        let out = d.root.to_xml();
        let d2 = parse_str(&out).unwrap();
        assert_eq!(d.root, d2.root);
    }

    #[test]
    fn ordered_insert() {
        let mut e = Element::new("w:rPr");
        e.set_child(Element::new("w:sz"), &["w:rFonts", "w:b", "w:i", "w:sz"]);
        e.set_child(Element::new("w:b"), &["w:rFonts", "w:b", "w:i", "w:sz"]);
        e.set_child(Element::new("w:rFonts"), &["w:rFonts", "w:b", "w:i", "w:sz"]);
        let names: Vec<_> = e.elements().map(|e| e.name.clone()).collect();
        assert_eq!(names, vec!["w:rFonts", "w:b", "w:sz"]);
    }
}
