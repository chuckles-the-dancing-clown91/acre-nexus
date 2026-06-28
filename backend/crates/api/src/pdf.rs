//! Real **PDF generation** for lease contracts and tenant letters.
//!
//! Built on [`printpdf`] using the standard Helvetica font (no font files to
//! ship) with a small word-wrapping layout engine and automatic page breaks. The
//! LLC's logo (when present) is embedded at the top of page one; embedding is
//! best-effort, so a bad/unsupported image never blocks document generation.

use anyhow::Context;
use printpdf::{
    BuiltinFont, Image, ImageTransform, IndirectFontRef, Mm, PdfDocument, PdfDocumentReference,
    PdfLayerIndex, PdfLayerReference, PdfPageIndex,
};

// US Letter, all dimensions in millimetres.
const PAGE_W: f32 = 215.9;
const PAGE_H: f32 = 279.4;
const M_LEFT: f32 = 20.0;
const M_TOP: f32 = 22.0;
const M_BOTTOM: f32 = 20.0;
const USABLE_W: f32 = PAGE_W - 2.0 * M_LEFT;
const PT_TO_MM: f32 = 0.352_777_8;

/// What to render into a document. `body` is already template-merged plain text.
pub struct DocSpec {
    pub title: String,
    pub logo: Option<Vec<u8>>,
    /// Small lines above the title (company letterhead / address).
    pub letterhead: Option<String>,
    pub body: String,
    pub signature_block: Option<String>,
    /// Fine print rendered at the end.
    pub footer: Option<String>,
}

/// Render `spec` to PDF bytes.
pub fn render(spec: &DocSpec) -> anyhow::Result<Vec<u8>> {
    let mut p = Painter::new(&spec.title)?;

    if let Some(logo) = &spec.logo {
        // Best-effort: log and continue if the image can't be embedded.
        if let Err(e) = p.place_logo(logo) {
            tracing::warn!("logo embed failed, rendering without it: {e:#}");
        }
    }

    if let Some(head) = spec.letterhead.as_deref().filter(|s| !s.trim().is_empty()) {
        p.paragraph(head, 9.5, false);
        p.gap(3.0);
    }

    p.paragraph(&spec.title, 16.0, true);
    p.gap(4.0);

    p.paragraph(&spec.body, 10.5, false);

    if let Some(sig) = spec.signature_block.as_deref().filter(|s| !s.trim().is_empty()) {
        p.gap(10.0);
        p.paragraph(sig, 10.5, false);
    }

    if let Some(footer) = spec.footer.as_deref().filter(|s| !s.trim().is_empty()) {
        p.gap(8.0);
        p.paragraph(footer, 8.0, false);
    }

    p.doc.save_to_bytes().context("pdf serialize failed")
}

struct Painter {
    doc: PdfDocumentReference,
    font: IndirectFontRef,
    bold: IndirectFontRef,
    page: PdfPageIndex,
    layer: PdfLayerIndex,
    /// Current baseline, in mm from the bottom of the page.
    y: f32,
}

impl Painter {
    fn new(title: &str) -> anyhow::Result<Self> {
        let (doc, page, layer) = PdfDocument::new(title, Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
        let font = doc.add_builtin_font(BuiltinFont::Helvetica)?;
        let bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;
        Ok(Painter {
            doc,
            font,
            bold,
            page,
            layer,
            y: PAGE_H - M_TOP,
        })
    }

    fn layer_ref(&self) -> PdfLayerReference {
        self.doc.get_page(self.page).get_layer(self.layer)
    }

    fn new_page(&mut self) {
        let (page, layer) = self.doc.add_page(Mm(PAGE_W), Mm(PAGE_H), "Layer 1");
        self.page = page;
        self.layer = layer;
        self.y = PAGE_H - M_TOP;
    }

    /// Ensure `needed` mm of vertical space remains, else start a new page.
    fn ensure(&mut self, needed: f32) {
        if self.y - needed < M_BOTTOM {
            self.new_page();
        }
    }

    fn gap(&mut self, h: f32) {
        self.ensure(h);
        self.y -= h;
    }

    fn line(&mut self, text: &str, size: f32, bold: bool) {
        let lh = size * PT_TO_MM * 1.35;
        self.ensure(lh);
        let font = if bold { &self.bold } else { &self.font };
        self.layer_ref()
            .use_text(text, size, Mm(M_LEFT), Mm(self.y), font);
        self.y -= lh;
    }

    /// Render a block of text, honouring explicit newlines and wrapping long lines.
    fn paragraph(&mut self, text: &str, size: f32, bold: bool) {
        let max = max_chars(size);
        for raw in text.split('\n') {
            if raw.trim().is_empty() {
                self.gap(size * PT_TO_MM);
                continue;
            }
            for wrapped in wrap(raw, max) {
                self.line(&wrapped, size, bold);
            }
        }
    }

    fn place_logo(&mut self, bytes: &[u8]) -> anyhow::Result<()> {
        let dynimg = image::load_from_memory(bytes).context("decode logo image")?;
        let (w, h) = (dynimg.width() as f32, dynimg.height() as f32);
        if w <= 0.0 || h <= 0.0 {
            anyhow::bail!("logo has zero dimension");
        }
        let dpi = 300.0_f32;
        let nat_w_mm = w / dpi * 25.4;
        let nat_h_mm = h / dpi * 25.4;
        let target_w = 45.0_f32.min(nat_w_mm).max(10.0);
        let scale = target_w / nat_w_mm;
        let draw_h = nat_h_mm * scale;
        let top_y = self.y - draw_h;
        let img = Image::from_dynamic_image(&dynimg);
        img.add_to_layer(
            self.layer_ref(),
            ImageTransform {
                translate_x: Some(Mm(M_LEFT)),
                translate_y: Some(Mm(top_y.max(M_BOTTOM))),
                scale_x: Some(scale),
                scale_y: Some(scale),
                dpi: Some(dpi),
                rotate: None,
            },
        );
        self.y = top_y - 5.0;
        Ok(())
    }
}

/// Approximate Helvetica characters that fit on one line at `size` points.
fn max_chars(size: f32) -> usize {
    let char_w_mm = 0.5 * size * PT_TO_MM;
    ((USABLE_W / char_w_mm).floor() as usize).max(20)
}

/// Greedy word wrap into lines of at most `max` characters.
fn wrap(line: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for word in line.split_whitespace() {
        if cur.is_empty() {
            cur.push_str(word);
        } else if cur.chars().count() + 1 + word.chars().count() <= max {
            cur.push(' ');
            cur.push_str(word);
        } else {
            out.push(std::mem::take(&mut cur));
            cur.push_str(word);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_valid_multipage_pdf() {
        // A long body forces wrapping + at least one page break.
        let body = (0..120)
            .map(|i| format!("Clause {i}: the tenant agrees to the foregoing terms and conditions set forth herein."))
            .collect::<Vec<_>>()
            .join("\n\n");
        let bytes = render(&DocSpec {
            title: "Residential Lease Agreement".into(),
            logo: None,
            letterhead: Some("Maple Holdings LLC\n123 Main St, Austin TX".into()),
            body,
            signature_block: Some("_____________________\nJane Doe, Managing Member".into()),
            footer: Some("Confidential — generated by Acre Nexus.".into()),
        })
        .expect("render should succeed");
        assert!(bytes.starts_with(b"%PDF"), "output must be a PDF");
        assert!(bytes.len() > 1500, "non-trivial document expected");
    }

    #[test]
    fn wrap_respects_width() {
        let lines = wrap(&"word ".repeat(40), 20);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| l.chars().count() <= 20));
    }
}
