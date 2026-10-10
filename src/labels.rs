//! A4 label sheets as a PDF, written by hand with no extra crate: barcode
//! bars are filled rectangles, text uses the built-in Helvetica font.
//!
//! The layout constants below can be tweaked freely; the tests check that a
//! barcode still fits in a cell.

use std::fmt::Write as _;

use crate::code128;

// --------------------------------------------------------- layout (in mm)

pub const PAGE_WIDTH: f32 = 210.0;
pub const PAGE_HEIGHT: f32 = 297.0;
/// Blank border around the grid (printers cannot print up to the edge).
pub const MARGIN: f32 = 8.0;
pub const COLUMNS: usize = 3;
pub const ROWS: usize = 8;
pub const LABELS_PER_PAGE: usize = COLUMNS * ROWS;
/// Width of the narrowest bar. 0.4 mm scans easily from a laser print.
pub const MODULE: f32 = 0.4;
pub const BAR_HEIGHT: f32 = 13.0;
/// Vertical gap between the caption, the bars and the code.
pub const GAP: f32 = 1.5;
/// Horizontal room kept free on each side of the caption.
pub const TEXT_PADDING: f32 = 3.0;
/// Font sizes, in points.
pub const CAPTION_SIZE: f32 = 8.0;
pub const CODE_SIZE: f32 = 11.0;
/// Cut lines: grey level (0 = black, 1 = white) and width in points.
pub const CUT_LINE_GREY: f32 = 0.6;
pub const CUT_LINE_WIDTH: f32 = 0.3;

const PT_PER_MM: f32 = 72.0 / 25.4;
const CELL_WIDTH: f32 = (PAGE_WIDTH - 2.0 * MARGIN) / COLUMNS as f32;
const CELL_HEIGHT: f32 = (PAGE_HEIGHT - 2.0 * MARGIN) / ROWS as f32;

pub struct Label {
    /// Encoded in the barcode and printed in clear under it.
    pub code: String,
    /// Printed above the barcode (e.g. the book title), shortened if too long.
    pub caption: String,
}

/// Builds the whole PDF file. Labels fill the grid row by row; a new page
/// starts every `LABELS_PER_PAGE` labels.
pub fn sheet_pdf(labels: &[Label]) -> Vec<u8> {
    let pages: Vec<&[Label]> = if labels.is_empty() {
        vec![&[]]
    } else {
        labels.chunks(LABELS_PER_PAGE).collect()
    };

    // Objects: 1 catalog, 2 page tree, 3 font, then a (page, content) pair per page.
    let mut pdf = PdfWriter::new();
    pdf.object(b"<< /Type /Catalog /Pages 2 0 R >>");
    let kids: Vec<String> = (0..pages.len())
        .map(|i| format!("{} 0 R", 4 + 2 * i))
        .collect();
    pdf.object(
        format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            pages.len()
        )
        .as_bytes(),
    );
    pdf.object(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    );
    for (i, page) in pages.iter().enumerate() {
        let content_id = 5 + 2 * i;
        pdf.object(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] \
                 /Resources << /Font << /F1 3 0 R >> >> /Contents {content_id} 0 R >>",
                PAGE_WIDTH * PT_PER_MM,
                PAGE_HEIGHT * PT_PER_MM
            )
            .as_bytes(),
        );
        let content = page_content(page);
        let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
        stream.extend_from_slice(&content);
        stream.extend_from_slice(b"\nendstream");
        pdf.object(&stream);
    }
    pdf.finish()
}

/// Minimal PDF file writer: numbers objects in order and builds the xref table.
struct PdfWriter {
    out: Vec<u8>,
    offsets: Vec<usize>,
}

impl PdfWriter {
    fn new() -> PdfWriter {
        // The binary comment tells tools the file contains 8-bit data.
        PdfWriter {
            out: b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec(),
            offsets: Vec::new(),
        }
    }

    fn object(&mut self, body: &[u8]) {
        self.offsets.push(self.out.len());
        self.out
            .extend_from_slice(format!("{} 0 obj\n", self.offsets.len()).as_bytes());
        self.out.extend_from_slice(body);
        self.out.extend_from_slice(b"\nendobj\n");
    }

    fn finish(mut self) -> Vec<u8> {
        let xref = self.out.len();
        let mut tail = format!("xref\n0 {}\n0000000000 65535 f \n", self.offsets.len() + 1);
        for off in &self.offsets {
            let _ = writeln!(tail, "{off:010} 00000 n ");
        }
        let _ = write!(
            tail,
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            self.offsets.len() + 1
        );
        self.out.extend_from_slice(tail.as_bytes());
        self.out
    }
}

/// Drawing commands for one page. PDF units are points, origin bottom-left.
fn page_content(labels: &[Label]) -> Vec<u8> {
    let pt = |mm: f32| mm * PT_PER_MM;
    let mut s = String::new();

    // Cut lines: the outline of every cell.
    let (left, right) = (MARGIN, PAGE_WIDTH - MARGIN);
    let (bottom, top) = (MARGIN, PAGE_HEIGHT - MARGIN);
    let _ = writeln!(s, "{CUT_LINE_GREY} G {CUT_LINE_WIDTH} w");
    for c in 0..=COLUMNS {
        let x = pt(left + c as f32 * CELL_WIDTH);
        let _ = writeln!(s, "{x:.2} {:.2} m {x:.2} {:.2} l S", pt(bottom), pt(top));
    }
    for r in 0..=ROWS {
        let y = pt(bottom + r as f32 * CELL_HEIGHT);
        let _ = writeln!(s, "{:.2} {y:.2} m {:.2} {y:.2} l S", pt(left), pt(right));
    }
    let _ = writeln!(s, "0 g");

    let mut out = s.into_bytes();
    for (i, label) in labels.iter().enumerate() {
        let (row, col) = (i / COLUMNS, i % COLUMNS);
        let cell_left = MARGIN + col as f32 * CELL_WIDTH;
        let cell_top = PAGE_HEIGHT - MARGIN - row as f32 * CELL_HEIGHT;
        draw_label(&mut out, label, cell_left, cell_top);
    }
    out
}

/// Caption, bars, then the code in clear, centred as a block in the cell.
fn draw_label(out: &mut Vec<u8>, label: &Label, cell_left: f32, cell_top: f32) {
    let pt = |mm: f32| mm * PT_PER_MM;
    let caption_h = CAPTION_SIZE / PT_PER_MM;
    let code_h = CODE_SIZE / PT_PER_MM;
    let block = caption_h + GAP + BAR_HEIGHT + GAP + code_h * 0.75;
    let center_x = cell_left + CELL_WIDTH / 2.0;
    let mut y = cell_top - (CELL_HEIGHT - block) / 2.0;

    // Caption (baseline about 80% of the font size below the top).
    y -= caption_h * 0.8;
    let max_width = pt(CELL_WIDTH - 2.0 * TEXT_PADDING);
    let caption = fit_text(&win_ansi(&label.caption), CAPTION_SIZE, max_width);
    draw_text(out, &caption, CAPTION_SIZE, pt(center_x), pt(y));
    y -= caption_h * 0.2 + GAP;

    // Bars: one rectangle per run of consecutive bar modules.
    if let Some(modules) = code128::encode(&label.code) {
        let x0 = center_x - modules.len() as f32 * MODULE / 2.0;
        let mut s = String::new();
        let mut i = 0;
        while i < modules.len() {
            if modules[i] {
                let start = i;
                while i < modules.len() && modules[i] {
                    i += 1;
                }
                let _ = writeln!(
                    s,
                    "{:.3} {:.3} {:.3} {:.3} re",
                    pt(x0 + start as f32 * MODULE),
                    pt(y - BAR_HEIGHT),
                    pt((i - start) as f32 * MODULE),
                    pt(BAR_HEIGHT)
                );
            } else {
                i += 1;
            }
        }
        s.push_str("f\n");
        out.extend_from_slice(s.as_bytes());
    }
    y -= BAR_HEIGHT + GAP + code_h * 0.75;

    // Code in clear, so it can be typed by hand if the scanner fails.
    let code = fit_text(&win_ansi(&label.code), CODE_SIZE, max_width);
    draw_text(out, &code, CODE_SIZE, pt(center_x), pt(y));
}

/// Centred single line of Helvetica text. `text` is already WinAnsi-encoded.
fn draw_text(out: &mut Vec<u8>, text: &[u8], size: f32, center_x: f32, baseline: f32) {
    let x = center_x - text_width(text, size) / 2.0;
    out.extend_from_slice(format!("BT /F1 {size} Tf {x:.2} {baseline:.2} Td (").as_bytes());
    for &b in text {
        if matches!(b, b'(' | b')' | b'\\') {
            out.push(b'\\');
        }
        out.push(b);
    }
    out.extend_from_slice(b") Tj ET\n");
}

const ELLIPSIS: u8 = 0x85;

/// Shortens `text` with "…" so it fits in `max_width` points.
fn fit_text(text: &[u8], size: f32, max_width: f32) -> Vec<u8> {
    if text_width(text, size) <= max_width {
        return text.to_vec();
    }
    let mut out = text.to_vec();
    while !out.is_empty() && text_width(&out, size) + text_width(&[ELLIPSIS], size) > max_width {
        out.pop();
    }
    while out.last() == Some(&b' ') {
        out.pop();
    }
    out.push(ELLIPSIS);
    out
}

/// Text in the PDF's WinAnsi encoding (covers French); other characters become '?'.
fn win_ansi(s: &str) -> Vec<u8> {
    s.chars()
        .map(|c| match c {
            ' '..='~' | '\u{A0}'..='ÿ' => c as u32 as u8,
            '€' => 0x80,
            '…' => 0x85,
            'Œ' => 0x8C,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '–' => 0x96,
            '—' => 0x97,
            'œ' => 0x9C,
            'Ÿ' => 0x9F,
            _ => b'?',
        })
        .collect()
}

/// Width in points. Helvetica widths (1/1000 em) from the standard AFM file;
/// accented letters use an upper bound, which can only shorten a caption.
fn text_width(text: &[u8], size: f32) -> f32 {
    const ASCII: [u16; 95] = [
        278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
        278, // ' '..'/'
        556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584,
        556, // '0'..'?'
        1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833, 722,
        778, // '@'..'O'
        667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469,
        556, // 'P'..'_'
        333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500, 222, 833, 556,
        556, // '`'..'o'
        556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584, // 'p'..'~'
    ];
    let units: u32 = text
        .iter()
        .map(|&b| match b {
            32..=126 => ASCII[(b - 32) as usize] as u32,
            0xC6 | 0x8C | 0x85 | 0x97 => 1000, // Æ Œ … —
            0xE6 | 0x9C => 944,                // æ œ
            0xC0..=0xDE => 778,                // accented capitals
            0xDF..=0xFF => 556,                // accented lowercase
            _ => 1000,
        })
        .sum();
    units as f32 * size / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label(code: &str, caption: &str) -> Label {
        Label {
            code: code.into(),
            caption: caption.into(),
        }
    }

    fn contains(haystack: &[u8], needle: &[u8]) -> bool {
        find(haystack, needle).is_some()
    }

    #[test]
    fn barcode_fits_in_a_cell() {
        let modules = code128::encode("L999999").unwrap().len() + 2 * code128::QUIET_ZONE;
        assert!(modules as f32 * MODULE <= CELL_WIDTH);
        let caption_h = CAPTION_SIZE / PT_PER_MM;
        let code_h = CODE_SIZE / PT_PER_MM;
        assert!(caption_h + GAP + BAR_HEIGHT + GAP + code_h <= CELL_HEIGHT);
    }

    #[test]
    fn one_page_per_24_labels() {
        let count = |n: usize| {
            let labels: Vec<Label> = (1..=n).map(|i| label(&format!("L{i:06}"), "T")).collect();
            let pdf = sheet_pdf(&labels);
            String::from_utf8_lossy(&pdf)
                .matches("/Type /Page ")
                .count()
        };
        assert_eq!(count(0), 1);
        assert_eq!(count(1), 1);
        assert_eq!(count(24), 1);
        assert_eq!(count(25), 2);
        assert_eq!(count(49), 3);
    }

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }

    /// Parses the ASCII number at the start of `bytes`.
    fn number(bytes: &[u8]) -> usize {
        let digits: Vec<u8> = bytes
            .iter()
            .copied()
            .take_while(u8::is_ascii_digit)
            .collect();
        String::from_utf8(digits).unwrap().parse().unwrap()
    }

    #[test]
    fn xref_offsets_point_at_their_objects() {
        let pdf = sheet_pdf(&[label("L000001", "A"), label("L000002", "B")]);
        let tail = pdf.len() - 30;
        let start = number(&pdf[tail + find(&pdf[tail..], b"startxref\n").unwrap() + 10..]);
        assert!(pdf[start..].starts_with(b"xref\n0 6\n0000000000 65535 f \n"));
        // catalog, pages, font, page, content
        for i in 0..5 {
            // "xref\n0 6\n" is 9 bytes, then 20 bytes per entry (the free one first)
            let entry = &pdf[start + 29 + 20 * i..start + 29 + 20 * (i + 1)];
            assert!(entry.ends_with(b" 00000 n \n"));
            let off = number(entry);
            assert!(pdf[off..].starts_with(format!("{} 0 obj", i + 1).as_bytes()));
        }
        assert!(pdf.ends_with(b"%%EOF\n"));
    }

    #[test]
    fn stream_length_is_exact() {
        let pdf = sheet_pdf(&[label("L000001", "A")]);
        let len_at = find(&pdf, b"/Length ").unwrap() + 8;
        let len = number(&pdf[len_at..]);
        let data = len_at + find(&pdf[len_at..], b"stream\n").unwrap() + 7;
        assert_eq!(&pdf[data + len..data + len + 10], b"\nendstream");
    }

    #[test]
    fn text_is_encoded_and_escaped() {
        let pdf = sheet_pdf(&[label("L000001", "L’été (tome 1) \\ Cœur")]);
        assert!(contains(
            &pdf,
            b"(L\x92\xE9t\xE9 \\(tome 1\\) \\\\ C\x9Cur) Tj"
        ));
        assert!(contains(&pdf, b"(L000001) Tj"));
        assert_eq!(win_ansi("日本"), b"??");
    }

    #[test]
    fn long_captions_are_shortened() {
        let long = "Une très longue histoire de dragons, de chevaliers et de princesses";
        let max = 100.0;
        let fitted = fit_text(&win_ansi(long), CAPTION_SIZE, max);
        assert!(text_width(&fitted, CAPTION_SIZE) <= max);
        assert_eq!(fitted.last(), Some(&ELLIPSIS));
        assert_eq!(fit_text(b"Court", CAPTION_SIZE, max), b"Court");
    }
}
