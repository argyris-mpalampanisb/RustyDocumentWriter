#!/usr/bin/env python3
"""Turn installed VS Code colour themes into UI palettes for src/app/theme.rs.

Usage: extract.py THEME.json [THEME.json ...]
A Zed theme family (like zed_builtin.json) works too. `--source NAME` labels the
themes that follow; `--name NAME` renames the next one. See generate.sh.
Prints one Rust `Palette` literal per theme. Follows "include" chains and
blends translucent colours over the surface they sit on, the way VS Code does.
"""
import json, os, re, sys

def jsonc(path):
    s = open(path, encoding="utf-8").read()
    out, i, n, q = [], 0, len(s), False
    while i < n:  # strip // and /* */ comments outside strings
        c = s[i]
        if q:
            out.append(c)
            if c == "\\": out.append(s[i + 1]); i += 1
            elif c == '"': q = False
        elif c == '"': q = True; out.append(c)
        elif s.startswith("//", i): i = s.find("\n", i); i = n if i < 0 else i; continue
        elif s.startswith("/*", i): i = s.find("*/", i) + 2; continue
        else: out.append(c)
        i += 1
    return json.loads(re.sub(r",(\s*[}\]])", r"\1", "".join(out)))

def manifest(path):
    """Label and uiTheme ("vs", "vs-dark", "hc-black", "hc-light") from the extension's package.json."""
    d = os.path.dirname(os.path.abspath(path))
    while d != "/" and not os.path.exists(os.path.join(d, "package.json")):
        d = os.path.dirname(d)
    try:
        pkg = jsonc(os.path.join(d, "package.json"))
    except (OSError, ValueError):
        return None, None
    for t in pkg.get("contributes", {}).get("themes", []):
        if os.path.normpath(os.path.join(d, t["path"])) == os.path.normpath(os.path.abspath(path)):
            label = t.get("label", "")
            return (None if label.startswith("%") else label), t.get("uiTheme")
    return None, None

def load(path):
    t = jsonc(path)
    colors, kind = {}, t.get("type")
    if "include" in t:
        base = load(os.path.join(os.path.dirname(path), t["include"]))
        colors.update(base["colors"]); kind = kind or base["type"]
    colors.update({k: v for k, v in t.get("colors", {}).items() if v})
    label, ui = manifest(path)
    if ui: kind = {"vs": "light", "vs-dark": "dark", "hc-black": "hc", "hc-light": "hcLight"}.get(ui, kind)
    return {"name": label or t.get("name") or os.path.basename(path), "type": kind or "dark", "colors": colors}

def rgba(h):
    h = h.lstrip("#")
    if len(h) in (3, 4): h = "".join(c * 2 for c in h)
    v = [int(h[i:i + 2], 16) for i in range(0, len(h), 2)]
    return (v + [255])[:4]

def over(fg, bg):
    a = fg[3] / 255
    return [round(fg[i] * a + bg[i] * (1 - a)) for i in range(3)]

def mix(a, b, t): return [round(a[i] + (b[i] - a[i]) * t) for i in range(3)]

def lum(c): return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]

def rel(c):
    f = lambda v: (v / 255) / 12.92 if v <= 10 else ((v / 255 + 0.055) / 1.055) ** 2.4
    return 0.2126 * f(c[0]) + 0.7152 * f(c[1]) + 0.0722 * f(c[2])

def contrast(a, b):
    x, y = sorted((rel(a), rel(b)))
    return (y + 0.05) / (x + 0.05)

def readable(c, on, target, toward):
    """Nudge `c` toward `toward` until it reaches `target` contrast against `on`."""
    for i in range(21):
        m = mix(c, toward, i / 20)
        if contrast(m, on) >= target: return m
    return toward

ZED = {  # Zed theme keys -> the VS Code keys palette() reads
    "editor.background": "editor.background", "panel.background": "sideBar.background",
    "title_bar.background": "titleBar.activeBackground", "background": "editorGroupHeader.tabsBackground",
    "element.hover": "list.hoverBackground", "element.active": "list.activeSelectionBackground",
    "element.selected": "list.inactiveSelectionBackground", "border": "sideBar.border", "text": "foreground",
    "text.muted": "descriptionForeground", "text.disabled": "disabledForeground", "icon": "icon.foreground",
    "text.accent": "textLink.foreground", "element.background": "input.background",
    "elevated_surface.background": "menu.background", "surface.background": "activityBar.background",
}

def zed_family(path):
    """Every theme in a Zed theme-family JSON, mapped onto VS Code colour keys."""
    for t in jsonc(path)["themes"]:
        st = t["style"]
        colors = {v: st[k] for k, v in ZED.items() if st.get(k)}
        colors["button.background"] = st.get("text.accent")
        yield {"name": t["name"], "type": t["appearance"], "colors": colors}

def palette(t):
    c, dark = t["colors"], t["type"] != "light" and t["type"] != "hcLight"
    def get(*keys, on=None):
        for k in keys:
            if k in c: return over(rgba(c[k]), on) if on else rgba(c[k])[:3]
        return None
    editor = get("editor.background") or ([0x1e] * 3 if dark else [0xff] * 3)
    ribbon = get("sideBar.background", "panel.background") or editor
    bg = get("titleBar.activeBackground", "activityBar.background", "editorGroupHeader.tabsBackground") or editor
    # The page canvas sits behind white pages: it must contrast with them.
    canvas = get("editorGroupHeader.tabsBackground", "panel.background", on=editor) or editor
    text = get("foreground", "sideBar.foreground", "editor.foreground") or ([0xcc] * 3 if dark else [0x33] * 3)
    hover = get("list.hoverBackground", "toolbar.hoverBackground", on=ribbon) or mix(ribbon, text, 0.12)
    if abs(lum(hover) - lum(ribbon)) < 9: hover = mix(ribbon, text, 0.12)
    pressed = get("list.activeSelectionBackground", on=ribbon) or mix(ribbon, text, 0.2)
    checked = get("list.inactiveSelectionBackground", on=ribbon) or mix(ribbon, text, 0.16)
    if abs(lum(checked) - lum(ribbon)) < 6: checked = mix(ribbon, text, 0.16)
    sep = get("sideBar.border", "panel.border", "editorGroup.border", "contrastBorder", on=ribbon)
    if not sep or abs(lum(sep) - lum(ribbon)) < 10: sep = mix(ribbon, text, 0.22)
    dim = get("descriptionForeground", on=ribbon) or mix(text, ribbon, 0.2)
    dis = get("disabledForeground", on=ribbon) or mix(text, ribbon, 0.5)
    icon = get("icon.foreground", on=ribbon) or mix(text, ribbon, 0.1)
    accent = get("button.background", "focusBorder", "activityBarBadge.background") or [0x2b, 0x88, 0xd8]
    link = get("textLink.foreground", "textLink.activeForeground") or accent
    field = get("input.background", on=ribbon) or editor
    window = get("menu.background", "editorWidget.background", "dropdown.background", on=editor) or ribbon
    side = get("activityBar.background") or bg
    # Safety nets: themes are written for code editors, not for a ribbon with white pages.
    if not dark and lum(canvas) > 232: canvas = mix(canvas, [0, 0, 0], 0.1)  # pages must stand out
    if abs(lum(canvas) - lum(ribbon)) < 5: canvas = mix(ribbon, [0, 0, 0], 0.35 if dark else 0.06)
    text = readable(text, ribbon, 7.0, [255] * 3 if dark else [0] * 3)
    dim = readable(dim, ribbon, 4.5, text)
    dis = readable(dis, ribbon, 2.0, text)
    icon = readable(icon, ribbon, 4.5, text)
    link = readable(link, ribbon, 4.5, [255] * 3 if dark else [0] * 3)
    accent = readable(accent, [255] * 3, 3.0, [0] * 3)  # white text sits on the accent
    if contrast(accent, ribbon) < 1.8: accent = readable(link, [255] * 3, 3.0, [0] * 3)
    return dict(name=t["name"], dark=dark, bg=bg, canvas=canvas, ribbon=ribbon, hover=hover, pressed=pressed,
                checked=checked, sep=sep, text=text, text_dim=dim, text_disabled=dis, icon=icon, accent=accent,
                accent_text=link, field=field, window=window, backstage_side=side, backstage=editor)

def rust(p, source):
    col = lambda v: "rgb(0x%02X, 0x%02X, 0x%02X)" % tuple(v)
    fields = ", ".join(f"{k}: {col(v)}" for k, v in p.items() if isinstance(v, list))
    return f'    Palette {{ name: "{p["name"]}", source: "{source}", dark: {str(p["dark"]).lower()}, {fields} }},'

args, source, rename = sys.argv[1:], "VS Code", None
while args:
    f = args.pop(0)
    if f == "--source": source = args.pop(0); continue
    if f == "--name": rename = args.pop(0); continue  # for themes whose label is a localisation key
    themes = list(zed_family(f)) if "themes" in jsonc(f) else [load(f)]
    for t in themes:
        if rename: t["name"], rename = rename, None
        print(rust(palette(t), source))
