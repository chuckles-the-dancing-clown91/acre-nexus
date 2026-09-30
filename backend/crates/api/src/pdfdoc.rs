//! **Document PDFs** — reports, payroll weeks, cost sheets and owner bills
//! that print like documents, not text dumps.
//!
//! Hand-rolled PDF 1.4 (the same no-dependency rule as [`crate::pdf`]) using
//! the two standard Helvetica faces every reader ships, with their real glyph
//! widths so columns size, truncate and right-align properly. A document is a
//! title block and a list of [`Block`]s — headings, paragraphs, key/value
//! summaries and tables (header row repeated on every page, zebra rows,
//! right-aligned money, an emphasised totals row). Wide tables get a landscape
//! page. Every page carries the workspace name, "Page n of N" and the date.

/// Helvetica advance widths (1/1000 em) for ASCII 32..=126.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

/// Helvetica-Bold advance widths for ASCII 32..=126.
const HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

/// One character in WinAnsi, with its width in each face.
fn glyph(c: char) -> (u8, u16, u16) {
    let code = c as u32;
    if (32..=126).contains(&code) {
        let i = (code - 32) as usize;
        return (code as u8, HELVETICA[i], HELVETICA_BOLD[i]);
    }
    match c {
        '—' => (0x97, 1000, 1000),
        '–' => (0x96, 556, 556),
        '…' => (0x85, 1000, 1000),
        '‘' => (0x91, 222, 278),
        '’' => (0x92, 222, 278),
        '“' => (0x93, 333, 500),
        '”' => (0x94, 333, 500),
        '•' => (0x95, 350, 350),
        '·' => (0xB7, 278, 278),
        '×' => (0xD7, 584, 584),
        '°' => (0xB0, 400, 400),
        '½' => (0xBD, 834, 834),
        '€' => (0x80, 556, 556),
        '£' => (0xA3, 556, 556),
        '©' => (0xA9, 737, 737),
        '\u{a0}' => (0x20, 278, 278),
        // Latin-1 letters keep their byte; width of a typical lowercase.
        _ if (0xC0..=0xFF).contains(&code) => (code as u8, 556, 611),
        _ => (b'?', 556, 611),
    }
}

/// Text width in points at `size`.
pub fn text_width(s: &str, size: f64, bold: bool) -> f64 {
    s.chars()
        .map(|c| {
            let (_, r, b) = glyph(c);
            if bold {
                b
            } else {
                r
            }
        })
        .map(|w| w as f64)
        .sum::<f64>()
        * size
        / 1000.0
}

/// Encode a string as a PDF literal (WinAnsi bytes, escaped).
fn literal(s: &str) -> String {
    let mut out = String::from("(");
    for c in s.chars() {
        let (b, _, _) = glyph(c);
        match b {
            b'(' | b')' | b'\\' => {
                out.push('\\');
                out.push(b as char);
            }
            32..=126 => out.push(b as char),
            _ => out.push_str(&format!("\\{:03o}", b)),
        }
    }
    out.push(')');
    out
}

/// Cut `s` to fit `max` points, with an ellipsis.
pub fn fit(s: &str, max: f64, size: f64, bold: bool) -> String {
    if text_width(s, size, bold) <= max {
        return s.to_string();
    }
    let ell = text_width("…", size, bold);
    let mut out = String::new();
    let mut w = 0.0;
    for c in s.chars() {
        let cw = text_width(&c.to_string(), size, bold);
        if w + cw + ell > max {
            break;
        }
        out.push(c);
        w += cw;
    }
    out.push('…');
    out
}

/// Word-wrap `s` into lines no wider than `max` points.
pub fn wrap(s: &str, max: f64, size: f64, bold: bool) -> Vec<String> {
    let mut lines = Vec::new();
    for para in s.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if text_width(&candidate, size, bold) <= max || line.is_empty() {
                line = if text_width(&candidate, size, bold) > max {
                    fit(&candidate, max, size, bold)
                } else {
                    candidate
                };
            } else {
                lines.push(std::mem::take(&mut line));
                line = fit(word, max, size, bold);
            }
        }
        lines.push(line);
    }
    lines
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Right,
}

#[derive(Debug, Clone)]
pub struct Column {
    pub label: String,
    /// Relative width; the table fills the page width.
    pub weight: f64,
    pub align: Align,
}

impl Column {
    pub fn left(label: &str, weight: f64) -> Column {
        Column {
            label: label.into(),
            weight,
            align: Align::Left,
        }
    }
    pub fn right(label: &str, weight: f64) -> Column {
        Column {
            label: label.into(),
            weight,
            align: Align::Right,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Table {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<String>>,
    pub totals: Option<Vec<String>>,
}

impl Table {
    /// A table from plain headers, right-aligning columns whose cells look
    /// like numbers or money, and weighting widths by content length.
    pub fn auto(headers: &[String], rows: Vec<Vec<String>>, totals: Option<Vec<String>>) -> Table {
        let columns = headers
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let cells: Vec<&String> = rows.iter().filter_map(|r| r.get(i)).collect();
                let numeric = !cells.is_empty()
                    && cells
                        .iter()
                        .filter(|c| !c.trim().is_empty())
                        .all(|c| looks_numeric(c));
                let longest = cells
                    .iter()
                    .map(|c| c.chars().count())
                    .max()
                    .unwrap_or(0)
                    .max(h.chars().count())
                    .clamp(4, 40);
                Column {
                    label: h.clone(),
                    weight: longest as f64,
                    align: if numeric { Align::Right } else { Align::Left },
                }
            })
            .collect();
        Table {
            columns,
            rows,
            totals,
        }
    }
}

fn looks_numeric(s: &str) -> bool {
    let t = s.trim();
    !t.is_empty()
        && t.chars()
            .all(|c| c.is_ascii_digit() || "$,.-%:()+× h".contains(c))
        && t.chars().any(|c| c.is_ascii_digit())
}

#[derive(Debug, Clone)]
pub enum Block {
    Heading(String),
    Paragraph(String),
    /// A two-column summary (label, value).
    KeyValues(Vec<(String, String)>),
    Table(Table),
}

#[derive(Debug, Clone, Default)]
pub struct Document {
    pub title: String,
    pub subtitle: Option<String>,
    /// The workspace, printed on every page.
    pub organization: String,
    pub landscape: bool,
    pub blocks: Vec<Block>,
}

const MARGIN: f64 = 42.0;
const BODY: f64 = 9.0;
const ROW_H: f64 = 15.0;

struct Page {
    ops: String,
}

struct Writer {
    w: f64,
    h: f64,
    pages: Vec<Page>,
    y: f64,
    org: String,
    title: String,
}

impl Writer {
    fn new(doc: &Document) -> Writer {
        let (w, h) = if doc.landscape {
            (792.0, 612.0)
        } else {
            (612.0, 792.0)
        };
        let mut wr = Writer {
            w,
            h,
            pages: Vec::new(),
            y: 0.0,
            org: doc.organization.clone(),
            title: doc.title.clone(),
        };
        wr.new_page();
        wr
    }

    fn cur(&mut self) -> &mut String {
        &mut self.pages.last_mut().expect("a page").ops
    }

    fn new_page(&mut self) {
        self.pages.push(Page { ops: String::new() });
        // Running header: the organisation and the document title.
        let (org, title, w, h) = (self.org.clone(), self.title.clone(), self.w, self.h);
        self.text(MARGIN, h - 28.0, 8.0, false, (0.45, 0.45, 0.45), &org);
        let tw = text_width(&title, 8.0, false);
        let t = fit(&title, w / 2.0, 8.0, false);
        self.text(
            w - MARGIN - tw.min(w / 2.0),
            h - 28.0,
            8.0,
            false,
            (0.45, 0.45, 0.45),
            &t,
        );
        self.line(
            MARGIN,
            h - 34.0,
            w - MARGIN,
            h - 34.0,
            0.5,
            (0.85, 0.85, 0.85),
        );
        self.y = h - 52.0;
    }

    fn ensure(&mut self, needed: f64) -> bool {
        if self.y - needed < MARGIN + 24.0 {
            self.new_page();
            true
        } else {
            false
        }
    }

    fn text(&mut self, x: f64, y: f64, size: f64, bold: bool, rgb: (f64, f64, f64), s: &str) {
        let font = if bold { "F2" } else { "F1" };
        let op = format!(
            "BT {:.3} {:.3} {:.3} rg /{font} {size:.1} Tf {x:.2} {y:.2} Td {} Tj ET\n",
            rgb.0,
            rgb.1,
            rgb.2,
            literal(s)
        );
        self.cur().push_str(&op);
    }

    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, rgb: (f64, f64, f64)) {
        let op = format!(
            "{:.3} {:.3} {:.3} rg {x:.2} {y:.2} {w:.2} {h:.2} re f\n",
            rgb.0, rgb.1, rgb.2
        );
        self.cur().push_str(&op);
    }

    fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, width: f64, rgb: (f64, f64, f64)) {
        let op = format!(
            "{:.3} {:.3} {:.3} RG {width:.2} w {x1:.2} {y1:.2} m {x2:.2} {y2:.2} l S\n",
            rgb.0, rgb.1, rgb.2
        );
        self.cur().push_str(&op);
    }

    fn content_width(&self) -> f64 {
        self.w - 2.0 * MARGIN
    }

    fn title_block(&mut self, doc: &Document) {
        let lines = wrap(&doc.title, self.content_width(), 18.0, true);
        for l in lines {
            self.ensure(24.0);
            self.text(MARGIN, self.y, 18.0, true, (0.1, 0.09, 0.06), &l);
            self.y -= 22.0;
        }
        if let Some(sub) = &doc.subtitle {
            for l in wrap(sub, self.content_width(), 10.0, false) {
                self.text(MARGIN, self.y, 10.0, false, (0.37, 0.35, 0.29), &l);
                self.y -= 14.0;
            }
        }
        // The Vantedge teal rule under the title.
        self.rect(MARGIN, self.y + 2.0, 48.0, 2.5, (0.055, 0.486, 0.525));
        self.y -= 16.0;
    }

    fn heading(&mut self, s: &str) {
        self.ensure(40.0);
        self.y -= 6.0;
        self.text(MARGIN, self.y, 12.0, true, (0.1, 0.09, 0.06), s);
        self.y -= 16.0;
    }

    fn paragraph(&mut self, s: &str) {
        for l in wrap(s, self.content_width(), BODY + 0.5, false) {
            self.ensure(14.0);
            self.text(MARGIN, self.y, BODY + 0.5, false, (0.2, 0.2, 0.2), &l);
            self.y -= 13.0;
        }
        self.y -= 4.0;
    }

    fn key_values(&mut self, kv: &[(String, String)]) {
        let label_w = kv
            .iter()
            .map(|(k, _)| text_width(k, BODY, false))
            .fold(0.0, f64::max)
            .min(self.content_width() * 0.45)
            + 18.0;
        for (k, v) in kv {
            self.ensure(ROW_H);
            let k = fit(k, label_w - 18.0, BODY, false);
            self.text(MARGIN, self.y, BODY, false, (0.37, 0.35, 0.29), &k);
            let v = fit(v, self.content_width() - label_w, BODY, true);
            self.text(MARGIN + label_w, self.y, BODY, true, (0.1, 0.09, 0.06), &v);
            self.y -= ROW_H - 2.0;
        }
        self.y -= 6.0;
    }

    fn table(&mut self, t: &Table) {
        if t.columns.is_empty() {
            return;
        }
        let total_w = self.content_width();
        let weights: f64 = t.columns.iter().map(|c| c.weight.max(1.0)).sum();
        let widths: Vec<f64> = t
            .columns
            .iter()
            .map(|c| total_w * c.weight.max(1.0) / weights)
            .collect();
        let size = if t.columns.len() > 10 { 7.0 } else { 8.0 };
        let pad = 4.0;
        let header = |wr: &mut Writer| {
            wr.ensure(ROW_H * 2.0);
            wr.rect(MARGIN, wr.y - 4.0, total_w, ROW_H, (0.93, 0.95, 0.95));
            let mut x = MARGIN;
            for (c, w) in t.columns.iter().zip(&widths) {
                let s = fit(&c.label, w - 2.0 * pad, size, true);
                let tx = match c.align {
                    Align::Left => x + pad,
                    Align::Right => x + w - pad - text_width(&s, size, true),
                };
                wr.text(tx, wr.y, size, true, (0.2, 0.2, 0.2), &s);
                x += w;
            }
            wr.y -= ROW_H;
        };
        header(self);
        let draw_row = |wr: &mut Writer, row: &[String], bold: bool, shade: bool| {
            if wr.ensure(ROW_H) {
                header(wr);
            }
            if shade {
                wr.rect(MARGIN, wr.y - 4.0, total_w, ROW_H, (0.975, 0.97, 0.955));
            }
            let mut x = MARGIN;
            for (i, (c, w)) in t.columns.iter().zip(&widths).enumerate() {
                let cell = row.get(i).map(String::as_str).unwrap_or("");
                let s = fit(cell, w - 2.0 * pad, size, bold);
                let tx = match c.align {
                    Align::Left => x + pad,
                    Align::Right => x + w - pad - text_width(&s, size, bold),
                };
                wr.text(tx, wr.y, size, bold, (0.1, 0.09, 0.06), &s);
                x += w;
            }
            wr.y -= ROW_H;
        };
        for (i, row) in t.rows.iter().enumerate() {
            draw_row(self, row, false, i % 2 == 1);
        }
        if t.rows.is_empty() {
            self.ensure(ROW_H);
            self.text(
                MARGIN + pad,
                self.y,
                size,
                false,
                (0.45, 0.45, 0.45),
                "Nothing in this period.",
            );
            self.y -= ROW_H;
        }
        if let Some(tot) = &t.totals {
            self.ensure(ROW_H + 2.0);
            self.line(
                MARGIN,
                self.y + ROW_H - 3.0,
                MARGIN + total_w,
                self.y + ROW_H - 3.0,
                0.8,
                (0.3, 0.3, 0.3),
            );
            draw_row(self, tot, true, false);
        }
        self.y -= 8.0;
    }

    fn finish(self, generated: &str) -> Vec<u8> {
        let n = self.pages.len();
        let mut objects: Vec<String> = Vec::new();
        // 1 catalog, 2 pages, 3 F1, 4 F2, then page+content pairs.
        objects.push("<< /Type /Catalog /Pages 2 0 R >>".into());
        let kids: Vec<String> = (0..n).map(|i| format!("{} 0 R", 5 + i * 2)).collect();
        objects.push(format!(
            "<< /Type /Pages /Kids [{}] /Count {n} >>",
            kids.join(" ")
        ));
        objects.push(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
                .into(),
        );
        objects.push(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
                .into(),
        );
        let (w, h) = (self.w, self.h);
        for (i, page) in self.pages.iter().enumerate() {
            let mut ops = page.ops.clone();
            // Footer.
            let foot = format!("Page {} of {n}", i + 1);
            let fw = text_width(&foot, 7.5, false);
            ops.push_str(&format!(
                "BT 0.55 0.55 0.55 rg /F1 7.5 Tf {:.2} 24 Td {} Tj ET\n",
                w - MARGIN - fw,
                literal(&foot)
            ));
            ops.push_str(&format!(
                "BT 0.55 0.55 0.55 rg /F1 7.5 Tf {MARGIN:.2} 24 Td {} Tj ET\n",
                literal(&format!("Generated {generated} · Vantedge"))
            ));
            objects.push(format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {w:.0} {h:.0}] \
                 /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
                6 + i * 2
            ));
            objects.push(format!(
                "<< /Length {} >>\nstream\n{ops}endstream",
                ops.len()
            ));
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
        let mut offsets = Vec::with_capacity(objects.len());
        for (i, o) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
        }
        let xref = out.len();
        out.extend_from_slice(
            format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes(),
        );
        for off in offsets {
            out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        out.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        out
    }
}

/// Render a document to PDF bytes.
pub fn render(doc: &Document) -> Vec<u8> {
    let mut wr = Writer::new(doc);
    wr.title_block(doc);
    for b in &doc.blocks {
        match b {
            Block::Heading(s) => wr.heading(s),
            Block::Paragraph(s) => wr.paragraph(s),
            Block::KeyValues(kv) => wr.key_values(kv),
            Block::Table(t) => wr.table(t),
        }
    }
    let generated = chrono::Utc::now().format("%b %-d, %Y").to_string();
    wr.finish(&generated)
}

/// `$1,234.56` from cents.
pub fn money(cents: i64) -> String {
    let neg = cents < 0;
    let c = cents.unsigned_abs();
    let dollars = (c / 100).to_string();
    let mut grouped = String::new();
    for (i, ch) in dollars.chars().enumerate() {
        if i > 0 && (dollars.len() - i).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    format!("{}${grouped}.{:02}", if neg { "-" } else { "" }, c % 100)
}

/// `12:05` (hours:minutes) from minutes.
pub fn hm(minutes: i64) -> String {
    format!("{}:{:02}", minutes / 60, minutes.abs() % 60)
}

/// `42.5%` from basis points.
pub fn pct(bps: i64) -> String {
    format!("{:.1}%", bps as f64 / 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_and_fitting() {
        // "Hi" = H 722 + i 222 at 10pt = 9.44pt.
        assert!((text_width("Hi", 10.0, false) - 9.44).abs() < 1e-9);
        assert!(text_width("Hi", 10.0, true) > text_width("Hi", 10.0, false));
        let long = "A very long description of a work order that won't fit";
        let f = fit(long, 60.0, 8.0, false);
        assert!(f.ends_with('…'));
        assert!(text_width(&f, 8.0, false) <= 60.0);
        assert_eq!(fit("short", 60.0, 8.0, false), "short");
    }

    #[test]
    fn wrapping_keeps_words_and_width() {
        let lines = wrap("one two three four five six seven", 40.0, 10.0, false);
        assert!(lines.len() > 1);
        for l in &lines {
            assert!(text_width(l, 10.0, false) <= 40.0 + 1e-6, "{l}");
        }
    }

    #[test]
    fn literals_escape_and_encode() {
        assert_eq!(literal("a(b)c\\"), "(a\\(b\\)c\\\\)");
        assert_eq!(literal("—"), "(\\227)");
        assert_eq!(literal("日"), "(?)");
    }

    #[test]
    fn money_and_hours() {
        assert_eq!(money(123_456_789), "$1,234,567.89");
        assert_eq!(money(-500), "-$5.00");
        assert_eq!(money(0), "$0.00");
        assert_eq!(hm(785), "13:05");
        assert_eq!(pct(4512), "45.1%");
    }

    #[test]
    fn auto_table_aligns_numbers_right() {
        let t = Table::auto(
            &["Name".into(), "Hours".into(), "Pay".into()],
            vec![vec!["Morgan".into(), "13:00".into(), "$400.00".into()]],
            None,
        );
        assert_eq!(t.columns[0].align, Align::Left);
        assert_eq!(t.columns[1].align, Align::Right);
        assert_eq!(t.columns[2].align, Align::Right);
    }

    #[test]
    fn renders_a_valid_multi_page_pdf() {
        let rows: Vec<Vec<String>> = (0..120)
            .map(|i| vec![format!("Row {i} — with a dash"), money(i * 1234), hm(i * 7)])
            .collect();
        let doc = Document {
            title: "Payroll — week of Sep 14".into(),
            subtitle: Some("California daily overtime".into()),
            organization: "Northwind Property Group".into(),
            landscape: false,
            blocks: vec![
                Block::KeyValues(vec![("Gross".into(), money(123_400))]),
                Block::Table(Table::auto(
                    &["Who".into(), "Gross".into(), "Hours".into()],
                    rows,
                    Some(vec!["Total".into(), money(1), hm(1)]),
                )),
            ],
        };
        let pdf = render(&doc);
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.starts_with("%PDF-1.4"));
        assert!(s.trim_end().ends_with("%%EOF"));
        assert!(s.contains("/Helvetica-Bold"));
        assert!(s.contains("Page 1 of"));
        let pages = s.matches("/Type /Page ").count();
        assert!(pages >= 3, "120 rows span pages: {pages}");
        // The xref offset points at the xref table (checked on the raw bytes).
        let tail = pdf
            .windows(10)
            .rposition(|w| w == b"startxref\n")
            .expect("startxref");
        let digits: String = pdf[tail + 10..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .map(|b| *b as char)
            .collect();
        let xref_at: usize = digits.parse().unwrap();
        assert!(pdf[xref_at..].starts_with(b"xref"));
    }
}
