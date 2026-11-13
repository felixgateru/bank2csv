//! PDF → positioned spans. Decryption is done by `lopdf` (via `pdf-extract`'s
//! re-export), which supports RC4, AESV2 and AESV3 standard security handlers.

use bank2csv_core::{Error, Page, Result, Span, SpanDocument};
use pdf_extract::{Document, LoadOptions, MediaBox, OutputDev, OutputError, Transform};

pub fn extract(bytes: &[u8], password: Option<&str>) -> Result<SpanDocument> {
    let probe = Document::load_mem(bytes).map_err(|e| Error::Corrupt(e.to_string()))?;
    let doc = if probe.is_encrypted() {
        // Objects inside compressed object streams can only be parsed after
        // decryption, so decrypt during load rather than calling `decrypt()` on a
        // document that was loaded without the key. Many statements use an empty
        // user password, so try that when none is given.
        let pw = password.unwrap_or("");
        let doc = Document::load_mem_with_options(bytes, LoadOptions::with_password(pw)).map_err(|e| {
            if e.to_string().to_ascii_lowercase().contains("password") {
                if password.is_none() { Error::Encrypted } else { Error::WrongPassword }
            } else {
                Error::Corrupt(e.to_string())
            }
        })?;
        if doc.is_encrypted() {
            // lopdf leaves the document marked encrypted when the key was not accepted.
            return Err(if password.is_none() { Error::Encrypted } else { Error::WrongPassword });
        }
        doc
    } else {
        probe
    };

    let mut collector = Collector::default();
    pdf_extract::output_doc(&doc, &mut collector).map_err(|e| Error::Corrupt(format!("{e:?}")))?;
    collector.flush();

    let doc = SpanDocument { pages: collector.pages };
    let has_text = doc.pages.iter().any(|p| p.spans.iter().any(|s| s.text.chars().any(|c| c.is_alphanumeric())));
    if !has_text {
        return Err(Error::NeedsOcr);
    }
    Ok(doc)
}

/// Receives glyphs from `pdf-extract` and merges them into spans.
#[derive(Default)]
struct Collector {
    pages: Vec<Page>,
    page_height: f64,
    flip: Option<Transform>,
    current: Option<Span>,
    /// Device-space x where the next glyph of the current span would start.
    next_x: f32,
}

impl Collector {
    fn flush(&mut self) {
        if let Some(mut span) = self.current.take() {
            let trimmed = span.text.trim();
            if !trimmed.is_empty() {
                span.text = trimmed.to_string();
                if let Some(page) = self.pages.last_mut() {
                    page.spans.push(span);
                }
            }
        }
    }
}

impl OutputDev for Collector {
    fn begin_page(&mut self, page_num: u32, media_box: &MediaBox, _art: Option<(f64, f64, f64, f64)>) -> Result<(), OutputError> {
        self.flush();
        self.page_height = media_box.ury - media_box.lly;
        self.flip = Some(Transform::row_major(1., 0., 0., -1., 0., self.page_height));
        self.pages.push(Page {
            number: page_num,
            width: (media_box.urx - media_box.llx) as f32,
            height: self.page_height as f32,
            spans: Vec::new(),
        });
        Ok(())
    }

    fn end_page(&mut self) -> Result<(), OutputError> {
        self.flush();
        if let Some(page) = self.pages.last_mut() {
            page.spans.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x0.total_cmp(&b.x0)));
        }
        Ok(())
    }

    fn output_character(&mut self, trm: &Transform, width: f64, spacing: f64, font_size: f64, ch: &str) -> Result<(), OutputError> {
        let flip = self.flip.unwrap_or_else(Transform::identity);
        let pos = trm.post_transform(&flip);
        let (x, y) = (pos.m31 as f32, pos.m32 as f32);
        let size_vec = trm.transform_vector(euclid_vec(font_size, font_size));
        let size = (size_vec.x * size_vec.y).abs().sqrt() as f32;
        let advance = trm.transform_vector(euclid_vec(width * font_size + spacing, 0.0)).x as f32;

        let is_space = ch.chars().all(char::is_whitespace);
        let gap_limit = size.max(1.0) * 0.8;
        let continues = match &self.current {
            Some(s) => (s.y - y).abs() < size.max(1.0) * 0.3 && (x - self.next_x).abs() < gap_limit,
            None => false,
        };
        if !continues {
            self.flush();
        }
        if is_space {
            // Spaces are kept inside spans; a run of them ends up as a gap, which
            // splits the span on the next glyph.
            if let Some(s) = self.current.as_mut() {
                if !s.text.ends_with(' ') {
                    s.text.push(' ');
                }
                self.next_x = x + advance;
            }
            return Ok(());
        }
        match self.current.as_mut() {
            Some(s) => {
                s.text.push_str(ch);
                s.x1 = x + advance;
            }
            None => {
                self.current = Some(Span { text: ch.to_string(), x0: x, x1: x + advance, y, font_size: size });
            }
        }
        self.next_x = x + advance;
        Ok(())
    }

    fn begin_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_word(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
    fn end_line(&mut self) -> Result<(), OutputError> {
        Ok(())
    }
}

fn euclid_vec(x: f64, y: f64) -> euclid::Vector2D<f64, pdf_extract::Space> {
    euclid::vec2(x, y)
}
