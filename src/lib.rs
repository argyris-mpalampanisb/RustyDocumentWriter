//! RustyDocumentWriter — a word processor built around faithful DOCX layout.
//!
//! * `xml` / `opc`   — lossless package & XML handling
//! * `model`         — editable document tree (paragraphs, runs, tables)
//! * `props`         — the DOCX formatting cascade (styles, themes, numbering)
//! * `document`      — a loaded DOCX: load / save / new
//! * `fonts`         — system fonts with GDI-compatible metrics
//! * `layout`        — faithful DOCX pagination & line layout
//! * `render`        — rasterizer for laid-out pages
//! * `edit`          — editing operations on the model

pub mod document;
pub mod edit;
pub mod fonts;
pub mod layout;
pub mod model;
pub mod opc;
pub mod props;
pub mod render;
pub mod template;
pub mod xml;
