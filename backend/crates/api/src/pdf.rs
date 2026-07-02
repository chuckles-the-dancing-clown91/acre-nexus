//! Minimal **text → PDF** writer for signed agreements.
//!
//! Produces a small, valid PDF 1.4 file (Letter pages, monospaced Courier)
//! directly — no external PDF crate, keeping the dependency rule the same way
//! [`crate::storage`] hand-rolls SigV4. The signed lease body plus its
//! signature certificate render as wrapped text pages; that is exactly what a
//! plain-text agreement needs, and every PDF reader can open it.

/// Page geometry (US Letter, 1" ≈ 72pt margins at 3/4").
const PAGE_W: f32 = 612.0;
const PAGE_H: f32 = 792.0;
const MARGIN: f32 = 54.0;
const FONT_SIZE: f32 = 10.0;
const LEADING: f32 = 12.5;
/// Courier glyphs are 0.6 em wide → usable columns per line.
const COLS: usize = ((PAGE_W - 2.0 * MARGIN) / (FONT_SIZE * 0.6)) as usize;

/// Lines that fit on one page.
fn lines_per_page() -> usize {
    ((PAGE_H - 2.0 * MARGIN) / LEADING) as usize
}

/// Escape a line for a PDF literal string and coerce it to the ASCII subset
/// Courier/WinAnsi renders predictably (common typographic characters get a
/// readable fallback; anything else becomes `?`).
fn escape_pdf_text(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    for ch in line.chars() {
        match ch {
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\\' => out.push_str("\\\\"),
            '•' => out.push('-'),
            '·' => out.push('-'),
            '—' | '–' => out.push('-'),
            '‘' | '’' => out.push('\''),
            '“' | '”' => out.push('"'),
            '→' => out.push_str("->"),
            '…' => out.push_str("..."),
            c if c.is_ascii() && !c.is_ascii_control() => out.push(c),
            '\t' => out.push_str("    "),
            _ => out.push('?'),
        }
    }
    out
}

/// Wrap `text` to the page's column budget, preserving existing newlines and
/// breaking on whitespace where possible.
pub fn wrap_text(text: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for raw in text.lines() {
        if raw.chars().count() <= COLS {
            lines.push(raw.to_string());
            continue;
        }
        let mut current = String::new();
        for word in raw.split(' ') {
            let candidate_len = if current.is_empty() {
                word.chars().count()
            } else {
                current.chars().count() + 1 + word.chars().count()
            };
            if candidate_len > COLS && !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            // A single overlong word hard-breaks at the column budget.
            let mut w = word;
            while w.chars().count() > COLS {
                let split_at = w.char_indices().nth(COLS).map(|(i, _)| i).unwrap_or(0);
                let (head, tail) = w.split_at(split_at);
                lines.push(head.to_string());
                w = tail;
            }
            if current.is_empty() {
                current = w.to_string();
            } else {
                current.push(' ');
                current.push_str(w);
            }
        }
        lines.push(current);
    }
    lines
}

/// Render `text` (pre-wrapping happens here) as a multi-page PDF.
pub fn text_to_pdf(text: &str) -> Vec<u8> {
    let lines = wrap_text(text);
    let per_page = lines_per_page().max(1);
    let pages: Vec<&[String]> = if lines.is_empty() {
        vec![&[]]
    } else {
        lines.chunks(per_page).collect()
    };

    // Object layout: 1 = catalog, 2 = pages root, 3 = font, then for each page
    // N: object (4 + 2i) = page, (5 + 2i) = its content stream.
    let mut objects: Vec<Vec<u8>> = Vec::new();
    let n_pages = pages.len();
    let kids: Vec<String> = (0..n_pages).map(|i| format!("{} 0 R", 4 + 2 * i)).collect();

    objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    objects.push(
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            n_pages
        )
        .into_bytes(),
    );
    objects.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>".to_vec());

    for (i, page_lines) in pages.iter().enumerate() {
        let content_ref = 5 + 2 * i;
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] \
                 /Resources << /Font << /F1 3 0 R >> >> /Contents {content_ref} 0 R >>"
            )
            .into_bytes(),
        );

        let mut stream = String::new();
        stream.push_str(&format!(
            "BT\n/F1 {FONT_SIZE} Tf\n{LEADING} TL\n{MARGIN} {} Td\n",
            PAGE_H - MARGIN - FONT_SIZE
        ));
        for line in page_lines.iter() {
            stream.push_str(&format!("({}) Tj\nT*\n", escape_pdf_text(line)));
        }
        stream.push_str("ET\n");
        let mut content =
            format!("<< /Length {} >>\nstream\n{stream}endstream", stream.len()).into_bytes();
        objects.push(std::mem::take(&mut content));
    }

    // Assemble the file with a correct xref table.
    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets: Vec<usize> = Vec::with_capacity(objects.len());
    for (i, obj) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(obj);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref_at = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_at}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_a_structurally_valid_pdf() {
        let pdf = text_to_pdf("Hello, world.\nSecond line.");
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.contains("/Type /Catalog"));
        assert!(s.contains("/BaseFont /Courier"));
        assert!(s.contains("(Hello, world.) Tj"));
        assert!(s.trim_end().ends_with("%%EOF"));

        // The xref offset in the trailer points at the actual xref table.
        let startxref: usize = s
            .split("startxref\n")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(&pdf[startxref..startxref + 4], b"xref");
    }

    #[test]
    fn long_documents_paginate() {
        let text = (0..200)
            .map(|i| format!("Line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let pdf = text_to_pdf(&text);
        let s = String::from_utf8_lossy(&pdf);
        let page_count = s.matches("/Type /Page ").count();
        assert!(page_count >= 4, "expected multiple pages, got {page_count}");
        assert!(s.contains(&format!("/Count {page_count}")));
    }

    #[test]
    fn escapes_and_transliterates() {
        assert_eq!(escape_pdf_text(r"a(b)c\d"), r"a\(b\)c\\d");
        assert_eq!(escape_pdf_text("• item — done"), "- item - done");
        assert_eq!(escape_pdf_text("naïve"), "na?ve");
    }

    #[test]
    fn wraps_long_lines_and_overlong_words() {
        let long = "word ".repeat(50);
        for line in wrap_text(&long) {
            assert!(line.chars().count() <= COLS);
        }
        let solid = "x".repeat(COLS * 2 + 5);
        let wrapped = wrap_text(&solid);
        assert_eq!(wrapped.len(), 3);
        assert!(wrapped.iter().all(|l| l.chars().count() <= COLS));
    }
}
