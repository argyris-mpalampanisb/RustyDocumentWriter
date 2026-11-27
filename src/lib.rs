//! RustyDocumentWriter — a word processor built around faithful DOCX layout.
//!
//! * `xml` / `opc`   — lossless package & XML handling
//! * `model`         — editable document tree (paragraphs, runs, tables)
//! * `props`         — the DOCX formatting cascade (styles, themes, numbering)
//! * `document`      — a loaded DOCX: load / save / new
//! * `convert`       — other formats (.doc, .odt, .rtf…) via LibreOffice
//! * `pdf`           — PDF export of the laid-out pages
//! * `fonts`         — system fonts with GDI-compatible metrics
//! * `layout`        — faithful DOCX pagination & line layout
//! * `render`        — rasterizer for laid-out pages
//! * `edit`          — editing operations on the model
//! * `shapes`        — built-in 3D shape library (rendered with Blender)
//! * `texttheme`     — one-click text theme presets (formatting + effects)
//! * `wordart`       — WordArt text boxes with Word 2010 text effects

pub mod convert;
pub mod document;
pub mod edit;
pub mod fonts;
pub mod layout;
pub mod model;
pub mod opc;
pub mod pagenum;
pub mod pdf;
pub mod props;
pub mod render;
pub mod shapes;
pub mod template;
pub mod texttheme;
pub mod wordart;
pub mod xml;
